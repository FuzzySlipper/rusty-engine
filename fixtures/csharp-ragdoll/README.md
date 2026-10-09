# C# ragdoll

Package-consumer fixture for [limited joints and ragdolls](../../docs/ragdolls.md).
It reuses the joint attachment fixture's character
(`../csharp-joint-attachments/content`), held at a sampled idle pose on a
floor. Its ragdoll has eleven bones: pelvis, spine and head, upper arms and
forearms, thighs and shins. Cones join the spine, the head, the shoulders and
the hips, and hinges are the elbows and knees.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpRagdoll.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpRagdoll.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands:

- `ragdoll.fall` starts a fresh Dynamics world, spawns the ragdoll from the
  reported idle pose, and hits it in the chest.
- `ragdoll.getup` plays idle again and blends the ragdoll out over a second.
- `ragdoll.inspect` reports the fall's steps and when the ragdoll rested. It
  also reports the lowest point its shapes reached (all time and now) and the
  least hinge flexion; a negative flexion would be an elbow or knee bending
  backwards. Its last field is a pose signature, recorded 480 steps after
  each hit.
