# World streaming and state contract

This is the current planning contract for C# products (tasks #8609–#8612).
It does not add movement modes, voxel metadata, or background Engine calls.
The public SDK surface remains the authority for available operations.

## Movement modes (#8609)

The Engine supports collision-swept walking/airborne character motion,
standing/crouched stance, jumps, steps, slopes, floor snap, platform carry,
external motion and optional tethers. `CharacterAirConfig` controls airborne
planar movement; it is not a swimming or flight mode. See the
[character source](../rust/crates/engine-spatial/src/character_controller.rs)
and [SDK character composition](csharp-sdk.md#character-tethers).

There is currently no water-volume admission, submersion/buoyancy solver,
water drag mode, ladder attachment, ledge-climb transition, or flight service.
A water-coloured solid voxel remains solid collision. Do not author a required
swim or ladder route against this SDK. Breath timers, stamina, drowning rules,
climb eligibility and animation selection are product policy; future Engine
water support should expose physical submersion facts rather than own these rules.

The intended boundary is Engine-owned swept motion, contact and attachment
constraints, with product-selected modes and environment meaning. Swimming
and climbing mechanisms remain deferred until a proving consumer supplies
water/climbable geometry, transitions and required collision behaviour. This
resolves #8609's explicit supported-direction alternative, not its proposed
swimming/climbing solver acceptance. CraftSurvive's initial no-fluid,
no-ladder scope does not require those mechanisms.

External velocity/impulses and vertical configuration can express a bounded
product-selected motion experiment using `ProposeCharacterStep`; they do not
establish stable buoyancy, ladder attachment or arbitrary collision-safe
flight. Record any such approximation as an approximation. Request the owning
Engine mechanism before relying on it for a required route. A free authoring
camera is camera policy, not a promise that a character can fly.

## Block state (#8610)

World voxel authority stores occupancy and a material slot, not orientation,
variant bits, growth stage, inventory or open/closed state. This is deliberate:
[the voxel value owner](../rust/crates/core-voxel/src/lib.rs) separates
metadata-bearing objects from bulk cells. Public slot values are transported
as `uint`; the scene accepts solid slots 1–4095, with zero representing empty
in payloads. Do not pack private state into spare slot bits.

Store one product entity/record per **semantic placed object**, not per face
or per ordinary terrain cell. Key its canonical state by stable product ID and
integer cell address; a door can occupy multiple cells while retaining one
state owner. Store orientation/stage there, save it with product persistence,
and rebuild derived Engine appearance/collision facts on restore. Region
annotations do not turn into a per-cell metadata channel.

- Unrotated cube variants may select distinct admitted material slots. This
  consumes slots and can split greedy mesh groups. Directional scene material
  bindings are per material slot/face, not per individual cell rotation.
- A rotated or articulated object uses ordinary retained Graphics placement
  and Engine collision facts. Clear conflicting terrain occupancy when it
  stops representing the object's shape; never leave a closed voxel collider
  behind an open door. Product action rules remain downstream.
- The entity record alone adds no renderer handle or collider. Create those
  only for objects that require them and reuse admitted resources. The Engine
  owns meshing and collision; do not build a second product voxel mesher.

### Sizing this choice

For a concrete compact save layout, three signed 64-bit cell coordinates,
one 16-bit definition ID, one orientation byte and one stage byte cost **28
bytes per object**, before stable ID, versioning and serialization framing.
Ten thousand such records therefore cost 280,000 bytes for those fields;
adding an explicit 64-bit identity makes that 360,000 bytes. These are packed
field arithmetic, not managed heap measurements or an Engine serialization ABI.
An `EntityStore` has dictionary/tree entries, entity records and component
storage; C# object alignment and container capacity add overhead. Measure that
heap separately instead of treating the packed number as resident memory.

Budget **N records**, up to **N retained appearances** and **N collision
instances** for N independent visible/colliding blocks, not 6N face entities.
Shared geometry/material resources are additional once per unique definition;
per-instance and native acceleration costs depend on shape and scene. This
route has no certified universal entity-count/frame-time budget. Cap active
objects by the product's measured frame/memory budget and keep distant records
unrealized. Bulk terrain should remain voxel residency; the sparse entity
choice avoids multiplying state and handles across every terrain cell.

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
it. A single large admission/remesh still blocks that callback until it returns.

Pure product computation may run on product-owned workers using **copied,
product-owned values only**, without Engine services, retained leases, mutable
product authority or `EntityStore` access. Transfer results through a bounded
mailbox and admit them in a later Update. Tag results with product generation
and chunk revision, discard stale work, and cancel/drain on restart/disposal.
Do not await a worker that needs a callback-held Engine operation. This is
ordinary C# computation, not an Engine worker facility or a second simulation
clock. Content/persistence service calls still belong on the callback lane.

### Supported step-budgeted example

The simplest first streaming implementation needs no workers:

```csharp
// Product fields: pending chunk requests, active generation, and a cursor.
// Called synchronously by IEngineProduct.Update or SimulationScheduler.
void AdvanceStreamingSlice()
{
    const int CellsPerSlice = 128; // Product tuning, not an Engine guarantee.
    if (pending is null) return;
    if (pending.Generation != generation) { pending = null; return; }
    pending.GenerateNextCells(CellsPerSlice); // Pure product data only.
    if (!pending.IsComplete) return;
    if (!MaySpendAdmissionBudgetThisStep()) return;
    // Invoke Voxel residency with the current scene revision and completed
    // payload here, then refresh VoxelScenePresentation on acceptance.
    AdmitCompletedChunk(pending);
    pending = null;
}
```

The named pending/generation/budget methods are product pseudocode, not SDK
APIs. Use current residency receipts and scene revisions, keep queue length
bounded, and retry only known rejected work—not an uncertain mutation. Loading
a cache does not bypass admission/remeshing cost. Measure a whole admission,
including collision/navigation and presentation, before promising a view
radius. World extent on disk is independent of the resident working set.

## Residency and remesh budgets (#8612)

See [voxel budget measurements](voxel-budgets.md) for existing hard limits,
reproducible CPU/memory measurements and a conservative scheduling policy.
