# Rope physics

Ropes are maximum-distance tethers and bead chains in a retained Dynamics
world. See [SDK use](csharp-lifecycle.md#bounded-dynamics-ropes) for the
generated API.

## Owners

`svc-collision::DynamicsSolver` is the only Rapier consumer. Each Dynamics world
keeps one live Rapier world holding its bodies, its static collision
environment, a maximum-distance `RopeJoint` per tether and the
[limited joints](ragdolls.md) between bodies. These persist between
steps, so contacts, sleeping and solver warm starts carry over. Changes apply
directly to that world. Rapier has no rolling resistance, so after each
substep the solver slows the rolling spin of a sphere or capsule touching
anything: its surface speed loses 0.02 × gravity each second (a solid ball
rolling at 2 m/s stops after about 30 m), and a capsule lying on its side
comes to rest and sleeps.
The Dynamics bridge in `csharp-engine-services` maps generated handles onto
solver bodies, owns chains, and binds the Spatial collision scene.

The pinned dependency's `dynamics/joint/rope_joint.rs` implements a coupled
linear limit on a generic joint, with no locked axes. Spherical joints lock
translation and give no slack, so they are rods rather than tethers. Impulse
joints are used; the product never sees a generic joint graph.

## Tethers and chains

A tether has a caller-selected ID, two endpoints and a maximum distance. An
endpoint is a world point or a body plus local anchor. A world point becomes a
collider-free fixed Rapier body owned by the tether. A new attachment must be
within reach (0.001 m tolerance). Otherwise the joint would pull the bodies
together in one step, so it is refused as `dynamics-tether-out-of-reach`. Both
endpoints on the same body are refused. Destroying a body removes its ropes,
which then read as invalidated.

A chain is an anchored series of sphere beads joined by tethers. It models beads
on massless links, not a solid rod or continuous cable. Adjacent beads do not
collide; nonadjacent self-collision follows ordinary collision groups; terrain
collides normally. Gaps between beads have no collision geometry. Chain beads are
ordinary bodies in the world.

## Time, lengths and readouts

With any rope or limited joint present, a step runs four internal substeps
with eight solver iterations by default. `ConfigureRopes` changes them; each must be at least one.
These subdivide the caller's update; they are not another clock. Nothing caps
rope, bead or body counts, or reel speed. The product chooses its own work and
rates.

`maximum_length` is the current effective length. Each substep it moves toward
`target_length` by at most `reel_speed` × substep. Re-authoring a tether with the
same endpoints changes target, speed and contacts but keeps the effective length,
so a direct edit cannot snap a loaded rope. New endpoints make a new attachment.
Reeling performs physical work; energy is not conserved while shortening.

Readouts give endpoint positions, distance, effective/target length, slack,
taut/caught state and a force proxy. Rapier keeps only the last internal
substep's limit impulse. The proxy divides it by the solver substep duration and
takes the maximum over a step's substeps. It matches a hanging mass's weight but
can miss catch peaks, so it is not a breaking tension. A catch is a
slack-to-taut transition that the solver tracks between steps.

## Kinematic character coupling

One optional tether participates in the character step. For a dynamic anchor,
the product calls `Dynamics.ObserveAnchor`. It reads the live body's anchor point,
point velocity, centre of mass and impulse response (inertia and locked axes
included). C# never duplicates those calculations.

The character integrates after controlled/external velocity, gravity and
platform departure, and before `move_and_slide`, relative to the anchor's point
velocity. When displacement exhausts slack it removes the outward radial
component and keeps the tangent. Corrections are collision-swept, never
teleported, within the character controller's query and recovery budgets. Terrain can leave
the constraint unresolved; the step receipt's tether fact reports that
instead of corrupting motion.

The tether fact carries a reaction impulse at the observed anchor point. The
character's effective mass and the anchor response share the correction, capped
on both sides by the controller's maximum dynamic impulse (500 N·s by default);
saturation is reported. The product
applies the reaction through `Dynamics.StepWithReactions` at its chosen update
order. The reaction is an ordinary impulse plus torque about the observed centre
of mass; applying it twice applies it twice. The character step never steps
Dynamics itself.

## Floating bodies

`Dynamics.SetWater` gives a world water and names the bodies it acts on; it
replaces both each time and holds until replaced. Water is flat planes
(`DynamicsWaterPlane`: a horizontal surface over a rectangle of x and z, a
horizontal current, a density, and linear and angular drag rates) and
[water fields](water-fields.md) (`DynamicsWaterFieldEntry`). A floating body
takes the first field whose wet cells lie under its centre (the surface and
current that field's latest snapshot reports there, read as each step begins),
else the first plane whose rectangle holds its centre.

```csharp
engine.Dynamics.SetWater(new DynamicsWaterRequest(world,
    new[] { DynamicsWaterPlane.Still(min, max, surface).Flowing(new Vector2(1, 0)) },
    new[] { raft, barrel }));
```

Every solver substep, the part of the body's shape under the surface pushes up
at its centroid with the weight of the water it displaces, so an offset load
tips a raft until the shifted centre of buoyancy balances it, and it rights
itself when the load goes. Drag moves the body's velocity toward the current
and slows its spin, at the plane's rates per second scaled by the submerged
fraction. A light body on a stiff waterline bobs quickly; its drag settles it
(`DynamicsWaterPlane.SettlingDrag`, 3 a second, settles a 180 kg raft within a
few seconds).

- Submerged volumes and centres of buoyancy are exact for spheres and boxes,
  and for capsules upright or lying down; a tilted capsule's cylinder and
  ends are integrated in slices cut exactly by the surface, within 1e-6 of the
  submerged volume, from the first centimetre of immersion.
- `DynamicsReadout.Water` reports, for the body's latest step, whether it
  floats, its submerged fraction at the last substep, and the buoyancy, drag
  and water torque (the buoyant moment and the angular drag) averaged over the
  step's substeps.
- The coupling is one way: the water does not feel the body, and the body does
  not displace it.
- A floating body stays awake while in the water.

`fixtures/csharp-floating-bodies` floats a raft, a ball and a lying capsule at
their Archimedes drafts, tips the raft under a load, and holds a raft in a
river current on a tether.

## Tests

- `svc-collision` solver tests cover:
  - hanging weight and a single catch;
  - momentum exchange and determinism;
  - slack catch, swing and release energy;
  - off-centre anchors with unequal masses;
  - reeling rate and edit continuity;
  - reach and anchor refusal;
  - locked axes;
  - a settling stack;
  - static-scene rebinding;
  - rope survival across body replacement;
  - translation.
- The bridge tests cover chains, terrain contact, contact suppression,
  character reactions, light-anchor catch energy and world-origin rebasing.
- `svc-collision` floating tests cover box, sphere and lying-capsule drafts
  against Archimedes, capsule ends against closed forms and fine sampling,
  angular drag in the reported torque, tipping under an offset load against hydrostatics and
  righting, a current carrying a raft, and bodies outside the water or not
  named; the water field tests float a box on a field's river.

Rope meshes, cloth and climbing are not part of this service. Hinges, cones
and ragdolls are in [limited joints and ragdolls](ragdolls.md).
