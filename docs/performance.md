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
- `product-dev-host-http` adds the local product-host HTTP admission path.

Run the complete probe from the repository root:

```sh
./scripts/run-performance-regression.sh
```

Each result is printed as a single `RUSTY_PERF` JSON record. Override the
crossover sample count with `RUSTY_PERF_ITERATIONS` (1 through 256).

## Interpreting regressions

Compare records from the same machine, build mode and GPU. There is
intentionally no universal millisecond gate: a CI software rasterizer and a
developer's GPU are different performance classes.
Use the first lane that regressed to localize investigation:

1. Rust staging points to Engine transaction or resource ownership work.
2. Managed C# points to downstream-style update cost or managed allocation.
3. Crossover points to generated ABI, lifecycle, or Engine service commit cost.
4. HTTP points to product-host transport or synchronization.

The probe is deliberately bounded and synthetic. A live product diagnostic is
available when the product host opts into its existing live-debug routes. Run
`engine.renderer` in the ordinary debug CLI or panel to print the runtime
renderer's statistics: its adapter, and for streamed frames the recent frame
rate, median render, readback and encode times, bytes per frame and per
second, and any skipped ops. Trusted product code reads the same statistics
(`ProductDevRendererStatistics` in `product-dev-host`: adapter, output, a
`stream` object while frames are streamed, and skipped ops by kind) as UTF-8
JSON bytes through `IEngineContext.Diagnostics.ReadRenderer()`, refreshed once a
second; the generated binding copies and releases the Engine byte lease before
returning. Reading it never draws a frame or synchronizes the GPU.

The renderer's own frame cost is read the same way, on the running product:
`engine.renderer` reports the median render, readback and encode milliseconds
of the recent streamed frames.

## Repeatable baseline artifacts

The baseline tools retain raw samples and repeat runs. Measurements are initially
report-only; increases are candidates for investigation, not statistical proof
of a regression. No new machine-independent CI timing gate is installed.

CPU/service/voxel lanes (release builds):

```sh
./scripts/run-cpu-performance-baseline.sh /tmp/engine-cpu-baseline workstation-release
```

This repeats the existing service/C#/HTTP probes three times and adds three
runs each of deterministic 32³ sculpted and 40³/56³ noisy/carved DC meshes. The
voxel timing covers `mesh_scalar_samples` only; constructing the scalar field
is outside the timed region. Output geometry counts and seeds are retained.
It does not measure gameplay generation, erosion, or collision construction.

Compare artifacts from the same execution environment and workload:

```sh
node scripts/performance-results.mjs compare BASELINE.json CANDIDATE.json
# Only after choosing a workload-appropriate policy:
node scripts/performance-results.mjs compare BASELINE.json CANDIDATE.json --fail-percent 20
```

The comparison reports median/p95 changes and spread between complete runs.
Different hardware/browser/backing size/workload configuration or missing lanes
produce an incompatible result, not a pass. Exit 2 means incomplete/incompatible;
exit 1 means the optional regression policy failed (or the command failed).
Without a threshold, compatible measured increases remain report-only, exit 0.
Repeat suspicious runs on an otherwise idle host before selecting a threshold.

Artifacts identify the capture host's CPU/OS/Node and Git revision/dirty state.
For remote browsers, this host is the collector, not the remote GPU machine;
use a stable explicit environment label and retain remote hardware/stream setup
with the evidence. GPU strings may be privacy-sanitized by Firefox. Do not
compare a software renderer or reduced-resolution run against a hardware run.


The crossover and HTTP lanes use the same canonical staged Product under both
CoreCLR and NativeAOT. `fixtures/csharp-crossover-performance` consumes the public
SDK staging targets with an explicit Engine contributor override. The runner
packs the current SDK, stages one Release bundle for both loaders through
`StageRustyEngineCombinedProduct`,
and launches the current Rust host with `--product` and each `--loader`. It does
not handwrite manifests or ABI glue. This measures one small demand-mode UI
publication; it is not a full game or graphics workload. Loader, runtime and
workload identity keep the two lanes distinct, including from the old trial.

An existing runtime browser shell is required at `target/runtime-pack/linux-x64`,
or set `RUSTY_PERF_RUNTIME_PACK` to its runtime-pack root. The runner copies its
browser shell into a disposable layout alongside the current source host; the
HTTP probe does not execute browser JavaScript. No release pair is published.

Measured camera/mesh followups and canonical crossover results are recorded in
[the followup report](performance-baselines/2026-09-08/followups/README.md).
