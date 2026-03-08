// ── ring_buffer.rs ── System RAM frame storage ──
//
// This is the delay line's memory. Conceptually a big array of frames:
//   uint8_t frames[900][width * height * 4];
//
// Only frames[0..loop_len] are "active" — the rest sit unused until
// the user changes loopBeats.
//
// Rust-isms:
//   Vec<Vec<u8>> = array of arrays (heap-allocated, like uint8_t** in C).
//   Each inner Vec<u8> is one frame: [R,G,B,A, R,G,B,A, ...].
//   `&[u8]` = a "slice" — pointer + length, like (uint8_t* data, size_t len)
//   but checked for bounds. `&self.frames[i]` gives you a slice automatically.

const MAX_LOOP_SECS: f32 = 30.0;
const ASSUMED_FPS: f32 = 30.0;

pub struct RingBuffer {
    // frames[i] is a Vec<u8> of size width*height*4
    // Total: 900 frames × ~8MB each = ~7.2GB max (but most are black/unused)
    frames: Vec<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    capacity: usize, // always 900 (MAX_LOOP_SECS * ASSUMED_FPS)
}

impl RingBuffer {
    /// Allocate all frame buffers upfront. At 1080p this is ~7.2GB.
    /// `vec![0u8; frame_size]` = calloc(frame_size, 1)
    pub fn new(width: u32, height: u32) -> Self {
        let frame_size = (width * height * 4) as usize;
        let capacity = (MAX_LOOP_SECS * ASSUMED_FPS) as usize; // 900

        let mut frames = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            frames.push(vec![0u8; frame_size]);
        }

        Self {
            frames,
            width,
            height,
            capacity,
        }
    }

    pub fn matches_resolution(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }

    /// Read a frame. Returns `&[u8]` — a read-only slice (pointer + length).
    /// In C: `const uint8_t* get_frame(RingBuffer* b, size_t i) { return b->frames[i % cap]; }`
    pub fn get_frame(&self, index: usize) -> &[u8] {
        &self.frames[index % self.capacity]
    }

    /// Write a frame. `data` is a slice — Rust checks the length at runtime.
    /// `copy_from_slice` is memcpy (panics if lengths differ, which .min() prevents).
    pub fn write_frame(&mut self, index: usize, data: &[u8]) {
        let idx = index % self.capacity;
        let frame = &mut self.frames[idx];
        let len = data.len().min(frame.len());
        frame[..len].copy_from_slice(&data[..len]);
    }

    /// Apply box-blur degradation to the active portion of the buffer.
    /// Called once per loop cycle (e.g. every 2 seconds at 120 BPM / 4 beats).
    /// This is the only CPU-intensive operation in the plugin.
    pub fn degrade(&mut self, quality: f32, loop_len: usize) {
        if quality >= 1.0 {
            return;
        }

        let strength = 1.0 - quality; // 0 = no blur, 1 = full blur
        let w = self.width as usize;
        let h = self.height as usize;
        let n = loop_len.min(self.capacity);

        for i in 0..n {
            // `Self::blur_frame` is a static method call — like blur_frame(&frames[i])
            // We pass `&mut self.frames[i]` because blur_frame modifies the frame in-place.
            Self::blur_frame(&mut self.frames[i], w, h, strength);
        }
    }

    /// 3×3 box blur blended with original.
    /// strength=0: no change, strength=1: fully blurred.
    ///
    /// We copy the frame first (`.to_vec()` = malloc + memcpy) to avoid
    /// reading pixels we've already modified. This is the classic
    /// "need two buffers for convolution" problem.
    ///
    /// `1..h.saturating_sub(1)` = range [1, h-2] — skips border pixels.
    /// `saturating_sub` = subtraction that clamps to 0 instead of underflowing
    /// (usize is unsigned, so 0-1 would panic in debug or wrap in release).
    fn blur_frame(frame: &mut [u8], w: usize, h: usize, strength: f32) {
        let original = frame.to_vec(); // snapshot before modification

        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                for c in 0..3 {
                    // RGB only (skip alpha at offset 3)
                    let mut sum: u32 = 0;
                    for dy in 0..3u32 {
                        for dx in 0..3u32 {
                            let nx = x - 1 + dx as usize;
                            let ny = y - 1 + dy as usize;
                            sum += original[(ny * w + nx) * 4 + c] as u32;
                        }
                    }
                    let avg = (sum / 9) as u8;
                    let orig = original[(y * w + x) * 4 + c];
                    // lerp(orig, avg, strength)
                    let blended = (orig as f32 * (1.0 - strength) + avg as f32 * strength) as u8;
                    frame[(y * w + x) * 4 + c] = blended;
                }
            }
        }
    }
}
