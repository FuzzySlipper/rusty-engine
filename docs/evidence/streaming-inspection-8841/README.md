# Playtest inspection from the runtime renderer (#8841)

In the streaming browser mode (#8786, `RUSTY_RENDER_OUTPUT=stream`), the runtime renders the world. Until this change, the crew-services `camera` and `drawing` ops and the capture presentation facts were answered by the Three surface only. Now the runtime renderer answers them.

The design is in [architecture: streaming browser mode](../../architecture.md#streaming-browser-mode), under "Inspection".

## What changed

**`render-wgpu`** (`composition.rs`):
- `Renderer::set_observer(Option<RendererCameraPose>)` replaces every primary view's camera pose; each view keeps its own projection.
- Offscreen views keep their cameras, as Three's `setObserver` did.
- Test: `tests/views.rs` `an_observer_pose_replaces_the_primary_view_camera_until_it_is_cleared`. It checks that the observed frame equals a direct render from that pose, and that clearing it restores the product camera's frame.

**`render-stream`:**
- `set_observer`;
- `set_on_demand` (on demand, changes wait for a request);
- `draw_now`, which draws one frame, even on demand or with no viewer, and waits until it is published;
- `inspection()`.

Each drawn frame records its facts (`DrawnFrame`): sequence on the frame route, simulation step, held flag, size, renderer id, retained world revision, composition and its revision, and observer. `ProductDevFrameStream::publish` now returns the sequence it assigns. `tests/stream.rs` adds the on-demand case.

**Runtime** (`frame_output.rs`, `lib.rs`):
- `engine.renderer.camera [none | x y z yawDegrees pitchDegrees]`;
- `engine.renderer.drawing [continuous | on-demand]`;
- `engine.renderer.frame`.

The debug catalog lists these only when the runtime renders the world (`ProductDevDebugCatalog::with_runtime_renderer_inspection`). A change draws a frame, and the answer names it (`frame: {sequence, step}`).

`engine.renderer.presentation` describes the last drawn frame in the browser observation's shape:
- `surfaceId` is `runtime-stream-<renderer id>`, and changes when a rebaseline replaces the renderer.
- `state` is `submitted`, or `pending` while a change or request awaits a frame.
- It carries the frontiers, the view revision, the viewport, and `views.{cameras, views, targets, presentations}`.
- It adds `frameSequence`, `simulationStep`, `held` and `observer`.
- `captureCorrelation` is `"frame-sequence"`: the canvas carries `data-rusty-frame-sequence` for the frame it shows.
- `observationRuntime` is always the current binding, because the renderer shares the runtime's process.

**Browser:**
- In streaming mode, `playtest-inspection.ts` sends `look`, `time`, `advance`, `drawing`, `frame` and `camera` to those commands.
- It then waits until the Engine canvas shows the frame the command named.
- `product-browser-host` no longer forwards held time to the stream surface. The stream surface's `inspection` member is never asked, and says so.

**Pick:** not answered, on purpose. No browser, runtime or C# caller asks the renderer for a pick; the only requester was `renderer-webview-host`, which has no dependents. The stream surface's `pick` member stays unsupported until a caller exists.

**Scope differs from Three:** the observer camera and drawing mode belong to the runtime renderer, so every attached page sees them. Three kept them per browser.

## Crew-services session

The run used the installed service, the unchanged `rusty-doom` profile, and Doom staged against this branch's SDK. Doom's pin `f1d747afc01c` predates #8840, so it was a private, uncommitted copy with the #8840 result renames applied. Responses are in `responses/`; screenshots are in `captures/` (crew-services originals, re-encoded).

| Step | Harness call | Result |
|---|---|---|
| 1 | `assist {op:"camera"}` | reads `drawing`, `held`, `observer` and the camera pose, plus `frame {sequence, step}` |
| 2 | `assist {op:"camera", move:[0,3,0], lookAt:[0,0,-20]}` | observer set; frame 538 names it (capture 01) |
| 3 | `capture {engine_presentation:true}` | `available: true`, `state: submitted`, `frameSequence: 545` with the observer pose |
| 4 | `assist {op:"camera", camera:null}`, then `capture` | product camera restored (capture 02); `observer: null` |
| 5 | `assist {op:"camera", camera:{position:[-7,14,14], yaw 0, pitch -45}}`, then `capture` | overview from above the map (capture 03) |
| 6 | `assist {op:"drawing", mode:"on-demand"}` | `frame-probe.py` sees no new frame for 1.5 s while the simulation runs |
| 7 | `assist {op:"frame"}`, then `capture` | exactly one new frame (1806, step 3364); `state: pending`, because the simulation moved on after it (capture 04) |
| 8 | `assist {op:"time", mode:"manual"}`, `camera orbit`, then `capture {engine_presentation:true, compare_to:<step 4>}` | observer drawn while held (`held: true`, `submitted`, capture 05) |

In step 8, `comparison.engine_presentation` reports `same_runtime`, `same_surface`, `same_submitted_cameras`, `same_submitted_view_layout` and `same_submitted_viewport` all `true`. `same_submitted_view_revision` is `false`: the product republished its composition in between.

## Checks run

- `cargo test -p render-wgpu -p render-stream -p product-dev-host -p csharp-product-runtime`: pass, including the new observer and on-demand tests.
- `cargo clippy --no-deps -D warnings` on those crates: clean.
- `scripts/dependency_boundary_check.py`: passed. `render-stream` now also names `render-host-contracts`.
- `pnpm run build` in `render/`: passes.
- `pnpm --filter product-browser-host test` (106 pass) and `--filter application-host test` (44 pass).

## Review fix: submitted cameras are the ones the frame drew from

**Finding.** `engine.renderer.presentation` copied the composition's camera
descriptors into both `views.cameras` and `views.sourceCameras`. An observed
frame therefore reported the product's pose as submitted
(`responses/04-capture-observer.json`: observer at `[0, 6, 3]`, pitch
−14.62, but `cameras` at `[-7, 1.62, 3]`, pitch 0). The crew-services
comparator reads `views.cameras`, so two different observer viewpoints
compared as `same_submitted_cameras: true`. Interpolated product cameras
also reported their descriptor instead of the sampled pose.

**Fix.**
- **`render-wgpu`.** `Renderer::drawn_cameras()` reports, aligned with the
  installed composition's cameras, what the last composition frame drew
  from:
  - a camera's pose and basis at the frame's presentation time (motion
    sampled);
  - where an observer replaced it in primary views, the observer's pose,
    marked `observer`, and the sampled pose its offscreen views still drew
    (`offscreen`). A camera that only offscreen views use is never replaced.
- **`render-stream`.** Each `DrawnFrame` records those cameras right after
  drawing.
- **Runtime** (`frame_output.rs`). `views.cameras` is each descriptor with
  the drawn `pose` and `basis`, plus `observer` and, when present,
  `offscreenPose`/`offscreenBasis`. `views.sourceCameras` keeps the
  descriptors as authored.
- The composition now resolves camera and target names once, when it is
  installed (#8849), so these facts come from the same indices the frame drew
  with.

**Evidence.**

| Test | What it checks |
|---|---|
| `render-wgpu` `tests/views.rs` `drawn_cameras_report_the_sampled_and_observer_poses_each_view_drew_from` | A motion camera drawn halfway between samples reports `[0.5, 0.6, 1.0]` (not the descriptor's latest `[1.0, …]`), and the frame's pixels equal a composition drawn from that pose. With an observer, the primary camera reports the observer pose (`observer: true`) and keeps its sampled pose as `offscreen`. An offscreen-only camera is untouched. A different observer viewpoint gives a different report. |
| `render-stream` `tests/stream.rs` | A drawn frame records the product camera, then, after `set_observer`, the observer's pose with `observer: true`. |
| `csharp-product-runtime` `frame_output::tests::submitted_cameras_carry_the_drawn_pose_and_keep_the_offscreen_one` | The JSON mapping: drawn `pose`, `observer`, `offscreenPose`, and the descriptor's id and projection. |

A live crew-services session again needs Doom on a current pair (#8861). The
chain from renderer to observation JSON is covered by the three tests above.

