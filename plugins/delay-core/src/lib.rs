//! delay-core — the shared GPU ring-buffer registry + frame barrier, behind a C ABI.
//!
//! Single owner across the two plugin DLLs (delay-write, delay-tap). They do NOT
//! statically import this library; each runtime-loads it from its own directory
//! (see `pluglib`), and the Windows/macOS/Linux loaders refcount a DLL by
//! canonical path — so both plugins bind ONE loaded delay_core, hence one
//! `static REGISTRY` and one lock. That single ownership is what makes the frame
//! barrier deterministic: the monotonic per-channel `frame_index` advances
//! exactly once per host frame no matter how many Tap/Write instances tick it
//! (spec v0.2 absolute addressing).
//!
//! GL note: OpenGL *function pointers* are per-DLL, so this library runs its own
//! `gl::load_with` (see `ensure_gl`). GL *objects* (texture names) are shared
//! across the plugins because Resolume gives every FFGL effect the same GL
//! context — so a texture allocated here is bound directly by the plugins.
//! Only scalars cross the C ABI; no structs, no GL calls from the plugin side
//! against core-owned state beyond binding the returned texture name.

use gl::types::*;
use std::sync::{Mutex, Once};

use delay_dsp::Ring;

/// Ring-buffer depth (layers of the 2D texture array). Caps the maximum loop.
///
/// 120 layers = 2 s at 60 fps. That is well past the 1/16..1/4-note (and 2-3
/// frame) loops this actually gets played at, and halving it from the old 240 is
/// what pays for the tape going full-res (`WRITE-TAPE-SCALE` = 1.0): at 1080p
/// RGBA16F that is ~2 GB/channel, ~4 GB with both channels live, on a 12 GB card.
/// Subdivisions longer than ~1 bar now clamp here instead of at 240.
const BUFFER_DEPTH: u32 = 120;

/// Number of shared channels. NOTE: kept a constant, not baked into assumptions —
/// raising it is a planned fast-follow. `REGISTRY`'s initializer below must be
/// updated in lockstep (const arrays of non-Copy types can't use `[x; N]`).
const NUM_CHANNELS: usize = 2;

/// The GPU ring buffer for one channel. Allocated lazily by the first writer.
/// v0.2: no stored write position — slots are addressed absolutely off the
/// channel's monotonic `frame_index` (see `Channel`).
struct Buffer {
    texture_array: GLuint,
    loop_length: u32,
    width: u32,
    height: u32,
}

/// Per-channel registry slot. `refcount`, the monotonic `frame_index`, and the
/// frame-barrier state live here (not on `Buffer`) so they survive reallocation
/// and exist before any buffer does — a Tap may `acquire` and tick a channel
/// before any Write has allocated it.
struct Channel {
    buffer: Option<Buffer>,
    refcount: u32,
    /// Monotonic per-channel frame counter (spec v0.2). Advanced exactly once per
    /// host frame by the barrier in `dc_frame_tick`. Both the write slot
    /// (`frame_index % buf_size`) and read slot (`(frame_index − loop_length) %
    /// buf_size`) derive from this single value, so they are independent of draw
    /// order. Wraps at u64 (≈ 10^10 years at 60 fps — never in practice).
    frame_index: u64,
    /// Host-provided frame id of the frame last ticked (barrier de-dup key).
    frame_id: u64,
    frame_active: bool,
}

const EMPTY: Channel = Channel {
    buffer: None,
    refcount: 0,
    frame_index: 0,
    frame_id: 0,
    frame_active: false,
};

// NUM_CHANNELS == 2. Extend this literal if NUM_CHANNELS changes.
static REGISTRY: Mutex<[Channel; NUM_CHANNELS]> = Mutex::new([EMPTY, EMPTY]);

/// Load this DLL's own GL function pointers. Idempotent; GL fn pointers are
/// per-DLL so each of delay-core / delay-write / delay-tap does this once.
fn ensure_gl() {
    static GL_INIT: Once = Once::new();
    GL_INIT.call_once(|| {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
    });
}

/// Maximum ring depth, so plugins can clamp their loop-length UI to the buffer.
#[no_mangle]
pub extern "C" fn dc_buffer_depth() -> u32 {
    BUFFER_DEPTH
}

/// Register an instance's use of a channel. Balanced by `dc_release`.
#[no_mangle]
pub extern "C" fn dc_acquire(channel: usize) {
    if channel >= NUM_CHANNELS {
        return;
    }
    let mut reg = REGISTRY.lock().unwrap();
    reg[channel].refcount += 1;
}

/// Unregister an instance. When the last user of a channel releases it, the
/// buffer slot is dropped and the barrier reset.
///
/// Deliberately does NO GL here: `dc_release` is reached from a plugin's Drop /
/// param-change, which may run off the GL thread (context not current) — a GL
/// delete there risks a teardown crash. So the texture name is leaked until
/// Resolume exits (the same as the shipped v2 delay line, which never freed it).
/// The only GL delete is in `dc_frame_tick`'s realloc path, which always
/// runs inside `draw()` where the context is current. If per-channel VRAM churn
/// ever matters, the fix is a delete queue drained from `draw()` — noted in the
/// v3 devlog.
#[no_mangle]
pub extern "C" fn dc_release(channel: usize) {
    if channel >= NUM_CHANNELS {
        return;
    }
    let mut reg = REGISTRY.lock().unwrap();
    let ch = &mut reg[channel];
    ch.refcount = ch.refcount.saturating_sub(1);
    if ch.refcount == 0 {
        ch.buffer = None; // leaks the GL texture name (see doc comment)
        ch.frame_active = false;
        ch.frame_index = 0; // fresh counter if the channel is re-acquired
    }
}

/// Tick `channel` for the current host frame (spec v0.2). Called by BOTH plugins:
/// a Write ticks with real `loop_length`/`width`/`height`; a Tap ticks with all
/// three set to 0 (it never sizes or resizes the buffer, it only needs the
/// frame_index to advance).
///
/// Two responsibilities, deliberately decoupled so draw order can't break either:
///
/// 1. **Buffer sizing** (only when `width`/`height` are non-zero — i.e. a Write):
///    allocate, or reallocate on a size change, and latch `loop_length`. NOT
///    gated on the barrier below, because a Tap may tick first and win the
///    barrier; the Write must still be able to (re)allocate afterwards. Single
///    writer per channel (`COMPOSE-SINGLE-WRITER`) means only one caller sets
///    `loop_length` per frame, so this stays stable within a frame.
///
/// 2. **The frame barrier**: the first caller of a given `frame_id` advances the
///    monotonic `frame_index` once and returns `1`; every later caller in the
///    same `frame_id` leaves it untouched and returns `0`. `frame_id` is the
///    host-provided per-frame identifier (FFGLData.host_time in ms), shared by
///    every instance drawn in the same host frame. Advancing exactly once is what
///    makes the write/read slots (computed by the plugins from `dc_frame_index` +
///    `dc_buf_size`) order-independent.
///
/// Returns the first-of-frame flag. Read the counter with `dc_frame_index` and
/// the texture with `dc_tex`.
#[no_mangle]
pub extern "C" fn dc_frame_tick(
    channel: usize,
    loop_length: u32,
    width: u32,
    height: u32,
    frame_id: u64,
) -> u32 {
    if channel >= NUM_CHANNELS {
        return 0;
    }

    // (1) Buffer sizing — a Write path (non-zero dims). A Tap passes zeros and
    // skips this entirely, so it never allocates or disturbs the buffer.
    let provides_buffer = width != 0 && height != 0;
    if provides_buffer {
        ensure_gl();
    }
    let mut reg = REGISTRY.lock().unwrap();
    let ch = &mut reg[channel];

    if provides_buffer {
        let needs_alloc = match &ch.buffer {
            Some(b) => b.width != width || b.height != height,
            None => true,
        };
        if needs_alloc {
            if let Some(old) = ch.buffer.take() {
                unsafe {
                    if old.texture_array != 0 {
                        gl::DeleteTextures(1, &old.texture_array);
                    }
                }
            }
            let tex = alloc_buffer(width, height);
            // RGBA16F = 8 bytes/texel. width/height are the (Write-scaled) tape
            // dims, so this reflects the real, reduced footprint.
            let vram_mb = (width as u64 * height as u64 * 8 * BUFFER_DEPTH as u64) / (1024 * 1024);
            tracing::info!(channel, width, height, depth = BUFFER_DEPTH, vram_mb, "channel buffer allocated");
            ch.buffer = Some(Buffer {
                texture_array: tex,
                // Clamp identically to delay-dsp's Ring so buf_size is reconciled
                // with the slot arithmetic the plugins run.
                loop_length: Ring::new(loop_length, BUFFER_DEPTH - 1).loop_length,
                width,
                height,
            });
        } else if let Some(buf) = ch.buffer.as_mut() {
            // Retune loop length without a realloc. Single writer per channel, so
            // this is set at most once per frame → buf_size is stable per frame.
            let new_len = Ring::new(loop_length, BUFFER_DEPTH - 1).loop_length;
            if new_len != buf.loop_length {
                // The loop length IS the ring modulus (buf_size = loop_length+1),
                // so changing it re-maps every slot's frame_index → the whole tape
                // is now temporally incoherent. Reseed it to black and let the read
                // warm up from black over one lap (CORE-DEPTH), exactly like a
                // fresh allocation. This is what stops a later *expansion* from
                // resurrecting stale frames in slots the shorter loop never
                // rewrote. Fires only on a real change (WRITE-TIME-LATCH holds
                // loop_length steady frame-to-frame), not every frame.
                unsafe { clear_texture_array(buf.texture_array) };
                buf.loop_length = new_len;
                tracing::info!(channel, loop_length = new_len, "loop length changed → tape reseeded to black");
            }
        }
    }

    // (2) Frame barrier — advance the monotonic counter once per host frame.
    let first_of_frame = !ch.frame_active || ch.frame_id != frame_id;
    if first_of_frame {
        ch.frame_index = ch.frame_index.wrapping_add(1);
        ch.frame_id = frame_id;
        ch.frame_active = true;
    }
    first_of_frame as u32
}

/// Texture-array name for the channel's ring buffer, or 0 if unallocated.
#[no_mangle]
pub extern "C" fn dc_tex(channel: usize) -> u32 {
    if channel >= NUM_CHANNELS {
        return 0;
    }
    REGISTRY.lock().unwrap()[channel]
        .buffer
        .as_ref()
        .map_or(0, |b| b.texture_array)
}

/// The channel's monotonic frame counter (spec v0.2). Plugins map it onto slots:
/// write slot = `frame_index % dc_buf_size`, read slot = `(frame_index −
/// loop_length) % dc_buf_size`. Advanced once per host frame by `dc_frame_tick`;
/// 0 before the first tick. Unlike the old `dc_write_pos`, this is stable for the
/// whole frame regardless of draw order.
#[no_mangle]
pub extern "C" fn dc_frame_index(channel: usize) -> u64 {
    if channel >= NUM_CHANNELS {
        return 0;
    }
    REGISTRY.lock().unwrap()[channel].frame_index
}

/// Ring size = loop_length + 1 (number of live layers), or 0 if unallocated.
/// Tap uses this to map an offset onto a layer; 0 means "no buffer yet".
#[no_mangle]
pub extern "C" fn dc_buf_size(channel: usize) -> u32 {
    if channel >= NUM_CHANNELS {
        return 0;
    }
    REGISTRY.lock().unwrap()[channel]
        .buffer
        .as_ref()
        .map_or(0, |b| b.loop_length + 1)
}

/// Allocate a cleared RGBA16F 2D texture array of `BUFFER_DEPTH` layers. Returns
/// the texture name, or 0 on GL error. Float storage removes the 8-bit banding in
/// the (linear-blended) feedback tail and holds values with headroom; over-unity
/// still can't persist across laps because the loop clips at Resolume's host FBO
/// (see DEFER-FLOAT). The Write sizes the tape by its TAPE_SCALE, now 1.0, so a
/// full 120-layer tape is ~2 GB/channel at 1080p. FBOs are NOT created here —
/// each plugin owns its own FBO and attaches this texture's layers to it, so only
/// the (process-shared) texture name crosses the DLL boundary.
fn alloc_buffer(width: u32, height: u32) -> GLuint {
    unsafe {
        let mut tex: GLuint = 0;
        gl::GenTextures(1, &mut tex);
        gl::BindTexture(gl::TEXTURE_2D_ARRAY, tex);
        gl::TexImage3D(
            gl::TEXTURE_2D_ARRAY,
            0,
            gl::RGBA16F as i32,
            width as i32,
            height as i32,
            BUFFER_DEPTH as i32,
            0,
            gl::RGBA,
            gl::HALF_FLOAT,
            std::ptr::null(),
        );
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);

        // Clear every layer to black (TexImage3D with null leaves it undefined) so
        // the first read never sees VRAM garbage. GPU-side, no CPU upload.
        clear_texture_array(tex);

        let err = gl::GetError();
        if err != gl::NO_ERROR {
            tracing::error!(gl_error = err, width, height, depth = BUFFER_DEPTH, "buffer allocation failed");
            gl::DeleteTextures(1, &tex);
            return 0;
        }
        tex
    }
}

/// Clear every layer of the ring's 2D texture array to transparent black,
/// GPU-side: an FBO + `glClear` per layer, no multi-GB CPU upload (120 layers of
/// full-res 1080p RGBA16F would be ~2 GB to stream otherwise). Used on allocation and on any
/// loop-length change (the modulus change scrambles the temporal mapping of every
/// slot, so the whole tape must reseed).
///
/// Runs on the GL thread (inside `dc_frame_tick`'s draw path) and shares
/// Resolume's context, so it saves and restores the only host state it disturbs:
/// the bound framebuffer, the clear colour, and the scissor enable.
///
/// # Safety
/// A current GL context must exist and `tex` must be a `GL_TEXTURE_2D_ARRAY` with
/// at least `BUFFER_DEPTH` layers.
unsafe fn clear_texture_array(tex: GLuint) {
    let mut prev_fbo: GLint = 0;
    let mut prev_clear: [GLfloat; 4] = [0.0; 4];
    gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut prev_fbo);
    gl::GetFloatv(gl::COLOR_CLEAR_VALUE, prev_clear.as_mut_ptr());
    let scissor_was_on = gl::IsEnabled(gl::SCISSOR_TEST) == gl::TRUE;

    let mut fbo: GLuint = 0;
    gl::GenFramebuffers(1, &mut fbo);
    gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
    if scissor_was_on {
        gl::Disable(gl::SCISSOR_TEST); // glClear is scissor-clipped; make sure it isn't
    }
    gl::ClearColor(0.0, 0.0, 0.0, 0.0);
    for layer in 0..BUFFER_DEPTH {
        gl::FramebufferTextureLayer(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, tex, 0, layer as i32);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    gl::FramebufferTextureLayer(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, 0, 0, 0);
    gl::BindFramebuffer(gl::FRAMEBUFFER, prev_fbo as GLuint);
    gl::DeleteFramebuffers(1, &fbo);
    if scissor_was_on {
        gl::Enable(gl::SCISSOR_TEST);
    }
    gl::ClearColor(prev_clear[0], prev_clear[1], prev_clear[2], prev_clear[3]);
}
