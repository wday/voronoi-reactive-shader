// ── looper.rs ── Main plugin struct and per-frame draw loop ──
//
// Rust concept guide (C/Python parallels):
//
// STRUCTS & IMPL
//   `struct Foo { x: i32 }` is like a C struct. No inheritance.
//   `impl Foo { fn bar(&self) }` attaches methods — like putting functions
//   in a C file that take `Foo*` as first arg, but with nicer syntax.
//   `&self` = const pointer (read-only borrow), `&mut self` = mutable pointer.
//
// TRAITS
//   `impl SomeTrait for Foo` is like implementing an interface (Java) or
//   satisfying a protocol. `SimpleFFGLInstance` is the trait that ffgl-core
//   requires — it defines new(), draw(), get_param(), set_param(), etc.
//
// OPTION<T>
//   `Option<RingBuffer>` is like `RingBuffer*` that can be NULL, but checked.
//   `None` = NULL, `Some(value)` = non-null.
//   `.as_ref().unwrap()` = "assert non-null, give me a read-only pointer".
//   `.as_mut().unwrap()` = same but mutable. Panics (crashes) if None.
//
// OWNERSHIP & BORROWING
//   Rust enforces at compile time: either ONE mutable reference OR many
//   read-only references, never both at once. This is why we can't do:
//     let shader = &self.shader;   // borrows self immutably
//     self.ensure_buffer(...);     // ERROR: needs &mut self
//   We work around it by ordering operations carefully or copying values
//   out (like `let state = self.state;`) before the mutable call.
//
// UNSAFE
//   All OpenGL calls are `unsafe` because Rust can't verify GPU state.
//   The `unsafe { }` block is a promise: "I checked this is correct."
//   Same concept as casting in C — the compiler trusts you.
//
// Vec<u8>
//   `Vec<u8>` ≈ `uint8_t*` + length + capacity (like Python's bytearray).
//   `vec![0u8; size]` = calloc(size, 1). Heap-allocated, freed on drop.
//
// DROP
//   `impl Drop for Foo` is the destructor — called automatically when the
//   value goes out of scope. Like C++ destructors or Python's __del__ but
//   guaranteed to run. We use it to free GL resources.

use std::time::Instant;

use gl::types::*;

use ffgl_core::handler::simplified::SimpleFFGLInstance;
use ffgl_core::{FFGLData, GLInput};

use crate::params::{self, LooperParams, NUM_PARAMS};
use crate::pbo::PboTransfer;
use crate::ring_buffer::RingBuffer;
use crate::shader::PassthroughShader;

/// The main plugin instance. Resolume creates one of these per effect slot.
///
/// C equivalent (conceptually):
///   typedef struct {
///       RingBuffer* ring_buffer;   // NULL until first frame
///       LooperParams params;
///       PboTransfer pbo;
///       PassthroughShader* shader; // NULL until first draw (needs GL context)
///       GLuint loop_texture;       // delayed frame uploaded from buffer
///       GLuint blend_texture;      // render target for GPU decay blend
///       GLuint blend_fbo;          // FBO attached to blend_texture
///       size_t write_pos;          // current position in ring buffer
///       size_t prev_write_pos;     // where we wrote last frame (for async download)
///       bool download_pending;     // true after first begin_download
///       size_t frames_since_wrap;  // counts up to loop_len, then resets
///       uint8_t* tmp_pixels;       // reusable scratch buffer (one per resolution)
///       size_t tmp_pixels_len;
///   } VideoLooper;
pub struct VideoLooper {
    ring_buffer: Option<RingBuffer>,
    params: LooperParams,
    pbo: PboTransfer,
    shader: Option<PassthroughShader>,
    loop_texture: GLuint,
    blend_texture: GLuint,
    blend_fbo: GLuint,
    write_pos: usize,
    prev_write_pos: usize,
    download_pending: bool,
    frames_since_wrap: usize,
    tmp_pixels: Vec<u8>,
    frame_count: u64,
}

impl VideoLooper {
    /// Called every frame to ensure we have a ring buffer matching the input
    /// resolution. On first frame or resolution change, (re)allocates everything.
    ///
    /// In C this would be:
    ///   void ensure_buffer(VideoLooper* self, uint32_t w, uint32_t h) {
    ///       if (self->ring_buffer && self->ring_buffer->width == w ...) return;
    ///       free(self->ring_buffer);
    ///       self->ring_buffer = ring_buffer_new(w, h);
    ///       ...
    ///   }
    fn ensure_buffer(&mut self, width: u32, height: u32) {
        // `match` on Option: like `if (ptr != NULL) { ... } else { ... }`
        let needs_realloc = match &self.ring_buffer {
            Some(buf) => !buf.matches_resolution(width, height),
            None => true,
        };

        if needs_realloc {
            let frame_size = (width * height * 4) as usize; // RGBA, 1 byte each
            self.ring_buffer = Some(RingBuffer::new(width, height));
            self.pbo.init(width, height);
            self.write_pos = 0;
            self.prev_write_pos = 0;
            self.download_pending = false;
            self.frames_since_wrap = 0;
            self.tmp_pixels = vec![0u8; frame_size]; // reusable scratch, like calloc

            unsafe {
                if self.loop_texture != 0 {
                    gl::DeleteTextures(1, &self.loop_texture);
                }
                if self.blend_texture != 0 {
                    gl::DeleteTextures(1, &self.blend_texture);
                }
                if self.blend_fbo != 0 {
                    gl::DeleteFramebuffers(1, &self.blend_fbo);
                }

                // loop_texture: we upload the delayed frame here each draw call
                let mut tex: GLuint = 0;
                gl::GenTextures(1, &mut tex);
                Self::init_texture(tex, width, height);
                self.loop_texture = tex;

                // blend_texture: GPU renders the decay blend into this
                // blend_fbo: framebuffer object attached to blend_texture
                // (rendering to an FBO = rendering to a texture instead of screen)
                let mut btex: GLuint = 0;
                gl::GenTextures(1, &mut btex);
                Self::init_texture(btex, width, height);
                self.blend_texture = btex;

                let mut fbo: GLuint = 0;
                gl::GenFramebuffers(1, &mut fbo);
                gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
                gl::FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    btex,
                    0,
                );
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0); // unbind, back to default
                self.blend_fbo = fbo;
            }
        }
    }

    /// Standard GL texture setup — RGBA, linear filtering, clamp edges.
    /// `unsafe fn` = "this function does unsafe things, caller must be in
    /// an unsafe block." Every GL call is technically unsafe since Rust
    /// can't verify GPU-side correctness.
    unsafe fn init_texture(tex: GLuint, width: u32, height: u32) {
        gl::BindTexture(gl::TEXTURE_2D, tex);
        // Allocate GPU memory for the texture (NULL data = don't upload yet)
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            std::ptr::null(), // no initial data, just allocate
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl::BindTexture(gl::TEXTURE_2D, 0);
    }

    /// How many frames fit in one loop at the current BPM?
    /// At 120 BPM with loopBeats=4: 4 × 0.5s = 2s × 30fps = 60 frames
    fn loop_frame_count(&self, bpm: f32) -> usize {
        if bpm <= 0.0 {
            return 120; // fallback: 4 seconds
        }
        let beat_duration = 60.0 / bpm;
        let loop_duration = self.params.loop_beats() as f32 * beat_duration;
        (loop_duration * 30.0).round().max(1.0) as usize
    }
}

// ── SimpleFFGLInstance trait implementation ──
// This is the "interface" that ffgl-core requires. Resolume calls these methods.
// In C terms: these are the function pointers in a vtable that the host invokes.
impl SimpleFFGLInstance for VideoLooper {
    /// Constructor. Called once when the effect is added to a layer.
    /// `gl_loader::init_gl()` + `gl::load_with(...)` = load OpenGL function
    /// pointers at runtime (like GLEW's glewInit() or gladLoadGL()).
    fn new(inst_data: &FFGLData) -> Self {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
        let _ = inst_data; // unused but required by trait signature

        Self {
            ring_buffer: None,  // allocated on first frame (we don't know resolution yet)
            params: LooperParams::new(),
            pbo: PboTransfer::new(),
            shader: None,       // allocated on first draw (needs GL context)
            loop_texture: 0,
            blend_texture: 0,
            blend_fbo: 0,
            write_pos: 0,
            prev_write_pos: 0,
            download_pending: false,
            frames_since_wrap: 0,
            tmp_pixels: Vec::new(), // empty until ensure_buffer
            frame_count: 0,
        }
    }

    /// Called every frame by Resolume. This is where all the work happens.
    /// See architecture.md for the data flow diagram.
    ///
    /// `&mut self` = mutable pointer to this plugin instance
    /// `data`      = host info (BPM, bar phase, viewport size, time)
    /// `frame_data`= input textures from upstream effects (usually exactly 1)
    fn draw(&mut self, data: &FFGLData, frame_data: GLInput) {
        // Lazy-init shader on first draw. We can't do this in new() because
        // the GL context might not be current yet when Resolume calls new().
        if self.shader.is_none() {
            self.shader = Some(PassthroughShader::new());
        }

        // `if let` with a block that returns a value — this is like:
        //   GLuint input_tex;
        //   if (frame_data.num_textures > 0) {
        //       input_tex = frame_data.textures[0].Handle;
        //       ensure_buffer(self, w, h);
        //   } else { glClear(...); return; }
        let input_tex = if !frame_data.textures.is_empty() {
            let t = &frame_data.textures[0];
            self.ensure_buffer(t.Width, t.Height);
            t.Handle as GLuint
        } else {
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            return;
        };

        // Read values into locals so we don't fight the borrow checker.
        // Rust won't let us call `self.pbo.foo()` while also holding a
        // reference to `self.ring_buffer` — both go through `self`.
        // Copying primitives out avoids this.
        let buf_width = self.ring_buffer.as_ref().unwrap().width;
        let buf_height = self.ring_buffer.as_ref().unwrap().height;
        let frame_size = self.tmp_pixels.len();
        let loop_len = self.loop_frame_count(data.host_beat.bpm);
        let decay = self.params.decay();
        let quality = self.params.quality();
        let dry_wet = self.params.dry_wet();

        let t_frame = Instant::now();

        // ── STEP 1: Finish previous frame's async download ──
        // The PBO download we started last frame should be done by now.
        // Map it, memcpy into ring buffer. No per-pixel math — just a copy.
        if self.download_pending {
            if self.pbo.finish_download(&mut self.tmp_pixels) {
                self.ring_buffer
                    .as_mut()       // Option<&mut RingBuffer> — like getting a mut ptr
                    .unwrap()       // assert non-null, get the &mut RingBuffer
                    .write_frame(self.prev_write_pos, &self.tmp_pixels);
            }
        }
        let t_after_download = t_frame.elapsed();

        // ── STEP 2: Upload delayed frame → loop_texture ──
        // Read frame at write_pos (what was there from one loop ago).
        // PBO upload: memcpy into PBO, then glTexSubImage2D from PBO (DMA).
        let read_pos = self.write_pos % loop_len;

        // We need a raw pointer here to avoid a borrow conflict:
        // `self.ring_buffer` is borrowed to get the frame data, but
        // `self.pbo.upload_to_texture()` needs `&mut self.pbo`.
        // In C you'd just pass the pointer — Rust makes us be explicit.
        let frame_ptr = self
            .ring_buffer
            .as_ref()
            .unwrap()
            .get_frame(read_pos)
            .as_ptr(); // get raw *const u8, releases the borrow
        let frame_slice = unsafe { std::slice::from_raw_parts(frame_ptr, frame_size) };
        self.pbo
            .upload_to_texture(frame_slice, self.loop_texture, buf_width, buf_height);

        let t_after_upload = t_frame.elapsed();

        // Save Resolume's current FBO and viewport — Resolume renders effects
        // into its own FBO, not the default framebuffer (0). We must restore it
        // before our final output render, otherwise we draw to the window and
        // Resolume sees nothing (black screen).
        let mut host_fbo: GLint = 0;
        let mut host_viewport: [GLint; 4] = [0; 4];
        unsafe {
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut host_fbo);
            gl::GetIntegerv(gl::VIEWPORT, host_viewport.as_mut_ptr());
        }

        // ── STEP 3: GPU decay blend → blend_fbo ──
        // Render mix(input, delayed, decay) into blend_fbo/blend_texture.
        // This replaces the old CPU blend loop. The GPU does the lerp in
        // the fragment shader — massively parallel, essentially free.
        let shader = self.shader.as_ref().unwrap();
        unsafe {
            // Redirect rendering to our FBO (not Resolume's, not the window)
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.blend_fbo);
            gl::Viewport(0, 0, buf_width as i32, buf_height as i32);
        }
        // decay=0: output=input (no echo), decay=1: output=delayed (frozen loop)
        shader.draw_mix(input_tex, self.loop_texture, decay);

        let t_after_blend = t_frame.elapsed();

        // ── STEP 4: Async download blend result ──
        // Start reading blend_texture into a PBO. The GPU does this via DMA
        // in the background. We'll map it next frame in step 1.
        self.pbo
            .begin_download(&self.blend_texture, buf_width, buf_height);
        self.download_pending = true;
        self.prev_write_pos = self.write_pos;

        // ── STEP 5: Render output into Resolume's FBO ──
        // mix(input, delayed, dry_wet) — same shader, different mix value.
        // dry_wet=0: live input only, dry_wet=1: loop only
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, host_fbo as GLuint);
            gl::Viewport(
                host_viewport[0],
                host_viewport[1],
                host_viewport[2],
                host_viewport[3],
            );
        }
        shader.draw_mix(input_tex, self.loop_texture, dry_wet);

        let t_after_output = t_frame.elapsed();

        // ── Advance write head ──
        self.write_pos += 1;
        self.frames_since_wrap += 1;
        let mut t_degrade = std::time::Duration::ZERO;
        if self.write_pos >= loop_len {
            self.write_pos = 0;
            // Degradation: CPU-side blur applied to all frames once per cycle.
            // This is the only CPU-heavy operation, and it's infrequent
            // (e.g. once every 2 seconds at 120 BPM / 4 beats).
            if quality < 1.0 {
                let t_before_degrade = Instant::now();
                self.ring_buffer
                    .as_mut()
                    .unwrap()
                    .degrade(quality, loop_len);
                t_degrade = t_before_degrade.elapsed();
            }
            self.frames_since_wrap = 0;
        }

        // Log timing every 60 frames (~2 seconds at 30fps)
        self.frame_count += 1;
        if self.frame_count % 60 == 0 || t_degrade.as_micros() > 0 {
            tracing::info!(
                frame = self.frame_count,
                download_us = t_after_download.as_micros(),
                upload_us = (t_after_upload - t_after_download).as_micros(),
                blend_us = (t_after_blend - t_after_upload).as_micros(),
                output_us = (t_after_output - t_after_blend).as_micros(),
                total_us = t_after_output.as_micros(),
                degrade_us = t_degrade.as_micros(),
                loop_len,
                "frame_timing"
            );
        }
    }

    // ── Parameter plumbing ──
    // Resolume calls these to read/write knob values.
    // `-> usize` means "returns usize". No `return` keyword needed —
    // the last expression in a function is the return value (like Ruby).

    fn num_params() -> usize {
        NUM_PARAMS
    }

    // `&'static dyn ParamInfo` = pointer to a ParamInfo trait object that
    // lives forever. In C: `const ParamInfo* param_info(int index)`.
    // `dyn` = dynamic dispatch (vtable), `'static` = not a temporary.
    fn param_info(index: usize) -> &'static dyn ffgl_core::parameters::ParamInfo {
        params::param_info(index)
    }

    fn get_param(&self, index: usize) -> f32 {
        self.params.get(index)
    }

    fn set_param(&mut self, index: usize, value: f32) {
        self.params.set(index, value);
    }

    fn plugin_info() -> ffgl_core::info::PluginInfo {
        ffgl_core::info::PluginInfo {
            unique_id: *b"VdLp",                // 4-byte ID, must be unique across FFGL plugins
            name: *b"Video Looper    ",          // exactly 16 bytes, padded with spaces
            ty: ffgl_core::info::PluginType::Effect, // receives input texture (vs Source)
            about: "Video delay line with feedback and tape degradation".to_string(),
            description: "Beat-synced video looper with decay and quality".to_string(),
        }
    }
}

/// Destructor — Rust calls this automatically when the VideoLooper is freed.
/// We must clean up GL resources manually (Rust's ownership system doesn't
/// know about GPU-side objects).
impl Drop for VideoLooper {
    fn drop(&mut self) {
        unsafe {
            if self.loop_texture != 0 {
                gl::DeleteTextures(1, &self.loop_texture);
            }
            if self.blend_texture != 0 {
                gl::DeleteTextures(1, &self.blend_texture);
            }
            if self.blend_fbo != 0 {
                gl::DeleteFramebuffers(1, &self.blend_fbo);
            }
        }
    }
}
