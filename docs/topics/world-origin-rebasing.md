# World-origin rebasing

Status: task 6895 implementation contract

## Problem and survey

Rusty Engine already preserves exact signed voxel and chunk identities in
`core-space`, and continuous collision queries use `f64` `WorldPos`. The
remaining precision-sensitive runtime surfaces are local `f32` values:

| Owner | Canonical or durable fact | Precision-sensitive derivative |
|---|---|---|
| `core-space` | signed `i64` voxel/chunk addresses and `f64` world positions | `VoxelGridSpec::origin_world` was an unused rebase hook |
| `entity-state` | stable entity/component identity and snapshots | Transform translation and character support/fall continuation are `f32` |
| `engine-spatial` | canonical signed voxel materials and exact source revisions | chunk transforms, controller/entity motion, and collision query frame |
| `svc-pathfinding` | exact signed walkable cells and paths | projected waypoints are local `f32` |
| `authored-scene` | stable scene/node identity and authored hierarchy | admitted runtime transforms are local `f32` |
| `render-model` / `render-projection` | stable retained handles and source identities | renderer transforms are intentionally local `f32` |
| CraftSurvive | signed terrain/edit addresses and seed/version overlay | player, support, camera, and retained chunk positions lose first-person precision far from zero |

Collision already supports an internal world offset, and voxel grids already
carry an explicit origin. The missing mechanism is one revisioned coordinate
frame that applies those hooks together with entity continuation state.

## Decision

Engine exposes two deliberately different coordinate forms:

- `GlobalPosition` is a canonical signed integer cell plus a normalized `f64`
  offset. It is stable under positive/negative rebases and JSON round trips.
- `WorldOriginState` is a revisioned signed integer origin and a configured
  local `f32` envelope. It contains no threshold, player selection, streaming
  radius, or scheduling policy.

For a point `global`, the local simulation/presentation value is always
re-derived as `global - origin`; accepted rebases never repeatedly subtract
from an accumulated global float. Stable entity, scene-node, static-instance,
voxel, chunk, and retained-handle identities do not include the origin.

`WorldOriginRebaseService` prepares a complete candidate from:

- the expected origin, entity-state, voxel source, and static-collision revisions;
- a caller-selected next integer origin; and
- a bounded identity-ordered set of entity/global-position bindings.

Preparation clones and validates entity and spatial candidates. Character
support translation plus fall/peak heights shift with the local frame;
velocities, rotations, support-local anchors, stable IDs, voxel addresses,
source revisions, and authored meaning do not. Voxel collision, navigation,
chunk mesh translations, and static collision are rebuilt into the same local
frame. Commit rechecks every guard and swaps origin, entities, and spatial
scene together. A stale or failed prepare/commit publishes nothing.

Triggers consume the rebased entity candidate, so overlap pairs remain stable.
Authored objects participate after their normal admission to stable entities.
Retained entity and voxel projectors observe only the accepted bounded local
facts; they retain their existing handles and cannot select or mutate an origin.

## Ownership and execution order

1. Downstream retains exact global positions for the objects it moves and
   updates them from accepted local motion through the current origin.
2. Downstream decides whether and where to rebase, then supplies one guarded
   request. Engine does not monitor a player or choose a threshold.
3. Engine prepares every participant and derived spatial projection without
   changing live state.
4. Engine atomically commits the origin, entity facts, and voxel/static spatial
   scene, advancing one rebase revision.
5. Trigger reconciliation and retained projection run from the accepted state.
   They may publish ordinary derived receipts but cannot become rebase authority.

Persistence policy remains downstream-owned. Engine supplies strict snapshots
for `GlobalPosition` and `WorldOriginState`; a save may persist either, both, or
neither according to product policy. Local transforms are reconstructable
runtime facts and are not a substitute for a saved global position in a
large-world product.

## Precision envelope

The default local envelope is 16,384 world units on each axis. At that range an
`f32` unit in the last place is at most about 0.002, while ordinary first-person
motion stays much closer to zero under a sensible downstream threshold. The
configured envelope is validated and reported; exceeding it is a typed prepare
failure, never a saturating conversion. The global integer-cell representation
retains signed identity far beyond the current voxel authority limit.

## Non-goals

- No automatic floating-origin scheduler or player-centric policy.
- No world streamer, persistence layout, save cadence, or migration policy.
- No renderer-selected origin and no global-coordinate GPU contract.
- No ambient registry or universal game/world owner.
- Objects outside the supplied `EntityState` remain the caller's explicit
  responsibility and cannot be claimed as rebased. Every root transform inside
  the supplied state is required at preparation time.

## Acceptance evidence

Focused tests must cover repeated positive/negative rebases, stale and invalid
requests, save/load/order stability, character support continuation, collision,
raycast/edit identity, navigation, triggers, authored admission, stable retained
handles, and local chunk projection. CraftSurvive supplies the product-owned
far-distance controller, targeting, streaming, and persistence evidence after
the accepted Engine branch is promoted to stable `main`.
