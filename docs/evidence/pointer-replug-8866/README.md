# The desktop window survives a pointer replug on Wayland (#8866)

#8859 found that on Wayland, replacing the seat's only pointer device took the
desktop runtime down. The main thread panicked in winit 0.30.13
(`seat/pointer/mod.rs:409`, "failed to get pointer data"). With hardware this
is a Bluetooth mouse reconnecting, a KVM switch, or unplugging the only mouse.

## Cause

When the pointer capability goes away, winit releases the `wl_pointer`. Pointer
events for that released object can still be dispatched afterwards.
`pointer_frame` looked up winit's data on the pointer with `expect`, and a
released proxy has none.

A diagnostic copy of winit that skipped such events instead logged, per run:

```
PATCH: 1 pointer events for ObjectId(wl_pointer@24), which has no winit data (alive: false)
PATCH: 1 pointer events for ObjectId(wl_pointer@61), which has no winit data (alive: false)
```

The same `expect` is still in 0.31.0-beta.3 (`winit-wayland`). 0.30.13 is the
latest stable release.

## Fix

- winit 0.30.13 is vendored in `rust/vendor/winit`, as `fidget-mesh` is, and
  patched through the workspace's `[patch.crates-io]`.
  - `pointer_frame` ignores events for a pointer without winit data.
  - `RUSTY_PATCH.md` records the source, the change, and when to remove it.
  - The examples, tests and docs are left out of the vendored copy.
- In `Cargo.lock`, `num_enum_derive` (an Android-only dependency of winit)
  now reuses `proc-macro-crate` 1.3.1, which the lock already held.

## Evidence

Doom in window mode on Wayland ran in the #8859 headless KWin, driven through
KWin's fake input. Each `fakeinput` invocation adds a virtual device and
removes it on exit, as a replug would. The sequence: move, click (lock), then
two motions, each from a new device.

| Pack | Panics (trials) |
|---|---|
| Before (`3439ce706`) | 2 of 2 |
| After (vendored winit) | 0 of 3 |

After a replug the page keeps receiving pointer events. Removing the device
that held the lock reaches the window as a focus change, so the lock ends, as
on any focus loss (#8790). A click with the new device locks again and turns
12.0° per 100 counts, as before the replug.

## Checks

- `desktop-shell` clippy with `web-overlay` passes, as does workspace clippy.
- `scripts/dependency_boundary_check.py` passes.
- A `--desktop` runtime pack was built with the vendored winit, and the trials
  above ran on it.
