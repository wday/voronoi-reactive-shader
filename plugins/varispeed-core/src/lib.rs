//! varispeed-core — the shared GPU ring buffer + write-cursor barrier for the
//! Varispeed atom, behind a C ABI. **Single tape** (no channels), **full-ring**
//! topology, **full-res RGBA16F** storage (VS-STORAGE).
//!
//! Fully separate from `delay-core`: the shipped delay (`dc_*`) is never touched
//! (VS-ISOLATION). Like delay-core, it is one owner across the two plugin DLLs
//! (Varispeed Read + Write), which runtime-load it from their own directory so
//! both bind ONE copy → one `static TAPE` and one lock. GL *objects* (the texture
//! name) are shared via Resolume's shared GL context; only scalars cross the ABI.
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

/// Ring depth (layers of the 2D texture array). Caps the max loop / capture
/// window. 240 (4 s @60 fps) — kept at **twice** `MAX_LOOP_FRAMES` so Reverse
/// (ping-pong) mode can still hold TWO full-length blocks at once (record-forward
/// + frozen-reverse), each up to ~2 s, tiling the ring exactly. Free/Confined get
/// the longer `depth - 1` max for free.
///
/// Halved from 480 (2026-08-29) to pay for the tape going full-res
/// (`WRITE-TAPE-SCALE` = 1.0): at 1080p RGBA16F that is ~3.98 GB for the single
/// tape, which fits on a 12 GB card next to Resolume. The old half-res 480 was
/// ~1.99 GB but cost ~0.35 round-trip gain at high spatial frequencies, which
/// mushed tight fractal feedback into blobs within a few laps.
const BUFFER_DEPTH: u32 = 240;

/// The GPU ring buffer. Allocated lazily by the first recording Write. Stored at
/// whatever (Write-scaled, now full-res) dimensions it is given.
struct Buffer {
    texture_array: GLuint,
    width: u32,
    height: u32,
}

/// The single shared tape. `record_index` (the write cursor), the frame-barrier
/// de-dup state, and the refcount live here so they survive reallocation and exist
/// before any buffer does (a Read may acquire before any Write has allocated).
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
}

const EMPTY: Tape = Tape {
    buffer: None,
    refcount: 0,
    record_index: 0,
    frame_id: 0,
    frame_active: false,
    loop_slot: -1,
    loop_slot_frame: 0,
};

static TAPE: Mutex<Tape> = Mutex::new(EMPTY);

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

/// Register a plugin instance's use of the tape. Balanced by [`vc_release`].
#[no_mangle]
pub extern "C" fn vc_acquire() {
    TAPE.lock().unwrap().refcount += 1;
}

/// Unregister. When the last user releases, the buffer is dropped and the barrier
/// reset. Does NO GL (may run off the GL thread) — the texture name leaks until
/// process exit, matching delay-core / the shipped v2 delay.
#[no_mangle]
pub extern "C" fn vc_release() {
    let mut t = TAPE.lock().unwrap();
    t.refcount = t.refcount.saturating_sub(1);
    if t.refcount == 0 {
        t.buffer = None; // leaks the GL texture name (see doc comment)
        t.frame_active = false;
        t.record_index = 0;
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
pub extern "C" fn vc_write_tick(width: u32, height: u32, frame_id: u64) -> u64 {
    ensure_gl();
    let mut t = TAPE.lock().unwrap();

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
            tracing::info!(width, height, depth = BUFFER_DEPTH, vram_mb, "varispeed tape allocated");
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
pub extern "C" fn vc_record_index() -> u64 {
    TAPE.lock().unwrap().record_index
}

/// Confine mode: the Read publishes the ring slot its play head sits on, tagged
/// with the current `frame_id`. The Write reads it back with [`vc_loop_slot`] and
/// records there, closing the feedback loop in place within the window.
#[no_mangle]
pub extern "C" fn vc_set_loop_slot(slot: u32, frame_id: u64) {
    let mut t = TAPE.lock().unwrap();
    t.loop_slot = slot as i64;
    t.loop_slot_frame = frame_id;
}

/// The Confine-mode write slot published by the Read this frame, or `-1` if none
/// (free-ring mode / no Read upstream). Frame-scoped: a stale `frame_id` returns -1.
#[no_mangle]
pub extern "C" fn vc_loop_slot(frame_id: u64) -> i64 {
    let t = TAPE.lock().unwrap();
    if t.loop_slot_frame == frame_id {
        t.loop_slot
    } else {
        -1
    }
}

/// Texture-array name for the ring buffer, or 0 if unallocated.
#[no_mangle]
pub extern "C" fn vc_tex() -> u32 {
    TAPE.lock().unwrap().buffer.as_ref().map_or(0, |b| b.texture_array)
}

/// Allocate a cleared RGBA16F 2D texture array of `BUFFER_DEPTH` layers (VS-STORAGE),
/// or 0 on GL error. Float removes banding; the Write stores at full resolution, so
/// the full-depth tape is ~3.98 GB at 1080p. Mirrors delay-core's alloc. FBOs are NOT created
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
