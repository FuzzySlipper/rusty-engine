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
  events meanwhile. Servo wakes the event loop when it has work; Chromium is
  pumped on each key, button or wheel event and every 2 ms. Either way input
  reaches the page and the page's request reaches the runtime without
  waiting for a frame. `engine.renderer` reports the window's
  presented frames (time and step), its median acquire, lock, draw and
  present times, when input reached the window, and which step consumed
  recent input.
- **UI.** The window shows the same browser shell page `rusty dev` serves,
  from the runtime's own HTTP host (`http://127.0.0.1:<port>/`). A web
  engine renders it off-screen and `render-wgpu`'s `web` module composites
  it over the frame in gamma space: Servo by default, or Chromium (CEF)
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
  takes input, and the page's controller facts still count. A key or pad
  button already held when the window takes input was the page's (Escape or
  B closing a menu, which asks for the lock): gameplay gets nothing of it
  until it is released and pressed again. A held stick carries over. Giving
  back clears for the reason the lock ended (`pointer-lock-loss` or
  `focus-loss`). The page
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
- **Browser features.** The page runs in an off-screen web engine, which
  shows no native windows of its own: no dialogs, pickers, print previews or new
  windows. What each feature does there is
  [below](#browser-features-in-the-window).
- **Focus.** The off-screen page keeps its engine's focus while the window
  is in the background, so the shell reports window focus to it: focus loss sends the
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
- **The page through live debug.** Under either engine,
  `engine.ui.eval <expression>` evaluates JavaScript (the rest of the line)
  in the window's UI page, in its global scope, and answers with the
  engine's name and the value as JSON (awaited when it is a promise;
  `undefined` reads `null`), or what it threw. The answer's
  `renderedAtUnixMs` is when the page had laid out what the finished script
  changed (two animation frames after it), or `null` when no frame came
  within 250 ms or the page unloaded first (the script navigated away). `engine.ui.dom [selector]`
  answers the document's HTML, or the first matching element's. The window
  takes the script at its next wake (at most a frame later); a command waits
  up to 5 s for the page. The host answers both outside the runtime's
  serialization, so updates go on meanwhile and an awaited script may call
  the host's routes. A streaming runtime refuses both: its page runs in the
  viewer's browser.
  - A change a script makes is shown a few frames after it is sent, with no
    further script or input: on Linux 15 to 75 ms under Servo, which updates
    its rendering on a 20 ms timer. The answer can arrive before that frame.
  - To capture what a script changed, read `engine.renderer` until
    `window.uiRepaints.shownAtUnixMs` holds a time after the answer's
    `renderedAtUnixMs`, then capture. A repaint shown earlier, while an
    awaited script was still running, can predate its change.
- **Debugging protocols.** `rusty dev --cef-switch name[=value]` (repeatable, forwarded to
  `rusty-product-host --cef-switch`) passes Chromium switches; with
  `remote-debugging-port=<port>` a CDP client attaches to a Chromium page.
  Servo's page takes a devtools client instead ([UI engine](#ui-engine)).
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

The window runs the product UI page in an off-screen web engine (Servo by
default, or Chromium), so a browser feature that would open a native window
does not. None of the features below
can lock the page's input. Those a page might rely on warn in its console
(`rusty desktop shell: …`). Game UI keeps to the
[green path](csharp-product-style.md#game-ui-and-diagnostic-ui); diagnostic UI
may use any of this and runs fully in a browser.

| Feature | Under Chromium | Under Servo, where it differs |
| --- | --- | --- |
| Native HTML5 drag and drop | Refused at once: `dragend` with `dropEffect` `none`, never `drop`. Warns. | Never starts: Servo fires no `dragstart`; pointer events go on. |
| `print()` | Does nothing; the shell replaces it, since off screen it never returns and freezes the page. Warns. Ctrl+P is an ordinary key. | |
| `alert`, `confirm`, `prompt` | Cancelled at once: `undefined`, `false`, `null`. Warns. | |
| `beforeunload` prompt | Not shown; the page unloads. | |
| `window.open`, `target="_blank"` links | Not opened; `window.open` returns `null`. Warns with the URL. | |
| Same-tab navigation | Replaces the UI page. | |
| File and colour pickers | Do not open. Warns on click. | |
| `<select>`, date and time pickers | Work: the popup is drawn over the page. | The shell draws a `<select>`'s option list over the page, inside the view (click, arrow keys, Enter, Escape); a `multiple` list toggles options with a click or Space and answers with Enter or a click outside. A click on a `date`, `time` or `datetime-local` input (or Alt+Down on one) opens the shell's calendar and hour and minute lists; a choice sets the value with `input` and `change` events, honouring `min` and `max` days. Other date-like types (`month`, `week`) stay text fields. |
| `<dialog>` with `showModal()` | Works. | |
| `title` tooltips | Not shown. | |
| Image cursors (`cursor: url(...) x y, auto`) | Shown with their hotspot, scaled to the window's display scale. A held pointer lock hides them, as it hides any cursor. | Shown the same way: Servo reports only the keyword, so the shell's page script reads the image cursor (`url(...)`, or an `image-set(...)`'s first image and its resolution) of the element under the pointer and hands its pixels to the window. |
| `cursor: none` | Hides the native cursor while the pointer is over that element. | |
| Context menu | `contextmenu` fires; no native menu appears. | |
| Download links | Nothing downloads. | Nothing downloads and the page stays (Servo would load the file in its place). Warns. |
| Clipboard | `navigator.clipboard.writeText` works; `readText` is denied. | `readText` resolves with an empty string; the system clipboard is never read. |
| Fullscreen API | Resolves and fills the view; the native window does not change. | |
| Notifications, geolocation and other permission prompts | Denied at once. | Servo has no Notification or Geolocation API; the shell's stand-ins deny at once. |
| Gamepads | Chromium reads them itself. | The shell hands Servo the first pad in the standard mapping, named `Standard gamepad`. |
| Text entry, Tab focus, wheel scrolling | Work. IME composition has not been checked. A held key's repeats report `repeat` false. A wheel notch scrolls 53 pixels on Linux and 44 on Windows. | Held keys report `repeat`. A wheel notch scrolls 53 pixels on both. |
| Inline `<svg>` | Works. | Drawn as a picture of its markup: document stylesheets, `:hover`, `:focus`, inherited fonts and pointer events inside it do not apply; `focus()` and Tab reach an element with a `tabindex`. See [inline SVG](#inline-svg-in-the-window). |

### Inline SVG in the window

Servo 0.7 draws an inline `<svg>` as an image: it serializes the element's
markup and rasterizes that with resvg. What follows from that:

- **Only the markup is drawn.** Presentation attributes (`fill`, `stroke`,
  `display`, `transform` and the like) and `style` attributes apply. Rules
  from the page's stylesheets do not, so neither do classes on HTML
  ancestors, `:hover` or `:focus`, nor a font or colour inherited from the
  page; `getComputedStyle` still reports them. A `<style>` element
  inside the `<svg>` is read by resvg's own CSS parser, which skips some
  rules; don't rely on it.
- **Changes redraw it.** Servo itself redraws only when the `<svg>`
  element's own attributes or children change. The shell's page script
  watches every `<svg>` and marks the outermost one when anything inside it
  changes (an attribute, a child, text), so the picture follows the DOM.
- **Events reach the `<svg>` element.** Pointer events over any part of it
  target the `<svg>` itself, never an inner shape.
- **Text.** A `<text>` draws only with a `font-family` attribute. The
  Engine's Servo carries a fix to Servo 0.7's SVG font cache, without which
  text in a font that had drawn once was lost when the picture was redrawn
  ([servo-script patch](../rust/vendor/servo-script/RUSTY_PATCH.md)).
- **Focus.** An element inside the `<svg>` with a `tabindex` takes focus
  from `focus()` and from Tab, sets `document.activeElement` and fires
  `focus`, `focusin` and the previous element's `focusout`, as in Chromium.
  The Engine's Servo carries the fix: Servo 0.7 gives nothing inside an
  `<svg>` a layout box and so refused it focus
  ([servo-script patch](../rust/vendor/servo-script/RUSTY_PATCH.md)). An
  element counts as hidden only through a `display="none"` attribute on it
  or an SVG ancestor. A click does not focus it (the click goes to the
  `<svg>`), and `:focus` does not draw: show the focus by changing an
  attribute on `focusin`/`focusout`.
- **Clicks.** A click in an `<svg>` leaves Servo's caret in it, and Servo
  then draws the whole picture selected (tinted) when it next redraws it.
  The shell's page script drops a caret left in an `<svg>`.

`scripts/green-path/check.py --only svg` checks each point from the window's
pixels, after a click in the `<svg>`, and focuses a `<g tabindex="0">` by
`focus()` and by Tab; under Chromium every item passes, and under Servo the
focus items pass too.

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
`servo` (the default) or `chromium`. Set it with
`RustyEngineProductWindowUi`, or for one launch with
`rusty dev --window-ui chromium`; `rusty-product-host --window-ui` overrides a
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

`rusty build --release <dir> [--compress] [--aot]` writes a release for the
platform it runs on (linux-x64 or win-x64). The release runs where no `rusty`,
.NET SDK, .NET runtime or checkout is installed:

```text
<dir>/
  <launcher>[.exe]         named by RustyEngineProductLauncherName
  product.rpak             manifest, UI and content (as `--pack` writes it)
  coreclr/ or native/      the product's assemblies or NativeAOT module
  bin/                     rusty-product-host, and beside it on Windows
                           nethost.dll, libEGL.dll and libGLESv2.dll, and with
                           DLSS NGX's library
  lib/cef/                 only with renderer.windowUi = chromium
  share/browser/           the host's browser shell
  share/third-party/       the licences
  dotnet/                  the .NET runtime, for CoreCLR and the tools
  tools/<project>/         each tool the product names
  *.rpak                   containers installed beside the product
```

- **The host** comes from the pinned pair's runtime pack, or from its desktop
  pack when the product opens a window. `--runtime <pack>` names another
  pack, as for `rusty dev`.
- **No debug information.** The host, the launcher and the tools' test host
  library are copied stripped; their debug information stays in the pack
  (`.debug/` on Linux, `symbols/` on Windows). On linux-x64 the desktop host
  is about 175 MB and the test host library about 40 MB.
- **The launcher** is the executable a player starts. It is named by
  `RustyEngineProductLauncherName`, which defaults to the assembly name, with
  `.exe` on Windows. It runs `bin/rusty-product-host --product
  <dir>/product.rpak` with the loader the product declares (`native/` when
  built with `--aot`), and passes its own arguments on.
  - Its persistence root is the player's data directory for the product id:
    `$XDG_DATA_HOME/<id>` or `~/.local/share/<id>` on Linux,
    `%LOCALAPPDATA%\<id>` on Windows.
  - On Windows it opens no console window. The host's output goes to
    `last-run.log` in that directory.
- **The .NET runtime.** A CoreCLR product stays framework-dependent. The
  release carries the runtime of the SDK that built it in `dotnet/`, laid out
  as a .NET root: `host/fxr/<version>` and
  `shared/Microsoft.NETCore.App/<version>`. The host loads hostfxr from
  `dotnet/` when it is there beside `bin/`, and otherwise from the installed
  runtime. Its licences are in `share/third-party/dotnet/`.
- **Tools.** Each `RustyEngineProductTool` item names a tool project, an
  executable with `RustyEngineToolHost`:

  ```xml
  <ItemGroup>
    <RustyEngineProductTool Include="../DataImport/DataImport.csproj" />
  </ItemGroup>
  ```

  - Each tool is published framework-dependent into `tools/<project name>/`.
    Its apphost finds the runtime at `../../dotnet`, and the test host
    library is beside the tool.
  - A player runs it from the release, for example
    `tools/DataImport/DataImport <folder>`.
  - Its release directory is `AppContext.BaseDirectory/../..`. A tool that
    installs a container packs it there with
    `ProductContentBundle.PackContainer`, and the product opens it by its
    relative name (see
    [containers installed beside the product](csharp-product-project.md#content-containers-installed-beside-the-product)).
- **Setup steps.** The launcher offers no setup step of its own. A product
  that cannot start without a container shows the player the tool to run on
  its own entry screen.
- **Rebuilding.** Building into the same directory again replaces
  `product.rpak`, the product's artifacts, the launcher, `bin/`, `lib/`,
  `share/`, `dotnet/` and `tools/`. Anything else stays, such as containers a
  player installed.
- **The licences** in `share/third-party/`:
  - CEF's and Chromium's credits (`cef/CREDITS.html`, BSD and others): the
    host links CEF's wrapper whichever engine draws;
  - the source of the MPL-2.0 crates `welding`, `grafting` and `fidget-mesh`,
    which must stay available to recipients;
  - `servo/MPL-2.0-SOURCES.txt`, the unmodified MPL-2.0 crates Servo brings
    and where each one's source is, and on Windows ANGLE's licence
    (`angle/LICENSE`, BSD): the host carries Servo whichever engine draws;
  - `NOTICES.txt`, the licence and notice texts of every third-party crate
    the host links and the native libraries they build in;
  - the .NET runtime's `LICENSE.txt` and `ThirdPartyNotices.txt` (MIT) in
    `dotnet/`;
  - the rest of the pack's licences (`dejavu-sans/`, with DLSS
    `nvidia-dlss/`).

Installers, updaters and code signing are chosen when a product asks for one.

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

Chromium, when chosen, runs unsandboxed: it shows the product's own first-party UI, never
arbitrary pages. Packaging (installers, updater, signing) is outside this
page.
