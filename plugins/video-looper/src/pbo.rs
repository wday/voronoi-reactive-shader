// ── pbo.rs ── Async GPU ↔ RAM transfer via Pixel Buffer Objects ──
//
// PBOs let us transfer pixel data between GPU and system RAM without
// stalling the rendering pipeline. Key concepts:
//
// PIXEL_PACK_BUFFER (download: GPU → RAM):
//   When a PBO is bound to PIXEL_PACK_BUFFER, glReadPixels writes into
//   the PBO instead of a CPU pointer. The GPU does this via DMA — the
//   CPU doesn't wait. We map the PBO on the NEXT frame to read the data.
//
// PIXEL_UNPACK_BUFFER (upload: RAM → GPU):
//   When a PBO is bound to PIXEL_UNPACK_BUFFER, glTexSubImage2D reads from
//   the PBO instead of a CPU pointer. We map the PBO to copy our data in,
//   then the texture upload happens via DMA.
//
// DOUBLE-BUFFERING:
//   We use 2 PBOs for each direction, alternating each frame. This ensures
//   the GPU has time to finish the DMA before we try to map the buffer.
//   Frame N: write into PBO[0], read from PBO[1] (filled 2 frames ago)
//   Frame N+1: write into PBO[1], read from PBO[0]
//
// BUFFER ORPHANING:
//   Before uploading, we call glBufferData with NULL. This tells the driver
//   "I don't need the old data, allocate a new buffer." The driver can then
//   start our upload immediately instead of waiting for the previous
//   glTexSubImage2D to finish reading from the old buffer.
//
// Rust-isms:
//   `[GLuint; 2]` = fixed-size array on the stack, like `GLuint pbos[2]` in C.
//   `as_mut_ptr()` = get raw C pointer for passing to GL functions.
//   `std::slice::from_raw_parts(ptr, len)` = make a Rust slice from a raw
//   pointer — like casting `void*` to `uint8_t*` with a known length.

use gl::types::*;
use std::ptr;

pub struct PboTransfer {
    download_pbos: [GLuint; 2], // ping-pong for GPU→RAM
    upload_pbos: [GLuint; 2],   // ping-pong for RAM→GPU
    current_download: usize,    // 0 or 1, flips each frame
    current_upload: usize,
    frame_size: usize,          // width * height * 4 bytes
    read_fbo: GLuint,           // cached FBO for glReadPixels
    initialized: bool,
}

impl PboTransfer {
    pub fn new() -> Self {
        Self {
            download_pbos: [0; 2],
            upload_pbos: [0; 2],
            current_download: 0,
            current_upload: 0,
            frame_size: 0,
            read_fbo: 0,
            initialized: false,
        }
    }

    /// Allocate all PBOs and the read FBO. Called once per resolution.
    pub fn init(&mut self, width: u32, height: u32) {
        self.cleanup();

        self.frame_size = (width * height * 4) as usize;

        unsafe {
            // GenBuffers(count, out_array) — fills array with new buffer IDs
            gl::GenBuffers(2, self.download_pbos.as_mut_ptr());
            gl::GenBuffers(2, self.upload_pbos.as_mut_ptr());

            // Allocate GPU-side memory for each PBO
            // STREAM_READ = "GPU writes, CPU reads, once per frame"
            for &pbo in &self.download_pbos {
                gl::BindBuffer(gl::PIXEL_PACK_BUFFER, pbo);
                gl::BufferData(
                    gl::PIXEL_PACK_BUFFER,
                    self.frame_size as isize,
                    ptr::null(),        // no initial data
                    gl::STREAM_READ,    // usage hint for driver optimization
                );
            }

            // STREAM_DRAW = "CPU writes, GPU reads, once per frame"
            for &pbo in &self.upload_pbos {
                gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, pbo);
                gl::BufferData(
                    gl::PIXEL_UNPACK_BUFFER,
                    self.frame_size as isize,
                    ptr::null(),
                    gl::STREAM_DRAW,
                );
            }

            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);

            // Cached FBO for reading textures — avoids gen/delete per frame
            gl::GenFramebuffers(1, &mut self.read_fbo);
        }

        self.current_download = 0;
        self.current_upload = 0;
        self.initialized = true;
    }

    /// Start async download of a texture into PBO (GPU → PBO).
    /// The data will be available to map on the NEXT frame via finish_download().
    ///
    /// Flow: attach texture to FBO → glReadPixels into PBO (async DMA)
    pub fn begin_download(&mut self, texture: &GLuint, width: u32, height: u32) {
        if !self.initialized {
            return;
        }

        let pbo = self.download_pbos[self.current_download];

        unsafe {
            // Attach the texture we want to read to our FBO
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.read_fbo);
            gl::FramebufferTexture2D(
                gl::READ_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                *texture,           // dereference &GLuint → GLuint (like *ptr in C)
                0,                  // mipmap level 0
            );

            // With a PBO bound, glReadPixels writes to PBO memory (GPU-side)
            // instead of a CPU pointer. The NULL last arg means "offset 0 in PBO".
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, pbo);
            gl::ReadPixels(
                0, 0,
                width as i32, height as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                ptr::null_mut(),    // offset into PBO, not a CPU pointer
            );
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, 0);
        }
    }

    /// Map the PBO from the PREVIOUS frame's download and copy data out.
    /// Returns true if data was copied.
    ///
    /// `&mut [u8]` = mutable slice — pointer + length, like (uint8_t* dest, size_t len).
    /// `copy_from_slice` = memcpy. The PBO is mapped read-only for the copy,
    /// then unmapped so the GPU can reuse it.
    pub fn finish_download(&mut self, dest: &mut [u8]) -> bool {
        if !self.initialized {
            return false;
        }

        // Read from the OTHER PBO (started last frame, should be done by now)
        let prev = 1 - self.current_download;
        let pbo = self.download_pbos[prev];

        let mut copied = false;
        unsafe {
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, pbo);
            // MapBuffer returns a CPU-accessible pointer to the PBO data
            let ptr = gl::MapBuffer(gl::PIXEL_PACK_BUFFER, gl::READ_ONLY);
            if !ptr.is_null() {
                // Wrap the raw pointer in a safe Rust slice for memcpy
                let src = std::slice::from_raw_parts(ptr as *const u8, self.frame_size);
                let len = dest.len().min(src.len());
                dest[..len].copy_from_slice(&src[..len]);
                gl::UnmapBuffer(gl::PIXEL_PACK_BUFFER);
                copied = true;
            }
            gl::BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
        }

        // Flip: next frame we'll write into this PBO, read from the other
        self.current_download = 1 - self.current_download;
        copied
    }

    /// Upload frame data from RAM into a GPU texture via PBO.
    ///
    /// Flow: orphan PBO → map → memcpy data in → unmap → glTexSubImage2D
    /// The orphan+map pattern avoids stalls (see "buffer orphaning" above).
    pub fn upload_to_texture(&mut self, data: &[u8], texture: GLuint, width: u32, height: u32) {
        if !self.initialized {
            return;
        }

        let pbo = self.upload_pbos[self.current_upload];

        unsafe {
            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, pbo);

            // Orphan: tell driver we don't need old data, allocate fresh
            gl::BufferData(
                gl::PIXEL_UNPACK_BUFFER,
                self.frame_size as isize,
                ptr::null(),
                gl::STREAM_DRAW,
            );
            // Map for CPU write access
            let ptr = gl::MapBuffer(gl::PIXEL_UNPACK_BUFFER, gl::WRITE_ONLY);
            if ptr.is_null() {
                gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
                return;
            }
            let dst = std::slice::from_raw_parts_mut(ptr as *mut u8, self.frame_size);
            let len = data.len().min(dst.len());
            dst[..len].copy_from_slice(&data[..len]); // memcpy into PBO
            gl::UnmapBuffer(gl::PIXEL_UNPACK_BUFFER);

            // With PBO bound, glTexSubImage2D reads from PBO (DMA, not CPU)
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0, 0, 0,
                width as i32, height as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                ptr::null(),        // offset into PBO, not CPU pointer
            );

            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }

        self.current_upload = 1 - self.current_upload;
    }

    /// Free all GL resources. Called by Drop (destructor) and before reinit.
    fn cleanup(&mut self) {
        if !self.initialized {
            return;
        }
        unsafe {
            gl::DeleteBuffers(2, self.download_pbos.as_ptr());
            gl::DeleteBuffers(2, self.upload_pbos.as_ptr());
            if self.read_fbo != 0 {
                gl::DeleteFramebuffers(1, &self.read_fbo);
            }
        }
        self.download_pbos = [0; 2];
        self.upload_pbos = [0; 2];
        self.read_fbo = 0;
        self.initialized = false;
    }
}

impl Drop for PboTransfer {
    fn drop(&mut self) {
        self.cleanup();
    }
}
