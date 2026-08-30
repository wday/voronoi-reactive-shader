//! Varispeed Write (`VsWr`) — the record head of the Varispeed atom.
//!
//! Records its input into the shared full-ring tape (varispeed-core, via pluglib)
//! at one frame per host frame: `tape[record_index mod depth] = Send * input`. It
//! is a pure write head (overwrite, never reads the buffer) and its video output is
//! a transparent passthrough — the loop is monitored by a Varispeed Read on the
//! same tape.
//!
//! **Send = 0 = freeze.** While recording (Send > 0) it calls `vc_write_tick`,
//! which advances the record cursor and sizes the tape. At Send = 0 it skips the
//! tick entirely, so the cursor parks and the buffer is held — the Read then loops
//! the captured window (varispeed playback). This is NOT the delay's Send=0 wipe.
//!
//! The tape is stored at full resolution (`TAPE_SCALE` = 1.0) + RGBA16F
//! (VS-STORAGE); the passthrough output is full-res too.

mod params;
mod shader;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use params::{WriteParams, NUM_PARAMS};
use pluglib::vc_api;
use shader::WriteShaders;

/// Linear resolution scale for the stored tape. 1.0 = stored at the full frame
/// resolution, so nothing in the loop is spatially resampled.
///
/// Was 0.5 (a quarter of the pixels) to keep the 480-layer tape near 2 GB. The
/// cost only showed up in feedback: that downscale plus the Read's bilinear
/// magnify gave the loop a round-trip gain of roughly 0.35 at high spatial
/// frequencies, so fine detail decayed ~3x faster per lap than the image as a
/// whole and tight fractal feedback mushed into blobs. Paid for by halving
/// `BUFFER_DEPTH` to 240 (VS-STORAGE / WRITE-TAPE-SCALE).
///
/// NOTE: this only removes the *spatial* per-lap loss. `read.frag.glsl` also does
/// a temporal `mix()` between bracketing frames, which is a second lowpass
/// whenever the read head sits between frames (Rate ±1/2x, or any Warp Depth > 0).
const TAPE_SCALE: f32 = 1.0;

/// Host-provided per-frame id for the write-cursor barrier: host_time as whole ms.
fn frame_id(data: &FFGLData) -> u64 {
    data.host_time
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct VarispeedWrite {
    params: WriteParams,
    shaders: Option<WriteShaders>,
    /// This plugin's own FBO; the shared buffer layer is attached per draw.
    fbo: GLuint,
    frame_count: u64,
}

impl VarispeedWrite {
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
        let vc = vc_api();
        let uv_scale = [width as f32 / hw_width as f32, height as f32 / hw_height as f32];
        let send = self.params.send();
        let shaders = self.shaders.as_ref().unwrap();

        // Record only while Send > 0. At Send = 0 we skip the tick entirely: the
        // record cursor parks and the buffer is kept — that IS the freeze (the Read
        // then loops the captured window).
        if send > 0.0 {
            // Tape stored at full resolution (TAPE_SCALE = 1.0) RGBA16F; these are
            // the sizing dims varispeed-core allocs the tape at exactly.
            let tape_w = ((width as f32 * TAPE_SCALE).round() as u32).max(1);
            let tape_h = ((height as f32 * TAPE_SCALE).round() as u32).max(1);
            let fid = frame_id(data);
            let record_index = (vc.write_tick)(tape_w, tape_h, fid);
            let tex = (vc.tex)();
            let depth = (vc.depth)();

            if tex != 0 && depth != 0 {
                // Confine mode: the Read published the slot its play head sits on —
                // record the FX'd input back into it (in-place loop feedback). Free
                // mode (no slot published this frame): append at the write cursor.
                let confined = (vc.loop_slot)(fid);
                let wp = if confined >= 0 {
                    (confined as u32) % depth
                } else {
                    (record_index % depth as u64) as u32
                };
                unsafe {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
                    gl::FramebufferTextureLayer(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, tex, 0, wp as i32);
                    // Viewport = tape (downscaled) dims; uv_scale still maps to the
                    // full-res input → a full-frame downsample into the smaller layer.
                    gl::Viewport(0, 0, tape_w as i32, tape_h as i32);
                }
                shaders.write_pass(input_tex, uv_scale, send);
            }
        }

        // Output = passthrough of the input (full-res). Transparent recorder.
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
        }
        shaders.output.draw(&shaders.quad, input_tex, uv_scale, 0, 0.0, 1.0, 0.0, 1.0);
    }
}

impl SimpleFFGLInstance for VarispeedWrite {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        (vc_api().acquire)();

        Self {
            params: WriteParams::new(),
            shaders: None,
            fbo: 0,
            frame_count: 0,
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

        // Save host GL state (shared context with Resolume). Pure-overwrite write
        // head touches no blend state; restore the enable flags and the active-unit
        // texture binding it disturbs (never leave the host's unit-0 texture unbound).
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
                send = format!("{:.2}", self.params.send()),
                recording = self.params.send() > 0.0,
                tex_w = width, tex_h = height,
                "varispeed-write status"
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
            unique_id: *b"VsWr",
            name: *b"Varispeed Write ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Varispeed record head: full-ring recorder; Send=0 freezes the loop for varispeed playback".to_string(),
            description: "Varispeed — Write head (pairs with Varispeed Read on the shared tape)".to_string(),
        }
    }
}

impl Drop for VarispeedWrite {
    fn drop(&mut self) {
        // Release the tape refcount. No GL here (context may not be current); the
        // FBO name leaks until process exit, matching the delay plugins.
        (vc_api().release)();
    }
}

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<VarispeedWrite>);
