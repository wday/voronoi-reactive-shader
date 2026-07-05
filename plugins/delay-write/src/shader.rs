use gl::types::*;

use pluglib::{OutputProgram, QuadGeometry, ShaderProgram};

static FS_WRITE: &str = include_str!("shaders/write.frag.glsl");
static FS_FADE: &str = include_str!("shaders/fade.frag.glsl");

/// The Write plugin's GL passes: `write` (input → buffer layer), `fade`
/// (Regen decay pre-pass), and the shared `output` (Thru↔Playback mix).
pub struct WriteShaders {
    write: ShaderProgram,
    fade: ShaderProgram,
    pub output: OutputProgram,
    loc_write_input: GLint,
    loc_write_uv_scale: GLint,
    loc_fade_buffer: GLint,
    loc_fade_layer: GLint,
    loc_fade_decay: GLint,
    pub quad: QuadGeometry,
}

impl WriteShaders {
    pub fn new() -> Self {
        let write = ShaderProgram::new(FS_WRITE);
        let fade = ShaderProgram::new(FS_FADE);
        let output = OutputProgram::new();

        let quad = QuadGeometry::new();
        quad.setup_attrs(write.program);

        let loc_write_input = write.uniform_loc("u_input");
        let loc_write_uv_scale = write.uniform_loc("u_uv_scale");
        let loc_fade_buffer = fade.uniform_loc("u_buffer");
        let loc_fade_layer = fade.uniform_loc("u_layer");
        let loc_fade_decay = fade.uniform_loc("u_decay");

        Self {
            write,
            fade,
            output,
            loc_write_input,
            loc_write_uv_scale,
            loc_fade_buffer,
            loc_fade_layer,
            loc_fade_decay,
            quad,
        }
    }

    /// Write `input_tex` into the currently bound FBO (a buffer layer).
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

    /// Scale buffer[layer] by `decay` into the current render target (Crossfade).
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
}
