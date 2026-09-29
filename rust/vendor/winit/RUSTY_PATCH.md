# winit patch

Source: crates.io `winit` 0.30.13, https://github.com/rust-windowing/winit/tree/v0.30.13
License: Apache-2.0; see LICENSE. The examples, tests and docs directories and
their manifest targets are left out.

Engine #8866 changes `src/platform_impl/linux/wayland/seat/pointer/mod.rs`:
`pointer_frame` ignores events for a `wl_pointer` that carries no winit data.

When a seat's only pointer is unplugged and another is plugged in, winit
releases the old `wl_pointer`. Events for it can still be dispatched, and the
released proxy has no data. 0.30.13 then panicked in `winit_data()` ("failed
to get pointer data"), which took the desktop runtime down. On a Bluetooth
mouse reconnect or a KVM switch the replugged pointer keeps working, and the
stale events are dropped.

The same `expect` remains in 0.31.0-beta.3 (`winit-wayland`). Remove this
vendored copy once a winit release carries a fix.
