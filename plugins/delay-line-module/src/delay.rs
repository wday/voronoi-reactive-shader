use std::time::Instant;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, DelayParams, Mode, NUM_PARAMS};
use crate::registry;
use crate::shader::DelayShaders;

pub struct DelayLine {
    params: DelayParams,
    shaders: Option<DelayShaders>,
    fps_estimate: f32,
    last_frame_time: Option<Instant>,
    frame_count: u64,
}

impl DelayLine {
    fn delay_frames(&self, bpm: f32) -> u32 {
        if bpm <= 0.0 {
            return 30;
        }
        let beat_duration = 60.0 / bpm;
        let delay_secs = self.params.subdivision_beats() * beat_duration;
        let d = (delay_secs * self.fps_estimate).round() as u32;
        d.clamp(1, registry::buffer_depth() - 1)
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

    fn draw_send(&mut self, data: &FFGLData, input_tex: GLuint, width: u32, height: u32, host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();
        let (buf_tex, fbo, write_pos) = registry::ensure_channel(channel, width, height);
        let depth = registry::buffer_depth();
        let d = self.delay_frames(data.host_beat.bpm);
        let read_layer = ((write_pos + depth - d) % depth) as f32;
        let shaders = self.shaders.as_ref().unwrap();

        // Write input to buffer[write_pos]
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTextureLayer(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                buf_tex,
                0,
                write_pos as i32,
            );
            gl::Viewport(0, 0, width as i32, height as i32);
        }
        shaders.write_pass(input_tex);

        // Output delayed frame (not passthrough) to host FBO
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.read_pass(buf_tex, read_layer);

        registry::advance_write_pos(channel);
    }

    fn draw_receive(&mut self, data: &FFGLData, input_tex: GLuint, host_fbo: GLint, host_viewport: [GLint; 4]) {
        let channel = self.params.channel();
        let shaders = self.shaders.as_ref().unwrap();

        // If no Send has written to this channel yet, passthrough
        let buf_info = match registry::read_channel(channel) {
            Some(info) => info,
            None => {
                unsafe {
                    gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
                    gl::Viewport(
                        host_viewport[0], host_viewport[1],
                        host_viewport[2], host_viewport[3],
                    );
                }
                shaders.write_pass(input_tex);
                return;
            }
        };

        let (buf_tex, write_pos, _buf_w, _buf_h) = buf_info;
        let depth = registry::buffer_depth();
        let d = self.delay_frames(data.host_beat.bpm);
        let read_layer = ((write_pos + depth - d) % depth) as f32;
        let feedback = self.params.feedback();

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0], host_viewport[1],
                host_viewport[2], host_viewport[3],
            );
        }
        shaders.receive_pass(input_tex, buf_tex, read_layer, feedback);
    }
}

impl SimpleFFGLInstance for DelayLine {
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data;

        Self {
            params: DelayParams::new(),
            shaders: None,
            fps_estimate: 60.0,
            last_frame_time: None,
            frame_count: 0,
        }
    }

    fn draw(&mut self, data: &FFGLData, frame_data: GLInput) {
        if self.shaders.is_none() {
            self.shaders = Some(DelayShaders::new());
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

        let width = frame_data.textures[0].Width;
        let height = frame_data.textures[0].Height;

        self.update_fps();

        // Save host GL state
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

        match self.params.mode() {
            Mode::Send => self.draw_send(data, input_tex, width, height, host_fbo, host_viewport),
            Mode::Receive => self.draw_receive(data, input_tex, host_fbo, host_viewport),
        }

        // Restore host GL state
        unsafe {
            if scissor_was_on { gl::Enable(gl::SCISSOR_TEST); }
            if blend_was_on { gl::Enable(gl::BLEND); }
            if depth_was_on { gl::Enable(gl::DEPTH_TEST); }
        }

        self.frame_count += 1;
        if self.frame_count % 300 == 0 {
            let mode_str = match self.params.mode() {
                Mode::Send => "send",
                Mode::Receive => "receive",
            };
            tracing::info!(
                frame = self.frame_count,
                mode = mode_str,
                channel = self.params.channel() + 1,
                fps = format!("{:.1}", self.fps_estimate),
                bpm = format!("{:.1}", data.host_beat.bpm),
                subdivision = format!("{:.2}", self.params.subdivision_beats()),
                feedback = format!("{:.2}", self.params.feedback()),
                "status"
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
            unique_id: *b"DLMd",
            name: *b"Delay Line      ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Send/receive delay line for modular feedback loops".to_string(),
            description: "Beat-synced delay with shared buffer channels".to_string(),
        }
    }
}
