# Donor provenance

The lab references or selectively adapts Asha code only when that code sits below the architecture being tested.

Inspected donor repository: `git@github.com:FuzzySlipper/asha-engine.git`
Pinned source commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`

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
| `@asha/contracts` | `ts/packages/contracts` | Sibling `link:` dependency, unchanged | Existing typed render-diff vocabulary and branded render/entity identities at the real presentation border. |
| `@asha/renderer-three` | `ts/packages/renderer-three` | Sibling `link:` dependency, unchanged | Existing retained Three/WebGL browser surface, resource lifecycle, projection metadata, and render-diff application. |
| `@asha/render-projection` | `ts/packages/render-projection` | Renderer transitive sibling dependency, unchanged | Renderer-neutral retained projection helpers used by the donor browser surface. |

No Asha source has been copied into the repository. `engine-spatial` is a successor-owned adapter and
system over the unchanged Rust donors. The browser shell supplies typed diffs directly; its Vite
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
collision, navigation, and mesh derivation.

Sibling references are intentional while Asha development is stopped for this decision. If this
lab becomes a durable independent successor, the references should be pinned as Git dependencies,
vendored with this ledger, or moved into a shared foundation repository before Asha resumes.
