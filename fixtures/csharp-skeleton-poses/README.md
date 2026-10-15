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

It holds a model on the viewmodel layer, placed in the camera's space, and
its page has two panes inspection views anchor to (`inspect-a` and
`inspect-b`): `engine.inspect.view create side anchor inspect-a`, then
`engine.inspect.view frame side side camera`.

It publishes the overlay set `fixture.guide`, one item in each overlay space:
a world segment along x, a box around the body in its model space, axes, a
marker and a label on the right hand joint, and a marker at the centre of the
camera's view. `engine.inspect.overlay show` draws it;
`engine.inspect.overlay show joints <handle>` and `show ik <handle>` draw the
Engine's joint chain and the arm's IK reach, with the body's handle from
`engine.scene.objects`.
- `pose.curl <0..1>` sets the finger curl weight.
- `pose.reach <true|false>` turns the IK on or off.
- `pose.reset` clears the worst-case readings.
