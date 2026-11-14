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
| `_NORMAL` (optional) | float or normalized signed VEC3 | The surface normal at the Gaussian, unit, pointing out of the figure, for [lighting](#lighting). |
| `_ALBEDO` (optional) | float or normalized unsigned VEC4 | The colour without its baked shading, **sRGB-encoded** as `_COLOR` (alpha unread), which a lit figure draws. |

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

## Pixel finish

`SetSplatAppearance` also draws a figure as pixel art, as a parts rig's
[`PixelGrid`](parts-rigs.md#on-a-screen-pixel-grid) does. The figure is
drawn smooth, then cut into square blocks of `PixelGrid` output pixels (at
least one render pixel under a render scale):

- **One grid, anchored at the origin.** The grid has a corner at the
  projection of the instance's origin, so the blocks hold their place on
  the figure as it or the camera moves.
- **Coverage-weighted blocks.** Each block is the average of the figure
  under it, weighted by its coverage, in display space (as its splats
  blend). A block draws where the figure covers at least `PixelCoverage`
  of it.
- **Depth per pixel.** The figure is drawn whole, and each block's pixels
  are tested against the world at the block's depth (the figure's under
  it, weighted as its colour), so a post in front hides blocks pixel by
  pixel.
- **Palette.** With a `Palette` of linear colours, each block takes the
  nearest by sRGB value. The view's finish (exposure, grading, tone
  mapping, fog) then applies to the blocks as to the rest of the scene.
- **Alone in its run.** A gridded instance is sorted on its own, not
  Gaussian by Gaussian with its neighbours: two gridded figures that
  overlap each draw whole blocks, the nearer over the farther.
- **Temporal stages.** Under TAAU or DLSS the blocks mark the reactive
  mask, so the stage keeps no history over them.
- **Pose rate.** `PoseRate` poses the instance's clips that many times a
  second, their time stepping by whole steps of Engine time, so figures
  at one rate step together.

```csharp
// Townsman 385 as the web shows him: 5-pixel blocks in his 52 colours.
engine.Animation.SetSplatAppearance(new(townsman, Vector3.One, 5, 0.5f, his48, 0,
    SplatLighting.Unlit, 0, SplatShadowCaster.None));
```

Against the web SDK's `pixel:5:his48` at its quarter camera (townsman 385,
viggle441, Walking, four phases at 1920 × 1080):

- **Same:** the block size, the palette, and colour per block.
- **Grid.** The web's grid is fixed to the screen and covers the whole
  frame, the world included. The Engine's follows the figure and covers
  only him, so his blocks do not crawl as he walks.
- **Edge.** The web averages each block with the background behind him,
  then snaps it. The Engine averages only his coverage and cuts the block
  at `PixelCoverage`, so his outline is his colours, not a blend with the
  ground.
- **Matching.** The web weights the colour distance by channel (2, 4, 3).
  The Engine matches by plain sRGB distance, as parts rigs do. On his
  blocks the two pick different colours for 5 to 6 % of blocks. Over the
  figure, the shares of each palette colour differ from the web's by an
  L1 of 0.14 to 0.15 (0.13 to 0.14 with the web's weights): alignment and
  edges account for most of it.

## Lighting

`SetSplatAppearance` with `Lighting = Lit` lights a figure with the world's
lights. Unlit (the default) draws it as baked, scaled by the multiplier.

```csharp
engine.Animation.SetSplatAppearance(new(townsman, Vector3.One, 0, 0.5f, default, 0,
    SplatLighting.Lit, LightFloor: 0.15f, ShadowCaster: SplatShadowCaster.Ellipsoids));
```

- **Per pixel, after blending.** The figure's Gaussians blend their depth
  and normal beside their colour. Where they draw, the composite lights the
  blended colour once with the world's own lighting: the sun through its
  cascades (so he receives shadows), clustered point and spot lights,
  ambient light, the sky and the indirect-light probes. Every Gaussian
  along a ray is lit alike.
- **The albedo.** A lit figure draws `_ALBEDO` where the asset has one,
  at `_COLOR`'s opacity. Otherwise the baked colour stands in, and the
  shading already in the paint is multiplied by the light, as the web SDK's
  relighting does. Either way it is diffuse only (roughness 1, no metal).
- **Light floor.** `LightFloor` (0 to 1) keeps that share of the baked
  colour however dark the light. At 0 the scene's ambient light is the
  floor.
- **Normals.** With `_NORMAL` the asset's normals light him, and his
  clothes shade as smooth forms. Without it, each Gaussian's shortest axis
  stands in, pointing away from the centre of the Gaussians its heaviest
  joint carries: right for the flat Gaussians on his surface, a guess for
  round ones, so lighting from the fallback is mottled.
- **Pixel grid.** Each block is averaged (colour, depth and normal), lit
  once, then snapped to the palette: a block takes one shade, and shadows
  fall on the block grid.
- **Runs.** Lit figures draw in runs of their own, apart from unlit ones
  and from lit ones with another floor.
- **Cost.** A lit figure always keeps the depth and normal (0.55 ms rather
  than 0.31 for a close 48k figure on an RX 9070 XT, as a figure something
  reads), and the light is one evaluation per covered pixel. Under motion
  the normal takes a target of its own.

**Casting shadows.** `ShadowCaster` sets how a figure casts into the shadow
maps of lights that request them, while its instance's `shadow_casting`
casts. Lit or unlit, a figure casts only when it is asked to.

| `ShadowCaster` | What casts | Look | Cost (RX 9070 XT, 1080p, walking 48k figures, the sun's four cascades) |
| --- | --- | --- | --- |
| `None` (default) | nothing | | |
| `Ellipsoids` | an ellipsoid around the Gaussians each joint carries, posed with it | a smooth mannequin: no hood, pouch or hem | +0.04 ms at 32 figures |
| `Splats` | its Gaussians, re-projected for each shadow layer and cut where their alpha falls under 0.3 | its own silhouette, hood and stride included | about 0.15 ms a figure: +0.3 ms at 1, +5 ms at 32 |

**Lit crowds.** Walking 48,000-Gaussian townsmen in the web crowd layout
(`examples/splat_crowd --lights`) under a sun casting through four cascades,
ambient light and a point light. Frame throughput, ms, median of three
rounds:

| Figures | GPU, size | Unlit | Lit | Lit, `Ellipsoids` | Lit, `Splats` |
| --- | --- | --- | --- | --- | --- |
| 1 | RTX 3080, 1080p | 0.52 | 0.75 | 0.83 | 1.00 |
| 32 | RTX 3080, 1080p | 2.43 | 3.96 | 4.12 | 9.37 |
| 32 | RTX 3080, 1440p | 3.12 | 5.25 | 5.41 | 10.73 |
| 32 | RX 9070 XT, 1080p | 2.25 | 2.79 | 2.93 | 7.86 |
| 32 | RX 9070 XT, 1440p | 2.55 | 3.49 | 3.64 | 8.54 |

Most of lighting's cost is the layer keeping depth and normal: the light
itself is a few hundredths of a millisecond. The web SDK's relighting cost
20 to 25 % of its frames at 32 figures, for its proxy meshes.

Use `Splats` for a few close figures and `Ellipsoids` for crowds. A
software adapter casts `Splats` as ellipsoids, and `GpuReadout.splats`
says so (`refused_casters`). Splat figures receive shadows only when lit.

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
  object, as it is between those: a sprite between two figures splits them
  into two runs, and the order turns as the camera moves round.
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
- **Depth and motion.** Beside its colour, a layer keeps the depth of
  what it drew and, when the frame draws motion, its motion, blended as the
  colour is: each Gaussian's depth and its centre's motion since the
  previous frame (from the instance's previous place and its set's
  previous palette), weighted by its share of the pixel. Where the layer
  covers at least half a pixel, the composite writes that depth into the
  view's depth and that motion into the motion target. So:
  - the finish fogs a figure at its own distance;
  - blended parts, sprites, particles and soft particles drawn after it
    meet it;
  - under temporal upscaling a figure's history follows it as it walks or
    its pose moves.

  Where the layer covers less, at its soft edge, the composite marks the
  reactive mask instead, so the upscaler keeps no history there, and the
  pixel keeps the depth behind it. The depth is held as a half float, to
  about one part in 2,000 of its distance.

  A run keeps them only when something after it reads them: fog
  (analytic or volumetric), motion (temporal upscaling, Ray
  Reconstruction), an image effect, particles, soft sprites, overlay
  lines, rain or snow, or any blended draw after the run. Otherwise the
  view's depth stays as the opaque world left it.

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
- **Measured cost.** One 48,000-Gaussian figure walking at
  1920 × 1080 takes 0.31 ms of GPU time on an RX 9070 XT about 750 pixels
  tall (2.2 m away), and 0.19 ms at 3.6 m. Drawing the layer dominates, and
  it grows with the figure's pixels. When something reads the figure's
  depth or motion, every fragment writes the depth target too: 0.55 ms at
  2.2 m and 0.30 ms at 3.6 m. A pixel grid always keeps the depth, and its
  blocks add about 0.05 ms to the world pass at 2.2 m.
- **Crowds.** Walking 48,000-Gaussian townsmen in the web crowd page's
  layout (a 1.4 m grid seen from in front, each on its own phase), as
  `examples/splat_crowd` draws them offscreen. The figures are a frame's
  throughput, CPU and GPU overlapping, without presenting; median of three
  rounds:

  | Figures | Gaussians | RTX 3080, 1080p | RTX 3080, 1440p | RX 9070 XT, 1080p | RX 9070 XT, 1440p |
  | --- | --- | --- | --- | --- | --- |
  | 1 | 48,000 | 0.49 ms | 0.67 ms | 0.37 ms | 0.59 ms |
  | 8 | 384,000 | 1.14 ms | 1.63 ms | 0.89 ms | 1.09 ms |
  | 16 | 768,000 | 1.59 ms | 2.17 ms | 1.39 ms | 1.52 ms |
  | 32 | 1,536,000 | 2.34 ms | 2.98 ms | 2.28 ms | 2.48 ms |

  The `splats` pass is most of it: on the 3080 at 1080p, 2.15 ms of the
  32-figure frame. Cost grows with the Gaussians projected and sorted and
  with the pixels the figures cover, not with the count of instances: one
  run sorts them all.

  On the same 3080 the web SDK draws 32 such figures at 26 to 41 fps
  (1582 × 948, in a browser). Its CPU sorter lags and throttles under load;
  the Engine sorts every Gaussian on the GPU each frame. A crowd of 32 close
  figures takes about a seventh of a 60 Hz frame on these GPUs.
