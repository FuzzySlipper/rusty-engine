# Donor provenance

The lab references or selectively adapts Asha code only when that code sits below the architecture being tested.

Inspected donor repository: `git@github.com:FuzzySlipper/asha-engine.git`
Pinned source commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`

## Crate portability inventory

Before selecting Asha code for a future migration milestone, consult the sibling donor's
[Asha Engine crate portability report](../../asha-engine/docs/asha-crate-portability-report.md)
at `/home/dev/asha-engine/docs/asha-crate-portability-report.md`. It audits all 97 Rust crates at
the pinned source commit, including dependency hotspots, portability classifications, extraction
order, structural-spine exclusions, and successor guardrails.

The full report remains beside the Asha sources so its crate links and dependency evidence stay in
their original context. It is a planning index, not blanket permission to port a classified crate.
Every milestone must still inspect the candidate's actual dependency closure and semantics, choose
reference/adapt/evidence/exclude treatment for its concrete consumer, and record the accepted use in
this ledger. If the donor revision changes, revalidate the relevant report rows rather than assuming
the old classification still holds.

| Local dependency/use | Asha source path | Treatment | Reason |
|---|---|---|---|
| `core-ids` | `engine-rs/crates/foundation/core-ids` | Sibling path dependency, unchanged | Mature typed identity newtypes; no high-level dependencies. |
| `core-math` | `engine-rs/crates/foundation/core-math` | Sibling path dependency, unchanged | Small deterministic vector values; no high-level dependencies. |
| `core-time` | `engine-rs/crates/foundation/core-time` | Sibling path dependency, unchanged | Stable tick values used by the lab scheduler; no scheduling policy. |
| `core-space` | `engine-rs/crates/foundation/core-space` | Sibling path dependency, unchanged | Typed voxel/chunk/world coordinates keep the substantial collision donor boundary intact. |
| `core-voxel` | `engine-rs/crates/state/core-voxel` | Sibling path dependency, unchanged | Canonical compact voxel values beneath spatial/collision services. |
| `svc-volume` | `engine-rs/crates/services/svc-volume` | Sibling path dependency, unchanged | Bounded chunk storage; no gameplay/runtime dependency. |
| `svc-spatial` | `engine-rs/crates/services/svc-spatial` | Sibling path dependency, unchanged | Canonical voxel partition and deterministic resident-chunk lifecycle. |
| `svc-collision` | `engine-rs/crates/services/svc-collision` | Sibling path dependency, unchanged | Substantial Parry-backed derived collision projection with point, ray, AABB, and continuous axis-sweep queries. Its dependency closure contains no Gameplay Fabric or runtime facade. |
| `svc-pathfinding` | `engine-rs/crates/services/svc-pathfinding` | Sibling path dependency, unchanged | Deterministic read-only navigation projection and bounded path queries over `svc-spatial::VoxelWorld`. Its production closure is only `core-math`, `core-space`, and `svc-spatial`; Rusty Engine owns navigation intent, movement, facts, and persistence. |
| `svc-rng` | `engine-rs/crates/services/svc-rng` | Sibling path dependency, unchanged | Small deterministic scoped SplitMix64 stream with no dependencies, ambient entropy, global state, lifecycle, or replay owner. Rusty Engine stores the seed and owns generation meaning. |
| `svc-mesh` | `engine-rs/crates/services/svc-mesh` | Sibling path dependency, unchanged | Deterministic visible-face meshing directly over the same `VoxelWorld` used by collision/navigation. Its closure is `core-space`, `core-voxel`, `svc-volume`, and `svc-spatial`; output is a derived presentation payload. |
| Generated-room algorithm evidence | `engine-rs/crates/services/svc-levelgen` | Algorithm adapted; crate not referenced | Its shell loop and validation informed the successor room generator, but `core-events`, replay/hash records, runtime-frame metadata, collision AABBs, and render-chunk summaries were not imported. Rusty Engine emits one canonical voxel result and lets named consumers derive from it. |
| Player input/controller evidence | `engine-rs/crates/protocol/protocol-input` and `engine-rs/crates/rules/rule-input` | Inspected only; no dependency or copied implementation | The useful boundary is authored physical controls resolving to semantic actions. Catalog hashing, context stacks, replay records, session configuration, and lifecycle routing are intentionally absent. |
| Camera/view evidence | `engine-rs/crates/protocol/protocol-view` | Inspected only; no dependency or copied implementation | Pose vocabulary and bounded look input informed names. Camera handles, bridge operations, controller modes, transition state, and persisted camera authority are intentionally absent; the browser derives one follow camera from accepted player pose. |
| Combat ray/target algorithm | `engine-rs/crates/services/svc-combat` | Small slab-ray/nearest-target algorithm adapted; crate not referenced | Deterministic AABB intersection and nearest-hit ordering are useful below the successor service. The donor `CombatState`, copied fire-control command state, health/replay hashes, readout/golden machinery, and independent health table were rejected because Rusty Engine entities and `CombatService` already own those meanings. |
| FPS combat/lifecycle evidence | `engine-rs/crates/rules/rule-lifecycle/src/lib.rs` (`apply_primary_fire_for_roles_with_entities`) and `fps_loaded.rs` | Inspected only; no dependency or copied implementation | Confirmed the old player-fire behavior and collision ordering, while providing negative evidence for role maps, runtime-session wrappers, entity-authoring policy routes, gameplay-event adapters, state rollback copies, and per-action replay records. |
| Presentation feedback evidence | `engine-rs/crates/render/render-animation`, `render-audio`, `render-billboard`, `render-particle`, `protocol/protocol-presentation`, `protocol/protocol-render`, and `render/render-bridge` | Inspected only; no dependency or copied implementation | Retained the one-way projection rule, disposable effect ownership, bounded transient work, entity/world anchoring, and fail-soft host realization as design evidence. Rejected the donor animation authority, asset catalog/hash closure, broad presentation/render operations, retained handle registries, origin/correlation/replay metadata, scene/level-generation bridge, and runtime-session routing. Rusty Engine instead owns one response-local semantic cue union at the browser-host border. |
| `core-assets` | `engine-rs/crates/foundation/core-assets` | Sibling path dependency, unchanged | Its zero-dependency `AssetId`/`AssetKind` vocabulary gives stored projects strict kind-prefixed identity without importing catalog resolution or lifecycle. |
| Stored project and scene evidence | `engine-rs/crates/state/core-catalog`, `state/core-scene`, `protocol/protocol-assets`, `protocol-diagnostics`, `protocol-entity-authoring`, and `protocol-scene` | Inspected only; successor-owned document and diagnostics | Typed identities, flat authored documents, reference validation, and path-bearing diagnostics informed M5. Catalog DAG/locks/material authority, scene bootstrap/spatial session, proposal commands, protocol codegen, and Asha diagnostic scopes were rejected. |
| Project content/bundle evidence | `engine-rs/crates/services/svc-project-content`, `svc-serialization`, `protocol/protocol-project-content`, `protocol-project-bundle`, and `rules/rule-project-bundle` | Structural evidence and exclusion | These closures combine provider manifests, extension/input protocols, load/save plans, prefabs, gameplay fabric, lifecycle, annotations, and session bootstrap. M5 instead decodes one static successor document and defers narrow serialization ideas to M6. |
| Canonical project codec and migration evidence | `engine-rs/crates/services/svc-serialization/src/json.rs`, `state/core-scene/src/{document,json,validate}.rs`, `state/core-snapshot/src/lib.rs`, the canonical-dump examples, and `tools/scene-diagnostics/src/roundtrip.rs` | Encoding/test lessons adapted; crates and tools not referenced | M6 retains fixed object-field order, canonical collection ordering, finite deterministic numbers, trailing-LF output, fixed-point/golden-style tests, and fail-closed schema selection. It does not import manifest/artifact hashes, `StateStore`, replay fingerprints, diagnostic protocols, voxel compaction, or scene bootstrap/session state. |
| `@asha/contracts` | `ts/packages/contracts` | Sibling `link:` dependency, unchanged | Existing typed render-diff vocabulary and branded render/entity identities at the real presentation border. |
| `@asha/renderer-three` | `ts/packages/renderer-three` | Sibling `link:` dependency, unchanged | Existing retained Three/WebGL browser surface, resource lifecycle, projection metadata, and render-diff application. |
| `@asha/render-projection` | `ts/packages/render-projection` | Renderer transitive sibling dependency, unchanged | Renderer-neutral retained projection helpers used by the donor browser surface. |

No Asha crate has been copied wholesale into the repository. `engine-spatial` is a successor-owned
adapter and system over unchanged Rust donors, and M3 adapts only the small ray/AABB query algorithm
named above. The browser shell supplies typed diffs directly; its Vite
alias replaces `renderer-three`'s unused encoded-frame convenience import with a local fail-closed
shim. The verification gate rejects old `RuntimeSession`, native bridge, Gameplay Fabric, or
`GameplayRuntimeHost` markers in the built browser bundle.

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

Sibling references are intentional while Asha development is stopped for this decision. If this
lab becomes a durable independent successor, the references should be pinned as Git dependencies,
vendored with this ledger, or moved into a shared foundation repository before Asha resumes.
