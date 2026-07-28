# Rusty Engine documentation

Start with [design.md](design.md) for the canonical architecture and
[agent-code-atlas.md](agent-code-atlas.md) to route a question to its owning
crate, package, or product boundary.

Rusty Engine's repository docs are deliberately self-contained. Den may hold
current planning and review state, but understanding the committed provider
does not require Den access.

## Root documents

| Document | Purpose |
|---|---|
| [design.md](design.md) | Canonical provider architecture, authority, dependency direction, and promotion rules |
| [agent-code-atlas.md](agent-code-atlas.md) | Agent-oriented navigation across code owners, public surfaces, and gates |
| [README.md](README.md) | This documentation index |

## Topic directories

| Directory | Contents |
|---|---|
| [topics/development/](topics/development/) | Rust source organization and implementation style |
| [topics/voxel/](topics/voxel/) | Stored voxel formats, conversion, animation, and runtime admission |

Several operational contracts intentionally remain at the docs root because
checked migration ledgers and isolated workspace gates refer to their stable
paths:

| Document | Purpose |
|---|---|
| [inspection-and-diagnostics.md](inspection-and-diagnostics.md) | Read-only inspection contract |
| [gameplay-rules-contract.md](gameplay-rules-contract.md) | Optional semantic-neutral Rust package and isolated TypeScript authoring, resolution, provenance, and diagnostics contract |
| [rendering-successor-contract.md](rendering-successor-contract.md) | Shared renderer transfer and ownership contract |
| [rendering-operations.md](rendering-operations.md) | Renderer verification and operational limits |
| [studio-migration-contract.md](studio-migration-contract.md) | Studio parity and owner-adoption contract |
| [studio-adapter-protocol.md](studio-adapter-protocol.md) | Closed external-project adapter protocol |
| [studio-downstream-entity-inspector-extensions.md](studio-downstream-entity-inspector-extensions.md) | Proposed typed downstream Entity inspector composition boundary and implementation order |

## Code maps

The [agent code atlas](agent-code-atlas.md) links coarse owner maps under
[`code-map/`](code-map/). Each map uses the same questions:

- What does this area own?
- What does it explicitly not own?
- Which source paths and public surfaces matter?
- Which checks prove a change?
- Which mistakes most often cross its boundary?

These are hand-curated maps of stable ownership, not generated file listings or
a mirror of the current task queue.

## Migration and review evidence

| Directory | Contents |
|---|---|
| [migration/](migration/) | Donor provenance, extraction contracts, experiment evidence, and durable migration decisions |
| [reviews/](reviews/) | Point-in-time review reports and recommendations |

Migration documents explain why the standalone provider has its current shape.
They are useful evidence, but [design.md](design.md), current code, and current
tests take precedence when describing implemented behavior.

## I need to...

| Goal | Start here |
|---|---|
| Understand the provider boundary | [design.md](design.md) |
| Find the crate or package that owns a behavior | [agent-code-atlas.md](agent-code-atlas.md) |
| Change entity components or atomic entity mutation | [Entity state and state machines](code-map/entity-state-and-state-machines.md) |
| Change stats, tracks, effects, items, equipment, damage, or restoration | [Gameplay mechanics](code-map/gameplay-mechanics.md) |
| Package a downstream-authored rules candidate without giving Engine its meaning | [Gameplay rules](code-map/gameplay-rules.md) |
| Check active provider limitations and scheduled certification | [Known limitations](known-limitations.md) |
| Change voxel collision, navigation, motion, or edits | [Spatial mechanisms](code-map/spatial-mechanisms.md) |
| Change a stored asset, scene, prefab, or import | [Content, assets, and scenes](code-map/content-assets-and-scenes.md) |
| Change voxel conversion or object playback | [Voxel assets and conversion](code-map/voxel-assets-and-conversion.md) |
| Change a retained Rust render frame | [Rust render model and projection](code-map/rust-render-model-and-projection.md) |
| Change Three/WebGL or a renderer host | [Renderer workspace and hosts](code-map/renderer-workspace-and-hosts.md) |
| Change the first-party authoring UI | [Studio](code-map/studio.md) |
| Plan a downstream typed Entity inspector panel | [Downstream Entity inspector extensions](studio-downstream-entity-inspector-extensions.md) |
| Add inspection or diagnostics | [Inspection and diagnostics](code-map/inspection-and-diagnostics.md) |
| Select or adapt donor behavior | [Migration cluster ledger](migration/migration-cluster-ledger.md) |
