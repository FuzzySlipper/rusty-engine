# C# dismemberment

Package-consumer fixture for
[severed parts](../../docs/csharp-capabilities.md#severed-parts). It reuses the joint
attachment fixture's character (`../csharp-joint-attachments/content`),
running in place on a floor, and cuts its head and its left arm off mid-run.
Each piece falls as its own body (a capsule for the head and one for the arm) and settles on the floor while the
character runs on.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpDismemberment.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpDismemberment.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands:

- `cut.head` and `cut.arm` cut the head (at `Head`) or the left arm (at
  `LeftArm`) off at the next update, with a push and a spin.
- `cut.reset` disposes the pieces: the character is whole again and their
  bodies leave the world.
- `cut.despawn` disposes the character's animation instance and removes it
  from the scene without disposing the pieces; the Engine releases them.
  `cut.spawn` brings the character back whole.
- `cut.inspect` reports the Dynamics world's body count after the last step,
  the character's latest reported pose time and left foot (it keeps
  running), and each piece: where it is, whether it rests and how many steps
  after its cut it came to rest, or `released`.
