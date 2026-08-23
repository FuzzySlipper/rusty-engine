# Shared rendering operations

Rusty Engine's renderer is one subsystem with two independently verified halves
and one Engine-owned Rust host:

- ordinary Rust crates (`render-model`, `render-projection`, and `render-presentation`) produce and
  validate renderer-neutral retained and presentation frames; and
- the isolated `render/` pnpm workspace decodes those frames and owns retained projection, Three,
  WebGL, WebAudio, DOM presentation hosts, inspection, and editor viewport mechanisms; and
- `renderer-webview-host` embeds a reproducible compiled renderer artifact and exposes the
  supported downstream boundary entirely through Rust.

Downstream code owns game/tool meaning and resource policy. It emits typed Rust frames, pre-admits
content-addressed resource bytes, and owns its outer window and event loop. It does not construct a
second retained scene or Three backend and does not import the renderer's TypeScript packages.

## Provider verification

The ordinary provider gate requires Rust and shell tools, but not Node or pnpm:

```bash
./scripts/verify.sh
```

It runs the Rust render suites, the complete donor disposition check, and the historical-runtime
isolation audit as part of the normal workspace. The browser implementation is deliberately a
separate install and gate:

```bash
./scripts/verify-render.sh
```

That command performs a frozen install under `render/`, checks the four-package dependency graph,
typechecks/builds/tests every package, and runs the real Chromium/WebGL/WebAudio/DOM/GLB proof.
Neither command requires an Asha, demo, or Studio sibling checkout.

## Rust-only downstream consumption

An ordinary downstream game declares one unconditional local Rust dependency:

```toml
[dependencies]
rusty-engine = { path = "../rusty-engine/rust/crates/rusty-engine" }
```

The checkout is consumed as it stands. Downstream does not fetch, pull, reset,
clean, checkout, pin, or enforce a provider freshness policy; operator update
policy belongs outside the consumer repository.

`rusty_engine::renderer_webview_host::RendererWebviewAdapter` mounts one Wry child webview in a
downstream-owned window. Named Rust methods submit retained and presentation frames, configure
views and camera state, pick, resize, and control lifecycle. Typed observations report readiness,
operation receipts/failures, renderer readouts, physical input, picks, diagnostics, and disposal.
No generic JavaScript invocation, eval, module import, or browser object is public.

The adapter uses the operating system webview. Linux builds therefore require GTK 3 and
WebKitGTK 4.1 development packages; the Engine CI installs `libgtk-3-dev` and
`libwebkit2gtk-4.1-dev` explicitly. Headless execution of the real Linux host proof additionally
uses Xvfb. These are native build/runtime prerequisites, not downstream TypeScript dependencies.
The current Wry child-window adapter is an X11 diagnostic/product path on Linux; its child embedding
does not claim Wayland support. A product needing rich DOM or the same browser composition in a
web app, Tauri, or Electron uses `@rusty-engine/application-host` instead of building another child
webview seam downstream.

The Engine-private bridge and compiled artifact remain under `render/private` and
`renderer-webview-host/artifacts`. First-party Engine tools may still use package-root TypeScript
APIs inside this repository, but those packages are not an ordinary downstream game surface.

Run the clean temporary-consumer proof locally or against a public review revision with:

```bash
./scripts/verify-rust-sdk-consumer.sh
```

This is a focused facade/consumer proof. Select a downstream checkout explicitly
when a cross-repository adapter or browser check is needed, and record exact
source heads in Den evidence rather than in an Engine dependency manifest.

The renderer-owning gate additionally rebuilds the closed artifact byte-for-byte, tests the Rust
contracts and adapter, and mounts it through real Wry/WebKit under X11. The Chromium artifact proof
verifies the same fixed private contract in a browser engine.

### Camera-relative retained presentation

Consumers use the shared retained frame; they do not construct another Three scene, camera, render
pass, or animation loop. Create a renderer-neutral root and parent ordinary retained assets below
it:

```ts
{
  schemaVersion: 1,
  ops: [
    {
      op: 'create',
      handle: renderHandle(900),
      parent: null,
      node: {
        geometry: { kind: 'group' },
        material: { color: [1, 1, 1, 1], wireframe: false },
        transform: {
          translation: [0, 0, 0],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        layer: 'viewmodel',
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: [],
          label: 'viewmodel-root',
        },
      },
    },
    {
      op: 'createAnimatedMeshInstance',
      handle: renderHandle(901),
      parent: renderHandle(900),
      instance: {
        asset: 'mesh-animation/example',
        transform: {
          translation: [0.45, -0.35, -1.2],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        materialOverrides: [],
        playback: null,
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: [],
          label: 'camera-relative-example',
        },
      },
    },
  ],
}
```

Downstream Rust still owns weapon/equipment meaning and emits the selected visual and bounded
disposable offsets as ordinary retained diffs. Apply those frames through `RendererSurface`.
Camera synchronization, `start`, `stop`, `renderOnce`, resize, reset, picking, and `dispose` stay on
that one surface. A rejected frame leaves both the neutral projection and backend unchanged.

### Exact held animated-mesh samples

`setAnimatedMeshPlayback` accepts the ordinary playback commands plus a neutral held sample:

```ts
{
  op: 'setAnimatedMeshPlayback',
  handle: renderHandle(901),
  playback: { kind: 'sample', clip: 'run', normalizedTime: 0.5 },
}
```

`normalizedTime` is finite and inclusive from `0` to `1`; the named clip must be
declared on the admitted animated-mesh asset. The command evaluates that instance
at the exact clip position and holds it across subsequent renderer advances.
Another instance remains independent. A later `sample`, `play`, or `stop` replaces
the held state through the same retained operation; `play` returns to advancing
playback. Invalid clip/time input rejects before retained projection or backend
playback changes. Renderer-host playback readout reports `status: 'sampled'` and
the exact `{ clip, normalizedTime }` `heldSample`, rather than presenting it as a
resumable pause.

This is presentation control for inspection, authored previews, and explicitly
projected visual moments. It does not define gameplay animation states, cues,
scheduling, root motion, or consequence policy; downstream Rust remains the
authority that decides whether and when to emit the retained command.

### Authored sky background

Publish an admitted retained texture before selecting it as the sky:

```ts
{
  schemaVersion: 1,
  ops: [
    { op: 'defineTexture', texture: admittedPanorama },
    { op: 'setSkyBackground', background: { texture: admittedPanorama.id } },
  ],
}
```

`admittedPanorama` is one content-hashed PNG payload with a 2:1 aspect ratio,
`srgb` color space, and `clamp` wrapping. Emit `background: null` to return to
the configured clear color. The sky is camera-rotation-relative only and is
excluded from depth, world handles, picks, collision, lighting, and reflection
state. Texture replacement refreshes it without changing authored identity.

### Bounded multi-view composition

`RendererSurfaceOptions.viewComposition` and `surface.configureViews(...)` accept schema-1
renderer-neutral camera, target, view, and presentation descriptors. The provider admits at most
four cameras, four named targets, eight views, four target presentations, 2,048 pixels per target
dimension, and 8,388,608 aggregate target pixels. A target revision is a caller-owned monotonic
resource identity: changing dimensions, format, depth, or sampling requires a higher revision, and
an old revision cannot be recreated after removal.

Each submission renders offscreen producers first, then primary views and target presentations by
`order` and stable identifier. Every view observes the same retained scene snapshot. A presentation
samples its target directly on the GPU into a normalized lower-left primary viewport; the public
surface exposes only an immutable status/resource readout, never a Three texture, WebGL framebuffer,
or CPU pixel buffer. A target that has not yet rendered reports `never_rendered`; a refreshed target
reports the exact surface submission that last produced it. Target-to-target presentation is
rejected to keep feedback unsupported and explicit.

Configuration validation and resource allocation complete before publication. Invalid identities,
non-finite camera facts, duplicate producers, stale revisions, quota overflow, and allocation
failure return a typed non-applying receipt and retain the previous composition. Resize uses the
surface's physical backing-buffer coordinates after accounting for the backend pixel ratio.
Replacing or removing a target disposes its GPU resource; disposing the surface releases every
target and presentation while preserving the one caller-owned start/stop/render loop. Omission
retains the compatible single-camera behavior.

## Reference consumer evidence

The exact revisions in this section are historical certification evidence for
the rendering migration. They are not current downstream dependency pins or
freshness instructions; current consumers use the adjacent Rust facade and
Engine-owned renderer/Studio boundaries.

Rusty Roguelike commit `098b6d6c468711b4c149583996ac5147c9f58941` used Engine commit
`8673aaa6d0b811195b3904f34d7729c0d6e92530` for the first exact multi-view consumer proof. One public
surface renders the already-admitted retained local scene through a bounded orthographic offscreen
view and GPU-presents it as a responsive inset. The real desktop/mobile proof inspects distinct
framebuffer regions, target revision replacement, narrow sizing, save/reopen continuity, and
unchanged Rust session/minimap facts; it does not move discovery or visibility into TypeScript.

`rusty-engine-demo` commit `42f428b0ee3f47de94d4372f512978f587d729f7` consumes that exact
Engine baseline with no sibling override. Its private render-contracts and renderer-three packages
are deleted. The loading-bay product maps typed Rust facts into shared presentation descriptors and
uses the shared retained surface plus audio, billboard, particle, and telemetry hosts. Its full gate
proves voxel meshes/edits, collision, navigation, combat, doors, beacon presentation, camera sync,
reset/reopen cleanup, and real Chromium behavior.

The camera-relative channel has a separate exact consumer certification. `rusty-engine-demo`
commit `4b58555631badbe58fc4c2828dfa4e5bd1effb60` pins all four renderer packages to Engine commit
`e622c941671bc0f167206b049ab94ea63495a86d`. It retains seven weapon-presentation nodes on
`viewmodel` through the existing `RendererSurface`, with no downstream Three scene, renderer, or
scheduling loop. Its real Chromium/WebGL acceptance covers world-camera motion, pick exclusion,
desktop and narrow resize, death/reset restoration, disposal on return to the main menu, and remount
on Continue.

Studio is an Engine-owned workspace and host. Downstream projects provide only
project data and a Rust adapter through the `.rusty-studio.json` boundary; they
must not install, import, build, or configure a second Studio or renderer
package surface.

## CI topology

| Gate | Trigger and purpose |
|---|---|
| `verify / verify` | Every Engine push/PR; Rust-only provider and completeness proof with no Node installation. |
| `render / verify-render` | Render/Rust-contract paths; frozen isolated workspace plus real browser proof. |
| `studio / verify-studio` | Engine-owned Studio workspace and host proof; it does not launch a downstream product. |
| Downstream integration commands | Explicitly selected affected consumer; narrow adapter/browser proof with exact heads retained in Den evidence. |

Cross-repository proof is intentionally explicit. Engine verification validates
Engine-owned surfaces, while each downstream repository owns its own full suite;
an Engine change does not automatically launch every downstream product.

## Surface timing and telemetry

`RendererSurface` owns the browser animation loop and every explicit `renderOnce` submission. The
single callback registers its sole successor before camera, presentation, and WebGL work so the
current submission cannot delay registration beyond the browser's next display scheduling window.
The surface is also the source of renderer timing observations. One immutable
`RendererSurfaceTimingSample` is produced after each successfully submitted frame:

- `frameIntervalMs` is render cadence: the difference between consecutive surface source
  timestamps. The existing telemetry metric `frameTimeMs` deliberately means this cadence. It is
  not CPU work duration and it is not GPU completion time.
- `backendSubmissionDurationMs` is synchronous host-clock time spent inside the backend
  `renderOnce` call. It does not include asynchronous GPU completion.
- the first frame, a regressed source timestamp, or a source gap above 60 seconds has no cadence
  value and carries a classified status instead of inventing zero;
- unavailable, regressed, or excessive backend-clock duration is likewise reported as unavailable;
  and
- the surface retains only its latest frozen sample. The telemetry collector's frame-time history
  remains separately bounded to 1..=240 samples.

An explicit caller receives timing directly from `surface.renderOnce(timeMs)`. An auto-started
caller reads the latest timing through `surface.timing()` without starting or polling another render
loop. `surface.submission()` returns the complete latest immutable submission sample: the same
timing plus renderer-owned statistics. The existing overlay host accepts that value directly:

```ts
const timing = surface.submission();
telemetry.sampleSurface({
  sourceTick,
  timing,
  counters: { entityCount },
}, timing.sourceTimeMs);
```

The older `telemetry.sample({ frameTimeMs, ... })` seam remains available for a non-surface owner
that already has a real cadence measurement. Downstream code must not use a placeholder zero or
reinterpret `frameTimeMs` as backend submission time. Timing is read-only presentation diagnostics;
it does not advance gameplay, camera authority, or animation scheduling.

### Submission and resource statistics

Every successful automatic, mount, camera-reset, or explicit submission also publishes one frozen
`RendererSurfaceStatisticsSample`. Each counter is a discriminated `available`, `unavailable`, or
`unsupported` value; a missing value is never represented as zero. The counter itself carries its
scope:

- `drawCallCount` and `triangleCount` are `perSubmission`. Three resets its public renderer-info
  counters before the combined world-plus-viewmodel submission and reads them after both passes.
  They do not claim GPU completion or time.
- `renderHandleCount`, `geometryResourceCount`, `materialResourceCount`,
  `textureResourceCount`, and `animatedInstanceCount` are `liveResident`. They describe exact
  backend-owned retained resources immediately after that submission, not authored/gameplay facts
  and not cumulative allocation totals.

Three caches live-resource counts after accepted retained mutations, so submitting or reading a
sample is constant work apart from the render itself. Resource identity is de-duplicated: two
instances sharing one retained geometry count as one geometry, while independently cloned animated
instance resources count independently.

Automatic WebGL2 submission uses asynchronous backend pacing without adding another render loop.
Software, unknown, and timer-fallback paths keep at most one timer measurement and, where
available, one completion fence in flight. Positively identified accelerated WebGL2 with working
timer-query support may instead use an eight-slot renderer-owned completion-query ring. Each timer
query is a completion observation for its enclosed command stream; a sync-fence ring supplies an
additional bound where WebGL exposes that mechanism. The fixed cap bounds queued command streams
while allowing display-rate work to continue when Radeon/ANGLE exposes a completed query
50–100 ms after the measured 4–7 ms GPU work. When
`EXT_disjoint_timer_query_webgl2` is available, completed submissions' GPU durations determine
an adaptive headroom interval before the next automatic submission. Some
software renderers report a short timer duration while their asynchronous completion still
occupies browser CPU. The Three backend therefore classifies the concrete WebGL renderer locally:
a positively identified software renderer uses all observed completion wall latency as effective
work. A valid timer result on positively identified accelerated hardware is authoritative for
execution duration, so delayed query-result polling does not inflate GPU work. The accelerated
prospective deadline is anchored to the accepted animation-frame submission timestamp, rather than
the later callback-observation wall clock; this avoids both charging already enclosed work twice
and skipping a display interval because callback jitter changed between frames. Unknown renderers
and timing fallback paths retain one ordinary 60 Hz polling allowance before wall latency
contributes. Every nonzero effective duration receives at least equal headroom, so
even a seconds-long software submission cannot exceed fifty-percent automatic duty. Up to 100 ms
of additional progressive headroom lowers ordinary slow work toward a twenty-percent floor without
adding an unbounded extra delay. The diagnostic reports the selected duty after that bound, not an
unachievable pre-cap target. This is not a fixed-rate timer: the
accelerated fast path can still submit four-millisecond work at 120 Hz and eight-millisecond work
at 60 Hz, while slow completion yields materially more CPU time. Delayed browser observability
can fill the accelerated ring, but cannot grow it or silently become an unbounded queue. Any
timer-query failure immediately restores strict single-slot admission. Timer-query and completion
polling is non-blocking and occurs only when the single RAF owner evaluates an automatic
submission. The host does not run timer-based readiness bursts between display callbacks: after
accelerated completion became a bounded ring, such probes could not create a presentation
opportunity and merely competed with delivery of the next RAF. Unsupported, disjoint, malformed,
or failed timer timing retains the completion-wall policy instead of silently disabling pacing.
`surface.automaticSubmissionPacing()` returns one
frozen renderer-owned diagnostic with the timer mode, current measurement state, renderer class
and allowance, latest completed decision's timer duration, observed completion age, effective
duration, selected duty, decision time, admission deadline, actual automatic admission
observation, selected and current admission limits, timer-query occupancy, and sync-fence
availability and occupancy. The same readout includes `hostAdmission`: exact lifetime outcome
counters plus the most recent 64 RAF admission attempts. Each attempt carries its source time,
request/resize/controls/presentation/retained-animation demand reasons, the
`admitted`/`backendBlocked`/`noDemand` outcome, and the backend's pre-submission capacity,
measurement, fence state, timer/effective duration, decision observation time, and deadline. The
same attempt also carries wall-clock boundaries for callback entry, successor registration,
demand evaluation, backend readiness, controls, camera, presentation, backend submission, and
callback exit; phases not reached by a no-demand or blocked attempt are explicitly null. The
bounded history therefore distinguishes callback CPU work and post-callback browser delay in
addition to sparse RAF delivery, missing owner demand, and backend rejection, even when a consumer
observes only occasional accepted submissions. Reading it never polls, submits, mutates demand, or
starts another loop. Explicit
`renderOnce`, camera reset, resource statistics, picking, and disposal keep their established
semantics.

The browser backend additionally caps a positively identified software rasterizer's backing-buffer
ratio at `0.25`, bounding requests at or above one device pixel per CSS pixel to one-sixteenth of
the CSS pixel count. This reduces the cost of the first complete submission as well as later camera
frames; scheduling alone cannot yield CPU time already consumed by one expensive raster. The CSS
viewport, camera projection, normalized picking, retained content, and lifecycle remain unchanged,
and lower caller requests are preserved. Accelerated and unknown renderers retain the requested
ratio.

An accepted retained mutation becomes visible in statistics on the next successful submission; a
rejected mutation changes neither renderer state nor the latest sample. Camera reset submits one
ordinary classified sample without changing resource counts. Stopping a surface does not alter its
latest sample. Disposing releases the backend resources, rejects future submissions, and leaves
already returned samples as immutable historical observations rather than rewriting them to zero.
A replacement `RendererSurface` starts a fresh sequence and never inherits samples or counters from
the disposed instance.

The Studio inspection surface uses the same statistics and timing owner. After a complete,
incremental, or presentation-only authored frame is accepted, `StudioViewportComponent` performs
one explicit submission and only then reads `RendererInspectionSurface.submission()`. Its public
`frameSubmitted` output associates that immutable sample with the authored generation and a closed
`complete`, `incremental`, or `presentation` update kind. This ordering prevents an earlier
automatic-loop sample from being attributed to newly accepted content. Selection and preview
changes retain the authored generation and publish `presentation`; the legacy generation-only
`frameApplied` output remains limited to accepted authored-generation changes.

The editor backend counts one complete submission across its runtime, authored, procedural-grid,
and overlay render passes. Three's per-render automatic statistics reset is disabled only while
that explicit multi-pass submission is measured, then restored. An empty final overlay therefore
cannot erase the visible authored and grid work from the reported draw and triangle counts.

`StudioShellComponent` forwards the child's `frameSubmitted` event unchanged through its own public
output. Product composition roots therefore observe the generation-associated renderer sample
without reaching through Angular internals, mounting another viewport, or creating a second
telemetry path. The shell retains its existing internal `frameApplied` workspace acknowledgement.

Rejected frames publish neither output nor a new submission. Resize, stop, and disposal do not
rewrite the last immutable historical sample, while remounting creates a new inspection surface
whose submission sequence begins independently. Studio receives no Three/WebGL handle and does not
create a telemetry loop or counter path of its own.

`RendererLiveTelemetryCollector.sampleSurface` takes renderer-owned counters from a complete
submission sample. Caller counters may still supply game/product values such as `entityCount`, but
cannot override draw, handle, geometry, material, texture, animation, or triangle observations.
The boundary rejects timing-only samples, malformed statistic shapes, statuses, or scopes, and
renderer-owned keys in the caller counter map before changing telemetry history or its latest
snapshot.

### Exact downstream certification

The external Loading Bay consumer certified this public surface in
`rusty-engine-demo` revision `92745e291097d22574ac3fe0d01c3b6e19a02697`, pinned to Engine
revision `a6857d03141e162511231c276ee751a3413c90e5`. The product used its ordinary auto-started
`RendererSurface`; the probe added no Three/WebGL import, private renderer access, second surface,
or second frame loop. Three explicit submissions observed the real placeholder, a temporary richer
load of 32 visible instances sharing four static resources, and the surface after destroying that
temporary root:

| Renderer statistic | Scope | Placeholder | Rich load | Restored |
| --- | --- | ---: | ---: | ---: |
| Draw calls | `perSubmission` | 39 | 71 | 39 |
| Live render handles | `liveResident` | 51 | 84 | 51 |
| Geometry resources | `liveResident` | 43 | 47 | 43 |
| Material resources | `liveResident` | 55 | 59 | 55 |
| Texture resources | `liveResident` | 0 | 0 | 0 |
| Animated instances | `liveResident` | 0 | 0 | 0 |
| Submitted triangles | `perSubmission` | 14,380 | 14,444 | 14,380 |

Every value had `available` status, including the exact zeros. The richer load therefore proves
counter sensitivity, shared-resource accounting, and disposal restoration through only the public
surface. It is a deterministic stress load, not a substitute for measuring the final authored
Loading Bay scene. The complete downstream product gate included a real Chromium/SwiftShader
browser run; those exact counts are lifecycle/correctness evidence and make no hardware GPU timing
claim. Downstream Den task #6378 and its checked evidence artifact own the product-side record.

## Resource and authority rules

- Static/shared/animated mesh bytes, audio clips, fonts, icons, and particle sprites arrive through
  narrow caller-owned resolvers with expected content hashes. There is no implicit URL fetch or
  ambient catalog/provider registry.
- Shared mesh borrows are copied before release. Renderer lifetime never aliases mutable gameplay
  or tool buffers.
- Picks, sampled animation cues, host readouts, and telemetry are observations. A downstream owner
  revalidates any action against current authoritative state.
- Camera state belongs to the caller or the disposable surface controller. It is not persisted into
  game authority by the renderer.
- Reset/dispose clears retained Three resources, audio graphs, DOM overlays, particles, telemetry,
  animation mixers, listeners, and render loops without replaying presentation history.

## Known limitations and intentional representation changes

- Three/WebGL is the only implemented GPU backend. The renderer-neutral frame and projection layers
  leave room for another backend, but no WebGPU implementation is claimed.
- Authored sky presentation is one equirectangular panorama. Cubemaps, HDR
  environment lighting, reflections, exposure policy, and post-processing are
  outside this contract.
- JavaScript-facing Rust integers are limited to `0..=2^53-1`; both Rust encoding and TypeScript
  decoding reject larger identities rather than silently losing precision.
- `RendererSurfaceOptions.lighting` schema 1 independently selects `neutral` or `disabled` default
  rigs for world and viewmodel, and enables a bounded retained-shadow budget. Omission preserves the
  old neutral/neutral, shadows-disabled behavior. `lightingReadout()` reports neutral counts and each
  retained request as active, disabled, or unsupported; rejected over-budget frames are atomic.
- Shadows remain the ordinary Three shadow-map implementation over retained scene meshes and
  directional/point/spot lights. Ambient requests are explicitly unsupported, and no automatic light
  placement, artistic exposure policy, or gameplay lighting is inferred.
- Historical downstream lighting certification is owned by Rusty Roguelike commit
  `e88856aca2b07212e79ca8a9a8cdc904cb49bd61`, which used Engine
  `b1f0415af6266783246371d227a2272de7d9f0d6`. Its clean full gate proves a Rust-projected torch
  count equals the public retained-light readout and a real Chromium framebuffer has localized warm
  falloff; the consumer retains placement and gameplay meaning.
- Multi-view composition currently supports one RGBA8 sRGB color policy, optional depth24, nearest
  or linear sampling, and GPU presentation onto the primary surface. It does not expose arbitrary
  render-target materials, CPU readback, target feedback, cubemaps, post-processing, gameplay
  minimap discovery, or another render scheduler. Exact downstream certification is Rusty Roguelike
  `098b6d6c468711b4c149583996ac5147c9f58941`, pinning Engine
  `8673aaa6d0b811195b3904f34d7729c0d6e92530`.
- Animation is deterministic controller/playback presentation, not skeletal state authority.
  Sampled cues cannot mutate gameplay directly.
- DOM billboards, particle billboards, and telemetry are default host realizations. A consumer may
  provide another explicit sink, but it must consume the same typed descriptors and lifecycle.
- Packages are currently versioned as one `0.1.0` family and distributed by exact Git revision;
  no semver registry publication or compatibility range is promised yet.
- Asha compatibility manifests, generated tunnels, runtime/session bridges, replay certification,
  catalog/project-bundle authority, and provider registries are intentionally replaced boundaries,
  not deferred rendering features. Every donor file and replacement is recorded in
  [`../render/donor-disposition.tsv`](../render/donor-disposition.tsv).
