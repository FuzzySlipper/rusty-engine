# Rigid-body dynamics

## Decision

Task #6531 reopens the earlier dynamic-physics deferral for a concrete downstream
consumer. Rusty Engine will use `rapier3d-f64` 0.34 in `svc-collision`, with default
features disabled and the `std` plus `enhanced-determinism` features enabled.
Rapier 0.34 depends on `parry3d-f64` 0.29, so the existing sole direct Parry edge
advances from 0.28 to 0.29. The workspace must contain one Parry version and no
second collision geometry stack.

Rapier is justified because the required behavior includes angular inertia,
friction and restitution contact response, sleeping, islands, continuous
collision, dynamic pairs, and static triangle environments. Extending the
existing kinematic axis sweeps into a bespoke solver would create an incomplete
and hard-to-audit physics engine. Rapier and Parry remain implementation details:
their handles, sets, events, and serialization do not cross the public service
boundary.

## Authority and execution

- `entity-state` stores schema-1 `RigidBodyComponent` facts under the stable
  `rusty.entity.rigid-body` identity. Transform and rigid-body slots are the
  durable authority; component revisions are live optimistic-concurrency guards
  and are reacquired after reopen.
- `svc-collision` owns the selected solver and derived collision/dynamics state.
  Voxel and exact admitted static-triangle projections are its static
  environment input. Solver caches, islands, contact manifolds, and handles are
  never save authority.
- One named `engine-spatial` rigid-body service validates a bounded request,
  prepares the complete candidate step off-side, atomically publishes exact
  entity slots, and only then advances its immutable derived readout. The
  public `prepare`/`commit` split permits useful caller work between those
  phases; any intervening transform or rigid-body slot change rejects the
  entire candidate as stale.
- Downstream Rust owns fixed-step scheduling, catch-up choice within Engine's
  bound, forces and impulses selected by gameplay, spawn/despawn, consequences,
  complete saves, and presentation.
- The existing `KinematicMotionSystem` remains the character/controller path.
  A rigid body and kinematic component on the same entity is rejected by the
  rigid-body service before simulation or publication.

This is not an ECS scheduler, a universal physics session, an ambient update
loop, or a component callback. A caller invokes the service directly.

## Numeric and repeatability contract

Canonical entity transforms and velocities remain bounded `f32` facts. The
backend performs collision and dynamics in `f64`, converting only at the typed
service border. Inputs are sorted by stable entity identity, the solver is
single-threaded, and `enhanced-determinism` removes architecture-dependent
transcendental implementations where the backend supports it.

The supported guarantee is deterministic repetition for identical canonical
inputs, fixed-step sequence, Engine/backend build, target, and floating-point
environment. It is not a promise of bit-identical results across different
Rapier releases, targets, CPUs, compiler settings, or step schedules. Durable
saves contain canonical facts, not solver caches, and continuation tests compare
the documented canonical tolerance after cache rebuild.

Discrete bodies are admitted only below the service's per-step translation
limit. Faster bodies must opt into continuous collision and remain below its
larger explicit limit; exceeding either is a typed rejection before mutation.
Dynamic triangle meshes are unsupported. Schema 1 admits spheres, cuboids, and
local +Y capsules with solver-derived inertia from a positive authored mass.

## Bounded service contract

One request admits at most 1,024 bodies, 4,096 actions, 4,096 returned active
contacts, and 8 fixed substeps. A step is in the inclusive range 1/1000 through
1/15 second as represented by the public `f32` request. Islands are implicitly
bounded by the admitted body count. The discrete translation estimate is at
most 1 cell-space unit per substep; CCD raises that explicit ceiling to 100 but
does not remove it. Forces persist across all requested substeps, while impulses
are applied exactly once.

`RigidBodyStepReceipt` contains the before/after motion facts for every body and
the active contacts observed after the final substep. Contact facts identify a
dynamic pair or a dynamic body against the canonical static environment; they
are bounded observations, not gameplay events or durable collision history.
The service rebuilds Rapier state from canonical entity and spatial facts for
each candidate, so snapshot reopen needs no solver serialization.

The narrow entity-state publication primitive admits at most 1,024 unique
transform/body pairs. It validates all candidates and both exact component slot
revisions before the first write, advances the entity-state revision once, and
does not generalize into a heterogeneous transaction language.

## Dependency audit

The intended normal dependency closure is:

```text
engine-spatial -> svc-collision -> rapier3d-f64 0.34 -> parry3d-f64 0.29
                               -> parry3d-f64 0.29 (direct query owner)
```

`entity-state` has no Rapier or Parry dependency. Ordinary Rust physics has no
renderer, browser, Studio, Node, URL/fetch, filesystem, or product dependency.
`cargo tree -d` and the repository dependency-boundary audit own the executable
duplicate/boundary proof.
