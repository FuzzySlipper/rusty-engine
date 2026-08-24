# Product model schema

## Ownership

`rust/crates/product-model` owns host-neutral validation for the current
`rusty.toml` Product Layout, immutable Compiled Composition artifact, their
immutable product-composition admission, and closed pre-start capability
linkage. It defines fixed authored and generated lanes, bounded identities and
payloads, strict decoding, typed references, deterministic encoding, and
structured diagnostics.

It does not discover or read product files, compile TypeScript, admit runtime
content, execute schedules, invoke a capability, mutate live state, build a
Product Assembly, run a host, or generate desktop wrappers. The catalog is
pure metadata and immutable linkage, never a handler table, service locator,
plugin registry, or generic dispatch API. Those capabilities are separate
campaign milestones. The current schema deliberately has no
compatibility-version family.

## Primary paths

- [`product-model/src/lib.rs`](../../rust/crates/product-model/src/lib.rs)
- [`product-model/src/manifest.rs`](../../rust/crates/product-model/src/manifest.rs)
- [`product-model/src/composition.rs`](../../rust/crates/product-model/src/composition.rs)
- [`product-model/src/admission.rs`](../../rust/crates/product-model/src/admission.rs)
- [`product-model/src/capability_catalog.rs`](../../rust/crates/product-model/src/capability_catalog.rs)
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
| `src/capability_catalog.rs` | Static Engine descriptor table, immutable caller-supplied Product Kernel descriptor validation, and complete pre-start target/use/access/budget linkage; no invocation, registry, or scheduler |
| `src/contract.rs` | Current Rust-owned descriptor for TypeScript generation and schema-drift checks; no version family |
| `src/path.rs` | Lexical product-relative path grammar and component-aware relationships |
| `src/diagnostic.rs` | Bounded machine-readable schema diagnostics |
| `tests/contract.rs` | Valid/invalid fixtures, fail-closed parsing, bounds, references, and deterministic encoding |

The complete downstream facade re-exports the crate as
`rusty_engine::product_model` without wrappers.

## Input descriptor admission

`intentDescriptors` are the one closed semantic target for input. Each owns
its stable intent id, digital/axis value kind, one admitted capability binding,
and that capability's bounded payload. `inputMap` is deliberately narrower:
an authored mapping id, one intent reference, and one explicit typed physical
trigger. It cannot carry another capability or opaque trigger meaning.

The trigger grammar is closed and generated into Runtime Composition: keyboard
press/hold/release (including a bounded keyboard chord), pointer buttons and
axes, wheel axes, and bounded selected-controller buttons/axes. The keyboard
catalog is exactly A–Z, 0–9, space/enter/escape, and left/right
shift/control/alt; hosts must not emit un-authorable controls. Context is a
bounded Product identity, not browser focus. Admission resolves every mapping
to its descriptor and rejects missing references, a digital/axis mismatch,
invalid chord, or an unavailable input-map capability before the runtime lane
is constructed.

Direct product UI claims name the descriptor id rather than a mapping. This
lets direct UI and multiple physical triggers converge on precisely the same
immutable capability/payload readout without selecting an arbitrary mapping.
See [runtime input](runtime-input.md) for the post-admission lane.

## Forbidden dependencies and shortcuts

- no filesystem, process, network, browser, renderer, TypeScript, Studio, or
  downstream-product dependency;
- no runtime evaluator, scheduler, clock, mutation service, service locator,
  registry, host, or wrapper generator;
- no callbacks, handlers, `TypeId`/`Any`, trait-object dispatch, mutable global
  registration, plugin discovery, or generic method-name invocation in
  capability linkage;
- no schedule conflict, dependency-order, runtime target resolution, or
  execution interpretation; admission only preserves validated declarations
  while the catalog resolves closed descriptor identities only for later named
  owners; and
- no schema-version, compatibility-matrix, or migration scaffolding before an
  independently evolving producer/consumer boundary exists; and
- no interpretation of product-owned opaque payload meaning.

## Closed capability linkage

`link_admitted_product_composition(...)` accepts one already-admitted
composition and the complete immutable Product Kernel descriptor slice emitted
by a source-linked Product Assembly. It resolves every declared binding before
startup, including an otherwise unreferenced binding. Engine targets resolve to
the closed `EngineCapability` enum; Kernel targets resolve to a deterministic
identity ordinal rather than caller declaration position. No unresolved or
partial linked value is returned.

Each descriptor carries its stable target/identity, kind (`system`,
`operation`, `query`, `projection`, or `migration`), supported current
composition uses, link availability, exact schedule access declarations,
compact-JSON payload budget, owner, source, and logical path. The initial
Engine table intentionally contains only the existing named
`engine.render.entity-project` projection (`EntityRenderProjector::project`).
The wider kind set lets a generated Product Kernel descriptor state its own
closed semantics without falsely claiming that Product Model executes it.

An authored target may appear in exactly one `capabilityBindings` entry. This
prevents aliases from obscuring source diagnostics or producing two supposedly
distinct static bindings to one owner; input, schedule, and timeline entries
reuse that one authored binding id. `engine.*` and `kernel.*` remain distinct
qualified target spaces even when their local names match.

The payload limit is measured only during Rust linkage with
`serde_json::to_vec` over the admitted `serde_json::Value` (compact Rust JSON,
without whitespace). It is an assembly admission budget, not an assertion
about canonical artifact byte length or a cross-language payload-size protocol.

## Focused verification

```bash
cargo test -p product-model --locked
cargo clippy -p product-model --all-targets --locked -- -D warnings
pnpm --dir rules run generate:check
pnpm --dir rules run verify
python3 scripts/dependency_boundary_check.py
./scripts/verify-rust-sdk-consumer.sh
```

Full provider verification remains `./scripts/verify.sh`.
