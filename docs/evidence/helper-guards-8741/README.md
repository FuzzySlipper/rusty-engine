# No revision guards or candidate edits in the service helpers (#8741)

C# entity and inventory helpers now write directly. The Rust static-mesh
collider guard that compared a revision to itself is gone. About 1,800 lines
were deleted and 290 added, including rewritten exercises.

## Removed

**`EntityStore`:**
- `PrepareBatch` and `EntityEdit`, together with the staging store behind them:
  `ForkForEdit`, copy-on-write tables and records, and the staging flag.
- The expected-revision parameter of `Set`, `Remove`, `SetLifecycle`,
  `Destroy`, `SetContainment`, `ClearContainment`, `Commit` and
  `EntityBatch.Set`.
- What stays: `Commit(batch)` applies a batch's writes in order and returns the
  revision before and after. The revisions are change counters callers may
  read; nothing checks them.

**Entity adapters** (character, Dynamics, kinematic, motion, trigger,
world-origin, graphics):
- the guard record types and `expectedGuard` parameters;
- re-capturing and re-validating the whole projection after the native call;
- the caller-supplied `maximumEntities`, `maximumBodies`, `maximumActions` and
  `maximumFactReadback` caps;
- re-validation of trusted native receipts.

Each adapter now reads the store, calls the native service and writes the
results. Two further changes:
- `EntityKinematicMotion.Prepare(...).Apply()` becomes one `Step`.
- The trigger projection reads every fact rather than a truncated prefix.

**Inventory:**
- Single operations (`Grant`, `Consume`, the transfers, split and merge, the
  unique-item and equipment operations) no longer run through a cloned
  candidate store. Each one was already atomic on its own: checks, then one
  write.
- Removed:
  - `Prepare(expectedRevision)`'s parameter;
  - the whole-store `ValidateStore` pass, which ran twice on every publish;
  - `InventoryEdit.Validate`.

**Rust:**
- The `expected_revision` parameter of static-mesh collider replacement and
  residency (`svc-collision`, `engine-spatial`). Every production caller passed
  the scene's current revision.
- The now-unreachable `StaticMeshCollisionError::RevisionMismatch`.

**Examples:**
- `EntityAdapterSafetyExercise`, whose only purpose was stale-publication
  rejection.
- The stale-guard sections of the entity example, which were rewritten to the
  direct behavior.

## Kept, and why

**`InventoryEdit` grouping.** Dagger's `CommitAtomic` consumes a payment and
grants an item together (#8760 survey). A capacity failure on the grant must not
keep the payment. The edit is now cheaper:
- its working copy shares inventory and equipment states, which operations
  replace rather than mutate;
- it copies only the containment child sets, which change in place;
- `Publish` swaps the working copy in.

**One check on `InventoryEdit.Publish`.** It refuses if the store changed
directly after the edit began. The swap would otherwise silently drop that
change: the addressable-stacks exercise does a direct `Consume` between
`Prepare` and `Publish`, and without the check that consume was lost. The cost
is one integer comparison per publish. The edit records the revision itself;
callers no longer pass one.

## Cost (`scripts/HelperCostBench.cs`, `results/helper-cost.json`)

Release build, two runs each, on the SDK from before (`eba944ce`) and after:
- a store of 2,000 inventories × 10 stacks;
- a store of 5,000 entities.

| Operation | Before | After |
|---|---|---|
| One `InventoryStore.Grant` | 3.49–3.53 ms | 6.8–6.9 µs |
| `InventoryEdit` with two grants, published | 3.49–3.62 ms | 53–55 µs |
| `EntityStore.Commit` of a one-write batch | 758–787 µs | 2.2–2.3 µs |

Before, every inventory operation cloned and validated the whole store, and
every entity batch copied the entity table.

## Exercises

- **`Rusty.Engine.Mechanics.Example`** passes. It covers addressable stacks,
  grants, transfers, split and merge, capacity refusal, and a grouped edit that
  fails and leaves the store unchanged. It also covers an edit refused after a
  direct store change.
- **`Rusty.Engine.Entities.Example`** passes. It covers:
  - containment and component writes;
  - a batch whose failing write leaves the earlier writes applied;
  - class components keeping their identity across a batch;
  - all seven adapters' successful paths (character, Dynamics, kinematic,
    motion, trigger, world-origin, graphics).
- **`Rusty.Engine.Application.Example`** builds again. It was already broken:
  its fake context lacked `Video` and `RenderOutput`.
- **`scripts/test-csharp-sdk-package.sh --coreclr-smoke`** passes.
- **Rust:** workspace tests pass (1,227, excluding `renderer-webview-host`,
  which needs GTK). Clippy is clean apart from the pre-existing #8757 lints.

## Migration

Products pin their SDK version, so nothing breaks until they move.

**rusty-dagger**
- `InventoryStore.Prepare(expectedWorldRevision ?? worldRevisionBefore)`
  becomes `Prepare()`, in `MechanicsInventoryContainerCoordinator`.
- `Prepare()` on its other edits is unchanged.

**rusty-d20**
- `Entities.PrepareBatch(batch, Entities.Revision).Publish()` becomes
  `Entities.Commit(batch)`.
- Where the code keeps an `EntityEdit prepared = ...` variable, the same change
  applies.

**rusty-craftsurvive**
- `projection.Publish(entries, MaximumProjectedEntities, null)` becomes
  `projection.Publish(entries)`.
- `store.Destroy(entity, store.GetEntityRevision(entity))` becomes
  `store.Destroy(entity)`.

**Adapter callers**
- Drop the guard and cap arguments.
- Kinematic callers replace `Prepare(...).Apply()` with `Step(...)`.

## Not in this task

- **The Dynamics anchor-reaction protocol** (observe → propose →
  `StepWithReactions` with generation checks) belongs with the retained Rapier
  world in #8738. The #8760 survey found no stale-observation case at its only
  caller.
- **Product-supplied revisions on other services** have their own tasks:
  - voxel edit, residency and history: #8739;
  - triggers and world origin: #8754;
  - character and kinematic receipts: #8755.
- **Persistence `PersistenceRevisionGuard`** stays. It is a real check against
  what is stored on disk.
- **`EntityMotionService::apply` / `FirstPersonMotionService`** in
  `engine-spatial` still take an expected revision, but only tests call them.
  That is legacy `EntityState` surface for #8738 and #8745.
