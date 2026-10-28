# Skeleton poses

`Animation.SetPose` lets a product drive the skeleton of an animated mesh:
arms that reach for things, heads that look at them, fingers that curl, and
skeletons that follow simulated bodies. `Animation.ReadJointPose` reads back
the pose that was drawn, so the product can work from the actual pose.

```csharp
ReadOnlySpan<AnimationJointInfo> rig = engine.Animation.ReadJoints(bodyResource).Span;
uint arm = AnimationJoints.IndexOf(rig, "RightArm");
uint forearm = AnimationJoints.IndexOf(rig, "RightForeArm");
uint hand = AnimationJoints.IndexOf(rig, "RightHand");
uint finger = AnimationJoints.IndexOf(rig, "RightHandIndex1");

// Every update, with the clips still playing underneath:
engine.Animation.SetPose(new(instance,
    new TwoBoneIk[] { new(arm, forearm, hand, PoseSpace.World, doorHandle, elbowPole, Weight: 1) },
    new[] { JointOverride.AddLocalRotation(finger, curl, gripWeight) }));

// The next update: the pose drawn for this one.
AnimationJointPoseResult drawn = engine.Animation.ReadJointPose(instance);
Vector3 handAt = drawn.Joints.Span[(int)hand].World.Translation;
```

`fixtures/csharp-skeleton-poses` reaches a circling target with a fixed pole,
curls a finger over the idle clip, and checks the drawn hand against the
target it was given (`pose.inspect`, `pose.curl <0..1>`, `pose.reach <bool>`,
`pose.far <bool>` for a target beyond reach).

## Fitting to a held model

To fit fingers to a held prop, read the drawn joints with `ReadJointPose` and
place probe points along each finger segment. Then ask
`Animation.QueryModelSurface` how far each probe is from the prop's own
triangles, and whether it passed inside, with the prop at the transform it is
drawn with
([model surface queries](csharp-product-project.md#live-content-from-product-owned-sources)).
Curl a joint until its probes come within reach. Both answers are in the
caller's space, so a viewmodel is fitted in camera space without entering
collision.

## Joints

`ReadJoints(resource)` lists the admitted rig's skin joints, sorted by joint
name. A joint's position in that list is the index pose controls and joint
reads use; `Parent` is its parent's index, or `uint.MaxValue` for a root. The
names are the exact authored glTF names, as for
[joint attachments](portable-assets.md#mesh-to-joint-attachments). A GLB
without a skin has no joints, and pose controls on it are refused.

## Order

A pose is evaluated in this order:

1. **Clips.** Playback or a controller samples the clips and blends them over
   the rest pose; then the clip layers apply in order.
2. **Two-bone IK**, in list order.
3. **Joint overrides.** Parents are applied before children; one joint's
   overrides apply in list order.
4. **Ragdoll layers.** A [ragdoll](ragdolls.md) places its bones as one
   layer over the result, mixed in by its blend weight.
5. **Override scales.** Children are scaled before parents.
6. **Skinning.** Joint attachments then hang from the result.

## Clip layers

`Animation.SetLayers` lays clips over what an instance plays (its playback
or its controller) at times the product gives. Use it to mirror another
simulation's animation layers, or to drive layered animation without a
graph. Each layer is one clip at a time, with a weight from 0 to 1:

- **Override** moves the joints the clip animates toward its pose.
- **Additive** adds the clip's change from its pose at a reference time:
  translation is added, rotation is applied after the joint's, and scale is
  multiplied. This is a delta clip.

A layer changes only the joints and properties its clip has channels for;
the others keep the pose below. Optional joint weights limit a layer to the
joints listed, each with its own weight. Layers apply in order, so a later
override replaces what an earlier additive layer added. Clips are indexed by
their position in `ReadClips`, which includes associated
[clip packs](portable-assets.md#animation-clips-and-clip-packs).

To hold a streamed base clip and its overlays at given times, sample the base
with `Sample` playback (or lay it as the first override at weight 1). Then set
the overlays every update. The list replaces the previous one, and an empty
list clears it:

```csharp
uint parry = AnimationClips.IndexOf(clips, "layer_parry_A");
uint breathe = AnimationClips.IndexOf(clips, "breathe_delta");
engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Sample, mainSequence,
    AnimationLoopMode.Once, 1, 1, false, 0, false, mainCycle));
layers.Clear();
layers.Override(parry, parryCycle, parryWeight, armWeights);
layers.Additive(breathe, breatheCycle, 1);
engine.Animation.SetLayers(layers.ToRequest(instance));
```

Times are fractions of the clip by default, or seconds with `normalized:
false`. Layers are evaluated before IK and joint overrides, by the same code
for the drawn pose and in `SampleJointPose` of the current playback. A sample
of an explicit clip time does not include them. `fixtures/csharp-animation-layers`
lays a waving arm at half weight and an additive nod over a run.

## Two-bone IK

`TwoBoneIk(root, mid, end, space, target, pole, weight)` rotates `root` and
`mid` so `end` reaches `target`. The chain bends in the plane through `root`,
`target` and `pole`, with `mid` on the pole's side, so a moving target never
flips the elbow. `mid` must descend from `root` and `end` from `mid`; twist
joints between them follow. A target beyond reach straightens the chain toward
it. `space` is `Model` (the instance's own space) or `World`. `weight` mixes
both turns in from 0 to 1.

## Overrides

A `JointOverride` sets a joint's rotation, translation and/or scale, mixed
over the pose below it by `weight` (0 to 1):

- `Local`: the joint's value relative to its parent. With `Additive`, the
  rotation is applied after the evaluated one (a curl over the clip), and the
  translation is added.
- `Model` or `World`: the joint is placed there and its descendants follow.
  It keeps its evaluated scale. `Additive` is refused here.

One joint's overrides apply in list order, whatever their space: a `Model`
placement followed by an additive `Local` turn keeps the turn, and the
reverse order keeps the placement.

Each override's weight is its own. A placing (`Model` or `World`) value
moves its joint from where the pose below puts it toward the placement, and
its descendants follow. A zero weight changes nothing, and a full-weight
placement reaches its target whatever its ancestors' weights.

A scale (`OverrideScale`, uniform, 0 or more) is the joint's local scale in
every space, with `Additive` multiplying the evaluated one. It applies last,
after ragdoll layers, about the joint where it is drawn, and its descendants
follow. Scale 0 collapses the joint's subtree into the joint: a first-person
body hides its head and arms by scaling the head and each shoulder to 0, and
the rest of the body keeps its clips. Read back, the joint has scale 0 and the
identity rotation; anything attached under it collapses with it.

`JointOverride.LocalRotation`, `AddLocalRotation`, `Place`, `Orient` and
`LocalScale` build the common cases.

## Writing and reading

`SetPose` replaces the instance's whole set of controls. Write it every update
while it changes, as with any other appearance fact; an unchanged set is not
sent again. Empty lists clear it, and disposing the instance clears what it
left on its object. A joint index outside the rig, an IK chain that does not
descend, or a value out of range is refused with `CSHARP_ANIMATION_POSE`, and
the previous controls stay.

The Engine evaluates each animated instance's pose once, as each call
settles, at the time the call reached (`animation-pose`'s `AnimatedScene`,
kept with the services' appearance state). Every realizer draws that pose,
the renderer included, and nothing evaluates it again. `ReadJointPose`
returns it, whether or not a frame is drawn:

- the pose drawn for the previous call, which is the clip time and the
  controls of that call;
- each joint's transform in the instance's space (`Model`) and in the scene
  (`World`), placed again whenever the instance or a parent moves;
- the instance's world placement, and the Engine time it was evaluated at;
- `IkChains`, one per `TwoBoneIk` in the order given, from the solve that ran:
  the target and the end joint as drawn (after overrides and layers), each in
  the instance's space and in the scene; `Shortfall`, the scene distance from
  the end joint to the target; and `Straightened`, true when the target lay
  beyond the chain's reach and the solve straightened the chain toward it.
  A product that labels a pose out of reach reads these instead of comparing
  joints with the targets it sent.

`Reported` stays false until a call that published the instance has
settled. No renderer is needed, so the headless test host (`EngineTestHost`)
reads poses, spawns ragdolls and cuts severed parts as a running product
does.

## Sampling at simulation time

`SampleJointPose` poses an instance now, without waiting a call: either a
given clip time, or what the instance plays at the time the call is drawn
at. It uses the evaluator and the playback state the drawn pose comes from,
so it gives the joints `ReadJointPose` will read for the same controls and
placement. Hit shapes and held items can then follow the bones during the update
that decides them, in the test host as well. See
[contact queries](contact-queries.md#pose-sampling).

A pose is evaluated only when it can change: while a clip plays or fades,
while a controller drives it, after a command, or when the instance moves and
its controls read its placement. An unchanged pose is not sent to realizers
again, and the renderer's CPU skinning runs only on frames that show a new
pose.
