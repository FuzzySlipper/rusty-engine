# Performance diagnostics

Rusty Engine keeps one checked performance probe that divides a frame-sized
operation into independently attributable layers:

- `rust-appearance-call-stage` measures beginning and ending a Rust graphics
  call with an 8 MiB retained renderer resource. A call owns the state for its
  duration and never copies it.
- `managed-csharp-update` measures a stable allocation-free managed update
  without Rust or browser work.
- `csharp-rust-crossover` measures generated CoreCLR and NativeAOT callbacks,
  the Engine service call, and output conversion.
- `product-host-http` adds the local product-host HTTP admission path.

Run the complete probe from the repository root:

```sh
./scripts/run-performance-regression.sh
```

Each result is printed as a single `RUSTY_PERF` JSON record. Override the
crossover sample count with `RUSTY_PERF_ITERATIONS` (1 through 256).

The crossover and HTTP lanes use the same canonical staged Product under both
CoreCLR and NativeAOT. `fixtures/csharp-crossover-performance` consumes the
public SDK staging targets with an explicit Engine contributor override. The
runner packs the current SDK, stages one Release bundle for both loaders
through `StageRustyEngineCombinedProduct`, and launches the current Rust host
with `--product`, each `--loader` and `--performance-probe`. This measures one
small demand-mode UI publication; it is not a full game or graphics workload.
Loader, runtime and workload identity keep the two lanes distinct.

The runner needs an existing runtime pack for its browser shell, at
`target/runtime-pack/linux-x64` (the `scripts/build-runtime-pack.sh` default)
or at `RUSTY_PERF_RUNTIME_PACK`. It copies that shell into a disposable layout
beside the current source host. The probe draws nothing and does not execute
browser JavaScript. No release pair is published.

## Interpreting regressions

Compare records from the same machine and build mode. There is intentionally
no universal millisecond gate: a CI runner and a developer workstation are
different performance classes.
Use the first lane that regressed to localize investigation:

1. Rust staging points to Engine call-state or resource ownership work.
2. Managed C# points to downstream-style update cost or managed allocation.
3. Crossover points to generated ABI, lifecycle, or Engine service commit cost.
4. HTTP points to product-host transport or synchronization.

## Renderer statistics

The probe is deliberately bounded and synthetic. On a running product started
with `--live-debug`, run `engine.renderer` in the live-debug panel or
`rusty-live-debug --origin ORIGIN --command engine.renderer`. It prints the
runtime renderer's adapter and output (`stream` or `window`), retained
operations skipped by kind, and, for streamed frames, the recent frame rate,
median render, readback and encode milliseconds, and bytes per frame and per
second. The desktop window streams nothing, so it reports no frame timing.
Where the adapter has timestamp queries, `gpu.passes` gives the median GPU
milliseconds of each renderer pass that is timed, with the adapter's
compute limits beside them: the ambient occlusion passes (`ao-prepass`,
`ao-occlusion`, `ao-blur`), then the first world view of a frame's `world`
pass, its `bloom-exposure` passes (with sun shafts) together and its
`finish` pass
([exposure, tone mapping and fog](lighting-and-sky.md#exposure-tone-mapping-and-fog)),
and a frame's share of building [the sky's
light](lighting-and-sky.md#the-skys-light) (`sky-light`).

To measure the renderer's frame cost without the product, take a scene
snapshot on the running product (`engine.renderer.snapshot <path>`, see
[presentation capture](presentation-capture.md#scene-snapshots)) and draw it
again on a fresh renderer:

```sh
rusty-scene-render scene.rscene out.png --width 1280 --height 720 --frames 600
```

It prints the adapter, the time to open and apply the snapshot, the mean
and median milliseconds of the extra frames, each with readback, and the
`gpu` pass medians. `--ambient-occlusion off|compute|raster` draws the
snapshot with that ambient occlusion path, to compare the paths on one
scene. Compare snapshots drawn on the same adapter; `WGPU_BACKEND` selects
it as for any wgpu program. A still camera reuses work a moving one repeats
(culled lists, a directional light's shadow cascades): add `--walk M` or
`--turn D` to move the snapshot's camera M metres forward or D degrees right
before each extra frame.

Trusted product code reads the same statistics (`ProductHostRendererStatistics`
in `product-host`) as UTF-8 JSON bytes through
`IEngineContext.Diagnostics.ReadRenderer()`. The runtime refreshes them once a
second; the generated binding copies the bytes before returning. Reading them
never draws a frame or synchronizes the GPU.

## Repeatable baseline artifacts

The baseline tools retain raw samples and repeat runs. Measurements are
report-only; increases are candidates for investigation, not statistical proof
of a regression. There is no machine-independent CI timing gate, and no
baseline artifact is currently recorded in the repository.

CPU/service/voxel lanes (release builds):

```sh
./scripts/run-cpu-performance-baseline.sh /tmp/engine-cpu-baseline workstation-release
```

This repeats the service/C#/HTTP probes three times and adds three runs each
of deterministic 32³ sculpted and 40³/56³ noisy/carved DC meshes. The voxel
timing covers `mesh_scalar_samples` only; constructing the scalar field is
outside the timed region. Output geometry counts and seeds are retained. It
does not measure gameplay generation, erosion, or collision construction.

Compare artifacts from the same execution environment and workload:

```sh
node scripts/performance-results.mjs compare BASELINE.json CANDIDATE.json
# Only after choosing a workload-appropriate policy:
node scripts/performance-results.mjs compare BASELINE.json CANDIDATE.json --fail-percent 20
```

The comparison reports median/p95 changes and spread between complete runs.
A different environment label, host CPU/OS/Node, runtime or workload
configuration, or a missing lane produces an incompatible result, not a pass.
Exit 2 means incomplete/incompatible; exit 1 means the optional regression
policy failed (or the command failed). Without a threshold, compatible measured
increases remain report-only, exit 0. Repeat suspicious runs on an otherwise
idle host before selecting a threshold.

Artifacts identify the capture host's CPU/OS/Node and Git revision/dirty
state. Use a stable explicit environment label for each machine and build
configuration.
