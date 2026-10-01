fn main() {
    // A desktop host loads libcef from the runtime pack's lib/cef.
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_none() {
        return;
    }
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("linux") => {
            println!("cargo:rustc-link-arg-bin=rusty-product-host=-Wl,-rpath,$ORIGIN/../lib/cef");
        }
        // Windows has no rpath: libcef.dll loads on first use, after welding
        // has pointed the DLL search at lib/cef (SetDllDirectoryW).
        Ok("windows") => {
            println!("cargo:rustc-link-arg-bin=rusty-product-host=/DELAYLOAD:libcef.dll");
            println!("cargo:rustc-link-arg-bin=rusty-product-host=delayimp.lib");
        }
        _ => {}
    }
}
