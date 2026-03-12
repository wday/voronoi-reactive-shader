use std::env;
use std::path::PathBuf;

fn main() {
    // Expect SPOUT_SDK_DIR to point to the Spout2 SDK directory containing
    // SpoutLibrary.h and SpoutLibrary.lib
    let sdk_dir = env::var("SPOUT_SDK_DIR").unwrap_or_else(|_| {
        // Default: look for Spout2 SDK relative to this crate
        let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
        let default = PathBuf::from(&manifest).join("SpoutSDK");
        if default.exists() {
            return default.to_string_lossy().into_owned();
        }
        panic!(
            "SPOUT_SDK_DIR not set and SpoutSDK/ not found next to Cargo.toml.\n\
             Download the Spout2 SDK from https://github.com/leadedge/Spout2/releases\n\
             and either:\n\
             - Extract to tools/spout-publish/SpoutSDK/\n\
             - Or set SPOUT_SDK_DIR=<path to SpoutLibrary dir>"
        );
    });

    let sdk_path = PathBuf::from(&sdk_dir);

    // Compile our C++ bridge
    cc::Build::new()
        .cpp(true)
        .file("csrc/spout_bridge.cpp")
        .include("csrc")
        .include(&sdk_path)
        .compile("spout_bridge");

    // Link against SpoutLibrary.lib
    println!("cargo:rustc-link-search=native={}", sdk_path.display());
    println!("cargo:rustc-link-lib=SpoutLibrary");

    // OpenGL for GL_RGBA constant
    println!("cargo:rustc-link-lib=opengl32");
}
