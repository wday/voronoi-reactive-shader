//! Spike plugin B — the "Delay Tap" stand-in. Reads the shared registry through
//! delay-core. If B sees values A wrote, the two DLLs share one registry.

extern "C" {
    fn dc_acquire(ch: usize);
    fn dc_release(ch: usize) -> u32;
    fn dc_begin_frame(ch: usize, frame_id: u64) -> u32;
    fn dc_last_value(ch: usize) -> u64;
    fn dc_tex_handle(ch: usize) -> u64;
    fn dc_write_pos(ch: usize) -> u32;
    fn dc_refcount(ch: usize) -> u32;
}

#[no_mangle]
pub extern "C" fn pb_open(ch: usize) {
    unsafe { dc_acquire(ch) }
}

#[no_mangle]
pub extern "C" fn pb_close(ch: usize) -> u32 {
    unsafe { dc_release(ch) }
}

#[no_mangle]
pub extern "C" fn pb_read_value(ch: usize) -> u64 {
    unsafe { dc_last_value(ch) }
}

#[no_mangle]
pub extern "C" fn pb_begin(ch: usize, frame_id: u64) -> u32 {
    unsafe { dc_begin_frame(ch, frame_id) }
}

#[no_mangle]
pub extern "C" fn pb_tex(ch: usize) -> u64 {
    unsafe { dc_tex_handle(ch) }
}

#[no_mangle]
pub extern "C" fn pb_write_pos(ch: usize) -> u32 {
    unsafe { dc_write_pos(ch) }
}

#[no_mangle]
pub extern "C" fn pb_refcount(ch: usize) -> u32 {
    unsafe { dc_refcount(ch) }
}
