//! varispeed-core — the shared GPU ring buffers + write-cursor barriers for the
//! Varispeed atom, behind a C ABI. **Two channels** (VS-CHANNELS), **full-ring**
//! topology, **full-res RGBA16F** storage (VS-STORAGE).
//!
//! Fully separate from `delay-core`: the shipped delay (`dc_*`) is never touched
//! (VS-ISOLATION). Like delay-core, it is one owner across the two plugin DLLs
//! (Varispeed Read + Write), which runtime-load it from their own directory so
//! both bind ONE copy → one `static REGISTRY` and one lock. GL *objects* (the
//! texture name) are shared via Resolume's shared GL context; only scalars cross
//! the ABI.
//!
//! Every entry point takes a `channel` selecting one of `NUM_CHANNELS` independent
//! tapes, so a Read/Write pair on channel 0 and another on channel 1 are two
//! separate feedback networks. Channels allocate lazily on their first recording
//! Write, so an unused channel costs no VRAM. An out-of-range channel is a no-op
//! returning a zero value — never a panic across the C ABI.
//!
//! **Freeze for free.** The Write advances the write cursor `record_index` (and
//! records a frame) via [`vc_write_tick`] ONLY while recording. Freezing (the
//! Write's Send=0) just stops calling it, so the cursor — and therefore the loop
//! window the Read plays — parks in place with no separate freeze-state to
//! coordinate. The Read never advances the cursor; it floats its own fractional
//! read position (varispeed-dsp) over the ring and reads `record_index` only to
//! anchor its window.

use gl::types::*;
use std::sync::{Mutex, Once};

/// Ring depth (layers of the 2D texture array), `MAX_LOOP_FRAMES + 1` (VS-CAPACITY).
/// 61 layers = a 60-frame (1.0 s @60 fps) max loop plus the stitch slot: at the
/// deepest tap the Read is on layer `R - 59` while the Write is about to fill
/// `R + 1`, 60 apart, so they never alias.
///
/// Varispeed is the **short-loop fractal box** — the smoother dynamics of short
/// loops are what make fractal and Euler-soup feedback work; long single-channel
/// delay is `DlyT`/`DlyW`'s job. Dropping Reverse (which needed `2 * MAX_LOOP_FRAMES`
/// to hold two blocks) plus the 1 s cap is what pays for a second channel at
/// full-res RGBA16F: **965 MiB per channel at 1080p, 1.88 GiB for both**, against
/// ~3.98 GB for the old single 240-layer tape.
const BUFFER_DEPTH: u32 = 61;

/// Independent tapes. Two Read/Write pairs on different channels are two separate
/// feedback networks, each with its own in-loop FX stack. `REGISTRY`'s initializer
/// must be written out in lockstep (a const array of non-Copy types can't use
/// `[x; N]`), as in delay-core.
const NUM_CHANNELS: usize = 2;

/// The GPU ring buffer. Allocated lazily by the first recording Write. Stored at
/// whatever (Write-scaled, now full-res) dimensions it is given.
struct Buffer {
    texture_array: GLuint,
    width: u32,
    height: u32,
}

/// One channel's tape. `record_index` (the write cursor), the frame-barrier de-dup
/// state, and the refcount live here so they survive reallocation and exist before
/// any buffer does (a Read may acquire before any Write has allocated).
struct Tape {
    buffer: Option<Buffer>,
    refcount: u32,
    /// Write cursor: advances exactly once per host frame **while recording**
    /// (see [`vc_write_tick`]). Write slot = `record_index mod BUFFER_DEPTH`.
    /// Frozen (Write not ticking) ⇒ static ⇒ the loop window parks. Wraps at u64
    /// (≈10^10 yr @60 fps — never in practice).
    record_index: u64,
    /// Host frame id of the frame last recorded (barrier de-dup key).
    frame_id: u64,
    frame_active: bool,
    /// Confine-mode handoff: the ring slot the Read's play head sits on, which the
    /// Write records back into (in-place feedback within the loop window). Published
    /// by the Read each frame (`vc_set_loop_slot`); the Write reads it with the same
    /// `frame_id` (`vc_loop_slot`). `-1` / stale frame_id ⇒ free-ring mode.
    loop_slot: i64,
    loop_slot_frame: u64,
    /// Confined window length last published, and the Read instance that published
    /// it. The length is the region the Write records into, so a *change* leaves
    /// stale content in the slots the new window no longer covers — the varispeed
    /// analogue of delay-core's incoherent-tape reseed (VS-CONFINED-RESEED).
    ///
    /// Scoped to Confined on purpose. Loop Length is a per-Read param and N Free
    /// Reads each own one (VS-MULTITAP); reseeding the shared tape because one tap
    /// retimed would wipe the other taps. Only Confined's length defines what the
    /// Write records, and only one Confined Read may share a channel, so only it
    /// has the authority to reseed.
    loop_len: u32,
    /// Owner of `loop_len`. Two Confined Reads on one channel already fight over
    /// `loop_slot`; without this they would also ping-pong `loop_len` and reseed
    /// the tape to black every frame.
    loop_owner: u64,
}

const EMPTY: Tape = Tape {
    buffer: None,
    refcount: 0,
    record_index: 0,
    frame_id: 0,
    frame_active: false,
    loop_slot: -1,
    loop_slot_frame: 0,
    loop_len: 0,
    loop_owner: 0,
};

static REGISTRY: Mutex<[Tape; NUM_CHANNELS]> = Mutex::new([EMPTY, EMPTY]);

/// Validate a channel index from across the C ABI. `None` ⇒ the caller no-ops.
fn chan(channel: u32) -> Option<usize> {
    let i = channel as usize;
    (i < NUM_CHANNELS).then_some(i)
}

/// Load this DLL's own GL function pointers (per-DLL; idempotent).
fn ensure_gl() {
    static GL_INIT: Once = Once::new();
    GL_INIT.call_once(|| {
        gl_loader::init_gl();
        gl::load_with(|s| gl_loader::get_proc_address(s).cast());
    });
}

/// Max ring depth, so the Read/Write can clamp their loop-length UI to the buffer.
#[no_mangle]
pub extern "C" fn vc_depth() -> u32 {
    BUFFER_DEPTH
}

/// Number of independent tapes, so the plugins can size their Channel param.
#[no_mangle]
pub extern "C" fn vc_channels() -> u32 {
    NUM_CHANNELS as u32
}

/// Register a plugin instance's use of the tape. Balanced by [`vc_release`].
#[no_mangle]
pub extern "C" fn vc_acquire(channel: u32) {
    let Some(i) = chan(channel) else { return };
    REGISTRY.lock().unwrap()[i].refcount += 1;
}

/// Unregister. When the last user releases, the buffer is dropped and the barrier
/// reset. Does NO GL (may run off the GL thread) — the texture name leaks until
/// process exit, matching delay-core / the shipped v2 delay.
#[no_mangle]
pub extern "C" fn vc_release(channel: u32) {
    let Some(i) = chan(channel) else { return };
    let mut reg = REGISTRY.lock().unwrap();
    let t = &mut reg[i];
    t.refcount = t.refcount.saturating_sub(1);
    if t.refcount == 0 {
        t.buffer = None; // leaks the GL texture name (see doc comment)
        t.frame_active = false;
        t.record_index = 0;
        t.loop_len = 0;
        t.loop_owner = 0;
    }
}

/// Called by the Write once per host frame **while recording**. Sizes/reallocs the
/// tape on a dimension change, advances `record_index` once per new `frame_id`
/// (barrier de-dup — guards against a double draw in one host frame), and returns
/// the new `record_index`. The Write records into `record_index mod vc_depth()`.
///
/// NOT called while frozen (Write Send=0), so the cursor parks and the Read's loop
/// window stays put. `width`/`height` are the Write-scaled (full-res) tape dims.
#[no_mangle]
pub extern "C" fn vc_write_tick(channel: u32, width: u32, height: u32, frame_id: u64) -> u64 {
    let Some(i) = chan(channel) else { return 0 };
    ensure_gl();
    let mut reg = REGISTRY.lock().unwrap();
    let t = &mut reg[i];

    if width != 0 && height != 0 {
        let needs_alloc = match &t.buffer {
            Some(b) => b.width != width || b.height != height,
            None => true,
        };
        if needs_alloc {
            if let Some(old) = t.buffer.take() {
                unsafe {
                    if old.texture_array != 0 {
                        gl::DeleteTextures(1, &old.texture_array);
                    }
                }
            }
            let tex = alloc_buffer(width, height);
            // RGBA16F = 8 bytes/texel; width/height are the full-res tape dims.
            let vram_mb = (width as u64 * height as u64 * 8 * BUFFER_DEPTH as u64) / (1024 * 1024);
            tracing::info!(channel, width, height, depth = BUFFER_DEPTH, vram_mb, "varispeed tape allocated");
            t.buffer = Some(Buffer { texture_array: tex, width, height });
        }
    }

    let first_of_frame = !t.frame_active || t.frame_id != frame_id;
    if first_of_frame {
        t.record_index = t.record_index.wrapping_add(1);
        t.frame_id = frame_id;
        t.frame_active = true;
    }
    t.record_index
}

/// Current write cursor. The Read anchors its loop window off this (no advance).
#[no_mangle]
pub extern "C" fn vc_record_index(channel: u32) -> u64 {
    let Some(i) = chan(channel) else { return 0 };
    REGISTRY.lock().unwrap()[i].record_index
}

/// Confine mode: the Read publishes the ring slot its play head sits on, tagged
/// with its channel and
/// the current `frame_id`. The Write reads it back with [`vc_loop_slot`] and
/// records there, closing the feedback loop in place within the window.
///
/// One slot per channel, so a channel supports at most ONE Confined Read; several
/// would overwrite each other's handoff. Multi-tap is Free-only (VS-MULTITAP).
///
/// `len` is the window the Read is looping over and `owner` identifies the Read.
/// When the owning Read changes its window, the tape is reseeded to black
/// (VS-CONFINED-RESEED): the slots the old, longer window covered still hold
/// content the new window never rewrites, which is what resurrects old footage.
/// Mirrors delay-core's loop-length reseed, but scoped to Confined — see
/// [`Tape::loop_len`].
///
/// **GL thread only.** The reseed issues GL calls, so this must be called from a
/// plugin's render, as varispeed-read does. Unlike [`vc_release`], it is not safe
/// to call from teardown.
#[no_mangle]
pub extern "C" fn vc_set_loop_slot(channel: u32, owner: u64, slot: u32, len: u32, frame_id: u64) {
    let Some(i) = chan(channel) else { return };
    let mut reg = REGISTRY.lock().unwrap();
    let t = &mut reg[i];
    t.loop_slot = slot as i64;
    t.loop_slot_frame = frame_id;

    // Only the established owner may reseed. A second Confined Read taking over the
    // handoff adopts the window silently rather than wiping the tape every frame.
    let owned = t.loop_owner == 0 || t.loop_owner == owner;
    if owned && len != t.loop_len {
        if t.loop_len != 0 {
            if let Some(buf) = t.buffer.as_ref() {
                ensure_gl();
                unsafe { clear_texture_array(buf.texture_array) };
                tracing::info!(channel, loop_len = len, "confined window changed → tape reseeded to black");
            }
        }
        t.loop_len = len;
    }
    t.loop_owner = owner;
}

/// The Confined window length published this frame, or `-1` if none. Frame-scoped,
/// like [`vc_loop_slot`]. The Write needs it to know which slots the Confined loop
/// owns, so its free-ring append can step around them (VS-CONFINED-SWEEP).
#[no_mangle]
pub extern "C" fn vc_loop_len(channel: u32, frame_id: u64) -> i64 {
    let Some(i) = chan(channel) else { return -1 };
    let reg = REGISTRY.lock().unwrap();
    if reg[i].loop_slot_frame == frame_id {
        reg[i].loop_len as i64
    } else {
        -1
    }
}

/// The Confine-mode write slot published by the Read this frame, or `-1` if none
/// (free-ring mode / no Read upstream). Frame-scoped: a stale `frame_id` returns -1.
#[no_mangle]
pub extern "C" fn vc_loop_slot(channel: u32, frame_id: u64) -> i64 {
    let Some(i) = chan(channel) else { return -1 };
    let reg = REGISTRY.lock().unwrap();
    if reg[i].loop_slot_frame == frame_id {
        reg[i].loop_slot
    } else {
        -1
    }
}

/// Texture-array name for the ring buffer, or 0 if unallocated.
#[no_mangle]
pub extern "C" fn vc_tex(channel: u32) -> u32 {
    let Some(i) = chan(channel) else { return 0 };
    REGISTRY.lock().unwrap()[i].buffer.as_ref().map_or(0, |b| b.texture_array)
}

/// Allocate a cleared RGBA16F 2D texture array of `BUFFER_DEPTH` layers (VS-STORAGE),
/// or 0 on GL error. Float removes banding; the Write stores at full resolution, so
/// one channel's full-depth tape is ~965 MiB at 1080p. Mirrors delay-core's alloc.
/// FBOs are NOT created
/// here — the plugins own their FBOs; only the texture name crosses the ABI.
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

        clear_texture_array(tex); // TexImage3D(null) leaves layers undefined

        let err = gl::GetError();
        if err != gl::NO_ERROR {
            tracing::error!(gl_error = err, width, height, depth = BUFFER_DEPTH, "varispeed tape allocation failed");
            gl::DeleteTextures(1, &tex);
            return 0;
        }
        tex
    }
}

/// Clear every layer to transparent black, GPU-side (FBO + `glClear` per layer, no
/// multi-GB CPU upload). Mirrors delay-core; saves/restores the host FBO binding,
/// clear colour, and scissor enable (shared Resolume context).
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
        gl::Disable(gl::SCISSOR_TEST);
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
