//! Delay Write (`DlyW`) — the record head of the v3 delay line.
//!
//! Records its input into a shared ring-buffer channel (delay-core, via pluglib)
//! and advances the tape one slot per frame. It is a **pure write head** — it
//! overwrites the slot and never reads the buffer:
//!
//!     tape[slot] = Send * input
//!
//! **Send** is the record/dub level (how hard the current frame commits — pulse
//! it, holding the Tap's Wet high, to pulse video echoes). All feedback and
//! mixing live in the Delay Tap read head (`out = Dry*source + Wet*tape`); the
//! loop gain around `source -> Tap -> FX -> Write` is `Send*Wet`. Its output is a
//! pure passthrough of the input — the record head is visually transparent; the
//! loop is monitored by a Delay Tap on the same Channel. A Write with no Tap
//! records into the aether.

mod params;
mod shader;

use std::time::{Instant, UNIX_EPOCH};

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use params::{SyncMode, WriteParams, NUM_PARAMS};
use pluglib::api;
use shader::WriteShaders;

/// Linear resolution scale for the stored tape. 1.0 = the tape is stored at the
/// full frame resolution, so nothing in the delay loop is resampled.
///
/// This was 0.5 (a quarter of the pixels) to hold a 240-layer tape near 1 GB per
/// channel. The cost only showed up in feedback: the downscale here plus the
/// Tap's bilinear magnify on read gave the loop a round-trip gain of roughly 0.35
/// at high spatial frequencies, so fine detail decayed about 3x faster per lap
/// than the image as a whole. Goopy fluid feedback liked that; tight fractal
/// feedback mushed into blobs within a few laps. With BUFFER_DEPTH cut to 120
/// (2 s at 60 fps — past the loop lengths this is played at) full res costs
/// ~2 GB/channel at 1080p, which fits in the VRAM budget.
///
/// Softening is now an intentional, sweepable effect on the Tap's read rather
/// than a property of the tape. Lower this only to buy VRAM back (e.g. at 4K).
const TAPE_SCALE: f32 = 1.0;

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
    /// Compute delay in frames from current params. Only called when inputs
    /// change. The conversion (incl. the BPM<=0 fallback, rounding and clamp)
    /// lives in `delay-dsp` so every branch is unit-tested (see delay-dsp timing
    /// tests); this just maps the local param values onto it.
    fn compute_delay_frames(&self, bpm: f32, max: u32) -> u32 {
        let mode = match self.params.sync_mode() {
            SyncMode::Subdivision => delay_dsp::SyncMode::Subdivision,
            SyncMode::Ms => delay_dsp::SyncMode::Ms,
            SyncMode::Frames => delay_dsp::SyncMode::Frames,
        };
        delay_dsp::delay_frames(
            mode,
            self.params.subdivision_beats(),
            self.params.delay_ms(),
            self.params.delay_frames_raw(),
            bpm,
            self.fps_estimate,
            max,
        )
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
        // Tape is stored at reduced resolution (TAPE_SCALE) + RGBA16F. Downscale
        // the sizing dims; the core allocates the tape at exactly these dims and is
        // otherwise resolution-agnostic (the Tap reads it with normalised uv +
        // linear filtering, so it upscales on read for free).
        let tape_w = ((width as f32 * TAPE_SCALE).round() as u32).max(1);
        let tape_h = ((height as f32 * TAPE_SCALE).round() as u32).max(1);
        // Tick the shared barrier once this frame: sizes/retunes the buffer and
        // advances the channel's monotonic frame_index (spec v0.2). Single writer
        // per channel, so the first-of-frame flag is unused.
        (api.frame_tick)(ch, loop_length, tape_w, tape_h, frame_id(data));
        let tex = (api.tex)(ch);
        let buf_size = (api.buf_size)(ch);
        let frame_index = (api.frame_index)(ch);

        let uv_scale = [width as f32 / hw_width as f32, height as f32 / hw_height as f32];
        let shaders = self.shaders.as_ref().unwrap();

        // Buffer allocation failed — pass the live input through untouched.
        if tex == 0 || buf_size == 0 {
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
                gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
            }
            shaders.output.draw(&shaders.quad, input_tex, uv_scale, 0, 0.0, 1.0, 0.0, 1.0);
            return;
        }

        // Write slot = frame_index mod buf_size (spec v0.2 absolute addressing).
        // Computed via delay-dsp so it stays reconciled with the Tap's read slot
        // and the unit tests. buf_size is stable across the whole frame, so this
        // slot is order-independent.
        let wp = delay_dsp::Ring::for_buffer(buf_size).write_slot(frame_index);

        // 1. Record into buffer[wp]:  tape[wp] = Send * input.  A pure write head:
        //    it overwrites the slot and never reads the buffer. ALL feedback and
        //    mixing lives in the Delay Tap (out = Dry*source + Wet*tape); the loop
        //    gain around source->Tap->FX->Write is Send*Wet. (This dropped the old
        //    `(1-Send)*Regen*old` term, which double-read the buffer — a leftover
        //    from the unified single-plugin delay; Regen is gone.) Blend stays
        //    disabled, so the record is a straight overwrite and Write touches no
        //    host blend state.
        let send = self.params.send();
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::FramebufferTextureLayer(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, tex, 0, wp as i32);
            // Viewport = tape (downscaled) dims, not the full frame. uv_scale still
            // maps to the full-res input, so this is a full-frame downsample into
            // the smaller layer (single bilinear tap; fine at 0.5×).
            gl::Viewport(0, 0, tape_w as i32, tape_h as i32);
        }
        shaders.write_pass(input_tex, uv_scale, send);

        // 2. Output = passthrough of the input. The record head is transparent;
        //    the loop is monitored by a Delay Tap on the same Channel.
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
        }
        shaders.output.draw(&shaders.quad, input_tex, uv_scale, 0, 0.0, 1.0, 0.0, 1.0);
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

        // Save host GL state (shared context with Resolume). The pure-overwrite
        // write head touches no blend state, so the restore only covers the
        // enable flags and the active-unit texture binding it does disturb (spec
        // v0.2 open-question #2: GL-state restore gap). GL_TEXTURE0's binding in
        // particular must be put back (project note: never leave the host's
        // unit-0 texture unbound).
        let mut host_fbo: GLint = 0;
        let mut host_viewport: [GLint; 4] = [0; 4];
        let scissor_was_on;
        let blend_was_on;
        let depth_was_on;
        let mut prev_active_tex: GLint = gl::TEXTURE0 as GLint;
        let mut prev_tex0: GLint = 0;
        unsafe {
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut host_fbo);
            gl::GetIntegerv(gl::VIEWPORT, host_viewport.as_mut_ptr());
            scissor_was_on = gl::IsEnabled(gl::SCISSOR_TEST) == gl::TRUE;
            blend_was_on = gl::IsEnabled(gl::BLEND) == gl::TRUE;
            depth_was_on = gl::IsEnabled(gl::DEPTH_TEST) == gl::TRUE;
            gl::GetIntegerv(gl::ACTIVE_TEXTURE, &mut prev_active_tex);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut prev_tex0);
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
            gl::Disable(gl::DEPTH_TEST);
        }

        self.draw_write(data, input_tex, width, height, hw_width, hw_height, host_fbo, host_viewport);

        // Restore host GL state (see the save comment above).
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, prev_tex0 as GLuint);
            gl::ActiveTexture(prev_active_tex as GLenum);
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
            about: "Delay record head: writes Send*input to a shared channel (pure write head), passes input through".to_string(),
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
