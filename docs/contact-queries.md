# Contact queries and melee

`Spatial.OverlapContacts` lists everything a volume overlaps.
`Spatial.SweepContacts` lists everything a shape meets as it moves and turns
from one pose to another, in order of impact. Both read the session's world
(voxels and static meshes) and the entity colliders and posed parts the caller
passes for the call. `Animation.SampleJointPose` gives the joints an
animation puts those parts on, at simulation time and without a renderer.
The Engine reports contacts. The product decides what they mean: a hit, a
parry, a block, or nothing.

## Shapes, poses and parts

A `SpatialShape` is defined in its own frame:

- `Box(halfExtents, center)`, or `Aabb(min, max)` for a world box placed at
  `SpatialPose.Identity`;
- `Capsule(a, b, radius)`;
- `Sphere(radius, center)`;
- `Segment(a, b)`.

A `SpatialPose` (translation, rotation) places the frame in the world. A
zero rotation is the identity.

A `SpatialEntityPart` is one oriented shape belonging to an entity. It carries
the product's `Part` id (a bone or a hit group) and an optional `Holder`, the
entity holding it. A sword or shield names the character holding it. An
entity can have any number of parts. They take part beside the ordinary
`SpatialEntityCollider` boxes, which answer as part 0.

`Filter` applies each part's own group and mask, as it does for colliders. A
disabled part is left out. An entity in `IgnoredEntities` ignores its parts
and the parts it holds, so an attacker ignores itself and its own weapon in
one entry.

`SpatialEntityPart.OnJoint(entity, part, jointPose, shape, offset)` places a
shape at an offset from a joint, using a joint from `SampleJointPose` or
`ReadJointPose`.

## Readouts

Both queries return a `SpatialContactResult`. Each `SpatialContact` gives:

- what was touched: `Kind` (`Voxel`, `StaticMesh` with its instance, asset
  and geometry hash, or `Entity` with `Entity`, `Part` and `Holder`);
- `Point`, the contact point on the touched surface, and `Normal`, that
  surface's outward normal, both in world space;
- `Fraction`, how far along a sweep the contact begins (0 for an overlap);
- `PenetrationDepth`, for an overlap, or for a sweep that starts inside its
  target (`StartSolid`).

Each target answers at most once. Each entity part or collider gives one
contact, each static-mesh instance gives one, and the voxels together give one
(the deepest overlap, or the earliest impact). Contacts are ordered by
fraction. At the same fraction the world comes first, then entities by id,
then part. `WorldContact` reports whether the world answered.
`MaxContacts` keeps the first so many and sets `Truncated`, and zero keeps
every contact. Surfaces that only touch do not overlap. Trigger colliders are
left out unless `IncludeTriggers` is set. `ExcludeWorld` leaves the world out.

`Spatial.CastRay` and `CastSegment` return only the nearest hit, a
`SpatialHit` with the same `Kind`, `Point` and outward `Normal` for voxels,
static meshes and entity colliders. A ray that starts inside an entity
collider hits it at distance 0 with a zero `Normal`.

A sweep moves the shape's `Pivot` (in the shape's frame) in a straight line
from its start pose to its end pose. Meanwhile the shape turns about the
pivot by the shortest rotation between the two poses. Use the grip as the
pivot for a wrist swing, or a point toward the shoulder for a wider arc. A
target that lies wholly between the end poses is still met, because each
target's first time of impact is found along the path. A long arc can be
chained from a few sweeps whose ends meet.

## Pose sampling

`SampleJointPose` poses an animated mesh through the same evaluator the
drawn pose comes from:

- `clip` at `time`, optionally blended toward `blendClip`;
- then, with `ApplyPoseControls`, the instance's `SetPose` controls and its
  ragdoll layer;
- placed at `Placement`.

It returns each rig joint in `ReadJoints` order, in the instance's space and
in the scene, as `ReadJointPose` reads a drawn pose. The same clip time,
controls and placement give the same joints the drawn pose has. The pose is
not drawn and nothing is retained. It works in the headless `EngineTestHost`,
so a recorded fight replays identically in a test.

`AnimationPoseSampleRequest.OfClip` and `OfBlend` build the common cases.
`Normalized` reads times as fractions of the clip, as `Sample` playback
does. Otherwise times are seconds, clamped to the clip.

### Current playback

`AnimationPoseSampleRequest.OfCurrentPlayback(instance, placement)` samples
what the instance plays: its direct playback (under any cross-fade or fade
out) or the first controller driving it. The sample is taken at the time the
call advances to, which is the time the renderer draws the call at.
Playback runs on the Engine's presentation timeline, which advances only with
admitted simulation steps. A command takes effect from the start of the call
that issues it, so a clip started this call is already a call's time in when
the call is drawn. The instance must be in the published scene.

`SampleModelJointPose(AnimationModelPoseSampleRequest.OfClip(resource, ...))`
samples a mesh resource without an instance or controls, for example to
measure a swing ahead of time.

## A melee swing

```csharp
// Where the swing was drawn last update, and where this update draws it.
JointPose before = previousHand;
JointPose after = engine.Animation.SampleJointPose(
    AnimationPoseSampleRequest.OfCurrentPlayback(attacker, placement)).Span[(int)rightHand];
previousHand = after;
SpatialShape blade = SpatialShape.Capsule(Vector3.Zero, new(0, 0, -0.9f), 0.04f);

// Each defender's body parts and held items, posed the same way.
SpatialEntityPart[] parts = defenders.SelectMany(defender => defender.Parts(engine.Animation)).ToArray();

SpatialContactResult swing = engine.Spatial.SweepContacts(SpatialContactQueryRequest.Sweep(session, blade,
    SpatialPose.Of(before.World).Then(gripOffset), SpatialPose.Of(after.World).Then(gripOffset),
    parts: parts, ignoredEntities: new[] { attackerEntity }));

foreach (SpatialContact contact in swing.Contacts.Span)
{
    if (contact.Kind != SpatialHitKind.Entity) { Clang(contact.Point, contact.Normal); break; }  // a wall first
    if (contact.Holder != 0 && IsWeapon(contact.Entity)) { Parry(contact); break; }
    if (contact.Holder != 0) { Block(contact); break; }
    Hit(contact.Entity, contact.Part, contact.Point);  // product damage rules by part
    break;
}
```

For the reaction, `Dynamics.ApplyRagdollImpulse(ragdoll, bone, point, impulse)`
pushes a ragdoll bone. `Dynamics.ApplyPointImpulse(body, point, impulse)`
pushes any Dynamics body at the contact point, moving it and spinning it as
an off-centre blow would. The next step applies the push.

For a hit-frame volume with no order, such as a cleave, a stomp or an
explosion, `OverlapContacts` with a sphere, box or capsule lists every actor
inside it, and the world beside them.
