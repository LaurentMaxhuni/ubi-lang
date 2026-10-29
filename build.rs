fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Reach the bounded parser's depth diagnostic on Windows' main thread.
        // Keep UI initialization on that thread; reserve virtual stack only.
        println!("cargo:rustc-link-arg-bin=ubi=/STACK:8388608");
    }
}
