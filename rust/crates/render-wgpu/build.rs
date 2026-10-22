//! NGX's static library, which `dlss_wgpu` links, reads the Windows registry
//! but does not link advapi32: a build with DLSS on Windows links it here.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_DLSS").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
