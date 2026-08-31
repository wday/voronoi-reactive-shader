//! Varispeed Read (`VsRd`) — the playback head of the Varispeed atom.
//!
//! Floats a fractional read position over the shared full-ring tape
//! (varispeed-core, via pluglib) and plays it at a variable Rate — forwards,
//! backwards, slower, faster, or freeze-frame — with sub-frame interpolation and a
//! bar-locked Doppler warp. All the variable-rate/time logic is the pure
//! `varispeed-dsp` (age model): `age` = frames behind the live write cursor,
//! advanced each frame by `dr - rate` where `dr = record_index - prev`. Because
//! `dr` is 0 while the Write is frozen (Send=0) and 1 while recording, **freeze is
//! implicit** — the Read never needs to know the Write's state.
//!
//! Output = `blend(Dry*live, Wet*loop)` in the selected Blend Space (Linear
//! default), the loop being the interpolated fractional read. Place the Tap-style
//! read first: `source → Varispeed Read → FX… → Varispeed Write`.

mod params;
mod shader;

use std::time::{Instant, UNIX_EPOCH};

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use params::{LoopMode, ReadParams, SyncMode, NUM_PARAMS};
use pluglib::vc_api;
use shader::ReadShaders;
use varispeed_dsp::{advance_age, advance_head, anchor_age, confined_slot, sample_age, sample_confined, warp_offset};

/// 4/4 assumption for subdivision → cycles-per-bar (see VS-DOPPLER note).
const BEATS_PER_BAR: f32 = 4.0;

fn frame_id(data: &FFGLData) -> u64 {
    data.host_time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct VarispeedRead {
    params: ReadParams,
    shaders: Option<ReadShaders>,
    frame_count: u64,

    // fps estimate + latched loop length (mirrors DlyW's time handling).
    fps_estimate: f32,
    last_frame_time: Option<Instant>,
    latched_loop: u32,
    last_bpm: f32,
    last_sync_mode: SyncMode,
    last_subdivision: f32,
    last_loop_ms: f32,
    last_loop_frames: u32,

    // Read-head state. `age` = free-float model (frames behind live); `head` =
    // confined-loop play position over the fixed [0, loop) window.
    age: f64,
    head: f64,
    prev_record_index: u64,
    have_prev: bool,

    // VS-ANCHOR seeding state: the loop length `age` was last seeded for, and the
    // mode last drawn in, so Free re-anchors on a length change and on entry to
    // Free. `None` = never drawn, so the first Free frame seeds.
    seeded_len: u32,
    last_mode: Option<LoopMode>,

    // The channel `vc_acquire` was called for, so a Channel change can rebalance
    // the refcount (mirrors DelayTap).
    acquired_channel: u32,
}

impl VarispeedRead {
    fn compute_loop_frames(&self, bpm: f32, max: u32) -> u32 {
        let mode = match self.params.sync_mode() {
            SyncMode::Subdivision => delay_dsp::SyncMode::Subdivision,
            SyncMode::Ms => delay_dsp::SyncMode::Ms,
            SyncMode::Frames => delay_dsp::SyncMode::Frames,
        };
        delay_dsp::delay_frames(
            mode,
            self.params.subdivision_beats(),
            self.params.loop_ms(),
            self.params.loop_frames_raw(),
            bpm,
            self.fps_estimate,
            max,
        )
    }

    /// Latched loop length, recomputed only when an input actually changes.
    fn loop_frames(&mut self, bpm: f32, max: u32) -> u32 {
        let sync = self.params.sync_mode();
        let subdivision = self.params.subdivision_beats();
        let loop_ms = self.params.loop_ms();
        let loop_raw = self.params.loop_frames_raw();

        let changed = sync != self.last_sync_mode
            || (bpm - self.last_bpm).abs() > 0.5
            || subdivision != self.last_subdivision
            || (loop_ms - self.last_loop_ms).abs() > 0.5
            || loop_raw != self.last_loop_frames
            || self.latched_loop == 0;

        if changed {
            self.latched_loop = self.compute_loop_frames(bpm, max);
            self.last_bpm = bpm;
            self.last_sync_mode = sync;
            self.last_subdivision = subdivision;
            self.last_loop_ms = loop_ms;
            self.last_loop_frames = loop_raw;
        }
        self.latched_loop.clamp(1, max)
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
    fn draw_read(
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
        let vc = vc_api();
        let ch = self.params.channel();
        let depth = (vc.depth)();
        let record_index = (vc.record_index)(ch);
        let tex = (vc.tex)(ch);

        let mode = self.params.loop_mode();

        // Loop length (read window) in frames, capped at the N+1 stitch: the newest
        // slot belongs to the writer, so the deepest tap is `depth - 1` (VS-CAPACITY).
        let max_len = depth.saturating_sub(1).max(1);
        let loop_len = self.loop_frames(data.host_beat.bpm, max_len);

        // VS-ANCHOR. Free's `age` is a fixed point at Rate 1x (`dr - rate == 0`), so
        // without seeding it would sit at its initial 0 — a 1-frame feedback loop
        // with Loop Length inert. Seeding it to `loop_len - 1` makes Loop Length the
        // tap time: directly settable, patch-recallable, and different per instance
        // so N Free Reads on one channel are N distinct taps (VS-MULTITAP).
        if mode == LoopMode::Free
            && (self.last_mode != Some(LoopMode::Free) || self.seeded_len != loop_len)
        {
            self.age = anchor_age(loop_len);
        }
        self.seeded_len = loop_len;
        self.last_mode = Some(mode);

        let rate = self.params.rate() as f64;

        // Bar-locked Doppler warp offset (frames).
        let warp = warp_offset(
            data.host_beat.barPhase,
            self.params.warp_rate_beats(),
            BEATS_PER_BAR,
            self.params.warp_depth_frames(),
        ) as f64;

        // Advance the relevant play accumulator and resolve the fractional read.
        let sample = match mode {
            LoopMode::Confined => {
                // Confined loop: fixed [0, loop_len) window; the head plays at `rate`.
                // Publish floor(head) so the Write records the FX'd output back into
                // the slot just read → in-place accumulating feedback, no ring reset.
                self.head = advance_head(self.head, rate, loop_len);
                (vc.set_loop_slot)(ch, confined_slot(self.head, loop_len), frame_id(data));
                sample_confined(self.head + warp, loop_len, depth)
            }
            LoopMode::Free => {
                // Free float: age anchored to the moving write cursor. dr = how far the
                // cursor moved (0 frozen, 1 recording) — freeze is implicit.
                if !self.have_prev {
                    self.prev_record_index = record_index;
                    self.have_prev = true;
                }
                let dr = record_index.saturating_sub(self.prev_record_index) as f64;
                self.prev_record_index = record_index;
                self.age = advance_age(self.age, dr, rate, loop_len);
                sample_age(record_index, self.age, warp, loop_len, depth)
            }
        };

        let uv_scale = [width as f32 / hw_width as f32, height as f32 / hw_height as f32];
        let shaders = self.shaders.as_ref().unwrap();
        let (dry, wet, gamma) = (self.params.dry(), self.params.wet(), self.params.gamma());

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
        }

        if tex == 0 || depth == 0 {
            // No tape yet: pass the live source through (Wet forced to 0).
            shaders.draw(input_tex, uv_scale, 0, [0.0; 4], 0.0, dry, 0.0, gamma);
        } else {
            let layers = [
                sample.prev as f32,
                sample.layer0 as f32,
                sample.layer1 as f32,
                sample.next as f32,
            ];
            shaders.draw(input_tex, uv_scale, tex, layers, sample.frac, dry, wet, gamma);
        }
    }
}

impl SimpleFFGLInstance for VarispeedRead {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        (vc_api().acquire)(0);

        Self {
            params: ReadParams::new(),
            shaders: None,
            frame_count: 0,
            fps_estimate: 60.0,
            last_frame_time: None,
            latched_loop: 0,
            last_bpm: 0.0,
            last_sync_mode: SyncMode::Subdivision,
            last_subdivision: 0.0,
            last_loop_ms: 0.0,
            last_loop_frames: 0,
            age: 0.0,
            head: 0.0,
            prev_record_index: 0,
            have_prev: false,
            seeded_len: 0,
            last_mode: None,
            acquired_channel: 0,
        }
    }

    fn draw(&mut self, data: &FFGLData, frame_data: GLInput) {
        if self.shaders.is_none() {
            self.shaders = Some(ReadShaders::new());
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

        // Save host GL state (shared context). The read pass binds textures on units
        // 0/1 and leaves unit 0 cleared; restore the active unit + unit-0 binding and
        // the enable flags. No blend state touched.
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

        self.draw_read(data, input_tex, width, height, hw_width, hw_height, host_fbo, host_viewport);

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
                loop_frames = self.latched_loop,
                rate = format!("{:.2}", self.params.rate()),
                age = format!("{:.1}", self.age),
                dry = format!("{:.2}", self.params.dry()),
                wet = format!("{:.2}", self.params.wet()),
                bpm = format!("{:.1}", data.host_beat.bpm),
                "varispeed-read status"
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
        if index == params::PARAM_CHANNEL {
            let new_ch = self.params.channel();
            if new_ch != self.acquired_channel {
                let vc = vc_api();
                (vc.release)(self.acquired_channel);
                (vc.acquire)(new_ch);
                self.acquired_channel = new_ch;
            }
        }
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"VsRd",
            name: *b"Varispeed Read  ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Varispeed playback head: variable-rate/freeze loop over a shared tape. Place first: Read->FX->Write".to_string(),
            description: "Varispeed — Read head (pairs with Varispeed Write on the shared tape)".to_string(),
        }
    }
}

impl Drop for VarispeedRead {
    fn drop(&mut self) {
        (vc_api().release)(self.acquired_channel);
    }
}

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<VarispeedRead>);
