# Desktop shell

The desktop shell is the primary deployment shape: the runtime process owns a
native window, `render-wgpu` presents the world to it, and the product's
TypeScript UI is composited over it.

## Shape

- **Window and frame.** `desktop-shell` (winit) opens the window on the main
  thread. `DesktopShell::open()` creates the event loop and the `Gpu` first;
  the runtime builds its `Renderer` on that device and applies publications
  before the window exists. Each frame presents the installed view
  composition to the window surface. The shell acquires the swapchain image
  before taking the scene and presents after releasing it, so a product call
  applying its changes never waits for vsync; only encoding and submitting
  the frame hold the scene. Between frames the shell waits for window events
  rather than in the acquire, waking about 2 ms before the display frees the
  next image, and pumps Chromium on each key, button or wheel event and every
  2 ms, so input reaches the page and the page's request reaches the runtime
  without waiting for a frame. `engine.renderer` reports the window's
  presented frames (time and step), its median acquire, lock, draw and
  present times, when input reached the window, and which step consumed
  recent input.
- **UI.** The window shows the same browser shell page `rusty dev` serves,
  from the runtime's own HTTP host (`http://127.0.0.1:<port>/`). Chromium
  (CEF) renders it off-screen and `render-wgpu`'s `web` module imports each
  repaint as a texture and composites it over the frame in gamma space. An
  idle UI costs nothing per frame.
- **Input.** Window input goes to the page, whose input capture sends the
  same normalized `runtime-input` facts it sends from a browser, with its UI
  arbitration unchanged. Chromium's off-screen mode has no Pointer Lock API,
  so the shell provides it: `requestPointerLock` grabs the native cursor and
  raw mouse motion reaches the page as `pointermove` events with
  `movementX`/`movementY`. Escape and focus loss end the lock. A product's
  `confined` cursor mode asks the shell to confine the visible native cursor
  to the window instead (winit's confining grab), so the page keeps ordinary
  cursor positions and DOM hover. Escape and focus loss end it too. Where the
  platform has no confining grab (macOS) the shell refuses, and the page
  falls back to the browser's drawn cursor under a lock.
  The off-screen page keeps Chromium's focus while the window is in the
  background, so the shell reports window focus to it: focus loss sends the
  page a `blur` and makes `document.hasFocus()` false, and the input capture
  clears its held input on that `blur`, as in a browser, locked or not.
- **Video.** `render-wgpu` plays video clips (WebM, VP9 profile 0, decoded
  in pure Rust by `render-video`) over the whole window, above the UI. The streaming mode
  draws video into its frames and shows those frames above the page
  ([architecture](architecture.md#runtime-rendered-output)).
- **Audio.** The runtime plays audio on the output device, video
  soundtracks included; streamed frames instead stream their sound to the
  watching pages (see [recorded audio](recorded-audio.md#realization)).
- **Live debug.** `rusty-live-debug --origin http://127.0.0.1:<port>`
  reaches the runtime's HTTP host over loopback, as in streaming mode.
  `rusty dev --cef-switch name[=value]` (repeatable, forwarded to
  `rusty-product-host --cef-switch`) passes Chromium switches; with
  `remote-debugging-port=<port>` a CDP client attaches to the page.
- **UI storage.** The page's local storage persists under the runtime's
  persistence root (`desktop-ui/`).
- **Window placement.** The window reopens at its last size, position (where
  the platform reports one; Wayland does not) and maximized state, kept in
  `desktop-window` under the persistence root. A position that no longer falls
  on a connected monitor is dropped. There is no fullscreen mode.
  - The saved position is the client area's. A new window's frame goes where
    it is asked to, so once the client area has landed the shell moves the
    window back by the frame's offset. winit's frame position is not used:
    KWin's Xwayland does not reparent windows, so winit reports the client
    area as the frame and the window would move down by its title bar on
    every run.

## Running

The product manifest's `renderer.output: window` selects the shell. Set it in
the product project, or for one launch on the command line:

```xml
<RustyEngineProductRenderOutput>window</RustyEngineProductRenderOutput>
```

```bash
rusty dev --project <product.csproj> --output window
```

- **Fetching.** The first window run downloads the pinned pair's desktop
  runtime pack into the cache beside the pair (`desktop-pack/`). It is the
  pair's host built with the `desktop` feature, plus Chromium's runtime.
  - `rusty` checks the pack's checksum, and that its ABI is the pair's.
  - `rusty status` shows whether it is installed.
  - A pair published without a desktop pack fails the window run with
    `RUSTY_DESKTOP_NOT_PUBLISHED`; `rusty update` moves to a pair that has one.
- **Publication.** The pair workflow builds the pack from the pair's revision
  (`scripts/build-desktop-runtime-pack-archive.sh`) and publishes
  `rusty-engine-desktop-pack-<version>-linux-x64.tar.xz` with the pair,
  about 166 MB. `pair-release.json` names it under `desktopPack`. The default
  pack stays free of Chromium.
- **Contributors.** `scripts/build-runtime-pack.sh --desktop` builds a pack
  from a checkout; pass it with `--runtime`.
  - Building needs `cmake` and `ninja`.
  - On Windows it runs in Git Bash inside the MSVC developer environment
    (`vcvars64.bat`), with MSVC's directory ahead of Git's own `link` on
    `PATH`, and builds `target/runtime-pack/win-x64`.
  - The CEF build script downloads CEF into `CEF_PATH` (default `target/cef`).
  - Default workspace builds and the verify workflow never enable the
    feature.
- **What `lib/cef` holds.** About 292 MB installed; `rusty-product-host
  --cef-dir <directory>` overrides the location for the browser process
  (Linux subprocesses load `libcef.so` through the dynamic linker).
  - `libcef.so`, stripped;
  - its paks, ICU data and V8 snapshot;
  - ANGLE's `libEGL.so` and `libGLESv2.so`;
  - one locale.

  SwiftShader and the bundled Vulkan loader are left out. The overlay imports
  Chromium's frames as GPU textures, so it needs the system GPU driver and
  Vulkan loader the world renderer already uses. Measured on RADV only.

## Shipping a product

A product release that opens a window ships:
- `bin/rusty-product-host` from the desktop pack, with `lib/cef` beside it
  (the host finds it at `../lib/cef`);
- the Product as `rusty build --pack <release>` writes it: `product.rpak`
  (manifest, UI and content in one file) with `coreclr/` or `native/` loose
  beside it, launched with `--product <release>/product.rpak`;
- the licences in `share/third-party/`:
  - CEF's and Chromium's credits (`cef/CREDITS.html`, BSD and others);
  - the source of the MPL-2.0 crates `welding`, `grafting` and `fidget-mesh`,
    which must stay available to recipients;
  - the rest of the pack's notices.

An installer is chosen when a product asks for one.

## Platforms

- **Linux:** X11 and Wayland, measured. The device is created with the
  DMA-BUF extensions Chromium's frames need. Pointer lock turns at the
  browser's rate on both.
  Wayland locks the pointer. X11 hides the cursor and recentres it rather than
  grabbing it, because during a grab XInput delivers every raw motion twice.
- **Windows:** measured on an RTX 3080 with Windows 11
  ([evidence](evidence/desktop-shell-windows/README.md)). That machine is the
  shared Windows test box: agents playtest on it through crew-services
  (`environment: "windows-desktop"`), and its access, agent and build setup are
  in crew-services `docs/playtest-windows.md`. The device is DX12,
  which Chromium's shared textures require; Chromium's D3D11 frames are copied
  into textures shared with it.
  - `libcef.dll` is delay-loaded, so the pack keeps the Linux layout: the host
    points the DLL search at `lib/cef` before Chromium's first use.
  - The browser is created asynchronously; the page takes keyboard focus once
    it exists.
  - Chromium derives `KeyboardEvent.code` from the key's scan code (0xE0-prefixed
    for extended keys), which the shell passes as the native key code.
  - `--headless` is Unix-only: the supervisor that opens the headless page is.
- **macOS:** not built and not in scope. `welding` lists an IOSurface→Metal
  import, and the shell already runs the window on the main thread, as macOS
  requires.

Chromium runs unsandboxed: it shows the product's own first-party UI, never
arbitrary pages. Packaging (installers, updater, signing) is outside this
page.
