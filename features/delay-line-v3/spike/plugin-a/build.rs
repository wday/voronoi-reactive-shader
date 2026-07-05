//! Link this plugin cdylib dynamically against the sibling `delay-core` cdylib,
//! and embed an $ORIGIN rpath so it finds libdelay_core.so next to itself at
//! load time (the Linux stand-in for macOS @loader_path / the Windows DLL-dir
//! problem — the real deployment concern this spike exercises).
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let target = Path::new(&manifest).parent().unwrap().join("target").join(&profile);
    println!("cargo:rustc-link-search=native={}", target.display());
    println!("cargo:rustc-link-lib=dylib=delay_core");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
}
