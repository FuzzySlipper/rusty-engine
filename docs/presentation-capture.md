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
frame the runtime's renderer drew:

- `runtime` is the Rust-owned instance/generation/control identity.
- `frameSequence` names the frame on the frame route, and the streamed canvas
  carries the same number as `data-rusty-frame-sequence`, so a capture of the
  page can be tied to the frame it shows (`captureCorrelation:
  "frame-sequence"`). `simulationStep` and `held` are the step the frame drew
  and whether inspection time was held.
- `views.cameras` are the poses the frame drew from: motion sampled, or the
  observer camera's where it replaced a primary view's camera (`observer`).
  `views.sourceCameras` are the product's descriptors.
- With no renderer in the process (a runtime built without one, as in tests),
  the answer is `available: false`.

A requested viewpoint can be compared against the camera the frame drew from,
rather than the latest live camera readout. Record the requested name/pose
separately. `engine.renderer.frame` draws one frame now and names it, and
`engine.renderer.drawing on-demand` keeps the renderer from drawing until
asked. GPU completion and whole-world readiness remain unavailable.

## crew-services GPU captures

Use the configured native GPU session and existing capture facility. Record
`engine.renderer.presentation` responses and their request/response times next
to the original capture and its sidecar. Revisit the product viewpoint and
compare the submitted camera and viewport before visual comparison. Keep
product overlays and diagnostics preserved unless an explicitly supported
capture policy says otherwise; `engine.renderer.hide` only hides Engine metrics.

A Moonlight/X11 PNG and a separately requested Engine observation are separate
evidence. Neither a successful input receipt, a delay, nor a submitted revision
identifies the frame in the PNG. Preserve `frame_correlation: unavailable` in
crew-services until a real capture handshake or image marker supplies that
identity. The current Engine facts improve diagnosis and repeatability without
claiming remote stream freshness.
