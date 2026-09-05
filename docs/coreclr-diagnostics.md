# CoreCLR profiling and managed debugging

The packaged Rust host loads ordinary CoreCLR. Standard .NET diagnostics attach
to its **product worker**, whose executable is named `rusty-product-host`, not
`dotnet`. The `rusty dev` process and its browser-facing supervisor do not run
product C#. A browser on another LAN machine does not change this: run the
managed tools on the Linux runtime machine, as the user running the product.

## Find the current worker

Engine contributors can resolve a selected product without guessing among
native-named processes:

```bash
python3 /path/to/rusty-engine/scripts/find-coreclr-worker.py \
  --project /path/to/Game.csproj > /tmp/game-worker.json
managed_pid=$(jq -r .pid /tmp/game-worker.json)
cat /tmp/game-worker.json
```

The Linux helper checks the live worker arguments, loaded CoreCLR, and default
Unix diagnostic socket. It returns `pid`, `parentPid`, `productDirectory`, and
`diagnosticPort`. Use `--product /exact/staged/Product` for custom staging, or
add `--pid N` to select among multiple matching sessions. No match or ambiguity
is an error. It reads live processes, not persisted den-serve records.

Rediscover after every source restage, worker recovery, `rusty dev` restart,
or den-serve restart. Both PID and socket can change. The helper is optional
contributor tooling, not a Python dependency of the SDK/runtime pack.
`dotnet-trace ps` is also useful, but may fail while enumerating unrelated
processes; explicit worker selection avoids that enumeration path.

CoreCLR's [diagnostic port](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/diagnostic-port)
is normally `${TMPDIR:-/tmp}/dotnet-diagnostic-PID-STARTTIME-socket` on Linux.
A missing socket can mean startup is incomplete, another user owns the process,
or diagnostics were disabled with `DOTNET_EnableDiagnostics=0`.

## Capture managed work and allocation

Install the ordinary Microsoft tools where convenient; no product changes are
needed. For example, install `dotnet-trace`, `dotnet-counters`, and `dotnet-dump`
with `dotnet tool install --tool-path /tmp/dotnet-tools TOOL_NAME`. Add that
directory to the shell's PATH. Keep captures outside watched product sources.

With .NET 10 diagnostics tools:

```bash
dotnet-trace collect --process-id "$managed_pid" \
  --profile dotnet-sampled-thread-time,gc-verbose --duration 00:00:12 \
  --output /tmp/game.nettrace
dotnet-trace report /tmp/game.nettrace topN -n 15
dotnet-trace convert /tmp/game.nettrace --format Speedscope --output /tmp/game

dotnet-counters collect --process-id "$managed_pid" --counters System.Runtime \
  --refresh-interval 1 --duration 00:00:12 --format json \
  --output /tmp/game-counters.json
```

The trace resolves product and generated callback methods. Sampled managed
thread time includes waits; its percentages are **not CPU utilization**. Use
`dotnet.process.cpu.time` counters alongside allocation, collection, heap, and
GC pause counters. Rates and per-generation collection counts must be interpreted
with their reported units. These captures help separate expensive C# callbacks
from allocation pressure; they do not attribute native Rust or browser GPU time.
For actual native/kernel CPU stacks, use a native profiler or the separately
supported `dotnet-trace collect-linux` route and its OS requirements.

See Microsoft's [trace guide](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/dotnet-trace)
and [counter guide](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/dotnet-counters).

## Break in a product callback

Start an explicitly debuggable session:

```bash
/path/to/runtime-pack/bin/rusty dev --project /path/to/Game.csproj \
  --runtime /path/to/runtime-pack --debugger
```

`--debugger` removes supervised worker startup and callback deadlines for this
CoreCLR session. Normal sessions retain their five-second deadlines and recovery.
Channel failures and shell protocol validation remain active. A paused or hung
callback can wait indefinitely in this mode; continue, detach, or stop the session
when finished. Source restaging still replaces workers, so avoid editing while
inspecting a paused callback and rediscover before attaching again. `--live-debug`
controls the Engine browser diagnostic console; it does not enable managed
breakpoints.

Build the product in Debug with portable PDBs and matching source. For reliable
local inspection, set `<Optimize>false</Optimize>` and
`<DebugType>portable</DebugType>` in the ordinary product project. Embedded source
(`<EmbedAllSources>true</EmbedAllSources>`) is optional. Keep the product DLL and
PDB together in staged `coreclr/` output; do not replace generated entrypoints.

[Samsung netcoredbg](https://github.com/Samsung/netcoredbg) is a working Linux
managed debugger for this host. It supports CLI and standard Debug Adapter
Protocol (`--interpreter=vscode`) clients. Run it from outside the watched
product tree so its history/log files do not trigger restaging:

```bash
cd /tmp
/path/to/netcoredbg --interpreter=cli --attach "$managed_pid"
```

Its [CLI commands](https://github.com/Samsung/netcoredbg/blob/master/docs/cli.md)
include `break /absolute/path/Product.cs:18`, `continue`, `backtrace`,
`print callbackNumber`, `delete 1`, `detach`, and `quit`. Choose an executable
line and a local from your actual product. In a DAP client, set a source
breakpoint, continue to the callback, inspect locals, then continue and detach.
Debugger expression support varies; a simple local read is the reliable starting
point. Prefer unoptimized code; a Release local may have been eliminated.

For an SSH workflow, open the product folder on the runtime machine using a
remote editor and run its managed debugger adapter there. A C# extension's
[CoreCLR attach configuration](https://code.visualstudio.com/docs/csharp/debugging)
uses `"type": "coreclr"`, `"request": "attach"`, and the discovered worker PID
as `"processId"`. Select the native-named worker explicitly. Source paths must
match its PDB, or be mapped by the debugger. The editor UI can be on Windows;
there is no need to expose a debugger port through the LAN browser host. The
Engine proof used netcoredbg's DAP adapter over local stdio, not a VS Code UI
certification.

### Suspend before managed startup

For startup EventPipe collection, stage first, then set
`DOTNET_DefaultDiagnosticPortSuspend=1` **only on the host launch**. Setting it
on `rusty dev` also affects the `dotnet` build commands it starts.

```bash
dotnet msbuild /path/to/Game.csproj -restore -t:StageRustyEngineCoreClrProduct
# Obtain the staged directory with -getProperty:RustyEngineStagedProductDirectory.
DOTNET_DefaultDiagnosticPortSuspend=1 /path/to/runtime-pack/bin/rusty-product-host \
  --product /path/to/staged/Product --loader coreclr --supervised \
  --runtime-instance-id 1 --debugger
```

Keep that terminal's stdin open. In another SSH terminal, discover the worker
and start `dotnet-trace collect --process-id "$managed_pid" ...`; its normal
resume behavior releases diagnostic startup suspension. The browser listener
becomes available after managed startup finishes. Diagnostic startup suspension
is distinct from a source debugger breakpoint.

## Dumps, native frames, and NativeAOT

A bounded managed stack capture uses the same worker PID:

```bash
dotnet-dump collect --process-id "$managed_pid" --type Mini --output /tmp/game.dmp
dotnet-dump analyze /tmp/game.dmp -c 'clrstack -all' -c exit
```

A mini dump is useful for stacks, not complete heap analysis. Collection briefly
suspends the runtime; use `--debugger` if investigation may exceed normal callback
deadlines. See the [managed dump guide](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/debug-linux-dumps).
For Rust stacks and native/mixed investigation, see [runtime profiling](runtime-profiling.md)
for the optimized symbol pack and user-mode Linux CPU capture. Use GDB/LLDB for
native debugger inspection. `dotnet-dump` is not a native debugger.

NativeAOT is a separate fidelity/release lane. It has no CoreCLR worker socket or
ordinary managed-debugger attach parity. Use the published native debug symbols
and native tools; Microsoft's [NativeAOT diagnostics guide](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/diagnostics)
describes its different support. No profiler runs continuously by default.
