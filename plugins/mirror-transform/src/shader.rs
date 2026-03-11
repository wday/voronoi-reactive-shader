use gl::types::*;
use std::ffi::CString;
use std::ptr;

static VS_SRC: &str = include_str!("shaders/fullscreen.vert.glsl");
static FS_TRANSFORM: &str = include_str!("shaders/transform.frag.glsl");

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

struct ShaderProgram {
    program: GLuint,
}

impl ShaderProgram {
    fn new(fs_src: &str) -> Self {
        unsafe {
            let vs = compile_shader(VS_SRC, gl::VERTEX_SHADER);
            let fs = compile_shader(fs_src, gl::FRAGMENT_SHADER);
            let program = link_program(vs, fs);
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);
            Self { program }
        }
    }

    fn uniform_loc(&self, name: &str) -> GLint {
        let c_name = CString::new(name).unwrap();
        unsafe { gl::GetUniformLocation(self.program, c_name.as_ptr()) }
    }

    fn use_program(&self) {
        unsafe { gl::UseProgram(self.program); }
    }

    fn unuse(&self) {
        unsafe { gl::UseProgram(0); }
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.program); }
    }
}

pub struct TransformShader {
    program: ShaderProgram,
    loc_input: GLint,
    loc_scale: GLint,
    loc_rotation: GLint,
    loc_swirl: GLint,
    loc_mirror: GLint,
    pub quad: QuadGeometry,
}

impl TransformShader {
    pub fn new() -> Self {
        let program = ShaderProgram::new(FS_TRANSFORM);
        let quad = QuadGeometry::new();
        quad.setup_attrs(program.program);

        let loc_input = program.uniform_loc("u_input");
        let loc_scale = program.uniform_loc("u_scale");
        let loc_rotation = program.uniform_loc("u_rotation");
        let loc_swirl = program.uniform_loc("u_swirl");
        let loc_mirror = program.uniform_loc("u_mirror");

        Self {
            program,
            loc_input,
            loc_scale,
            loc_rotation,
            loc_swirl,
            loc_mirror,
            quad,
        }
    }

    pub fn render(&self, input_tex: GLuint, scale: f32, rotation: f32, swirl: f32, mirror: bool) {
        self.program.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_input, 0);
            gl::Uniform1f(self.loc_scale, scale);
            gl::Uniform1f(self.loc_rotation, rotation);
            gl::Uniform1f(self.loc_swirl, swirl);
            gl::Uniform1f(self.loc_mirror, if mirror { 1.0 } else { 0.0 });
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.program.unuse();
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
