# Runtime-owned browser I/O (#8740)

## Question

Should the CoreCLR runtime process own browser I/O, replacing the foreground
shell's relay (worker pipe → decode → validate → re-encode → SSE) and the
replay machinery that exists to bridge the two processes? Worker ownership,
WebSocket and lock-free queues were hypotheses, not requirements.

## Setup

- Engine `main` at `d2573502` (includes #8734 compact bytes/cacheable
  resources, #8746 in-place rebinds, #8750, #8751), release runtime pack
  from `scripts/build-runtime-pack.sh`.
- Product: the packaged SDK fixture (`fixture.sdk-package`, realtime 60 Hz,
  `key:key-w:held` mapping) staged by `test-csharp-sdk-package.sh`.
- No browser: `scripts/probe.py` speaks SSE and input POSTs directly.
- One shared Linux machine running other agents' hosts, so treat small
  differences as noise.

Topologies (`scripts/start.sh`):

| Name | Processes | Browser connects to |
|---|---|---|
| `worker` (current) | shell + CoreCLR worker | shell, which relays to the worker |
| `owned` (prototype) | supervisor + CoreCLR runtime | runtime, on the supervisor's listener |
| `inprocess` (reference) | one | the single process; **no signal isolation** (#8686) |

The prototype is on branch `experiment/8740-worker-owned-io` (`e9802516`).
It is about 150 lines:
- a supervisor mode (`--worker-owned-io`) that binds the listener, clears
  close-on-exec on it, and spawns the ordinary in-process host in its own
  process group with `--serve-listener-fd`;
- stdin close stops the runtime;
- a trigger file replaces the runtime;
- `ProductDevHostConfig::with_listener`;
- one reconnect rule: a `Last-Event-ID` beyond this process's history gets
  the lag response, so the browser reconnects fresh.

## Results

Input: 150–200 alternating key presses about every 30 ms, timed from POST
send to the SSE `runtime-input-result` whose `consumedThrough` covers it
(`scripts/latency.py`). CPU: process-tree ticks over 10 s with one subscriber
attached (`scripts/cpu.sh`).

| | worker (current) | owned (prototype) | inprocess |
|---|---|---|---|
| POST round trip p50 / p95 | 0.47 / 0.62 ms | 0.35 / 0.50 ms | 0.35 / 0.48 ms |
| input → consumed result p50 / p95 / max | 10.2 / 16.4 / 17.4 ms | 10.2 / 16.3 / 17.1 ms | 10.0 / 15.5 / 16.9 ms |
| idle CPU, 1 subscriber | 2.2% | 1.6% | 1.1% |
| idle SSE traffic | 41 KB/s | 67 KB/s | 65 KB/s |

End-to-end input latency is bounded by the 16.7 ms tick; the topology changes
it by less than 0.2 ms. The in-process host publishes a `runtime-readout`
every tick, while the worker path sends that telemetry to the shell
instead. That accounts for the higher idle traffic in the last two columns
(#8768).

The cost of the relay is per output byte. `scripts/relay_bench.rs` encodes N
moving-object transform updates per tick. The in-process path runs one SSE
encode. The relay path runs the worker's typed → `Value` → bytes encode and
then the shell's bytes → `Value` → typed → SSE encode. Release build, mean of
60 ticks:

| moving objects | wire bytes | in-process encode | relay (worker encode + shell decode/re-encode) |
|---|---|---|---|
| 100 | 21 KB | 0.02 ms | 0.17 + 0.28 = 0.45 ms |
| 1,000 | 212 KB | 0.21 ms | 1.66 + 2.67 = 4.3 ms |
| 10,000 | 2.1 MB | 3.5 ms | 19.7 + 29.5 = 49 ms |

At 1,000 moving objects and 60 Hz, the relay alone is about a quarter of a
core. At 10,000, it cannot keep up with the tick at all.

## Exercises (prototype)

- **Reload / reconnect:** every probe attach to `/outputs/fresh` receives a
  complete baseline.
- **Runtime replacement** (`scripts/replace.py`):
  - the new runtime was ready 732 ms after the trigger, and the old stream
    closed at 740 ms;
  - an EventSource-style resume with the old cursor gets `rusty-output-lag`;
  - `/outputs/fresh` then delivers the new incarnation's baseline at 743 ms.
  - The browser-visible gap is runtime startup; no history crossed the
    replacement.
- **Stalled subscriber** (`scripts/slow.py`):
  - a client that stops reading for 6.4 s leaves another client's latency
    unchanged (p50 9.6 ms);
  - the kernel buffered about 450 KB, and the history ring (about 4 s at
    60 events/s) would lag it out.
- **Terminal signal:** SIGINT to the supervisor's process group exits
  everything in 60 ms. The runtime, in its own group, shut down through
  stdin close (`reason: supervisor-stdin-closed`) with exit status 0. This
  keeps the #8686 signal isolation.

## Decision

**Adopt runtime-owned browser I/O for CoreCLR.** It is the simplest
topology that keeps signal isolation:
- the supervisor shrinks to binding, spawning, replacing and signalling;
- the relay and its bridging machinery go away;
- per-byte output cost drops 12–20×.

**Not adopted:**
- **WebSocket and binary framing:** SSE plus POST performed identically once
  the hop was gone. The measured cost is the relay's decode and re-encode, not
  SSE text, and bulk bodies already use content-addressed cacheable routes
  (#8734).
- **A lock-free input queue:** the existing mailbox answers a POST in
  0.35 ms, and the tick dominates.

## Add-backs this topology needs

- **Answer while no runtime is serving.** During startup, or after a failed
  runtime pauses, connections otherwise wait in the listen backlog.
  Smallest remedy: while no runtime is ready, the supervisor accepts and
  returns 503 (and the static shell). This is not in the prototype.
- **Remove the per-tick `runtime-readout`** from the in-process host
  scheduler: publish it on change.
- The existing `ReplaceRuntime` stdin contract, the one automatic restart
  and headless launch move to the supervisor unchanged.

## Deletion inventory after adoption

- `csharp-product-runtime/src/main.rs`: the `WorkerRuntime` shell adapter,
  worker reader and relay, `run_worker`, `worker_scheduler`, and the
  worker request, receipt and feedback relays. That is about 2,200 of
  about 4,070 non-test lines.
- `product-dev-host`:
  - `worker.rs` (383 non-test lines) and `diagnostic_relay.rs` (141);
  - the host's worker configuration and hooks (`worker_outputs`,
    `worker_generation`, `initial_worker_outputs`, `worker_failures`,
    `worker_diagnostics`, `worker_owns_scheduler`,
    `disposable_worker_runtime`);
  - `replace_worker_projection`, the `ConnectionBoundary` acknowledgement,
    and private and pending baselines.
- Then, with one process owning the bus, the replay machinery can go:
  - `Last-Event-ID` resume and the lag event: every reconnect gets a fresh
    baseline;
  - the history ring, replaced by per-subscriber live queues;
  - fragments and transfer ids, in both Rust and `local-transport.ts` (lift
    the browser event bound);
  - retired-resource retention for the history window.

## Implementation handoff

- **#8766:** adopt runtime-owned browser I/O for CoreCLR (the supervisor,
  the 503 add-back) and delete the shell relay.
- **#8767:** once one process owns the bus, delete the replay machinery
  (every reconnect gets a fresh baseline).
- **#8768:** stop the per-tick `runtime-readout` and idle `runtime-progress`
  events.

## Reproducing

Build a runtime pack from the experiment branch for `owned`. Stage a Product
(for example with `RUSTY_ENGINE_SDK_TEST_KEEP_WORK=1
scripts/test-csharp-sdk-package.sh --coreclr-smoke`). Then:

```sh
RUSTY_PACK_BIN=… RUSTY_PRODUCT=… RUSTY_WORK=… scripts/start.sh worker &
python3 scripts/latency.py 40821 200 scripts   # port from product.json
scripts/cpu.sh 40821
python3 scripts/replace.py 40821 "$RUSTY_WORK/replace" scripts   # owned only
python3 scripts/slow.py 40821 scripts
```

To run `relay_bench.rs`, include it from the `product-dev-host` host tests
module (`#[path = ".../relay_bench.rs"] mod relay_bench;`) and run
`cargo test --release -p product-dev-host --lib bench_8740 -- --ignored --nocapture`.

## Limits

- This is one small fixture on a shared machine, with no browser-visible
  exercise.
- Replacement was measured on the prototype only; the current shell's
  replacement was not timed.
- NativeAOT already runs in-process and is unaffected.
