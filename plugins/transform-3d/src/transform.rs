use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, Transform3DParams, NUM_PARAMS};
use crate::shader::{TransformShader, Uniforms};

pub struct Transform3D {
    params: Transform3DParams,
    shader: Option<TransformShader>,
}

impl SimpleFFGLInstance for Transform3D {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: Transform3DParams::new(),
            shader: None,
        }
    }

    fn draw(&mut self, _data: &FFGLData, frame_data: GLInput) {
        if self.shader.is_none() {
            self.shader = Some(TransformShader::new());
        }

        let (input_tex, uv_scale, texel, aspect) = if !frame_data.textures.is_empty() {
            let t = &frame_data.textures[0];
            let uv_scale = [
                t.Width as f32 / t.HardwareWidth as f32,
                t.Height as f32 / t.HardwareHeight as f32,
            ];
            let texel = [
                1.0 / t.HardwareWidth as f32,
                1.0 / t.HardwareHeight as f32,
            ];
            // CONTENT aspect, not hardware: the NPOT padding must not tilt the
            // perspective. Guard against a zero-height texture on the first frame.
            let aspect = if t.Height > 0 {
                t.Width as f32 / t.Height as f32
            } else {
                1.0
            };
            (t.Handle as GLuint, uv_scale, texel, aspect)
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
            &Uniforms {
                scale: self.params.scale(),
                perspective: self.params.perspective(),
                rot_x: self.params.rot_x(),
                rot_y: self.params.rot_y(),
                rot_z: self.params.rot_z(),
                anamorph: self.params.anamorph(),
                swirl: self.params.swirl(),
                translate_x: self.params.translate_x(),
                translate_y: self.params.translate_y(),
                edges: self.params.edges(),
                fold: self.params.fold(),
                aspect,
                uv_scale,
                texel,
            },
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
            unique_id: *b"Tx3D",
            name: *b"Transform 3D    ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "3-axis rotation with Catmull-Rom resampling and mirror folds".to_string(),
            description: "Perspective X/Y/Z rotation, swirl and kaleidoscope edges, \
                          bicubic-sampled for deep feedback loops"
                .to_string(),
        }
    }
}
