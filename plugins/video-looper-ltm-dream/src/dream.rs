// ── dream.rs ── Main plugin struct and per-frame draw loop ──
//
// The Dream Looper's hot path (all GPU, no CPU pixel work):
//
//   1. INGEST:     Render input texture → Tier 0's current write layer
//   2. DOWNSAMPLE: Tier 0 → Tier 1, Tier 1 → Tier 2, Tier 2 → Tier 3
//   3. COMPOSITE:  Sample across all tiers, blend weighted → host FBO
//   4. ADVANCE:    Bump all write pointers
//
// Every step is a GPU shader pass. Zero CPU involvement per frame.

use std::time::Instant;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::midi::MidiOut;
use crate::params::{self, DreamParams, NUM_PARAMS};
use crate::pyramid::{Pyramid, NUM_TIERS};
use crate::shader::DreamShaders;

pub struct DreamLooper {
    pyramid: Pyramid,
    params: DreamParams,
    shaders: Option<DreamShaders>,
    midi: Option<MidiOut>,
    input_width: u32,
    input_height: u32,
    frame_count: u64,
}

impl SimpleFFGLInstance for DreamLooper {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            pyramid: Pyramid::new(),
            params: DreamParams::new(),
            shaders: None,
            midi: None,
            input_width: 0,
            input_height: 0,
            frame_count: 0,
        }
    }

    fn draw(&mut self, _data: &FFGLData, frame_data: GLInput) {
        // Lazy-init shaders and MIDI on first draw
        if self.shaders.is_none() {
            self.shaders = Some(DreamShaders::new());
        }
        if self.midi.is_none() {
            self.midi = Some(MidiOut::new());
        }

        let input_tex = if !frame_data.textures.is_empty() {
            let t = &frame_data.textures[0];

            // (Re)allocate pyramid on resolution change
            if t.Width != self.input_width || t.Height != self.input_height {
                self.input_width = t.Width;
                self.input_height = t.Height;
                self.pyramid.init(t.Width, t.Height);
            }

            t.Handle as GLuint
        } else {
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            return;
        };

        if !self.pyramid.initialized {
            return;
        }

        let t_frame = Instant::now();
        let shaders = self.shaders.as_ref().unwrap();

        // Save host FBO and GL state (Resolume or other effects may leave
        // scissor/blend/depth enabled, which would corrupt our FBO writes)
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

        // ── STEP 1: Ingest — render input + beat-synced feedback into Tier 0 ──
        let (shift_x, shift_y) = self.params.shift();
        let feedback = self.params.feedback();
        let tier0 = self.pyramid.tiers[0].as_ref().unwrap();
        let prev_array_tex = tier0.array_texture;
        // Beat-synced delay: read N frames back, clamped to buffer depth
        let delay = self.params.delay_frames().min(tier0.depth - 1).max(1);
        let prev_layer = ((tier0.write_ptr + tier0.depth - delay) % tier0.depth) as f32;
        self.pyramid.bind_layer_for_write(0);
        let rotation = self.params.rotation();
        let scale = self.params.scale();
        let hue_shift = self.params.hue_shift();
        let sat_shift = self.params.sat_shift();
        let swirl = self.params.swirl();
        let mirror = if self.params.mirror() { 1.0 } else { 0.0 };
        let fold = self.params.fold_threshold();
        shaders.ingest(input_tex, prev_array_tex, prev_layer, shift_x, shift_y, feedback, rotation, scale, hue_shift, sat_shift, swirl, mirror, fold);

        let t_after_ingest = t_frame.elapsed();

        // ── STEP 2: Downsample chain — T0→T1→T2→T3 ──
        for i in 1..NUM_TIERS {
            // Source: the layer we just wrote in tier i-1
            let (src_tex, src_layer) = {
                let src = self.pyramid.tiers[i - 1].as_ref().unwrap();
                (src.array_texture, src.write_ptr as f32)
            };

            // Target: current write layer in tier i
            self.pyramid.bind_layer_for_write(i);
            shaders.downsample(src_tex, src_layer);
        }

        let t_after_downsample = t_frame.elapsed();

        // ── STEP 3: Composite — one tap per tier → host FBO ──
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }

        let delay_frames = self.params.delay_frames() as f32;
        let dry = self.params.dry();
        let wet = self.params.wet();
        let taps = self.params.tap_levels();

        let cu = &shaders.composite_uniforms;
        shaders.composite.use_program();
        unsafe {
            // Bind live input to texture unit 0
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(cu.input, 0);

            // Bind tier array textures to units 1-4
            for (i, tier_opt) in self.pyramid.tiers.iter().enumerate() {
                if let Some(tier) = tier_opt {
                    gl::ActiveTexture(gl::TEXTURE1 + i as u32);
                    gl::BindTexture(gl::TEXTURE_2D_ARRAY, tier.array_texture);
                    gl::Uniform1i(cu.tiers[i], 1 + i as i32);
                    gl::Uniform1f(cu.write_ptrs[i], tier.write_ptr as f32);
                    gl::Uniform1f(cu.depths[i], tier.depth as f32);
                }
            }

            gl::Uniform1f(cu.delay, delay_frames);
            gl::Uniform1f(cu.dry, dry);
            gl::Uniform1f(cu.wet, wet);
            for (i, &t) in taps.iter().enumerate() {
                gl::Uniform1f(cu.taps[i], t);
            }
        }
        shaders.quad.draw();

        // Clean up texture bindings, restore TEXTURE0 as active unit
        unsafe {
            for i in (0..5).rev() {
                gl::ActiveTexture(gl::TEXTURE0 + i);
                gl::BindTexture(gl::TEXTURE_2D, 0);
                gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
            }
            // Restore GL state that Resolume/other effects may depend on
            if scissor_was_on { gl::Enable(gl::SCISSOR_TEST); }
            if blend_was_on { gl::Enable(gl::BLEND); }
            if depth_was_on { gl::Enable(gl::DEPTH_TEST); }
        }
        shaders.composite.unuse();

        let t_after_composite = t_frame.elapsed();

        // ── STEP 4: Advance all write pointers ──
        for i in 0..NUM_TIERS {
            self.pyramid.advance(i);
        }

        // Log timing every 60 frames
        self.frame_count += 1;
        if self.frame_count % 60 == 0 {
            tracing::info!(
                frame = self.frame_count,
                ingest_us = t_after_ingest.as_micros(),
                downsample_us = (t_after_downsample - t_after_ingest).as_micros(),
                composite_us = (t_after_composite - t_after_downsample).as_micros(),
                total_us = t_after_composite.as_micros(),
                "frame_timing"
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
        // Send MIDI CC on relevant param changes
        if let Some(midi) = &mut self.midi {
            match index {
                params::PARAM_SUBDIVISION => {
                    midi.send_subdivision(self.params.subdivision_beats());
                }
                params::PARAM_FEEDBACK => {
                    midi.send_feedback(self.params.feedback());
                }
                _ => {}
            }
        }
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"DtLm",
            name: *b"Dream LTM       ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Dream-tiered temporal pyramid for fluid motion trails".to_string(),
            description: "Logarithmic video persistence with GPU-only memory".to_string(),
        }
    }
}
