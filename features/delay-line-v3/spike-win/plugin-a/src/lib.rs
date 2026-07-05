//! Spike plugin A ("Delay Write" stand-in) — binds delay-core at runtime via
//! pluglib and writes into the shared registry.

use pluglib::api;

#[no_mangle]
pub extern "C" fn pa_open(ch: usize) {
    (api().acquire)(ch)
}

#[no_mangle]
pub extern "C" fn pa_close(ch: usize) -> u32 {
    (api().release)(ch)
}

#[no_mangle]
pub extern "C" fn pa_write(ch: usize, frame_id: u64, value: u64) -> u32 {
    let first = (api().begin_frame)(ch, frame_id);
    (api().write_value)(ch, value);
    first
}

#[no_mangle]
pub extern "C" fn pa_tex(ch: usize) -> u64 {
    (api().tex_handle)(ch)
}
