# Soft bodies

A soft body is a deformable body in a Dynamics world: cloth (flags,
banners, cloaks) or an elastic volume (a jelly, a bouncy creature). Its
particles are joined by edges, bending constraints and, for volumes,
tetrahedral cells. It collides with the world's static environment, its
bodies and other soft bodies, and the Engine draws it.

```csharp
// A banner pinned along its hoist, blown by the wind, drawn two-sided.
DynamicsSoftBodyRequest shape = DynamicsSoftBodyRequest.Cloth(world,
    origin: new(0, 4, 0), across: new(.1f, 0, 0), down: new(0, -.1f, 0),
    columns: 21, rows: 13, mass: .6f);
uint[] hoist = [.. Enumerable.Range(0, 13).Select(row => shape.ClothParticle(0, (uint)row))];
DynamicsSoftBody flag = engine.Dynamics.CreateSoftBody(shape with { Pinned = hoist });
Appearance look = engine.Dynamics.CreateSoftBodyAppearance(new(flag, flagMaterial, TwoSided: true));

// Every update: wind, then the step; publish the appearance at the origin.
engine.Dynamics.SetSoftBodyForces(DynamicsSoftBodyForcesRequest.Blowing(flag, new(0, 0, 7)));
engine.Dynamics.Step(new(world, dt, 1, ReadOnlyMemory<DynamicsAction>.Empty));
DynamicsSoftBodyReadout now = engine.Dynamics.ReadSoftBody(flag);
```

`fixtures/csharp-soft-bodies` flies that flag against a held ball, squashes
a jelly under a dropped ball or a press, and makes a blob jump.

## Owners

Rapier 0.36's soft bodies do the simulation (with its `fem` feature).
`svc-collision::DynamicsSolver` keeps them in the world's one live Rapier
world beside its bodies, so they share contacts, islands and steps. The
Dynamics bridge maps the generated handles onto them, resolves joint
attachments from evaluated poses, and reshapes their appearances. A
Graphics appearance draws the surface; the product only publishes it.

## Shapes

`DynamicsSoftBodyRequest` names a `DynamicsSoftBodyShape` and reads the
fields it uses. Positions are in the world. The builders fill in the rest:

| Shape | Builder | Made of | UVs |
|---|---|---|---|
| `Cloth` | `Cloth(world, origin, across, down, columns, rows, mass)` | a sheet with structural, shear and bending edges | 0 to 1 across the columns and down the rows |
| `Box` | `Box(world, center, halfExtents, particles, mass, youngModulus)` | tetrahedra on a grid | the front (x, y) projection |
| `Shell` | `Shell(world, center, radius, subdivisions, mass)` | an icosphere with bending across its edges, keeping its volume | longitude and latitude |
| `Surface` | `Surface(world, positions, uvs, triangles, mass)` | the triangles' vertices as particles: a cloak, a banner of any shape | as given |
| `Solid` | `Solid(world, positions, uvs, tetrahedra, mass, youngModulus)` | the given tetrahedra; drawn as their boundary | as given |
| `Filled` | `Filled(world, positions, uvs, triangles, cellSize, mass, youngModulus)` | a closed mesh filled with tetrahedra; drawn as the mesh riding them | as given |

`ClothParticle(column, row)` is a cloth's particle index. Settings on every
request (use `with`):

- `Mass` spreads evenly over the particles. `LinearDamping` (0.1 from the
  builders) and `GravityScale`.
- `ParticleRadius`: the surface's contact thickness, so the body rests that
  far from what it touches. 0 takes half the mean edge, which for a coarse
  body leaves a visible gap; the fixture uses 1.5–2 cm.
- `CellModel`: `Volume` (each cell keeps its volume; a cheap jelly),
  `Corotational` (linear elasticity of `YoungModulus` and `PoissonRatio`) or
  `NeoHookean` (stiffer squeezed, softer stretched; twice the cost).
- `Solver`: `Constraints` (swept with the contacts each substep) or `Fem`
  (implicit finite elements over the whole body: stiff bodies hold their
  shape at a factorization per step).
- `VolumePreservation` with `VolumeFactor` (above 1 inflates),
  `ShapeMatching` (pulls toward the rest shape), `SelfContacts`.
- `MeshCollision` (`Filled` only): meet the world through the mesh rather
  than the cells' boundary, which stands off the mesh by up to a cell. The
  mesh's triangles must be at least as fine as the cells.
- `CollisionGroups`, `CollisionMask`, `Friction`, `Restitution`, as for
  bodies. `ExtraSubsteps` and `ExtraIterations` (3 by default) buy accuracy
  for the body and what it touches. `CanSleep`.

`DynamicsSoftBodyMaterial` holds each constraint family's stiffness as a
spring frequency (Hz) and damping ratio (`Edge`, `Bend`, `Volume`,
`ShapeMatching`), elastic cells' `YoungModulus`, `PoissonRatio` and
`ElasticDampingRatio`, `DeformationDamping` (damps wobble, not motion),
plastic flow of cells and edges (`PlasticYield`, `EdgePlasticYield` and
their creep and maximum), and tearing (`TearStrain`, `TearForce`,
`TearSmoothing`, `InteriorStrength`, `MaxTearsPerStep`, `MinPiece`; zero
never tears). `Default` is Rapier's; `Cloth` stiffens edges and softens
bending; `Elastic(youngModulus)` sets a solid's stiffness; `Tearing(strain)`
adds a tear threshold. `UpdateSoftBodyMaterial` changes a live body's
material and volume preservation.

## Holding and moving particles

A pinned particle and a driven one are both moved, never pushed; they
differ in who says where they go.

- **Pins** (`Pinned` at creation, `SetSoftBodyPins` after) hold particles
  where they are. Setting pins again releases the ones not listed.
- **Targets** (`SetSoftBodyTargets`) drive particles along a straight path to
  a world position by the end of the next step, so what they drag sees their
  velocity. Set them every step to move along a path. A target not listed
  again goes free, unless the particle is pinned: it then holds where the
  drive left it.
- **Body attachments** (`SetSoftBodyBodyAttachments`) hold each particle to
  the point of a Dynamics body where it is when attached, both ways: the
  body drags the cloth and the cloth pulls on the body. Destroying the body
  ends them; `ReplaceBody` keeps them.
- **Joint attachments** (`SetSoftBodyJointAttachments`) drive particles to
  offsets in an animated instance's joints (`Animation.ReadJoints` indices).
  With `CaptureOffsets` each offset is where the particle is now in its
  joint's frame (the instance needs a pose: publish it a call before).
  Before each step the Engine places each offset by the evaluated pose and
  carries it forward by the pose's latest motion, so the edge keeps up with
  the pose drawn this call; with no pose it holds the last targets. An empty
  list ends them. Joint targets win over product targets for one particle.

Particle indices are the body's own. A tear that splits a piece off makes
it a body of its own, read and drawn with this one, whose particles cannot
be addressed.

## Forces

`SetSoftBodyForces` holds until set again:

- `Acceleration` pushes every free particle alike.
- `Wind` with `Drag`: air moving at `Wind` pushes each surface triangle along
  its normal by `Drag × area × v|v|`, `v` the air's speed through it. A
  flag edge-on to the wind feels nothing; one flapping across it feels the
  most, so it flutters. `FlagDrag` is 0.6 (half the air's density times a
  flat plate's drag coefficient). The product owns the weather: vary the
  wind each step for gusts.

`ApplySoftBodyVelocityChange` changes every free particle's velocity within
a radius of a point, fading to nothing at the radius (0: the whole body):
a hit, a press, a jump. `ApplySoftBodyRadialVelocityChange` pushes them away
from a centre (negative pulls in).

## Collision

Soft bodies collide with the static environment from `BindWorldCollision`
(voxel chunks and static meshes), with bodies (both ways) and with each
other, by `CollisionGroups` and `CollisionMask`. Their contacts are counted
in `ReadSoftBody`'s `ContactCount`, not in the bodies' contact facts.
`SelfContacts` lets a surface collide with itself; the tests check only that
a folding self-colliding sheet steps finite and stays above the floor.

## Time

While a world holds a soft body, `ConfigureSoftBodies` sets how many
substeps each step takes (1 by default; ropes' substeps when the world also
holds ropes, if more), how many extra substeps a body hit fast may take (4),
how much stiffer soft contacts are than rigid ones (4) and the strain past
which a constraint is solved again after the contacts (0.75). A sleeping
body is not redrawn.

## Drawing

`CreateSoftBodyAppearance(softBody, material, twoSided)` returns a Graphics
`Appearance` drawing the body's surface with the material: the cells'
boundary or the cloth's triangles over the particles, or a filled mesh's
own triangles. Publish it like any appearance, with an `AppearanceFact` at
the origin, as its vertices are in the world. `TwoSided` also draws each
triangle's back with its own normal, for cloth under a one-sided material.
Dispose it as any appearance (after a snapshot without it). It keeps its
last shape once its soft body is gone.

After each step that moved the body, the Engine replaces the appearance's
retained mesh in place: positions, smooth area-weighted normals, the UVs
the body was made with (a tear's copies of a particle keep its UV) and
fresh bounds. It is one committed description like any other, so the
window, the stream, captures, snapshots and offline renders draw the same
shape. The renderer draws it as a static mesh instance whose payload was
replaced (`RenderDiff::ReplaceMeshPayload`), with the appearance's
material. The replaced mesh carries no previous positions: temporal
anti-aliasing and upscaling see the deformation without motion vectors.

## Readouts

`ReadSoftBody` gives, with every piece torn off it: particle and piece
counts, centre of mass, mean velocity, bounds, enclosed and rest volume of
its closed pieces, the largest edge stretch, the contacts it touches,
whether it sleeps and a topology version that changes with each tear.
`ReadSoftBodyParticles` gives each particle's position, velocity, mass,
whether it is kinematic (pinned or driven) and whether it is on the
surface.

## Lifecycle

Disposing a soft body removes it and every piece of it. Destroying its
world ends it. `RebaseWorldOrigin` moves its particles and its product
targets with the world; joint targets follow the pose. `UpdateBody`,
`ReplaceBody`, ropes, joints and ragdolls behave as before in a world that
also holds soft bodies.

## Cost

A soft body's step cost grows with its particles and constraints, its
substeps (`ConfigureSoftBodies`, `ExtraSubsteps`) and its cell model and
solver. The fixture's `soft.cost <columns>`
times a flag of that many columns and rows in wind against the same world
empty, and `soft.inspect` reports its scene's mean step time.

## Limits

- At one substep, cloth sweeping fast into a body can pass into it: the
  fixture's flag, first blown against its held ball, reached 5 cm inside.
  Three `ExtraSubsteps` on the flag kept it outside.
- A filled mesh's cells stand off the mesh: a 0.6 m cube filled at 0.2 m
  cells rests about 0.29 m above the floor unless `MeshCollision` is on.
- The contact thickness is the particle radius; coarse bodies at the default
  float visibly.
- A whole surface is re-sent after every step that moves it (positions,
  normals and UVs; twice the vertices when two-sided).
- No per-vertex motion vectors (see Drawing).
- Joint attachments follow the pose one call behind, carried forward by its
  latest motion; a sudden change of motion lags a step.

## Tests

- `svc-collision` soft-body tests: a banner hangs from its pins, swings
  downwind in wind and back when it calms; cloth drapes over a held ball and
  the floor without passing through; a jelly squashes landing and recovers
  its height and volume; a falling ball dents a jelly; targets drive pinned
  and free particles and release them; a body attachment pulls both ways
  and ends with the body; a rebase moves particles and targets; the drawn
  surface's UVs, normals, bounds and two-sided back; a torn banner draws
  every piece with its UVs; a filled mesh draws its own triangles;
  self-contacts and FEM bodies step finite; validation and removal.
- Bridge tests: an appearance is reshaped by the steps that move its body,
  and not once the body is gone; joint attachments carry cloth with a posed
  character rising half a metre in a call, and end.
- `render-projection` sends a deformed mesh's payload only when it changes;
  `render-wgpu` draws a replaced static-mesh payload as a new renderer given
  the retained world draws it.
