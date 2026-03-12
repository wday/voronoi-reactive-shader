//! FFI bindings to spout_bridge — flat C API wrapping SpoutLibrary

use std::ffi::CString;
use std::fmt;

// Opaque handle from spout_bridge.h
type SpoutBridgeHandle = *mut std::ffi::c_void;

extern "C" {
    fn spout_bridge_create(
        name: *const std::ffi::c_char,
        width: std::ffi::c_uint,
        height: std::ffi::c_uint,
    ) -> SpoutBridgeHandle;

    fn spout_bridge_send_image(
        h: SpoutBridgeHandle,
        pixels: *const u8,
        width: std::ffi::c_uint,
        height: std::ffi::c_uint,
        invert: std::ffi::c_int,
    ) -> std::ffi::c_int;

    fn spout_bridge_release(h: SpoutBridgeHandle);
}

pub struct SpoutSender {
    handle: SpoutBridgeHandle,
}

#[derive(Debug)]
pub struct SpoutError(String);

impl fmt::Display for SpoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SpoutSender {
    pub fn new(name: &str, width: u32, height: u32) -> Result<Self, SpoutError> {
        let c_name = CString::new(name).map_err(|e| SpoutError(e.to_string()))?;

        let handle = unsafe { spout_bridge_create(c_name.as_ptr(), width, height) };

        if handle.is_null() {
            return Err(SpoutError(
                "failed to create Spout sender — is SpoutLibrary.dll available?".into(),
            ));
        }

        Ok(SpoutSender { handle })
    }

    pub fn send_image(&self, pixels: &[u8], width: u32, height: u32) {
        unsafe {
            spout_bridge_send_image(
                self.handle,
                pixels.as_ptr(),
                width,
                height,
                1, // invert — ffmpeg rawvideo is top-down, Spout expects bottom-up
            );
        }
    }
}

impl Drop for SpoutSender {
    fn drop(&mut self) {
        unsafe {
            spout_bridge_release(self.handle);
        }
    }
}

// SpoutBridgeHandle is a raw pointer used only from the main thread
// (stdin read loop is single-threaded). Safe to send across threads
// if needed, but we don't.
unsafe impl Send for SpoutSender {}
