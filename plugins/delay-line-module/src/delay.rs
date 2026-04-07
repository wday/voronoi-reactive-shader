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
    /// Latched loop length in frames (Write mode only).
    /// Only recomputed when inputs change significantly.
    latched_delay: u32,
    last_bpm: f32,
    last_sync_mode: SyncMode,
    last_subdivision: f32,
    last_delay_ms: f32,
    last_delay_frames: u32,
}

impl DelayLine {
    /// Compute delay in frames from current params. Only called when inputs change.
    fn compute_delay_frames(&self, bpm: f32, max: u32) -> u32 {
        let d = match self.params.sync_mode() {
            SyncMode::Subdivision => {
                if bpm <= 0.0 {
                    return 30_u32.min(max);
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
        d.clamp(1, max)
    }

    /// Get latched delay, recomputing only when BPM or params actually change.
    fn delay_frames(&mut self, bpm: f32, max: u32) -> u32 {
        let sync = self.params.sync_mode();
        let subdivision = self.params.subdivision_beats();
        let delay_ms = self.params.delay_ms();
        let delay_raw = self.params.delay_frames_raw();

        let changed = sync != self.last_sync_mode
            || (bpm - self.last_bpm).abs() > 0.5
            || subdivision != self.last_subdivision
            || (delay_ms - self.last_delay_ms).abs() > 0.5
            || delay_raw != self.last_delay_frames
            || self.latched_delay == 0;

        if changed {
            self.latched_delay = self.compute_delay_frames(bpm, max);
            self.last_bpm = bpm;
            self.last_sync_mode = sync;
            self.last_subdivision = subdivision;
            self.last_delay_ms = delay_ms;
            self.last_delay_frames = delay_raw;
        }

        self.latched_delay.clamp(1, max)
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

    /// Read: output the oldest frame in the ring buffer.
    /// read_pos = (write_pos + 1) % buf_size — exactly loop_length frames old.
    fn draw_read(&mut self, host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();

        let buf_info = match registry::read_channel(channel) {
            Some(info) => info,
            None => {
                // No buffer yet — output black
                unsafe {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
                    gl::Viewport(
                        host_viewport[0], host_viewport[1],
                        host_viewport[2], host_viewport[3],
                    );
                    gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT);
                }
                return;
            }
        };

        let (buf_tex, write_pos, buf_size, _w, _h) = buf_info;
        let read_pos = (write_pos + 1) % buf_size;

        let shaders = self.shaders.as_ref().unwrap();

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.read_output_pass(buf_tex, read_pos as f32);
    }

    /// Write: write input into the buffer, output what was written.
    ///
    /// Write formula: decay * old + (1 - decay) * input
    ///   decay=0 → clean overwrite, decay=0.9 → long trails
    fn draw_write(&mut self, data: &FFGLData, input_tex: GLuint, width: u32, height: u32, hw_width: u32, hw_height: u32, host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();
        let loop_length = self.delay_frames(data.host_beat.bpm, registry::buffer_depth());
        let (buf_tex, fbo, write_pos) = registry::begin_frame_write(channel, loop_length, width, height);
        let shaders = self.shaders.as_ref().unwrap();
        let uv_scale = [
            width as f32 / hw_width as f32,
            height as f32 / hw_height as f32,
        ];
        let decay = self.params.decay();
        let write_layer = write_pos as f32;

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

        // frame[write_pos] = decay * frame[write_pos] + (1 - decay) * input
        shaders.fade_pass(buf_tex, write_layer, decay);
        unsafe {
            gl::Enable(gl::BLEND);
            let s = 1.0 - decay;
            gl::BlendColor(s, s, s, s);
            gl::BlendFunc(gl::CONSTANT_COLOR, gl::ONE);
        }
        shaders.write_pass(input_tex, uv_scale);
        unsafe {
            gl::Disable(gl::BLEND);
        }

        // Output to host FBO: render what we just wrote to the buffer
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.read_output_pass(buf_tex, write_layer);
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
            latched_delay: 0,
            last_bpm: 0.0,
            last_sync_mode: SyncMode::Subdivision,
            last_subdivision: 0.0,
            last_delay_ms: 0.0,
            last_delay_frames: 0,
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

        match self.params.mode() {
            Mode::Read => self.draw_read(host_fbo, host_viewport),
            Mode::Write => self.draw_write(data, input_tex, width, height, hw_width, hw_height, host_fbo, host_viewport),
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
                Mode::Read => "read",
                Mode::Write => "write",
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
                delay_frames = self.latched_delay,
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
            about: "Read/write delay line for modular feedback loops with fx insert".to_string(),
            description: "Beat-synced delay with shared buffer channels".to_string(),
        }
    }
}
