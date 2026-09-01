use gl::types::*;
use std::ffi::CString;
use std::ptr;

static VS_SRC: &str = include_str!("shaders/fullscreen.vert.glsl");
static FS_VORONOI: &str = include_str!("shaders/voronoi.frag.glsl");

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
                    2,
                    gl::FLOAT,
                    gl::FALSE as GLboolean,
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
                    2,
                    gl::FLOAT,
                    gl::FALSE as GLboolean,
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
        unsafe {
            gl::UseProgram(self.program);
        }
    }

    fn unuse(&self) {
        unsafe {
            gl::UseProgram(0);
        }
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.program);
        }
    }
}

pub struct VoronoiUniforms {
    pub texel_size: [f32; 2],
    pub fractal: f32,
    pub density: f32,
    pub layer_spread: f32,
    pub layer_mix: f32,
    pub depth: f32,
    pub coastline: f32,
    pub drift_chaos: f32,
    pub warp: f32,
    pub edge_width: f32,
    pub edge_glow: f32,
    pub color_shift: f32,
    pub color_sat: f32,
    pub image_influence: f32,
    pub nc_kernel: f32,
    pub cert_contrast: f32,
    pub cert_brightness: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub image_blend: f32,
    pub anim_time: f32,
    pub beat_locked: f32,
}

pub struct VoronoiShader {
    program: ShaderProgram,
    loc_input: GLint,
    loc_texel_size: GLint,
    loc_fractal: GLint,
    loc_density: GLint,
    loc_layer_spread: GLint,
    loc_layer_mix: GLint,
    loc_depth: GLint,
    loc_coastline: GLint,
    loc_drift_chaos: GLint,
    loc_warp: GLint,
    loc_edge_width: GLint,
    loc_edge_glow: GLint,
    loc_color_shift: GLint,
    loc_color_sat: GLint,
    loc_image_influence: GLint,
    loc_nc_kernel: GLint,
    loc_cert_contrast: GLint,
    loc_cert_brightness: GLint,
    loc_brightness: GLint,
    loc_contrast: GLint,
    loc_image_blend: GLint,
    loc_anim_time: GLint,
    loc_beat_locked: GLint,
    pub quad: QuadGeometry,
}

impl VoronoiShader {
    pub fn new() -> Self {
        let program = ShaderProgram::new(FS_VORONOI);
        let quad = QuadGeometry::new();
        quad.setup_attrs(program.program);

        let loc_input = program.uniform_loc("u_input");
        let loc_texel_size = program.uniform_loc("u_texel_size");
        let loc_fractal = program.uniform_loc("u_fractal");
        let loc_density = program.uniform_loc("u_density");
        let loc_layer_spread = program.uniform_loc("u_layer_spread");
        let loc_layer_mix = program.uniform_loc("u_layer_mix");
        let loc_depth = program.uniform_loc("u_depth");
        let loc_coastline = program.uniform_loc("u_coastline");
        let loc_drift_chaos = program.uniform_loc("u_drift_chaos");
        let loc_warp = program.uniform_loc("u_warp");
        let loc_edge_width = program.uniform_loc("u_edge_width");
        let loc_edge_glow = program.uniform_loc("u_edge_glow");
        let loc_color_shift = program.uniform_loc("u_color_shift");
        let loc_color_sat = program.uniform_loc("u_color_sat");
        let loc_image_influence = program.uniform_loc("u_image_influence");
        let loc_nc_kernel = program.uniform_loc("u_nc_kernel");
        let loc_cert_contrast = program.uniform_loc("u_cert_contrast");
        let loc_cert_brightness = program.uniform_loc("u_cert_brightness");
        let loc_brightness = program.uniform_loc("u_brightness");
        let loc_contrast = program.uniform_loc("u_contrast");
        let loc_image_blend = program.uniform_loc("u_image_blend");
        let loc_anim_time = program.uniform_loc("u_anim_time");
        let loc_beat_locked = program.uniform_loc("u_beat_locked");

        Self {
            program,
            loc_input,
            loc_texel_size,
            loc_fractal,
            loc_density,
            loc_layer_spread,
            loc_layer_mix,
            loc_depth,
            loc_coastline,
            loc_drift_chaos,
            loc_warp,
            loc_edge_width,
            loc_edge_glow,
            loc_color_shift,
            loc_color_sat,
            loc_image_influence,
            loc_nc_kernel,
            loc_cert_contrast,
            loc_cert_brightness,
            loc_brightness,
            loc_contrast,
            loc_image_blend,
            loc_anim_time,
            loc_beat_locked,
            quad,
        }
    }

    /// Render one frame. `input_tex` is the host's input on unit 0; pass 0 when
    /// the host gives none (the shader then reads black and behaves as a pure
    /// generator, which is what Image Influence = 0 already selects).
    pub fn render(&self, input_tex: GLuint, u: &VoronoiUniforms) {
        self.program.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_input, 0);
            gl::Uniform2f(self.loc_texel_size, u.texel_size[0], u.texel_size[1]);
            gl::Uniform1f(self.loc_fractal, u.fractal);
            gl::Uniform1f(self.loc_density, u.density);
            gl::Uniform1f(self.loc_layer_spread, u.layer_spread);
            gl::Uniform1f(self.loc_layer_mix, u.layer_mix);
            gl::Uniform1f(self.loc_depth, u.depth);
            gl::Uniform1f(self.loc_coastline, u.coastline);
            gl::Uniform1f(self.loc_drift_chaos, u.drift_chaos);
            gl::Uniform1f(self.loc_warp, u.warp);
            gl::Uniform1f(self.loc_edge_width, u.edge_width);
            gl::Uniform1f(self.loc_edge_glow, u.edge_glow);
            gl::Uniform1f(self.loc_color_shift, u.color_shift);
            gl::Uniform1f(self.loc_color_sat, u.color_sat);
            gl::Uniform1f(self.loc_image_influence, u.image_influence);
            gl::Uniform1f(self.loc_nc_kernel, u.nc_kernel);
            gl::Uniform1f(self.loc_cert_contrast, u.cert_contrast);
            gl::Uniform1f(self.loc_cert_brightness, u.cert_brightness);
            gl::Uniform1f(self.loc_brightness, u.brightness);
            gl::Uniform1f(self.loc_contrast, u.contrast);
            gl::Uniform1f(self.loc_image_blend, u.image_blend);
            gl::Uniform1f(self.loc_anim_time, u.anim_time);
            gl::Uniform1f(self.loc_beat_locked, u.beat_locked);
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
