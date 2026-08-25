# Runtime lifecycle

## Ownership

`rust/crates/runtime-lifecycle` owns one instance-owned, host-neutral runtime
lifecycle selected from a validated `ProductManifest`. It admits simulation and
presentation work as explicit data plans, carries runtime-generation and
control-revision correlation tokens, and reports bounded clock/catch-up facts.

It does not own product orchestration, an executor, a scheduler, callbacks,
closures, input capture, schedule execution, timeline meaning, mutation,
projection implementation, rendering, storage, a DOM/browser/Tauri host, or a
clock source. A downstream product supplies its monotonic time and invokes its
own named owners using these plans. Token validation is idempotent correlation
evidence, not completion, consumption, or mutation authority.

The sole Product Model linkage is
`RuntimeLifecycle::from_product_manifest(instance_id, &ProductManifest)`: it
selects the already-validated `realtime`, `demand`, or `external` mode and
reuses Product Model's 240 Hz / 16-step realtime bounds. The caller supplies a
distinct `RuntimeInstanceId` for each simultaneously live owner; every
simulation/presentation/phase token, lifecycle receipt, and readout carries
that identity, preventing equal generation/revision values from a second
explicit instance from cross-validating. The lifecycle never loads a manifest
or interprets another product field.

## Modes and lifecycle facts

- **Realtime** accepts only a caller-supplied `HostMonotonicTime`. Its exact
  scaled accumulator is `delta_ns * fixed_step_hz` against `1_000_000_000`,
  preserving fractional debt without integer-period drift. The `u64 * u32`
  multiplication is promoted to `u128` before arithmetic (at most 96 bits),
  so the accepted maximum host timestamp and Product Manifest rate cannot
  overflow it. It admits at most `max_catch_up_steps`, records and drops excess
  *whole* debt, and never admits presentation implicitly.
- **Demand** admits exactly one caller-requested simulation step and has no
  clock input.
- **External** admits the caller-supplied exact next `ExternalStep`; duplicate
  or skipped step values are rejected, with no clock input.
- **Presentation** has an independent `admit_presentation` call and token. It
  remains available in `Paused` for menus and inspection; simulation admission
  does not.

`RuntimeGeneration` changes only on start and restart. The separate
`RuntimeControlRevision` changes on start, restart, pause, resume, fault, and
shutdown, so a prior token cannot become valid after a pause/resume without
misrepresenting it as a new runtime generation. Shutdown is terminal; create a
new lifecycle instance for a new runtime. Pause/resume clears only the
realtime clock baseline (so paused elapsed wall time is not admitted) while
retaining substep fractional debt; restart, fault, and shutdown clear both.
`validate_phase_token` receives the expected named phase and rejects a token
for a different handoff.

## Primary paths

- [`runtime-lifecycle/src/lib.rs`](../../rust/crates/runtime-lifecycle/src/lib.rs)
- [`runtime-lifecycle/src/lifecycle.rs`](../../rust/crates/runtime-lifecycle/src/lifecycle.rs)
- [`runtime-lifecycle/src/model.rs`](../../rust/crates/runtime-lifecycle/src/model.rs)
- [`runtime-lifecycle/tests/runtime_lifecycle.rs`](../../rust/crates/runtime-lifecycle/tests/runtime_lifecycle.rs)
- [`rusty-engine facade`](../../rust/crates/rusty-engine/src/lib.rs)

The complete downstream facade re-exports this crate as
`rusty_engine::runtime_lifecycle`.

`runtime-input` consumes an `InputSnapshot` phase token together with the
explicit lifecycle instance and validates it before forming a frame. A product
still owns when it asks for a snapshot and every later consequence; lifecycle
validation only prevents paused, faulted, shutdown, stale, or wrong-phase
input data from being attached to an admitted step.

## Public data flow

```text
validated ProductManifest + host-owned monotonic time / direct step intent
        |
        v
runtime-lifecycle instance
        |
        +--> SimulationAdmission
        |      +--> input snapshot token
        |      +--> schedule token
        |      +--> timeline token
        |      +--> mutation token
        |      `--> projection token
        |
        `--> PresentationAdmission (separate call)
                |
                v
downstream named owners and host presentation
```

The arrows are data/correlation handoffs. They do not invoke a callback,
discover a service, perform a mutation, or select a renderer.

## Focused verification

```bash
cargo test -p runtime-lifecycle --locked
cargo clippy -p runtime-lifecycle --all-targets --locked -- -D warnings
python3 scripts/dependency_boundary_check.py
./scripts/verify.sh
```

The focused suite covers all modes, Product Manifest selection, exact 144 Hz
fractional carry, regression, catch-up/drop limits, max timestamp arithmetic,
pause/resume fractional carry, generation/control-revision staleness,
cross-instance token rejection, wrong-phase rejection, idempotent token
validation, fault, shutdown, wrong mode calls, and separate paused presentation
admission.
