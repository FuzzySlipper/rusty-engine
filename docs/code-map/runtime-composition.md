# Runtime composition

`rust/crates/runtime-composition` owns one instance-local, host-neutral live
composition root. It combines `runtime-lifecycle`, `runtime-input`,
`runtime-schedule`, `runtime-timeline`, `runtime-mutation`, and projection
outputs around one concrete `ProductRuntimeAdapter`. The root admits each
simulation step in the fixed input, simulation, consequences, commit, and
projection order and preserves the owning services' token, revision, atomicity,
retry, and rebind contracts.

The adapter is ordinary statically linked Rust selected by generated Product
Assembly. Runtime Composition stores no callback registry, erased product
owner, ambient service locator, browser object, clock, scheduler policy, or
product persistence. Failures fault the lifecycle; restart reconstructs the
owned cursors and requires the product adapter to restore its own intended
state explicitly.

## Primary paths

- [`runtime-composition/src/lib.rs`](../../rust/crates/runtime-composition/src/lib.rs)
- [`runtime-composition/src/adapter.rs`](../../rust/crates/runtime-composition/src/adapter.rs)
- [`runtime-composition/src/root.rs`](../../rust/crates/runtime-composition/src/root.rs)
- [`runtime-composition/src/error.rs`](../../rust/crates/runtime-composition/src/error.rs)
- [`runtime-composition/tests/counter.rs`](../../rust/crates/runtime-composition/tests/counter.rs)

The counter conformance test covers demand, external, and realtime admission;
typed input; mutation publication and empty completion; projection; pause and
same-generation rebind; explicit fault; shutdown; and restart recovery.

## Focused verification

```bash
cargo test -p runtime-composition --locked
cargo clippy -p runtime-composition --all-targets --locked -- -D warnings
```

