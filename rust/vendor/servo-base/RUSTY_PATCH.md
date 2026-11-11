# servo-base patch

Source: crates.io `servo-base` 0.7.0, https://github.com/servo/servo/tree/v0.7.0/components/base
License: MPL-2.0. The desktop pack ships this modified source in
`share/third-party/servo-base`.

Engine #10720 changes `threadboost.rs` and `Cargo.toml`. On Windows,
`boost_thread` raises an `Elevated` or `Critical` thread to
`THREAD_PRIORITY_ABOVE_NORMAL`, still gated by `perf_thread_boost_enabled`.
`Default` leaves the thread as it is. `Cargo.toml` adds windows-sys's
`Win32_System_Threading` feature for `SetThreadPriority`.

Servo marks its script thread `Critical`, and the constellation, WebRender's
render backend and scene builder, and WebRender's worker pool `Elevated`. 0.7.0
implements the hook only on Linux (core affinity) and OpenHarmony (QoS). On
Windows it was a no-op. So during a game's setup, with the embedder's .NET
thread pool filling every core, Servo's script thread waited for a core
(2.6 s ready against 4.6 s running, from travel to 15 s after, in Rusty Dagger on
den-win11), and the window's UI page hitched: its longest setup frame gap was
131–212 ms. With the Windows arm, it was about 90 ms, and the game window's
frames were no worse.

Remove this vendored copy once a Servo release implements `boost_thread` on
Windows. The arm is a candidate for an upstream Servo contribution.
