# Desktop shell

The desktop shell is the primary deployment shape: the runtime process owns a
native window, `render-wgpu` presents the world to it, and the product's
TypeScript UI is composited over it. Decision and measurements:
[evidence #8790](evidence/desktop-shell-8790/README.md).

## Shape

- **Window and frame.** `desktop-shell` (winit) opens the window on the main
  thread. `DesktopShell::open()` creates the event loop and the `Gpu` first;
  the runtime builds its `Renderer` on that device and applies publications
  before the window exists. Each frame presents the installed view
  composition to the window surface.
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
  `movementX`/`movementY`. Escape and focus loss end the lock.
  The off-screen page keeps Chromium's focus while the window is in the
  background, so the shell reports window focus to it: focus loss sends the
  page a `blur` and makes `document.hasFocus()` false, and the input capture
  clears its held input on that `blur`, as in a browser, locked or not.
- **Video.** `render-wgpu` plays video clips (WebM, VP9 profile 0, decoded
  in pure Rust by `render-video`) over the whole window, above the UI, as
  the browser's video element covered the page
  ([evidence #8791](evidence/video-8791/README.md)). The streaming mode
  draws video into its frames and shows those frames above the page
  ([architecture](architecture.md#streaming-browser-mode)).
- **Audio.** The shell's runtime plays audio on the output device
  (`RUSTY_AUDIO_OUTPUT=device`, see [recorded audio](recorded-audio.md)),
  video soundtracks included.
- **Live debug.** Unchanged: `rusty-live-debug --origin http://127.0.0.1:<port>`
  reaches the runtime's HTTP host over loopback. `RUSTY_CEF_SWITCHES`
  passes Chromium switches (comma-separated `name[=value]`); with
  `remote-debugging-port=<port>` a CDP client such as Playwright attaches
  to the page.
- **UI storage.** The page's local storage persists under the runtime's
  persistence root (`desktop-ui/`).
- **Window placement.** The window reopens at its last size, position (where
  the platform reports one; Wayland does not) and maximized state, kept in
  `desktop-window` under the persistence root. A position that no longer falls
  on a connected monitor is dropped. There is no fullscreen yet; no product
  has asked for it.

## Running

`RUSTY_RENDER_OUTPUT=window` selects the shell. A pinned product needs nothing
else:

```bash
RUSTY_RENDER_OUTPUT=window rusty dev --project <product.csproj>
```

- **Fetching.** The first window run downloads the pinned pair's desktop
  runtime pack into the cache beside the pair (`desktop-pack/`). It is the
  pair's host built with the `desktop` feature, plus Chromium's runtime.
  - `rusty` checks the pack's checksum, and that its ABI is the pair's.
  - `rusty status` shows whether it is installed.
  - Pairs published before the desktop pack have none. `rusty update` moves
    to one that has.
- **Publication.** The pair workflow builds the pack from the pair's revision
  (`scripts/build-desktop-runtime-pack-archive.sh`) and publishes
  `rusty-engine-desktop-pack-<version>-linux-x64.tar.xz` with the pair,
  about 166 MB. `pair-release.json` names it under `desktopPack`. The default
  pack stays free of Chromium.
- **Contributors.** `scripts/build-runtime-pack.sh --desktop` builds a pack
  from a checkout; pass it with `--runtime`.
  - Building needs `cmake` and `ninja`.
  - The CEF build script downloads CEF into `CEF_PATH` (default `target/cef`).
  - Default workspace builds and the verify workflow never enable the
    feature.
- **What `lib/cef` holds.** About 292 MB installed; `RUSTY_CEF_DIR` overrides
  the location.
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
- the staged product bundle;
- the licences in `share/third-party/`:
  - CEF's and Chromium's credits (`cef/CREDITS.html`, BSD and others);
  - the source of the MPL-2.0 crates `welding`, `grafting` and `fidget-mesh`,
    which must stay available to recipients;
  - the rest of the pack's notices.

An installer or archive format is chosen when a product asks for one.

## Platforms

- **Linux:** X11 and Wayland, measured. The device is created with the
  DMA-BUF extensions Chromium's frames need. Pointer lock turns at the
  browser's rate on both ([evidence #8859](evidence/desktop-input-8859/README.md)).
  Wayland locks the pointer. X11 hides the cursor and recentres it rather than
  grabbing it, because during a grab XInput delivers every raw motion twice.
- **Windows:** the device prefers DX12, which Chromium's shared textures
  require. Not run here; `welding` records it working on hardware.
- **macOS:** not built.

Chromium runs unsandboxed: it shows the product's own first-party UI, never
arbitrary pages. Packaging (installers, updater, signing) is outside this
page.
