# Product model schema

## Ownership

`rust/crates/product-model` owns host-neutral validation for the current
`rusty.toml` Product Layout and immutable Compiled Composition artifact. It
defines fixed authored and generated lanes, bounded identities and payloads,
strict decoding, typed references, deterministic encoding, and structured
diagnostics.

It does not discover or read product files, compile TypeScript, admit runtime
content, execute schedules, mutate live state, build a Product Assembly, run a
host, or generate desktop wrappers. Those capabilities are separate campaign
milestones. The current schema deliberately has no compatibility-version
family.

## Primary paths

- [`product-model/src/lib.rs`](../../rust/crates/product-model/src/lib.rs)
- [`product-model/src/manifest.rs`](../../rust/crates/product-model/src/manifest.rs)
- [`product-model/src/composition.rs`](../../rust/crates/product-model/src/composition.rs)
- [`product-model/src/path.rs`](../../rust/crates/product-model/src/path.rs)
- [`product-model/src/diagnostic.rs`](../../rust/crates/product-model/src/diagnostic.rs)
- [`product-model/tests/contract.rs`](../../rust/crates/product-model/tests/contract.rs)
- [`product-model fixtures`](../../fixtures/product-model)

## Source routes

| Path | Owner |
|---|---|
| `src/manifest.rs` | Product Layout fields, lifecycle selection, wrapper policy, fixed lanes, output separation |
| `src/composition.rs` | Current Compiled Composition DTO, admission checks, references, payload budgets, canonical bytes |
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
