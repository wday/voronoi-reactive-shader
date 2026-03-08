// ── pyramid.rs ── VRAM-resident temporal pyramid ──
//
// Each tier is a GL_TEXTURE_2D_ARRAY: a stack of 2D textures indexed by layer.
// Think of it like a ring buffer, but entirely on the GPU — no CPU memory.
//
// GL_TEXTURE_2D_ARRAY vs GL_TEXTURE_3D:
//   Both store "stacks" of 2D images, but TEXTURE_2D_ARRAY doesn't interpolate
//   between layers. texture(sampler, vec3(u, v, layer)) returns exactly that
//   layer — no blending with adjacent layers. This is what we want: each layer
//   is a discrete frame, not a volume to sample through.
//
// glFramebufferTextureLayer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, array_tex, 0, layer):
//   Attaches ONE layer of the array texture as the FBO's render target.
//   This lets us render into a specific frame slot without touching others.
//   Like array indexing: array[layer] = render_output.

use gl::types::*;

/// Configuration for one tier of the pyramid.
/// Resolution is relative to the input: scale=0.5 means half width and height.
pub struct TierConfig {
    pub scale: f32,    // 1.0, 0.5, 0.25, 0.125
    pub depth: u32,    // number of frames (array layers)
}

/// Default pyramid: 4 tiers, each ~same VRAM cost.
pub const TIER_CONFIGS: [TierConfig; 4] = [
    TierConfig { scale: 1.0,   depth: 288 },
    TierConfig { scale: 0.5,   depth: 576 },
    TierConfig { scale: 0.25,  depth: 1152 },
    TierConfig { scale: 0.125, depth: 2304 },
];

pub const NUM_TIERS: usize = TIER_CONFIGS.len();

/// One tier of the temporal pyramid — a ring buffer of GPU textures.
pub struct Tier {
    pub array_texture: GLuint,  // GL_TEXTURE_2D_ARRAY handle
    pub fbo: GLuint,            // FBO for rendering into individual layers
    pub width: u32,
    pub height: u32,
    pub depth: u32,             // number of layers (frames)
    pub write_ptr: u32,         // current write position, wraps at depth
}

/// The full pyramid: 4 tiers of texture arrays.
pub struct Pyramid {
    // `[T; N]` is a fixed-size array — a general language type, not just
    // initialization syntax. It means "exactly N elements of type T, inline."
    //
    // In C:     Option<Tier> tiers[4];      // stack-allocated, fixed size
    // In Rust:  [Option<Tier>; NUM_TIERS]   // same thing
    //
    // Contrast with Vec<T> which is heap-allocated and growable (like T* + len + cap).
    // [T; N] is always stack-allocated (or inline in the parent struct), always
    // exactly N elements, size known at compile time. You see it everywhere:
    //   [u8; 4]         — 4 bytes (an IP address, a pixel)
    //   [f32; 16]       — a 4x4 matrix
    //   [GLuint; 2]     — the PBO ping-pong pair in video-looper
    //   [Option<Tier>; 4] — our 4 pyramid tiers, each either Some(tier) or None
    //
    // The `; N` part MUST be a compile-time constant (const, literal, or const fn).
    // That's why NUM_TIERS is `pub const`, not `let`.
    pub tiers: [Option<Tier>; NUM_TIERS],
    pub initialized: bool,
}

impl Pyramid {
    pub fn new() -> Self {
        Self {
            tiers: [None, None, None, None],
            initialized: false,
        }
    }

    /// Allocate all tiers based on input resolution.
    /// Each tier gets its own GL_TEXTURE_2D_ARRAY and FBO.

    // `&mut self` means this method needs exclusive (mutable) access to the
    // struct. In C terms: `void init(Pyramid* self)` where you're going to
    // write through the pointer.
    //
    // The implication: the CALLER must have a mutable reference to the Pyramid.
    // If someone else is reading from `self.tiers` at the same time, Rust won't
    // let you call this — compile error. This is the borrow checker enforcing
    // "no simultaneous readers and writers."
    //
    // Contrast with `&self` (shared/read-only borrow): multiple callers can
    // hold `&self` at once, but none of them can modify the struct.
    // `bind_layer_for_write` below uses `&self` — it only reads tier data to
    // pass to GL, even though GL itself mutates GPU state. Rust doesn't track
    // GPU-side mutations, only Rust-side memory.
    pub fn init(&mut self, input_width: u32, input_height: u32) {
        self.cleanup();

        for (i, config) in TIER_CONFIGS.iter().enumerate() {
            let w = (input_width as f32 * config.scale).max(1.0) as u32;
            let h = (input_height as f32 * config.scale).max(1.0) as u32;

            let mut tex: GLuint = 0;
            let mut fbo: GLuint = 0;

            // `unsafe` is required because the `gl::` functions are FFI calls
            // into the OpenGL driver (a C library). Rust can't verify:
            //   1. That the GL context is valid and current on this thread
            //   2. That the arguments are correct (e.g. tex is a valid handle)
            //   3. That the driver won't corrupt memory
            //
            // In C, ALL of these calls would "just work" (or silently corrupt).
            // Rust makes you explicitly opt in with `unsafe` — it's not saying
            // "this is dangerous," it's saying "the compiler can't prove this
            // is correct, so YOU are responsible for correctness."
            //
            // Every GL call in this codebase is unsafe for the same reason.
            // The `unsafe` block doesn't make the code run differently — it
            // just tells the compiler "I've audited this, trust me."
            unsafe {
                // Allocate the texture array: w × h × depth layers, RGBA8
                gl::GenTextures(1, &mut tex);
                gl::BindTexture(gl::TEXTURE_2D_ARRAY, tex);
                gl::TexImage3D(
                    gl::TEXTURE_2D_ARRAY,
                    0,                      // mipmap level
                    gl::RGBA8 as i32,
                    w as i32,
                    h as i32,
                    config.depth as i32,    // number of layers
                    0,                      // border (must be 0)
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    std::ptr::null(),       // no initial data
                );
                gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
                // CLAMP_TO_BORDER returns black for out-of-bounds UVs instead of
                // repeating the edge row/column (CLAMP_TO_EDGE), which caused
                // visible lines when rotation/swirl maps UVs outside [0,1].
                gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_BORDER as i32);
                gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_BORDER as i32);
                // Border color defaults to (0,0,0,0) — black/transparent
                gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);

                // FBO for rendering into individual layers
                gl::GenFramebuffers(1, &mut fbo);
            }

            let vram_mb = (w as u64 * h as u64 * 4 * config.depth as u64) / (1024 * 1024);
            tracing::info!(
                tier = i,
                width = w,
                height = h,
                depth = config.depth,
                vram_mb = vram_mb,
                "tier allocated"
            );

            self.tiers[i] = Some(Tier {
                array_texture: tex,
                fbo,
                width: w,
                height: h,
                depth: config.depth,
                write_ptr: 0,
            });
        }

        self.initialized = true;
    }

    /// Advance write pointer for a tier, wrapping at depth.
    pub fn advance(&mut self, tier_index: usize) {
        if let Some(tier) = &mut self.tiers[tier_index] {
            tier.write_ptr = (tier.write_ptr + 1) % tier.depth;
        }
    }

    /// Bind a specific layer of a tier as the FBO render target.
    /// After this call, any rendering goes into that layer.
    pub fn bind_layer_for_write(&self, tier_index: usize) {
        if let Some(tier) = &self.tiers[tier_index] {
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, tier.fbo);
                gl::FramebufferTextureLayer(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    tier.array_texture,
                    0,                          // mipmap level
                    tier.write_ptr as i32,      // which layer to write
                );
                gl::Viewport(0, 0, tier.width as i32, tier.height as i32);
            }
        }
    }

    fn cleanup(&mut self) {
        for tier_opt in &mut self.tiers {
            if let Some(tier) = tier_opt.take() {
                unsafe {
                    gl::DeleteTextures(1, &tier.array_texture);
                    gl::DeleteFramebuffers(1, &tier.fbo);
                }
            }
        }
        self.initialized = false;
    }
}

impl Drop for Pyramid {
    fn drop(&mut self) {
        self.cleanup();
    }
}
