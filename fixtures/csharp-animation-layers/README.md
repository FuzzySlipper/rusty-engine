# C# animation layers

Package-consumer fixture for [clip layers](../../docs/skeleton-poses.md#clip-layers)
and [clip packs](../../docs/portable-assets.md#animation-clips-and-clip-packs).
It reuses the joint attachment fixture's character
(`../csharp-joint-attachments/content`). The body plays its `run`. The right
arm waves over it at half weight: the pack's `wave` clip as an override
layer, masked to `RightArm` and `RightForeArm`. The head nods over both: the
pack's `nod` clip as an additive layer. Both clips come from
`body.clips.glb`, a clip pack that carries only the character's skeleton.
`make_clip_pack.py` writes it from `body.glb`. Every update sets the layers at
the product's own times and samples the pose that update draws
(`SampleJointPose` of the current playback).

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpAnimationLayers.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpAnimationLayers.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands:

- `layers.inspect` lists the clips `ReadClips` reports, each with its clip
  pack (0 for the body's own clips), and the sampled hands.
- `layers.wave <0..1>` sets the wave layer's weight.
- `layers.nod <true|false>` turns the additive nod on or off.
