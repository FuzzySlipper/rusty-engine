# Runtime schedule

## Ownership

`rust/crates/runtime-schedule` owns the runtime realization of a linked
Product Model schedule. It resolves the closed `input`, `simulation`,
`consequences`, `commit`, and `projection` phases into a stable topological
order, retaining the authored composition mode and standard-anchor status for
inspection. It additionally validates system cadence, same-phase dependencies,
placement constraints, and unordered read/write conflicts before binding to a
live lifecycle instance.

The bound `RuntimeSchedule` is one instance-owned, non-cloneable progression
lane. It validates the caller's lifecycle phase token and runtime identity for
each phase, invokes only systems due for the supplied `SimulationStep`, and
stages all dispatcher outputs until the whole phase succeeds. The dispatcher
is supplied explicitly on every call; the crate stores no callbacks, service
registry, executor, clock, timer, component reference, or gameplay state.

## Primary paths

- [`runtime-schedule/src/lib.rs`](../../rust/crates/runtime-schedule/src/lib.rs)
- [`runtime-schedule/src/compile.rs`](../../rust/crates/runtime-schedule/src/compile.rs)
- [`runtime-schedule/src/runtime.rs`](../../rust/crates/runtime-schedule/src/runtime.rs)
- [`runtime-schedule/src/inspection.rs`](../../rust/crates/runtime-schedule/src/inspection.rs)
- [`runtime-schedule/src/error.rs`](../../rust/crates/runtime-schedule/src/error.rs)
- [`runtime-schedule/tests/schedule.rs`](../../rust/crates/runtime-schedule/tests/schedule.rs)
- [`rusty-engine facade`](../../rust/crates/rusty-engine/src/lib.rs)

The complete downstream facade re-exports the crate as
`rusty_engine::runtime_schedule`.

## Data flow and phase ordering

```text
LinkedProductComposition
        |
        v
CompiledRuntimeSchedule -- inspection / deterministic JSON
        |
        +--> bind(running RuntimeLifecycle)
                    |
                    v
          RuntimeSchedule (instance + generation + control revision)
                    |
  phase token + SimulationStep + explicit dispatcher/context
                    |
                    v
  input -> simulation -> consequences -> commit -> projection
                    |
                    `--> staged typed outputs on successful phase
```

The schedule phase maps directly to lifecycle tokens:

| Product Model phase | Lifecycle token |
|---|---|
| `input` | `InputSnapshot` |
| `simulation` | `Schedule` |
| `consequences` | `Timeline` |
| `commit` | `Mutation` |
| `projection` | `Projection` |

Cadence is integer step based: a system is due when
`step >= offsetSteps` and `(step - offsetSteps) % everySteps == 0`. The rule
is independent of realtime, demand, or external admission; the lifecycle
remains the source of step identity and host time remains outside this crate.

## Inspection and boundaries

`ScheduleInspection` is a typed printable readout and compact newline JSON.
It contains phase order, composition mode, retained/replaced standard anchor,
an explicit ordered item list containing retained `Standard.<phase>` anchors,
system final indices and IDs, capability target/kind/use and resolved target,
owner and provenance, declared access, cadence, payload budget, dependency
declarations, placement, and payload. It deliberately has no independent
schema/version field: compatibility follows actual Product Model changes.

The dispatcher receives only immutable `ScheduleSystemInvocation` data and a
caller-owned read-only context. It may call a named downstream owner and stage
its own domain output, but mutation and consequences remain outside the
schedule lane. Failed dispatch leaves phase progression unchanged (the lane
does not roll back effects already performed inside a caller dispatcher), and
`dispose` is terminal. Initial binding must happen before the lifecycle admits
its first simulation step; a later initial bind is rejected as already
advanced. A lifecycle pause/resume or restart makes a bound lane stale. At a
phase boundary, call `rebind` (or its `synchronize` alias): same-generation
control revisions retain completed progress and reconcile the next-step cursor
to the lifecycle's cumulative admission count; admissions not represented by
that cursor are reported as invalidated, never as completed. Rebind should
happen before admitting new work under the resumed revision. The immutable
schedule readout exposes that bounded invalidated-admission count. A newer
generation resets step progression and the invalidation count. If a revision changes while a phase is active,
`RebindActiveStep` rejects the operation because the incomplete phase chain
cannot be resumed safely; restart the lifecycle/new generation and bind a
fresh lane, or dispose and reconstruct the existing lane instead.

## Focused verification

```bash
cargo test -p runtime-schedule --locked
cargo clippy -p runtime-schedule --all-targets --locked -- -D warnings
cargo test -p product-model --locked
./scripts/verify.sh
```
