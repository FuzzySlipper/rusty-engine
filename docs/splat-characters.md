# Gaussian splat characters

A splat character is an animated GLB whose mesh is a cloud of Gaussians
rather than triangles: each point a small soft ellipsoid with its own
colour, as asset-pipeline bakes characters from paintings and orbit videos.
The Engine admits it like any animated GLB, poses it with the
`Animation` service, skins its Gaussians on the GPU and draws them,
sorted back to front and blended into the world.

## The asset

One GLB with one or more nodes drawing splat primitives. A splat primitive
is a `POINTS` primitive (`mode` 0) with one value per Gaussian in each of:

| Attribute | Type | Meaning |
| --- | --- | --- |
| `POSITION` | float VEC3 | Centre, in the node's space (bind space on a skinned node). |
| `_ROTATION` | float or normalized signed VEC4 | Orientation, a unit quaternion xyzw. |
| `_SCALE` | float VEC3 | Standard deviation along each of the Gaussian's own axes, in metres (linear, not log). |
| `_COLOR` | float or normalized unsigned VEC4 | **sRGB-encoded** display colour in rgb; **linear** opacity in a. |
| `JOINTS_0`, `WEIGHTS_0` | as glTF | The skin, on a skinned node: at most 256 joints a primitive indexes. |

`_COLOR` is not `COLOR_0`. glTF defines `COLOR_0` as linear, and these
colours are display-referred: they are the values the splats were trained
to blend.

The figure stands on y = 0, +Y up, in metres.

A GLB with splat primitives has only splat primitives, and carries no clips
of its own. Admission refuses a missing attribute, attributes of different
lengths and non-finite values. Its bounds are the Gaussians' centres to
three standard deviations, each node's at rest (a skinned node's in bind
space). A skinned one names its rig: its skin's named joints, held in
place, as clip packs and joint reads need.

### PINOC characters

asset-pipeline writes a character baked for Viggle's PINOC clips on one of
two rigs:

- **Mixamo (the default).** The 22 Mixamo body bones under the names
  PINOC's Mixamo clips use (`mixamorig:Hips` and on). `Hips` is the top
  joint and carries the walk's travel; an identity `character` node holds
  it. The figure faces +Z at rest and when played. Its clips are PINOC's
  Mixamo exports, whose skeleton hangs under a Z-up `Armature.001`: the
  binding folds that wrapper into `Hips` (below).
- **viggle441.** The joints of Viggle's canonical 441-joint humanoid that
  its Gaussians use, plus their ancestors up to `root`, under their
  canonical names and parents:
  - each joint's rest translation is the character's own (the `.vsplat`
    `joints` block), and its rest rotation the canonical one;
  - the Gaussians stay in the `.vsplat`'s own space.

  That rest faces +X. PINOC's own clips hold `root` turned half round, so a
  played character faces +Z and walks along it.

Products open it as any animated GLB:

```csharp
RenderResource body = engine.Animation.OpenAnimatedMeshFromContent(new(bodySource));
```

`CreateAnimatedMeshAppearance` and `CreateInstance` place it, and every
instance of one resource reads the same resident Gaussians.

## Posing

A splat character is posed as any animated mesh
([skeleton poses](skeleton-poses.md)): playback, clip layers, IK, joint
overrides and joint reads.

**Clip packs on other bone lengths.** PINOC clips come on the canonical
skeleton: every joint the character has, and many it does not, at the
canonical rest translations. `AssociateAnimationClipPackByJointNames`
binds such a pack:

- the pack names every joint of the character;
- rotation channels apply as authored;
- a translation channel applies only where it moves, so the character
  keeps its own bone lengths;
- a top joint's channels (a joint with no joint above it, such as Mixamo's
  `Hips`) are carried out of the frame of the nodes above it that are not
  joints, and into the character's: a Z-up export's armature turned
  +90° about x binds as its Y-up clip.

`AssociateAnimationClipPack` would refuse the pack, since its rig is not
the character's.

**Pelvis fit.** A clip's pelvis (or `Hips`) translation is authored for
the clip's own legs. `FitAnimationClipTranslation` fits one joint's
translation in one clip as it binds: each key `t` becomes
`scale * t + offset` per axis, after any wrapper is folded in. For
asset-pipeline's fitted `k` and `A` (its `character.json`) that is scale
(k, k, k) and offset (0, A, 0).

```csharp
engine.Animation.AssociateAnimationClipPackByJointNames(new(body, walking, producer, license));
engine.Animation.FitAnimationClipTranslation(new(body, walking, "Armature.001|Scene", "mixamorig:Hips",
    new(0.8836f, 0.8836f, 0.8836f), new(0, 0.0866f, 0)));
```

Associate and fit before creating an appearance for the character.

**Arm offset.** An arm offset added to every clip, such as upper arms 10°
further out and forearms a further 15°, is an additive `Model`
`JointOverride` on each upper arm and forearm: a turn about +z (the played
figure's forward), positive for the left arm, negative for the right.

**Colour multiplier.** `SetSplatAppearance` scales an appearance's splat
colour in linear light, as a stand-in for lighting: (1, 1, 1) draws them as
baked, lower for a figure in shade.

Posed bounds (the inspection bounds fact) hold each joint's Gaussians moved
by that joint.

## How it draws

- **Resident data.** The renderer reads the GLB as admitted and keeps
  32 bytes per Gaussian on the GPU, shared by every instance:
  - the centre;
  - the colour and opacity;
  - the rotation;
  - the standard deviations as halves.
- **Skinning.** A skinned set's Gaussians are skinned on the GPU each frame
  as they are projected, from a palette of its joints:
  - each joint's matrix in the instance's space with its scale removed (a
    joint's scale never inflates the Gaussians), times its inverse bind;
  - the four joints blended linearly by weight;
  - the centre moved by the blend, and the Gaussian turned by the blend
    with its columns' lengths removed.

  Only the palette, three rows a joint, is written per frame.
- **Kernel.** It is the one the splats were trained with, as the web SDK
  (PlayCanvas 2.23) draws them:
  - the 3D covariance is projected through the view's Jacobian;
  - 0.3 px² is added to its diagonal;
  - a quad reaches 2·√(2λ) along each eigen-axis;
  - the falloff over A = |uv|² is (e^−4A − e^−4) / (1 − e^−4);
  - alpha under 1/255 is dropped.

  A different falloff loses the trained paint, so this is not a setting.
- **Order.** A view's splat instances take their places among the blended
  draws (blended parts and sprites) by the distance to their bounds' centre.
  Instances that follow one another there are one **run**. A run's Gaussians
  are projected and radix-sorted back to front together on the GPU every
  frame, so two figures standing close, or touching, interleave Gaussian
  by Gaussian. Between a run and a sprite or blended part the order is per
  object, as it is between those.
- **Colour.** Each run draws into a layer of its own, tested against the
  world's opaque depth and writing none. The layer blends the colours as
  they are, in display space, and is composited into the HDR target at the
  run's place, decoded to linear light.
  - Blended in linear light, as the HDR target blends, a figure comes out
    clearly lighter: townsman 385's mean figure colour rises by about 25
    levels.
  - The finish then treats the figure as any other HDR colour. With tone
    mapping off or `Rolloff` below its knee and exposure 1, a figure shows
    the colours it was trained with.
  - Where a soft edge lets the world through, that light mixes in linear
    light, so a silhouette against a bright background reads a little
    lighter than in the web SDK.
- **Fog and temporal upscaling.** A splat writes no depth, so finish fog
  sees what is behind it. Under temporal upscaling its pixels carry the
  reactive mask, so the upscaler keeps no history there.

Measured against a CPU reference of the web kernel, townsman 385
(48,000 Gaussians, 1920 × 1080, five views), at rest and walking with his
Gaussians skinned on the CPU from the same joints, differs as follows:

- **Figure mean:** within 0.13 levels per channel.
- **Pixels at least half covered:** a median difference of 0.5 levels and
  a 99th percentile of 6.4 to 10.6.
- **Soft edges:** differ by up to about 30 levels (95th percentile), as
  explained under **Colour** above.

Walking with the pelvis fit, the viggle441 townsman's planted ball of each
foot drifts as it does in the web SDK: 0.55 and 1.25 cm (median per stance,
left and right) against 0.6 and 1.2 cm, at 1.49 m/s. On both rigs, at eight phases of
the walk, every Gaussian stands within 0.02 mm of asset-pipeline's offline
pose of the same clip. The Mixamo townsman on PINOC's Mixamo Walking walks
at 1.60 m/s. Its right foot slides about 5.5 cm a stance in both:
that clip slides it on its own figure too, an asset issue rather than the
binding's.

## Devices and cost

- **Requirements.** Splats need compute shaders, storage buffers read in
  vertex shaders and indirect draws. A device without them draws no splats,
  and the readout says why. Software adapters draw them, slowly.
- **Limit.** A run sorts at most 4,194,304 Gaussians. An instance that does
  not fit draws nothing, and the readout counts it.
- **Readout.** `GpuReadout.splats` reports the last frame's runs, instances
  and Gaussians, summed over its views. The `splats` pass timer covers the
  projection, sort and layers of the frame's first view with a run.
- **Measured cost.** One 48,000-Gaussian figure at 1920 × 1080 took
  0.31 ms of GPU time on an RX 9070 XT about 750 pixels tall (2.2 m away),
  and 0.20 ms at 3.6 m, at rest or walking.
