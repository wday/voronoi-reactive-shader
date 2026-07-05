//! delay-core — the shared GPU ring-buffer registry + frame barrier, behind a C ABI.
//!
//! Single owner across the two plugin DLLs (delay-write, delay-tap). They do NOT
//! statically import this library; each runtime-loads it from its own directory
//! (see `pluglib`), and the Windows/macOS/Linux loaders refcount a DLL by
//! canonical path — so both plugins bind ONE loaded delay_core, hence one
//! `static REGISTRY` and one lock. That single ownership is what makes the frame
//! barrier deterministic when several Write instances hit the same channel in one
//! frame (Stage 3 additive accumulation).
//!
//! GL note: OpenGL *function pointers* are per-DLL, so this library runs its own
//! `gl::load_with` (see `ensure_gl`). GL *objects* (texture names) are shared
//! across the plugins because Resolume gives every FFGL effect the same GL
//! context — so a texture allocated here is bound directly by the plugins.
//! Only scalars cross the C ABI; no structs, no GL calls from the plugin side
//! against core-owned state beyond binding the returned texture name.

use gl::types::*;
use std::sync::{Mutex, Once};

/// Ring-buffer depth (layers of the 2D texture array). Caps the maximum loop.
const BUFFER_DEPTH: u32 = 240;

/// Number of shared channels. NOTE: kept a constant, not baked into assumptions —
/// raising it is a planned fast-follow. `REGISTRY`'s initializer below must be
/// updated in lockstep (const arrays of non-Copy types can't use `[x; N]`).
const NUM_CHANNELS: usize = 2;

/// The GPU ring buffer for one channel. Allocated lazily by the first writer.
struct Buffer {
    texture_array: GLuint,
    write_pos: u32,
    loop_length: u32,
    width: u32,
    height: u32,
}

/// Per-channel registry slot. `refcount` and the frame-barrier state live here
/// (not on `Buffer`) so they survive reallocation and exist before any buffer
/// does — a Tap may `acquire` a channel before any Write has allocated it.
struct Channel {
    buffer: Option<Buffer>,
    refcount: u32,
    /// Host-provided frame id of the frame currently being written (see barrier).
    frame_id: u64,
    frame_active: bool,
}

const EMPTY: Channel = Channel {
    buffer: None,
    refcount: 0,
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
/// The only GL delete is in `dc_begin_frame_write`'s realloc path, which always
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
    }
}

/// Begin a write for `channel` this frame. Allocates (or reallocates on a size
/// change) the ring buffer, then applies the **frame barrier**:
///
/// - The first writer of a given `frame_id` advances `write_pos` (owns the new
///   slot / the fade+clear) and returns `1`.
/// - Subsequent writers in the same `frame_id` do NOT advance and return `0` —
///   they accumulate into the same slot the first writer opened.
///
/// `frame_id` is a host-provided per-frame identifier (FFGLData.host_time in ms),
/// shared by every plugin instance drawn in the same host frame. This replaces
/// the old 2 ms wall-clock gate, which mis-fired under variable render load and
/// could not order multi-writer accumulation.
///
/// Returns the first-writer flag. Read back the resulting slot with `dc_tex` /
/// `dc_write_pos`.
#[no_mangle]
pub extern "C" fn dc_begin_frame_write(
    channel: usize,
    loop_length: u32,
    width: u32,
    height: u32,
    frame_id: u64,
) -> u32 {
    if channel >= NUM_CHANNELS || width == 0 || height == 0 {
        return 0;
    }
    ensure_gl();
    let mut reg = REGISTRY.lock().unwrap();
    let ch = &mut reg[channel];

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
        let vram_mb = (width as u64 * height as u64 * 4 * BUFFER_DEPTH as u64) / (1024 * 1024);
        tracing::info!(channel, width, height, depth = BUFFER_DEPTH, vram_mb, "channel buffer allocated");
        ch.buffer = Some(Buffer {
            texture_array: tex,
            write_pos: 0,
            loop_length: loop_length.clamp(1, BUFFER_DEPTH - 1),
            width,
            height,
        });
        // Fresh buffer: let the next writer be treated as first-of-frame.
        ch.frame_active = false;
    }

    let first_writer = !ch.frame_active || ch.frame_id != frame_id;
    if first_writer {
        // The first writer of a frame owns the frame's parameters and the slot
        // advance. Later writers on the same channel/frame (additive
        // accumulation) leave loop_length and write_pos untouched, so buf_size
        // and the target slot stay stable while they accumulate.
        let ll = loop_length.clamp(1, BUFFER_DEPTH - 1); // -1 leaves a spare slot
        let buf = ch.buffer.as_mut().unwrap();
        buf.loop_length = ll;
        let buf_size = ll + 1;
        buf.write_pos = (buf.write_pos + 1) % buf_size;
        ch.frame_id = frame_id;
        ch.frame_active = true;
    }
    first_writer as u32
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

/// Current write layer (the newest frame just written), or 0 if unallocated.
#[no_mangle]
pub extern "C" fn dc_write_pos(channel: usize) -> u32 {
    if channel >= NUM_CHANNELS {
        return 0;
    }
    REGISTRY.lock().unwrap()[channel]
        .buffer
        .as_ref()
        .map_or(0, |b| b.write_pos)
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

/// Allocate a cleared RGBA8 2D texture array of `BUFFER_DEPTH` layers. Returns
/// the texture name, or 0 on GL error. FBOs are NOT created here — each plugin
/// owns its own FBO and attaches this texture's layers to it, so only the
/// (process-shared) texture name crosses the DLL boundary.
fn alloc_buffer(width: u32, height: u32) -> GLuint {
    unsafe {
        let mut tex: GLuint = 0;
        gl::GenTextures(1, &mut tex);
        gl::BindTexture(gl::TEXTURE_2D_ARRAY, tex);
        gl::TexImage3D(
            gl::TEXTURE_2D_ARRAY,
            0,
            gl::RGBA8 as i32,
            width as i32,
            height as i32,
            BUFFER_DEPTH as i32,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            std::ptr::null(),
        );
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D_ARRAY, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

        // Clear every layer to black to avoid VRAM-garbage on first read.
        let black = vec![0u8; (width * height * 4) as usize];
        for layer in 0..BUFFER_DEPTH {
            gl::TexSubImage3D(
                gl::TEXTURE_2D_ARRAY,
                0,
                0, 0, layer as i32,
                width as i32, height as i32, 1,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                black.as_ptr().cast(),
            );
        }
        gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);

        let err = gl::GetError();
        if err != gl::NO_ERROR {
            tracing::error!(gl_error = err, width, height, depth = BUFFER_DEPTH, "buffer allocation failed");
            gl::DeleteTextures(1, &tex);
            return 0;
        }
        tex
    }
}
