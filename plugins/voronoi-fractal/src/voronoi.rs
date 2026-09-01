use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::clock::DriftClock;
use crate::params::{self, VoronoiParams, NUM_PARAMS};
use crate::shader::{VoronoiShader, VoronoiUniforms};

/// Free-run integrator rate. Only used when Beat Sync = Off; the synced path
/// derives its phase from the host transport and never touches this.
const ASSUMED_FPS: f32 = 60.0;

pub struct VoronoiFractal {
    params: VoronoiParams,
    shader: Option<VoronoiShader>,
    clock: DriftClock,
}

impl SimpleFFGLInstance for VoronoiFractal {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: VoronoiParams::new(),
            shader: None,
            clock: DriftClock::new(),
        }
    }

    fn draw(&mut self, data: &FFGLData, frame_data: GLInput) {
        if self.shader.is_none() {
            self.shader = Some(VoronoiShader::new());
        }

        // Resolution/aspect from the viewport → correct with or without input.
        let texel_size = unsafe {
            let mut vp = [0i32; 4];
            gl::GetIntegerv(gl::VIEWPORT, vp.as_mut_ptr());
            let w = vp[2].max(1) as f32;
            let h = vp[3].max(1) as f32;
            [1.0 / w, 1.0 / h]
        };

        let input_tex = if !frame_data.textures.is_empty() {
            frame_data.textures[0].Handle as GLuint
        } else {
            0
        };

        let period = self.params.sync_period_bars();
        let anim_time = self.clock.tick(
            data.host_beat.barPhase,
            period,
            self.params.drift_speed(),
            ASSUMED_FPS,
        );

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

        let uniforms = VoronoiUniforms {
            texel_size,
            fractal: self.params.fractal_mode(),
            density: self.params.density(),
            layer_spread: self.params.layer_spread(),
            layer_mix: self.params.layer_mix(),
            depth: self.params.depth(),
            coastline: self.params.coastline(),
            drift_chaos: self.params.drift_chaos(),
            warp: self.params.warp(),
            edge_width: self.params.edge_width(),
            edge_glow: self.params.edge_glow(),
            color_shift: self.params.color_shift(),
            color_sat: self.params.color_sat(),
            image_influence: self.params.image_influence(),
            nc_kernel: self.params.nc_kernel(),
            cert_contrast: self.params.cert_contrast(),
            cert_brightness: self.params.cert_brightness(),
            fill_level: self.params.fill_level(),
            brightness: self.params.brightness(),
            contrast: self.params.contrast(),
            image_blend: self.params.image_blend(),
            anim_time,
            beat_locked: DriftClock::beat_locked(period),
        };
        self.shader.as_ref().unwrap().render(input_tex, &uniforms);

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
            // Bumped from VrFr when Fill Level (param 22) was added: Resolume
            // caches param descriptors against unique_id, so a layout change is
            // invisible under the old id. See CREDITS/devstate.
            unique_id: *b"VrF2",
            name: *b"Voronoi Fract v2",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Hierarchical jittered Voronoi with image-driven coastlines".to_string(),
            description: "Fractal Voronoi partitions whose boundaries follow input structure; \
                          bar-locked drift. Layered mode ports the Voronoi Reactive ISF look."
                .to_string(),
        }
    }
}
