# Gaussian splat characters

A splat character is an animated GLB whose mesh is a cloud of Gaussians
rather than triangles: each point a small soft ellipsoid with its own
colour, as asset-pipeline bakes characters from paintings and orbit videos.
The Engine admits it like any animated GLB and draws it as Gaussians,
sorted back to front and blended into the world.

Splat characters draw at rest. They are not posed by clips or skinned to a
skeleton yet.

## The asset

One GLB with one or more nodes drawing splat primitives. A splat primitive
is a `POINTS` primitive (`mode` 0) with one value per Gaussian in each of:

| Attribute | Type | Meaning |
| --- | --- | --- |
| `POSITION` | float VEC3 | Centre, in the node's space (bind space on a skinned node). |
| `_ROTATION` | float or normalized signed VEC4 | Orientation, a unit quaternion xyzw. |
| `_SCALE` | float VEC3 | Standard deviation along each of the Gaussian's own axes, in metres (linear, not log). |
| `_COLOR` | float or normalized unsigned VEC4 | **sRGB-encoded** display colour in rgb; **linear** opacity in a. |
| `JOINTS_0`, `WEIGHTS_0` | as glTF | The skin, on a skinned node. |

`_COLOR` is not `COLOR_0`. glTF defines `COLOR_0` as linear, and these
colours are display-referred: they are the values the splats were trained
to blend.

The figure stands on y = 0, +Y up, in metres, facing +Z.

A GLB with splat primitives has only splat primitives, and carries no clips
of its own. Admission refuses a missing attribute, attributes of different
lengths and non-finite values. Its bounds are the Gaussians' centres to
three standard deviations, each node's at rest (a skinned node's in bind
space).

Products open it as any animated GLB:

```csharp
RenderResource body = engine.Animation.OpenAnimatedMeshFromContent(new(bodySource));
```

`CreateAnimatedMeshAppearance` and `CreateInstance` place it, and every
instance of one resource reads the same resident Gaussians.

## How it draws

- **Resident data.** The renderer reads the GLB as admitted and keeps
  32 bytes per Gaussian on the GPU, shared by every instance:
  - the centre;
  - the colour and opacity;
  - the rotation;
  - the standard deviations as halves.
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

Measured against a CPU reference of the web kernel, townsman 385 at rest
(48,000 Gaussians, 420 × 720, five views) differs as follows:

- **Figure mean:** within 0.12 levels per channel.
- **Pixels at least half covered:** a median difference of 0.5 levels and
  a 99th percentile of 7 to 10.
- **Soft edges:** differ by up to about 30 levels (95th percentile), as
  explained under **Colour** above.

## Devices and cost

- **Requirements.** Splats need compute shaders, storage buffers read in
  vertex shaders and indirect draws. A device without them draws no splats,
  and the readout says why. Software adapters draw them, slowly.
- **Limit.** A run sorts at most 4,194,304 Gaussians. An instance that does
  not fit draws nothing, and the readout counts it.
- **Readout.** `GpuReadout.splats` reports the last frame's runs, instances
  and Gaussians, summed over its views. The `splats` pass timer covers the
  projection, sort and layers of the frame's first view with a run.
- **Measured cost.** One 48,000-Gaussian figure at 420 × 720 took 0.17 ms
  of GPU time on an RX 9070 XT.
