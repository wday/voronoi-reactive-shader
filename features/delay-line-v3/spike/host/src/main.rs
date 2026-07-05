//! Spike host — models Resolume: dlopen two independent plugin DLLs, then check
//! that state written through plugin A is visible through plugin B. If so, the
//! single shared `delay-core` (and its `static` registry) is doing its job.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
}
const RTLD_NOW: c_int = 2;

unsafe fn open(path: &str) -> *mut c_void {
    let c = CString::new(path).unwrap();
    let h = dlopen(c.as_ptr(), RTLD_NOW);
    if h.is_null() {
        let e = dlerror();
        let msg = if e.is_null() {
            "unknown".to_string()
        } else {
            CStr::from_ptr(e).to_string_lossy().into_owned()
        };
        panic!("dlopen {path} failed: {msg}");
    }
    h
}

unsafe fn sym(h: *mut c_void, name: &str) -> *mut c_void {
    let c = CString::new(name).unwrap();
    let s = dlsym(h, c.as_ptr());
    if s.is_null() {
        panic!("dlsym {name} failed");
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: host <plugin-a.so> <plugin-b.so>");
        std::process::exit(2);
    }
    let (a_path, b_path) = (&args[1], &args[2]);
    let mut pass = true;
    macro_rules! check {
        ($label:expr, $cond:expr) => {{
            let c = $cond;
            if !c {
                pass = false;
            }
            println!("[{}] {}", if c { "PASS" } else { "FAIL" }, $label);
        }};
    }

    unsafe {
        let a = open(a_path);
        let b = open(b_path);

        let pa_open: extern "C" fn(usize) = std::mem::transmute(sym(a, "pa_open"));
        let pa_close: extern "C" fn(usize) -> u32 = std::mem::transmute(sym(a, "pa_close"));
        let pa_write: extern "C" fn(usize, u64, u64) -> u32 = std::mem::transmute(sym(a, "pa_write"));
        let pa_tex: extern "C" fn(usize) -> u64 = std::mem::transmute(sym(a, "pa_tex"));

        let pb_open: extern "C" fn(usize) = std::mem::transmute(sym(b, "pb_open"));
        let pb_close: extern "C" fn(usize) -> u32 = std::mem::transmute(sym(b, "pb_close"));
        let pb_read_value: extern "C" fn(usize) -> u64 = std::mem::transmute(sym(b, "pb_read_value"));
        let pb_begin: extern "C" fn(usize, u64) -> u32 = std::mem::transmute(sym(b, "pb_begin"));
        let pb_tex: extern "C" fn(usize) -> u64 = std::mem::transmute(sym(b, "pb_tex"));
        let pb_write_pos: extern "C" fn(usize) -> u32 = std::mem::transmute(sym(b, "pb_write_pos"));
        let pb_refcount: extern "C" fn(usize) -> u32 = std::mem::transmute(sym(b, "pb_refcount"));

        let ch = 0usize;

        pa_open(ch);
        pb_open(ch);
        check!("both plugins acquire same channel -> refcount 2", pb_refcount(ch) == 2);
        let (ta, tb) = (pa_tex(ch), pb_tex(ch));
        check!("both see the SAME buffer handle (shared alloc)", ta != 0 && ta == tb);

        let magic: u64 = 0xDEAD_BEEF_CAFE_F00D;
        let first1 = pa_write(ch, 1, magic);
        check!("A is first writer of frame 1", first1 == 1);
        check!(
            "B READS THE VALUE A WROTE (shared registry across DLLs)",
            pb_read_value(ch) == magic
        );

        let first_b = pb_begin(ch, 1);
        check!("B is a subsequent writer in the same frame (barrier is shared)", first_b == 0);
        let wp = pb_write_pos(ch);

        let first2 = pa_write(ch, 2, 0x1234);
        check!("A is first writer of a NEW frame 2", first2 == 1);
        check!("write_pos advanced exactly once per new frame", pb_write_pos(ch) == wp + 1);

        check!("release drops refcount to 1", pa_close(ch) == 1);
        check!("final release frees the channel (refcount 0)", pb_close(ch) == 0);
        check!("freed channel reports no buffer handle", pb_tex(ch) == 0);
    }

    println!();
    if pass {
        println!("SPIKE RESULT: PASS — cross-DLL shared registry + frame barrier + teardown all work");
        std::process::exit(0);
    } else {
        println!("SPIKE RESULT: FAIL");
        std::process::exit(1);
    }
}
