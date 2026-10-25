# nethost-sys patch

Source: crates.io `nethost-sys` 0.8.0, https://github.com/OpenByteDev/nethost-sys
License: MIT. The crate's own `Cargo.lock` is left out.

Engine #10366 links the Windows desktop host with Servo, whose prebuilt
SpiderMonkey uses the dynamic C runtime (`/MD`). 0.8.0 always links the
static `libnethost.lib`, built with the static one (`/MT`), and MSVC refuses
the mix (LNK2038).

- `Cargo.toml` adds the `dll` feature.
- `build/mod.rs`: with `dll`, Windows links `nethost.lib`, the import library
  of `nethost.dll`, instead of `libnethost.lib`.
- `build/download.rs` keeps `nethost.dll` from the NuGet package beside the
  libraries, for the host's package to ship beside the executable.

Remove this copy once nethost-sys can link nethost dynamically.
