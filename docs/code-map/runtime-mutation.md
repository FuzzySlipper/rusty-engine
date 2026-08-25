# Runtime mutation

`rust/crates/runtime-mutation` owns the sole admitted Runtime Composition
Mutation publication boundary. It consumes a fully linked
`product_model::LinkedProductComposition` plus an immutable Product Assembly
descriptor slice. Mutation selection is intentionally not a new
`CapabilityUse`: authored composition references remain `input-map`,
`schedule`, and `timeline`, while the assembly explicitly selects which linked
`Operation` bindings may publish into a named in-memory domain.

## Primary paths

- [`runtime-mutation/src/lib.rs`](../../rust/crates/runtime-mutation/src/lib.rs)
- [`runtime-mutation/src/compile.rs`](../../rust/crates/runtime-mutation/src/compile.rs)
- [`runtime-mutation/src/model.rs`](../../rust/crates/runtime-mutation/src/model.rs)
- [`runtime-mutation/src/runtime.rs`](../../rust/crates/runtime-mutation/src/runtime.rs)
- [`runtime-mutation/src/inspection.rs`](../../rust/crates/runtime-mutation/src/inspection.rs)
- [`runtime-mutation/src/error.rs`](../../rust/crates/runtime-mutation/src/error.rs)
- [`runtime-mutation/tests/mutation.rs`](../../rust/crates/runtime-mutation/tests/mutation.rs)
- [`rusty-engine facade`](../../rust/crates/rusty-engine/src/lib.rs)

The complete downstream facade re-exports this crate as
`rusty_engine::runtime_mutation`.

## Static assembly and resolved batches

`MutationCapabilityDescriptor` is data-only: it names an authored binding id,
exact target, stable publication domain, named owner, and exact operation wire
type. Compilation matches
each descriptor against a linked binding, requires linkable `Operation` kind,
retains the resolved target/provenance and capability payload budget, and
rejects duplicates, missing bindings, target drift, invalid identities, and
unavailable capabilities before runtime binding. One catalog has exactly one
publication domain and a deterministic SHA-256 catalog identity; the live
`MutationAuthority` must report that same domain before planning. No descriptor stores a
handler, callback, trait object, service locator, or registry entry.

`MutationBatch` is a bounded, nonempty ordered list of capability-specific
payloads with stable batch id, causation, and provenance identities. The lane
resolves each operation against the closed assembly selection and checks
duplicate operation ids, exact binding/target linkage, compact bounded JSON,
and the selected capability budget before invoking a planner. Each batch has a
deterministic SHA-256 fingerprint over its compact Rust JSON envelope. This is
a local readback/idempotency identity, not a cross-language canonical format.
The payload is
opaque to Engine and is interpreted only by the Product Assembly's closed Rust
planner; this is not a generic JSON write command.

## Atomic publication

`MutationAuthority` exposes only an infallible exact `Guard: Clone + Eq`
readout. A caller supplies `MutationPlanner<A, E>` for each Mutation token;
the planner receives `&A` and the resolved ordered batch and returns an owned
`MutationStage<A, E>` containing a complete candidate and one bounded
named-owner evidence item per operation. The lane validates evidence identity,
binding target, resolved target, publication domain, owner, and order, computes
the candidate guard and full receipt,
revalidates the live guard, authority domain, and lifecycle token, prepares all
bounded lane bookkeeping, then performs exactly one `*authority = candidate`
assignment. Any returned pre-publication error leaves the live authority
unchanged. This is conditional Result-failure atomicity under a trusted closed
Product Assembly. Rust cannot prove candidate derivation, complete guards, or
the absence of interior side effects. Panic, allocation failure, and destructor
atomicity are not claimed; authority, guard, planner error, and evidence values
must have inert, non-panicking destruction.

The honest atomicity unit is exactly one successful nonempty batch for one
Mutation token and one caller-owned in-memory authority bundle. It does not
cover a whole tick/world, durable persistence, projection/rendering,
notifications, or external/outbox work. Owners unable to produce an owned
candidate stay outside this bundle and report after-commit status through a
separate product-owned path.

The preceding `runtime-schedule` dispatcher is a caller contract, not a Rust
capability sandbox: its `commit` phase may emit inert planning data, but this
crate cannot police interior side effects performed by an arbitrary dispatcher.
Downstream products must route authoritative writes through this named
mutation planner/publication boundary and treat schedule dispatch effects as
outside the mutation transaction.

## Exactly-once and lifecycle

The bound lane is instance-owned and non-cloneable. It validates exact
instance/generation/control revision, `RuntimePhase::Mutation`, admitted step
cursor, and lifecycle Running state. One successful batch advances the cursor;
an exact retry within the 32-entry retained history returns the stored prior
receipt without republishing, even after later lifecycle admissions. Reusing a
retained batch id with different bytes or using another batch for an applied
step conflicts. Eviction is counted explicitly; an evicted old step still
cannot republish because the monotonic step cursor has advanced. Planner,
preflight, evidence, candidate-guard, or stale-live-guard failures consume no
step and are retryable.

Same-generation pause/resume rebind reconciles admitted-but-uncommitted steps
as invalidated readout count; it never reports them completed or rewinds input
or timeline lanes. Same-generation rebind retains historical receipts for
readback, while a new generation resets progress and history. Older admitted
tokens remain eligible only in monotonic lane order, which permits deliberate
catch-up after multiple lifecycle admissions. Foreign, stale, wrong-phase,
paused, faulted, shutdown, disposed, or lane-out-of-order tokens cannot publish.

Sparse schedule cadence still requires every admitted Mutation token to be
accounted for. `complete_empty_step` validates that exact token and advances
the same cursor without receiving authority, a planner, or a batch. Retained
exact retries return the prior empty-completion receipt; a batch and an empty
completion conflict for the same step. Empty evidence is bounded, survives a
same-generation rebind, resets on a new generation, and an evicted old step
remains ineligible because the monotonic cursor has advanced.

Receipts retain binding, step, batch/causation/provenance, ordered resolved
operations with binding index, target, kind, resolved target, publication
domain, owner, linked provenance source/path, and payload, plus observed and
committed guards, batch fingerprint, catalog identity, and exact owner evidence.
Inspection is typed deterministic newline JSON and has no independent version
field: compatibility follows actual Product Model and assembly changes.

## Focused verification

```bash
cargo test -p runtime-mutation --locked
cargo clippy -p runtime-mutation --all-targets --locked -- -D warnings
cargo test -p product-model --locked
./scripts/verify.sh
```
