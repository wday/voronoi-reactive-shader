//! pluglib — shared plumbing for the delay-write / delay-tap plugins.
//!
//! Two responsibilities:
//!   1. `api()` — runtime-load delay-core from THIS plugin's own directory and
//!      bind its C ABI. See `loader` for the platform-specific mechanics (the
//!      Windows path is the one that matters and was proven in the Stage-0 spike).
//!   2. GL helpers (`QuadGeometry`, `ShaderProgram`) + shared shader sources,
//!      so both plugins build their passes from one implementation.

mod loader;

pub use loader::{api, Api};

use gl::types::*;
use std::ffi::CString;
use std::ptr;

/// Shared fullscreen-quad vertex shader.
pub const VS_SRC: &str = include_str!("shaders/fullscreen.vert.glsl");
/// Shared output pass: `out = clamp(dry*node_input + wet*buffer[layer], 0, 1)`.
/// The Tap uses both gains (Dry/Wet); the Write uses dry=1, wet=0 (passthrough).
pub const FS_OUTPUT: &str = include_str!("shaders/output.frag.glsl");

/// A unit quad (pos + uv) drawn as a triangle strip. Attribute locations are
/// bound to 0/1 at link time so one VAO works across programs.
pub struct QuadGeometry {
    vao: GLuint,
    vbo: GLuint,
}

impl QuadGeometry {
    pub fn new() -> Self {
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
        }
        Self { vao, vbo }
    }

    pub fn setup_attrs(&self, program: GLuint) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

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
    }

    pub fn draw(&self) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            gl::BindVertexArray(0);
        }
    }
}

impl Drop for QuadGeometry {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

/// A compiled+linked program (shared VS + a given FS). Panics on GL build error.
pub struct ShaderProgram {
    pub program: GLuint,
}

impl ShaderProgram {
    pub fn new(fs_src: &str) -> Self {
        unsafe {
            let vs = compile_shader(VS_SRC, gl::VERTEX_SHADER);
            let fs = compile_shader(fs_src, gl::FRAGMENT_SHADER);
            let program = link_program(vs, fs);
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);
            Self { program }
        }
    }

    pub fn uniform_loc(&self, name: &str) -> GLint {
        let c_name = CString::new(name).unwrap();
        unsafe { gl::GetUniformLocation(self.program, c_name.as_ptr()) }
    }

    pub fn use_program(&self) {
        unsafe { gl::UseProgram(self.program); }
    }

    pub fn unuse(&self) {
        unsafe { gl::UseProgram(0); }
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.program); }
    }
}

/// The shared node-output pass (`FS_OUTPUT`): `clamp(dry*input + wet*buffer[layer])`.
/// Input goes to texture unit 0 (sampler2D), buffer to unit 1 (sampler2DArray).
/// Leaves unit 0 active with nothing bound, matching the host's expectation.
pub struct OutputProgram {
    prog: ShaderProgram,
    loc_input: GLint,
    loc_uv_scale: GLint,
    loc_buffer: GLint,
    loc_layer: GLint,
    loc_dry: GLint,
    loc_wet: GLint,
}

impl OutputProgram {
    pub fn new() -> Self {
        let prog = ShaderProgram::new(FS_OUTPUT);
        let loc_input = prog.uniform_loc("u_input");
        let loc_uv_scale = prog.uniform_loc("u_uv_scale");
        let loc_buffer = prog.uniform_loc("u_buffer");
        let loc_layer = prog.uniform_loc("u_layer");
        let loc_dry = prog.uniform_loc("u_dry");
        let loc_wet = prog.uniform_loc("u_wet");
        Self { prog, loc_input, loc_uv_scale, loc_buffer, loc_layer, loc_dry, loc_wet }
    }

    /// Bind `quad`'s vertex attributes for this program's VAO. Call once after
    /// creating the quad (attribute locations 0/1 are bound at link for every
    /// program, so one setup serves all passes). Without this the VAO has no
    /// enabled attributes and `draw` rasterizes a degenerate quad → black output.
    pub fn setup_quad(&self, quad: &QuadGeometry) {
        quad.setup_attrs(self.prog.program);
    }

    /// Draw `clamp(dry*input + wet*buffer[layer])` into the currently bound FBO.
    /// `buffer_tex` may be 0 (samples black) — pass `wet = 0.0` (and `dry = 1.0`)
    /// for a clean passthrough of `input_tex` when no ring buffer exists yet.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        quad: &QuadGeometry,
        input_tex: GLuint,
        uv_scale: [f32; 2],
        buffer_tex: GLuint,
        layer: f32,
        dry: f32,
        wet: f32,
    ) {
        self.prog.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_input, 0);
            gl::Uniform2f(self.loc_uv_scale, uv_scale[0], uv_scale[1]);

            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, buffer_tex);
            gl::Uniform1i(self.loc_buffer, 1);
            gl::Uniform1f(self.loc_layer, layer);

            gl::Uniform1f(self.loc_dry, dry);
            gl::Uniform1f(self.loc_wet, wet);
        }
        quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.prog.unuse();
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
        let msg = String::from_utf8_lossy(&buf);
        panic!("Shader compile error: {msg}");
    }
    shader
}

unsafe fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    let program = gl::CreateProgram();
    gl::AttachShader(program, vs);
    gl::AttachShader(program, fs);

    // GLSL 150 has no layout(location=…); bind before linking so all programs
    // sharing a VAO agree on attribute locations.
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
        let msg = String::from_utf8_lossy(&buf);
        panic!("Program link error: {msg}");
    }
    program
}
