# Runtime standard capabilities

`rust/crates/runtime-standard-capabilities` owns closed, host-neutral Runtime
Composition capability mechanisms. Its first capability,
`engine.runtime.observe-pairs`, is one named observation shape, not an
extensible query language or callback executor.

## Primary paths

- [`runtime-standard-capabilities/src/lib.rs`](../../rust/crates/runtime-standard-capabilities/src/lib.rs)
- [`contract exporter`](../../rust/crates/runtime-standard-capabilities/src/bin/export-runtime-standard-capabilities-contract.rs)
- [`Product Model catalog`](../../rust/crates/product-model/src/capability_catalog.rs)
- [`Runtime mutation`](runtime-mutation.md)
- [`Runtime schedule`](runtime-schedule.md)

## Closed flow

The schedule payload names only typed observer/target component identities,
the linked Product Kernel operation binding, fixed operation/result type, and
bounded quotas. Static Rust adapters provide local eye/origin, forward, range,
facing threshold, evidence, and target body-center facts. `EntityState`
supplies active typed components and world transforms; `engine-spatial`
supplies canonical center-ray occlusion. The service returns a deterministic
target-ordered reduction and converts it into exactly one bounded mutation
operation. The caller supplies batch identity and uses `runtime-mutation` to
stage and publish the product candidate.

Generated Product Assembly adapters use
`ObservePairsPlan::compile_system(&CompiledSystem, &CompiledMutationCatalog)`
against the exact immutable artifacts retained by `runtime-composition`.
Their fixed `ProductKernelRuntimeDefinition` supplies concrete observer and
target component types plus product-owned entity/spatial facts. A direct
`evaluate_and_batch::<Observer, Target>(...)` call returns one
`ObservePairsEmission` for the adapter's existing `prepare_mutation` path.
No runtime type erasure, callback, registry, or string method dispatch is
involved; Product Assembly never synthesizes product facts.

The static schedule owns due-step cadence. This crate retains compiled cadence
as inspection data but owns no clock, scheduler, dispatcher callback,
registry, mutation authority, host, renderer, or product alert semantics.
Malformed active typed facts, role identity drift, transform absence, quota
overflow, and spatial failures reject the whole evaluation; inactive entities
are skipped. There is no partial mutation or silent truncation.

## Generated contract

```bash
cargo run -q -p runtime-standard-capabilities --bin export-runtime-standard-capabilities-contract
```

This Rust-owned JSON descriptor is the TypeScript authoring generator input.
It declares exact payload fields, fixed `center-ray` visibility, result kind,
Engine access identities, and current quotas. It has no independent schema
version: actual contract changes are the compatibility boundary.
