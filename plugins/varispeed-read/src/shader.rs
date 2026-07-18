use gl::types::*;

use pluglib::{QuadGeometry, ShaderProgram};

static FS_READ: &str = include_str!("shaders/read.frag.glsl");

/// The Varispeed Read pass: interpolate two ring layers (`u_frac`) then blend with
/// the live source (Dry/Wet, Blend Space). Input → unit 0 (sampler2D), buffer →
/// unit 1 (sampler2DArray). Leaves unit 0 active with nothing bound (host expects
/// unit-0 usable).
pub struct ReadShaders {
    prog: ShaderProgram,
    loc_input: GLint,
    loc_uv_scale: GLint,
    loc_buffer: GLint,
    loc_layer0: GLint,
    loc_layer1: GLint,
    loc_frac: GLint,
    loc_dry: GLint,
    loc_wet: GLint,
    loc_gamma: GLint,
    pub quad: QuadGeometry,
}

impl ReadShaders {
    pub fn new() -> Self {
        let prog = ShaderProgram::new(FS_READ);
        let quad = QuadGeometry::new();
        quad.setup_attrs(prog.program);

        let loc_input = prog.uniform_loc("u_input");
        let loc_uv_scale = prog.uniform_loc("u_uv_scale");
        let loc_buffer = prog.uniform_loc("u_buffer");
        let loc_layer0 = prog.uniform_loc("u_layer0");
        let loc_layer1 = prog.uniform_loc("u_layer1");
        let loc_frac = prog.uniform_loc("u_frac");
        let loc_dry = prog.uniform_loc("u_dry");
        let loc_wet = prog.uniform_loc("u_wet");
        let loc_gamma = prog.uniform_loc("u_gamma");

        Self {
            prog,
            loc_input,
            loc_uv_scale,
            loc_buffer,
            loc_layer0,
            loc_layer1,
            loc_frac,
            loc_dry,
            loc_wet,
            loc_gamma,
            quad,
        }
    }

    /// Draw into the currently bound FBO. `buffer_tex` may be 0 (samples black) —
    /// pass `wet = 0.0` for a clean passthrough of `input_tex` (empty buffer).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        input_tex: GLuint,
        uv_scale: [f32; 2],
        buffer_tex: GLuint,
        layer0: f32,
        layer1: f32,
        frac: f32,
        dry: f32,
        wet: f32,
        gamma: f32,
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
            gl::Uniform1f(self.loc_layer0, layer0);
            gl::Uniform1f(self.loc_layer1, layer1);
            gl::Uniform1f(self.loc_frac, frac);
            gl::Uniform1f(self.loc_dry, dry);
            gl::Uniform1f(self.loc_wet, wet);
            gl::Uniform1f(self.loc_gamma, gamma);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.prog.unuse();
    }
}
