use gl::types::*;
use std::ffi::CString;
use std::ptr;

static VS_SRC: &str = include_str!("shaders/fullscreen.vert.glsl");
static FS_TRANSFORM: &str = include_str!("shaders/transform3d.frag.glsl");

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

/// Every uniform of the transform pass, in pipeline order.
pub struct Uniforms {
    pub scale: f32,
    pub perspective: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub anamorph: f32,
    pub swirl: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    pub edges: f32,
    pub fold: f32,
    pub aspect: f32,
    pub uv_scale: [f32; 2],
    pub texel: [f32; 2],
}

pub struct TransformShader {
    program: ShaderProgram,
    loc_input: GLint,
    loc_scale: GLint,
    loc_persp: GLint,
    loc_rot_x: GLint,
    loc_rot_y: GLint,
    loc_rot_z: GLint,
    loc_anamorph: GLint,
    loc_swirl: GLint,
    loc_translate_x: GLint,
    loc_translate_y: GLint,
    loc_edges: GLint,
    loc_fold: GLint,
    loc_aspect: GLint,
    loc_uv_scale: GLint,
    loc_texel: GLint,
    pub quad: QuadGeometry,
}

impl TransformShader {
    pub fn new() -> Self {
        let program = ShaderProgram::new(FS_TRANSFORM);
        let quad = QuadGeometry::new();
        quad.setup_attrs(program.program);

        Self {
            loc_input: program.uniform_loc("u_input"),
            loc_scale: program.uniform_loc("u_scale"),
            loc_persp: program.uniform_loc("u_persp"),
            loc_rot_x: program.uniform_loc("u_rot_x"),
            loc_rot_y: program.uniform_loc("u_rot_y"),
            loc_rot_z: program.uniform_loc("u_rot_z"),
            loc_anamorph: program.uniform_loc("u_anamorph"),
            loc_swirl: program.uniform_loc("u_swirl"),
            loc_translate_x: program.uniform_loc("u_translate_x"),
            loc_translate_y: program.uniform_loc("u_translate_y"),
            loc_edges: program.uniform_loc("u_edges"),
            loc_fold: program.uniform_loc("u_fold"),
            loc_aspect: program.uniform_loc("u_aspect"),
            loc_uv_scale: program.uniform_loc("u_uv_scale"),
            loc_texel: program.uniform_loc("u_texel"),
            program,
            quad,
        }
    }

    pub fn render(&self, input_tex: GLuint, u: &Uniforms) {
        self.program.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_input, 0);
            gl::Uniform1f(self.loc_scale, u.scale);
            gl::Uniform1f(self.loc_persp, u.perspective);
            gl::Uniform1f(self.loc_rot_x, u.rot_x);
            gl::Uniform1f(self.loc_rot_y, u.rot_y);
            gl::Uniform1f(self.loc_rot_z, u.rot_z);
            gl::Uniform1f(self.loc_anamorph, u.anamorph);
            gl::Uniform1f(self.loc_swirl, u.swirl);
            gl::Uniform1f(self.loc_translate_x, u.translate_x);
            gl::Uniform1f(self.loc_translate_y, u.translate_y);
            gl::Uniform1f(self.loc_edges, u.edges);
            gl::Uniform1f(self.loc_fold, u.fold);
            gl::Uniform1f(self.loc_aspect, u.aspect);
            gl::Uniform2f(self.loc_uv_scale, u.uv_scale[0], u.uv_scale[1]);
            gl::Uniform2f(self.loc_texel, u.texel[0], u.texel[1]);
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
