fn main() {
    // On macOS (Intel/homebrew), the `xz2` crate (a transitive dependency via
    // codex's compression stack) sometimes fails to link against the system
    // `liblzma` because the homebrew `/usr/local/lib` path is not in the
    // default linker search path on newer toolchains. Help the linker find it.
    // This is a no-op on platforms where the path does not exist.
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=native=/usr/local/lib");
    }
}
