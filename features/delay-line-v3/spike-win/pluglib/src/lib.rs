//! Windows plugin-side loader for delay-core.
//!
//! THE Windows-specific problem this spike exists to solve: the Windows loader
//! does not search a DLL's own directory when resolving that DLL's *static*
//! imports. So a plugin that statically imports delay_core.dll would fail to
//! load from Resolume's plugin folder. The fix, verified here:
//!
//!   1. Find THIS plugin module's HMODULE from a code address inside it
//!      (GetModuleHandleExW with FROM_ADDRESS — the address is inside pluglib,
//!      which is statically linked into the plugin DLL).
//!   2. Resolve the plugin's own path, take its directory.
//!   3. LoadLibraryExW("<that dir>\\delay_core.dll", LOAD_WITH_ALTERED_SEARCH_PATH)
//!      — an absolute path, so it loads the sibling regardless of search order.
//!   4. GetProcAddress the C ABI into a cached function table.
//!
//! Two plugins doing this against the same absolute path share ONE loaded
//! delay_core.dll (the Windows loader refcounts by canonical path), hence one
//! `static` registry — exactly what the split needs.

use std::ffi::{c_void, OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::OnceLock;

type Hmodule = *mut c_void;

extern "system" {
    fn GetModuleHandleExW(flags: u32, module_name: *const u16, phmodule: *mut Hmodule) -> i32;
    fn GetModuleFileNameW(hmodule: Hmodule, filename: *mut u16, size: u32) -> u32;
    fn LoadLibraryExW(name: *const u16, file: Hmodule, flags: u32) -> Hmodule;
    fn GetProcAddress(hmodule: Hmodule, name: *const u8) -> *mut c_void;
    fn GetLastError() -> u32;
}

const FROM_ADDRESS: u32 = 0x0000_0004;
const UNCHANGED_REFCOUNT: u32 = 0x0000_0002;
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

pub struct Api {
    pub acquire: extern "C" fn(usize),
    pub release: extern "C" fn(usize) -> u32,
    pub begin_frame: extern "C" fn(usize, u64) -> u32,
    pub write_value: extern "C" fn(usize, u64),
    pub last_value: extern "C" fn(usize) -> u64,
    pub tex_handle: extern "C" fn(usize) -> u64,
    pub write_pos: extern "C" fn(usize) -> u32,
    pub refcount: extern "C" fn(usize) -> u32,
}
unsafe impl Sync for Api {}
unsafe impl Send for Api {}

static API: OnceLock<Api> = OnceLock::new();

// Address inside this module (and therefore inside the plugin DLL).
fn marker() {}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn load() -> Api {
    unsafe {
        // 1. This plugin's HMODULE from a code address inside it.
        let mp: fn() = marker;
        let addr = mp as usize as *const u16;
        let mut hmod: Hmodule = std::ptr::null_mut();
        let ok = GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, addr, &mut hmod);
        assert!(ok != 0, "GetModuleHandleExW failed (err {})", GetLastError());

        // 2. The plugin's own path -> its directory.
        let mut buf = [0u16; 32768];
        let n = GetModuleFileNameW(hmod, buf.as_mut_ptr(), buf.len() as u32);
        assert!(n > 0, "GetModuleFileNameW failed (err {})", GetLastError());
        let path = PathBuf::from(OsString::from_wide(&buf[..n as usize]));
        let dir = path.parent().expect("plugin module has no parent dir");

        // 3. Load the sibling delay_core.dll by absolute path.
        let core = dir.join("delay_core.dll");
        let core_w = wide(core.as_os_str());
        let hcore = LoadLibraryExW(core_w.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH);
        assert!(
            !hcore.is_null(),
            "LoadLibraryExW failed for {} (err {})",
            core.display(),
            GetLastError()
        );

        // 4. Bind the C ABI.
        macro_rules! proc {
            ($n:literal) => {{
                let p = GetProcAddress(hcore, concat!($n, "\0").as_ptr());
                assert!(!p.is_null(), concat!("GetProcAddress ", $n, " failed"));
                std::mem::transmute(p)
            }};
        }
        Api {
            acquire: proc!("dc_acquire"),
            release: proc!("dc_release"),
            begin_frame: proc!("dc_begin_frame"),
            write_value: proc!("dc_write_value"),
            last_value: proc!("dc_last_value"),
            tex_handle: proc!("dc_tex_handle"),
            write_pos: proc!("dc_write_pos"),
            refcount: proc!("dc_refcount"),
        }
    }
}

pub fn api() -> &'static Api {
    API.get_or_init(load)
}
