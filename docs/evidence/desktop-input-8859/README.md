# Desktop-shell input on Wayland and X11 (#8859)

#8790 drove input only on X11, through XTest, and XTest relative motion
measured double. This run drives both platforms with a virtual pointer and
keyboard, measures the browser path for the same motions, and fixes the one
divergence it found.

## How input was driven

- **Compositor.** A private headless KWin (`scripts/headless-input.sh`: the
  #8790 compositor with `KWIN_WAYLAND_NO_PERMISSION_CHECKS=1`) with Xwayland.
- **Input.** `scripts/fakeinput` is a small client for KWin's
  `org_kde_kwin_fake_input`: absolute and relative pointer motion, buttons and
  evdev keys. It keeps one virtual device for a whole session, as a plugged-in
  mouse and keyboard would. KWin feeds that device's events through its
  ordinary seat:
  - on Wayland: `wl_pointer`, relative-pointer, pointer-constraints and
    `wl_keyboard`;
  - on X11: Xwayland's pointer, relative-pointer and keyboard devices.
- **The probe (`scripts/wayland_probe.sh`).**
  1. Click the canvas.
  2. Move 10 × 10 counts, then +100 in one event, then −50.
  3. Hold W for 600 ms, press Escape, press F3.
- **Readings.** Yaw and position come from Doom's `loading-bay.readout`. The
  page's pointer lock state is read over CDP (`RUSTY_CEF_SWITCHES`). Doom turns
  0.12° per unit of `movementX`.
- **Browser path.** `scripts/browser_probe.sh` runs Doom in stream mode and
  Chromium under the same compositor (`--ozone-platform=wayland` or `x11`),
  with the same probe.

## Results

| Path | Lock by click | +100 counts | +100 in one event | −50 | W 600 ms | Escape | F3 |
|---|---|---|---|---|---|---|---|
| Browser, Wayland | yes | 12.0° | 24.0° | 18.0° | moves | unlocks | — |
| Browser, X11 | yes | 12.0° | 24.0° | 18.0° | moves | unlocks | — |
| Desktop window, Wayland | yes | 12.0° | 24.0° | 18.0° | moves 3.6 units | unlocks | panel opens |
| Desktop window, X11, before | yes | **24.0°** | **48.0°** | **36.0°** | moves | unlocks | panel opens |
| Desktop window, X11, after | yes | 12.0° | 24.0° | 18.0° | moves | unlocks | panel opens |

- **Wayland is at parity with the browser.** `CursorGrabMode::Locked` works
  through pointer-constraints. Raw motion (`DeviceEvent::MouseMotion` from
  relative-pointer, unaccelerated) is 1:1. Keyboard codes (evdev + 8 through
  `keys.rs`) move the player.
  - Screenshot: `screenshots/wayland-window-f3.jpg`, with the debug panel
    open.
- **X11 turned twice as fast** with a real virtual device, so #8790's doubling
  was not an XTest artefact.
  - `scripts/winitprobe` (a bare winit 0.30.13 window) shows the cause. With the
    cursor under a confining grab, each raw motion arrives as two
    `DeviceEvent::MouseMotion` of the same delta. Without a grab it arrives
    once. `xinput test-xi2 --root` sees one `RawMotion` per motion.
  - XInput delivers raw events to the root window's selection and, during a
    pointer grab, to the grabbing client as well. X11 has no locked grab, so
    the shell fell back to `Confined`, which is such a grab.
- **Fix (`desktop-shell/src/overlay.rs`).** On X11 the lock no longer grabs.
  It hides the cursor and warps it back to the window's centre on every cursor
  move.
  - Raw motion arrives once, and the turn rate matches the browser's.
  - After 2,000 counts of motion the cursor is still in the window: it stays
    the active window and stays locked, and a click lands in it.
  - Wayland (`Locked`) and other platforms (`Confined`) are unchanged.
  - Screenshot: `screenshots/x11-window-f3.jpg`.

## Found, not fixed here: #8866

On Wayland, removing the seat's only pointer device and adding another, then
moving it, panics winit 0.30.13's pointer dispatch ("failed to get pointer
data"). The runtime goes down, locked or not. With hardware this is a
Bluetooth mouse reconnecting or a KVM switch. The shell cannot catch a panic
raised inside winit's dispatch; #8866 owns the upstream fix or a patch.

## Limits

- **No physical mouse.** The virtual device is unaccelerated, so pointer
  acceleration curves were not exercised. The shell reads unaccelerated raw
  motion on both platforms.
  - On Wayland, winit reports relative-pointer's unaccelerated delta.
  - On X11, it reports XInput raw values.
- **X11 through Xwayland, not a native Xorg server.** The doubling comes from
  XInput's raw-event delivery during a grab, which applies to both, but only
  Xwayland was run.

## Checks

- `desktop-shell` clippy with `web-overlay` passes, as does fmt.
- A `--desktop` runtime pack was built from this change, and the probes ran on
  it.

## Reproduce

Set `WORK` to a working directory that holds the built `fakeinput`, `desk/`
with these scripts, and the compositor state. Start the compositor with
`setsid dbus-run-session scripts/headless-input.sh $WORK/desk/state3`. Then
run `start_doom_window.sh` (add `x11` for the Xwayland path, with
`X11_DISPLAY`), followed by `wayland_probe.sh`.
