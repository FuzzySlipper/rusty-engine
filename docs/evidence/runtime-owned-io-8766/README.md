# Runtime-owned browser I/O for CoreCLR (#8766)

Adopts the #8740 decision: the CoreCLR runtime process serves the browser
directly, and the shell relay is deleted.

## Topology

Packaged CoreCLR launches (`rusty dev`, `--headless`, and a direct
`rusty-product-host --product … --loader coreclr`) now run two processes:

- **Supervisor** (`csharp-product-runtime/src/supervisor.rs`):
  - binds the product listener and keeps terminal signals;
  - starts the runtime with `--serve-listener-fd N`, in its own process
    group, clearing close-on-exec on that one descriptor;
  - owns the `rusty dev` `replace-runtime` stdin contract, one automatic
    restart after a runtime crash, the failure pause and headless launch.
- **Runtime:** the ordinary in-process host (CoreCLR, Engine, product,
  HTTP/SSE) on the inherited listener. It starts the product at load, as the
  worker did, so browserless flows keep running. It prints `RUSTY_RUNTIME
  ready`, then waits for `serve` on stdin before it accepts. Stdin EOF is the
  clean stop, and it disposes the product.

**Replacement** stops the old runtime first, bounded at 10 s and then killed,
and only then starts the next. Two incarnations never share persistence.

**Add-backs, each with its observed need:**

- **503 while no runtime serves.** Without it, a browser waits in the listen
  backlog during startup and indefinitely while paused. The supervisor
  answers JSON for runtime routes and a refreshing page for navigations,
  carrying the pause reason. It stops answering before sending `serve`, so
  the two processes never accept at once.
- **Browser fresh-stream retry.** `EventSource` does not retry an HTTP 503.
  An open page that reconnected during a replacement stayed `degraded`
  forever (observed on dagger). `local-transport.ts` now reopens a fresh
  stream that closed before its baseline, with backoff from 250 ms to 2 s.
- **Starting the product at load in the runtime.** The in-process host
  previously started it on the first browser connect. Without this, the
  #8686 group-signal proof's fixture never updated.
- **Non-panicking stdout in the runtime.** A runtime orphaned by a SIGKILLed
  supervisor panicked on `println!` into the broken pipe instead of
  disposing its product.

**Deliberately not kept:** the worker's 5-second callback deadline. A stuck
callback shows `inFlightOperation` and its age in live-debug telemetry, and
the next restage replaces the runtime. `--debugger` now only lifts the
30-second startup deadline.

## Deleted

About 5,850 Rust lines removed and 860 added, including the 649-line
supervisor with its tests:

- **`csharp-product-runtime/src/main.rs`** (−3,804):
  - the `WorkerRuntime` proxy and worker reader/relay;
  - `run_worker`, `worker_scheduler`, and the request/receipt/feedback relays;
  - the prototype `--worker-owned-io` mode;
  - `--worker` and `--worker-channel`.
- **product-dev-host:**
  - `worker.rs` (−524) and `diagnostic_relay.rs` (−272);
  - the host's worker configuration and threads;
  - `replace_worker_projection`, the projection gate and epoch, and the
    `ConnectionBoundary` cursor;
  - the worker update telemetry;
  - the worker output and resource encodings (`to_worker_value` and base64
    bodies).
- **`runtime-session`:** `PreparedRuntimeReplacement`.
- **`runtime-diagnostics`:** `RuntimeWorkerPhases` and
  `RuntimeOperationActivity`.
- **TypeScript:** `workerUpdate` in the live-debug client (−145) and in the
  Studio panel (−38). The `local-transport.ts` diff is mostly the stream
  setup moved into `openFreshStream`.
- **Script:** `scripts/find-coreclr-worker.py` becomes `find-coreclr-runtime.py`
  and matches `--serve-listener-fd`.

## Results

**Signals** (`results/group-signal.json`; the #8686 `group-signal-proof.py`
on the packaged voxel fixture, fresh session, `killpg`). Direct and
supervised launches, with SIGINT and SIGTERM, all:

- exit 0 with exactly one `DISPOSED`, in 0.064 s;
- run the runtime in its own process group, which is reaped.

**Supervisor paths** (`scripts/supervisor_exercise.py`,
`results/supervisor-exercise.json`, packaged SDK fixture):

| Path | Outcome |
|---|---|
| `replace-runtime` frame (as `rusty dev` writes it) | new incarnation in 0.75 s; old runtime reaped; 33 of 35 requests in the window got 503, none failed |
| Runtime SIGKILL | automatic restart, new incarnation in 0.75 s |
| Second SIGKILL | paused: 503 JSON `DEV_HOST_RUNTIME_UNAVAILABLE` with the reason, and a refreshing page for navigation |
| Restage while paused | recovered in 0.73 s |
| Supervisor stdin close | exit 0 in 0.064 s, runtime reaped |
| Direct launch, runtime SIGKILL | exit 1, `DEV_HOST_RUNTIME_EXIT … stopping the host` |

**Visible exercise: rusty-dagger under `rusty dev --engine-source --live-debug`**
(`scripts/exercise-dagger.mjs`, `results/dagger-exercise.json`):

- **Input:** remapping `move.forward` W → K through the Controls UI reached
  the product (12 input POSTs, in-place rebind).
- **Reload:** it reattached to the same incarnation, and the world continued.
- **Runtime replacement by source restage, with the page left open:**
  - `rusty dev` rebuilt for about 12.6 s, then replaced the runtime;
  - the page's fresh stream got 503 at 12.65, 12.90, 13.40, 14.40 and
    16.41 s, then 200 at 18.43 s;
  - it returned to `ready` on the new incarnation (…405 → …406) with the
    same canvas and a live WebGL context;
  - no page errors.
- **Input after replacement:** remapping back to W reached the new runtime
  (12 more POSTs), and `playtest.observe` reported `["KeyW"]`.
- **Warning capture:** 0 Engine events. The two browser warnings are the known
  dagger inventory art and the Chromium ReadPixels stall (as in #8765 and
  #8768). There is no baseline, so no clean delta is claimed.

**Idle, SDK fixture, one subscriber** (`results/idle-*.txt`, #8768's
`idle.sh`): process-tree CPU 2.2% → 1.4% of one core; SSE 21.0 → 20.7 KB/s.
The remaining traffic is #8770.

**Moving objects:** the in-process SSE encode (the #8740 bench's direct half,
rerun on this code, release build, mean of 60 ticks):

| moving objects | before: relay (worker encode + shell decode/re-encode, #8740) | after: direct |
|---|---|---|
| 100 | 0.45 ms | 0.025 ms |
| 1,000 | 4.3 ms | 0.19 ms |
| 10,000 | 49 ms | 3.4 ms |

No product-level moving-object workload exists to run through the real host.
This is the encode cost only.

**Other checks:**
- Rust: product-dev-host (49 + 29), csharp-product-runtime (44 + 26,
  including the supervisor frame, 503 and lock tests), runtime-session,
  runtime-diagnostics, rusty-cli.
- Clippy is clean except the pre-existing `collapsible_match` (#8757).
- `pnpm test`: product-browser-host 108 (with a fresh-stream reopen test),
  live-debug-client 5, live-debug-panel 4 + artifact 3. The artifact test's
  telemetry fixture now includes `runtimeProgressUnavailableReason` and
  `updateAttribution`: the panel template reads them, and the progress line
  did not render without them.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes. Its host step
  is the in-process `--exercise` path; the supervisor paths are covered
  above.
- Doc links pass.

## Limits

- **Replacement adds startup time as a 503 gap.** Unlike the relay, the page
  does not keep one SSE connection across a replacement. The fresh-stream
  backoff can add up to 2 s after the runtime is ready.
- **Hung callbacks are no longer detected** automatically (see above).
- **The Unix supervisor is the only one.** On other platforms a packaged
  CoreCLR launch runs in process; the runtime pack targets linux-x64.
- **The replay machinery is untouched:** history ring, `Last-Event-ID`,
  fragments. It is #8767.
