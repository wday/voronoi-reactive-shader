//! Delay Tap (`DlyT`) — the reader half of the v3 delay line.
//!
//! Reads a shared ring-buffer channel (delay-core, via pluglib) at a variable
//! offset and blends it against its own input (Buffer Mix). Read-only: used to
//! build `Tap → FX → Write` feedback loops with an FX insert. Multi-tap is
//! reserved for Stage 5; this skeleton reads a single tap.

mod params;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use params::{TapParams, NUM_PARAMS};
use pluglib::{api, OutputProgram, QuadGeometry};

pub struct DelayTap {
    params: TapParams,
    quad: Option<QuadGeometry>,
    output: Option<OutputProgram>,
    acquired_channel: usize,
    frame_count: u64,
}

impl DelayTap {
    fn draw_tap(
        &mut self,
        input_tex: GLuint,
        uv_scale: [f32; 2],
        host_fbo: GLint,
        host_viewport: [GLint; 4],
    ) {
        let api = api();
        let ch = self.params.channel();
        let tex = (api.tex)(ch);
        let wp = (api.write_pos)(ch);
        let buf_size = (api.buf_size)(ch);

        // Map Tap Offset onto a layer: 0 = newest (wp), 1 = oldest (loop end).
        let (read_pos, wet) = if buf_size == 0 || tex == 0 {
            (0u32, 0.0) // no buffer yet — pass input through (wet forced to 0)
        } else {
            let k = (self.params.tap_offset() * (buf_size - 1) as f32).round() as u32;
            let k = k.min(buf_size - 1);
            let read_pos = (wp + buf_size - k) % buf_size;
            (read_pos, self.params.buffer_mix())
        };

        let quad = self.quad.as_ref().unwrap();
        let output = self.output.as_ref().unwrap();
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(host_viewport[0], host_viewport[1], host_viewport[2], host_viewport[3]);
        }
        output.draw(quad, input_tex, uv_scale, tex, read_pos as f32, wet);
    }
}

impl SimpleFFGLInstance for DelayTap {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        (api().acquire)(0);

        Self {
            params: TapParams::new(),
            quad: None,
            output: None,
            acquired_channel: 0,
            frame_count: 0,
        }
    }

    fn draw(&mut self, data: &FFGLData, frame_data: GLInput) {
        if self.output.is_none() {
            self.quad = Some(QuadGeometry::new());
            self.output = Some(OutputProgram::new());
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
        let uv_scale = [width as f32 / hw_width as f32, height as f32 / hw_height as f32];

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

        self.draw_tap(input_tex, uv_scale, host_fbo, host_viewport);

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
                tap_offset = format!("{:.2}", self.params.tap_offset()),
                buffer_mix = format!("{:.2}", self.params.buffer_mix()),
                bpm = format!("{:.1}", data.host_beat.bpm),
                "delay-tap status"
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
                let api = api();
                (api.release)(self.acquired_channel);
                (api.acquire)(new_ch);
                self.acquired_channel = new_ch;
            }
        }
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"DlyT",
            name: *b"Delay Tap       ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Delay reader: taps a shared channel at an offset, blends against input".to_string(),
            description: "v3 delay line — Tap half (pairs with Delay Write)".to_string(),
        }
    }
}

impl Drop for DelayTap {
    fn drop(&mut self) {
        (api().release)(self.acquired_channel);
    }
}

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<DelayTap>);
