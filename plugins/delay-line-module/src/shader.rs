use gl::types::*;
use std::ffi::CString;
use std::ptr;

static VS_SRC: &str = include_str!("shaders/fullscreen.vert.glsl");
static FS_WRITE: &str = include_str!("shaders/write.frag.glsl");
static FS_READ: &str = include_str!("shaders/read.frag.glsl");
static FS_RECEIVE: &str = include_str!("shaders/receive.frag.glsl");
static FS_SEND_OUTPUT: &str = include_str!("shaders/send_output.frag.glsl");
static FS_FADE: &str = include_str!("shaders/fade.frag.glsl");

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

pub struct DelayShaders {
    write: ShaderProgram,
    read: ShaderProgram,
    receive: ShaderProgram,
    send_output: ShaderProgram,
    fade: ShaderProgram,
    loc_write_input: GLint,
    loc_write_uv_scale: GLint,
    loc_read_buffer: GLint,
    loc_read_layer: GLint,
    loc_read_uv_scale: GLint,
    loc_rx_input: GLint,
    loc_rx_buffer: GLint,
    loc_rx_layer: GLint,
    loc_rx_feedback: GLint,
    loc_rx_uv_scale: GLint,
    loc_so_input: GLint,
    loc_so_passthrough: GLint,
    loc_so_uv_scale: GLint,
    loc_fade_buffer: GLint,
    loc_fade_layer: GLint,
    loc_fade_decay: GLint,
    pub quad: QuadGeometry,
}

impl DelayShaders {
    pub fn new() -> Self {
        let write = ShaderProgram::new(FS_WRITE);
        let read = ShaderProgram::new(FS_READ);
        let receive = ShaderProgram::new(FS_RECEIVE);
        let send_output = ShaderProgram::new(FS_SEND_OUTPUT);
        let fade = ShaderProgram::new(FS_FADE);

        let quad = QuadGeometry::new();
        quad.setup_attrs(write.program);

        let loc_write_input = write.uniform_loc("u_input");
        let loc_write_uv_scale = write.uniform_loc("u_uv_scale");
        let loc_read_buffer = read.uniform_loc("u_buffer");
        let loc_read_layer = read.uniform_loc("u_layer");
        let loc_read_uv_scale = read.uniform_loc("u_uv_scale");
        let loc_rx_input = receive.uniform_loc("u_input");
        let loc_rx_buffer = receive.uniform_loc("u_buffer");
        let loc_rx_layer = receive.uniform_loc("u_layer");
        let loc_rx_feedback = receive.uniform_loc("u_feedback");
        let loc_rx_uv_scale = receive.uniform_loc("u_uv_scale");
        let loc_so_input = send_output.uniform_loc("u_input");
        let loc_so_passthrough = send_output.uniform_loc("u_passthrough");
        let loc_so_uv_scale = send_output.uniform_loc("u_uv_scale");
        let loc_fade_buffer = fade.uniform_loc("u_buffer");
        let loc_fade_layer = fade.uniform_loc("u_layer");
        let loc_fade_decay = fade.uniform_loc("u_decay");

        Self {
            write, read, receive, send_output, fade,
            loc_write_input, loc_write_uv_scale,
            loc_read_buffer, loc_read_layer, loc_read_uv_scale,
            loc_rx_input, loc_rx_buffer, loc_rx_layer, loc_rx_feedback, loc_rx_uv_scale,
            loc_so_input, loc_so_passthrough, loc_so_uv_scale,
            loc_fade_buffer, loc_fade_layer, loc_fade_decay,
            quad,
        }
    }

    /// Passthrough: render input texture to currently bound FBO.
    /// uv_scale corrects for hardware texture padding (Width/HardwareWidth, Height/HardwareHeight).
    pub fn write_pass(&self, input_tex: GLuint, uv_scale: [f32; 2]) {
        self.write.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_write_input, 0);
            gl::Uniform2f(self.loc_write_uv_scale, uv_scale[0], uv_scale[1]);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.write.unuse();
    }

    /// Read: output a specific layer from the buffer (wet-only, no mixing).
    /// uv_scale corrects for hardware texture padding, same as write_pass.
    pub fn read_pass(&self, buffer_tex: GLuint, layer: f32, uv_scale: [f32; 2]) {
        self.read.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, buffer_tex);
            gl::Uniform1i(self.loc_read_buffer, 0);
            gl::Uniform1f(self.loc_read_layer, layer);
            gl::Uniform2f(self.loc_read_uv_scale, uv_scale[0], uv_scale[1]);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
        }
        self.read.unuse();
    }

    /// Passthrough: blit input_tex to host FBO scaled by passthrough level.
    /// passthrough=0 → black, passthrough=1 → full input. No buffer read.
    pub fn passthrough_pass(&self, input_tex: GLuint, passthrough: f32, uv_scale: [f32; 2]) {
        self.send_output.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_so_input, 0);
            gl::Uniform1f(self.loc_so_passthrough, passthrough);
            gl::Uniform2f(self.loc_so_uv_scale, uv_scale[0], uv_scale[1]);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.send_output.unuse();
    }

    /// Fade: blit buffer[layer] scaled by decay into current render target.
    /// Used by first Send with decay>0 to preserve previous iteration content.
    pub fn fade_pass(&self, buffer_tex: GLuint, layer: f32, decay: f32) {
        self.fade.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, buffer_tex);
            gl::Uniform1i(self.loc_fade_buffer, 0);
            gl::Uniform1f(self.loc_fade_layer, layer);
            gl::Uniform1f(self.loc_fade_decay, decay);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
        }
        self.fade.unuse();
    }

    /// Receive: mix input with delayed buffer frame.
    /// output = clamp(input + feedback * buffer[layer])
    pub fn receive_pass(&self, input_tex: GLuint, buffer_tex: GLuint, layer: f32, feedback: f32, uv_scale: [f32; 2]) {
        self.receive.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_rx_input, 0);

            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, buffer_tex);
            gl::Uniform1i(self.loc_rx_buffer, 1);
            gl::Uniform1f(self.loc_rx_layer, layer);
            gl::Uniform1f(self.loc_rx_feedback, feedback);
            gl::Uniform2f(self.loc_rx_uv_scale, uv_scale[0], uv_scale[1]);
        }
        self.quad.draw();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.receive.unuse();
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

    // Force consistent attribute locations across all programs sharing the same VAO.
    // GLSL 150 doesn't support layout(location=...), so we must bind before linking.
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
