//! See plugin-a/build.rs — same dynamic-link-against-delay-core setup.
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let target = Path::new(&manifest).parent().unwrap().join("target").join(&profile);
    println!("cargo:rustc-link-search=native={}", target.display());
    println!("cargo:rustc-link-lib=dylib=delay_core");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
}
