# Camera, crossover and retained-mesh followups

Tasks #7884, #7886 and #7887 follow the original baseline in the parent directory.

## Retained mesh application

`gpu-after.json` uses the identical version-2 fixture and isolated Firefox GPU
harness as `../gpu-reference.json`: 960×540 backing, pixel ratio 1, three runs,
30 warmup and 120 measured frames. Median of the three run medians, milliseconds:

| Workload | Before applyFrame | After applyFrame | Change |
| --- | ---: | ---: | ---: |
| 256 cubes | 25.18 | 16.78 | -33.4% |
| 8,192 triangles | 35.18 | 6.18 | -82.4% |
| 131,072 triangles | 654.50 | 79.64 | -87.8% |

The dense workload retains 66,049 vertices. Steady CPU submission remains 0.30 ms
and admitted frame intervals remain 17.06 ms. GPU timer queries were unavailable;
these numbers do not measure GPU execution, upload completion or streamed FPS.
Scene application excludes authored array construction and subsequent cold draw.

The surface and backend previously staged the same retained projection three
times. Each stage validated the payload, copied the static asset twice, and
copied the instance payload. The backend now realizes within one synchronous
projection transaction; only success commits that staged projection. Caller,
returned instruction and readback data remain detached. Static asset copies use
direct field/array copies, and instances share the immutable retained asset.

`mesh-stage-profile.json` is an instrumented local Node/no-GL diagnostic, **not**
a browser baseline. It compares transpiled source from `208e517` (including the
old surface preflight) with the final working source, with one warmup and three
runs. The old path made six complete static-asset structuredClone calls per
frame; none remain on the new path. Median total CPU time was 889.28 → 47.40 ms;
all structuredClone time was 835.82 → 23.80 ms, and final backend realization was
2.05 ms. Remaining generic copies, allocations, validation and timing overhead
are included in totals. Node timings must not be compared numerically to Firefox.
The separate browser measurement above confirms the user-facing application gain.

The candidate was captured on a dirty `208e517` checkout containing these fixes;
the raw artifact records that honestly. This is measured change evidence, not a
claim of a clean-revision release baseline. Neither fixture nor installed harness
configuration changed. Both owned GPU sessions were released. Final screenshot:
`/home/dev/dsh-crew/experiments/wolf-den-srv/controller/state/d6ada2fb-819f-4e6b-9660-c1d5e2e7814a/e195718e-f694-4ed0-b792-bbe74bbf9836.png`.

## Camera demand

Internal camera snapshots now retain exact position and angles for rendering,
listener synchronization, movement resolution and demand. The public diagnostic
pose remains rounded. The browser regression exercises sub-rounding position,
pitch and yaw changes (including the adjacent sine values at steps 51/52), then
verifies that repeating an identical pose does not demand another frame.

## Canonical CoreCLR and NativeAOT

`cpu-canonical.json` retains 27 records from three complete CPU wrapper runs:
Rust service staging, managed updates, both canonical loaders and HTTP paths,
plus the three DC workloads. The new crossover product publishes one numeric UI
value per demand update through the generated Engine API. Each loader receives
50 measured updates after eight warmups; HTTP is a separate 50-request series.
The same generated Release bundle carries both loader artifacts.

Median of three run medians/p95s, milliseconds:

| Lane | Loader | Median | p95 | Run median range |
| --- | --- | ---: | ---: | ---: |
| csharp-rust-crossover | coreclr | 0.002515 | 0.004528 | 0.002474–0.002544 |
| product-dev-host-http | coreclr | 0.258729 | 0.456725 | 0.241027–0.289889 |
| csharp-rust-crossover | nativeaot | 0.002234 | 0.004879 | 0.002124–0.002295 |
| product-dev-host-http | nativeaot | 0.262556 | 0.398808 | 0.253811–0.273058 |

The managed lane records .NET 10.0.11. These short warmed crossover samples are
not a claim about long-running tiered JIT behavior or game performance. The old
trial and this product perform different work; their timings must not be treated
as a before/after speedup. Loader, product/workload and configuration identity
separate them in the comparator.

This capture records dirty `e181167a`; the measured canonical source was then
committed as `ffbae22497eeb98570314340cb7cf69e10692136`. The renderer measurements
above correspond to `e181167a90a02a9fe6a640dd4fb8b5e316498c1d`. The environment
label explicitly identifies a shared development host. These are retained
observations, with no universal CI timing threshold installed.

Verification: 47 browser tests, compiled renderer tests (including transactional
failure and detached mesh ownership), reproducible renderer artifacts, boundary
checks, comparator tests, three canonical CPU captures, Rust formatting and
focused host clippy all passed. Renderer/studio/docs CI passed on `e181167a`.
