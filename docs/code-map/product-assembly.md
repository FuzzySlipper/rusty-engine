# Product Assembly planning and publication

## Ownership

`rust/crates/product-assembly` owns the Engine-side boundary that turns one
admitted Product Layout, fresh compiler/host bytes, and product-relative source
lanes into a deterministic generated Product Assembly. It admits the current
Compiled Composition from caller-supplied bytes, closes authored `rules/`,
`ui/`, and optional `kernel/` source lanes, materializes only the
`ContentManifest`-required runtime bodies, generates a product-specific Rust
workspace, and records exact hashes in a versionless receipt. The generated
workspace has a product-specific package and executable, plus the fixed
`rusty_product` library crate. Its public `product` module exposes the same
`GeneratedProductDevRuntime` used by the development host, including its
constructor, so a separately generated desktop wrapper can source-link the
runtime instead of copying it.

Publication stages the complete `generated/` tree, reads every staged output
back, and swaps the tree as one transaction. Existing output is retained as a
recoverable sibling until the new tree is verified. Reads and writes are
capability-relative and reject symlinks, special files, path escapes, stale
inputs, extra output files, and tampered bytes.

The crate does not compile TypeScript, own browser transport, serve HTTP/SSE,
interpret product payload meaning, or retain runtime callbacks/registries.
Fresh compiled composition and browser host/template assets are explicit typed
`AssemblyGenerationInputs`/`BrowserBundleInputs`; the browser descriptor
requires `index.html`, `main.js`, `bridge.js`,
`engine/product-browser-host.js`, `runtime-adapter.js`, and the declared
compiled `ui/*.js` entry. A caller owns their production. Empty Engine-only
compositions generate a concrete empty lifecycle/phase executable. A product
with capabilities must provide the fixed source symbol `RustyProductRuntime`;
generated source validates its complete Kernel descriptor/owner selection,
compiles its mutation descriptors against the linked composition, builds its
concrete adapter from immutable `include_bytes!` resources, and places that
adapter in the same `RuntimeComposition`. Capability arms are never emitted as
callbacks, erased registrations, or silent no-ops.

## Primary paths

- [`product-assembly/src/lib.rs`](../../rust/crates/product-assembly/src/lib.rs)
- [`product-assembly/src/source.rs`](../../rust/crates/product-assembly/src/source.rs)
- [`product-assembly/src/receipt.rs`](../../rust/crates/product-assembly/src/receipt.rs)
- [`product-assembly/src/filesystem.rs`](../../rust/crates/product-assembly/src/filesystem.rs)
- [`product-assembly/src/publish.rs`](../../rust/crates/product-assembly/src/publish.rs)
- [`product-assembly/src/error.rs`](../../rust/crates/product-assembly/src/error.rs)
- [`product-assembly/src/tests.rs`](../../rust/crates/product-assembly/src/tests.rs)

## Source routes

| Path | Owner |
|---|---|
| `src/source.rs` | Fresh-input admission, authored/content closure, generated Rust library/binary source, receipt assembly, and plan/verify APIs |
| `src/filesystem.rs` | Open-directory capability traversal, no-follow reads, UTF-8 relative paths, and aggregate bounds |
| `src/receipt.rs` | Strict versionless receipt schema, exact hashes, ordering, role/path rules, and bounded readback decode |
| `src/publish.rs` | Complete generated-tree stage, exact readback, swap, rollback, and recoverable cleanup diagnostics |
| `src/error.rs` | Bounded machine-readable diagnostics |
| `src/tests.rs` | Determinism, relocation, stale/tampered output, content cache rejection, symlink rejection, receipt strictness, rollback, generated library/binary and Kernel compilation, external library consumption, and nonempty intent-to-mutation-to-UI proof |

## Public workflow

1. The product command constructs `AssemblyGenerationInputs` from newly
   materialized Compiled Composition bytes and the browser host asset map.
2. `plan_product_assembly` or its capability-descriptor variant reads the
   authored root and returns an immutable `AssemblyPlan`; no generated output
   is used as a generation input. The plan publishes `src/lib.rs` with
   `pub mod product;`, a thin `src/main.rs` that invokes
   `rusty_product::product::run`, and the generated `src/product.rs` runtime
   implementation. Kernel source is linked from the library root so both the
   development binary and external wrappers use the exact same runtime type.
3. The caller invokes `AssemblyPlan::publish`, then may use
   `verify_product_assembly` with the same fresh inputs for strict stale and
   tamper verification. `verify_existing_product_assembly` is a diagnostic
   convenience for already-published trees, not a substitute for fresh host
   inputs.

## Forbidden dependencies and shortcuts

- no absolute paths or runtime reach-through into a sibling checkout;
- no ambient filesystem reads after the product capability is opened;
- no symlink-following directory traversal, dynamic registries, callback
  storage, erased invocation, or silently generated no-op capability arms;
- no whole-root content admission that bypasses `ContentManifest`, no cache
  bodies in runtime content, and no generated output as the source of a fresh
  regeneration; and
- no claim that this crate supplies the CLI command family, TypeScript
  compiler, browser host, or desktop wrapper.

## Focused verification

```bash
cargo test -p product-assembly --locked
cargo clippy -p product-assembly --all-targets --locked -- -D warnings
cargo check -p rusty-engine --locked
```

Full provider verification remains `./scripts/verify.sh`; browser and product
command evidence belongs to their owning lanes.
