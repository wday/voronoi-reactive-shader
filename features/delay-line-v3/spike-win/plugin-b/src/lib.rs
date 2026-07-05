//! Spike plugin B ("Delay Tap" stand-in) — binds delay-core at runtime via
//! pluglib and reads the shared registry. If B sees A's writes, one delay_core
//! is shared across the two independently-loaded plugin DLLs.

use pluglib::api;

#[no_mangle]
pub extern "C" fn pb_open(ch: usize) {
    (api().acquire)(ch)
}

#[no_mangle]
pub extern "C" fn pb_close(ch: usize) -> u32 {
    (api().release)(ch)
}

#[no_mangle]
pub extern "C" fn pb_read_value(ch: usize) -> u64 {
    (api().last_value)(ch)
}

#[no_mangle]
pub extern "C" fn pb_begin(ch: usize, frame_id: u64) -> u32 {
    (api().begin_frame)(ch, frame_id)
}

#[no_mangle]
pub extern "C" fn pb_tex(ch: usize) -> u64 {
    (api().tex_handle)(ch)
}

#[no_mangle]
pub extern "C" fn pb_write_pos(ch: usize) -> u32 {
    (api().write_pos)(ch)
}

#[no_mangle]
pub extern "C" fn pb_refcount(ch: usize) -> u32 {
    (api().refcount)(ch)
}
