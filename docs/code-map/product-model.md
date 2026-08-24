# Product model schema

## Ownership

`rust/crates/product-model` owns host-neutral validation for the current
`rusty.toml` Product Layout, immutable Compiled Composition artifact, and
their immutable product-composition admission. It defines fixed authored and
generated lanes, bounded identities and payloads, strict decoding, typed
references, deterministic encoding, and structured diagnostics.

It does not discover or read product files, compile TypeScript, admit runtime
content, execute schedules, mutate live state, build a Product Assembly, run a
host, or generate desktop wrappers. Those capabilities are separate campaign
milestones. The current schema deliberately has no compatibility-version
family.

## Primary paths

- [`product-model/src/lib.rs`](../../rust/crates/product-model/src/lib.rs)
- [`product-model/src/manifest.rs`](../../rust/crates/product-model/src/manifest.rs)
- [`product-model/src/composition.rs`](../../rust/crates/product-model/src/composition.rs)
- [`product-model/src/admission.rs`](../../rust/crates/product-model/src/admission.rs)
- [`product-model/src/contract.rs`](../../rust/crates/product-model/src/contract.rs)
- [`product-model/src/bin/export-product-model-contract.rs`](../../rust/crates/product-model/src/bin/export-product-model-contract.rs)
- [`product-model/src/path.rs`](../../rust/crates/product-model/src/path.rs)
- [`product-model/src/diagnostic.rs`](../../rust/crates/product-model/src/diagnostic.rs)
- [`product-model/tests/contract.rs`](../../rust/crates/product-model/tests/contract.rs)
- [`product-model fixtures`](../../fixtures/product-model)

## Source routes

| Path | Owner |
|---|---|
| `src/manifest.rs` | Product Layout fields, lifecycle selection, wrapper policy, fixed lanes, output separation |
| `src/composition.rs` | Current Compiled Composition DTO, typed reference checks, declarative schedule accesses, payload budgets, and explicit ECMAScript-number canonical bytes |
| `src/admission.rs` | Product/Layout identity linkage plus immutable resolved capability/definition declaration readouts; no evaluation or scheduling |
| `src/contract.rs` | Current Rust-owned descriptor for TypeScript generation and schema-drift checks; no version family |
| `src/path.rs` | Lexical product-relative path grammar and component-aware relationships |
| `src/diagnostic.rs` | Bounded machine-readable schema diagnostics |
| `tests/contract.rs` | Valid/invalid fixtures, fail-closed parsing, bounds, references, and deterministic encoding |

The complete downstream facade re-exports the crate as
`rusty_engine::product_model` without wrappers.

## Forbidden dependencies and shortcuts

- no filesystem, process, network, browser, renderer, TypeScript, Studio, or
  downstream-product dependency;
- no runtime evaluator, scheduler, clock, mutation service, service locator,
  registry, host, or wrapper generator;
- no schedule conflict, dependency-order, target-resolution, or execution
  interpretation; admission only preserves validated declarations for later
  named owners;
- no schema-version, compatibility-matrix, or migration scaffolding before an
  independently evolving producer/consumer boundary exists; and
- no interpretation of product-owned opaque payload meaning.

## Focused verification

```bash
cargo test -p product-model --locked
cargo clippy -p product-model --all-targets --locked -- -D warnings
python3 scripts/dependency_boundary_check.py
./scripts/verify-rust-sdk-consumer.sh
```

Full provider verification remains `./scripts/verify.sh`.
