//! Delay Write (`DlyW`) — the record head of the v3 delay line.
//!
//! Records its input into a shared ring-buffer channel (delay-core, via pluglib)
//! and advances the tape one slot per frame. The record is additive-with-decay:
//!
//!     tape[slot] = Regen * old  +  Send * input
//!
//! where `old` is this slot's loop-old content. **Regen** is the decay rate (the
//! ring-out tail); **Send** is the dub throw (how hard the current frame commits
//! — pulse it, holding the Tap's Wet high, to pulse video echoes). Its output is
//! a pure passthrough of the input — the record head is visually transparent;
//! the loop is monitored by a Delay Tap on the same Channel. A Write with no Tap
//! records into the aether. Single writer per channel keeps the accumulation a
//! trivial read-modify-write (no multi-writer barrier).

mod params;
mod shader;

use std::time::{Instant, UNIX_EPOCH};

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use params::{SyncMode, WriteParams, NUM_PARAMS};
use pluglib::api;
use shader::WriteShaders;

/// Host-provided per-frame id for the frame barrier: host_time as whole ms.
/// Shared by every instance drawn in the same host frame (if Resolume sets it).
fn frame_id(data: &FFGLData) -> u64 {
    data.host_time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct DelayWrite {
    params: WriteParams,
    shaders: Option<WriteShaders>,
    /// This plugin's own FBO; the shared buffer layer is attached per draw.
    fbo: GLuint,
    /// Channel currently held via delay-core refcount (kept in sync on change).
    acquired_channel: usize,
    fps_estimate: f32,
    last_frame_time: Option<Instant>,
    frame_count: u64,
    latched_delay: u32,
    last_bpm: f32,
    last_sync_mode: SyncMode,
    last_subdivision: f32,
    last_delay_ms: f32,
    last_delay_frames: u32,
}

impl DelayWrite {
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
            SyncMode::Frames => self.params.delay_frames_raw(),
        };
        d.clamp(1, max)
    }

    /// Latched delay, recomputed only when BPM or timing params actually change.
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

    #[allow(clippy::too_many_arguments)]
    fn draw_write(
        &mut self,
        data: &FFGLData,
        input_tex: GLuint,
        width: u32,
        height: u32,
        hw_width: u32,
        hw_height: u32,
        host_fbo: GLint,
        host_viewport: [GLint; 4],
    ) {
        let api = api();
        let ch = self.params.channel();
        let loop_length = self.delay_frames(data.host_beat.bpm, (api.buffer_depth)());
        // Advance the tape once this frame (single writer: barrier flag unused).
        (api.begin_frame_write)(ch, loop_length, width, height, frame_id(data));
        let tex = (api.tex)(ch);
        let wp = (api.write_pos)(ch);
        let buf_size = (api.buf_size)(ch);

        let uv_scale = [width as f32 / hw_width as f32, height as f32 / hw_height as f32];
        let shaders = self.shaders.as_ref().unwrap();

        // Buffer allocation failed — pass the live input through untouched.
        if tex == 0 || buf_size == 0 {
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
                gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
            }
            shaders.output.draw(&shaders.quad, input_tex, uv_scale, 0, 0.0, 1.0, 0.0);
            return;
        }

        // 1. Record into buffer[wp]:  tape[wp] = Regen*old + Send*input.
        //    `old` is this slot's loop-old content (single writer). The fade
        //    pre-pass scales it in place by Regen; the additive write then adds
        //    Send*input via a CONSTANT_COLOR=Send, dst=ONE blend. A Delay Tap
        //    upstream reads this same slot pre-overwrite (one full lap back).
        let regen = self.params.regen();
        let send = self.params.send();
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::FramebufferTextureLayer(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, tex, 0, wp as i32);
            gl::Viewport(0, 0, width as i32, height as i32);
        }
        shaders.fade_pass(tex, wp as f32, regen);
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendColor(send, send, send, send);
            gl::BlendFunc(gl::CONSTANT_COLOR, gl::ONE);
        }
        shaders.write_pass(input_tex, uv_scale);
        unsafe {
            gl::Disable(gl::BLEND);
        }

        // 2. Output = passthrough of the input. The record head is transparent;
        //    the loop is monitored by a Delay Tap on the same Channel.
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
        }
        shaders.output.draw(&shaders.quad, input_tex, uv_scale, 0, 0.0, 1.0, 0.0);
    }
}

impl SimpleFFGLInstance for DelayWrite {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        // Register the default channel with delay-core (no GL; safe here).
        (api().acquire)(0);

        Self {
            params: WriteParams::new(),
            shaders: None,
            fbo: 0,
            acquired_channel: 0,
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
            self.shaders = Some(WriteShaders::new());
            unsafe {
                gl::GenFramebuffers(1, &mut self.fbo);
            }
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

        // Save host GL state (shared context with Resolume).
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

        self.draw_write(data, input_tex, width, height, hw_width, hw_height, host_fbo, host_viewport);

        // Restore host GL state.
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            if scissor_was_on { gl::Enable(gl::SCISSOR_TEST); }
            if blend_was_on { gl::Enable(gl::BLEND); }
            if depth_was_on { gl::Enable(gl::DEPTH_TEST); }
        }

        self.frame_count += 1;
        if self.frame_count % 300 == 0 {
            tracing::info!(
                frame = self.frame_count,
                channel = self.params.channel() + 1,
                delay_frames = self.latched_delay,
                regen = format!("{:.2}", self.params.regen()),
                send = format!("{:.2}", self.params.send()),
                fps = format!("{:.1}", self.fps_estimate),
                bpm = format!("{:.1}", data.host_beat.bpm),
                tex_w = width, tex_h = height,
                "delay-write status"
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
        // Keep the delay-core refcount attached to the channel we actually use.
        if index == params::PARAM_CHANNEL {
            let new_ch = self.params.channel();
            if new_ch != self.acquired_channel {
                let api = api();
                (api.release)(self.acquired_channel);
                (api.acquire)(new_ch);
                self.acquired_channel = new_ch;
            }
        }
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"DlyW",
            name: *b"Delay Write     ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Delay record head: records input to a shared channel (Regen decay + Send throw), passes input through".to_string(),
            description: "v3 delay line — Write head (pairs with Delay Tap on the same Channel)".to_string(),
        }
    }
}

impl Drop for DelayWrite {
    fn drop(&mut self) {
        // Release the channel refcount. No GL here (context may not be current);
        // the FBO name leaks until process exit, matching the shipped v2 plugin.
        (api().release)(self.acquired_channel);
    }
}

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<DelayWrite>);
