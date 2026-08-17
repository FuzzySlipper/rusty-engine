# Gameplay rules

## Purpose

Route work involving the optional, semantic-neutral package boundary used by
rules-heavy downstream games. This crate carries an opaque authored payload
through bounded admission, canonical bytes, exact dependencies, provenance,
and diagnostics without defining what any rule means.

## Owns

- `gameplay-rules`: stable domain, package, source, and subject identities;
  positive versions; exact dependencies; and immutable admitted packages.
- Strict schema-1 safe-integer and schema-2 binary64 JSON decoding,
  deterministic canonical encoding, and lowercase SHA-256 fingerprints.
- Complete package-set validation and deterministic topological ordering.
- Bounded source-correlated diagnostic reports supplied by downstream semantic
  compilers.
- Exact limits and typed failure identity for every provider-owned step.

## Does not own

- A formula, predicate, operation, condition, effect, behavior, definition, or
  d20 vocabulary.
- Semantic validation or compilation of the opaque payload.
- A package registry, component store, mutable catalog, evaluator, VM, script
  host, scheduler, event bus, runtime session, replay log, or complete save.
- Binding payload values to `entity-state`, `gameplay-mechanics`, or any other
  service.
- Filesystem layout, loading policy, package publication, project persistence,
  TypeScript execution, runtime float precision, or product UI.

## Primary paths

- [`gameplay-rules/src/lib.rs`](../../rust/crates/gameplay-rules/src/lib.rs)
- [`gameplay-rules/src/identity.rs`](../../rust/crates/gameplay-rules/src/identity.rs)
- [`gameplay-rules/src/package.rs`](../../rust/crates/gameplay-rules/src/package.rs)
- [`gameplay-rules/src/json.rs`](../../rust/crates/gameplay-rules/src/json.rs)
- [`gameplay-rules/src/resolve.rs`](../../rust/crates/gameplay-rules/src/resolve.rs)
- [`gameplay-rules/src/diagnostic.rs`](../../rust/crates/gameplay-rules/src/diagnostic.rs)
- [`gameplay-rules/src/error.rs`](../../rust/crates/gameplay-rules/src/error.rs)
- [`gameplay-rules/src/contract.rs`](../../rust/crates/gameplay-rules/src/contract.rs)
- [`gameplay-rules/tests/contract.rs`](../../rust/crates/gameplay-rules/tests/contract.rs)
- [`gameplay-rules-contracts`](../../rules/packages/gameplay-rules-contracts/src/index.ts)
- [`gameplay-rules-authoring`](../../rules/packages/gameplay-rules-authoring/src/index.ts)
- [`Rust-owned contract generator`](../../rules/scripts/generate-contract.mjs)
- [`rules workspace boundary audit`](../../rules/scripts/check-boundaries.mjs)
- [`schema-1 canonical fixture`](../../fixtures/gameplay-rules/package-v1.canonical.json)
- [`schema-2 binary64 fixture`](../../fixtures/gameplay-rules/package-v2-binary64.canonical.json)
- [Optional gameplay rules contract](../gameplay-rules-contract.md)
- [Current downstream facade and Studio boundary](../topics/development/downstream-engine-revisions.md)
- [Completed first-consumer evidence](../gameplay-mechanics-campaign-closeout.md)
- [`gameplay-rules` donor disposition](../../migration/gameplay-rules-donor/disposition.tsv)

## Public composition

A downstream Rust compiler may construct a candidate directly or decode a
checked artifact. Both paths return the same `AdmittedRulePackage`:

```rust
let admitted = admit_rule_package(candidate)?;
let canonical = encode_rule_package(&admitted);
let decoded = decode_canonical_rule_package(&canonical)?;
assert_eq!(decoded, admitted);
```

`decode_rule_package` accepts structurally valid non-canonical input and
returns its canonical admitted representation.
`decode_canonical_rule_package` additionally requires byte-for-byte canonical
input. Admission sorts dependencies, sources, and provenance; recursively
sorts opaque payload object keys; preserves array order; applies the selected
schema's safe-integer or finite-binary64 policy; and caches the complete
canonical bytes and their SHA-256 fingerprint.

Package admission does not make payload meaning valid. A downstream domain
compiler must inspect `payload()`, produce its own Rust definitions, and decide
whether and where to publish them. `RuleDiagnosticReport` gives that compiler
bounded codes, paths, messages, package identity, and optional subject/source
correlation without giving Engine ownership of its codes.

Multiple admitted packages may be passed to `resolve_rule_packages`. The
resolver preflights all aggregate costs, exact identities, logical-version
conflicts, dependencies, optional fingerprint pins, and cycles before
returning one owned `ResolvedRulePackages`. Its order is topological with
lexical identity as the ready-set tie breaker. It retains no global or mutable
package state.

## Admission and persistence boundary

The outer artifact and known envelope collections are bounded before an
over-limit nested value is expanded. JSON depth, node, string, integer, and
canonical-byte limits are checked with typed failures. Direct construction
uses the same validation and a bounded canonical writer, so it cannot allocate
an oversized canonical intermediate merely to reject it afterward.

Canonical bytes are suitable content evidence for a downstream cache,
dependency pin, or durable file. Engine does not choose a path, load a file,
publish a package, migrate a downstream payload schema, or persist compiled
definitions. A schema-1 or schema-2 artifact is a package candidate, not a
complete game save and not proof of semantic admission.

## Acceptance gates and fixtures

```bash
cargo test -p gameplay-rules --locked
cargo clippy -p gameplay-rules --all-targets --locked -- -D warnings
./scripts/verify-rules.sh
./scripts/check-gameplay-rules-donor-disposition.sh
./scripts/check-doc-links.sh
./scripts/audit-standalone.sh
./scripts/verify.sh
```

The Rust and TypeScript contract suites cover direct/artifact convergence,
byte-identical cross-language canonical emission, canonical fixture
fingerprinting, duplicate JSON keys, malformed UTF-8 and Unicode, unsafe
numbers, unknown fields, duplicate metadata, malformed provenance, exact
dependency failures, deterministic cycles and diagnostics, plain-data-only
authoring, and every per-package and aggregate exact/one-over bound.

Ordinary provider verification remains Node-free. The optional isolated
TypeScript authoring workspace and its `verify-rules` gate are a separate
surface and add no runtime dependency to this Rust crate.

## Common agent mistakes

- Treating structural package admission as downstream semantic admission.
- Adding a shared rule node or d20 enum because one downstream payload uses it.
- Publishing admitted packages into a global registry or component store.
- Resolving an implicit latest version instead of the exact dependency.
- Hashing authored input rather than complete canonical bytes.
- Reimplementing canonical JSON through ordinary serializer defaults that
  lose duplicate keys, use unstable field order, accept non-finite/underflowed
  numbers, or format binary64 differently from the schema-2 contract.
- Using source paths as filesystem authority.
- Persisting callbacks, closures, executable values, or compiled runtime state
  in the opaque envelope.

## Follow-up routing

- Runtime stats, tracks, effects, items, and damage:
  [Gameplay mechanics](gameplay-mechanics.md).
- Entity component publication:
  [Entity state and state machines](entity-state-and-state-machines.md).
- Durable project files and load/save policy:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Frozen cross-language and downstream ownership:
  [Optional gameplay rules contract](../gameplay-rules-contract.md).
