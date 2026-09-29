# Policy caps and duplicate validation (#8742)

The trusted product path no longer stops at arbitrary counts or sizes. Checks
that protect a real representation, backend or resource bound stay. Dynamics
caps went in #8738, and voxel edit and residency caps in #8739.

## Removed

| Area | Removed | Before |
|---|---|---|
| Audio (`csharp-engine-services/src/audio.rs`) | 8 MiB per clip, 64 clips, 32 MiB total. Also the optional-preload `SkippedCapacity` outcome and the receipt's count/byte/budget fields. | The 65th clip, or a 9 MiB track, failed to open. |
| Static-mesh collision (`svc-collision/src/static_mesh.rs`) | 256 assets, 4,096 instances, 1M/2M vertices, and 2M/4M triangles. Also `replace_all`'s second geometry validation and hash, and the receipt's unread count fields. | A 4,097th instance failed the whole replacement. |
| Spatial queries and content (`engine-spatial/src/occlusion.rs`, `csharp-engine-services/src/spatial.rs`) | Occlusion: 4,096 entities in the world, 8 ignored identities, 4,096 hitbox overrides. The same caps on service queries. Spatial-content byte, vertex, triangle, navigation-cell and ±10M coordinate quotas. The Engine ceiling of 65,536 on the caller's navigation cell budget. | Any ray cast failed once the world held 4,097 entities. |
| Perception (`engine-spatial/src/perception.rs`) | 64 observers, 256 targets, 1,024 pairs from `evaluate`, and 256 aggregates. | — |
| Triggers (`engine-spatial/src/trigger.rs`) | 4,096 definitions and 1M active overlaps, including on restore and snapshot. | The 4,097th trigger was refused. |
| World origin, motion, kinematics | 1,024 rebase entities, 64 prepared rebases, 1,024 motion entities, and 1,024 kinematic rows/selection. Kinematic calls over the limit returned 0 with no diagnostic. | — |
| Rigid bodies (`entity-state/src/rigid_body.rs`) | Upper bounds on mass, extent, center of mass, speed, damping, gravity scale, friction and restitution. Values must still be finite, and positive or non-negative. | Past a bound, a Dynamics readout silently lost its mass properties. |
| Character tether | The 0.25 reel-speed cap. The speed must be non-negative. | — |
| Voxels (`engine-spatial`) | 1M solid cells in a freshly built scene, the 4,096-cell primitive expansion cap, and the radius-4 line cap. | — |
| Particles (`render-presentation`, `renderer-host/particle-host.ts`) | The 64-emitter and 1,024-per-emitter caps, in Rust and in the TS host. | The 65th emitter was refused. |
| Billboards (`renderer-host/billboard-host.ts`) | The 500-billboard submission cap. | The 501st billboard raised `hostFailure`, which degraded the whole billboard domain. |
| View composition (`render-host-contracts`, `render-contracts/view-composition.ts`) | 4 cameras, 4 targets, 8 views and 4 presentations. | — |
| Live debug | The 64 KiB command and result caps, in the product generator, runtime, dev host and CLI. | A large debug catalog failed to describe itself. |
| Content store | The 1 MiB per-read cap. | An oversized read returned 0 with no diagnostic. |
| Textures | Three unused constants: `MAX_RETAINED_TEXTURES` and the aggregate byte caps. | — |
| TS live path | `RendererPresentationHostSet` deep-decoded every presentation frame (about 940 lines of decoder) before the hosts ran. The transport re-validated each view composition, which the renderer validates again when it configures it, and deep-copied every UI projection, which the application host validates on ingest. The decoders remain for contract tests. | — |

## Bug fixed on the way

`ProductDevHostError::new` truncated its detail to 512 bytes with
`String::truncate`. That panics when byte 512 falls inside a multi-byte
character. The detail is no longer truncated.

## Kept, and why

- **Particle simulation budget** (4,096 reserved particles). The browser
  simulates every live particle each frame, so this bounds real per-frame
  cost. Its outcome is already graceful: an optional burst is clamped or
  dropped, and a receipt says so. Only the arbitrary emitter counts went.
- **Billboard visible layout cap** (256). Extra billboards are hidden, not
  refused; it bounds DOM layout work.
- **Renderer target dimension and pixel limits** (2,048 px, 8 Mpx). These bound
  GPU memory for offscreen targets.
- **Representation limits:**
  - `MAX_VOXEL_MATERIAL_SLOT` (4,095), the voxel value's material bits;
  - `MAX_VOXEL_COORDINATE_ABS`, which keeps f32 render translations exact
    enough;
  - `MAX_CHUNK_SIZE` (64), the mesher's row width;
  - `MAX_TEXTURE_ENCODED_BYTES` (u32).
- **Paging bounds** (trigger read items and overlap pages, perception page
  size). These belong to result buffers (#8744).
- **Finite, positive and index checks.** They stop NaN or out-of-range indices
  from reaching Rapier, parry or Three.js.

## Evidence

Each removed cap has a test of ordinary use beyond it:

| Test | Crosses |
|---|---|
| `audio.rs` `mixed_containers_share_owners_and_admit_past_former_budgets` | 74 clips, four of 9 MiB |
| `audio.rs` `optional_preload_skips_missing_without_faulting_the_call_then_admits` | 9 MiB optional preload |
| `static_mesh.rs` `admits_more_assets_and_instances_than_the_former_caps` | 300 assets, 5,000 instances, ray hit |
| `tests/occlusion.rs` `large_queries_run_and_invalid_queries_are_typed_without_changing_authority` | 5,000 entities, 16 ignored |
| `tests/perception.rs` `evaluates_more_observers_targets_and_pairs_than_the_former_caps` | 80 × 300: 24,000 pairs |
| `tests/mechanics.rs` `registers_more_triggers_than_the_former_cap` | 5,000 triggers |
| `tests/voxel_authoring.rs` `primitives_expand_past_former_caps_and_reject_invalid_materials` | radius 5, 5,000 cells |
| `tests/rigid_body.rs` `rigid_body_mass_is_strictly_positive_and_finite` | 10,000-tonne body |
| `tests/domain_projectors.rs` `particle_emitter_count_is_bounded_only_by_the_particle_budget` | 100 emitters |
| `billboard-host.test.ts` `the host accepts more billboards than the former 500` | 600 billboards |
| `view-composition.test.ts` `admits more cameras than the former cap of four` | 6 cameras |
| `csharp-product-runtime` debug result test | 100 KiB result |

Tests whose only purpose was a removed rule were rewritten into the tests
above. That covers the audio capacity receipt, occlusion quotas, primitive
caps, the rigid-body mass ceiling, the composition camera boundary and the
malformed-frame rejection in the audio-host test. The transport's
UI-projection test no longer asserts a detached frozen copy.

The spatial-artifact fixture's invalid artifact now has minimum bounds above
its maximum. Before, it only exceeded the removed coordinate quota. It still
proves that a refused artifact changes nothing.

### Checks

- **Workspace:** 1,184 Rust tests pass. Clippy adds no findings beyond the
  pre-existing #8757 lints.
- **TypeScript:** typecheck and all package tests pass; the browser bundles in
  `render/artifacts` are rebuilt.
- **C#:**
  - the SDK and `Rusty.Engine.Entities.Example` build;
  - `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes;
  - `scripts/test-runtime-pack.sh` passes.

## Migration

- **Optional audio preload:** `AudioOptionalPreloadReceipt` keeps only
  `Outcome` and `Clip`, and `SkippedCapacity` is gone. `SkippedMissing` still
  reports an absent resource.
- **Removed constants and error variants:** code matching `TooMany*` errors
  or reading these `MAX_*` constants drops those arms. This covers static mesh,
  occlusion, perception, world origin, primitives and scene building.
- **`ProductDevDebugResult::new`** is infallible.

## Not done here

Filed as #8798:
- presentation content limits: billboard text, argument and meter counts,
  particle descriptor ranges, sprite and animation-cue caps;
- authored-content and persistence caps;
- the transport's remaining per-output decoders and host re-validation;
- entity translation and velocity bounds;
- the static-mesh and trigger stale guards.
