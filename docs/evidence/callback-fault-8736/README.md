# No callback transactions; an exception faults and pauses (#8736)

A product call is no longer a transaction. Engine services change their state
directly while the product runs, nothing is rolled back, and an exception that
escapes a callback faults the lifecycle instead of tainting and replacing the
runtime process.

## Removed

About 3,000 lines deleted and 1,050 added across Rust, the C# generators and
docs, including rewritten tests.

- **Per-call candidates.** Every service used to clone its state at the start of
  each call (graphics through a copy-on-write `Arc`, audio, video, camera,
  render output, UI streams with their latest JSON trees, voxel content, voxel
  scene presentation, implicit fields) and clone `PresentationWorld` when the
  call finished. A call now takes each state by move and hands it back.
- **Rollback and its commit protocol:**
  - `EngineServiceSet::discard_call`, `take_call`/`commit_call` (now one
    `finish_call`), and every bridge's `discard_call`;
  - `prefer_engine_call_error`;
  - the implicit-surfaces callback-error latch, the last service where a
    caught Engine error still failed the whole callback;
  - Dynamics' deferred world/body destroys and their "pending" checks;
  - the `complete_call(committed, terminal)` ABI export;
  - the generated C# `LeaseReleaseCoordinator`: owned-handle `Dispose` now
    releases the native resource immediately, and debug-state transitions
    apply as soon as the callback returns.
- **Taint and respawn:**
  - `CsharpProductRuntime::tainted`, `taint_after_callback` and the 12
    `require_not_tainted` guards;
  - `recover_voxel_presentation_outputs` / `recover_from_canonical`;
  - the timeline resync receipts;
  - the host's `request_incarnation_replacement`: a runtime error never ends
    the host process now.
- **Unused attach calls:** `begin_attach_call` and its ghost-plate rebase and
  projector resets. Fresh baselines come from `snapshot_outputs`.

## What replaces it

- `finish_call` settles the call's renderer work and always gives every
  service its state back, even if settling fails.
- **An escaping exception** (or an Engine failure while settling) keeps what
  the call did and publishes it. The runtime then:
  - logs the full exception with its stack trace;
  - moves the lifecycle to `Faulted`, so simulation stops with the product
    loaded;
  - sends renderers a fresh baseline, since a failure may have lost some of
    the call's renderer work.
- **`Resume` now accepts `Faulted`** and continues the same product.
  `Restart` resets it, as before. `ReportFault` from the product takes the
  same path.
- The supervisor restarts the runtime only when the process dies.

## Exercises

`scripts/moving-workload/` is an ordinary SDK product:
- it admits one generated mesh of about 8 MB (160,000 vertices);
- every update, it republishes a snapshot of 1,000 moving objects;
- with `BENCH_THROW_AT=N`, update N moves everything, opens and disposes a
  texture, then throws `InvalidOperationException`.

It is built against the SDK before and after this change and run on the
matching runtime pack.

**Exception, with a browser attached** (`scripts/exception-exercise.mjs`,
`results/exception-*.json`, both launched `--supervised` as under `rusty dev`,
throwing at update 240):

| | Before | After |
|---|---|---|
| Runtime | exited; the supervisor restarted it with fresh product state; it threw again at update 240 and paused | stayed up: `running → faulted` on the same instance |
| Browser | `degraded`, with 503s on reconnect | stayed `ready`, received a fresh baseline |
| Exception log | none visible to the page | `CSHARP_PRODUCT_CALL: … InvalidOperationException: bench exception at update 240` with the stack trace |
| Resume | 503, runtime unavailable | accepted; `faulted → running`; simulation step 241 → 420 in 3 s |
| Supervisor restarts | 2, then paused | 0 |

**Callback cost** (`scripts/bench.py`, `results/moving-workload-*.json`):
the host's update attribution over 908 callbacks, two 15-second runs each:

| | Callback p50 | Callback p95 | Post-callback (latest) |
|---|---|---|---|
| Before | 1.28–1.30 ms | 2.03 ms | 410–612 µs |
| After | 1.09–1.10 ms | 1.65 ms | 103–157 µs |

Mesh bodies were already shared (#8732), so the first-write copy was smaller
than the review estimated. The remaining callback time is the full-snapshot
publish of 1,000 objects, which #8737 owns.

**Caught refusals** still leave the callback running: the SDK smoke's
`CAUGHT_REFUSAL_CHECKS_PASSED`, now including implicit surfaces.

## Checks

- **Rust tests pass:**
  - csharp-engine-services 183;
  - csharp-product-runtime 43 + 26;
  - product-dev-host 43 + 29;
  - runtime-lifecycle and runtime-session.
- **Tests that asserted rollback or taint** were deleted or rewritten to the
  new contract:
  - an escaped update faults, keeps the product and resumes;
  - an exception after a voxel edit publishes the edit plus a baseline;
  - a failed debug command does not stop the product;
  - a destroy is final;
  - UI publication is visible immediately.
- Clippy is clean (workspace, excluding `renderer-webview-host`, which needs
  GTK) apart from the pre-existing #8757 lints.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes, including the
  host exercise's fault and restart checks.

## Limits

- **A hung callback is still unbounded.** Its deadline went with #8766.
- **The fault path always rebaselines renderers**, even when nothing was lost.
- **An exception inside `Create` still fails the load.** No product exists to
  pause.
- **Timeline completion** that throws reports a rejected ticket and faults;
  the product decides what to do on resume.
