//! Runtime loader for **varispeed-core** (`vc_*`), parallel to the delay-core
//! loader (`loader.rs`). Same proven mechanism — locate this module's directory
//! from a code address inside it, then `LoadLibraryEx` the sibling
//! `varispeed_core.dll` by absolute path so two plugins share ONE copy — but
//! entirely separate from the `dc_*` path (VS-ISOLATION): adding this touches
//! nothing the shipped delay uses.

use std::ffi::c_void;
use std::sync::OnceLock;

/// Bound varispeed-core entry points. Signatures MUST match varispeed-core's `vc_*`.
///
/// Every tape-addressing call takes a leading `channel` (VS-CHANNELS); an
/// out-of-range channel is a no-op returning a zero value.
pub struct VcApi {
    pub depth: extern "C" fn() -> u32,
    pub channels: extern "C" fn() -> u32,
    pub acquire: extern "C" fn(u32),
    pub release: extern "C" fn(u32),
    pub write_tick: extern "C" fn(u32, u32, u32, u64) -> u64,
    pub record_index: extern "C" fn(u32) -> u64,
    pub tex: extern "C" fn(u32) -> u32,
    pub set_loop_slot: extern "C" fn(u32, u32, u64),
    pub loop_slot: extern "C" fn(u32, u64) -> i64,
}
unsafe impl Sync for VcApi {}
unsafe impl Send for VcApi {}

static VC_API: OnceLock<VcApi> = OnceLock::new();

/// A symbol inside this rlib — and therefore inside the plugin DLL it links into.
fn marker() {}

/// Load varispeed-core (once) and return its bound C ABI.
pub fn vc_api() -> &'static VcApi {
    VC_API.get_or_init(load)
}

#[cfg(windows)]
fn load() -> VcApi {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;

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

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    unsafe {
        let addr = marker as *const u16;
        let mut hmod: Hmodule = std::ptr::null_mut();
        let ok = GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, addr, &mut hmod);
        assert!(ok != 0, "GetModuleHandleExW failed (err {})", GetLastError());

        let mut buf = [0u16; 32768];
        let n = GetModuleFileNameW(hmod, buf.as_mut_ptr(), buf.len() as u32);
        assert!(n > 0, "GetModuleFileNameW failed (err {})", GetLastError());
        let path = PathBuf::from(OsString::from_wide(&buf[..n as usize]));
        let dir = path.parent().expect("plugin module has no parent dir");

        let core = dir.join("varispeed_core.dll");
        let core_w = wide(core.as_os_str());
        let hcore = LoadLibraryExW(core_w.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH);
        assert!(
            !hcore.is_null(),
            "LoadLibraryExW failed for {} (err {})",
            core.display(),
            GetLastError()
        );

        macro_rules! proc {
            ($n:literal) => {{
                let p = GetProcAddress(hcore, concat!($n, "\0").as_ptr());
                assert!(!p.is_null(), concat!("GetProcAddress ", $n, " failed"));
                std::mem::transmute(p)
            }};
        }
        VcApi {
            depth: proc!("vc_depth"),
            channels: proc!("vc_channels"),
            acquire: proc!("vc_acquire"),
            release: proc!("vc_release"),
            write_tick: proc!("vc_write_tick"),
            record_index: proc!("vc_record_index"),
            tex: proc!("vc_tex"),
            set_loop_slot: proc!("vc_set_loop_slot"),
            loop_slot: proc!("vc_loop_slot"),
        }
    }
}

#[cfg(unix)]
fn load() -> VcApi {
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};
    use std::path::PathBuf;

    #[repr(C)]
    struct DlInfo {
        dli_fname: *const c_char,
        dli_fbase: *mut c_void,
        dli_sname: *const c_char,
        dli_saddr: *mut c_void,
    }
    extern "C" {
        fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    #[cfg(target_os = "macos")]
    const CORE_FILE: &str = "libvarispeed_core.dylib";
    #[cfg(not(target_os = "macos"))]
    const CORE_FILE: &str = "libvarispeed_core.so";

    unsafe {
        let mut info = DlInfo {
            dli_fname: std::ptr::null(),
            dli_fbase: std::ptr::null_mut(),
            dli_sname: std::ptr::null(),
            dli_saddr: std::ptr::null_mut(),
        };
        let ok = dladdr(marker as *const c_void, &mut info);
        assert!(ok != 0 && !info.dli_fname.is_null(), "dladdr failed for plugin module");
        let path = PathBuf::from(CStr::from_ptr(info.dli_fname).to_string_lossy().into_owned());
        let dir = path.parent().expect("plugin module has no parent dir");

        let core = dir.join(CORE_FILE);
        let core_c = CString::new(core.to_string_lossy().as_bytes()).unwrap();
        let hcore = dlopen(core_c.as_ptr(), RTLD_NOW);
        assert!(!hcore.is_null(), "dlopen failed for {}", core.display());

        macro_rules! proc {
            ($n:literal) => {{
                let name = CString::new($n).unwrap();
                let p = dlsym(hcore, name.as_ptr());
                assert!(!p.is_null(), concat!("dlsym ", $n, " failed"));
                std::mem::transmute(p)
            }};
        }
        VcApi {
            depth: proc!("vc_depth"),
            channels: proc!("vc_channels"),
            acquire: proc!("vc_acquire"),
            release: proc!("vc_release"),
            write_tick: proc!("vc_write_tick"),
            record_index: proc!("vc_record_index"),
            tex: proc!("vc_tex"),
            set_loop_slot: proc!("vc_set_loop_slot"),
            loop_slot: proc!("vc_loop_slot"),
        }
    }
}
