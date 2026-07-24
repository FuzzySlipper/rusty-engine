# Donor provenance

Rusty Engine internalizes or selectively adapts Asha code only when that code sits below a
successor-owned boundary and has a concrete consumer.

Inspected donor repository: `git@github.com:FuzzySlipper/asha-engine.git`
Pinned source commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`

## Crate portability inventory

The historical donor's
[pinned Asha Engine crate portability report](https://github.com/FuzzySlipper/asha-engine/blob/a431974330589761c9e35fc4f8a55996a1b5ee48/docs/asha-crate-portability-report.md)
audits all 97 Rust crates at the pinned source commit, including dependency hotspots, portability
classifications, extraction order, structural-spine exclusions, and successor guardrails.

The report remains with the historical Asha sources so its crate links and dependency evidence stay
in their original context. It is a planning index, not blanket permission to port a classified crate.
Every milestone must still inspect the candidate's actual dependency closure and semantics, choose
reference/adapt/evidence/exclude treatment for its concrete consumer, and record the accepted use in
this document. If the donor revision changes, revalidate the relevant report rows rather than
assuming the old classification still holds.

For the final repository-independence extraction, the bounded source, test, fixture, license,
destination, and exclusion decisions are frozen in the
[M9 extraction contract](m9-extraction-contract.md). That contract is narrower than the portability
report and is the authority for M9B-M9D.

Every accepted live Rust package is now an ordinary workspace member at
`rust/crates/<package-name>`. The table records historical source paths and treatment; origin does
not create a second crate hierarchy.

| Local dependency/use | Asha source path | Treatment | Reason |
|---|---|---|---|
| `core-ids` | `engine-rs/crates/foundation/core-ids` | Internalized and narrowed to `EntityId` | The consumed typed entity identity remains; unused abstract/project/session/prefab IDs were excluded with the old structural vocabulary. |
| `core-math` | `engine-rs/crates/foundation/core-math` | Internalized unchanged at the pinned commit | Small deterministic vector values; no high-level dependencies. |
| `core-time` | `engine-rs/crates/foundation/core-time` | Internalized unchanged at the pinned commit | Stable tick values used by the lab scheduler; no scheduling policy. |
| `core-space` | `engine-rs/crates/foundation/core-space` | Internalized; test fixture path adapted | Typed voxel/chunk/world coordinates keep the substantial collision donor boundary intact. |
| `core-voxel` | `engine-rs/crates/state/core-voxel` | Internalized unchanged at the pinned commit | Canonical compact voxel values beneath spatial/collision services. |
| `svc-volume` | `engine-rs/crates/services/svc-volume` | Internalized unchanged at the pinned commit | Bounded chunk storage; no gameplay/runtime dependency. |
| `svc-spatial` | `engine-rs/crates/services/svc-spatial` | Internalized unchanged at the pinned commit | Canonical voxel partition and deterministic resident-chunk lifecycle. |
| `svc-collision` | `engine-rs/crates/services/svc-collision` | Internalized unchanged at the pinned commit | Substantial Parry-backed derived collision projection with point, ray, AABB, and continuous axis-sweep queries. Its dependency closure contains no Gameplay Fabric or runtime facade. |
| `svc-pathfinding` | `engine-rs/crates/services/svc-pathfinding` | Production source internalized; tests narrowly adapted | Deterministic read-only navigation projection and bounded path queries over `svc-spatial::VoxelWorld`. Its production closure is only `core-math`, `core-space`, and `svc-spatial`; Rusty Engine owns navigation intent, movement, facts, and persistence. |
| `svc-rng` | `engine-rs/crates/services/svc-rng` | Internalized unchanged at the pinned commit | Small deterministic scoped SplitMix64 stream with no dependencies, ambient entropy, global state, lifecycle, or replay owner. Rusty Engine stores the seed and owns generation meaning. |
| `svc-mesh` | `engine-rs/crates/services/svc-mesh` | Internalized; test fixture path adapted | Deterministic visible-face meshing directly over the same `VoxelWorld` used by collision/navigation. Its closure is `core-space`, `core-voxel`, `svc-volume`, and `svc-spatial`; output is a derived presentation payload. |
| Generated-room algorithm evidence | `engine-rs/crates/services/svc-levelgen` | Algorithm adapted; crate not referenced | Its shell loop and validation informed the successor room generator, but `core-events`, replay/hash records, runtime-frame metadata, collision AABBs, and render-chunk summaries were not imported. Rusty Engine emits one canonical voxel result and lets named consumers derive from it. |
| Player input/controller evidence | `engine-rs/crates/protocol/protocol-input` and `engine-rs/crates/rules/rule-input` | Inspected only; no dependency or copied implementation | The useful boundary is authored physical controls resolving to semantic actions. Catalog hashing, context stacks, replay records, session configuration, and lifecycle routing are intentionally absent. |
| Camera/view evidence | `engine-rs/crates/protocol/protocol-view` | Inspected only; no dependency or copied implementation | Pose vocabulary and bounded look input informed names. Camera handles, bridge operations, controller modes, transition state, and persisted camera authority are intentionally absent; the browser derives one follow camera from accepted player pose. |
| Combat ray/target algorithm | `engine-rs/crates/services/svc-combat` | Small slab-ray/nearest-target algorithm adapted; crate not referenced | Deterministic AABB intersection and nearest-hit ordering are useful below the successor service. The donor `CombatState`, copied fire-control command state, health/replay hashes, readout/golden machinery, and independent health table were rejected because Rusty Engine entities and `CombatService` already own those meanings. |
| FPS combat/lifecycle evidence | `engine-rs/crates/rules/rule-lifecycle/src/lib.rs` (`apply_primary_fire_for_roles_with_entities`) and `fps_loaded.rs` | Inspected only; no dependency or copied implementation | Confirmed the old player-fire behavior and collision ordering, while providing negative evidence for role maps, runtime-session wrappers, entity-authoring policy routes, gameplay-event adapters, state rollback copies, and per-action replay records. |
| Presentation feedback evidence | `engine-rs/crates/render/render-animation`, `render-audio`, `render-billboard`, `render-particle`, `protocol/protocol-presentation`, `protocol/protocol-render`, and `render/render-bridge` | Inspected only; no dependency or copied implementation | Retained the one-way projection rule, disposable effect ownership, bounded transient work, entity/world anchoring, and fail-soft host realization as design evidence. Rejected the donor animation authority, asset catalog/hash closure, broad presentation/render operations, retained handle registries, origin/correlation/replay metadata, scene/level-generation bridge, and runtime-session routing. Rusty Engine instead owns one response-local semantic cue union at the browser-host border. |
| `core-assets` | `engine-rs/crates/foundation/core-assets` | Internalized unchanged at the pinned commit | Its zero-dependency `AssetId`/`AssetKind` vocabulary gives stored projects strict kind-prefixed identity without importing catalog resolution or lifecycle. |
| Stored project and scene evidence | `engine-rs/crates/state/core-catalog`, `state/core-scene`, `protocol/protocol-assets`, `protocol-diagnostics`, `protocol-entity-authoring`, and `protocol-scene` | Inspected only; successor-owned document and diagnostics | Typed identities, flat authored documents, reference validation, and path-bearing diagnostics informed M5. Catalog DAG/locks/material authority, scene bootstrap/spatial session, proposal commands, protocol codegen, and Asha diagnostic scopes were rejected. |
| Project content/bundle evidence | `engine-rs/crates/services/svc-project-content`, `svc-serialization`, `protocol/protocol-project-content`, `protocol-project-bundle`, and `rules/rule-project-bundle` | Structural evidence and exclusion | These closures combine provider manifests, extension/input protocols, load/save plans, prefabs, gameplay fabric, lifecycle, annotations, and session bootstrap. M5 instead decodes one static successor document and defers narrow serialization ideas to M6. |
| Canonical project codec and migration evidence | `engine-rs/crates/services/svc-serialization/src/json.rs`, `state/core-scene/src/{document,json,validate}.rs`, `state/core-snapshot/src/lib.rs`, the canonical-dump examples, and `tools/scene-diagnostics/src/roundtrip.rs` | Encoding/test lessons adapted; crates and tools not referenced | M6 retains fixed object-field order, canonical collection ordering, finite deterministic numbers, trailing-LF output, fixed-point/golden-style tests, and fail-closed schema selection. It does not import manifest/artifact hashes, `StateStore`, replay fingerprints, diagnostic protocols, voxel compaction, or scene bootstrap/session state. |
| Voxel asset/import/conversion evidence | `svc-mesh-import`, `svc-voxel-conversion`, `svc-voxel-asset`, `protocol-voxel-conversion`, `protocol-voxel-asset`, `tools/asset-import`, and `harness/fixtures/voxel-conversion` | Narrow format/parser/algorithm lessons adapted; crates not referenced | M7B retains offline GLB parsing, explicit bounded settings, sparse runs, canonical bytes, source provenance, and atomic artifact installation. It excludes registries, catalog/lock graphs, plan/preview/apply, providers, evidence graphs, replay, bridges, Studio, and project-bundle control planes. |
| Kenney wall fixture and license | `harness/fixtures/voxel-conversion/{kenney-wall-a.glb,KENNEY-RETRO-URBAN-KIT-LICENSE.txt}` | Copied byte-for-byte to `fixtures/voxel-conversion` | The real conversion/product proof is now repository-local. The 3,352-byte GLB retains SHA-256 `6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00`; the adjacent 318-byte CC0 text retains SHA-256 `3679c62e69e67da74fec17327635e67c92991ac82b0bdfcc203d8ecd473c016a`. |
| `@asha/contracts` | `ts/packages/contracts/src/generated/{ids,render}.ts` | Replaced by bounded `@rusty-engine/render-contracts` | Keeps branded entity/render identities and the four render operations the product emits. Generated protocol barrels, compatibility/codegen claims, bridge handles, assets, editor values, and gameplay/runtime envelopes are excluded. |
| `@asha/renderer-three` | `ts/packages/renderer-three/src/{three-renderer,browser-surface}.ts` | Narrow successor fork as `@rusty-engine/renderer-three` | Retains typed primitive lifecycle, inline mesh upload/disposal, deterministic inspection, camera placement, and real Three/WebGL mounting. The local API is bounded to the browser shell. |
| `@asha/render-projection` | `ts/packages/render-projection` | Inspected and excluded | Rusty Engine's `RuntimeProjectionAdapter` already owns whole-state-to-diff projection; no donor projection export reaches the product. |

The accepted Rust packages now live alongside the rest of the workspace under `rust/crates`;
`engine-spatial` remains the successor-owned adapter and system above the low-level spatial
services, and M3 adapts only the small ray/AABB query algorithm named above. The browser shell
supplies typed diffs directly to local packages. There is no encoded frame entry point,
runtime-bridge shim, or Vite alias. The verification gate rejects old `RuntimeSession`, native
bridge, Gameplay Fabric, or `GameplayRuntimeHost` markers in the built browser bundle.

M2A deliberately does not reference Asha's input or view crates. TypeScript resolves DOM device
events against admitted binding data and submits only `ResolvedPlayerAction`; Rust owns controller
interpretation and collision-resolved pose. The renderer's existing `setCameraPose` method receives
a presentation-only offset derived from that pose. No input catalog, camera state, or per-frame
authority bridge entered the successor.

M2B references `svc-rng` and `svc-mesh` unchanged. It does not reference `svc-levelgen`, because
that otherwise-useful generator owns `core-events` output and several replay/projection summaries
that would recreate parallel authority. The adapted successor loop is deliberately smaller: seed
and dimensions produce material voxels, then the already-owned `VoxelWorld` is the sole input to
collision, navigation, and mesh derivation. Its centered exit aperture is successor-owned geometry,
not a transplanted portal or control abstraction.

M3 deliberately does not reference `svc-combat`. Its useful ray/AABB intersection and stable
nearest-target ordering now sit inside the successor-owned `CombatService`, which reads live entity
transforms, `HealthComponent`, `WeaponComponent`, and the canonical voxel collision scene directly.
Health, ammo, cooldown eligibility, damage, and defeat have no donor-owned mirror or hash. A lethal
hit emits the existing typed `EnemyDefeated` consequence into the explicit encounter/door drain;
no FPS runtime session, role registry, proposal policy, or replay record entered the path.

M4 donor inspection used the pinned evidence revision above. The relevant presentation files are
unchanged in the current Asha checkout at `6462a6de20d48ea1a3b7456826804bd9507860a5`, so the newer
checkout added no unreviewed semantic drift to this decision. None of the four render crates or
their protocol dependencies enters Rusty Engine. Their strongest shared lesson is narrower than
their APIs: presentation reads accepted state/facts in one direction, retained posture can be
rebuilt, impulses can be discarded, and host failure never changes authority. The successor border
therefore preserves movement, attack, damage, defeat, and door payloads as a small closed union in
the browser response. Animation posture is rebuilt from current entity state; cues are never added
to `GameRuntime`, `GameSession`, the journal, or a snapshot.

The successor implementation is pinned by
`bb16dbd5aa65878e9dadf36912d3478a06898f51` (typed Rust response projection),
`2146e94020787d798f37a2f0fd17e4c8259bc71a` (DOM/Web Audio realization), and
`3ea43745208af284caa11680b221bb9c1131bd4a` (drop/restart/Chromium proof), with review correction
`59b4f4039fde0b63444d97fec2879b78195af5f1` (concrete pulse/audio reset ownership and proof). The product gate realizes
all four feedback families, schedules an oscillator/gain envelope, discards one cue-bearing response,
and proves a fresh readout has identical gameplay with no replayed cue. Both reset checks begin with
active concrete pulse, DOM, and audio targets, clear them, and then rebuild current posture.
The gate then opens a fresh Chromium page against the same mutated Rust host and proves defeated/open
posture is rebuilt with no cues, pulse attributes, transient DOM nodes, or active audio targets while
the complete state response remains unchanged.
Existing fact payload changes flow through without changing the presentation border; changing an
existing effect stays in the TypeScript adapter/CSS/tests. A genuinely new outcome changes its typed
producer plus the small closed mapping, not a donor protocol or generic signal route.

M5A re-audited the asset, catalog, scene, diagnostics, entity-authoring, project-content,
serialization, and project-bundle candidates at the same pinned revision. All inspected paths are
unchanged at current Asha head `6462a6de20d48ea1a3b7456826804bd9507860a5`. Only `core-assets`
enters the dependency graph; it is a `std`-only leaf. `core-catalog` has a small production closure
(`core-assets`, `core-voxel`) but its DAG, lock, fallback, and material rules remain unearned.
`core-scene` also carries `core-entity` and Asha bootstrap/spatial-session assumptions, so only its
document lessons were adapted. `svc-project-content` and the project protocols retain the static
provider/extension control plane, while `rule-project-bundle` reaches gameplay fabric, lifecycle,
prefabs, voxel annotations, and project load plans. They remain explicit exclusions. M5's checked-in
schema-v7 artifact, successor-owned serde shapes, and local structured diagnostic value are not wire
protocols, provider envelopes, or a second compiler authority.

The M5 implementation is pinned by
`cfa3aea016a14113c2b1969b209d40d66eb46bf0` (document/types/diagnostics and donor boundary),
`d29a1b01681f60e3fbec40bfd53262ef33e80231` (all-or-nothing Rust admission), and
`6fedb77302628cc00bfbee4576a4bf3029ea2554` (static product host, optional equivalent TypeScript
candidate, content variation, and project/session persistence proof). No Asha catalog, scene,
diagnostics, serialization, project-content, or bundle crate entered in those changes.

M6A re-audited `svc-serialization`, `core-scene`, `core-snapshot`, `svc-project-content`,
`protocol-project-bundle`, and `rule-project-bundle` at pinned commit
`a431974330589761c9e35fc4f8a55996a1b5ee48`. Those paths are byte-unchanged at current Asha head
`6462a6de20d48ea1a3b7456826804bd9507860a5`. The successor adapts only deterministic JSON shape,
canonical ordering, fixed-point proof, and fail-closed version selection. It explicitly rejects the
donor artifact table, content hashes, save/load plans, prefab registry, compaction journal, replay
record, provider manifest, workspace lifecycle, bootstrap/session state, and universal runtime hash.
Rust owns the one concrete schema-6 to schema-7 migration; TypeScript may still materialize a
candidate, but it neither selects migration semantics nor emits canonical saved bytes.

The M6 implementation is pinned by
`5072f0c0a5cd03448c3543d6763f3dd9082fa54c` (canonical codec and explicit predecessor migration),
`a3eae545558a8e47c652af9a159c708dd32eb950` (admitted-token bounded durable store), and
`d17ed7f28d9d386072eb745f6ec1f5d789e89978` (filesystem product startup and literal separation
proof). No Asha serialization, snapshot, scene, project-content, bundle, rule, or diagnostics crate
was added. The filesystem service remains successor-owned and accepts only static data carrying the
same semantic-admission token used to construct the runtime.

M7A re-audited `rule-voxel-edit`, `svc-spatial`, `svc-collision`, `svc-pathfinding`, `svc-mesh`,
and `voxel-diagnostics` at the pinned revision. Those paths are byte-unchanged at current Asha head
`6462a6de20d48ea1a3b7456826804bd9507860a5`. The successor retains complete-batch validation,
bounded amplification, deterministic coordinate order, authoritative pick revalidation as a later
product-border option, and the spatial service's dirty-neighbour lesson. It deliberately does not
import Asha's global voxel command/event union, preview/apply protocol, replay divergence records,
generation command, edit history, persistence callbacks, scene composition, or diagnostics tool.
`VoxelEditService` instead owns only the live product's typed set/clear transaction, stale-source
revision check, and atomic collision/navigation/mesh coherence rule. Runtime snapshot and explicit
project save persist concrete accepted material voxels, not events or history.

The M7A implementation is pinned by
`cddf89f79201a7ae657beffbdd3dd87fb84f818f` (successor transaction/revision/limit vocabulary),
`eb5ee0b177e3568b5b52f2492d6503123ad94519` (atomic material authority and full coherent
projection rebuild), and `e4db64716ef9d5a9bb07d9d0048b94737cd09850` (snapshot/project
materialization, typed product route, Chromium/persistence proof, and bounded workload). No Asha
edit rule, command/event protocol, diagnostics, history, persistence, or scene-composition crate was
added. Existing unchanged spatial/collision/navigation/mesh donors remain beneath the
successor-owned scene and service.

M7B re-audited `svc-mesh-import`, `svc-voxel-conversion`, `svc-voxel-asset`,
`protocol-voxel-conversion`, `protocol-voxel-asset`, `asset-import`, and the real Kenney wall
fixture at the pinned revision. Every inspected path is byte-unchanged at current Asha head
`6462a6de20d48ea1a3b7456826804bd9507860a5`. The selected fixture is the 3,352-byte CC0
`harness/fixtures/voxel-conversion/kenney-wall-a.glb`, SHA-256
`6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00`; Asha's bounded importer
proves it contains 48 positions, 36 indices, two triangle groups, and two named material slots.

The successor-owned boundary is documented in [voxel-asset-format.md](voxel-asset-format.md).
`voxel-asset` retains only strict schema/grid/bounds/material/provenance values, bounded sparse +X
runs, canonical SHA-256 bytes, classified source paths, and preflight conversion settings. The GLB
parser remains in the separate offline tool; `game-host` depends only on the durable asset crate. A
content hash protects the artifact itself and never becomes an action precondition, replay
certificate, or runtime revision. Asha's source registry, catalog/lock graph, plan/preview/apply
sequence, provider interface, evidence URI graph, command registry, bridge, replay, lifecycle,
texture protocol, Studio surface, and project-bundle machinery remain explicit exclusions.

The M7B.1 format boundary is pinned by `17545406494bc93f12d3668b845a533cee8ceb4d`, with the
cross-row sparse-run canonicalization correction pinned by
`a51bf6e61b0c4e52d1bc4613440310d82638d216`.
The M7B.2 parser/converter, canonical real artifact, atomic CLI, and ordinary M5/M7A admission path
are pinned by `b3481fadf1586c2cfea167d569af0bd6333af6b5`. `game-host` depends on
successor-owned `voxel-asset` only; the separate `voxel-convert` authoring crate contains `gltf` and
filesystem installation. No Asha conversion/import/asset crate was added.

M7B.3 is pinned by `2cdad99c0d012643fe157fa6db51495a31327d98`. Its checked schema-v7 project,
focused snapshot/authored-save coverage, real Chromium collision/navigation/mesh/edit path, and
bounded conversion workload all consume successor-owned values. The built browser bundle excludes
converter/request vocabulary; `cargo tree -p game-host` excludes both `gltf` and `voxel-convert`.
Explicit authored save expands the accepted result to ordinary static material voxels rather than
persisting an asset job, edit history, provider identity, or replay record. M7C remains unscheduled
because no annotation/history consumer emerged.

M9B internalized the accepted twelve-crate Rust closure from pinned commit
`a431974330589761c9e35fc4f8a55996a1b5ee48`. Its initial origin-oriented directories made source
comparison obvious while extraction was still being proven. Once Rusty Engine became the durable
repository, the packages moved to `rust/crates/<package>` and their test data to `fixtures`; this
document and Git history now carry provenance instead of directory topology. `core-ids` retains the
donor's consumed `EntityId` behavior but drops its unused abstract/project/session/prefab ID
families. Other production source is unchanged apart from removing stale origin-oriented comments.
`core-space` and `svc-mesh` only point their existing tests at local fixture paths.
`svc-pathfinding`'s excluded dev-only `svc-levelgen` dependency was replaced in tests by the same
tiny solid shell built directly as a `VoxelWorld`; its path and projection goldens remain identical.
Cargo metadata resolves no local package outside Rusty Engine, and all focused low-level tests run
as ordinary workspace tests. No runtime, protocol, lifecycle, replay, bridge, bundle, provider,
editor, or level-generation crate was copied.

M9C replaced the linked TypeScript closure with `@rusty-engine/render-contracts` and
`@rusty-engine/renderer-three`. The contract is a hand-owned closed union of the four operations
emitted by `RuntimeProjectionAdapter`; it does not claim to be generated. The renderer substantially
narrows the pinned donor's retained-scene and browser-surface algorithms to primitive lifecycle,
partial visual updates, validated inline mesh replacement, deterministic snapshots, resource
disposal, camera placement, and real WebGL. Consumer tracing confirmed that animation, sprites,
catalog/static-mesh definitions, lights as diff operations, picking, editor/tunnel conveniences,
generic render projection, encoded frames, and buffer handles had no product caller, so those
families were excluded instead of copied. Focused package tests and the existing Chromium product
gate cover the retained behavior.

M9D copied the exact Kenney GLB and CC0 text to `fixtures/voxel-conversion`, moved request, test,
workload, documentation, and persisted provenance paths to that location, and removed the Asha
checkout/install/build steps from CI. The source hash, settings hash, voxel cells, material mapping,
and generated geometry remain unchanged. Because source and license paths are intentionally part of
canonical provenance, the regenerated artifact content hash changed from
`8d5c4037cee3279ac66870b285ca794b35e35fa3e3026a51cd4ae506b3f7397e` to
`086d81f12403192c6d7568289c2b47771741e5620a967e5b5fe5093fd5608ab7`; the checked project embeds
that exact canonical result.

The operational dependency baseline is now empty: Cargo metadata, pnpm resolution, runtime and test
asset paths, scripts, executable documentation, and CI require only Rusty Engine. A clean clone with
no Asha sibling is the final certification boundary. Rusty Engine is canonical for new work; Asha
and its Den project remain immutable historical evidence and source locators only.
