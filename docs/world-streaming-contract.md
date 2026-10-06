# World streaming and state contract

These capabilities are part of the ordinary generated C# SDK. The packaged
[`csharp-world-streaming`](../fixtures/csharp-world-streaming/README.md)
fixture exercises them.

## Movement modes (#8609)

Set `CharacterControllerCommand.Movement` when calling
`Spatial.ProposeCharacterStep`. Walking (the default) is the ordinary
walking/airborne, jump, stance, step, slope, platform and tether behavior.
Swimming, climbing and flying use that same collision sweep and contact solver;
products publish the returned transform and motion through their ordinary state.

- **Swimming:** supply the selected water AABB as `Minimum`/`Maximum`, speed,
  acceleration, drag, gravity scale, buoyancy and vertical intent (-1..1).
  The Engine derives immersion from the capsule's vertical extent when its
  center is inside the water's horizontal bounds. Gravity becomes
  `gravity * (buoyancy * immersion - gravityScale)` and water drag is exponential.
  Outside the volume, ordinary walking/airborne behavior resumes. Water geometry
  is environmental input, not solid voxel occupancy or a fluid simulation.
- **Climbing:** supply a vertical rail's bottom/top **character-center** positions,
  `ClimbReach`, speed and vertical intent. Within reach and the height interval,
  the Engine approaches the rail through collision sweeps, disables gravity and
  clamps travel to its endpoints. A blocked rail cannot teleport the character
  through a wall or ceiling. Selecting Walking releases it; moving out of the
  attachment interval also returns to ordinary motion. Products choose eligible
  ladders/surfaces and top/bottom dismount destinations.
- **Flying:** speed, acceleration, drag and three-axis intent produce swept
  motion without gravity or floor snap. This supports collision-aware authoring
  and debug movement; eligibility and bindings remain product policy.

`CharacterStepReceipt.Movement` reports accepted mode, immersion,
`HeadSubmerged`, climb attachment and endpoint facts at the accepted position.
Products own breath timers, drowning consequences, stamina, animations and
selection among overlapping volumes/rails. These facts are not a breathing timer.
The request is read during the call and must be supplied on each step; the
Engine keeps no environment registry and no second simulation clock.
See [movement source](../rust/crates/engine-spatial/src/character_modes.rs) and
[solver tests](../rust/crates/engine-spatial/tests/character_modes.rs).

## Per-cell state (#8610)

A solid cell stores a material slot and fifteen state bits: two low bits encode
quarter turns about +Y; thirteen upper bits encode a product-defined variant or
stage (0–8191). Use `VoxelCellState.Encode(quarterTurns, variant)` in C#.
A quarter turn maps authored +X to world -Z. State zero preserves ordinary
unrotated cells. Material IDs retain their own 1–4095 range; never pack state
into material IDs. Empty cells have state zero.

`VoxelEdit.State` and `VoxelReadout.State` provide write and read access. `VoxelResidencyTransaction.States` is either empty (all default)
or parallel to the complete material-slot array; operations use the same offsets.
Input arrays are copied before the call returns. State-only changes advance
source/projection revisions, invalidate affected meshes and participate in chunk
and authority hashes.

GreedyCubes groups only cells with equal material **and state**. It rotates
texture coordinates and maps geometric faces back to authored local faces.
`VoxelSceneFaceMaterialBinding.Variant` selects a variant-specific authored face
material; missing overrides fall back to variant zero, then the base material.
Those are ordinary Appearance materials, including atlas-region materials.
`ReadMaterialMapping` includes variant in its provenance rows. Collision remains
cube occupancy: rotating state does not turn a cell into a door-shaped collider.
Nonzero state requires a material drawn as GreedyCubes; reconstructed smooth
surfaces have no authored cube-face orientation and reject it explicitly.

The stable packed cell encoding remains 32 bits (material, solid tag, state).
Dense C# state input costs four extra bytes per cell when provided; it is optional
for default-state chunks. Rust in-memory layout and full scene/mesh costs are
reported by the [budget probe](voxel-budgets.md). Varying states can prevent greedy
merging, so state diversity affects geometry cost even for a solid volume.
Rich inventories, articulated doors and behavior belong to product objects;
an ordinary block keeps its orientation and stage without an entity.

## Engine call affinity (#8611)

All `IEngineContext` services and their native handles are **callback-confined**:
call them synchronously within the Engine-invoked product construction,
lifecycle, update or debug callback that permits the operation. Do not call
any service from `Task.Run`, a timer, a finalizer, an arbitrary thread, or an
async continuation after that callback returns. Read-only queries and handle
`Dispose` are included. Individual operations still have narrower lifecycle
rules (for example input remapping); this contract does not widen them.

This is a serialized callback lane, not a promise of one permanent OS/managed
thread ID or an installed `SynchronizationContext`. Host operations share the
[runtime session guard](../rust/crates/product-host/src/session.rs), but that
lock does not protect arbitrary product worker calls into native function
pointers. The [spatial bridge](../rust/crates/csharp-engine-services/src/spatial.rs)
contains mutable native state and `Rc<RefCell<...>>`; generated wrappers do not
marshal worker calls. Both CoreCLR and NativeAOT use this contract. Product
code is trusted, so there is no extra locking, thread negotiation or defensive
dispatch.

`SimulationScheduler.Advance` executes callbacks synchronously on the caller's
admitted update path. It does not start workers, preempt expensive work or make
an Engine call asynchronous. Split work into bounded pieces before scheduling
it. ApplyResidency blocks until it returns; it rebuilds only the changed chunks.

Pure product computation may run on product-owned workers using **copied,
product-owned values only**, without Engine services, mutable product authority
or `EntityStore` access. Transfer results through a bounded
mailbox and admit them in a later Update. Tag results with product generation
and chunk revision, discard stale work, and cancel/drain on restart/disposal.
Do not await a worker that needs a callback-held Engine operation. This is
ordinary C# computation, not an Engine worker facility or a second simulation
clock. Content/persistence service calls still belong on the callback lane.

### Residency and edits apply in place

`Voxel.ApplyResidency` admits, replaces and evicts whole chunks, and
`Voxel.ApplyEdits` changes cells. Both write into the live scene and rebuild
only the changed chunks' meshes and colliders. Admitting an identical chunk
or evicting a non-resident one is a no-op; a batch that changes nothing returns
a receipt with zero changes, and an edit batch returns `NoChanges`.

The Engine keeps no undo history. The product owns undo: it applies inverse
edits. It also owns which chunks stay resident. The costs are in
[the voxel budgets](voxel-budgets.md); admitting a chunk costs about what
building that one chunk costs.
See [the local change tests](../rust/crates/engine-spatial/tests/voxel_local_changes.rs)
and the packaged streaming fixture for executable usage.

Resident reconstructed chunks far from the camera can be drawn from coarse
meshes with `VoxelScenePresentation.SetLevelOfDetail`; residency and collision
are unchanged. See
[distance level of detail](smooth-voxel-surfaces.md#distance-level-of-detail)
and `fixtures/csharp-voxel-lod`.

## Residency and remesh budgets (#8612)

See [voxel residency and edit costs](voxel-budgets.md) for limits, CPU and
memory measurements, and geometry diversity.
