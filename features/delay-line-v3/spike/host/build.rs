fn main() {
    // dlopen/dlsym live in libdl on older glibc (harmless no-op on newer).
    println!("cargo:rustc-link-lib=dylib=dl");
}
