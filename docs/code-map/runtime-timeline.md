# Runtime timeline

`runtime-timeline` is the instance-owned Runtime Composition timeline lane.
It consumes only a `product-model::LinkedProductComposition`; the Product
Model `Timeline` and `TimelineStep` declarations remain static descriptive
templates and do not become callbacks or live queue entries.

## Primary paths

- [`runtime-timeline/src/lib.rs`](../../rust/crates/runtime-timeline/src/lib.rs)
- [`runtime-timeline/src/compile.rs`](../../rust/crates/runtime-timeline/src/compile.rs)
- [`runtime-timeline/src/model.rs`](../../rust/crates/runtime-timeline/src/model.rs)
- [`runtime-timeline/src/runtime.rs`](../../rust/crates/runtime-timeline/src/runtime.rs)
- [`runtime-timeline/src/inspection.rs`](../../rust/crates/runtime-timeline/src/inspection.rs)
- [`runtime-timeline/src/error.rs`](../../rust/crates/runtime-timeline/src/error.rs)
- [`runtime-timeline/tests/timeline.rs`](../../rust/crates/runtime-timeline/tests/timeline.rs)

## Ownership

- `CompiledTimelineCatalog::compile` resolves each admitted timeline step to
  its exact linked capability binding, target, resolved target, operation kind,
  owner, source/provenance path, and opaque payload.
- `RuntimeTimeline` owns one bounded queue of scheduled operations and one
  bounded set of asynchronous completion tickets for one explicit lifecycle
  instance. It owns no clock, executor, callback, service registry, host
  object, component reference, or product state.
- `RuntimeTimeline::release_due` is the only simulation re-entry point. It
  requires the current `RuntimePhase::Timeline` token and emits immutable
  operation/completion records. A downstream mutation/consequence owner
  decides what those records mean and performs any capability call.
- `TimelineSnapshot` contains mechanism state only. Products choose their own
  save/persistence policy and may serialize this typed value at their boundary.

## Deterministic queue

Schedule, cancel, replace, ticket registration, and ticket cancellation are
all token-gated against the current `RuntimePhase::Timeline` binding. An
external completion may finish outside that phase through the queue-only
`admit_completion(&RuntimeLifecycle, envelope)` method, which still requires a
Running lifecycle with the exact lane binding before recording inert data. The
lane issues insertion/ticket sequence values and validates the compiled step
before publishing queue state; no stale lane-only mutator is public.

Scheduled operations sort by `(due SimulationStep, insertion sequence,
caller operation identity)`. `Once` removes after one release. `Every` has a
nonzero interval and bounded finite remaining count; one occurrence is
released per admitted step, so an overdue operation is a bounded deterministic
backlog rather than a fabricated retroactive step. In particular, realtime
clock catch-up/drop decisions remain owned by `runtime-lifecycle`: a dropped
step never creates a timeline token, and the next admitted step releases all
due entries with `due <= current` up to the caller's release prefix bound.

## Completion tickets

Before external work starts, a caller registers a ticket bound to a compiled
timeline step, exact capability target/kind, caller operation identity and
revision, `RuntimeSourceKind` (`filesystem`, `network`, `inference`, or
`external`), correlation identity, and product-owned result-contract identity.
The completion envelope can contain only that ticket, exact binding and
correlation/provenance, and bounded success/failure opaque data; it cannot
provide or replace a capability, source kind, or result contract.

Ticket issue sequence—not completion arrival order—controls release. Completed
tickets arriving out of order remain held behind the first pending ticket.
Cancellation or a failure completion closes the gap as data; a ticket can be
completed exactly once. Cancel, replace, and recurrence advancement atomically
cancel only Pending tickets bound to the invalidated operation revision, while
preserving already Completed outcomes as releasable facts. A later completion
cannot remain behind a permanent stale gap.

## Lifecycle and snapshots

Pause/resume changes the lifecycle control revision. `rebind` retains inert
scheduled operations, deterministically removes old-revision completion
tickets, and advances the release cursor past lifecycle steps admitted before
the new binding. Those invalidated admissions are counted in the readout,
snapshot, and rebind receipt but are never reported as released; the next
admitted token is the recovery boundary. Restart begins a new generation and
clears operations, tickets, cursors, and lane counters with a receipt.
Faulted, shutdown, paused, stale, foreign, or wrong-phase tokens cannot
release or queue work. Rebind and release are rejected while an active release
is in progress.

Snapshots are validated into a temporary candidate before publication: bounds,
unique operation/ticket identities, canonical operation/ticket order, issue
sequence/cursor relationships, ticket correlation/result-contract/provenance
bounds, exact compiled step target/kind, operation-ticket revision/template
bindings, recurrence invariants, lifecycle binding, admission invalidation
cursor, and release cursor are all checked. Failed restore leaves the live lane
unchanged.

Focused checks:

```text
cargo test -p runtime-timeline --locked
cargo clippy -p runtime-timeline --all-targets --locked -- -D warnings
```
