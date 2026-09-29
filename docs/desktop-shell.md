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
  leaves video to the browser, whose element sits above the page.
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

## Running

`RUSTY_RENDER_OUTPUT=window` selects the shell for a runtime built with the
`desktop` feature:

```bash
RUSTY_RENDER_OUTPUT=window rusty dev --runtime <desktop runtime pack> --project <product.csproj>
```

A desktop runtime pack is built with `scripts/build-runtime-pack.sh --desktop`.
It ships Chromium's runtime in `lib/cef` (about 322 MB installed, 105 MB
compressed); `RUSTY_CEF_DIR` overrides that location. Building it needs
`cmake` and `ninja`, and the CEF build script downloads CEF into `CEF_PATH`
(default `target/cef`). Default workspace builds and CI never enable the
feature.

## Platforms

- **Linux:** X11 and Wayland, measured. The device is created with the
  DMA-BUF extensions Chromium's frames need.
- **Windows:** the device prefers DX12, which Chromium's shared textures
  require. Not run here; `welding` records it working on hardware.
- **macOS:** not built.

Chromium runs unsandboxed: it shows the product's own first-party UI, never
arbitrary pages. Packaging (installers, updater, signing) is outside this
page.
