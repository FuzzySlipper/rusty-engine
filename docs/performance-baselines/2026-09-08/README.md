# Initial post-audit performance measurements

The provisional v1 diagnostic before/after records retain their actual dirty
revision marker. They compare identical browser, canvas and workload settings;
only diagnostic resource counting changed. Full retained snapshots were being
cloned just to count textures and atlases (#7885).

| Workload | Median frame interval before | After |
| --- | ---: | ---: |
| 256 retained cubes | 33.14 ms | 33.14 ms |
| 8,192-triangle relief | 33.14 ms | 33.14 ms |
| 131,072-triangle relief | 283.26 ms | 33.14 ms |

The old fixture requested camera motion from a separate observer callback,
limiting it to roughly every other display callback. Version 2 supplies demand
inside the Engine callback. Thus use these paired v1 records for the diagnostic
copy comparison only; do not compare their cadence to v2 as an Engine speedup.
The old Firefox clock also quantized small CPU times to zero.

The v2 GPU baseline uses Firefox through the existing Wolf service on den-srv,
1280×720/30fps streamed video, fixed 960×540 rendered canvas, and localhost
forwarding with COOP/COEP headers. Browser cadence and stream cadence differ.
Firefox reports a privacy-sanitized Radeon identity; GPU timer queries are
unavailable. Sub-millisecond CPU times are resolved with the isolated clock.
Captures were taken outside the final timed run. The hardware harness session
and temporary forwarder were released afterward.

CPU records measure the local release-mode Engine probes and DC meshing, not
the remote CPU. The crossover fixture is NativeAOT; CoreCLR is represented only
by the managed update microbenchmark until the packaged crossover followup.

These are initial references, not universal timing gates. Compare repeated runs
on matching environments, inspect spread, and investigate increases before
choosing a failure policy. Scene application timing includes CPU realization,
not cold GPU shader completion. Field construction is excluded from DC meshing.

Followups: #7884 separates exact camera demand from rounded diagnostics; #7886
adds the normal packaged CoreCLR crossover fixture; #7887 profiles dense mesh
realization. #7885 removes the confirmed diagnostic snapshot overhead now.

## Clean revision references

Both [CPU](cpu-reference.json) and [GPU](gpu-reference.json) captures identify
clean source revision `90c4e9717cd721dbbf98447b90de20e799f9f086`. Each row below is
the median of three run medians, in milliseconds. Raw samples and run spread
remain in the JSON artifacts; the old service probes retain their original
per-run summary records.

| CPU workload | Median ms |
| --- | ---: |
| Rust appearance staging with retained 8 MiB resource | 0.000301 |
| Managed update, 256 entities | 0.0028 |
| NativeAOT C#–Rust crossover | 0.010109 |
| Product host HTTP | 0.310598 |
| DC sculpted 32³ | 2.860818 |
| DC noisy/carved 40³ | 13.966754 |
| DC noisy/carved 56³ | 39.030978 |

| GPU browser workload | CPU submission | Frame interval | Diagnostic read | CPU scene application |
| --- | ---: | ---: | ---: | ---: |
| 256 retained cubes | 1.84 | 17.06 | 0.88 | 25.18 |
| 8,192-triangle relief | 0.30 | 17.06 | 0.44 | 35.18 |
| 131,072-triangle relief | 0.30 | 17.06 | 0.44 | 654.50 |

GPU timer duration is unavailable, not represented by these CPU columns. The
observed isolated Firefox clock quantum was approximately 0.02 ms. Repeated
CPU p95 measurements can vary appreciably at these small durations; the current
reference deliberately has no default failure percentage.

The final GPU capture completed at 10:08:20 UTC. Its lease expired at 10:08:53;
subsequent stop/status confirmed already released with no sessions or lobbies.
CPU reference sampling then ran without the Moonlight viewer. The earlier
successful loaded-scene observations are retained by the playtest service under
session `4aef4cd3-2b99-4431-be0b-c118622d6f7b`.
