// ── shader.rs ── GPU mix shader + fullscreen quad rendering ──
//
// This file has two parts:
//   1. The GLSL shaders (vertex + fragment) — run on the GPU
//   2. The Rust code that compiles, links, and invokes them
//
// THE SHADER (GLSL):
//   Vertex shader: passes through a fullscreen quad (4 corners, -1 to +1).
//   Fragment shader: samples two textures at each pixel, blends with mix().
//   `mix(a, b, t)` = `a * (1-t) + b * t` — same as lerp.
//   This is the core of both decay blend and dry/wet output.
//
// THE QUAD:
//   We render a quad that covers the entire viewport (-1,-1 to +1,+1).
//   UV coordinates (0,0 to 1,1) map to the texture. TRIANGLE_STRIP with
//   4 vertices draws two triangles that fill the screen.
//
// WHY THIS MATTERS FOR THE LOOPER:
//   Step 3 (decay blend): draw_mix(input, delayed, decay) → FBO
//     decay=0 → output=input, decay=1 → output=delayed (frozen)
//   Step 5 (output):      draw_mix(input, delayed, dry_wet) → screen
//     dry_wet=0 → live input, dry_wet=1 → loop output
//
// Rust-isms:
//   `static X: &str = "..."` — string literal with static lifetime.
//     Unlike C's `const char*`, Rust strings carry their length (not null-terminated).
//     We convert to CString when passing to GL (which needs null termination).
//   `.cast()` — pointer type cast, like `(void*)ptr` in C.
//   `as *const _` — cast to raw pointer, letting Rust infer the type.
//   `std::mem::size_of::<f32>()` — like `sizeof(float)` in C.

use gl::types::*;
use std::ffi::CString;
use std::ptr;

// ── GLSL shader source (embedded as Rust string literals) ──
// GLSL is a C-like language that runs on the GPU. Two stages:
//
// VERTEX SHADER: runs once per vertex (4 times for our quad).
//   Inputs (`in`):  per-vertex attributes from the VBO
//   Outputs (`out`): passed to the fragment shader, interpolated across the triangle
//   Must set `gl_Position`: the vertex's position in clip space (-1 to +1)

static VS_SRC: &str = "
#version 150
in vec2 position;       // from VBO: x,y in [-1,+1] (screen corners)
in vec2 texcoord;       // from VBO: u,v in [0,1] (texture corners)
out vec2 v_uv;          // pass UV to fragment shader (GPU interpolates between vertices)

void main() {
    v_uv = texcoord;
    gl_Position = vec4(position, 0.0, 1.0);  // z=0 (2D), w=1 (no perspective)
}
";

// FRAGMENT SHADER: runs once per OUTPUT PIXEL (millions of times in parallel).
//   This is where the GPU's parallelism shines — each pixel is independent.
//   Inputs (`in`):      interpolated values from vertex shader
//   Uniforms:           constants set by CPU before the draw call
//   `texture(sampler, uv)`: sample a pixel from a texture at UV coordinates
//   `mix(a, b, t)`:    lerp — `a*(1-t) + b*t`. Built-in GLSL function.
//   Output (`out`):     the final RGBA color for this pixel

static FS_SRC: &str = "
#version 150
in vec2 v_uv;           // interpolated UV from vertex shader
out vec4 out_color;      // RGBA output for this pixel

uniform sampler2D tex_a;     // texture unit 0 (e.g. live input)
uniform sampler2D tex_b;     // texture unit 1 (e.g. delayed frame)
uniform float mix_amount;    // 0.0 = all tex_a, 1.0 = all tex_b

void main() {
    vec4 a = texture(tex_a, v_uv);   // sample input at this pixel's UV
    vec4 b = texture(tex_b, v_uv);   // sample delayed at same UV
    out_color = mix(a, b, mix_amount); // lerp between them
}
";

/// Compiled shader program + VAO/VBO for the fullscreen quad.
/// Created once, reused every frame.
///
/// C equivalent:
///   typedef struct {
///       GLuint program, vao, vbo;
///       GLint loc_tex_a, loc_tex_b, loc_mix;  // uniform locations
///   } PassthroughShader;
pub struct PassthroughShader {
    program: GLuint,
    vao: GLuint,      // Vertex Array Object — stores vertex format state
    vbo: GLuint,      // Vertex Buffer Object — stores the quad vertex data
    loc_tex_a: GLint, // glGetUniformLocation result for "tex_a"
    loc_tex_b: GLint,
    loc_mix: GLint,
}

impl PassthroughShader {
    pub fn new() -> Self {
        unsafe {
            let vs = compile_shader(VS_SRC, gl::VERTEX_SHADER);
            let fs = compile_shader(FS_SRC, gl::FRAGMENT_SHADER);
            let program = link_program(vs, fs);

            // GPU SHADER COMPILATION MODEL — think of it like C compilation:
            //
            //   Source code (.glsl)  →  compile  →  shader object (.o)
            //   Multiple .o files    →  link     →  program (executable)
            //
            // CreateShader + CompileShader = compile one stage (vertex or fragment)
            //   into a "shader object" — an intermediate representation on the GPU,
            //   like a .o file. It's not runnable on its own.
            //
            // CreateProgram + AttachShader + LinkProgram = link vertex + fragment
            //   stages into a complete GPU "program" — the final executable that the
            //   GPU runs per-vertex and per-pixel. The program contains the compiled
            //   machine code for this specific GPU. It lives in GPU memory (VRAM).
            //
            // After linking, the intermediate shader objects are like .o files after
            // you've produced the binary — they served their purpose. DeleteShader
            // frees them from the GL driver's bookkeeping. The linked program retains
            // all the compiled code it needs.
            //
            // When we later call UseProgram(program), the GPU loads this program into
            // its shader cores. The vertex shader runs once per vertex (4 times for
            // our quad). The fragment shader runs once per output pixel (millions of
            // times in parallel — this is why GPUs are fast for image processing).
            //
            // This compile→link model is universal in OpenGL/Vulkan/DirectX/Metal.
            // It exists because the GPU needs both stages wired together (vertex
            // outputs must match fragment inputs), and the driver may optimize across
            // stages during linking.
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);

            // Fullscreen quad: 4 vertices, each has (x,y) position + (u,v) texcoord
            // TRIANGLE_STRIP order: bottom-left, bottom-right, top-left, top-right
            #[rustfmt::skip]
            static QUAD: [f32; 16] = [
                // pos        // uv
                -1.0, -1.0,   0.0, 0.0,  // bottom-left
                 1.0, -1.0,   1.0, 0.0,  // bottom-right
                -1.0,  1.0,   0.0, 1.0,  // top-left
                 1.0,  1.0,   1.0, 1.0,  // top-right
            ];

            // VAO = Vertex Array Object, VBO = Vertex Buffer Object. Correct.
            //
            // VBO: a GPU-side buffer holding raw vertex data (our 16 floats).
            //   Think of it as malloc on the GPU — just bytes, no structure.
            //
            // VAO: records HOW to interpret the VBO's bytes. It stores:
            //   - which VBO is the data source
            //   - the layout: "bytes 0-7 are position (2 floats), bytes 8-15
            //     are texcoord (2 floats), stride is 16 bytes per vertex"
            //   - which shader attributes are enabled
            //
            // The VAO is like a schema/format descriptor. Once set up, you just
            // bind the VAO before drawing and GL knows everything about the vertex
            // layout. Without VAOs (old GL), you'd re-specify the layout every draw.
            //
            // Pattern: GenX creates a handle (like malloc returning a pointer).
            // BindX makes it "current" — all subsequent calls affect the bound object.
            // This is the OpenGL state machine pattern: bind, configure, unbind.
            // It feels odd coming from direct-API languages, but it's how GL works.
            // All GL objects (textures, buffers, FBOs, VAOs) follow this pattern.
            let mut vao: GLuint = 0;
            let mut vbo: GLuint = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            // Bind VAO first, then VBO. The VAO "records" which VBO we bind and
            // the VertexAttribPointer calls that follow. Later, BindVertexArray(vao)
            // replays all of this state in one call.
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            // Upload quad data to GPU (STATIC_DRAW = uploaded once, drawn many times)
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (QUAD.len() * std::mem::size_of::<f32>()) as isize,
                QUAD.as_ptr().cast(), // cast &f32 to *const c_void
                gl::STATIC_DRAW,
            );

            // STRINGS AS GPU API — your intuition is exactly right.
            //
            // The shader source code (GLSL) declares variables by name:
            //   `in vec2 position;`    ← an input attribute named "position"
            //   `uniform float mix_amount;` ← a parameter named "mix_amount"
            //
            // The GPU compiles this into machine code, but the driver keeps a
            // name→location table (like a symbol table in a linker). We query it
            // by string name to get an integer "location" — an index we use in
            // subsequent calls to set values or describe data layout.
            //
            // It IS message-passing: CPU side says "where is 'position'?", driver
            // looks it up, returns location 0. Then VertexAttribPointer(0, ...)
            // says "location 0 reads 2 floats from offset 0 in the VBO."
            //
            // You could also use `layout(location = 0) in vec2 position;` in GLSL
            // to hardcode the location and skip the string lookup entirely. The
            // string approach is more portable and common in older GL code.
            //
            // Tell GL how to interpret the VBO's raw bytes for each shader input.
            // This connects the VBO data layout to the shader's `in` variables.
            let pos_name = CString::new("position").unwrap();
            let pos_attr = gl::GetAttribLocation(program, pos_name.as_ptr());
            gl::EnableVertexAttribArray(pos_attr as GLuint);
            gl::VertexAttribPointer(
                pos_attr as GLuint,
                2,                                          // 2 components (x, y)
                gl::FLOAT,
                gl::FALSE as GLboolean,                     // don't normalize
                (4 * std::mem::size_of::<f32>()) as i32,    // stride: 4 floats per vertex
                ptr::null(),                                // offset: 0 bytes
            );

            // TEXTURE COORDINATES (texcoords / UVs)
            //
            // A texcoord tells the GPU where to sample a texture for each vertex.
            // It's a 2D coordinate in "texture space" where (0,0) = top-left (or
            // bottom-left depending on convention) and (1,1) = opposite corner.
            //
            // For our fullscreen quad:
            //   vertex (-1,-1) has texcoord (0,0) — bottom-left of screen → BL of texture
            //   vertex (+1,+1) has texcoord (1,1) — top-right of screen → TR of texture
            //
            // The GPU interpolates texcoords across the triangle. So a pixel at the
            // center of the screen gets texcoord (0.5, 0.5) — the center of the texture.
            // The fragment shader receives this as `v_uv` and calls `texture(tex, v_uv)`
            // to sample the color at that point.
            //
            // "UV" is just the conventional name for texture coordinates (U = horizontal,
            // V = vertical), like X/Y but for texture space. "texcoord" and "UV" are
            // interchangeable terms.
            //
            // This is standard in all GPU programming. Without texcoords, the GPU
            // wouldn't know which part of the texture maps to which part of the geometry.
            //
            // Ref: https://learnopengl.com/Getting-started/Textures

            // Attribute "texcoord" = 2 floats at offset 8 bytes (after position)
            let uv_name = CString::new("texcoord").unwrap();
            let uv_attr = gl::GetAttribLocation(program, uv_name.as_ptr());
            gl::EnableVertexAttribArray(uv_attr as GLuint);
            gl::VertexAttribPointer(
                uv_attr as GLuint,
                2,
                gl::FLOAT,
                gl::FALSE as GLboolean,
                (4 * std::mem::size_of::<f32>()) as i32,
                (2 * std::mem::size_of::<f32>()) as *const _, // offset: skip 2 floats
            );

            gl::BindVertexArray(0);

            // ATTRIBUTES vs UNIFORMS — the two ways to feed data into a shader:
            //
            // ATTRIBUTES (`in` variables in GLSL):
            //   Per-vertex data. Changes for each vertex. Read from the VBO.
            //   In our case: `position` and `texcoord` — 4 vertices, 4 values each.
            //   These are NOT a standard set — you name them whatever you want in
            //   your GLSL code. Common conventions: position, normal, texcoord, color.
            //   The VertexAttribPointer calls above tell GL how to read YOUR names
            //   from YOUR VBO layout. It's entirely custom per shader.
            //
            // UNIFORMS (`uniform` variables in GLSL):
            //   Per-draw-call constants. Same value for every vertex and pixel in
            //   one draw call. Set from CPU via Uniform1f/Uniform1i before drawing.
            //   In our case: `tex_a`, `tex_b` (which texture units to sample),
            //   and `mix_amount` (the blend factor).
            //
            // A "uniform location" is just an integer handle (like a file descriptor)
            // that identifies which uniform variable you're setting. You look it up
            // once by name, then use the integer for all subsequent Uniform calls.
            // It's the same pattern as GetAttribLocation but for uniforms.
            //
            // Think of it as:
            //   attributes = function parameters (different per call/vertex)
            //   uniforms   = global config (same for entire draw call)
            //
            // Ref: https://learnopengl.com/Getting-started/Shaders

            // Cache uniform locations (looked up once by name, used every frame by index)
            let tex_a_name = CString::new("tex_a").unwrap();
            let tex_b_name = CString::new("tex_b").unwrap();
            let mix_name = CString::new("mix_amount").unwrap();

            let loc_tex_a = gl::GetUniformLocation(program, tex_a_name.as_ptr());
            let loc_tex_b = gl::GetUniformLocation(program, tex_b_name.as_ptr());
            let loc_mix = gl::GetUniformLocation(program, mix_name.as_ptr());

            Self {
                program,
                vao,
                vbo,
                loc_tex_a,
                loc_tex_b,
                loc_mix,
            }
        }
    }

    /// Render two textures blended: mix=0 → shows tex_a, mix=1 → shows tex_b.
    /// Renders into whatever framebuffer is currently bound (screen or FBO).
    ///
    /// GL state changes: binds program, textures to units 0+1, draws quad, unbinds.
    /// `&self` = read-only access (no mutation needed to draw).
    pub fn draw_mix(&self, tex_a: GLuint, tex_b: GLuint, mix: f32) {
        unsafe {
            // Activate the linked shader program — all subsequent draws use this
            gl::UseProgram(self.program);

            // TEXTURE UNITS — the GPU has numbered "slots" (0, 1, 2, ...) that
            // can each hold one texture. The shader's `sampler2D` uniforms are
            // set to the slot NUMBER (not the texture ID). This indirection lets
            // you swap textures without recompiling the shader.
            //
            // ActiveTexture(TEXTURE0) = "I'm configuring slot 0"
            // BindTexture(tex_a)      = "put this texture in the current slot"
            // Uniform1i(loc, 0)       = "tell the shader: 'tex_a' reads from slot 0"
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex_a);
            gl::Uniform1i(self.loc_tex_a, 0);

            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, tex_b);
            gl::Uniform1i(self.loc_tex_b, 1);

            // Set the mix uniform — the shader reads this for every pixel
            gl::Uniform1f(self.loc_mix, mix);

            // Bind our VAO (which remembers the VBO + vertex layout from setup),
            // then draw 4 vertices as a triangle strip (= 2 triangles = fullscreen quad).
            // This triggers the GPU pipeline: vertex shader × 4, then fragment shader
            // × every pixel in the current viewport.
            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            gl::BindVertexArray(0);

            // Clean up GL state (good practice in a plugin — don't leak bindings)
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            // Restore TEXTURE0 as active unit — shared GL hosts expect this default
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);

            gl::UseProgram(0);
        }
    }
}

/// Destructor — free GL resources when the shader is dropped.
impl Drop for PassthroughShader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteProgram(self.program);
        }
    }
}

// ── Shader compilation helpers ──
// Standard GL boilerplate — same in any language. Create shader, upload source,
// compile, check for errors.

unsafe fn compile_shader(src: &str, ty: GLenum) -> GLuint {
    let shader = gl::CreateShader(ty);
    let c_str = CString::new(src.as_bytes()).unwrap(); // add null terminator for GL
    gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
    gl::CompileShader(shader);

    let mut status = gl::FALSE as GLint;
    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);
    if status != (gl::TRUE as GLint) {
        let mut len = 0;
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
        let mut buf = vec![0u8; len as usize];
        gl::GetShaderInfoLog(
            shader,
            len,
            ptr::null_mut(),
            buf.as_mut_ptr() as *mut GLchar,
        );
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
        gl::GetProgramInfoLog(
            program,
            len,
            ptr::null_mut(),
            buf.as_mut_ptr() as *mut GLchar,
        );
        let msg = String::from_utf8_lossy(&buf);
        panic!("Program link error: {msg}");
    }
    program
}
