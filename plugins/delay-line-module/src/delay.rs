use std::time::Instant;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, DelayParams, Mode, SyncMode, NUM_PARAMS};
use crate::registry;
use crate::shader::DelayShaders;

pub struct DelayLine {
    params: DelayParams,
    shaders: Option<DelayShaders>,
    fps_estimate: f32,
    last_frame_time: Option<Instant>,
    frame_count: u64,
}

impl DelayLine {
    /// Convert current delay settings into a frame offset for the circular buffer.
    ///
    /// - Called from all three modes (`Send`, `Receive`, `Tap`) to translate the
    ///   plugin's user-configured delay into an integral frame index.
    /// - Uses `sync_mode`:
    ///     * `Subdivision` -> beat sync via current `bpm` (needs host beat info)
    ///     * `Ms` -> time-based delay in milliseconds
    ///     * `Frames` -> direct frame count from user parameter
    /// - Clamps into `[1, registry::buffer_depth() - 1]` to avoid zero or out-of-buffer wrap.
    ///
    /// Assumptions:
    /// - `bpm` is valid and positive for subdivision mode; if <=0 it falls back to 30 frames.
    /// - `fps_estimate` has been periodically refreshed by `update_fps()`.
    /// - `registry::buffer_depth()` reflects currently allocated delay depth.
    fn delay_frames(&self, bpm: f32) -> u32 {
        let d = match self.params.sync_mode() {
            SyncMode::Subdivision => {
                if bpm <= 0.0 {
                    return 30;
                }
                let beat_duration = 60.0 / bpm;
                let delay_secs = self.params.subdivision_beats() * beat_duration;
                (delay_secs * self.fps_estimate).round() as u32
            }
            SyncMode::Ms => {
                let delay_secs = self.params.delay_ms() / 1000.0;
                (delay_secs * self.fps_estimate).round() as u32
            }
            SyncMode::Frames => {
                self.params.delay_frames_raw()
            }
        };
        d.clamp(1, registry::buffer_depth() - 1)
    }

    fn update_fps(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame_time {
            let delta = now.duration_since(last).as_secs_f32();
            if delta > 0.0 && delta < 1.0 {
                let instant_fps = 1.0 / delta;
                self.fps_estimate += 0.05 * (instant_fps - self.fps_estimate);
            }
        }
        self.last_frame_time = Some(now);
    }

    /// Render a send-pass into the delay buffer and mix output back to host frame buffer.
    ///
    /// Purpose:
    /// - Write the current input frame into the plugin’s circular delay buffer.
    /// - Optionally apply decay/fade/blend behavior on first use, then accumulate feedback.
    /// - Finally, restore rendering to the host FBO and output the delayed result or crossfade with live input.
    ///
    /// When:
    /// - Called in `Mode::Send` from `draw()` every frame.
    ///
    /// Assumptions:
    /// - `self.shaders` is initialized and `DelayShaders` are ready.
    /// - `host_fbo` + `host_viewport` represent host context state saved earlier.
    /// - `registry::begin_frame_write()` yields valid buffer texture layer and `write_pos`.
    /// - `delay_frames(data.host_beat.bpm)` reflects user mode + BPM sync and is in-bounds.
    ///
    /// Behavior:
    /// - Binds local delay FBO, renders input into current write layer.
    /// - If transfer is first in channel and no decay: simple overwrite.
    /// - Otherwise uses blend and fade modes to create feedback drop-in.
    /// - Returns control to host FBO and passes input through at `passthrough` level (0=black).
    fn draw_send(&mut self, data: &FFGLData, input_tex: GLuint, width: u32, height: u32, hw_width: u32, hw_height: u32, host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();
        let (buf_tex, fbo, write_pos, is_first) = registry::begin_frame_write(channel, width, height);
        let depth = registry::buffer_depth();
        let d = self.delay_frames(data.host_beat.bpm);
        // Circular buffer read index: we want the frame `d` behind write_pos.
        // With math it is write_pos - d, but for wrapping and unsigned safety we add one full cycle first.
        // This is like music intervals: moving up a fifth is complementary to moving down a fourth.
        //   write_pos + depth - d  -- one cycle above then back `d` steps, equivalent to backwards `d`
        //   % depth                -- wrap into [0, depth-1]
        // Example: depth=5, write_pos=0, d=1 -> (0+5-1)%5 = 4 (back one step)
        let read_layer = ((write_pos + depth - d) % depth) as f32;
        let shaders = self.shaders.as_ref().unwrap();
        let uv_scale = [
            width as f32 / hw_width as f32,
            height as f32 / hw_height as f32,
        ];
        let decay = self.params.decay();

        // Bind buffer[write_pos] as render target
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTextureLayer(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                buf_tex,
                0,
                write_pos as i32,
            );
            gl::Viewport(0, 0, width as i32, height as i32);
        }

        if is_first && decay <= 0.0 {
            // Clean overwrite — seed / backward compatible path
            shaders.write_pass(input_tex, uv_scale);
        } else if is_first {
            // First Send with decay: fade previous iteration, then additive write
            shaders.fade_pass(buf_tex, read_layer, decay);
            unsafe {
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::ONE, gl::ONE);
            }
            shaders.write_pass(input_tex, uv_scale);
            unsafe {
                gl::Disable(gl::BLEND);
            }
        } else {
            // Subsequent Send in same frame: additive blend
            unsafe {
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::ONE, gl::ONE);
            }
            shaders.write_pass(input_tex, uv_scale);
            unsafe {
                gl::Disable(gl::BLEND);
            }
        }

        // Output to host FBO: pass input through at passthrough level (0=black).
        // Delayed reads are the responsibility of Tap/Receive nodes downstream.
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.passthrough_pass(input_tex, self.params.passthrough(), uv_scale);
    }

    fn draw_receive(&mut self, data: &FFGLData, input_tex: GLuint, uv_scale: [f32; 2], host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();
        let shaders = self.shaders.as_ref().unwrap();

        // If no Send has written to this channel yet, passthrough
        let buf_info = match registry::read_channel(channel) {
            Some(info) => info,
            None => {
                unsafe {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
                    gl::Viewport(
                        host_viewport[0], host_viewport[1],
                        host_viewport[2], host_viewport[3],
                    );
                }
                shaders.write_pass(input_tex, uv_scale);
                return;
            }
        };

        let (buf_tex, write_pos, _buf_w, _buf_h) = buf_info;
        let depth = registry::buffer_depth();
        let d = self.delay_frames(data.host_beat.bpm);
        let read_layer = ((write_pos + depth - d) % depth) as f32;
        let feedback = self.params.feedback();

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.receive_pass(input_tex, buf_tex, read_layer, feedback, uv_scale);
    }

    fn draw_tap(&mut self, data: &FFGLData, uv_scale: [f32; 2], host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();
        let shaders = self.shaders.as_ref().unwrap();

        // If no Send has written to this channel yet, output black
        let buf_info = match registry::read_channel(channel) {
            Some(info) => info,
            None => {
                unsafe {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
                    gl::Viewport(
                        host_viewport[0], host_viewport[1],
                        host_viewport[2], host_viewport[3],
                    );
                    gl::ClearColor(0.0, 0.0, 0.0, 0.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                }
                return;
            }
        };

        let (buf_tex, write_pos, _buf_w, _buf_h) = buf_info;
        let depth = registry::buffer_depth();
        let d = self.delay_frames(data.host_beat.bpm);
        let read_layer = ((write_pos + depth - d) % depth) as f32;

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.read_pass(buf_tex, read_layer, uv_scale);
    }
}

impl SimpleFFGLInstance for DelayLine {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: DelayParams::new(),
            shaders: None,
            fps_estimate: 60.0,
            last_frame_time: None,
            frame_count: 0,
        }
    }

    fn draw(&mut self, data: &FFGLData, frame_data: GLInput) {
        if self.shaders.is_none() {
            self.shaders = Some(DelayShaders::new());
        }

        let input_tex = if !frame_data.textures.is_empty() {
            frame_data.textures[0].Handle as GLuint
        } else {
            0
        };

        let (width, height, hw_width, hw_height) = if !frame_data.textures.is_empty() {
            (
                frame_data.textures[0].Width,
                frame_data.textures[0].Height,
                frame_data.textures[0].HardwareWidth,
                frame_data.textures[0].HardwareHeight,
            )
        } else {
            (1920, 1080, 1920, 1080)
        };

        self.update_fps();

        // Save host GL state
        let mut host_fbo: GLint = 0;
        let mut host_viewport: [GLint; 4] = [0; 4];
        let scissor_was_on;
        let blend_was_on;
        let depth_was_on;
        unsafe {
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut host_fbo);
            gl::GetIntegerv(gl::VIEWPORT, host_viewport.as_mut_ptr());
            scissor_was_on = gl::IsEnabled(gl::SCISSOR_TEST) == gl::TRUE;
            blend_was_on = gl::IsEnabled(gl::BLEND) == gl::TRUE;
            depth_was_on = gl::IsEnabled(gl::DEPTH_TEST) == gl::TRUE;
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
            gl::Disable(gl::DEPTH_TEST);
        }

        let uv_scale = [
            width as f32 / hw_width as f32,
            height as f32 / hw_height as f32,
        ];

        match self.params.mode() {
            Mode::Send => self.draw_send(data, input_tex, width, height, hw_width, hw_height, host_fbo, host_viewport),
            Mode::Receive => {
                if input_tex != 0 {
                    self.draw_receive(data, input_tex, uv_scale, host_fbo, host_viewport);
                }
            }
            Mode::Tap => self.draw_tap(data, uv_scale, host_fbo, host_viewport),
        }

        // Restore host GL state
        unsafe {
            if scissor_was_on { gl::Enable(gl::SCISSOR_TEST); }
            if blend_was_on { gl::Enable(gl::BLEND); }
            if depth_was_on { gl::Enable(gl::DEPTH_TEST); }
        }

        self.frame_count += 1;
        if self.frame_count % 300 == 0 {
            let mode_str = match self.params.mode() {
                Mode::Send => "send",
                Mode::Receive => "receive",
                Mode::Tap => "tap",
            };
            let sync_str = match self.params.sync_mode() {
                SyncMode::Subdivision => "subdivision",
                SyncMode::Ms => "ms",
                SyncMode::Frames => "frames",
            };
            tracing::info!(
                frame = self.frame_count,
                mode = mode_str,
                channel = self.params.channel() + 1,
                sync = sync_str,
                fps = format!("{:.1}", self.fps_estimate),
                bpm = format!("{:.1}", data.host_beat.bpm),
                delay_frames = self.delay_frames(data.host_beat.bpm),
                feedback = format!("{:.2}", self.params.feedback()),
                passthrough = format!("{:.2}", self.params.passthrough()),
                decay = format!("{:.2}", self.params.decay()),
                tex_w = width, tex_h = height,
                hw_w = hw_width, hw_h = hw_height,
                vp = format!("{:?}", host_viewport),
                "status"
            );
        }
    }

    fn num_params() -> usize {
        NUM_PARAMS
    }

    fn param_info(index: usize) -> &'static dyn ffgl_core::parameters::ParamInfo {
        params::param_info(index)
    }

    fn get_param(&self, index: usize) -> f32 {
        self.params.get(index)
    }

    fn set_param(&mut self, index: usize, value: f32) {
        self.params.set(index, value);
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"DLMd",
            name: *b"Delay Line      ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Send/receive/tap delay line for modular feedback loops".to_string(),
            description: "Beat-synced delay with shared buffer channels".to_string(),
        }
    }
}
