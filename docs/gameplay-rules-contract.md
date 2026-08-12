# Optional gameplay rules contract

Status: accepted architecture; Rust schema-1 substrate and isolated TypeScript
authoring support implemented

This contract freezes the smallest host-neutral rules-package support justified
by the first Rusty D20 slice. It is deliberately narrower than an RPG
intermediate representation. A mechanics-only game can ignore it, and another
rules-heavy game can use it without importing Rusty D20 or adopting d20
semantics.

The owning rule remains:

> Engine can carry and correlate a downstream rules package. The downstream
> domain defines, validates, compiles, and executes its meaning.

## Dependency direction

```text
downstream TypeScript authoring (optional, build time)
                 |
                 v
normalized downstream candidate payload
                 |
                 v
gameplay-rules package envelope / dependency resolution / diagnostics
                 |
                 v
downstream Rust semantic decoder and compiler
                 |
                 +--> entity-state components
                 +--> gameplay-mechanics services (optional)
                 +--> other direct Engine services

rusty-d20 ------------------------------------------------^
  owns every d20-shaped box and every arrow that applies game meaning
```

`gameplay-rules` is an optional sibling of `entity-state` and
`gameplay-mechanics`, not a layer above either crate. Its Rust implementation
may depend on serialization, hashing, and ordinary value libraries, but it
must not depend on a downstream game, Node, TypeScript, a browser,
`entity-state`, or `gameplay-mechanics`.

`entity-state` remains the component and exact-slot mutation authority.
`gameplay-mechanics` remains the optional owner of reusable stats, tracks,
sources, effects, inventory, equipment, damage, and restoration. A downstream
compiler may produce definitions consumed by those mechanisms, but
`gameplay-rules` does not bind an opaque payload to them.

The shipped Rust paths and focused gates are mapped in
[Gameplay rules](code-map/gameplay-rules.md). The implementation stores no
package globally: admission and resolution return immutable owned values to
the caller.

## Four visible representations

The transition between authored rules and live gameplay must retain four
separate, inspectable forms:

1. **TypeScript authoring source.** Downstream builders, functions, loops, and
   content tables used before runtime. Source may be convenient and
   non-canonical.
2. **Normalized downstream candidate artifact.** Immutable JSON data with a
   downstream-owned payload schema inside the Engine envelope. Rusty D20 owns
   the d20 candidate schema.
3. **Canonical downstream Rust definitions.** Domain types produced only after
   downstream structural and semantic admission. These are not
   `gameplay-rules` types merely because their bytes arrived in its envelope.
4. **Runtime state and operations.** Ordinary entity components, direct named
   services, explicit downstream orchestration, and typed operation receipts.

Structural package admission is not semantic admission. A caller must be able
to construct the same candidate directly in Rust or decode a checked artifact
without installing or executing Node or TypeScript at runtime.

## Upstream ownership

The Rust `gameplay-rules` crate owns only:

- stable domain, package, source, and optional subject identities;
- a positive package version and exact package dependencies;
- one strict, bounded, versioned package envelope containing an opaque JSON
  payload;
- deterministic canonical encoding and a SHA-256 content fingerprint;
- source records and bounded provenance correlations;
- bounded, source-correlated diagnostics;
- package-set validation and deterministic dependency ordering; and
- direct Rust construction, strict decode, admission, canonical encode, and
  package-set resolution helpers.

These facilities are semantic-neutral because no accepted value names a game
operation and the payload remains opaque. They do not create a shared
definition registry or a runtime package store.

The first public Rust surface is:

```text
RuleDomainId
RulePackageId
RuleSourceId
RuleSubjectId
RuleVersion
RulePackageDependency
RuleSource
RuleProvenance
RulePackageCandidate
AdmittedRulePackage
ResolvedRulePackages
RuleFingerprint
RuleDiagnostic
RuleDiagnosticReport
RulePackageError
RulePackageSetError
RuleDiagnosticError

admit_rule_package(candidate)
decode_rule_package(bytes)
decode_canonical_rule_package(bytes)
encode_rule_package(package)
resolve_rule_packages(packages)
```

Names may gain conventional Rust module qualification during implementation,
but their responsibilities and separation are frozen here.

`decode_rule_package` accepts a structurally valid non-canonical document and
returns the admitted canonical package. `decode_canonical_rule_package`
additionally requires the supplied bytes to equal canonical encoding.
`encode_rule_package` emits the canonical bytes. None of these functions
invokes a downstream semantic compiler.

## Envelope and identity

The schema-1 artifact has the following logical fields:

```text
kind: "rusty.gameplay-rules.package"
schemaVersion: 1
domain: RuleDomainId
package: RulePackageId
version: RuleVersion
dependencies: [RulePackageDependency]
sources: [RuleSource]
provenance: [RuleProvenance]
payload: JSON value
```

Unknown envelope fields are rejected. The payload is required, may use any
finite JSON shape within the shared bounds, and is never reinterpreted by
Engine.

All IDs are non-empty, trimmed, printable ASCII strings of at most 128 bytes.
They are case-sensitive and retain authored spelling. `RuleVersion` is a
positive JavaScript-safe integer. A package identity is
`(domain, package, version)`.

Dependencies name the exact domain, package, and version they require. They
may optionally pin the dependency's canonical fingerprint. Version ranges and
implicit "latest" resolution are excluded because they make the same package
set resolve differently over time.

Source records name a stable `RuleSourceId` and a bounded logical path; they do
not grant filesystem access. A provenance entry correlates one
`RuleSubjectId` in the downstream payload to one source plus an optional
bounded line and column. Subject IDs are opaque correlation keys rather than
definition identities owned by Engine.

Duplicate package identities, dependencies, source identities, provenance
subject identities, or JSON object keys are rejected. One resolved set cannot
contain two versions of the same `(domain, package)`. A package cannot depend
on itself. Source line and column values, when present, are positive
JavaScript-safe integers.

## Canonical form and fingerprint

Canonical encoding is UTF-8 JSON with:

- the fixed envelope field order shown above;
- recursively sorted payload object keys;
- key ordering by the UTF-8 bytes of each decoded key, with no Unicode
  normalization;
- array order preserved because downstream schemas may assign it meaning;
- dependencies sorted by domain, package, and version;
- sources sorted by source identity;
- provenance sorted by subject identity;
- the shortest normal JSON representation for safe integers and booleans;
- minimal JSON escaping for quotation mark, reverse solidus, and control
  characters while other Unicode scalar values remain UTF-8;
- no non-finite or floating-point number representation;
- no byte-order mark, duplicate object key, unpaired surrogate, or non-Unicode
  input;
- no insignificant whitespace; and
- exactly one trailing line feed.

The shared envelope permits JSON integers only in
`[-9_007_199_254_740_991, 9_007_199_254_740_991]` and rejects fractional
numbers. A downstream schema that needs wider integers, exact ratios, or
decimal values must encode and validate them explicitly. This keeps
canonicalization lossless in both Rust and TypeScript and independent of host
floating-point behavior.

The package fingerprint is lowercase hexadecimal SHA-256 over the complete
canonical bytes. It is content evidence for dependency pinning, caches, and
diagnostics, not a substitute for the package identity or downstream
compatibility policy.

## Resolution and failure behavior

`resolve_rule_packages` validates the entire supplied set before returning:

1. package identities are unique;
2. each `(domain, package)` has only one supplied version;
3. every dependency resolves to exactly one supplied package;
4. the exact version and optional fingerprint match;
5. the dependency graph is acyclic; and
6. all aggregate bounds are satisfied.

The result uses deterministic topological order. When several packages are
ready, `(domain, package, version)` lexical order breaks the tie. A rejected
set publishes no partial result.

Public failures preserve actionable identity rather than collapsing to a
string. The initial families are:

- malformed UTF-8 or JSON;
- wrong artifact kind or unsupported schema version;
- unknown or missing envelope field;
- invalid identity, version, source location, or integer;
- artifact, payload, collection, nesting, string, or diagnostic quota
  exceeded;
- duplicate package, dependency, source, or provenance identity;
- conflicting versions of one logical package;
- non-canonical bytes when canonical input was required;
- fingerprint mismatch;
- missing or version-mismatched dependency;
- dependency cycle; and
- downstream diagnostic report invalid or over quota.

Every error identifies the package when known and a deterministic logical path.
Errors associated with a payload subject may carry its source correlation.
Ordering follows package identity, then logical path, then stable error code.

The package helpers never publish to a component store, catalog, global
registry, filesystem, or runtime. Callers receive a complete candidate result
or a complete diagnostic report and decide whether to publish downstream
compiled definitions.

## Bounds

The first schema uses fixed provider bounds:

| Surface | Maximum |
|---|---:|
| encoded artifact | 4 MiB |
| packages in one resolved set | 64 |
| canonical bytes across one resolved set | 16 MiB |
| dependencies per package | 32 |
| dependencies across one resolved set | 512 |
| sources per package | 64 |
| sources across one resolved set | 1,024 |
| provenance entries per package | 4,096 |
| provenance entries across one resolved set | 16,384 |
| diagnostics in one report | 256 |
| identity bytes | 128 |
| diagnostic code bytes | 64 |
| source path bytes | 512 |
| diagnostic logical path bytes | 512 |
| diagnostic message bytes | 2,048 |
| JSON nesting depth | 64 |
| JSON nodes | 100,000 |
| JSON nodes across one resolved set | 400,000 |
| one JSON string | 1 MiB |

Admission checks outer byte and collection bounds before expanding nested
values. JSON depth, node, and string costs include the opaque payload.
Aggregate package-set work is checked before graph traversal. Rejection does
not retain a partial package or diagnostic collection.

## Diagnostics contract

A downstream compiler may use `RuleDiagnosticReport` to return errors through
the same source-correlation vocabulary. A diagnostic has a stable bounded
code, severity (`error` or `warning`), deterministic logical path, bounded
message, optional package identity, and optional subject/source correlation.

Messages are for people; callers branch on the code and structured
correlation. Engine validates and sorts a completed report but does not define
domain diagnostic codes. A report containing any error is not an admitted
downstream definition set. Warnings do not silently authorize publication:
the downstream compiler still returns its own explicit success value.

## Explicit exclusions

The support surface contains no Engine-owned:

- formula, predicate, operation, action, effect, sequence, condition, or
  behavior nodes;
- generic definition-reference vocabulary beyond package dependencies and
  opaque diagnostic subjects;
- mechanics-binding enum or source-to-service compiler;
- evaluator, virtual machine, callback, closure, or script host;
- mutable package registry, service locator, plugin mechanism, or global cache;
- runtime session, scheduler, turn owner, tick owner, event bus, replay log, or
  command router;
- d20 rolls, abilities, defenses, attacks, saves, classes, feats, spells,
  slots, reactions, or condition vocabulary; or
- complete save, content repository, product migration, renderer, or UI
  policy.

Ordinary new downstream semantic primitives must not require an Engine enum
edit. If a later concrete consumer proves a genuinely shared mechanism, it
gets its own focused design and review rather than being smuggled into the
opaque envelope.

## Isolated TypeScript ownership

The optional build-time workspace is rooted at `rules/`, separate from the
ordinary Rust provider and from `render/` and `studio/`. Its initial packages
are:

- `@rusty-engine/gameplay-rules-contracts`: generated schema-1 envelope,
  identity, bound, and diagnostic DTOs plus strict decoding; and
- `@rusty-engine/gameplay-rules-authoring`: semantic-neutral helpers that
  normalize an already downstream-shaped JSON payload, canonicalize the
  envelope, correlate sources, and emit immutable candidate bytes.

Rust exports the small shared contract and bounds consumed by generation.
Generated files are checked for drift. TypeScript does not generate domain
schemas, validate d20 semantics, execute rules, call runtime services, or own
mutable gameplay state.

`scripts/verify-rules.sh` owns the isolated package installation and tests,
generated-contract drift, strict decode/canonicalization fixtures, and the
Node-free boundary audit. CI exposes it as `verify-rules`.
`scripts/verify-all.sh` may aggregate that isolated gate, while
`scripts/verify.sh` remains Node-free.

This workspace needs no browser test. Rusty D20 owns its real browser and
product proof.

## First-consumer proof

Rusty D20 must prove the support surface with a bounded but real slice:

- its own candidate payload schema and Rust semantic compiler;
- direct Rust construction and canonical artifact admission producing the
  same compiled definitions;
- a small d20 ability/check-or-attack/defense/damage/effect sequence;
- malformed, unknown, incompatible, cyclic, oversized, and semantically
  invalid candidates rejected with source-correlated diagnostics;
- admitted definitions driving ordinary `entity-state`,
  `gameplay-mechanics`, and deterministic RNG calls;
- one TypeScript-authored content variation requiring no Engine source edit
  and no new Rust semantic primitive; and
- a real durable UI path, authoritative save/reopen, and rendered result in
  the downstream product.

The in-repository infrastructure/builder fixture is a separate falsification
probe for direct mechanics composition. It is not a second rules consumer and
does not justify broadening this contract.

That historical first-consumer proof is complete at reviewed Rusty D20 revision
`793dd6037d99091d958f675c98b35320b9aca307`, using reviewed Engine revision
`fb608e323a8b44a55195f5720101224ff37fd5db` and exact `rusty-engine-ui` donor
revision `68ddfa5430ec3bc2cf7ca96963982db9511e79ba`. The downstream proof includes
two source-correlated TypeScript-authored compositions, Node-free strict Rust
compilation, canonical mechanics/RNG execution, a real Rust-host browser path,
and fresh-process save/reopen. Its content-only addition regression uses the
published D20 authoring surface without an Engine edit or new Rust semantic
primitive. See the
[gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md)
for the exact review/gate ledger and non-claims.

## Compatibility posture

Schema 1 is strict. Unknown envelope fields and unsupported schema versions
fail closed. Downstream payload compatibility belongs to its domain and
package version. A future Engine envelope schema requires an explicit decoder,
canonicalization rule, migration posture, and cross-language fixture; it is
not inferred from unknown fields.

Rusty D20 may evolve its candidate schema without changing Engine when the
opaque payload remains within this contract. Mechanics-only consumers incur no
dependency or runtime cost.

## Donor decision

The pinned `asha-rpg` source is behavioral evidence, not a topology template.
The bounded disposition is recorded in
[`migration/gameplay-rules-donor/disposition.tsv`](../migration/gameplay-rules-donor/disposition.tsv).

Retained lessons are strict package admission, exact dependency graphs,
canonical JSON, source provenance, typed diagnostics, executable-value
rejection, bounded compilation, and fail-before-publication. Formula,
predicate, program, operation, registry, semantic compiler, session, replay,
content-patch, generated RPG vocabulary, and runtime ownership remain
downstream or excluded.

No donor implementation file is copied verbatim and no donor checkout is a
build or verification dependency.
