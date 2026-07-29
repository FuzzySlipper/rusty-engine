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
sample is constant work apart from the render itself. It performs no scene traversal, GPU readback,
query, fence, or synchronization in the frame loop. Resource identity is de-duplicated: two
instances sharing one retained geometry count as one geometry, while independently cloned animated
instance resources count independently.

An accepted retained mutation becomes visible in statistics on the next successful submission; a
rejected mutation changes neither renderer state nor the latest sample. Camera reset submits one
ordinary classified sample without changing resource counts. Stopping a surface does not alter its
latest sample. Disposing releases the backend resources, rejects future submissions, and leaves
already returned samples as immutable historical observations rather than rewriting them to zero.
A replacement `RendererSurface` starts a fresh sequence and never inherits samples or counters from
the disposed instance.

`RendererLiveTelemetryCollector.sampleSurface` takes renderer-owned counters from a complete
submission sample. Caller counters may still supply game/product values such as `entityCount`, but
cannot override draw, handle, geometry, material, texture, animation, or triangle observations.

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
