# Shared rendering operations

Rusty Engine's renderer is one subsystem with two independently verified halves:

- ordinary Rust crates (`render-model`, `render-projection`, and `render-presentation`) produce and
  validate renderer-neutral retained and presentation frames; and
- the isolated `render/` pnpm workspace decodes those frames and owns retained projection, Three,
  WebGL, WebAudio, DOM presentation hosts, inspection, and editor viewport mechanisms.

Downstream code owns game/tool meaning and resource policy. It emits typed frames and supplies
explicit resource resolvers; it does not construct a second retained scene or Three backend.

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

## Exact-revision downstream consumption

An external pnpm consumer pins all four packages to one public 40-character Engine revision:

```json
{
  "dependencies": {
    "@rusty-engine/render-contracts": "github:FuzzySlipper/rusty-engine#<sha>&path:render/packages/render-contracts",
    "@rusty-engine/render-projection": "github:FuzzySlipper/rusty-engine#<sha>&path:render/packages/render-projection",
    "@rusty-engine/renderer-host": "github:FuzzySlipper/rusty-engine#<sha>&path:render/packages/renderer-host",
    "@rusty-engine/renderer-three": "github:FuzzySlipper/rusty-engine#<sha>&path:render/packages/renderer-three"
  }
}
```

The consumer must allow the corresponding four codeload package keys to run their `prepare`
scripts. Each package builds its checked `dist/` from the selected commit. Internal layers are
peer dependencies, which prevents hidden workspace copies from creating divergent contract or
projection identities.

Run the repository's clean temporary-consumer proof against any public Engine commit with:

```bash
./scripts/verify-render-consumer.sh <40-character-public-sha>
```

The script creates a new package outside this checkout, installs all four Git subdirectories,
rejects local/workspace/file resolutions, checks every lock entry against the requested SHA, then
decodes and applies one retained frame through contracts, neutral projection, the host fixture, and
Three. Engine revision `8cb49db6cfe9471faa23ab0661656a2366a83d8c` is the first recorded
successful public-package baseline.

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

## Reference consumer evidence

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

Studio is the second intended consumer. It uses the repository-local workspace while being built
in this repository, but it must import the same four public package surfaces and host/editor APIs;
it must not create an independent viewport renderer.

## CI topology

| Gate | Trigger and purpose |
|---|---|
| `verify / verify` | Every Engine push/PR; Rust-only provider and completeness proof with no Node installation. |
| `render / verify-render` | Render/Rust-contract paths; frozen isolated workspace plus real browser proof. |
| `render-consumer / verify-render-consumer` | Public `main` commits changing package/preparation surfaces; fresh exact-SHA Git-subdirectory install and execution. |
| `rusty-engine-demo / verify` | Every demo push/PR; fresh exact-revision dependency install and complete downstream product proof. |

The consumer gate is intentionally post-push: an exact public Git dependency cannot prove a commit
that has not yet been published. Pull requests still receive the local render gate; the public
package-preparation path is exercised immediately when the commit reaches `main`.

## Surface timing and telemetry

`RendererSurface` owns the browser animation loop and every explicit `renderOnce` submission, so it
is also the source of renderer timing observations. One immutable `RendererSurfaceTimingSample` is
produced after each successfully submitted frame:

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
prospective deadline is anchored when the enclosing timer query begins, rather than after the
synchronous submission returns; this avoids charging already enclosed work twice. Unknown renderers
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
polling is non-blocking. The single RAF remains the only render scheduler; during accelerated
demand, renderer-host may run one bounded readiness-probe burst between display callbacks. Those
probes advance available fence and timer-query observation but never render or request another RAF, are
cancelled on replacement, stop, reset, and disposal, and are not used for software or unknown
renderers. Unsupported, disjoint, malformed, or failed timer timing retains the completion-wall
policy instead of silently disabling pacing. `surface.automaticSubmissionPacing()` returns one
frozen renderer-owned diagnostic with the timer mode, current measurement state, renderer class
and allowance, latest completed decision's timer duration, observed completion age, effective
duration, selected duty, decision time, admission deadline, actual automatic admission
observation, selected and current admission limits, timer-query occupancy, and sync-fence
availability and occupancy. The same readout includes `hostAdmission`: exact lifetime outcome
counters plus the most recent 64 RAF admission attempts. Each attempt carries its source time,
request/resize/controls/presentation/retained-animation demand reasons, the
`admitted`/`backendBlocked`/`noDemand` outcome, and the backend's pre-submission capacity,
measurement, fence state, timer/effective duration, decision observation time, and deadline. The
bounded history makes sparse RAF delivery, missing owner demand,
and backend rejection distinguishable even when a consumer observes only occasional accepted
submissions. Reading it never polls, submits, mutates demand, or starts another loop. Explicit
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
- JavaScript-facing Rust integers are limited to `0..=2^53-1`; both Rust encoding and TypeScript
  decoding reject larger identities rather than silently losing precision.
- Shadow descriptors preserve intent, but the Three host reports explicit degradation when shadows
  are disabled or unsupported instead of pretending the requested result was realized.
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
