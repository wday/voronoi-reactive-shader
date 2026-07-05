//! Stage-0 (Windows) spike: the shared ring-buffer registry as a C-ABI cdylib.
//! Identical to the Linux spike's core — the registry is platform-agnostic.
//! What differs on Windows is HOW the plugins locate and bind this DLL (see
//! `pluglib`): runtime LoadLibraryEx from the plugin's own directory, because
//! the Windows loader will not resolve a static import from a plugin's folder.

use std::sync::Mutex;

const N: usize = 2;

#[derive(Clone, Copy)]
struct Entry {
    tex_handle: u64,
    last_value: u64,
    write_pos: u32,
    refcount: u32,
    frame_id: u64,
    frame_active: bool,
}

const EMPTY: Entry = Entry {
    tex_handle: 0,
    last_value: 0,
    write_pos: 0,
    refcount: 0,
    frame_id: 0,
    frame_active: false,
};

static REG: Mutex<[Entry; N]> = Mutex::new([EMPTY; N]);

#[no_mangle]
pub extern "C" fn dc_acquire(ch: usize) {
    if ch >= N {
        return;
    }
    let mut r = REG.lock().unwrap();
    let e = &mut r[ch];
    e.refcount += 1;
    if e.tex_handle == 0 {
        e.tex_handle = 0x1000 + ch as u64;
    }
}

#[no_mangle]
pub extern "C" fn dc_release(ch: usize) -> u32 {
    if ch >= N {
        return 0;
    }
    let mut r = REG.lock().unwrap();
    let e = &mut r[ch];
    e.refcount = e.refcount.saturating_sub(1);
    if e.refcount == 0 {
        *e = EMPTY;
    }
    e.refcount
}

#[no_mangle]
pub extern "C" fn dc_begin_frame(ch: usize, frame_id: u64) -> u32 {
    if ch >= N {
        return 0;
    }
    let mut r = REG.lock().unwrap();
    let e = &mut r[ch];
    if !e.frame_active || e.frame_id != frame_id {
        e.frame_id = frame_id;
        e.frame_active = true;
        e.write_pos = e.write_pos.wrapping_add(1);
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn dc_write_value(ch: usize, value: u64) {
    if ch < N {
        REG.lock().unwrap()[ch].last_value = value;
    }
}

#[no_mangle]
pub extern "C" fn dc_last_value(ch: usize) -> u64 {
    if ch >= N { 0 } else { REG.lock().unwrap()[ch].last_value }
}

#[no_mangle]
pub extern "C" fn dc_tex_handle(ch: usize) -> u64 {
    if ch >= N { 0 } else { REG.lock().unwrap()[ch].tex_handle }
}

#[no_mangle]
pub extern "C" fn dc_write_pos(ch: usize) -> u32 {
    if ch >= N { 0 } else { REG.lock().unwrap()[ch].write_pos }
}

#[no_mangle]
pub extern "C" fn dc_refcount(ch: usize) -> u32 {
    if ch >= N { 0 } else { REG.lock().unwrap()[ch].refcount }
}
