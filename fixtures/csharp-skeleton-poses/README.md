# C# skeleton poses

Package-consumer fixture for [skeleton poses](../../docs/skeleton-poses.md).
It reuses the joint attachment fixture's character
(`../csharp-joint-attachments/content`). The right arm reaches a target that
circles in front of the character with a fixed pole (two-bone IK), one weight
curls the right index finger over the idle clip, and every update compares
the reported hand with the target drawn for the previous call.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpSkeletonPoses.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpSkeletonPoses.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands:

- `pose.inspect` reports joint reports, the hand's error against the drawn
  target, the worst pole-side dot, elbow flips, the finger's turn from the
  hand, and the reported IK chain: its shortfall, whether it straightened, and
  the worst difference between that shortfall and the hand-to-target distance
  computed from the reported joints.
- `pose.far <true|false>` moves the circle beyond the arm's reach.

It holds a model on the viewmodel layer, placed in the camera's space. Its
page mounts two inspection view panes: `side`, framing the space before the
camera, and `hand`, framing the right hand from below. It registers inspection
targets (`engine.inspect.target list`): `right-hand`, `held-tip` and
`view-centre`, which `engine.inspect.view frame side side target held-tip`
frames.

It publishes the overlay set `fixture.guide`, one item in each overlay space:
a world segment along x, a box around the body in its model space, axes, a
marker and a label on the right hand joint, and a marker at the centre of the
camera's view. `engine.inspect.overlay show` draws it;
`engine.inspect.overlay show joints <handle>` and `show ik <handle>` draw the
Engine's joint chain and the arm's IK reach, with the body's handle from
`engine.scene.objects`.
It publishes the landmark pairs `fixture.grip` from a `HandRig` on the right
hand (`engine.inspect.landmarks fixture.grip`): `reach`, the wrist within
reach of the circle's centre (outside it after `pose.far true`), `trigger`,
the index pad on the held model's tip, which it never reaches, and
`pointing`, the index finger's last bone aimed at the circle's centre. The
`hand` pane's Overlays button shows them there. `pose.landmarks` reads the
same set from C# (`Graphics.ReadLandmarks`) with the command's field names.

- `pose.curl <0..1>` sets the finger curl weight.
- `pose.reach <true|false>` turns the IK on or off.
- `pose.reset` clears the worst-case readings.
