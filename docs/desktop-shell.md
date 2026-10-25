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
  the frame hold the scene. The acquire waits on a thread of its own, asked
  about 2 ms before the display should free the next image; the compositor
  can hold images for several frames, and the event loop keeps taking window
  events meanwhile. The shell pumps Chromium on each key, button or wheel
  event and every 2 ms, so input reaches the page and the page's request
  reaches the runtime without waiting for a frame. `engine.renderer` reports the window's
  presented frames (time and step), its median acquire, lock, draw and
  present times, when input reached the window, and which step consumed
  recent input.
- **UI.** The window shows the same browser shell page `rusty dev` serves,
  from the runtime's own HTTP host (`http://127.0.0.1:<port>/`). A web
  engine renders it off-screen and `render-wgpu`'s `web` module composites
  it over the frame in gamma space: Chromium (CEF) by default, or Servo
  ([UI engine](#ui-engine)). Each Chromium repaint is imported as a new
  texture. An idle UI costs nothing per frame. Off-screen Chromium paints popup widgets
  (a `<select>`'s list, date and colour pickers) as a separate surface. The
  overlay draws it over the page at its rect while it shows. A popup that
  would reach past the window is moved inside it, and pointer events over it
  are moved back to where Chromium placed it. Chromium routes pointer, wheel
  and key events to the open popup itself.
- **Input.** Window input goes to the page, whose input capture sends the
  same normalized `runtime-input` facts it sends from a browser, with its UI
  arbitration unchanged. Chromium's off-screen mode has no Pointer Lock API,
  so the shell provides it: `requestPointerLock` grabs the native cursor and
  raw mouse motion reaches the page as `pointermove` events with
  `movementX`/`movementY`. Escape and focus loss end the lock.
- **Gameplay input while the lock is held.** The page asks for the lock only
  in gameplay, so while the shell holds it the shell also hands keys, mouse
  buttons, wheel, motion and the gamepad straight to the runtime's input
  mailbox, as the same facts the page would make. The gamepad is the first
  connected pad, read through gilrs on a thread of its own in Chromium's
  standard mapping (buttons 0 to 15, sticks down positive, triggers pressed
  past 30/255), with the values unfiltered as the page's are. A UI page running long tasks (layout, GC)
  then cannot delay movement and look. Taking and giving back input are
  control fences: the runtime publishes a fresh binding marked
  `windowInput`, the page's input under it does not count, and its stale
  batches drop. Where gilrs cannot read gamepads the window says so when it
  takes input, and the page's controller facts still count. Taking presses
  the keys and pad buttons already held; giving back clears for the
  reason the lock ended (`pointer-lock-loss` or `focus-loss`). The page
  still receives every event for its UI, so a UI hotkey (an inventory key,
  chat Enter) also reaches gameplay as a press, then the clear when the UI
  ends the lock: a product keeps such keys out of its gameplay mappings.
  Escape ends the lock and is never a gameplay key. A harness's input claim
  holds input from the window too. Confined and unlocked cursors, interface
  mode and stream output keep the page's path, where the page samples the
  gamepad on its own schedule. A product's
  `confined` cursor mode asks the shell to confine the visible native cursor
  to the window instead (winit's confining grab), so the page keeps ordinary
  cursor positions and DOM hover. Escape and focus loss end it too. Where the
  platform has no confining grab (macOS) the shell refuses, and the page
  falls back to the browser's drawn cursor under a lock.
- **Browser features.** The page runs in off-screen Chromium, which shows no
  native windows of its own: no dialogs, pickers, print previews or new
  windows. What each feature does there is
  [below](#browser-features-in-the-window).
- **Focus.** The off-screen page keeps Chromium's focus while the window is
  in the background, so the shell reports window focus to it: focus loss sends the
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

## Browser features in the window

The window runs the product UI page in off-screen Chromium, so a browser
feature that would open a native window does not. None of the features below
can lock the page's input. Those a page might rely on warn in its console
(`rusty desktop shell: …`). Game UI keeps to the
[green path](csharp-product-style.md#game-ui-and-diagnostic-ui); diagnostic UI
may use any of this and runs fully in a browser.

| Feature | In the window | Under Servo, where it differs |
| --- | --- | --- |
| Native HTML5 drag and drop | Refused at once: `dragend` with `dropEffect` `none`, never `drop`. Warns. | Never starts: Servo fires no `dragstart`; pointer events go on. |
| `print()` | Does nothing; the shell replaces it, since off screen it never returns and freezes the page. Warns. Ctrl+P is an ordinary key. | |
| `alert`, `confirm`, `prompt` | Cancelled at once: `undefined`, `false`, `null`. Warns. | |
| `beforeunload` prompt | Not shown; the page unloads. | |
| `window.open`, `target="_blank"` links | Not opened; `window.open` returns `null`. Warns with the URL. | |
| Same-tab navigation | Replaces the UI page. | |
| File and colour pickers | Do not open. Warns on click. | |
| `<select>`, date picker | Work: the popup is drawn over the page. | The shell draws a `<select>`'s option list over the page, inside the view (click, arrow keys, Enter, Escape); a `multiple` list toggles options with a click or Space and answers with Enter or a click outside. Servo has no date picker: a date input is a plain text field. |
| `<dialog>` with `showModal()` | Works. | |
| `title` tooltips | Not shown. | |
| Image cursors (`cursor: url(...) x y, auto`) | Shown with their hotspot, scaled to the window's display scale. A held pointer lock hides them, as it hides any cursor. | The fallback keyword shows: Servo reports no image cursors. |
| `cursor: none` | Hides the native cursor while the pointer is over that element. | |
| Context menu | `contextmenu` fires; no native menu appears. | |
| Download links | Nothing downloads. | |
| Clipboard | `navigator.clipboard.writeText` works; `readText` is denied. | `readText` resolves with an empty string; the system clipboard is never read. |
| Fullscreen API | Resolves and fills the view; the native window does not change. | |
| Notifications, geolocation and other permission prompts | Denied at once. | |
| Gamepads | Chromium reads them itself. | The shell hands Servo the first pad in the standard mapping, named `Standard gamepad`. |
| Text entry, Tab focus, wheel scrolling | Work. IME composition has not been checked. A held key's repeats report `repeat` false. A wheel notch scrolls 53 pixels on Linux and 44 on Windows. | Held keys report `repeat`. A wheel notch scrolls 53 pixels on both. |

## Running

The product manifest's `renderer.output: window` selects the shell. Set it in
the product project, or for one launch on the command line:

```xml
<RustyEngineProductRenderOutput>window</RustyEngineProductRenderOutput>
```

```bash
rusty dev --project <product.csproj> --output window
```

- **Ending.** Closing the window, or the product's `Host.RequestExit()`
  ([quit](csharp-lifecycle.md#quit)), disposes the product and stops the
  host. A supervised host reports it with exit code 79, so its supervisor and
  `rusty dev` stop rather than restart it as after a crash.
- **Fetching.** The first window run downloads the pinned pair's desktop
  runtime pack into the cache beside the pair (`desktop-pack/`). It is the
  pair's host built with both UI engines (the `desktop` and `servo`
  features), plus Chromium's runtime.
  - `rusty` checks the pack's checksum, and that its ABI is the pair's.
  - `rusty status` shows whether it is installed.
  - A pair published without a desktop pack fails the window run with
    `RUSTY_DESKTOP_NOT_PUBLISHED`; `rusty update` moves to a pair that has one.
- **Publication.** The pair workflow builds the pack from the pair's revision
  (`scripts/build-desktop-runtime-pack-archive.sh`) and publishes
  `rusty-engine-desktop-pack-<version>-linux-x64.tar.xz` with the pair.
  `pair-release.json` names it under `desktopPack`. The default
  pack stays free of either engine.
- **Contributors.** `scripts/build-runtime-pack.sh --desktop` builds a pack
  with both UI engines from a checkout; pass it with `--runtime`.
  - Building needs `cmake`, `ninja` and, for Servo, Python 3 (`python`
    and `python3` on `PATH`; on Windows `PYTHON3` naming it as well).
  - On Windows it runs in Git Bash inside the MSVC developer environment
    (`vcvars64.bat`), with MSVC's directory ahead of Git's own `link` on
    `PATH`, and builds `target/runtime-pack/win-x64`.
  - The CEF build script downloads CEF into `CEF_PATH` (default `target/cef`).
    The pack takes the distribution of the CEF version cef-dll-sys builds
    against and refuses to build when that one is missing.
  - Default workspace builds and the verify workflow never enable either
    engine.
- **What `lib/cef` holds.** The host loads libcef only when Chromium draws
  the UI (on Linux through welding's loader, on Windows by delay-loading).
  `rusty-product-host --cef-dir <directory>` overrides the location for the
  browser process (Chromium's Linux subprocesses take `libcef.so` from
  `lib/cef` or the host's rpath).
  - `libcef.so`, stripped;
  - its paks, ICU data and V8 snapshot (ANGLE is inside `libcef.so`);
  - on Windows `libcef.dll` (ANGLE inside it too), `chrome_elf.dll` and the
    D3D shader compilers instead of `libcef.so`;
  - one locale.

  SwiftShader and the bundled Vulkan loader are left out. The overlay imports
  Chromium's frames as GPU textures, so it needs the system GPU driver and
  Vulkan loader the world renderer already uses.

## UI engine

The manifest's `renderer.windowUi` picks the engine that draws the UI page:
`chromium` (the default) or `servo`. Set it with
`RustyEngineProductWindowUi`, or for one launch with
`rusty dev --window-ui servo`; `rusty-product-host --window-ui` overrides a
release's manifest. `engine.renderer`'s `window.ui` and the host's closing
`RUSTY_DESKTOP` line name the engine (for Servo, its version and the GL
renderer it draws with).

- **Servo** (0.7) runs in the host process, on the window's thread, and
  draws with GL into two shared colour buffers. Servo paints when it has a
  new frame and the window composites the last finished buffer; it never
  paints into the buffer the GPU may still be reading. Servo wakes the
  window's event loop when it has work, so an idle page runs nothing.
  - On Linux the buffers are linear GBM buffers on the render node of the
    GPU the window uses, imported once each as DMA-BUFs.
  - On Windows Servo draws with ANGLE (GLES over D3D11) on the window's GPU,
    matched by LUID, into D3D11 textures with NT handles; each finished frame
    is imported again, which acquires it from the external queue.
- Without a GPU with GL and DMA-BUF import (Linux), or ANGLE on the window's
  adapter (Windows), the window refuses to open and names the reason; it
  never falls back to Chromium.
- Servo 0.7 lacks some CSS Chromium has, among them `:has()`, `::backdrop`
  and `autofocus`, and sizes form controls differently. Check a product's
  UI in the window under the engine it ships with.
- `rusty dev --servo-pref name=value` (repeatable, forwarded to
  `rusty-product-host --servo-pref`) sets a Servo preference.
- The pair's desktop pack carries it; a host built without Servo refuses a
  product that picks it.
- Window input, the pointer-lock and confinement shim, focus and the
  page's console reach Servo through the same shell paths as Chromium. Servo
  takes the shell's page scripts at the start of every document. The
  [browser-features table](#browser-features-in-the-window) names what
  differs.
- Page storage persists under `desktop-ui/servo/` in the persistence root.
- **Inspecting the page.** `--servo-pref devtools_server_enabled=true`
  starts Servo's devtools server (Firefox's remote debugging protocol) on
  127.0.0.1:7000, or `--servo-pref devtools_server_listen_address=<addr>`;
  the window accepts its connections. Servo has no Chrome DevTools Protocol.

## Shipping a product

A product release that opens a window ships:
- `bin/rusty-product-host` from the desktop pack;
- with Chromium (`renderer.windowUi = chromium`), `lib/cef` beside it (the
  host finds it at `../lib/cef`); with Servo, no `lib/cef`;
- on Windows, `nethost.dll` and ANGLE's `libEGL.dll` and `libGLESv2.dll`
  from the pack's `bin/`, beside the host;
- the Product as `rusty build --pack <release>` writes it: `product.rpak`
  (manifest, UI and content in one file) with `coreclr/` or `native/` loose
  beside it, launched with `--product <release>/product.rpak`;
- any content containers the product opens by name, beside `product.rpak`
  (see [containers installed beside the product](csharp-product-project.md#content-containers-installed-beside-the-product));
- the licences in `share/third-party/`:
  - CEF's and Chromium's credits (`cef/CREDITS.html`, BSD and others): the
    host links CEF's wrapper whichever engine draws;
  - the source of the MPL-2.0 crates `welding`, `grafting` and `fidget-mesh`,
    which must stay available to recipients;
  - with Servo, `servo/MPL-2.0-SOURCES.txt`, the unmodified MPL-2.0 crates
    Servo brings and where each one's source is, and on Windows ANGLE's
    licence (`angle/LICENSE`, BSD);
  - `NOTICES.txt`, the licence and notice texts of every third-party crate
    the host links and the native libraries they build in;
  - the rest of the pack's licences (`dejavu-sans/`, with DLSS
    `nvidia-dlss/`).

An installer is chosen when a product asks for one.

## Platforms

- **Linux:** X11 and Wayland, on Vulkan. The desktop host links
  `libudev.so.1` (gilrs's gamepad discovery) and `libfontconfig.so.1`
  (Servo's fonts). The device is created with the
  DMA-BUF extensions Chromium's frames need. Pointer lock turns at the
  browser's rate on both.
  Wayland locks the pointer. X11 hides the cursor and recentres it rather than
  grabbing it, because during a grab XInput delivers every raw motion twice.
- **Windows:** Windows 11, on Vulkan. The shared Windows test box has an
  RTX 3080: agents playtest on it through crew-services
  (`environment: "windows-desktop"`), and its access, agent and build setup are
  in crew-services `docs/playtest-windows.md`.
  - The Engine builds no DX12 backend. Each Chromium repaint, a D3D11
    texture, is copied on the GPU Vulkan draws with (matched by its LUID)
    into a texture with an NT handle. The device imports that texture
    (`VK_KHR_external_memory_win32`) and takes it from the external queue
    family, on the render thread.
  - Without a Vulkan GPU with that extension, or on a software adapter, the
    window refuses to open and the host's error names the reason. The
    renderer readout names the adapter and its backend.
  - `libcef.dll` is delay-loaded, so the pack keeps the Linux layout: the host
    points the DLL search at `lib/cef` before Chromium's first use.
  - The browser is created asynchronously; the page takes keyboard focus once
    it exists.
  - Chromium derives `KeyboardEvent.code` from the key's scan code (0xE0-prefixed
    for extended keys), which the shell passes as the native key code.
  - `--headless` needs a Chromium-based browser; `--chromium` can name Edge's
    `msedge.exe`.
- **macOS:** not built and not in scope. `welding` lists an IOSurface→Metal
  import, and the shell already runs the window on the main thread, as macOS
  requires.

Chromium runs unsandboxed: it shows the product's own first-party UI, never
arbitrary pages. Packaging (installers, updater, signing) is outside this
page.
