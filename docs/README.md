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
| [gameplay-mechanics-campaign-closeout.md](gameplay-mechanics-campaign-closeout.md) | Exact GM0-GM7 mechanics/rules consumer evidence, measured limits, donor pins, and stopping point |
| [textured-voxel-campaign-closeout.md](textured-voxel-campaign-closeout.md) | VTX0-VTX6 ownership, measured geometry/resource costs, exact consumer proof, and stopping point |
| [topics/fps-character-controller-survey.md](topics/fps-character-controller-survey.md) | Task 6847 source survey, feature matrix, licensing posture, and adopted transfer decisions |
| [topics/fps-character-controller-proposal.md](topics/fps-character-controller-proposal.md) | Adopted host-neutral controller design, current Rust API, ownership, and remaining proof routes |
| [topics/gameplay/downstream-adoption.md](topics/gameplay/downstream-adoption.md) | Disposition-first downstream adoption of gameplay rules, TypeScript authoring, mechanics, resolution, intents, and diagnostics |
| [topics/development/upstream-promotion-and-authoring-dsl.md](topics/development/upstream-promotion-and-authoring-dsl.md) | Proactive Engine promotion criteria and the optional TypeScript gameplay authoring DSL boundary |
| [README.md](README.md) | This documentation index |

## Topic directories

| Directory | Contents |
|---|---|
| [topics/development/](topics/development/) | Rust source organization, implementation style, and downstream integration boundaries |
| [topics/gameplay/](topics/gameplay/) | Downstream gameplay adoption, authoring, mechanics, resolution, and migration guidance |
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
| [topics/development/downstream-renderer-and-studio.md](topics/development/downstream-renderer-and-studio.md) | Central downstream Rust facade, bundled rich-DOM application host, renderer, and Engine-hosted Studio boundary |
| [topics/development/verification-and-ci.md](topics/development/verification-and-ci.md) | Verification ownership, CI routing, single-pass renderer evidence, and ordinary task workflow |
| [studio-migration-contract.md](studio-migration-contract.md) | Studio parity and owner-adoption contract |
| [studio-adapter-protocol.md](studio-adapter-protocol.md) | Closed external-project adapter protocol |
| [topics/studio-service.md](topics/studio-service.md) | Persistent generic Studio service operations |
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
| Structure or migrate a downstream game's gameplay code and optional TypeScript authoring | [Downstream gameplay adoption](topics/gameplay/downstream-adoption.md) |
| Decide whether a mechanism belongs upstream or in a TypeScript authoring DSL | [Upstream promotion and authoring DSL](topics/development/upstream-promotion-and-authoring-dsl.md) |
| Package a downstream-authored rules candidate without giving Engine its meaning | [Gameplay rules](code-map/gameplay-rules.md) |
| Audit the completed mechanics/rules campaign and exact consumers | [Gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md) |
| Audit the completed textured-voxel campaign and exact consumer | [Textured voxel campaign closeout](textured-voxel-campaign-closeout.md) |
| Use Rusty Engine from a downstream repository or open it in Studio | [Downstream renderer and Studio boundary](topics/development/downstream-renderer-and-studio.md) |
| Check active provider limitations and scheduled certification | [Known limitations](known-limitations.md) |
| Change voxel collision, navigation, motion, or edits | [Spatial mechanisms](code-map/spatial-mechanisms.md) |
| Change the reusable FPS controller, character-motion publication, or look math | [FPS controller design](topics/fps-character-controller-proposal.md), [survey](topics/fps-character-controller-survey.md), and [spatial mechanisms](code-map/spatial-mechanisms.md) |
| Change a stored asset, scene, prefab, or import | [Content, assets, and scenes](code-map/content-assets-and-scenes.md) |
| Change voxel conversion or object playback | [Voxel assets and conversion](code-map/voxel-assets-and-conversion.md) |
| Change runtime voxel tiles or atlas mapping | [Runtime voxel surface textures](topics/voxel/voxel-surface-textures.md) |
| Change a retained Rust render frame | [Rust render model and projection](code-map/rust-render-model-and-projection.md) |
| Change the complete Rust facade or Rust-only renderer boundary | [Rust SDK capability index](rust-sdk-capabilities.md) |
| Integrate a downstream game with rendering or Studio | [Downstream renderer and Studio boundary](topics/development/downstream-renderer-and-studio.md) |
| Change Three/WebGL or a renderer host | [Renderer workspace and hosts](code-map/renderer-workspace-and-hosts.md) |
| Choose the focused or CI verification gate for a change | [Verification and CI ownership](topics/development/verification-and-ci.md) |
| Change the first-party authoring UI | [Studio](code-map/studio.md) |
| Plan a downstream typed Entity inspector panel | [Downstream Entity inspector extensions](studio-downstream-entity-inspector-extensions.md) |
| Add inspection or diagnostics | [Inspection and diagnostics](code-map/inspection-and-diagnostics.md) |
| Select or adapt donor behavior | [Migration cluster ledger](migration/migration-cluster-ledger.md) |
