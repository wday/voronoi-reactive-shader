use gl::types::*;
use std::sync::Mutex;

const BUFFER_DEPTH: u32 = 900;
const NUM_CHANNELS: usize = 4;

pub struct ChannelBuffer {
    pub texture_array: GLuint,
    pub fbo: GLuint,
    pub write_pos: u32,
    pub width: u32,
    pub height: u32,
    pub refcount: u32,
}

static REGISTRY: Mutex<[Option<ChannelBuffer>; NUM_CHANNELS]> =
    Mutex::new([None, None, None, None]);

pub fn buffer_depth() -> u32 {
    BUFFER_DEPTH
}

/// Get buffer info for reading. Returns (texture_array, write_pos, width, height) or None.
pub fn read_channel(channel: usize) -> Option<(GLuint, u32, u32, u32)> {
    let reg = REGISTRY.lock().unwrap();
    reg[channel].as_ref().map(|b| (b.texture_array, b.write_pos, b.width, b.height))
}

/// Ensure buffer exists for channel at given resolution. Returns (texture_array, fbo, write_pos).
pub fn ensure_channel(channel: usize, width: u32, height: u32) -> (GLuint, GLuint, u32) {
    let mut reg = REGISTRY.lock().unwrap();

    let needs_alloc = match &reg[channel] {
        Some(b) => b.width != width || b.height != height,
        None => true,
    };

    if needs_alloc {
        // Clean up old resources
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
        });
    }

    let b = reg[channel].as_ref().unwrap();
    (b.texture_array, b.fbo, b.write_pos)
}

/// Advance write pointer for channel.
pub fn advance_write_pos(channel: usize) {
    let mut reg = REGISTRY.lock().unwrap();
    if let Some(b) = &mut reg[channel] {
        b.write_pos = (b.write_pos + 1) % BUFFER_DEPTH;
    }
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
        gl::BindTexture(gl::TEXTURE_2D_ARRAY, 0);

        let mut fbo: GLuint = 0;
        gl::GenFramebuffers(1, &mut fbo);

        (tex, fbo)
    }
}
