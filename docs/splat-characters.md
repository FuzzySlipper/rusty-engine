# Gaussian splat characters

A splat character is an animated GLB whose mesh is a cloud of Gaussians
rather than triangles: each point a small soft ellipsoid with its own
colour, as asset-pipeline bakes characters from paintings and orbit videos.
The Engine admits it like any animated GLB, poses it with the
`Animation` service, skins its Gaussians on the GPU and draws them,
sorted back to front and blended into the world. A character may instead
be [cards](#cards): a few thousand small textured rectangles in place of
tens of thousands of Gaussians, for crowds.

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
| `_GROUP` (optional) | unsigned byte SCALAR | The Gaussian's group, which a product may [tint](#group-tints): 0 for none, 1 to 255 a group. Without it no Gaussian is in a group. |

`_COLOR` is not `COLOR_0`. glTF defines `COLOR_0` as linear, and these
colours are display-referred: they are the values the splats were trained
to blend.

The figure stands on y = 0, +Y up, in metres.

A GLB with splat primitives has only splat primitives, and carries no clips
of its own. Admission refuses a missing attribute, attributes of different
lengths, non-finite values and a `_GROUP` that is not one unsigned byte
per Gaussian. Its bounds are the Gaussians' centres to
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

## Cards

A card character draws each point as a card: a small rectangle in 3D,
turned and sized by its point, carrying one tile of the character's atlas.
asset-pipeline distils them from the same bakes as its Gaussian bodies
(villager 381 is 6,144 cards). A card primitive is a `POINTS` primitive
marked `extras.splatKind = "billboard"`, with one value per card in each
of:

| Attribute | Type | Meaning |
| --- | --- | --- |
| `POSITION` | float VEC3 | Centre, in the node's space (bind space on a skinned node). |
| `_ROTATION` | float or normalized signed VEC4 | Orientation, a unit quaternion xyzw: the card's local x and y lie in it, z is its normal. |
| `_SCALE` | float VEC2 | Half-extents along its local x and y, in metres. |
| `_TILE_ORIGIN` | unsigned short VEC2 | Its tile, in tiles from the atlas's top left. |
| `JOINTS_0`, `WEIGHTS_0` | as glTF | The skin, on a skinned node. |
| `_GROUP` (optional) | unsigned byte SCALAR | Its group, which a product may [tint](#group-tints). |
| `_NORMAL` (optional) | float or normalized signed VEC3 | The surface normal at the card, unit, pointing out of the figure (not the card's own plane), for [lighting](#lighting). |

`extras.bbsplat.tilePx` is a tile's side in atlas pixels and
`extras.bbsplat.atlasTexture` the index of the atlas's texture, whose
image is embedded in the GLB (PNG). Its `atlasW` and `atlasH` are not read:
the image's own size is. The atlas holds display-referred colour and
straight alpha, as `_COLOR` does. Cards have no `_COLOR` and no material.
`extras.bbsplat.albedoTexture` (optional) names a second atlas on the same
tiles: the colour without its baked shading, sRGB-encoded as `_ALBEDO`,
which a lit figure draws.

- **The card.** Corner c in {−1, +1}² stands at
  `centre + c.x·u + c.y·v`, u and v the card's half-axes. It reads its tile
  at `(tile·tilePx + 0.5 + (c + 1) / 2 · (tilePx − 1)) / atlas size`,
  perspective-correct across it, sampled linearly, so it never reaches a
  neighbouring tile. A fragment's alpha is the texel's, at most 0.99, and
  under 1/255 it draws nothing. Cards are drawn from both sides.
- **Skinned**, the blend of a card's joints moves its centre and carries
  its half-axes, as it carries the figure around it.
- **Refused:** a missing attribute, attributes of different lengths,
  non-finite values, a `_TILE_ORIGIN` that is not two unsigned shorts, no
  `tilePx`, an `atlasTexture` that names no texture. A GLB is Gaussians or
  cards, not both, and its card primitives share one atlas and one albedo
  atlas (or none).
- **Bounds.** Each card's centre to its half-diagonal.

Cards draw as Gaussians do: posed by the `Animation` service, projected
and radix-sorted on the GPU, drawn into the same display-space layer and
composited, keeping depth and motion where something reads them, with
`SetSplatAppearance`'s multiplier and pixel grid and `SetSplatGroupTint`.
A card's depth is its plane's at each pixel, not its centre's.

- **Runs.** A run is one card character's instances, sorted card by card
  together. Different card characters, and cards and Gaussians, draw in
  runs apart, ordered by distance as runs are.
- **Lit** as Gaussians are ([lighting](#lighting)): each card blends its
  normal beside its depth, and the composite lights the figure per pixel.
  A lit card draws its albedo atlas where the asset has one, at the colour
  atlas's alpha; otherwise its colour atlas, its baked shading multiplied
  by the light. Without `_NORMAL`, a card's normal points from the centre
  of the cards its heaviest joint carries to its own: a smooth outward
  guess, as a trained card's own plane is not.

  381 lit as cards (with `_NORMAL` and an albedo atlas) and as his
  15,908-Gaussian body, walking, under a sun, ambient light and a point
  light (`splat_capture --lit 0.15 --lit-scene`): the figure's mean colour
  differs by at most 1.5 levels from the front, sides and three-quarter,
  and the cards' back reads about 6 levels darker. In `splat_crowd
  --lights`, ms of the `splats` pass, RTX 3080, 1080p, median of three
  rounds:

  | 32 figures | Unlit cards | Lit cards | Lit 16k Gaussians | Lit 32k Gaussians |
  | --- | --- | --- | --- | --- |
  | close | 1.04 | 1.71 | 1.97 | 3.05 |
  | 42 m | 0.46 | 0.58 | 1.18 | 2.00 |
- **Shadows.** A card figure casts as ellipsoids for `Splats` and
  `Ellipsoids`, and as its [core](#the-core) for `Core`.
- **Readout.** `GpuReadout.splats` counts cards apart from Gaussians.

**Cost.** Villager 381 walking in `examples/splat_crowd`'s layout, as
6,144 cards and as his 15,908- and 31,798-Gaussian bodies, on an RTX 3080 at 1920 ×
1080: the `splats` pass, and the run's projection, sort and draw timed
apart, ms, median of three rounds. Close is the crowd's default camera
(3.3 to 10.7 m away); far is 42 m, the figures about 60 pixels tall.

| Figures | Body | Close: splats | project | sort | draw | Far: splats | project | sort | draw |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | cards | 0.24 | 0.006 | 0.046 | 0.170 | 0.08 | 0.006 | 0.046 | 0.014 |
| 1 | 16k Gaussians | 0.20 | 0.007 | 0.046 | 0.128 | 0.10 | 0.007 | 0.046 | 0.029 |
| 1 | 32k Gaussians | 0.24 | 0.010 | 0.047 | 0.171 | 0.12 | 0.009 | 0.047 | 0.052 |
| 8 | cards | 0.65 | 0.036 | 0.051 | 0.545 | 0.18 | 0.036 | 0.051 | 0.076 |
| 8 | 16k Gaussians | 0.57 | 0.044 | 0.063 | 0.443 | 0.29 | 0.044 | 0.063 | 0.169 |
| 8 | 32k Gaussians | 0.80 | 0.058 | 0.090 | 0.626 | 0.49 | 0.058 | 0.090 | 0.328 |
| 16 | cards | 0.81 | 0.073 | 0.058 | 0.657 | 0.23 | 0.083 | 0.066 | 0.099 |
| 16 | 16k Gaussians | 0.78 | 0.088 | 0.090 | 0.589 | 0.40 | 0.088 | 0.091 | 0.206 |
| 16 | 32k Gaussians | 1.15 | 0.119 | 0.149 | 0.869 | 0.68 | 0.118 | 0.147 | 0.404 |
| 32 | cards | 0.98 | 0.147 | 0.080 | 0.739 | 0.40 | 0.144 | 0.078 | 0.164 |
| 32 | 16k Gaussians | 1.12 | 0.174 | 0.147 | 0.788 | 0.73 | 0.174 | 0.147 | 0.393 |
| 32 | 32k Gaussians | 1.74 | 0.233 | 0.248 | 1.243 | 1.27 | 0.233 | 0.248 | 0.770 |

Drawing dominates up close, and there a card costs about what the
16k body's Gaussians do: most of a tile is transparent, and each of its
fragments still samples the atlas. Against the 32k body, 32 card figures
take 0.98 ms against 1.74 close and 0.40 against 1.27 at crowd distance.
At crowd distance cards draw in about half the 16k body's time and a
fifth of the 32k's, and sort in about half and a third, as there are
fewer of them. A card costs about twice a Gaussian to project (four
corners, and twice the record).

**Against a skinned mesh.** The same crowd with villager 381 as a
triangle mesh, his scaffold, the mesh his splats were fitted to. Two versions:
- **10k triangles:** the scaffold decimated to a usual game character's budget,
  textured with his colour bake. It is on the splat body's own 22 Mixamo joints
  (nodes, skin and inverse binds as `381.glb`), walking the same clip as the
  splat bodies, with no clips of its own.
- **45.7k triangles:** the whole scaffold, untextured, on its own 36-joint rig
  and walk clip.

GPU is the `world` pass, the splats' passes inside it, plus the `skinning`
pass for a mesh, which poses every figure every frame.
CPU is a frame's, the mesh's skinned on the GPU / on the CPU (as before
#11006, `splat_crowd --cpu-skinning`). The default scene, no shadows; ms,
median of three rounds, on an RTX 3080 at 1920 × 1080:

| Figures | Body | Close: GPU | CPU | Far: GPU | CPU |
| --- | --- | --- | --- | --- | --- |
| 1 | cards (6,144) | 0.43 | 0.30 | 0.20 | 0.25 |
| 1 | 16k Gaussians | 0.39 | 0.29 | 0.22 | 0.25 |
| 1 | 32k Gaussians | 0.44 | 0.33 | 0.25 | 0.25 |
| 1 | mesh, 10k triangles | 0.10 | 0.20 / 0.6 | 0.04 | 0.20 / 0.6 |
| 1 | mesh, 45.7k triangles | 0.12 | 0.20 / 1.8 | 0.06 | 0.21 / 2.0 |
| 8 | cards (6,144) | 0.79 | 0.38 | 0.32 | 0.32 |
| 8 | 16k Gaussians | 0.71 | 0.37 | 0.43 | 0.35 |
| 8 | 32k Gaussians | 1.00 | 0.42 | 0.60 | 0.39 |
| 8 | mesh, 10k triangles | 0.16 | 0.25 / 3.4 | 0.09 | 0.26 / 3.4 |
| 8 | mesh, 45.7k triangles | 0.28 | 0.27 / 15.7 | 0.21 | 0.26 / 18.4 |
| 16 | cards (6,144) | 0.95 | 0.45 | 0.37 | 0.40 |
| 16 | 16k Gaussians | 0.93 | 0.45 | 0.51 | 0.42 |
| 16 | 32k Gaussians | 1.30 | 0.46 | 0.80 | 0.45 |
| 16 | mesh, 10k triangles | 0.20 | 0.39 / 8.9 | 0.12 | 0.41 / 9.5 |
| 16 | mesh, 45.7k triangles | 0.40 | 0.34 / 37.4 | 0.32 | 0.41 / 37.6 |
| 32 | cards (6,144) | 1.14 | 0.59 | 0.53 | 0.55 |
| 32 | 16k Gaussians | 1.28 | 0.59 | 0.85 | 0.58 |
| 32 | 32k Gaussians | 1.91 | 0.60 | 1.41 | 0.60 |
| 32 | mesh, 10k triangles | 0.28 | 0.44 / 24.7 | 0.20 | 0.43 / 24.8 |
| 32 | mesh, 45.7k triangles | 0.66 | 0.49 / 91.1 | 0.58 | 0.49 / 81.3 |

Skinned on the GPU, 32 figures of the 10k mesh take 0.28 ms of GPU close
and 0.20 at 42 m, a quarter of the cards' close and two fifths far. The
45.7k mesh takes 0.66 and 0.58, about half the cards' close and about the
cards' far, where the splat bodies draw few pixels. Either mesh takes about
0.1 ms less CPU a frame than a splat body. Skinned on the CPU, as every
skinned mesh was before #11006, 32 of the 10k mesh take 25 ms a frame and of
the 45.7k mesh 81 to 91 ms. With the scene's lights (a sun casting through
its cascades), 32 figures close cast for 0.22 ms (10k mesh) and 0.83 ms
(45.7k mesh) in the `shadows` pass, against 0.10 to 0.12 ms for the splat
bodies' ellipsoids.

Against a CPU reference of asset-pipeline's Workbench card renderer at the
same camera and pose (381, 1920 × 1080, front, at rest and at two moments
of his Mixamo walk), the figure's mean colour differs by 0.14 to 0.26
levels a channel, and pixels at least half covered by a median of 0 and a
99th percentile of 1.

## The core

A splat figure may carry a matte inset core: a closed mesh of a few hundred
triangles just inside its splats, drawn opaque in the figure's colours.
Front splats often stop short of full opacity and lean on those behind
them; where the figure is thin or hollow (a hood around a face), the world
shows through. The core fills those gaps with the figure's own colour, and
since it writes depth, the GPU skips the figure's splats behind it before
shading them. asset-pipeline extracts it from the character's scaffold.

A core is a `TRIANGLES` primitive marked `extras.splatCore = true` in the
mesh of a figure's splats (Gaussians or cards), on the same skin:

| Attribute | Type | Meaning |
| --- | --- | --- |
| `POSITION` | float VEC3 | In the node's space (bind space on a skinned node). |
| `NORMAL` | float VEC3 | Unit, pointing out. |
| `COLOR_0` | as glTF | The figure's colour there, **linear**, as an unlit figure draws it. |
| `_ALBEDO` (optional) | float or normalized unsigned VEC4 | **sRGB-encoded**, as the Gaussians' `_ALBEDO`: what a lit figure's core draws. |
| `JOINTS_0`, `WEIGHTS_0` | as glTF | The skin, on a skinned node. |

Indices as glTF. A splat mesh has at most one core. Admission refuses a
core without `NORMAL` or `COLOR_0`, attributes of different lengths,
non-finite positions or normals, a joint outside the skin and an index
past the vertices. The core stands inside the splats: it does not widen
the figure's bounds.

- **Drawn** in the world's opaque pass, posed with the figure, writing
  depth and, under a temporal stage, motion.
- **Unlit**, it draws `COLOR_0` times the appearance's multiplier. **Lit**,
  it draws `_ALBEDO` (or `COLOR_0` without one) under the world's lights,
  rough and not metallic.
- **Pixel grid.** A figure on a pixel grid draws no core: its blocks test
  each pixel against the world's depth, and its core would cut them.
- **Shadows.** A core casts only for `SplatShadowCaster.Core`
  ([casting](#lighting)): the figure's own shape, at a mesh's cost.
- A figure without a core draws as it did.

**Seal.** The share of a figure's interior (8 pixels in from its outline,
above the hips, five views at 1920 × 1080) that a wall behind it changes
by more than 4 levels, without and with its core: villager 021, 3.6 to
0.13 % from the front and 6.1 to 0.25 % from the back; 381, 4.2 to 3.5 %
from the front and 1.0 to 0.3 % from the side. What is left is where the
figure has no core behind it (arms, hands, a staff) and its outline.

**Cost.** The core is skinned on the GPU, as every animated mesh is
([skeleton poses](skeleton-poses.md)): 32 figures' cores take about 0.01 ms
of the `skinning` pass, and on the CPU a core costs its palette and a job,
about 6 µs a figure whatever its size. It saves GPU time by hiding the
splats behind it. Walking figures in `examples/splat_crowd`'s layout on an
RTX 3080 at 1920 × 1080, 32 figures close, ms, the CPU the median of three
rounds and the GPU the least of them:

| Figure | Core | World pass (GPU), without / with | CPU a frame, without / with |
| --- | --- | --- | --- |
| villager 021, 31,817 Gaussians | 380 vertices, 800 triangles | 2.10 / 1.63 | 0.68 / 0.86 |
| villager 381, 15,908 Gaussians | 1,202 vertices, 2,400 triangles | 1.27 / 1.02 | 0.75 / 0.96 |
| villager 381, 6,144 cards | the same | 1.14 / 0.81 | 0.64 / 0.74 |

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

## Group tints

`SetSplatGroupTint` recolours one group of an instance's Gaussians (its
`_GROUP`): eyes closing in a blink, a face flushing, runes lighting up.

```csharp
// Lids down: the eye Gaussians (group 1) take the skin's colour.
engine.Animation.SetSplatGroupTint(new(townsman, 1, skin, blink));
```

- **Per instance.** Each Gaussian of the group draws
  `lerp(baked, Target, Weight)`. The instance's other Gaussians, and every
  other instance of the resource, draw as they did.
- **Display colour.** `Target` is sRGB-encoded display colour, as `_COLOR`
  is, and the lerp is in display space. It applies to the colour the figure
  draws, `_COLOR` or a lit figure's `_ALBEDO`, before the multiplier,
  lighting, the pixel grid and its palette. Opacity stays as baked.
- **Held.** The tint holds until changed, and is sent to the renderer only
  when it changes. A product may set it on every update. `Weight` 0 clears
  it.
- **Limit.** An instance tints at most 8 groups at once. A ninth is
  refused until one is cleared. Group 0, and a `Target` or `Weight` outside
  0 to 1, are refused.
- **Cost.** The group rides in the unused byte of each Gaussian's resident
  albedo word, so it adds no memory. Only a tinted instance reads it, and
  then a few compares per Gaussian against the instance's table. 32
  walking 48,000-Gaussian figures, every one blinking (its weight set
  every frame), take 2.33 ms a frame against 2.32 ms untinted on an
  RTX 3080 at 1920 × 1080, and 2.93 ms against 2.93 at 2560 × 1440.

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
| `Core` | its [core](#the-core), as the world's meshes cast; nothing without one | its own shape, without the splats' soft edge | a mesh of a few hundred triangles a figure |

**Lit crowds.** Walking 48,000-Gaussian townsmen in the web crowd layout
(`examples/splat_crowd --lights`) under a sun casting through four cascades,
ambient light and a point light. Frame throughput, ms, median of three
rounds:

| Figures | GPU, size | Unlit | Lit | Lit, `Ellipsoids` | Lit, `Splats` |
| --- | --- | --- | --- | --- | --- |
| 1 | RTX 3080, 1080p | 0.52 | 0.75 | 0.83 | 1.00 |
| 32 | RTX 3080, 1080p | 2.43 | 3.96 | 4.12 | 9.37 |
| 32 | RTX 3080, 1440p | 3.12 | 5.25 | 5.41 | 10.73 |
| 1 | RX 9070 XT, 1080p | 0.50 | 0.70 | 0.74 | 0.95 |
| 32 | RX 9070 XT, 1080p | 2.29 | 2.75 | 2.83 | 7.79 |
| 32 | RX 9070 XT, 1440p | 2.51 | 3.44 | 3.51 | 8.35 |

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

  Beside them, 8 more: its normal, its albedo and its group (and,
  skinned, 8 of skin).
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
- **Readout.** `GpuReadout.splats` reports the last frame's runs, instances,
  Gaussians and cards, summed over its views, and how many of each were
  sorted and drawn (`surviving_gaussians`, `surviving_cards`), read back
  from the GPU a frame or two later; a frame that draws no splats keeps
  none. The `splats` pass timer covers the projection, sort and layers of
  the frame's first view with a run.
- **Only what can show is sorted.** A run sorts only the splats its
  projection kept, in the order they stand in the run, so splats at the same
  depth draw in the same order every frame.
- **Occlusion culling.** With `RendererSettings.OcclusionCulling` (the
  product's `RustyEngineProductOcclusionCulling`), a view
  reduces its depth after its opaque pass (the world and the figures'
  [cores](#the-core)) to a farthest-depth pyramid, timed as
  `splats-occlusion`, and a splat lying wholly behind it is dropped before
  the sort. A Gaussian is tested at its centre's depth over its quad, a
  card at its nearest corner's over its four corners. Every fragment of a
  dropped splat would have failed the depth test, so the frame is the same
  to the pixel. A figure on a pixel grid is not tested: its layer is drawn
  whole.

  The pyramid costs about 0.03 ms a view on an RTX 3080 at 1920 × 1080.
  What it saves depends on what hides the splats. Walking 381s in
  `examples/splat_crowd`'s layout, 32 figures close, ms, median of three
  rounds:

  | Figures | Kept | Sort, off / on | Draw, off / on |
  | --- | --- | --- | --- |
  | 16k Gaussians, half behind a wall | 47 % | 0.196 / 0.133 | 0.423 / 0.359 |
  | cards, half behind a wall | 48 % | 0.122 / 0.098 | 0.366 / 0.348 |
  | 16k Gaussians with 2,400-triangle cores | 85 % | 0.197 / 0.183 | 0.516 / 0.482 |
  | cards with cores | 93 % | 0.120 / 0.119 | 0.405 / 0.398 |

  Fragments behind a core or a wall already failed early depth tests, so
  the draw gains less than the share dropped; the sort and projection's
  vertex work are what culling takes off. A crowd with nothing hidden pays
  the pyramid for nothing.
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
