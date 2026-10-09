# Limited joints and ragdolls

`Dynamics.SetJoint` links two bodies of a Dynamics world with a hinge or a
cone, each with limits and damping. `Dynamics.CreateRagdoll` builds a ragdoll
from them: one body per chosen bone of an animated character. The ragdoll
spawns at the pose the character was drawn in and moving as it moved. While
it lives, the skeleton follows the bodies through
[pose overrides](skeleton-poses.md), mixed over the clips by a blend weight.

```csharp
// A call before the fall: the instance reports its joints.
engine.Animation.SetPose(AnimationPoseRequest.ReportOnly(resident));

// The fall, and the hit that causes it.
DynamicsRagdoll ragdoll = engine.Dynamics.CreateRagdoll(new(world, resident, bones, links,
    CollisionGroups: 1, CollisionMask: uint.MaxValue, Friction: 0.8f, Restitution: 0,
    LinearDamping: 0.05f, AngularDamping: 0.3f, Blend: 1));
engine.Dynamics.ApplyRagdollImpulse(new(ragdoll, Bone: 1, hitPoint, hitImpulse));

// Every update: step the world as usual; the skeleton follows its bodies.
engine.Dynamics.Step(new(world, dt, steps, ReadOnlyMemory<DynamicsAction>.Empty));
DynamicsRagdollResult fallen = engine.Dynamics.ReadRagdoll(ragdoll);

// Getting up: play a clip and blend the ragdoll out, then dispose it.
engine.Dynamics.SetRagdollBlend(new(ragdoll, weight));
ragdoll.Dispose();
```

`fixtures/csharp-ragdoll` drops the joint-attachment fixture's character from
its idle pose with a hit to the chest (`ragdoll.fall`), and blends it back to
idle (`ragdoll.getup`). `ragdoll.inspect` reports:

- when the ragdoll came to rest;
- how close its shapes came to the floor;
- the hinges' least flexion;
- its pose 480 steps after the hit, so that falls can be compared.

## Joints

A `DynamicsJointRequest` names a caller-selected joint ID, two bodies of one
world, and a frame on each body: a body-local anchor and rotation. The X axis
of a frame is the hinge or twist axis, and angles count from where the two
frames coincide.

- `DynamicsJointLimits.Hinge(min, max)` turns about X only, between `min`
  and `max` radians.
- `DynamicsJointLimits.Cone(swing, twistMin, twistMax)` keeps the second
  frame's X axis within `swing` of the first's (a round cone) and twists about
  it between `twistMin` and `twistMax`.
- `Damping` (N·m·s per radian) resists turning about the hinge or twist axis,
  so limbs settle instead of swinging. A cone's swing has no damping of its
  own: use the bodies' angular damping.

The joint's two bodies collide with each other only with `ContactsEnabled`.
Setting an existing ID replaces that joint, and `RemoveJoint` releases it.
`UpdateBody` keeps a body's joints. Destroying the body, or replacing it with a
new handle, removes them. Joints are Rapier
impulse joints in the world's live solver. A world with joints steps with
the same substeps and iterations as ropes ([rope physics](rope-physics.md)).

## Describing a ragdoll

A ragdoll description belongs to the product, per rig. It is a list of bones
and a list of links. Joint indices are those of `Animation.ReadJoints`.

**Bones.** Each `DynamicsRagdollBone` is a body hanging from a rig joint:

- It spans from that joint to where `EndJoint` is at rest. A bone with no end
  joint (`NoEndJoint`; a head or a hand) spans `Length` metres along the
  joint's +Y.
- `Capsule` takes `Radius`.
- `Box` takes `Radius` as its half width along the joint's X and `HalfDepth`
  along its Z.
- `Mass` is in kilograms; inertia follows from the shape.
- The builders are `DynamicsRagdollBone.Capsule`, `CapsuleAlong` and `Box`.

**Links.** Each `DynamicsRagdollLink` joins a `Parent` bone to a `Child` bone
at the child's joint:

- `Axis` is the hinge or twist axis in the child joint's own frame at rest.
- Limits count from the rest pose, so they mean the same turn whatever pose
  the ragdoll spawns in.
- A cone's axis is usually the bone (+Y in most rigs). An elbow or knee
  hinge turns about whichever joint axis is perpendicular to its bend. Rigs
  differ even between the left and right side, so read the axes off the rest
  pose before writing them down.

**Choosing bones.** Simulate the skeleton's root, the joint every simulated
bone descends from (`Hips`, or `Hip` in Tripo rigs). Unsimulated joints keep
their clip pose relative to their parent, so:

- an unsimulated root stays standing where the clip put it;
- a spine or skirt hanging from it then stretches from there to the fallen
  body.

Joints between simulated bones (twist bones, an unsimulated chest between the
spine and the neck, fingers) follow their parent.

**Collision.**

- Linked bones never collide with each other.
- Other bones of the ragdoll, and other ragdolls, collide by `CollisionGroups`
  and `CollisionMask`, as ordinary bodies do. Give ragdolls a group their mask
  leaves out to keep them from touching each other.
- Shapes that overlap at rest without a link push apart on the first step.
  Choose radii that keep unlinked bones apart.
- Ragdoll bodies collide with the world's bound static environment
  (`BindWorldCollision`).
- The character controller sees them only through the obstacles the product
  passes it.

## Lifecycle

**Creating.** `CreateRagdoll` refuses with `CSHARP_RAGDOLL_POSE` until the
instance has reported its joints (`ReportJoints` on its pose, a call before).

- It spawns the bodies at the joints drawn for the previous call.
- With two reports, it gives them the velocities that pose was moving with.
- It links them, and places the bones at `Blend`.
- A refused ragdoll leaves no bodies or joints behind.

The ragdoll's joint IDs count down from `ulong.MaxValue`, so product joints
should use smaller IDs.

**Every step.** After each `Step`, `StepAndRead` or `StepWithReactions` of its
world, a ragdoll places each bone at its body as a `World` override with the
ragdoll's blend weight. These overrides apply after the product's own pose
overrides.

**Reading and hitting.**

- `ReadRagdoll` returns each bone's joint placement in the world, the blend,
  and `Resting`: true once every body sleeps. Sleeping bodies do not move, so
  a resting ragdoll does not jitter.
- `ApplyRagdollImpulse` applies an impulse at a world point of one bone's
  body, as a hit does.

**Getting up.**

- `SetRagdollBlend` mixes the bodies over the clips from 0 to 1. To get up,
  play the get-up clip and lower the blend over a few updates.
- The bodies keep simulating while the blend changes.
- Move the character's appearance to where the ragdoll lies before it stands:
  the clips draw it at its own placement.

**Disposing.** Disposing the ragdoll removes its bodies and joints and releases
the bones to the clips. Destroying its world ends it too. Destroying its
instance leaves the bodies simulating until the ragdoll is disposed.

**Determinism.** A world steps deterministically: the same world history and
inputs give the same fall. The fixture's falls repeat to five decimals on the
Kenney rig and on a Tripo-rigged resident. A world that has held other bodies
carries their history in Rapier's arena and pair order. The fixture therefore
starts each fall in a fresh world.
