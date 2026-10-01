# Desktop shell on Windows (#8858)

Recorded on 2026-10-01 on `den-win11`: Windows 11 Pro (build 26300), RTX 3080
(driver 32.0.15.9186), 1920x1080 display. The pack was built on the box with
`scripts/build-runtime-pack.sh --desktop` at the pair revision Doom pins
(`2dbff6a3cc70`) plus this change, and Doom's room study ran with
`rusty dev --runtime <pack> --output window`.

## Window and overlay

- `engine.renderer`: `adapter` "NVIDIA GeForce RTX 3080 (Dx12)", `output`
  "window", 59.9 frames per second over the last 120 presented frames; median
  acquire 1.75 ms, lock 0.0007 ms, draw 0.92 ms, present 0.21 ms.
- [desktop.png](desktop.png): the whole desktop. The window shows the world
  with the product's HUD, hint line and Debug panel composited over it.
- [window-capture.png](window-capture.png): the window alone, as Windows
  composes it (`PrintWindow` with full content), unfocused.
- [world-capture.png](world-capture.png): `frames/capture?format=png` in window
  output, 1280×720 at step 1101: the world without the overlay.
- Live debug answers at the host's origin: `engine.time`, `engine.renderer`,
  `playtest.observe`.

## Input (OS-level, through `SendInput`)

Sent by crew-services' Windows agent under a foreground lease; facts from
`playtest.observe`. Keys go as scan codes, as a keyboard sends them.

| Input | Result |
| --- | --- |
| Left click | Pointer lock taken; pistol fired (bullets 50 → 49) |
| W held 600 ms | Walked from z 3.0 to z −0.81 |
| Mouse moved 400 px right, locked | Turned right about 48° (forward 0, −1 → 0.743, −0.669) |
| Mouse moved 300 px left, locked | Turned left about 36° |
| Left Ctrl | Fired (49 → 48) |

[after-os-input.png](after-os-input.png) shows the moved, turned view with 48
bullets on the HUD.

## Defects found and fixed

- `PlatformCefProducer::new` takes the host's wgpu context on Windows; the
  shell did not pass it, so the desktop feature did not compile there.
- `libcef.dll` was linked at load time, so the host could not start with the
  pack's `lib/cef` layout; it is now delay-loaded.
- Opening the overlay asked the page for focus before Windows' asynchronous
  browser existed and failed the window; the first focus now waits for it.
- The shell passed a `WM_KEYDOWN` lParam as the native key code, and Chromium
  left `KeyboardEvent.code` empty, so no gameplay key reached the product (the
  page saw `keydown` with `key` "w" and `code` ""). It now passes the scan code;
  W, ArrowUp and ControlLeft arrive as `KeyW`, `ArrowUp` and `ControlLeft`.
- Building on Windows: `RELOAD_ASSETS_COMMAND` lived in the Unix-only
  supervisor; the product UI types check used `node_modules/.bin/tsc` and
  compared CRLF, backslash-separated output; `--headless`, which needs that
  supervisor, is now refused instead of ignored.

## Not covered

Dagger's cinematic and soundtrack were not run: Dagger pins another pair
revision, which needs its own Windows pack.

## A published pair on Windows (#8889)

Pair `0.1.0-dev.73f9e99ece2e` carries win-x64 archives, built on the box with
`scripts/publish-windows-pair-packs.sh` and listed under `targets."win-x64"`
in its `pair-release.json`. With Doom pinned to it on the box:

- `rusty install` downloaded and verified
  `rusty-engine-csharp-pair-0.1.0-dev.73f9e99ece2e-win-x64.tar.gz`.
- `rusty status`: exact pin and pair feed declared; the cache holds the pair;
  dotnet 10.0.401, curl and tar found; ready.
- `rusty dev --output window` resolved the pin, ran the pair's own `rusty`,
  downloaded the win-x64 desktop pack, and opened Loading Bay — Doom E1M1:
  "NVIDIA GeForce RTX 3080 (Dx12)", 59.9 fps
  ([published-pair.png](published-pair.png)).

The room study did not build against this pair: #9032 changed
`CollisionNavigationConfig`, which `LoadingBayRoomStudy.cs` constructs.
