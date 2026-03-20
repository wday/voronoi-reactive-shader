use gl::types::*;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const BUFFER_DEPTH: u32 = 240;
const NUM_CHANNELS: usize = 2;

/// Threshold for detecting a new frame. Resolume layers render sub-millisecond
/// apart within a frame; frames are 8-16ms apart (120-60fps).
const FRAME_THRESHOLD: Duration = Duration::from_millis(2);

pub struct ChannelBuffer {
    pub texture_array: GLuint,
    pub fbo: GLuint,
    pub write_pos: u32,
    pub width: u32,
    pub height: u32,
    pub refcount: u32,
    pub last_advance_time: Instant,
}

static REGISTRY: Mutex<[Option<ChannelBuffer>; NUM_CHANNELS]> =
    Mutex::new([None, None]);

pub fn buffer_depth() -> u32 {
    BUFFER_DEPTH
}

/// Get buffer info for reading. Returns (texture_array, write_pos, width, height) or None.
pub fn read_channel(channel: usize) -> Option<(GLuint, u32, u32, u32)> {
    let reg = REGISTRY.lock().unwrap();
    reg[channel].as_ref().map(|b| (b.texture_array, b.write_pos, b.width, b.height))
}

/// Begin a frame write for the given channel. Returns (tex, fbo, write_pos, is_first).
///
/// If >2ms since last advance, advances `write_pos` and returns `is_first=true`.
/// Otherwise returns current `write_pos` and `is_first=false` (subsequent Send in same frame).
pub fn begin_frame_write(channel: usize, width: u32, height: u32) -> (GLuint, GLuint, u32, bool) {
    let mut reg = REGISTRY.lock().unwrap();

    // ensure_channel logic inlined so we only lock once
    let needs_alloc = match &reg[channel] {
        Some(b) => b.width != width || b.height != height,
        None => true,
    };

    if needs_alloc {
        if let Some(old) = reg[channel].take() {
            cleanup_gl(&old);
        }

        let (tex, fbo) = alloc_buffer(width, height);
        let vram_mb = (width as u64 * height as u64 * 4 * BUFFER_DEPTH as u64) / (1024 * 1024);
        tracing::info!(channel, width, height, depth = BUFFER_DEPTH, vram_mb, "channel buffer allocated");

        reg[channel] = Some(ChannelBuffer {
            texture_array: tex,
            fbo,
            write_pos: 0,
            width,
            height,
            refcount: 0,
            last_advance_time: Instant::now() - Duration::from_secs(1),
        });
    }

    let b = reg[channel].as_mut().unwrap();
    let now = Instant::now();
    let is_first = now.duration_since(b.last_advance_time) > FRAME_THRESHOLD;

    if is_first {
        b.write_pos = (b.write_pos + 1) % BUFFER_DEPTH;
        b.last_advance_time = now;
    }

    (b.texture_array, b.fbo, b.write_pos, is_first)
}

/// Increment refcount when an instance starts using a channel.
pub fn acquire(channel: usize) {
    let mut reg = REGISTRY.lock().unwrap();
    if let Some(b) = &mut reg[channel] {
        b.refcount += 1;
    }
}

/// Decrement refcount. Clean up if zero.
pub fn release(channel: usize) {
    let mut reg = REGISTRY.lock().unwrap();
    if let Some(b) = &mut reg[channel] {
        b.refcount = b.refcount.saturating_sub(1);
        if b.refcount == 0 {
            cleanup_gl(b);
            reg[channel] = None;
        }
    }
}

fn cleanup_gl(b: &ChannelBuffer) {
    unsafe {
        if b.texture_array != 0 {
            gl::DeleteTextures(1, &b.texture_array);
        }
        if b.fbo != 0 {
            gl::DeleteFramebuffers(1, &b.fbo);
        }
    }
}

fn alloc_buffer(width: u32, height: u32) -> (GLuint, GLuint) {
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

        // Clear all layers to black — avoid VRAM garbage artifacts
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
            return (0, 0);
        }

        let mut fbo: GLuint = 0;
        gl::GenFramebuffers(1, &mut fbo);

        (tex, fbo)
    }
}
