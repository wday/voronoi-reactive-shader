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

// ── Ingest: write live input + shifted previous frame (recursive feedback) ──
// Each stored frame = live + shifted(previous) * feedback, so echoes compound.
static FS_INGEST: &str = "
#version 150
in vec2 v_uv;
out vec4 out_color;
uniform sampler2D u_input;
uniform sampler2DArray u_prev_tier;
uniform float u_prev_layer;
uniform vec2 u_shift;
uniform float u_feedback;
uniform float u_rotation;
uniform float u_scale;
uniform float u_hue_shift;
uniform float u_sat_shift;
uniform float u_swirl;
uniform float u_mirror;
uniform float u_fold;

// RGB ↔ HSV conversion
vec3 rgb2hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0/3.0, 2.0/3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));
    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}
vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0/3.0, 1.0/3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

// Triangle wave fold: values above threshold mirror back instead of clamping
vec3 fold(vec3 v, float t) {
    float period = t * 2.0;
    return t - abs(mod(v, vec3(period)) - vec3(t));
}

void main() {
    vec4 live = texture(u_input, v_uv);
    // Scale, swirl, rotate around center, then shift
    vec2 centered = v_uv - 0.5;
    centered *= u_scale;
    // Swirl: angular displacement proportional to distance from center
    if (u_swirl != 0.0) {
        float r = length(centered);
        float angle = u_swirl * r;
        float cs = cos(angle);
        float ss = sin(angle);
        centered = vec2(centered.x * cs - centered.y * ss,
                        centered.x * ss + centered.y * cs);
    }
    float c = cos(u_rotation);
    float s = sin(u_rotation);
    vec2 rotated = vec2(centered.x * c - centered.y * s,
                        centered.x * s + centered.y * c);
    vec2 transformed_uv = rotated + 0.5 + u_shift;
    // Mirror or clip at edges
    float inBounds = 1.0;
    if (u_mirror > 0.5) {
        // Reflect: fold UV back into 0..1 range (kaleidoscope)
        transformed_uv = 1.0 - abs(mod(transformed_uv, 2.0) - 1.0);
    } else {
        // Clip: zero out when UV leaves the frame
        inBounds = step(0.0, transformed_uv.x) * step(transformed_uv.x, 1.0)
                 * step(0.0, transformed_uv.y) * step(transformed_uv.y, 1.0);
    }
    vec4 prev = texture(u_prev_tier, vec3(transformed_uv, u_prev_layer)) * inBounds;
    // Hue + saturation shift — accumulates through echoes
    vec3 prev_rgb = prev.rgb * u_feedback;
    if ((u_hue_shift != 0.0 || u_sat_shift != 0.0) && dot(prev_rgb, prev_rgb) > 0.001) {
        vec3 hsv = rgb2hsv(prev_rgb);
        hsv.x = fract(hsv.x + u_hue_shift);
        hsv.y = clamp(hsv.y + u_sat_shift, 0.0, 1.0);
        prev_rgb = hsv2rgb(hsv);
    }
    // Screen blend: brightens without the per-channel hard edges that max() creates
    vec3 color = live.rgb + prev_rgb - live.rgb * prev_rgb;
    // Fold luminance above threshold — inverts instead of clamping
    color = fold(color, u_fold);
    out_color = vec4(clamp(color, 0.0, 1.0), 1.0);
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
//
// Each tier samples multiple "taps" spread across its temporal depth.
// Tier 0 (64 frames, full res): recent sharp echoes
// Tier 3 (4096 frames, 1/8 res): deep blurry memory
//
// The trail_length param controls how far back to reach (0 = shallow, 1 = full depth).
// trail_opacity controls the blend strength of the trail vs live input.
static FS_COMPOSITE: &str = "
#version 150
in vec2 v_uv;
out vec4 out_color;

uniform sampler2D u_input;
uniform sampler2DArray u_tier0;
uniform sampler2DArray u_tier1;
uniform sampler2DArray u_tier2;
uniform sampler2DArray u_tier3;
uniform int u_active_tiers;
uniform float u_trail_opacity;
uniform float u_trail_length;       // 0..1: how deep into history to reach
uniform float u_weight0;
uniform float u_weight1;
uniform float u_weight2;
uniform float u_weight3;

uniform float u_write_ptr0;
uniform float u_write_ptr1;
uniform float u_write_ptr2;
uniform float u_write_ptr3;
uniform float u_depth0;
uniform float u_depth1;
uniform float u_depth2;
uniform float u_depth3;

// Sample a frame from a tier at a given temporal offset from the write head
vec4 sampleTier(sampler2DArray tier, float writePtr, float depth, float offset) {
    float index = mod(writePtr - offset, depth);
    return texture(tier, vec3(v_uv, index));
}

// Sample multiple taps from a tier, spread across its depth.
// Returns weighted average with exponential falloff into the past.
vec4 sampleTierMulti(sampler2DArray tier, float writePtr, float depth, float reach) {
    // reach = how far into history (fraction of depth)
    float maxOffset = max(depth * reach, 2.0);
    vec4 accum = vec4(0.0);
    float total_w = 0.0;

    // 4 taps per tier, exponentially spaced
    // tap 0: ~6% into history (recent echo)
    // tap 3: ~100% of reach (deepest memory)
    for (int t = 0; t < 4; t++) {
        float frac = float(t + 1) / 4.0;       // 0.25, 0.5, 0.75, 1.0
        float offset = frac * maxOffset;
        float w = 1.0 / (1.0 + float(t));      // 1.0, 0.5, 0.33, 0.25 — recency bias
        accum += w * sampleTier(tier, writePtr, depth, offset);
        total_w += w;
    }
    return accum / total_w;
}

void main() {
    vec4 live = texture(u_input, v_uv);
    vec4 trail = vec4(0.0);

    // Per-tier weight: raw, unnormalized — can overdrive above 100%
    float reach = max(u_trail_length, 0.05);

    if (u_active_tiers >= 1 && u_weight0 > 0.001) {
        trail += u_weight0 * sampleTierMulti(u_tier0, u_write_ptr0, u_depth0, reach);
    }
    if (u_active_tiers >= 2 && u_weight1 > 0.001) {
        trail += u_weight1 * sampleTierMulti(u_tier1, u_write_ptr1, u_depth1, reach);
    }
    if (u_active_tiers >= 3 && u_weight2 > 0.001) {
        trail += u_weight2 * sampleTierMulti(u_tier2, u_write_ptr2, u_depth2, reach);
    }
    if (u_active_tiers >= 4 && u_weight3 > 0.001) {
        trail += u_weight3 * sampleTierMulti(u_tier3, u_write_ptr3, u_depth3, reach);
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

/// Cached uniform locations for the composite shader.
/// Queried once at init, used every frame without string lookups.
pub struct CompositeUniforms {
    pub input: GLint,
    pub tiers: [GLint; 4],
    pub write_ptrs: [GLint; 4],
    pub depths: [GLint; 4],
    pub active_tiers: GLint,
    pub trail_opacity: GLint,
    pub trail_length: GLint,
    pub weights: [GLint; 4],
}

/// Cached uniform locations for the ingest (feedback) shader.
pub struct IngestUniforms {
    pub input: GLint,
    pub prev_tier: GLint,
    pub prev_layer: GLint,
    pub shift: GLint,
    pub feedback: GLint,
    pub rotation: GLint,
    pub scale: GLint,
    pub hue_shift: GLint,
    pub sat_shift: GLint,
    pub swirl: GLint,
    pub mirror: GLint,
    pub fold: GLint,
}

/// All three shader programs used by the dream looper.
pub struct DreamShaders {
    pub ingest: ShaderProgram,
    pub downsample: ShaderProgram,
    pub composite: ShaderProgram,
    pub ingest_uniforms: IngestUniforms,
    pub composite_uniforms: CompositeUniforms,
    loc_ds_source_tier: GLint,
    loc_ds_source_layer: GLint,
    pub quad: QuadGeometry,
}

impl DreamShaders {
    pub fn new() -> Self {
        let ingest = ShaderProgram::new(FS_INGEST);
        let downsample = ShaderProgram::new(FS_DOWNSAMPLE);
        let composite = ShaderProgram::new(FS_COMPOSITE);

        let quad = QuadGeometry::new();
        // All three shaders share the same vertex shader, so attribute
        // locations are identical. Set up once with any program.
        quad.setup_attrs(ingest.program);

        // Cache all uniform locations at init time
        let ingest_uniforms = IngestUniforms {
            input: ingest.uniform_loc("u_input"),
            prev_tier: ingest.uniform_loc("u_prev_tier"),
            prev_layer: ingest.uniform_loc("u_prev_layer"),
            shift: ingest.uniform_loc("u_shift"),
            feedback: ingest.uniform_loc("u_feedback"),
            rotation: ingest.uniform_loc("u_rotation"),
            scale: ingest.uniform_loc("u_scale"),
            hue_shift: ingest.uniform_loc("u_hue_shift"),
            sat_shift: ingest.uniform_loc("u_sat_shift"),
            swirl: ingest.uniform_loc("u_swirl"),
            mirror: ingest.uniform_loc("u_mirror"),
            fold: ingest.uniform_loc("u_fold"),
        };
        let loc_ds_source_tier = downsample.uniform_loc("u_source_tier");
        let loc_ds_source_layer = downsample.uniform_loc("u_source_layer");

        let composite_uniforms = CompositeUniforms {
            input: composite.uniform_loc("u_input"),
            tiers: [
                composite.uniform_loc("u_tier0"),
                composite.uniform_loc("u_tier1"),
                composite.uniform_loc("u_tier2"),
                composite.uniform_loc("u_tier3"),
            ],
            write_ptrs: [
                composite.uniform_loc("u_write_ptr0"),
                composite.uniform_loc("u_write_ptr1"),
                composite.uniform_loc("u_write_ptr2"),
                composite.uniform_loc("u_write_ptr3"),
            ],
            depths: [
                composite.uniform_loc("u_depth0"),
                composite.uniform_loc("u_depth1"),
                composite.uniform_loc("u_depth2"),
                composite.uniform_loc("u_depth3"),
            ],
            active_tiers: composite.uniform_loc("u_active_tiers"),
            trail_opacity: composite.uniform_loc("u_trail_opacity"),
            trail_length: composite.uniform_loc("u_trail_length"),
            weights: [
                composite.uniform_loc("u_weight0"),
                composite.uniform_loc("u_weight1"),
                composite.uniform_loc("u_weight2"),
                composite.uniform_loc("u_weight3"),
            ],
        };

        Self {
            ingest, downsample, composite,
            ingest_uniforms, composite_uniforms,
            loc_ds_source_tier, loc_ds_source_layer,
            quad,
        }
    }

    /// Ingest with recursive feedback: live + shifted(previous) * decay.
    /// prev_array_tex is tier 0's texture array, prev_layer is the previous write slot.
    pub fn ingest(
        &self,
        input_tex: GLuint,
        prev_array_tex: GLuint,
        prev_layer: f32,
        shift_x: f32,
        shift_y: f32,
        feedback: f32,
        rotation: f32,
        scale: f32,
        hue_shift: f32,
        sat_shift: f32,
        swirl: f32,
        mirror: f32,
        fold: f32,
    ) {
        let iu = &self.ingest_uniforms;
        self.ingest.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, input_tex);
            gl::Uniform1i(iu.input, 0);

            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, prev_array_tex);
            gl::Uniform1i(iu.prev_tier, 1);
            gl::Uniform1f(iu.prev_layer, prev_layer);

            gl::Uniform2f(iu.shift, shift_x, shift_y);
            gl::Uniform1f(iu.feedback, feedback);
            gl::Uniform1f(iu.rotation, rotation);
            gl::Uniform1f(iu.scale, scale);
            gl::Uniform1f(iu.hue_shift, hue_shift);
            gl::Uniform1f(iu.sat_shift, sat_shift);
            gl::Uniform1f(iu.swirl, swirl);
            gl::Uniform1f(iu.mirror, mirror);
            gl::Uniform1f(iu.fold, fold);
        }
        self.quad.draw();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        self.ingest.unuse();
    }

    /// Downsample from source tier's newest layer into the current FBO target.
    pub fn downsample(&self, source_array_tex: GLuint, source_layer: f32) {
        self.downsample.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D_ARRAY, source_array_tex);
            gl::Uniform1i(self.loc_ds_source_tier, 0);
            gl::Uniform1f(self.loc_ds_source_layer, source_layer);
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
