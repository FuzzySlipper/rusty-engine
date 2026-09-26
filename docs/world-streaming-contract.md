# World streaming and state contract

These capabilities are implemented through the ordinary generated C# SDK.
Authorized Engine work does not require an identified or already-running
consumer; Engine fixtures can prove capabilities while a product is in development.

## Movement modes (#8609)

Set `CharacterControllerCommand.Movement` when calling
`Spatial.ProposeCharacterStep`. Walking (the default) retains the existing
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
The request is copied for the call and must be supplied on each step; no second
simulation clock or retained environment registry is introduced.
See [movement source](../rust/crates/engine-spatial/src/character_modes.rs) and
[solver tests](../rust/crates/engine-spatial/tests/character_modes.rs).

## Per-cell state (#8610)

A solid cell stores a material slot and fifteen state bits: two low bits encode
quarter turns about +Y; thirteen upper bits encode a product-defined variant or
stage (0–8191). Use `VoxelCellState.Encode(quarterTurns, variant)` in C#.
A quarter turn maps authored +X to world -Z. State zero preserves ordinary
unrotated cells. Material IDs retain their own 1–4095 range; never pack state
into material IDs. Empty cells have state zero.

`VoxelEdit.State`, `VoxelReadout.State` and `VoxelAtReceipt.State` provide write
and read access. `VoxelResidencyTransaction.States` is either empty (all default)
or parallel to the complete material-slot array; operations use the same offsets.
Input arrays are copied before the call returns. State-only changes advance
source/projection revisions, invalidate affected meshes and participate in chunk
and authority hashes. Undo/redo and history export/restore preserve both sides
of state changes. History schema **4** records this expanded authority.

GreedyCubes groups only cells with equal material **and state**. It rotates
texture coordinates and maps geometric faces back to authored local faces.
`VoxelSceneFaceMaterialBinding.Variant` selects a variant-specific authored face
material; missing overrides fall back to variant zero, then the base material.
Those are ordinary Appearance materials, including atlas-region materials.
`ReadMaterialMapping` includes variant in its provenance rows. Collision remains
cube occupancy: rotating state does not turn a cell into a door-shaped collider.
Nonzero state requires GreedyCubes; reconstructed smooth surfaces have no authored
cube-face orientation and reject it explicitly.

The stable packed cell encoding remains 32 bits (material, solid tag, state).
Dense C# state input costs four extra bytes per cell when provided; it is optional
for default-state chunks. Rust in-memory layout and full scene/mesh costs are
reported by the [budget probe](voxel-budgets.md). Varying states can prevent greedy
merging, so state diversity affects geometry cost even for a solid volume.
Rich inventories, articulated doors and behavior still belong to product objects;
ordinary blocks no longer require an entity merely to retain orientation/stage.

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
[runtime session guard](../rust/crates/runtime-session/src/lib.rs), but that
lock does not protect arbitrary product worker calls into native function
pointers. The [spatial bridge](../rust/crates/csharp-engine-services/src/spatial.rs)
contains mutable native state and `Rc<RefCell<...>>`; generated wrappers do not
marshal worker calls. Both CoreCLR and NativeAOT use this contract. No extra
locking, thread negotiation or defensive dispatch has been added for trusted
product code.

`SimulationScheduler.Advance` executes callbacks synchronously on the caller's
admitted update path. It does not start workers, preempt expensive work or make
an Engine call asynchronous. Split work into bounded pieces before scheduling
it. Synchronous ApplyResidency still blocks until it returns; use the preparation path below to overlap projection building.

Pure product computation may run on product-owned workers using **copied,
product-owned values only**, without Engine services, retained leases, mutable
product authority or `EntityStore` access. Transfer results through a bounded
mailbox and admit them in a later Update. Tag results with product generation
and chunk revision, discard stale work, and cancel/drain on restart/disposal.
Do not await a worker that needs a callback-held Engine operation. This is
ordinary C# computation, not an Engine worker facility or a second simulation
clock. Content/persistence service calls still belong on the callback lane.

### Engine-owned background residency preparation

Within an admitted callback, call `Voxel.StartResidencyPreparation(transaction)`.
It copies the inputs and retains an immutable scene snapshot. One worker per
Spatial session builds the normal collision/navigation/mesh candidate; no managed
pointer, Engine lease or product authority crosses into that worker.

In later callbacks, call `PollResidencyPreparation(new(session, id))`.
Pending is nonblocking. Ready exposes candidate receipt facts without changing
the live scene. Call `CommitResidencyPreparation` to publish on the callback lane;
it returns Pending if preparation is not ready. Commit rechecks source/residency,
static collision, world-origin/rebase and chunk-lease generations. A conflicting
edit, rebase, collider update or lease change rejects the stale candidate instead
of overwriting newer state. Apply the returned revision and refresh ordinary
VoxelScenePresentation after Committed. History policy is explicit as on
synchronous residency; ResetToPublishedAuthority resets history at publication.

`CancelResidencyPreparation` discards the candidate and joins the bounded worker.
Session destruction does the same. Cancellation/teardown can wait for current
preparation to finish; polling does not. This prevents work retaining a retired
scene after its owner is gone. A terminal failed poll requires cancellation
before another start; a commit attempt consumes a ready preparation. Products
must keep their own generation and requested-region policy, and cancel unwanted
work during restart. Service calls, including poll/commit/cancel, remain on the
callback lane. Pure product generation may still use copied-data workers.

Preparation moves expensive projection construction off the callback, but is
not free: inputs are copied at start, the old and candidate scenes coexist,
commit checks hashes and may release old allocations, and rendering/upload is
separate. Measure those phases; no fixed frame-time guarantee is implied.
See [worker lifecycle tests](../rust/crates/engine-spatial/tests/voxel_preparation.rs)
and the packaged streaming fixture for executable SDK usage.

## Residency and remesh budgets (#8612)

See [voxel budget measurements](voxel-budgets.md) for limits, CPU/memory evidence,
geometry diversity and synchronous versus prepared publication costs.
