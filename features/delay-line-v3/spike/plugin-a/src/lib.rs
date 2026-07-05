//! Spike plugin A — the "Delay Write" stand-in. Writes into the shared registry
//! through delay-core's C ABI (declared extern, resolved at load time).

extern "C" {
    fn dc_acquire(ch: usize);
    fn dc_release(ch: usize) -> u32;
    fn dc_begin_frame(ch: usize, frame_id: u64) -> u32;
    fn dc_write_value(ch: usize, value: u64);
    fn dc_tex_handle(ch: usize) -> u64;
}

#[no_mangle]
pub extern "C" fn pa_open(ch: usize) {
    unsafe { dc_acquire(ch) }
}

#[no_mangle]
pub extern "C" fn pa_close(ch: usize) -> u32 {
    unsafe { dc_release(ch) }
}

/// Write `value` to the channel for `frame_id`; returns 1 if first writer.
#[no_mangle]
pub extern "C" fn pa_write(ch: usize, frame_id: u64, value: u64) -> u32 {
    unsafe {
        let first = dc_begin_frame(ch, frame_id);
        dc_write_value(ch, value);
        first
    }
}

#[no_mangle]
pub extern "C" fn pa_tex(ch: usize) -> u64 {
    unsafe { dc_tex_handle(ch) }
}
