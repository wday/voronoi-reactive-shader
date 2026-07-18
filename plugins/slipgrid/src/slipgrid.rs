use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, SlipgridParams, NUM_PARAMS};
use crate::shader::{SlipgridShader, SlipgridUniforms};

pub struct Slipgrid {
    params: SlipgridParams,
    shader: Option<SlipgridShader>,
}

impl SimpleFFGLInstance for Slipgrid {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: SlipgridParams::new(),
            shader: None,
        }
    }

    fn draw(&mut self, _data: &FFGLData, frame_data: GLInput) {
        // The GL context only exists once we are drawing, so build lazily.
        if self.shader.is_none() {
            self.shader = Some(SlipgridShader::new());
        }

        let (input_tex, uv_scale) = if !frame_data.textures.is_empty() {
            let t = &frame_data.textures[0];
            let uv_scale = [
                t.Width as f32 / t.HardwareWidth as f32,
                t.Height as f32 / t.HardwareHeight as f32,
            ];
            (t.Handle as GLuint, uv_scale)
        } else {
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            return;
        };

        // Save host GL state
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

        let uniforms = SlipgridUniforms {
            grid: self.params.grid(),
            intensity: self.params.intensity(),
            locality: self.params.locality(),
            edge_gravity: self.params.edge_gravity(),
            iterations: self.params.iterations(),
            mode: self.params.mode(),
            seed: self.params.seed(),
            dry_wet: self.params.dry_wet(),
            uv_scale,
        };
        self.shader.as_ref().unwrap().render(input_tex, &uniforms);

        // Restore host GL state
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
            unique_id: *b"SlpG",
            name: *b"Slipgrid        ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Tile permutation with edge-seeking gravity".to_string(),
            description: "Shuffles the frame in tiles; tiles are drawn to image contours"
                .to_string(),
        }
    }
}
