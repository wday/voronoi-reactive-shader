use gl::types::*;

use pluglib::{OutputProgram, QuadGeometry, ShaderProgram};

static FS_WRITE: &str = include_str!("shaders/write.frag.glsl");

/// The Write plugin's GL passes: `write` (Send·input → buffer layer, a pure
/// overwrite — the write head never reads the buffer) and the shared `output`
/// (passthrough of the input; the record head is visually transparent).
pub struct WriteShaders {
    write: ShaderProgram,
    pub output: OutputProgram,
    loc_write_input: GLint,
    loc_write_uv_scale: GLint,
    loc_write_send: GLint,
    pub quad: QuadGeometry,
}

impl WriteShaders {
    pub fn new() -> Self {
        let write = ShaderProgram::new(FS_WRITE);
        let output = OutputProgram::new();

        let quad = QuadGeometry::new();
        quad.setup_attrs(write.program);

        let loc_write_input = write.uniform_loc("u_input");
        let loc_write_uv_scale = write.uniform_loc("u_uv_scale");
        let loc_write_send = write.uniform_loc("u_send");

        Self {
            write,
            output,
            loc_write_input,
            loc_write_uv_scale,
            loc_write_send,
            quad,
        }
    }

    /// Overwrite the currently bound buffer layer with `send * input_tex`. Blend
    /// must be disabled by the caller — this is a straight write, no read of the
    /// existing slot content.
    pub fn write_pass(&self, input_tex: GLuint, uv_scale: [f32; 2], send: f32) {
        self.write.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.loc_write_input, 0);
            gl::Uniform2f(self.loc_write_uv_scale, uv_scale[0], uv_scale[1]);
            gl::Uniform1f(self.loc_write_send, send);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.write.unuse();
    }
}
