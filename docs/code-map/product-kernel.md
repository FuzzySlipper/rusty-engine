# Product Kernel assembly

`rust/crates/product-kernel` owns the narrow downstream Rust extension lane for
product-specific systems and operations. It links a closed Product Kernel
declaration to the admitted Product Model composition before a runtime
lifecycle exists. The declaration binds concrete downstream contract types to
an immutable capability descriptor catalog and generated owner enum; the
assembly retains no handler, callback, registry, erased value, or dynamic
loader.

Schemas and migrations are separate offline declarations. Their identities and
concrete contract types are exported for authoring and migration tooling but
are not `kernel.*` live capability targets. A `Migration` capability entry is
rejected by Product Kernel validation, and a migration target absent from the
live descriptor catalog cannot enter a schedule.

## Primary paths

- [`product-kernel/src/lib.rs`](../../rust/crates/product-kernel/src/lib.rs)
- [`product-kernel/src/declaration.rs`](../../rust/crates/product-kernel/src/declaration.rs)
- [`product-kernel/src/assembly.rs`](../../rust/crates/product-kernel/src/assembly.rs)
- [`product-kernel/src/context.rs`](../../rust/crates/product-kernel/src/context.rs)
- [`product-kernel/src/render.rs`](../../rust/crates/product-kernel/src/render.rs)
- [`product-kernel/src/tests.rs`](../../rust/crates/product-kernel/src/tests.rs)

## Source-linked declaration

The legacy Product Layout form, `kernel.entry`, links one explicit Rust source
module. The current package form, `kernel.package = "kernel/Cargo.toml"`, is
for an ordinary downstream library crate (including its internal modules and
bounded local kernel crates). It must export the same fixed
`RustyProductRuntime` type at the crate root. Product Assembly copies the
complete admitted `kernel/` lane and generated code depends on the copied
package as a normal Cargo dependency; it never reduces package mode to a
`#[path]` import.

Package Cargo manifests are admitted without an ambient workspace, registry,
build-script, or target-specific dependency graph. Every non-Engine local
dependency must resolve within `kernel/`; the fixed `rusty-engine` dependency
is rewritten to the explicit generated Assembly facade path. The Engine facade
re-exports `serde` and `serde_json` for product DTO serialization, avoiding a
second package registry graph. Symlinks, lane escapes, malformed identities,
and a missing fixed runtime export fail before product publication.

`product_kernel_declaration!` is one source declaration for a product's closed
capabilities, schema identities, and offline migrations. Each capability names
its concrete `ProductKernelCapabilityContract`, whose associated
`Snapshot`/`Request`/`Result`/`Error` types remain downstream-owned. The macro
emits:

- a closed owner enum for ordinary downstream `match` expressions;
- exact immutable `ProductKernelCapabilityDescriptor` values for the
  `product_model` linker;
- schema and migration descriptors with concrete type identities; and
- a versionless deterministic JSON contract and TypeScript module renderer.

The JSON and TypeScript renderers sort capabilities, schemas, and migrations by
their stable identities. Declaration order therefore does not alter generated
contract bytes. Compatibility follows actual changes to this current contract;
there is no independent version field.

For the closed DTO boundary only, `product-kernel` re-exports `serde` and
`serde_json`. A bounded package kernel that depends on `rusty-engine` uses
`rusty_engine::product_kernel::serde` and
`rusty_engine::product_kernel::serde_json`, including
`#[serde(crate = "rusty_engine::product_kernel::serde")]` on derives. The
broad `rusty-engine` facade does not re-export serialization crates.

## Product Assembly and contexts

`ProductAssembly::<Declaration>::link(admitted, selections)` first validates
the declaration, then wraps
`product_model::link_admitted_product_composition`, and finally requires every
Product Kernel binding to have exactly one generated-owner selection. Target,
kind, concrete contract type, availability, access, and payload facts are
checked before the assembly value is published. Missing/stale/type/kind
failures are pre-start failures.

`ProductSystemContext` accepts only a validated `RuntimePhase::Schedule` token;
`ProductOperationContext` accepts only `RuntimePhase::Mutation`. Both carry
borrowed product-owned snapshot/request values and no scheduler or mutation
authority. Downstream functions keep their closed return and error types.
`ProductProjectionContext` accepts only a validated `RuntimePhase::Projection`
token and carries an immutable product snapshot plus the admitted simulation
step. It is the typed input boundary for an owned downstream UI DTO; it carries
no renderer, host, callback, clock, scheduler, or mutation authority.

## TypeScript generation

`ProductKernelDeclaration::contract_json()` and
`ProductKernelDeclaration::contract_typescript()` are checked `Result` paths;
both run the complete declaration validator before emitting. The TypeScript
method emits a product-local module that imports `bindProductKernelCatalog` from
`@rusty-engine/runtime-composition-authoring`, exports the frozen catalog,
closed `ProductKernelTarget` type, and `productKernelCapability` helper. A
downstream `generate` command writes this module and `generate:check` compares
bytes against a fresh Rust export; the Rust provider has no Node or browser
dependency.

## Forbidden shortcuts and focused verification

- no `Any`, `TypeId`, trait-object handler table, dynamic discovery, plugin ABI,
  generic string/JSON invocation, alternate scheduler, or alternate mutation
  path;
- no live schedule target for offline schema migrations; and
- no schema-version family or compatibility churn beyond the actual current
  source contract.

```bash
cargo test -p product-kernel --locked
cargo clippy -p product-kernel --all-targets --locked -- -D warnings
cargo test -p rusty-engine --locked
```
