# Desktop shell (#8790)

A product runs in a native window owned by the runtime process. `render-wgpu`
presents the world to the window surface, and the product's TypeScript UI,
the same browser shell page `rusty dev` serves, is rendered off-screen by
Chromium (CEF) and composited over it. See [docs/desktop-shell.md](../../desktop-shell.md).

```bash
RUSTY_RENDER_OUTPUT=window rusty dev --runtime <pack built with --desktop> --project <product>
```

## Overlay decision: CEF off-screen rendering composited by wgpu

All measurements ran in a private headless KDE compositor on RADV:
`scripts/headless-compositor.sh` gives Wayland and, through its own Xwayland,
X11, on a private D-Bus session. Screenshots come from spectacle in that
session. The machine carried a load average of 15–30 from other lanes, so
absolute numbers are pessimistic; every pattern ran under the same load.
Raw numbers are in `measurements/`, the spike sources in `cef-spike/` and
`rust/crates/desktop-shell/examples/overlay_spike.rs`.

| Pattern | Linux X11 | Linux Wayland |
|---|---|---|
| 1. Transparent wry child webview over the wgpu surface | **Fails.** The "transparent" child composites opaque black over the scene, because X11 child windows are not alpha-blended (`screenshots/p1-child-x11.png`). | **Unsupported.** wry's `build_as_child` on a winit window returns `UnsupportedWindowHandle`; wry needs a GTK container there. |
| 2. wgpu offscreen, read back, frame drawn in the webview (WebKitGTK, custom protocol, WebGL upload) | Works but falls short. At 720p: 48 frames/s shown, 29 ms p50 latency. At 1080p: 25 frames/s, 74 ms. The WebKit web process sits at ~100% CPU. Under load: 11 frames/s and 136 ms (720p), 6 frames/s and 278 ms (1080p). | Needs a GTK window; same mechanism. |
| 3. CEF accelerated off-screen rendering, imported into wgpu as a texture | 60 fps scene and 60 UI fps with an animated HUD. 0.33 ms per import, 7–15% CPU for the whole process tree. | 60 fps scene and 50–60 UI fps animated. ~2 ms per import under load. |

Further facts for pattern 3:
- **Idle UI.** A UI repaint happens only when the DOM changes; an idle HUD
  costs about 5 imports per second.
- **Latency.** A DOM change reaches the imported texture in 10–17 ms (p50).
- **Startup.** CEF initializes in 136–315 ms; the first UI frame arrives
  after 250–850 ms.
- **Bundle.** The stripped runtime is 322 MB installed: `libcef.so` 268 MB,
  plus paks, ICU, the V8 snapshot, ANGLE/SwiftShader and one locale. It
  compresses to about 105 MB with xz.
- **Pointer lock.** CEF's off-screen mode rejects the Pointer Lock API
  (`WrongDocumentError`), so the shell provides it (below).

**Why pattern 3.**
- Rust owns the window, the frame and the input.
- The game frame goes straight to the surface with no readback, and the UI
  costs only when it repaints.
- It is one engine (Chromium), the same as the browser development path, and
  it keeps CDP available for tooling.

**Costs.**
- About 105 MB compressed.
- CEF re-executes the host binary for its subprocesses.
- Chromium runs unsandboxed. It shows only the product's first-party UI.
- `welding` 0.14.1 is young (MPL-2.0, one maintainer). The seam is small:
  texture import and input forwarding.
- **Linux:** the device needs the DMA-BUF extensions. Without
  `VULKAN_EXTERNAL_MEMORY_DMA_BUF`, X11 import fails on Chromium's
  implicit-modifier buffers (measured).
- **Windows:** the device needs DX12 for the shared-texture import.

**Tauri** has no runtime role under this pattern. Packaging is out of scope;
whatever packages the application ships `lib/cef` beside the binary.

## What was built

- **`desktop-shell`** (new crate, sole owner of `winit`).
  - `DesktopShell::open()` makes the event loop and a `Gpu` for the display
    first, so the runtime builds its renderer on the window's device before
    the window exists.
  - `run()` opens the window and draws the `DesktopScene` every frame.
  - `overlay.rs`: window input goes to the page, whose unchanged input
    capture sends the same normalized `runtime-input` facts as in a browser,
    with its UI arbitration intact. Key events carry the platform keycode, so
    `KeyboardEvent.code` matches the browser.
  - Pointer lock is a page shim: `requestPointerLock` asks the shell to grab
    the cursor (a console message), and raw mouse motion comes back as
    `pointermove` with `movementX/Y`. Escape and focus loss end the lock, as
    in Chrome. Cursor shapes follow the page.
- **`render-wgpu`, `web` module** (feature `web-overlay`, off by default,
  owner of `welding` and `cef`).
  - Imports the page's repaints and composites them after the scene through a
    non-sRGB view of the swapchain image, in gamma space as a browser does.
  - The page profile lives under the persistence root.
  - Component updates and background networking are disabled.
  - The password store is `basic` (no desktop keyring). With a persistent
    profile, Chromium otherwise asks KWallet or libsecret over D-Bus for a
    cookie key. On this machine the owner's locked session blocked that
    call, and with it every page load (found on X11, where the runs used
    the owner's session bus). A user's desktop would show a keyring
    prompt.
  - Also: `Gpu::for_display`, `WindowSurface::create`, and a DMA-BUF-capable
    device (Linux) or DX12 (Windows) when the feature is on.
- **`render-stream`: driver split.**
  - `SceneDriver` is the committed scene on a caller's `Gpu`: apply,
    rebaseline, simulation, animation and video facts, and `draw`.
  - `FrameStreamer` is now only the stream's render thread over a driver.
- **Runtime.**
  - `RUSTY_RENDER_OUTPUT=window` (next to `stream`) builds the driver on the
    shell's device.
  - `main` opens the shell before loading the product and runs the window on
    the main thread. The supervisor stdin, signal and host-stop wait moves to
    a scoped thread; closing the window stops the host the same way.
  - Window mode defaults to `RUSTY_AUDIO_OUTPUT=device`.
  - The feature is `desktop` (off by default), with an rpath to
    `$ORIGIN/../lib/cef`, and CEF subprocesses exit early in `main`.
  - `RUSTY_CEF_SWITCHES` passes Chromium switches through, for example
    `remote-debugging-port=9333` for CDP.
- **Browser shell.**
  - The bootstrap `renderer.output: "window"` mounts `mountWindowSurface`: no
    frame pull, a transparent canvas and page, and only audio and telemetry
    left to browser hosts.
  - The host treats the window surface as runtime-rendered, as it treats the
    stream (#8841's held time, drawing and observer camera belong to the
    runtime renderer).
- **Pack.** `scripts/build-runtime-pack.sh --desktop` ships the stripped CEF
  runtime in `lib/cef`, with Chromium's credits in `share/third-party/cef`.
  The default pack and CI never enable the feature.

## Product evidence (Linux, RADV)

Doom and Dagger ran as private copies staged through
`rusty dev --engine-source` with a `--desktop` pack from this branch. Both
products needed their mechanical #8840 renames to compile against current
main; that is not a desktop change, and nothing was pushed downstream.

**Doom, Wayland** (`screenshots/doom-wayland.png`).
- The world, sprites and viewmodel draw in the window, with the whole
  TypeScript HUD over them: status bar, debug toggle, hint line.
- 3,358 frames in 57.3 s, none skipped; 3 UI imports for a static HUD.
- Shutdown was clean (`RUSTY_DESKTOP` report, `supervisor-stdin-closed`).

**Doom, X11, real input through XTest** (`scripts/xtest.py`).
- A click on the world takes the shell's pointer lock.
- Mouse motion turns the player: yaw 0° → 14.4°.
- Holding W moves the player: (−7, 3) → (−5.4, −2.6).
- Ctrl fires: bullets 50 → 48. These come from `combat.observe` before and
  after, and from `screenshots/doom-x11-after-input.png`.
- Escape ends the lock. F3 opens the product's debug panel over the world
  (`doom-x11-debug-panel.png`), and a click on one of its buttons opens the
  in-game live-debug panel, connected (`doom-x11-live-debug-panel.png`).

**Input probe** (`scripts/probe.html`, `screenshot shell-x11-after.png`).
- `KeyW KeyA KeyS KeyD Space ControlLeft Digit1` arrive with the browser's
  codes.
- Wheel, button click and canvas click work.
- 10 locked motion events arrive; Escape releases the lock without reaching
  the page.

**Live debug.** `rusty-live-debug --origin http://127.0.0.1:4461 --command
combat.observe` answers over loopback, and `engine.renderer.status` reports
`render-wgpu (desktop window)`.

**Dagger.**
- The title screen (`dagger-title.png`) and its Begin button work through the
  shell.
- The opening cinematics play through the window (#8791's evidence).
- 8,148 frames in 136.1 s, none skipped.

**Re-verified on main `c3962ba3b`** (after #8840's trigger changes and
#8841's streaming inspection):
- Doom on X11 with the owner's session bus: yaw 14.4°, moved, fired
  (50 → 48); 8 UI imports.
- Dagger's cinematic plays over the title screen.
- Stream mode delivered 888 frames in 15 s.

**Other runs.**
- **Resize.** The surface and the page both follow the window.
- **Stream mode after the driver split.** Doom in headless Chromium received
  904 frames in 15 s.

## Checks

- `cargo test` passes for `render-wgpu`, `render-video`, `render-audio`,
  `render-stream`, `csharp-engine-services`, `csharp-product-runtime` and
  `desktop-shell` (the last also with `web-overlay`).
- Workspace clippy `-D warnings` (default features, as CI runs it) and
  clippy with `desktop`/`web-overlay` pass.
- fmt passes.
- `scripts/dependency_boundary_check.py` passes, as do its tests.
- `product-browser-host` tests pass (106).
- Windows cross-check: `cargo check -p desktop-shell --target
  x86_64-pc-windows-gnu` (without CEF) passes.

## Limits

- **Windows and macOS were not run.** CEF for Windows builds with MSVC,
  which is not available here. `welding` records its DX12 (Windows) and
  IOSurface→Metal (macOS) imports working on hardware.
- **Wayland input was not driven.** The headless compositor has no synthetic
  input. Wayland rendering, overlay and presentation were measured; the input
  path is the same winit events, but pointer grab there
  (`CursorGrabMode::Locked`) is unexercised.
- **XTest artefact.** Xwayland delivers each XTest *relative* motion as two
  identical raw events, so locked motion measured double under XTest.
  Absolute motion arrives once. A physical mouse was not available.
- **Window placement.** The window opens at 1280×720; nothing restores its
  size or position yet.

## Review fix: focus loss reaches the page without pointer lock

The review of `bdaa9cedf` found that `Focused(false)` only ended a pointer
lock. Outside a lock the page was never told the window lost focus. Chromium's
off-screen page keeps its own focus, so it got no `blur`, and
`application-host`'s input capture never cleared its held input. The shell
also kept its held-button and modifier flags.

**Fix (`desktop-shell/src/overlay.rs`).**
- The page shim (previously only the pointer-lock shim) now also reports
  window focus. `window.__rustyDesktopFocus(false)` dispatches `blur` on
  `window` and makes `document.hasFocus()` false; `true` dispatches `focus`.
  With `hasFocus()` false, controller sampling also stops while the window is
  in the background, as in a browser tab.
- On focus loss the shell ends any lock (as before), clears its held buttons
  and modifiers, and reports the loss.
- A page that loads while the window is unfocused is told so once the shim is
  installed.

**Probe.** Doom ran in window mode on X11 in the private headless KWin, driven
through XTest, with `RUSTY_CEF_SWITCHES=remote-debugging-port=9333` so the page
could be read over CDP (`scripts/mouse_probe.py`, `scripts/cdp.mjs`):
1. hold the right mouse button on the canvas (it does not request a lock);
2. map an xterm, which takes focus, and release the button over it;
3. refocus the shell through its title bar and move over the canvas.

| Pack | `blur` seen | `hasFocus()` while unfocused | `pointermove.buttons` after refocus |
|---|---|---|---|
| Before (`fbc78ee4c` desktop pack) | no | `true` | `[2, 2, 2]`: the button is still held |
| After (this fix) | yes, then `focus` on refocus | `false` | `[0, 0, 0]` |

`scripts/focus_probe.py` runs the keyboard version. It holds W, moves focus
to the xterm, releases W there, and refocuses. With the fix, the player stops
at focus loss and stays still after refocus, both unlocked (after Escape) and
locked.

The keyboard case also passes before the fix: winit sends synthetic key
releases for held keys when a window loses focus, and the shell forwards
them. It sends no synthetic mouse-button release, which is why the mouse case
failed.

**Checks.**
- `cargo clippy -p desktop-shell --all-targets -D warnings`, with and without
  `web-overlay`, and stable clippy.
- `cargo fmt --check`.
- The runtime pack was built with `--desktop`.
