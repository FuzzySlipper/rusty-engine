# Product materialization

`rust/crates/product-materializer` owns the build-time green path from the
declared Product Layout to fresh Compiled Composition, admitted runtime content,
and the fixed browser bundle inputs consumed by `product-assembly`. It copies
only the declared TypeScript closure into private scratch space, typechecks the
rules lane without DOM types and the UI lane with DOM types, evaluates the
build-time composition DSL, re-admits canonical bytes through Product Model,
and bundles the named `mountProductUi` export with Engine-owned browser roots.

Authored scanners, typechecking, and rules evaluation run under the admitted
Node permission boundary. Vite is an explicit trusted Engine tool over a
pre-scanned scratch closure; emitted JavaScript is parsed again before
publication and may not introduce dynamic, bare, absolute, or closure-escaping
imports. The materializer owns no live runtime, browser server, product meaning,
or filesystem reach-through after generation.

## Primary paths

- [`product-materializer/src/lib.rs`](../../rust/crates/product-materializer/src/lib.rs)
- [`product-materializer/src/materialize.rs`](../../rust/crates/product-materializer/src/materialize.rs)
- [`product-materializer/tests/product_assembly.rs`](../../rust/crates/product-materializer/tests/product_assembly.rs)

The integration test proves authored TypeScript through materialization and
Product Assembly, byte-identical delete/regenerate, relocation, build after
authored-lane removal, generated-host serving, typed lifecycle execution, and
clean shutdown. Its explicit ignored gate launches real Chromium against the
actual generated loopback host with no mocked transport.

## Focused verification

```bash
cargo test -p product-materializer --locked
cargo clippy -p product-materializer --all-targets --locked -- -D warnings
./scripts/verify-product-materializer.sh  # after Rules and renderer preparation
```
