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
- [`gameplay-standard/tests/contract.rs`](../../rust/crates/gameplay-standard/tests/contract.rs)
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

## Focused verification

```bash
cargo test -p gameplay-standard --locked
cargo clippy -p gameplay-standard --all-targets --locked -- -D warnings
cargo run -p gameplay-standard --example select_capabilities --locked
python3 scripts/dependency_boundary_check.py
./scripts/verify-rust-sdk-consumer.sh
```

Full provider verification remains `./scripts/verify.sh`.
