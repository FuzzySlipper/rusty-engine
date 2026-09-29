# Rope physics

Campaign #6992 introduced ropes; #8738 moved them into the retained Dynamics
world. This describes the current source. See
[SDK use](csharp-lifecycle.md#bounded-dynamics-ropes) for the generated API.

## Owners

`svc-collision::DynamicsSolver` is the only Rapier consumer. Each Dynamics world
keeps one live Rapier world holding its bodies, its static collision
environment and a maximum-distance `RopeJoint` per tether. These persist between
steps, so contacts, sleeping and solver warm starts carry over. Changes apply
directly to that world; nothing is prepared, revision-checked or rolled back.
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

With any rope present, a step runs four internal substeps with eight solver
iterations by default. `ConfigureRopes` changes them; each must be at least one.
These subdivide the caller's update; they are not another clock. Nothing caps
rope, bead or body counts, or reel speed. The product chooses its own work and
rates.

`maximum_length` is the current effective length. Each substep it moves toward
`target_length` by at most `reel_speed` × substep. Re-authoring a tether with the
same endpoints changes target, speed and contacts but keeps the effective length,
so a direct edit cannot snap a loaded rope. New endpoints make a new attachment.
Reeling performs physical work; energy is not conserved while shortening.

Readouts give endpoint positions, distance, effective/target length, slack,
taut/caught state and a force proxy. Rapier 0.34 keeps only the last internal
substep's limit impulse. The proxy divides it by the solver substep duration and
takes the maximum over a step's substeps. It matches a hanging mass's weight but
can miss catch peaks, so it is not a breaking tension. A catch is a
slack-to-taut transition that the solver tracks between steps.

## Character coupling

One optional tether participates in the character step. For a dynamic anchor,
the product calls `Dynamics.ObserveAnchor`. It reads the live body's anchor point,
point velocity, centre of mass and impulse response (inertia and locked axes
included). C# never duplicates those calculations.

The character integrates after controlled/external velocity, gravity and
platform departure, and before `move_and_slide`, relative to the anchor's point
velocity. When displacement exhausts slack it removes the outward radial
component and keeps the tangent. Corrections are collision-swept, never
teleported, within the existing query and recovery budgets. Terrain can leave
the constraint unresolved; the receipt reports that instead of corrupting motion.

The receipt carries a reaction impulse at the observed anchor point. The
character's effective mass and the anchor response share the correction, capped
on both sides by the maximum dynamic impulse; saturation is reported. The product
applies the reaction through `Dynamics.StepWithReactions` at its chosen update
order. The reaction is an ordinary impulse plus torque about the observed centre
of mass. It carries no revision or generation, and applying it twice applies it
twice. The character step never steps Dynamics itself.

## Evidence

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
- [#8738 evidence](evidence/retained-dynamics-8738/README.md) compares the
  retained world with the old per-step rebuild.

Rope meshes, cloth, general constraint graphs, climbing and animation remain
out of scope.
