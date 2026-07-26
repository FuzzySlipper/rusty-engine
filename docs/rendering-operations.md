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

## Reference consumer evidence

`rusty-engine-demo` commit `42f428b0ee3f47de94d4372f512978f587d729f7` consumes that exact
Engine baseline with no sibling override. Its private render-contracts and renderer-three packages
are deleted. The loading-bay product maps typed Rust facts into shared presentation descriptors and
uses the shared retained surface plus audio, billboard, particle, and telemetry hosts. Its full gate
proves voxel meshes/edits, collision, navigation, combat, doors, beacon presentation, camera sync,
reset/reopen cleanup, and real Chromium behavior.

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

An explicit caller receives the sample directly from `surface.renderOnce(timeMs)`. An auto-started
caller reads the latest sample through `surface.timing()` without starting or polling another render
loop. The existing overlay host accepts that value directly:

```ts
const timing = surface.timing();
telemetry.sampleSurface({
  sourceTick,
  timing,
  counters: { entityCount, drawCallCount },
}, timing.sourceTimeMs);
```

The older `telemetry.sample({ frameTimeMs, ... })` seam remains available for a non-surface owner
that already has a real cadence measurement. Downstream code must not use a placeholder zero or
reinterpret `frameTimeMs` as backend submission time. Timing is read-only presentation diagnostics;
it does not advance gameplay, camera authority, or animation scheduling.

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
