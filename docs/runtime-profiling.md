# Runtime timing and native profiles

Use [CoreCLR diagnostics](coreclr-diagnostics.md) for managed attachment,
EventPipe, counters, and dumps. This guide adds worker-boundary timing and
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

Worker-hosted callbacks and direct demand/external calls retain their attribution.
The worker scheduler additionally publishes `workerUpdate`: worker PID, runtime
readout/counters, phase durations, and the shell-local age of that observation.
`inFlightOperation` also observes worker scheduler activity without acquiring the
product lock. Source replacement clears old samples and worker timings; a new
worker's first callback is the start of a new distribution. Intentional worker
retirement does not create an unexpected-EOF error. An unexpected exit still does.

Runtime progress measures completed observations in the receiving host, not a
browser clock or simulation-step count. No samples, only one sample, or no recent
progress produces an explicit reason instead of a fabricated rate. A paused or
busy worker leaves an aging last sample; diagnostics reads remain independent
of the callback lock.

| Fact | Meaning |
| --- | --- |
| `callbackDurationUs` | Elapsed C# callback, **including** native Engine services it calls. |
| Character/residency/scene service totals | Nested within the callback; do not add them to it. |
| `postCallbackDurationUs` | Rust staging, presentation reduction/output conversion, commit and completion after callback return. |
| Worker `operationDurationUs` | The whole scheduled operation, including input/lifecycle handling, callback and post-callback work. |
| Worker `outputConversionDurationUs` | Converting/validating output values for the worker envelope. |
| Worker `outputEncodeWriteDurationUs` | Encoding that output envelope and writing its channel frame. |
| Worker `inputQueueAgeUs` | Oldest drained input's wait in the worker mailbox; null when no input was drained. Shell input admission/queue observations remain separate. |
| `shellDeliveryIntervalUs` | Shell-local interval from receiving scheduler activity to receiving its completion telemetry. Overlaps worker work, encoding and delivery; **not network latency** or an additive phase. |
| Shell output decode / queue / publication | Local output conversion, wait in the bounded publication queue, and retained publication time. |

These are elapsed durations, not CPU profiles. None subtracts absolute timestamps
from different processes. Keep the runtime identity and readout with a capture;
never correlate an old worker's trace with a replacement's callback statistics.
A lost optional timing observation emits a degraded diagnostic and leaves the
last sample visibly aging; timing loss alone does not terminate the product.

## Optimized Linux native capture

Runtime packs now build Rust with release optimization and `line-tables-only`
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

Use a standard Linux `perf` installation. On the tested host,
`perf_event_paranoid=2` permits sampling this user's process in user mode:

```bash
# Start a disposable investigation session. Enable JIT symbol export only here.
DOTNET_PerfMapEnabled=1 /path/to/runtime-pack/bin/rusty dev \
  --project /path/to/Game.csproj --runtime /path/to/runtime-pack --live-debug

# In another terminal, rediscover the current managed/native product worker.
python3 /path/to/rusty-engine/scripts/find-coreclr-worker.py \
  --project /path/to/Game.csproj > /tmp/worker.json
managed_pid=$(jq -r .pid /tmp/worker.json)

# Capture outside watched product sources. CPU samples exclude sleeping time.
mkdir -p /tmp/runtime-profile
cd /tmp/runtime-profile
perf record -F 500 -e cpu-clock:u --call-graph dwarf,4096 \
  -p "$managed_pid" -o perf.data -- sleep 8
perf inject --jit -i perf.data -o perf-jit.data
perf report --stdio --no-children -g none --full-source-path \
  --sort dso,symbol,srcline -i perf-jit.data > hotspots.txt
```

`--call-graph dwarf` supports optimized Rust which may omit frame pointers.
The bounded stack dump can truncate deep stacks; a named native leaf/source line
is useful without claiming every managed/native transition unwinds completely.
Use child/inclusive views deliberately: summing parent and leaf percentages
counts the same samples repeatedly. See [perf record](https://man7.org/linux/man-pages/man1/perf-record.1.html)
and [kernel perf permissions](https://docs.kernel.org/admin-guide/perf-security.html).

CoreCLR's [perf map/JIT dump export](https://learn.microsoft.com/en-us/dotnet/core/runtime-config/debugging-profiling#export-perf-maps-and-jit-dumps)
provides managed code names to native tools. Keep the `/tmp/perf-PID.map` and
`/tmp/jit-PID.dump`, injected `jitted-*.so` files, raw/injected data, and rendered
report with the capture. JIT export has overhead while code is compiled, so keep
it opt-in. An unresolved JIT address is not evidence of expensive Rust; consult
the managed EventPipe capture for method attribution when native resolution is
incomplete.

Record diagnostics immediately before and after sampling. `worker.json` includes
`runtimeInstanceId`; compare it to `workerUpdate.readout.runtime.instanceId` and
the callback sample binding. Also retain the matching pack's `runtime-manifest.json`,
`symbols/build-info.txt`, product DLL/PDBs, tool versions, and exact commands.
Rediscover after any restart, as described in the managed guide.

CPU samples distinguish scheduled native/managed work from waits. Pair them with
`System.Runtime` CPU-time counters and the elapsed callback/worker phases to
identify time that needs further investigation. Ordinary EventPipe sampled
thread time includes waits and does not provide Rust CPU stacks. This user-mode
recipe does not claim kernel stacks or off-CPU wait-stack attribution.

On the tested host, .NET `collect-linux` could find a process but could not access
tracefs; samply's recording mode also declined the current perf policy. The
explicit user-mode `perf` capture worked without changing host permissions.
[Native-aware .NET collection](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/dotnet-trace)
is an alternative where its platform requirements are met, not a dependency of
this workflow. NativeAOT remains a separate native-debugging lane.
