fn main() {
    // A desktop host loads libcef from the runtime pack's lib/cef.
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
    {
        println!("cargo:rustc-link-arg-bin=rusty-product-host=-Wl,-rpath,$ORIGIN/../lib/cef");
    }
}
