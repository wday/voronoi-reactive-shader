// ── shader.rs ── GPU shaders for the Dream Looper ──
//
// Three shader programs:
//   1. Passthrough — copy input texture into a texture array layer
//   2. Downsample  — read from one tier, write (smaller) into the next
//   3. Composite   — sample across all tiers, blend into final output
//
// All use the same fullscreen quad (VAO/VBO).

use gl::types::*;
use std::ffi::CString;
use std::ptr;

// ── Shared vertex shader — fullscreen quad ──
static VS_SRC: &str = "
#version 150
in vec2 position;
in vec2 texcoord;
out vec2 v_uv;
void main() {
    v_uv = texcoord;
    gl_Position = vec4(position, 0.0, 1.0);
}
";

// ── Passthrough: copy a 2D texture into an array layer ──
static FS_PASSTHROUGH: &str = "
#version 150
in vec2 v_uv;
out vec4 out_color;
uniform sampler2D u_input;
void main() {
    out_color = texture(u_input, v_uv);
}
";

// ── Downsample: read from a texture array layer, write to another ──
// The GPU's bilinear filtering handles the downsampling naturally.
static FS_DOWNSAMPLE: &str = "
#version 150
in vec2 v_uv;
out vec4 out_color;
uniform sampler2DArray u_source_tier;
uniform float u_source_layer;
void main() {
    out_color = texture(u_source_tier, vec3(v_uv, u_source_layer));
}
";

// ── Composite: sample from all tiers, blend weighted ──
// Each tier contributes based on its weight. Lower tiers (deeper history,
// lower resolution) have lower weight, creating the "dream trail" effect.
static FS_COMPOSITE: &str = "
#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;          // live input (always full weight)
uniform sampler2DArray u_tier0;
uniform sampler2DArray u_tier1;
uniform sampler2DArray u_tier2;
uniform sampler2DArray u_tier3;
uniform int u_active_tiers;         // how many tiers to sample (1-4)
uniform float u_trail_opacity;      // overall trail strength

// Write pointers and depths for temporal offset calculation
uniform float u_write_ptr0;
uniform float u_write_ptr1;
uniform float u_write_ptr2;
uniform float u_write_ptr3;
uniform float u_depth0;
uniform float u_depth1;
uniform float u_depth2;
uniform float u_depth3;

// Per-tier weight (decreasing with depth = increasing dreaminess)
const float TIER_WEIGHTS[4] = float[4](0.8, 0.4, 0.2, 0.1);

// Sample a frame from a tier at a given temporal offset from the write head
vec4 sampleTier(sampler2DArray tier, float writePtr, float depth, float offset) {
    float index = mod(writePtr - offset, depth);
    return texture(tier, vec3(v_uv, index));
}

void main() {
    vec4 live = texture(u_input, v_uv);
    vec4 trail = vec4(0.0);
    float total_weight = 0.0;

    // Sample the most recent frame from each active tier
    // The offset of 1.0 means \"one frame ago\" — the last fully written frame
    if (u_active_tiers >= 1) {
        float w = TIER_WEIGHTS[0];
        trail += w * sampleTier(u_tier0, u_write_ptr0, u_depth0, 1.0);
        total_weight += w;
    }
    if (u_active_tiers >= 2) {
        float w = TIER_WEIGHTS[1];
        trail += w * sampleTier(u_tier1, u_write_ptr1, u_depth1, 1.0);
        total_weight += w;
    }
    if (u_active_tiers >= 3) {
        float w = TIER_WEIGHTS[2];
        trail += w * sampleTier(u_tier2, u_write_ptr2, u_depth2, 1.0);
        total_weight += w;
    }
    if (u_active_tiers >= 4) {
        float w = TIER_WEIGHTS[3];
        trail += w * sampleTier(u_tier3, u_write_ptr3, u_depth3, 1.0);
        total_weight += w;
    }

    if (total_weight > 0.0) {
        trail /= total_weight;  // normalize
    }

    out_color = mix(live, trail, u_trail_opacity);
}
";

/// Shared fullscreen quad geometry — reused by all shader programs.
pub struct QuadGeometry {
    pub vao: GLuint,
    pub vbo: GLuint,
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

    /// Set up vertex attributes for a given shader program.
    /// Must be called after binding the VAO.
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

/// Compiled shader program handle with cached uniform locations.
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

/// All three shader programs used by the dream looper.
pub struct DreamShaders {
    pub passthrough: ShaderProgram,
    pub downsample: ShaderProgram,
    pub composite: ShaderProgram,
    pub quad: QuadGeometry,
}

impl DreamShaders {
    pub fn new() -> Self {
        let passthrough = ShaderProgram::new(FS_PASSTHROUGH);
        let downsample = ShaderProgram::new(FS_DOWNSAMPLE);
        let composite = ShaderProgram::new(FS_COMPOSITE);

        let quad = QuadGeometry::new();
        // All three shaders share the same vertex shader, so attribute
        // locations are identical. Set up once with any program.
        quad.setup_attrs(passthrough.program);

        Self { passthrough, downsample, composite, quad }
    }

    /// Copy a 2D input texture into the current write layer of a tier.
    pub fn ingest(&self, input_tex: GLuint) {
        self.passthrough.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(self.passthrough.uniform_loc("u_input"), 0);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.passthrough.unuse();
    }

    /// Downsample from source tier's newest layer into the current FBO target.
    pub fn downsample(&self, source_array_tex: GLuint, source_layer: f32) {
        self.downsample.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, source_array_tex);
            gl::Uniform1i(self.downsample.uniform_loc("u_source_tier"), 0);
            gl::Uniform1f(self.downsample.uniform_loc("u_source_layer"), source_layer);
        }
        self.quad.draw();
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
        }
        self.downsample.unuse();
    }
}

// ── Shader compilation helpers ──

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
