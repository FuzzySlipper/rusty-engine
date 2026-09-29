# Lane: runtime

**Tasks, in order:** #8753, #8772, #8745, #8770. **Start:** now. Hold #8770
until the main lane's #8736 fix is on main (see below).
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

## Tasks

- **#8753: a managed `NullReferenceException` crashes the CoreCLR host with
  SIGSEGV.**
  - Reproduce with the SDK smoke consumer
    (`RUSTY_ENGINE_SDK_TEST_KEEP_WORK=1`). Confirm the actual mechanism before
    fixing; the task lists the leads: signal hooks, Rust-created threads, and
    the signal masks the supervisor sets for its child.
  - #8736 (`eba944ce`) already turns an escaping managed exception into a
    fault-and-pause in process. Your job is the hardware-originated case:
    NRE, and SIGFPE for divide by zero.
  - Cover the direct host (`--exercise`) and the supervised runtime.
- **#8772: the supervisor cannot stop or replace a runtime that is still
  starting.**
  - `RuntimeProcess::wait_ready` in `csharp-product-runtime/src/supervisor.rs`
    ignores signals, stdin EOF and `replace-runtime` while it waits.
- **#8745 (narrowed; all Rust).** Delete:
  - the `runtime-session` recovery vocabulary;
  - `runtime-ui/src/channel.rs` and its JSON helpers;
  - the test-only `render-projection` surfaces (`entity.rs`, `debug.rs`,
    `model_preview.rs`, `RetainedNodeProjector`, `PresentationProjectorSet`).

  Then classify `engine-inspector` once #8794 (hygiene lane) has deleted
  Studio. Notes:
  - #8736 already deleted `begin_attach_call` and its projector resets;
    confirm and just record it.
  - Keep `StableHandleRegistry` in `render-projection/src/retained.rs`.
    `runtime_appearance.rs` (#8737) and `voxel.rs` use it.
- **#8770: stop publishing empty frames and unchanged UI projections on idle
  ticks.**
  - **Wait for the main lane's #8736 fix.** A review found that a fault rebind
    publishes a Binding without the retained UI projection, so the HUD goes
    blank while paused. The fix republishes the current projection under the
    fault binding in `csharp-product-runtime/src/lib.rs`, the publication path
    #8770 changes. Rebase on it, and keep the rule that a rebind or a fresh
    baseline always carries the current projection.
  - Measure with `docs/evidence/runtime-readout-8768/scripts/idle.sh`.

## Files

- **Owns:**
  - `csharp-product-runtime/src/main.rs`, `supervisor.rs`;
  - `runtime-session`, `runtime-ui`;
  - the test-only `render-projection` modules listed above;
  - `engine-inspector`.
- **Shared:** in `csharp-product-runtime/src/lib.rs`, the main lane edits
  `fault_after_call`. Keep your edits to other functions, or rebase over it.

## Evidence

Follow each task's acceptance. For #8753, add the null-dereference case to the
packaged smoke fixture.
