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
