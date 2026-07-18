use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, ContourParams, NUM_PARAMS};
use crate::shader::{ContourShader, ContourUniforms};

/// Free-running clock: u_time advances 1/ASSUMED_FPS per frame (seconds). The
/// shader's drift_offset() turns it into cyclic per-axis motion, so exact fps
/// doesn't matter — Drift X/Y are taste knobs.
const ASSUMED_FPS: f32 = 60.0;

pub struct ContourField {
    params: ContourParams,
    shader: Option<ContourShader>,
    phase: f32,
}

impl SimpleFFGLInstance for ContourField {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: ContourParams::new(),
            shader: None,
            phase: 0.0,
        }
    }

    fn draw(&mut self, _data: &FFGLData, frame_data: GLInput) {
        // GL context exists only while drawing → build lazily.
        if self.shader.is_none() {
            self.shader = Some(ContourShader::new());
        }

        // Resolution from the current viewport, so aspect is correct whether the
        // host hands us an input texture or not (this is a generator — it renders
        // either way, unlike the effect plugins that bail to black on no input).
        let texel_size = unsafe {
            let mut vp = [0i32; 4];
            gl::GetIntegerv(gl::VIEWPORT, vp.as_mut_ptr());
            let w = vp[2].max(1) as f32;
            let h = vp[3].max(1) as f32;
            [1.0 / w, 1.0 / h]
        };

        // Optional input (ignored by the shader); bind it if present, else 0.
        let input_tex = if !frame_data.textures.is_empty() {
            frame_data.textures[0].Handle as GLuint
        } else {
            0
        };

        // Advance the geological drift.
        self.phase += 1.0 / ASSUMED_FPS;

        // Save host GL state.
        let scissor_was_on;
        let blend_was_on;
        let depth_was_on;
        unsafe {
            scissor_was_on = gl::IsEnabled(gl::SCISSOR_TEST) == gl::TRUE;
            blend_was_on = gl::IsEnabled(gl::BLEND) == gl::TRUE;
            depth_was_on = gl::IsEnabled(gl::DEPTH_TEST) == gl::TRUE;
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
            gl::Disable(gl::DEPTH_TEST);
        }

        let uniforms = ContourUniforms {
            texel_size,
            scale: self.params.scale(),
            warp: self.params.warp(),
            contours: self.params.contours(),
            line_weight: self.params.line_weight(),
            elevation: self.params.elevation(),
            jitter: self.params.jitter(),
            breakup: self.params.breakup(),
            warmth: self.params.warmth(),
            invert: self.params.invert(),
            contrast: self.params.contrast(),
            drift_x: self.params.drift_x(),
            drift_y: self.params.drift_y(),
            time: self.phase,
        };
        self.shader.as_ref().unwrap().render(input_tex, &uniforms);

        // Restore host GL state.
        unsafe {
            if scissor_was_on {
                gl::Enable(gl::SCISSOR_TEST);
            }
            if blend_was_on {
                gl::Enable(gl::BLEND);
            }
            if depth_was_on {
                gl::Enable(gl::DEPTH_TEST);
            }
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
            unique_id: *b"CtrF",
            name: *b"Contour Field   ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Finding Ground P1 — generated topographic contour field".to_string(),
            description: "Draws iso-contour line-work of a procedural terrain; a generator (ignores input)"
                .to_string(),
        }
    }
}
