use std::ffi::CString;
use std::ptr;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::parameters::{ParamInfo, ParameterTypes, SimpleParamInfo};
use ffgl_core::{FFGLData, GLInput};

static VS_SRC: &str = include_str!("shaders/fullscreen.vert.glsl");
static FS_SRC: &str = include_str!("shaders/slew.frag.glsl");

const NUM_PARAMS: usize = 1;

static PARAM_INFOS: std::sync::LazyLock<[SimpleParamInfo; NUM_PARAMS]> =
    std::sync::LazyLock::new(|| {
        [SimpleParamInfo {
            name: CString::new("Rate").unwrap(),
            param_type: ParameterTypes::Standard,
            default: Some(0.1),
            ..Default::default()
        }]
    });

pub struct SlewLimiter {
    rate: f32,
    gpu: Option<GpuState>,
}

struct GpuState {
    program: GLuint,
    vao: GLuint,
    vbo: GLuint,
    fbo: GLuint,
    textures: [GLuint; 2],
    tex_width: u32,
    tex_height: u32,
    current: usize, // ping-pong index: read from current, write to 1-current
    loc_input: GLint,
    loc_previous: GLint,
    loc_rate: GLint,
    loc_uv_scale: GLint,
}

impl GpuState {
    fn new() -> Self {
        let program = unsafe {
            let vs = compile_shader(VS_SRC, gl::VERTEX_SHADER);
            let fs = compile_shader(FS_SRC, gl::FRAGMENT_SHADER);
            let p = link_program(vs, fs);
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);
            p
        };

        let (vao, vbo) = create_quad(program);

        let loc_input = uniform_loc(program, "u_input");
        let loc_previous = uniform_loc(program, "u_previous");
        let loc_rate = uniform_loc(program, "u_rate");
        let loc_uv_scale = uniform_loc(program, "u_uv_scale");

        Self {
            program,
            vao,
            vbo,
            fbo: 0,
            textures: [0; 2],
            tex_width: 0,
            tex_height: 0,
            current: 0,
            loc_input,
            loc_previous,
            loc_rate,
            loc_uv_scale,
        }
    }

    fn ensure_textures(&mut self, width: u32, height: u32) {
        if self.tex_width == width && self.tex_height == height && self.fbo != 0 {
            return;
        }

        // Clean up old
        unsafe {
            if self.textures[0] != 0 {
                gl::DeleteTextures(2, self.textures.as_ptr());
            }
            if self.fbo != 0 {
                gl::DeleteFramebuffers(1, &self.fbo);
            }
        }

        // Allocate two textures for ping-pong
        unsafe {
            gl::GenTextures(2, self.textures.as_mut_ptr());
            for &tex in &self.textures {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RGBA8 as i32,
                    width as i32,
                    height as i32,
                    0,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    ptr::null(),
                );
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            }
            gl::BindTexture(gl::TEXTURE_2D, 0);

            gl::GenFramebuffers(1, &mut self.fbo);
        }

        self.tex_width = width;
        self.tex_height = height;
        self.current = 0;
    }

    fn render(&mut self, input_tex: GLuint, rate: f32, uv_scale: [f32; 2], host_fbo: GLint, host_viewport: [GLint; 4]) {
        let prev_tex = self.textures[self.current];
        let write_tex = self.textures[1 - self.current];

        // Pass 1: render slew result into write_tex
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                write_tex,
                0,
            );
            gl::Viewport(0, 0, self.tex_width as i32, self.tex_height as i32);

            gl::UseProgram(self.program);

            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_input, 0);

            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, prev_tex);
            gl::Uniform1i(self.loc_previous, 1);

            // x⁴ curve: ~0.4 on knob → 0.03 rate, sweet spot gets most travel
            let r2 = rate * rate;
            gl::Uniform1f(self.loc_rate, r2 * r2);
            gl::Uniform2f(self.loc_uv_scale, uv_scale[0], uv_scale[1]);

            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            gl::BindVertexArray(0);
        }

        // Pass 2: blit write_tex to host FBO
        unsafe {
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.fbo);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, host_fbo as GLuint);
            gl::BlitFramebuffer(
                0, 0, self.tex_width as i32, self.tex_height as i32,
                host_viewport[0], host_viewport[1],
                host_viewport[0] + host_viewport[2],
                host_viewport[1] + host_viewport[3],
                gl::COLOR_BUFFER_BIT,
                gl::LINEAR,
            );
        }

        // Swap ping-pong
        self.current = 1 - self.current;

        // Clean up
        unsafe {
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
        }
    }
}

impl Drop for GpuState {
    fn drop(&mut self) {
        unsafe {
            if self.textures[0] != 0 {
                gl::DeleteTextures(2, self.textures.as_ptr());
            }
            if self.fbo != 0 {
                gl::DeleteFramebuffers(1, &self.fbo);
            }
            if self.program != 0 {
                gl::DeleteProgram(self.program);
            }
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

impl SimpleFFGLInstance for SlewLimiter {
    fn new(_inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());

        Self {
            rate: 0.1,
            gpu: None,
        }
    }

    fn draw(&mut self, _data: &FFGLData, frame_data: GLInput) {
        if self.gpu.is_none() {
            self.gpu = Some(GpuState::new());
        }

        let input_tex = if !frame_data.textures.is_empty() {
            frame_data.textures[0].Handle as GLuint
        } else {
            return;
        };

        let (width, height, hw_width, hw_height) = (
            frame_data.textures[0].Width,
            frame_data.textures[0].Height,
            frame_data.textures[0].HardwareWidth,
            frame_data.textures[0].HardwareHeight,
        );

        let mut host_fbo: GLint = 0;
        let mut host_viewport: [GLint; 4] = [0; 4];
        let scissor_was_on;
        let blend_was_on;
        unsafe {
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut host_fbo);
            gl::GetIntegerv(gl::VIEWPORT, host_viewport.as_mut_ptr());
            scissor_was_on = gl::IsEnabled(gl::SCISSOR_TEST) == gl::TRUE;
            blend_was_on = gl::IsEnabled(gl::BLEND) == gl::TRUE;
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
        }

        let uv_scale = [
            width as f32 / hw_width as f32,
            height as f32 / hw_height as f32,
        ];

        let gpu = self.gpu.as_mut().unwrap();
        gpu.ensure_textures(width, height);
        gpu.render(input_tex, self.rate, uv_scale, host_fbo, host_viewport);

        unsafe {
            if scissor_was_on { gl::Enable(gl::SCISSOR_TEST); }
            if blend_was_on { gl::Enable(gl::BLEND); }
        }
    }

    fn num_params() -> usize {
        NUM_PARAMS
    }

    fn param_info(index: usize) -> &'static dyn ParamInfo {
        &PARAM_INFOS[index]
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.rate,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.rate = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"SlwL",
            name: *b"Slew Limiter    ",
            ty: ffgl_core::info::PluginType::Effect,
            about: "Per-pixel temporal slew limiter".to_string(),
            description: "Limits how fast each pixel can change color".to_string(),
        }
    }
}

unsafe fn compile_shader(src: &str, ty: GLenum) -> GLuint {
    let shader = gl::CreateShader(ty);
    let c_str = CString::new(src.as_bytes()).unwrap();
    gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
    gl::CompileShader(shader);

    let mut status = gl::FALSE as GLint;
    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);
    if status != (gl::TRUE as GLint) {
        let mut len = 0;
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
        let mut buf = vec![0u8; len as usize];
        gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
        panic!("Shader compile error: {}", String::from_utf8_lossy(&buf));
    }
    shader
}

unsafe fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    let program = gl::CreateProgram();
    gl::AttachShader(program, vs);
    gl::AttachShader(program, fs);

    let pos_name = CString::new("position").unwrap();
    let uv_name = CString::new("texcoord").unwrap();
    gl::BindAttribLocation(program, 0, pos_name.as_ptr());
    gl::BindAttribLocation(program, 1, uv_name.as_ptr());

    gl::LinkProgram(program);

    let mut status = gl::FALSE as GLint;
    gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);
    if status != (gl::TRUE as GLint) {
        let mut len = 0;
        gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
        let mut buf = vec![0u8; len as usize];
        gl::GetProgramInfoLog(program, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
        panic!("Program link error: {}", String::from_utf8_lossy(&buf));
    }
    program
}

fn uniform_loc(program: GLuint, name: &str) -> GLint {
    let c_name = CString::new(name).unwrap();
    unsafe { gl::GetUniformLocation(program, c_name.as_ptr()) }
}

fn create_quad(program: GLuint) -> (GLuint, GLuint) {
    #[rustfmt::skip]
    static QUAD: [f32; 16] = [
        -1.0, -1.0,   0.0, 0.0,
         1.0, -1.0,   1.0, 0.0,
        -1.0,  1.0,   0.0, 1.0,
         1.0,  1.0,   1.0, 1.0,
    ];

    let mut vao: GLuint = 0;
    let mut vbo: GLuint = 0;

    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (QUAD.len() * std::mem::size_of::<f32>()) as isize,
            QUAD.as_ptr().cast(),
            gl::STATIC_DRAW,
        );

        let pos_name = CString::new("position").unwrap();
        let pos_attr = gl::GetAttribLocation(program, pos_name.as_ptr());
        if pos_attr >= 0 {
            gl::EnableVertexAttribArray(pos_attr as GLuint);
            gl::VertexAttribPointer(
                pos_attr as GLuint,
                2, gl::FLOAT, gl::FALSE as GLboolean,
                (4 * std::mem::size_of::<f32>()) as i32,
                ptr::null(),
            );
        }

        let uv_name = CString::new("texcoord").unwrap();
        let uv_attr = gl::GetAttribLocation(program, uv_name.as_ptr());
        if uv_attr >= 0 {
            gl::EnableVertexAttribArray(uv_attr as GLuint);
            gl::VertexAttribPointer(
                uv_attr as GLuint,
                2, gl::FLOAT, gl::FALSE as GLboolean,
                (4 * std::mem::size_of::<f32>()) as i32,
                (2 * std::mem::size_of::<f32>()) as *const _,
            );
        }

        gl::BindVertexArray(0);
    }

    (vao, vbo)
}
