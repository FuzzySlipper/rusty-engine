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
  target, the worst pole-side dot, elbow flips and the finger's turn from the
  hand.
- `pose.curl <0..1>` sets the finger curl weight.
- `pose.reach <true|false>` turns the IK on or off.
- `pose.reset` clears the worst-case readings.
