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
    new[] { JointOverride.AddLocalRotation(finger, curl, gripWeight) },
    ReportJoints: true));

// The next update: the pose drawn for this one.
AnimationJointPoseResult drawn = engine.Animation.ReadJointPose(instance);
Vector3 handAt = drawn.Joints.Span[(int)hand].World.Translation;
```

`fixtures/csharp-skeleton-poses` reaches a circling target with a fixed pole,
curls a finger over the idle clip, and checks the reported hand against the
target it was given (`pose.inspect`, `pose.curl <0..1>`, `pose.reach <bool>`).

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
   the rest pose.
2. **Two-bone IK**, in list order.
3. **Joint overrides.** Parents are applied before children; one joint's
   overrides apply in list order.
4. **Skinning.** Joint attachments then hang from the result.

## Two-bone IK

`TwoBoneIk(root, mid, end, space, target, pole, weight)` rotates `root` and
`mid` so `end` reaches `target`. The chain bends in the plane through `root`,
`target` and `pole`, with `mid` on the pole's side, so a moving target never
flips the elbow. `mid` must descend from `root` and `end` from `mid`; twist
joints between them follow. A target beyond reach straightens the chain toward
it. `space` is `Model` (the instance's own space) or `World`. `weight` mixes
both turns in from 0 to 1.

## Overrides

A `JointOverride` sets a joint's rotation, its translation or both, mixed
over the pose below it by `weight` (0 to 1):

- `Local`: the joint's value relative to its parent. With `Additive`, the
  rotation is applied after the evaluated one (a curl over the clip), and the
  translation is added.
- `Model` or `World`: the joint is placed there and its descendants follow.
  It keeps its evaluated scale. `Additive` is refused here.

`JointOverride.LocalRotation`, `AddLocalRotation`, `Place` and `Orient`
build the common cases.

## Writing and reading

`SetPose` replaces the instance's whole set of controls. Write it every update
while it changes, as with any other appearance fact; an unchanged set is not
sent again. Empty lists clear it, and disposing the instance clears what it
left on its object. A joint index outside the rig, an IK chain that does not
descend, or a value out of range is refused with `CSHARP_ANIMATION_POSE`, and
the previous controls stay.

With `ReportJoints`, the renderer evaluates the instance when each call is
applied and reports its joints before the next call, whether or not a frame
is drawn. `ReadJointPose` returns the latest report:

- the pose drawn for the previous call, which is the clip time and the
  controls of that call;
- each joint's transform in the instance's space (`Model`) and in the scene
  (`World`);
- the instance's world placement, and the Engine time it was evaluated at.

`Reported` stays false until the first report. Reports need the renderer, so
the headless test host (`EngineTestHost`) has none.

A reporting instance is evaluated, and its CPU skinning runs, on every call
instead of only on frames that show a change. Turn reporting off when nothing
reads the pose.
