# Runtime timing and native profiles

Use [CoreCLR diagnostics](coreclr-diagnostics.md) for managed attachment,
EventPipe, counters, and dumps. This guide adds callback timing and
optimized Rust CPU sampling. These observations support investigation; they do
not change product scheduling or enable continuous stack collection.

## Read the product/runtime lane

Start the packaged product with `rusty dev --live-debug`. Its optional Engine
live-debug panel and `rusty-live-debug` CLI read the same bounded snapshot:

```bash
curl -fsS -X POST -H 'Content-Type: application/json' -d '{}' \
  http://127.0.0.1:PORT/__rusty/product/runtime/diagnostics/read > /tmp/before.json
jq '.telemetry' /tmp/before.json
```

`updateAttribution` retains up to 2,048 completed C# callback samples, with
p50/p95/max and the incarnation's slowest callback. Each sample identifies its
runtime instance/generation/control revision, simulation step, and admitted
step count. Realtime catch-up is one callback carrying several admitted steps;
callback frequency is not the fixed simulation frequency.

Realtime callbacks and direct demand/external calls retain their attribution.
`inFlightOperation` and its age observe a running callback without acquiring
the product lock. A new runtime incarnation starts a new distribution.

Runtime progress measures completed updates in the runtime process, not a
browser clock or simulation-step count. No samples, only one sample, or no recent
progress produces an explicit reason instead of a fabricated rate. A paused or
busy runtime leaves an aging last sample; diagnostics reads remain independent
of the callback lock.

| Fact | Meaning |
| --- | --- |
| `callbackDurationUs` | Elapsed C# callback, **including** native Engine services it calls. |
| Character/residency/scene service totals | Nested within the callback; do not add them to it. |
| `postCallbackDurationUs` | Rust staging, presentation reduction/output conversion, commit and completion after callback return. |

These are elapsed durations, not CPU profiles. Keep the runtime identity with a
capture; never correlate an old runtime's trace with a replacement's callback
statistics.

## Simulation and display cadence

The runtime host schedules realtime observations against absolute deadlines.
Callback, output conversion and delivery time consume the tick budget instead
of adding another full interval after each operation. After a missed deadline
the next observation is one interval after the late one finishes; lifecycle
admission still owns the fixed-step catch-up cap and dropped-step accounting.
Pausing resets the host schedule phase. Observation intervals round up to
avoid waking before an exact fixed-step boundary.

The runtime renderer draws independently of the C# callback: streamed frames
come from a render thread, and the desktop window draws every frame itself.
Configured product cameras hold the latest published pose by default. The
opt-in `CameraView.UpdateCameraSample` path samples translation or full camera
pose at render time; see [camera
composition](csharp-lifecycle.md#retained-camera-composition). This does not
introduce a variable-rate C# callback. Rust work invoked synchronously from an
update still consumes that update's budget, even if it uses worker threads
internally. Work that should finish later needs an explicit asynchronous
job/result boundary; waiting for it inside a fixed update keeps it on the
critical path. Fast GPU timing does not establish smooth camera delivery.
Compare the `engine.renderer` frame rate and render/readback/encode medians
(see [renderer statistics](performance.md#renderer-statistics)) with runtime
progress and callback cost. Do not infer a fixed simulation rate from the
frame rate or the number of batched C# callbacks.

## Optimized Linux native capture

Runtime packs build Rust with release optimization and `line-tables-only`
debug information. The executable contains unwind information and file/line
mappings; `symbols/` retains the matching debug companions and `build-info.txt`
with source revision, dirty-state indication, compiler and profile information.
Keep the entire matching pack with the profile. Ordinary product launch still
needs no source checkout. An investigator uses that revision's source for code
inspection. A dirty contributor pack is explicitly identified as such.

For native local-variable debugging, contributors can build a separate pack
with `CARGO_PROFILE_RELEASE_DEBUG=2 ./scripts/build-runtime-pack.sh --output NEW_DIR`.
This preserves optimization; some variables can still be optimized away.
See [Cargo debug profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#debug).

Use a standard Linux `perf` installation. `perf_event_paranoid=2` permits
sampling this user's process in user mode:

```bash
# Start a disposable investigation session. Enable JIT symbol export only here.
DOTNET_PerfMapEnabled=3 /path/to/runtime-pack/bin/rusty dev \
  --project /path/to/Game.csproj --runtime /path/to/runtime-pack --live-debug

# In another terminal, rediscover the current managed/native runtime process.
python3 /path/to/rusty-engine/scripts/find-coreclr-runtime.py \
  --project /path/to/Game.csproj > /tmp/runtime.json
managed_pid=$(jq -r .pid /tmp/runtime.json)

# Capture outside watched product sources. CPU samples exclude sleeping time.
mkdir -p /tmp/runtime-profile
cd /tmp/runtime-profile
perf record -F 500 -e cpu-clock:u --call-graph dwarf,4096 \
  -p "$managed_pid" -o perf.data -- sleep 8
perf report --stdio --no-children -g none --full-source-path \
  --sort dso,symbol,srcline -i perf.data > hotspots.txt
perf script --no-inline -G -i perf.data -F ip,sym,dso > leaf-addresses.txt
```

`--call-graph dwarf` supports optimized Rust which may omit frame pointers.
The bounded stack dump can truncate deep stacks; a named native leaf/source line
is useful without claiming every managed/native transition unwinds completely.
Use child/inclusive views deliberately: summing parent and leaf percentages
counts the same samples repeatedly. See [perf record](https://man7.org/linux/man-pages/man1/perf-record.1.html)
and [kernel perf permissions](https://docs.kernel.org/admin-guide/perf-security.html).

CoreCLR's [perf map export](https://learn.microsoft.com/en-us/dotnet/core/runtime-config/debugging-profiling#export-perf-maps-and-jit-dumps)
records managed code address ranges and names. Keep `/tmp/perf-PID.map` with the
raw data and rendered report. Export has overhead while code is compiled, so
keep it opt-in. `perf report` resolves Rust but can leave managed
`memfd:doublemapper` addresses unnamed; match those leaf PCs against the
exported map, and use an EventPipe trace for a directly named managed report.
Do not attribute unresolved JIT samples to Rust. Complete automatic mixed-stack
symbolization is not provided.

Record diagnostics immediately before and after sampling. `runtime.json` includes
`runtimeInstanceId`; compare it to the callback sample binding. Also retain the matching pack's `runtime-manifest.json`,
`symbols/build-info.txt`, product DLL/PDBs, tool versions, and exact commands.
Rediscover after any restart, as described in the managed guide.

CPU samples distinguish scheduled native/managed work from waits. Pair them with
`System.Runtime` CPU-time counters and the elapsed callback durations to
identify time that needs further investigation. Ordinary EventPipe sampled
thread time includes waits and does not provide Rust CPU stacks. This user-mode
recipe does not capture kernel stacks or off-CPU wait stacks.

[Native-aware .NET collection](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/dotnet-trace)
(`dotnet-trace collect-linux`) is an alternative where its tracefs access and
other platform requirements are met, not a dependency of this workflow.
NativeAOT remains a separate native-debugging lane.
