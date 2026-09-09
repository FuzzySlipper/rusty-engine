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

The built-in `engine.renderer.presentation` debug command returns the latest
browser observation through the existing renderer feedback and debug transports:

- `runtime` is the Rust-owned instance/generation/control identity. Feedback is
  fenced against a different runtime binding. `observationRuntime` retains the
  binding of the stored feedback; after control replacement the old observation
  is unavailable until feedback for the current binding arrives.
- `available: false` and `presentation: null` mean no supporting browser
  observation has arrived. This is a successful query with unavailable data.
- `observationAgeMs` is time since Rust received the feedback, not screenshot
  age or GPU latency. The browser reports on the existing diagnostics cadence.
- `presentation.surfaceId` changes when the renderer surface is replaced.
- `presentation.submitted` is captured immediately after successful WebGL
  submission. It records the surface-local render sequence/source time, actual
  CSS and backing viewport dimensions, Rust-owned publication frontiers,
  configured views/cameras/offscreen targets, and known material/sprite fallbacks.
  `fallbackCamera` describes the default camera; for a configured primary view,
  use its camera ID in `views.cameras`.
- `viewRevision` is a **surface-local composition counter**, not a Rust
  publication revision. Compare it only within the same surface. Publication
  stream revisions are copied from the canonical Rust publications after their
  successful realization.
- `state: pending` means a newer installed publication, view composition or
  viewport has not been submitted, or an asynchronous presentation application
  is outstanding. `unavailable` marks an unusable surface. `submitted` means
  those currently observed facts agree with the last submission.

For a requested stream revision, compare it against that stream in `submitted`;
a continuously changing scene need not become globally idle. Require the same
runtime and surface identities. A requested viewpoint can be compared against
the submitted view camera, rather than the latest live camera readout. Record
the requested name/pose separately. Timeouts should retain the last observation,
age, target and pending facts.

These observations do not promise all future streaming work is done. The
pending count covers asynchronous presentation applications on this surface;
offscreen target status and fallback counts describe their own resource scopes.
Animation can advance between frames. GPU completion, whole-world readiness and
remote screenshot correlation remain explicitly unavailable.

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
claiming remote stream freshness. The browser feedback path does not require
DOM access on the GPU harness or a den-services adapter.
