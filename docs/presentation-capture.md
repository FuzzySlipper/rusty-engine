# Viewpoints and presentation observations

Products own inspection viewpoints and any player/camera changes they cause.
`CameraQueries.TryLookAtPose(position, target, verticalYawDegrees, out pose)`
provides an optional derived camera pose in degrees. Vertical targets retain
an explicit yaw; coincident or nonfinite inputs return false. Apply the result
through ordinary `CameraView` or player look state. No Engine viewpoint registry,
automatic navigation, or global pause is required.

The controller interaction fixture exposes `viewpoint.visit entrance`, `near`
and `side` through its existing product debug module. A visit explicitly moves
the player, resets its motion and publishes the camera. Its returned viewpoint
name and pose are product facts; they are not proof that a frame has appeared.

## Engine observation

The built-in `engine.renderer.presentation` debug command describes the last
frame the runtime's streamed renderer drew:

- `runtime` is the Rust-owned instance/generation/control identity.
- `frameSequence` names the frame on the frame route, and the streamed canvas
  carries the same number as `data-rusty-frame-sequence`, so a capture of the
  page can be tied to the frame it shows (`captureCorrelation:
  "frame-sequence"`). `simulationStep` and `held` are the step the frame drew
  and whether inspection time was held.
- `views.cameras` are the poses the frame drew from: motion sampled, or the
  observer camera's where it replaced a primary view's camera (`observer`).
  `views.sourceCameras` are the product's descriptors.
- Without a streamed renderer (window output, or a runtime built without a
  renderer, as in tests) the answer is `available: false`.

A requested viewpoint can be compared against the camera the frame drew from,
rather than the latest live camera readout. Record the requested name/pose
separately. `engine.renderer.frame` draws one frame now and names it, and
`engine.renderer.drawing on-demand` keeps the renderer from drawing until
asked. GPU completion and whole-world readiness are reported as unavailable.

## Captures

Three captures are available, and they show different things:

- **World frames.** `GET /__rusty/product/runtime/frames?after=N` returns the
  next frame the runtime drew, with its `RSF1` header: sequence, simulation
  step, size, format, held and video flags (see
  [architecture](architecture.md#runtime-rendered-output)). The frame holds
  the world only, not the product's DOM UI. The step in the header is the
  frame's own, so no separate observation is needed to correlate it.
- **Tool captures.**
  `GET /__rusty/product/runtime/frames/capture?format=png|rgba[&width=W&height=H]`
  draws one world frame for a tool, in stream or window output.
  - **Its own target.** It draws at the requested size, or else at the
    output's (the stream's last frame, or the window's surface). It never
    counts as a viewer and is not published to the stream, so no page or
    window changes size.
  - **The answer.** An `RSF1` frame with a lossless PNG (format `3`, the
    default) or raw RGBA (`2`), with the simulation step and the held and
    video flags. Its sequence counts captures, apart from the stream's.
  - **Cameras.** `X-Rusty-Frame-Cameras` holds the drawn cameras, as
    `engine.renderer.presentation` reports them.
  - **Window output.** A window capture holds the world only; the UI
    overlay is not in it.
  - **Tying a capture to a step.** Hold time with `engine.time.mode manual`,
    advance it with `engine.time.advance`, and capture: the header's step is
    the one `engine.time` reports.
- **Composite page screenshots.** A screenshot of the page shows the world
  frame under the product UI and HUD. Read the canvas's
  `data-rusty-frame-sequence` (and `-step`, `-held`) at capture time, or
  record an `engine.renderer.presentation` answer next to the image.

The stream draws at the most recent viewer's size, so a harness pulling
`frames` and an attached page change each other's resolution; a harness
uses `frames/capture` instead. Record the viewport with a page screenshot. Keep product overlays and diagnostics unless the
capture says otherwise; `engine.renderer.hide` hides only the Engine metrics.
