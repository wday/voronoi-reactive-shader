use std::env;
use std::path::PathBuf;

fn main() {
    // Expect SPOUT_SDK_DIR to point to the Spout2 SDK directory containing
    // SpoutLibrary.h and SpoutLibrary.lib
    let sdk_dir = env::var("SPOUT_SDK_DIR").unwrap_or_else(|_| {
        // Default: look for Spout2 SDK relative to this crate
        let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
        let manifest = PathBuf::from(&manifest);
        // Check local SpoutSDK/ next to Cargo.toml
        let local = manifest.join("SpoutSDK");
        if local.exists() {
            return local.to_string_lossy().into_owned();
        }
        // Check vendored Spout2 submodule
        let vendored = manifest
            .join("../../vendor/spout2/SPOUTSDK/SpoutLibrary");
        if vendored.exists() {
            return vendored.to_string_lossy().into_owned();
        }
        panic!(
            "SPOUT_SDK_DIR not set and no Spout2 SDK found.\n\
             Looked in:\n\
             - tools/spout-publish/SpoutSDK/\n\
             - vendor/spout2/SPOUTSDK/SpoutLibrary/\n\
             Set SPOUT_SDK_DIR or run: git submodule update --init"
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
