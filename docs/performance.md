# Performance diagnostics

Rusty Engine keeps one checked performance probe that divides a frame-sized
operation into independently attributable layers:

- `rust-appearance-call-stage` measures Rust service staging with an 8 MiB
  retained renderer resource. Resource bytes must be shared, not copied, when a
  transactional C# call begins.
- `managed-csharp-update` measures a stable allocation-free managed update
  without Rust or browser work.
- `csharp-rust-crossover` measures generated CoreCLR and NativeAOT callbacks,
  Engine service transaction, and output conversion.
- `product-dev-host-http` adds the local product-host HTTP admission path.
- `browser-renderer-submission` measures explicit submissions through the real
  renderer surface and reports draw/resource statistics, backing resolution,
  renderer identity, and pacing classification.

Run the complete probe from the repository root:

```sh
./scripts/run-performance-regression.sh
```

Each result is printed as a single `RUSTY_PERF` JSON record. Override the
crossover sample count with `RUSTY_PERF_ITERATIONS` (1 through 256). The browser
probe deliberately runs with the repository Playwright configuration, which
currently selects SwiftShader; its renderer and actual backing resolution are
part of the record rather than hidden environmental assumptions.

## Interpreting regressions

Compare records from the same machine, build mode, browser, and renderer. There
is intentionally no universal millisecond gate: a hardware browser, CI
software rasterizer, and developer laptop are different performance classes.
Use the first lane that regressed to localize investigation:

1. Rust staging points to Engine transaction or resource ownership work.
2. Managed C# points to downstream-style update cost or managed allocation.
3. Crossover points to generated ABI, lifecycle, or Engine service commit cost.
4. HTTP points to product-host transport or synchronization.
5. Renderer submission points to backend realization, scene complexity, GPU
   completion, or resolution/pacing policy.

The probe is deliberately bounded and synthetic. A live product diagnostic is
available when the product host opts into its existing live-debug routes. Run
`engine.renderer` in the ordinary debug CLI or Angular panel to print the
latest browser snapshot. The same snapshot is available to trusted product
code as UTF-8 JSON bytes through
`IEngineContext.Diagnostics.ReadRenderer()`; the generated binding copies and
releases the Engine byte lease before returning. It includes renderer/vendor
identity when available, CSS and backing resolution, accepted submission
cadence and CPU submission time, asynchronous pacing/fence observations,
draw/triangle/live-resource counts, realized texture encoded/decoded sizes,
sprite/material fallback counts, and voxel-specialized material count. Reading
it never submits a frame or synchronizes the GPU.

## Headless Chromium readback warnings

Chromium 149 with headless software rendering can log `GPU stall due to
ReadPixels` while presenting a WebGL canvas, even when application JavaScript
never reads pixels. In the #7798 probe, both CraftSurvive and a page containing
only a WebGL clear emitted it without screenshots or calls to instrumented
WebGL/canvas readback APIs. The clear-only headed Xvfb control did not emit it.
This isolates that observation to the browser's headless/software presentation
path; it does not establish an Engine synchronous readback or a hardware-browser
performance regression.

Keep the warning in report-only captures. For a new occurrence, compare an empty
WebGL control and instrument application readback calls before attributing it to
Ghost Plate capture. This finding is not a blanket warning suppression or a
claim that other readback stalls have the same source.

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

Loaded renderer lanes:

```sh
node render/scripts/run-performance-baseline.mjs --serve \
  --port 4191 --output /tmp/engine-gpu-baseline --environment den-srv-wolf-firefox
```

Open the printed page in the owned accelerated browser session. It measures
256 retained cubes and relief meshes with 8,192 and 131,072 triangles at a fixed
960×540 CSS canvas. Actual backing resolution is recorded; software pacing can
reduce it. Three runs each use 30 warmup and 120 measured automatic submissions.
Camera motion supplies demand inside the Engine callback, independently of measured progress. CPU submission time, diagnostic read cost, scene application cost, admitted-frame
intervals, and available asynchronous GPU timers are distinct measurements.
Absent GPU timing is not zero. The streamed video rate is not a GPU timer.

For sub-millisecond Firefox CPU measurements, reach the server through container
localhost forwarding. The server supplies COOP/COEP isolation headers; the
artifact records `secureContext`, `crossOriginIsolated`, and the observed clock
quantum. A zero timing below that quantum is unresolved, not free work. The
installed browser preferences are not changed.

The server saves raw browser records and `baseline.json`, then stops. The owner
must still stop its GPU harness session. Current harness entry point:
`/home/dev/crew-services/docs/playtest.md`; check `playtest status` before
acquiring the single slot and preserve another owner's lease. No harness
configuration is modified by the Engine runner. The ordinary Playwright probe
continues to measure empty-scene software submission overhead separately.

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
packs the current SDK, stages a Release bundle through `VerifyRustyEngineAot`,
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
