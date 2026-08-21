# Gameplay standard

## Ownership

`rust/crates/gameplay-standard` owns only static capability metadata and named
module namespaces for opt-in gameplay adoption. It has four current modules:
`entity_state`, `mechanics`, `resolution`, and `rules`. Each exports a
`READOUT` with a bounded stable identity, a positive version, and
`CapabilityMaturity::Incubating`, then re-exports the exact API of its focused
owner.

The crate is not a gameplay runtime or a layer above those owners. It owns no
registry, discovery API, aggregate world/session object, scheduler,
persistence, bootstrap, service location, trait-object collection, global
module enumeration, game vocabulary, numeric-expression semantics, or dynamic
extension format. `entity-state`, `gameplay-mechanics`,
`gameplay-resolution`, and `gameplay-rules` remain directly usable with or
without this convenience layer.

Availability is broader than a product default. A downstream product chooses
its own small preset, explicit module set, or no standard metadata at all;
every listed capability remains explicitly selectable. Typed product
extensions remain ordinary downstream Rust composition. Future neutral modules
are additive named namespaces and readouts, not entries in a closed enum or a
changed global runtime facade.

## Primary paths

- [`gameplay-standard/Cargo.toml`](../../rust/crates/gameplay-standard/Cargo.toml)
- [`gameplay-standard/src/lib.rs`](../../rust/crates/gameplay-standard/src/lib.rs)
- [`gameplay-standard/src/resolution.rs`](../../rust/crates/gameplay-standard/src/resolution.rs)
- [`gameplay-standard/tests/contract.rs`](../../rust/crates/gameplay-standard/tests/contract.rs)
- [`gameplay-standard/tests/standard_resolution.rs`](../../rust/crates/gameplay-standard/tests/standard_resolution.rs)
- [`gameplay-standard/examples/select_capabilities.rs`](../../rust/crates/gameplay-standard/examples/select_capabilities.rs)
- [Canonical design](../design.md)
- [Rust SDK capability index](../rust-sdk-capabilities.md)

## Public selection

Select descriptors independently; no aggregate owner is constructed:

```rust
use gameplay_standard::modules::{mechanics, resolution};

let selected = [&mechanics::READOUT, &resolution::READOUT];
assert_eq!(selected[0].identity().as_str(), "mechanics");
assert_eq!(selected[1].identity().as_str(), "resolution");

let stat = mechanics::StatId::parse("health")?;
let attempt = resolution::ResolutionId::new(1)?;
```

The readouts describe availability and maturity only. The selected owners still
perform their own construction and named service calls, while the downstream
product owns meaning, defaults, timing, state, and persistence.

## Numeric families and authoring handoff

`exact` keeps `MechanicsScalar` and `ExactRatio` intact while exposing an
ordered, bounded exact expression tree. `continuous` owns only finite
normalized-binary64 values and its independently bounded expression tree.
They have separate evaluators, input bundles, comparisons, errors, and
conversion names; no universal numeric value, callback, scheduler, continuous
stat/track, or coercion matrix exists.

Each family exposes an inspectable requirement artifact: typed inputs are
sorted and deduplicated, and every input role must occur in the definition's
canonical capability-role requirements. A downstream exact or continuous leaf
may implement the matching static compile trait to produce a closed Rust tree
before admission. This is typed composition, not a runtime callback, registry,
or opaque-payload route.

Definitions carry a family, evaluator-semantics version, subject/source
correlation, ordered tree, and canonical role requirements through the
`gameplay-rules` schema-1/schema-2 package path. Rust owns evaluation and the
canonical fixture source. Task #7180 consumes that surface to generate strict
TypeScript authoring/contracts; it must not add a TypeScript runtime evaluator.

## Standard resolution leaves

`StandardPredicate` is only an exact expression comparison. Branching stays in
the existing `gameplay_resolution::Program::When`; no Boolean program node or
second traversal is introduced. `StandardOperation` supplies a first typed
mechanics column: spend/restore a track, submit bounded damage, and apply or
remove an effect. Each leaf uses an explicit capability role binding to one
`EntityId`, checks the operation's named capability, evaluates its exact
amount, and captures a deliberately conservative standard mechanics entity
snapshot in `StandardOperationPlan`: every mechanics component slot on each
participating entity is guarded whether present or absent, plus Item slots for
currently equipped items. This is intentionally broader than an individual
service's current reads so planning remains sequence-safe. Each
evaluated exact operand retains its
evaluator semantics version, optional admitted-definition identity, canonical
input requirements, supplied referenced values, and exact result; the plan
also records catalog version and fingerprint.

Planning returns `StandardMechanicsEffect`, which contains the existing
mechanics request type unchanged. It does not mutate authoritative state.
Only a product-owned `ResolutionTransaction` may execute the effect against a
private candidate and publish that candidate once. Candidate execution rebases
the request guard to its private slot revision; the plan retains the complete
conservative authoritative mechanics snapshot for product readout and commit
guarding. Before staging the candidate, `validate_source_state` checks those
original facts plus catalog provenance, so a stale plan fails without a
publication. Candidate execution returns a typed `StandardMechanicsReceipt`;
the product transaction owns retaining those receipts alongside its own
publication receipt rather than reducing them to strings or adding them to a
global runtime. The standard
crate owns no world transaction, target search, attack, turn, timer, effect
timing, consequence, save, or product extension meaning. `ComposedPredicate`
and `ComposedOperation<Product>` are closed enum seams that keep product leaves
typed rather than adding an extension ID/payload tunnel.

## Focused verification

```bash
cargo test -p gameplay-standard --locked
cargo clippy -p gameplay-standard --all-targets --locked -- -D warnings
cargo run -p gameplay-standard --example select_capabilities --locked
python3 scripts/dependency_boundary_check.py
./scripts/verify-rust-sdk-consumer.sh
```

Full provider verification remains `./scripts/verify.sh`.
