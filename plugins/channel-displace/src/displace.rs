use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, DisplaceParams, NUM_PARAMS};
use crate::shader::DisplaceShader;

pub struct ChannelDisplace {
    params: DisplaceParams,
    shader: Option<DisplaceShader>,
}

impl SimpleFFGLInstance for ChannelDisplace {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: DisplaceParams::new(),
            shader: None,
        }
    }

    fn draw(&mut self, _data: &FFGLData, frame_data: GLInput) {
        if self.shader.is_none() {
            self.shader = Some(DisplaceShader::new());
        }

        let input_tex = if !frame_data.textures.is_empty() {
            frame_data.textures[0].Handle as GLuint
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

        self.shader.as_ref().unwrap().render(
            input_tex,
            self.params.amount(),
            self.params.pattern(),
            self.params.angle(),
            self.params.dry_wet(),
        );

        // Restore host GL state
        unsafe {
            if scissor_was_on { gl::Enable(gl::SCISSOR_TEST); }
            if blend_was_on { gl::Enable(gl::BLEND); }
            if depth_was_on { gl::Enable(gl::DEPTH_TEST); }
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
            unique_id: *b"ChDp",
            name: *b"Channel Displace",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Cross-channel UV displacement for strange attractor dynamics".to_string(),
            description: "Couples RGB channels via spatial displacement".to_string(),
        }
    }
}
