# Voxel lighting and product-driven skies

Use retained `Graphics` lights for cave and torch illumination. They support
point/spot attenuation and directional light. `ShadowIntent.Requested` draws
shadow maps only when the renderer enables shadows (`render-wgpu`
`RendererOptions::shadows`). A C# product enables them for both streamed and
window presentation with `RustyEngineProductSceneShadows` set to `enabled` in
its project. The default is `disabled`; requesting a light's shadows alone does
not enable the renderer. The CPU sample below uses each light's intent independently. Set
`RustyEngineProductDefaultWorldLights` to `disabled` for a dark unlit world;
otherwise the default neutral rig lights it. Emissive material
color makes a surface visible but does not emit light onto other surfaces. Pair
a torch's emissive appearance with a retained point light when it should light
its surroundings. Updating or disabling that light changes the same retained light.

## Shadows

Each shadowed light renders the scene into layers of a shadow atlas: four
cascades for a directional light, one layer for a spot light, six for a point
light. Every shown part of the scene layer casts, so a new object needs no
shadow setup, unless its appearance fact says `ShadowCasting.None`: a water
plane, glass or a fog card then throws no shadow, and small clutter costs no
caster draws; it still receives shadows. A layer draws only the casters inside
its view and, for a point or spot light with a `Range`, the ones that range
reaches. Give lamps a range: a lamp without one casts from everything within
500 m of it.

A directional light's shadow follows the camera, wherever its node is. Its
cascades split the view from the camera's near plane out to the light's
`Range` (set `HasRange`; 100 m by default, never past the camera's far
plane), each nearer cascade covering less ground in more detail. Receivers
blend from one cascade into the next, and the shadow fades out at the range.
A longer range spreads the same maps over more ground, so near shadows get
coarser. Each world view, a capture included, fits the cascades to its own
camera, so a composition that draws several world views a frame renders them
once for each.

A layer re-renders only when its view changes (the light moves, turns or
changes range; a cascade when the camera moves) or a part in it is added,
removed, moved or posed. A lamp that flickers by changing colour or intensity
re-renders nothing, and a moving object re-renders only the layers that see
it.

### Quality and budget

A light's descriptor tunes its own shadow:

- `ShadowResolution`: texels on a side of each of its layers, 256, 512, 1024
  or 2048 (other values round up; 0 takes the default: 512 for point and spot
  lights and an ambient light's sky, 1024 for each cascade). Layers share
  2048² atlas pages by size, so a 2048 layer takes a page, a 512 one a
  sixteenth. A lamp a metre from a wall needs fewer texels than a sun.
- `ShadowSoft`: a wider filter, 5×5 samples rather than 3×3, for softer edges
  at more sampling cost.
- `ShadowPriority`: which lights a shadow budget keeps (below).

Casters draw with a slope-scaled depth bias, and receivers look a layer up
from a point moved one and a half of its texels along their normal, so lit
surfaces do not shadow themselves at any resolution or distance.

`RustyEngineProductShadowBudget` caps the shadow layers rendered at once
(0, the default, for no limit). With a budget, the Engine chooses which
requesting lights cast each frame: higher `ShadowPriority` first, then nearer
the camera (a directional or ambient light counts as nearest), each while its
layers fit; a light already casting counts a metre nearer so the choice does
not flicker. The others light without a shadow. A product keeps its intents and
priorities and needs no nearest-N policy of its own. `engine.renderer.status`
reports the layers, pages, budget, casting lights, the renderer handles of
lights the budget left out, and the layers and casters re-rendered last frame.

## Dark caves: an ambient light's sky

An ambient light reaches every surface alike, so on its own a cave is as
bright as the open ground beside it. Give the sky's ambient light
`ShadowIntent.Requested` (with renderer shadows enabled) and it reaches a
surface only where the sky above it is open: ground under rock, an overhang
or a roof loses it, fading over about a metre at a cave mouth, while open
ground and the walls facing out of it keep it. Keep a second, unshadowed
ambient light for the fill a dark interior should still have, and torches for
the rest.

- The sky is looked at straight down over a square centred on the light's
  position and snapped to its texels, so move the light with the player (or
  camera) in a large world. The square is 64 m a side by default; the
  light's `Range` (set `HasRange`) is half its side, so a landscape's view
  distance or one room gets its own extent. A wider square spreads the same
  texels over more ground (raise `ShadowResolution` with it). Outside the
  square nothing is occluded.
- It is one more shadow layer: every shown part inside the square casts into
  it, and it re-renders only when the square moves or a part in it changes. Each lit fragment takes 25
  more comparison samples. Meshing is unchanged.
- It is presentation only. `Voxel.SampleDirectLighting` (below) still treats
  ambient light as unoccluded.

## A hemisphere light

`LightKind.Hemisphere` lights a surface from the sky above and the ground
below, blended by how far it faces up: its `Color` is the sky's, its
`GroundColor` the ground's, both scaled by `Intensity`
(`LightDescriptor.Hemisphere(sky, ground, intensity)`). The Engine's neutral
rig lights the world with one, so a product that disables the default world
lights (`RustyEngineProductDefaultWorldLights`) to own its lighting makes its
own rather than falling back to a flat ambient. It casts no shadow, metals
reflect it as they reflect ambient light, and `Voxel.SampleDirectLighting`
blends it by the sample's normal.

## Indirect light: the probe volume

An ambient light's sky ([above](#dark-caves-an-ambient-lights-sky)) darkens
a cave under one light; the probe volume lights an interior from what is
actually there. `CameraView.SetIndirectLight(new IndirectLightRequest(center,
extent, spacing, bounces, ambient))` asks for one volume over the box
`center ± extent`. The Engine places probes `spacing` apart (0.5 to 8 m, at
most 262,144 probes), traces 64 rays from each through the world's shown
opaque triangles, and keeps what arrives as low-order spherical harmonics
(four RGB coefficients a probe). A ray that leaves the scene sees the sky:
the hemisphere light, the sky's light when it is on, and the ambient light
when `ambient` is `IndirectAmbient.Sky`. A ray that hits a surface sees its
emission and its colour (times its texture's mean, or its atlas region's)
lit by the directional, point and spot lights that reach it through shadow
rays; `bounces` passes (1 to 4) carry that light on, each pass one bounce
further. A probe inside rock is filled from its neighbours: one on a wall's
surface from the side its rays found open, one inside the wall from the side
its nearest way out faces.

Where the volume covers a surface the standard shader samples it trilinearly
(offset along the normal) in place of the ambient and hemisphere rows and
the sky's light, fading back to them over one cell past the box; metals
reflect it. With `IndirectAmbient.Sky` an enclosed room gets only what its
torches bounce and what comes in through its openings, so a cave is dark by
itself and a doorway spills daylight a few metres in. With
`IndirectAmbient.Floor` the ambient light stays everywhere and the probes
add the hemisphere, bounce and emission over it, for a product whose ambient
is a deliberate fill. An extent of zero turns the volume off; the next frame
draws exactly as before.

- The bake runs on worker threads, never the render thread: the old volume
  keeps drawing until the new one is uploaded. Any retained change inside the
  box (a part, a material, a light) rebakes the whole volume once the scene
  has been still for a quarter second, so a world built over many frames
  bakes once; while something inside keeps moving every frame it rebakes
  every two seconds instead. The cost scales with probes × rays × scene.
  `engine.renderer` reports `gpu.indirectLight`: probes, probes filled from
  neighbours, triangles traced, the last bake's milliseconds, the bakes so
  far, whether one is pending, and the volume's GPU bytes. On 20 threads the 19.5 k-probe
  cave of the lighting exploration (30 k triangles, two bounces) bakes in
  about 110 ms and the 41 k-probe Hotel corridor (180 k triangles) in about
  170 ms. Starting a bake while the sky's light is on reads its irradiance
  back from the device (144 bytes, a short wait on the render thread).
- Trilinear sampling reads the probes within one cell of a surface, on
  both sides of a wall thinner than the spacing: at 2 m a 1 m wall lets about
  a fifth of the daylight beyond it onto its inner face. Where thin walls
  matter, keep the spacing at or under their thickness (the fixture's room
  uses 0.5 m), or thicken them.
- It is one volume. A product moves it with the player in a large world; a
  volume that follows the camera and rebakes only the bricks the world
  changed in is #9597.
- Per lit fragment the shader adds three trilinear reads of one small 3D
  texture (RGBA16F, 24 bytes a probe), six for metals and under the sky's
  light. At 1080p that is 0.01 to 0.05 ms of world pass on an RX 9070 XT and
  1 to 4 ms on llvmpipe (4 to 14 percent of its world pass; the share is
  highest where the pass is otherwise cheap). `rusty-scene-render --indirect-light
  cx,cy,cz,ex,ey,ez,spacing,bounces[,floor]` bakes a volume before its frames
  and reports it under `gpu.indirectLight`.

## The sky's light

The background can light the world: a sky panorama (or the two a blend
mixes), or the clear colour as a uniform sky. It is off by default, so a
scene keeps its look until the product turns it on:

```csharp
engine.CameraView.SetSkyLight(new(Intensity: 1));
```

- Every standard-shader surface takes the sky's irradiance as diffuse light,
  from nine spherical-harmonics coefficients, so ground under a blue sky
  turns blue and a sunset warms what faces it. Every surface also reflects
  the sky, prefiltered for its roughness from a 128² cube with
  GGX-prefiltered mips. A metal tints the reflection with its colour; a
  dielectric reflects a few percent, more at grazing angles, so wet rock,
  water and glossy paint show the sky. This reflection replaces a metal's
  uniform reflection of ambient and hemisphere light. Those lights still
  light as before.
- An ambient light's [sky layer](#dark-caves-an-ambient-lights-sky), occlusion
  maps and screen-space ambient occlusion shade the sky's light as they shade
  ambient light, so a cave stays dark.
- `Intensity` (0 to 16) scales the background's radiance: panorama colours
  are read as linear light, so at 1 a white surface under a white sky is
  white.
- The Engine builds it on the GPU whenever the background changes (its
  selection, blend amount or colour). A whole build takes about 0.6 ms on an
  RX 9070 XT and 30 ms on llvmpipe. The first build lands in its frame. A
  blend that moves every frame is built over the next three frames while the
  last light holds, about 0.2 ms (6 ms on llvmpipe) a frame. A still sky
  costs a cube sample and nine coefficients per shaded fragment.
  `engine.renderer` times the build as `sky-light`.
- Product shaders that call `standard_shade` get it with the rest of the
  standard lighting.

## Renderer settings

`RendererSettings` is the product's view of the renderer's pipeline features
and quality: `Read()` returns a `RendererSettingsReadout` and
`Set(RendererSettingsRequest)` replaces every setting from the next frame,
and `Read()` reports the change from the next product call.
The request holds shadows on or off and their budget (0 for no limit),
ambient occlusion (`Disabled`, `ScreenSpace` or `DistanceField`, with its
strength and radius), antialiasing (`Off`, `Msaa2` or `Msaa4`), the render
scale (0.5 to 1: the world, viewmodel, labels and effects draw at that
fraction of the frame's size and are upscaled bilinearly into it, while a
playing video and the browser or window UI keep their full size), vsync,
and the clustered lighting and GPU culling switches. The product manifest's
`RustyEngineProduct*` properties are the initial values
([the product project](csharp-product-project.md#renderer-settings)), so a
graphics menu starts from `Read().Requested`, changes what it offers and sets
the rest back unchanged. The selection is retained with the scene (a
rebaseline keeps it, and a scene snapshot records it); the Engine chooses
how each setting is drawn, such as the pass that computes screen-space
occlusion.

The readout carries `Requested`, what the product or its manifest asked for,
and `Effective`, what draws, with a refusal per setting the device draws
differently: `NoComputeShaders` (distance-field occlusion and clustered
lighting fall back to the screen-space pass and the light loop),
`NoIndirectDraws` (GPU culling keeps the CPU list), `UnsupportedSampleCount`
(the adapter cannot multisample at that count, so the default 4 draws) and
`NoDisplay` (streamed output has no display, so vsync is moot) and
`VsyncOnly` (the window's display has no immediate or mailbox present mode,
so frames wait for its refresh with vsync off). A refused
setting is not an error: `Set` accepts any finite, non-negative strength, positive
radius and render scale from 0.5 to 1, and refuses only values outside
those ranges.
`engine.renderer` reports the same under `settings`
([renderer statistics](performance.md#renderer-statistics)). The
`csharp-lighting-sky` fixture's `lighting.settings`, `lighting.antialiasing`,
`lighting.occlusion`, `lighting.shadows` and `lighting.pipeline` commands
exercise it.

## Contact darkening: screen-space ambient occlusion

The sky's shadow darkens at the scale of a cave; screen-space ambient
occlusion darkens within about 0.75 m where surfaces meet: corners, the foot
of a wall, a crate on the floor. It scales only the ambient and hemisphere
light of opaque, lit parts (`Surface.occlusion`, so product shaders calling
`standard_shade` get it), so it shows where ambient light carries the scene
and barely in one lit mostly by torches. It is off by default; a product
turns it on with `RustyEngineProductAmbientOcclusion` (see
[the product project](csharp-product-project.md#screen-space-ambient-occlusion)),
and `engine.renderer` reports its passes' GPU time.

Its `distanceField` mode replaces the screen-space pass with a cone trace
through the voxel chunks' signed distance fields, which the chunk mesher
builds from the chunk and its neighbours and the renderer keeps in one 3D
atlas; occlusion then comes from the world around a surface, out to half a
chunk, rather than from what the view shows. The depth pre-pass and the blur
stay the same, so the two modes swap on one product setting. The fields are
built, published and held in the atlas only while that mode draws: a scene
whose product does not select it pays nothing for them.

## The standard shader

Retained meshes, voxel surfaces and GLB parts all draw with the Engine's
standard shader: metallic-roughness lighting (Lambert diffuse plus GGX
specular) from the scene's lights and requested shadows, in linear light,
then finished by the product's exposure, tone mapping and fog (below).
`MaterialRequest.Metalness` (and a GLB's metallic factor) runs from 0, a
dielectric, to 1, a metal. A metal has no diffuse; its specular takes the base
colour, and it reflects ambient light (and a hemisphere light along the
reflection) as a uniform environment, so its look depends on those lights.
Dielectrics take ambient light as diffuse only. With [the sky's
light](#the-skys-light) on, every surface reflects the sky instead. A
material compiles only the features its contents use:

| Feature | A material has it when |
| --- | --- |
| Unlit | it is a GLB `KHR_materials_unlit` material, an Engine primitive, or `MaterialRequest.Unlit` is set (signage, UI in the world: the base colour as it is, no lights or emission) |
| Alpha mask | its alpha mode is `MaterialAlphaMode.Mask`, or its voxel surface is masked |
| Voxel surface | it carries a voxel surface mapping |
| Normal map | it is a GLB material with a normal texture, or `MaterialRequest.NormalMap` names one |
| Emissive map | it is a GLB material with an emissive texture, or `MaterialRequest.EmissionMap` names one (sRGB; it multiplies `EmissionColor` × `EmissionIntensity`) |
| Occlusion map | it is a GLB material with an occlusion texture, or `MaterialRequest.OcclusionMap` names one (opened with `TextureColorSpace.Linear`; its red channel scales the ambient, hemisphere and sky light the surface takes) |
| Triplanar | its `TriplanarSharpness` is nonzero ([three planes](smooth-voxel-surfaces.md#textures-on-reconstructed-surfaces)) |
| Stochastic tiling | its `StochasticTiling` is nonzero (three blended hex tiles per sample) |

Materials with the same features share pipelines and batch together. A new
feature set compiles once, when its first material is defined: about 8 ms on
an RX 9070 XT, then about 0.4 ms for each further pass it draws in. Doom's E1M1
uses two. The shader is composed from importable WGSL modules in
`render-shaders/src/shaders/` (`types`, `view`, `material`, `surface`,
`lighting`, `tonemap`, `finish`, `shade`).

Instances of one mesh need not each have a material to look different:
`Graphics.UpdateStaticMeshMaterialFactors(new(appearance, factors))` sets,
per material slot of one static mesh appearance, a base colour, a texture
tint and an emission over the slot's material (`MeshMaterialFactors.Tint`
for the common case), as `Animation.UpdateAnimatedMeshMaterialFactors` does
for a GLB's slots; an empty set restores the materials' own. Appearances of
one mesh each carry their own factors, and the instances still batch.

## Product shaders

Grow the standard shader through an Engine request when a look is common. For
a look of the product's own (toon or rim lighting, dissolves, stylised
colour), write a shader. It is one WGSL file in the product's content, opened
like a texture:

```csharp
RenderResource toon = engine.Graphics.OpenResource(new RenderResourceRequest("shaders/toon.wgsl")).Handle;
Material material = engine.Graphics.CreateMaterial(request with
{
    Shader = new MaterialShader(toon, new Vector4(0.2f, 0.8f, 1f, 3f)),
});
```

The file defines `shade`, which the world pass calls in place of the standard
shade stage, after the standard stages have sampled the material's textures,
normal map, triplanar planes and alpha mask:

```wgsl
#import rusty::types::Surface
#import rusty::material::material
#import rusty::view::frame
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
    let lit = standard_shade(surface);
    let view = normalize(frame.camera.xyz - surface.world_position);
    let rim = pow(1.0 - max(dot(surface.normal, view), 0.0), material.parameters[0].w);
    return vec4<f32>(lit.rgb + material.parameters[0].rgb * rim, lit.a);
}
```

- **Per material, not per instance.** `MaterialShader`'s four parameter
  vectors and two textures belong to the material, like the standard
  shader's colour and maps: an object that needs its own values gets its own
  material (`UpdateMaterial` changes them at runtime), and the per-instance
  factors above (base colour, texture tint, emission) still apply over a
  product-shaded material. A per-instance parameter vector is not built until
  a product needs one.
- **What it gets.** `Surface` holds the base colour and alpha, the tint
  (material, node and vertex colour without the texture), the shading normal,
  world position, uv, roughness, metalness, occlusion and emission.
  `material.parameters` holds the four `MaterialShader` vectors, and
  `product_map_a` and `product_map_b` (with `product_sampler_a`/`_b`, in
  `rusty::material`) its `TextureA` and `TextureB`: white when unset, sampled
  as they were opened (a ramp or noise read as data is opened with
  `TextureColorSpace.Linear`). `frame.time.x` (`rusty::view`) is the Engine's
  presentation time in seconds: it advances with the simulation, holds while
  it is paused, and needs no material update, so scrolling, pulsing and
  dissolving cost nothing per frame on the C# side. Any standard module may
  be imported: `rusty::shade::standard_shade` lights and finishes a surface,
  `rusty::lighting` has `standard_radiance` and the light rows (laid out
  in `rusty::types::Light`; a shadowed directional light's row carries its
  cascades' split depths and view axis rather than a position and range),
  `rusty::finish::finish` returns its colour unchanged (the finish pass
  finishes everything the world draws), and `rusty::material` has the
  material's textures and samplers.
- **What it returns.** The fragment's colour in linear light, and its
  alpha. The [finish pass](#exposure-tone-mapping-and-fog) applies bloom,
  exposure, tone mapping and fog after it. It may `discard`.
- **Variants.** It compiles once per standard feature set its materials use
  and may test them (`#ifdef NORMAL_MAP`, `UNLIT`, `VOXEL_SURFACE`,
  `TRIPLANAR`…). Its own keywords, like Unity's shader features, are chosen
  when it is opened: `new RenderResourceRequest("shaders/fx.wgsl") with {
  ShaderKeywords = "DISSOLVE GLOW" }` opens that variant as its own resource
  (in any order), compiled with `#ifdef DISSOLVE` and `#ifdef GLOW` true.
  Keywords are upper case (`[A-Z_][A-Z0-9_]*`) and may not be a standard
  feature's name. Materials sharing a shader variant and feature set batch
  together; each variant adds its own pipelines.
- **Shadows.** A shader may also define `fn cast_shadow(caster: Caster)`,
  which the shadow pass calls for its materials after the alpha mask and
  which may `discard`, so a dissolve's shadow follows its image. `Caster`
  holds the uv, world position and alpha (part, vertex and base texture).
  It may read `frame.time`: while such a caster draws, the shadow maps
  redraw as presentation time moves. Defined under a keyword's `#ifdef`, only
  that variant has it. Without it, materials cast through the standard
  caster.
- **Checked when opened.** `OpenResource` checks the keywords and composes
  the shader (and its caster stage) with the standard modules, refusing an
  error with `CSHARP_SHADER`, naming the file, line and column. A product
  shader draws in blended passes too.
- **Lifetime.** A shader resource is held while a material uses it. Under
  `rusty dev`, an edited loose `.wgsl` restarts the runtime and is checked
  again; the stream and the window draw it alike.

```wgsl
#import rusty::types::{Surface, Caster}
#import rusty::material::{material, product_map_a, product_sampler_a}
#import rusty::view::frame
#import rusty::shade::standard_shade

// Noise below a rising threshold is cut away, in the image and the shadow.
fn dissolved(uv: vec2<f32>) -> bool {
    let noise = textureSampleLevel(product_map_a, product_sampler_a, uv, 0.0).r;
    return noise < fract(frame.time.x * material.parameters[0].x);
}

fn shade(surface: Surface) -> vec4<f32> {
#ifdef DISSOLVE
    if dissolved(surface.uv) {
        discard;
    }
#endif
    return standard_shade(surface);
}

fn cast_shadow(caster: Caster) {
#ifdef DISSOLVE
    if dissolved(caster.uv) {
        discard;
    }
#endif
}
```

## Exposure, tone mapping and fog

The world (lit and unlit meshes, voxel surfaces, GLB parts, sprites and
particles) draws in linear light into a 16-bit floating-point target, with the
view's multisampling, and blends there. A full-screen finish pass then
finishes it: bloom, exposure, colour grading, the tone mapping operator, then
distance fog, sample by sample at edges, and writes the average over the
background into the view's single-sample image. The background, a clear
colour or sky panorama, is drawn there first and never finished. Labels test
the world's depth sample by sample in their shader, so their edges keep the
multisampled coverage. These settings are retained camera-view state, like
the sky, and products change them at runtime, for a cave, underwater or at
night:

```csharp
engine.CameraView.SetToneMapping(new(ToneMappingOperator.AcesFilmic, exposure));
engine.CameraView.SetFog(new(FogMode.Linear, fogColor, Start: 3, End: 30, Density: 0));
engine.CameraView.SetFog(new(FogMode.Off, default, 0, 0, 0));
engine.CameraView.SetBloom(new(Threshold: 1, Intensity: 0.6f));
engine.CameraView.SetAutoExposure(new(Enabled: true, Speed: 1.5f, MinExposure: 0.25f, MaxExposure: 4));
engine.CameraView.SetColorGrading(new(Temperature: -0.15f, Tint: 0, Contrast: 0.1f, Saturation: 0.2f));
engine.CameraView.SetAtmosphere(new(FogBaseHeight: 20, FogFalloffHeight: 40, HazeColor: new(1f, 0.7f, 0.45f, 1),
    HazeExponent: 8, SunRadiusDegrees: 1.5f, SunHalo: 0.35f));
engine.CameraView.SetSunShafts(new(Intensity: 0.8f, Length: 0));
```

- **Exposure** multiplies lit colour; it defaults to 1.
- **Operators.** `None` (the default) clamps at the target's range, so bright
  lights clip to white or a saturated primary. `Neutral` (Khronos PBR Neutral)
  keeps base colours true and compresses only highlights. `AcesFilmic` adds
  film-like contrast and rolls highlights toward white. Choose one before
  tuning light intensities: the same lights read differently under each.
  Emission and light above 1 are kept until the operator, so a lamp shade at
  full emission rolls off under `Neutral` or `AcesFilmic` instead of clipping.
- **Bloom** spreads the world's light above `Threshold` (linear, before
  exposure, with a soft knee below it) through six half-resolution mips and
  adds it back at `Intensity` (0 to 16) before exposure and the operator, so
  a glow rolls off as the light does. It is off by default; an intensity of 0
  turns it off. A threshold of 1 keeps ordinary lit surfaces out and lets
  emission and highlights above white glow. The background does not bloom,
  and each view of a split screen blooms only its own viewport.
- **Auto exposure** scales the exposure toward the one that brings the
  world's log-average luminance (weighted by coverage, background excluded)
  to middle grey, clamped to `MinExposure..MaxExposure`, closing
  `1 - e^(-Speed × t)` of the gap in `t` seconds of presentation time, so it
  holds while the simulation is paused. The tone mapping exposure multiplies
  it, so it stays the product's brightness choice. The background is not
  exposed: choose a range that keeps the world in step with its sky (a dusk
  world raised to middle grey reads as day under a dusk sky). The first
  frame after it is enabled takes its target at once. It is off by default;
  `Enabled: false` turns it off. It adapts once a frame, measuring the
  frame's first world view's viewport.
- **Colour grading** adjusts the world's colour before the operator, so the
  operator still rolls off what grading brightens. Each control runs from -1
  to 1, and 0 leaves the colour as it is: `Temperature` cools toward blue or
  warms toward yellow (white balance), `Tint` shifts toward green or
  magenta, `Contrast` scales the distance from middle grey on a log scale
  (flat at -1, doubled at 1) and `Saturation` the distance from grey (none
  at -1, doubled at 1). `default(ColorGradingRequest)` turns it off. A
  scene's warm lights pushing a cool material toward olive under `Neutral`
  are pulled back by a cooler temperature and a little saturation. Fog and
  the background are not graded, so fog still fades geometry into a
  background of its colour.
- **Fog.** `Linear` has none before `start` and is full at `end` (metres from
  the camera). `Exponential` leaves `exp(-density × distance)` of the colour,
  and `ExponentialSquared` leaves `exp(-(density × distance)²)`, which is
  clearer near and denser far. The colour is linear RGB; alpha is ignored.
  Fog blends after tone mapping, so a fog colour equal to the background
  colour fades distant geometry exactly into it. Over a sky panorama, pick a
  colour that matches its horizon. Distance comes from the depth buffer: a
  blended surface takes the fog of what is behind it, and over the background
  it is not fogged.
- **Atmosphere** makes the fog read as air, and puts the sun in the sky.
  The sun is the brightest enabled directional world light. All zero
  (`default(AtmosphereRequest)`) turns it off, and each part is off at
  zero:
  - Fog has its `SetFog` density at `FogBaseHeight` (render world y) and
    thins by `e` every `FogFalloffHeight` above it, thickening below.
    Each fragment takes the mean density along its ray from the camera,
    so valleys fill and distant peaks stand clear.
  - Looking toward the sun, the fog colour turns toward `HazeColor`
    (linear RGB) by the cosine between the view and the sun raised to
    `HazeExponent`. A larger exponent gathers the haze closer to the sun.
  - The sky, panorama or clear colour alike, shows a disc of
    `SunRadiusDegrees` (up to 20) and a halo a few degrees wide of
    strength `SunHalo`. Both take the sun's colour, dimmed when its
    intensity is below 1, and follow its direction.
- **Sun shafts** stream light from the sun past whatever stands in front of
  it: a tree line at dusk, a doorway. At half resolution, the sky the world
  leaves uncovered near the sun is blurred along rays toward the sun, and
  added in the sun's colour at `Intensity` (0 to 16; 0 turns them off)
  before exposure and the operator, as bloom is. `Length` (0 to 1) is how
  far the rays reach, as a fraction of the way from each pixel to the sun;
  0 takes the Engine's default, 0.6. The sun is the atmosphere's: the
  brightest enabled directional world light. The shafts fade out as the sun
  nears the horizon or moves half a view beyond its edge, and draw nothing
  below the horizon or behind the camera. Each view of a split screen
  treats its own viewport.
- **Captures.** `RenderOutput.CaptureImage` uses its request's own exposure
  and tone mapping, without auto exposure or colour grading. It keeps the
  scene's fog, atmosphere, bloom, sun shafts and sky light as it keeps the
  scene's lights, whichever background it selects.

Changing any of these recompiles nothing: they are values in uniforms, so a
product may update them every frame. Fog and tone mapping do not change
`Voxel.SampleDirectLighting`, which reports linear light.

A product shader's `shade` returns linear colour, and the finish pass
finishes every fragment; `rusty::finish::finish` returns its colour
unchanged, so calling it changes nothing.

The world's target costs 8 bytes a sample (66 MB at 1920×1080 with 4×
multisampling), plus a single-sample copy (17 MB) and bloom's mips (6 MB)
while bloom or auto exposure is on. `engine.renderer` times the frame's first
world view's `world`, `bloom-exposure` and `finish` passes
([performance](performance.md)). On an RX 9070 XT the finish pass takes about
0.04 to 0.06 ms at 1280×720 and 0.07 to 0.13 ms at 1920×1080 (colour grading
adds about 0.005 ms), and bloom with auto exposure about 0.15 ms. On llvmpipe
the finish pass takes 3 to 8 ms at 1280×720 and 7 to 17 ms at 1920×1080, and
bloom with auto exposure 4 to 10 ms, a large share of a light scene's frame.

## Read light at a voxel location

`Voxel.SampleDirectLighting(VoxelLightSampleRequest)` evaluates the same typed
`LightDescriptor` values used by `Graphics.CreateLight` / `UpdateLight`. Keep
those descriptors in product state and pass the selected world lights to both
operations. Do not create a product renderer, flood-fill solver, or second light
registry. The query returns linear RGB incident irradiance, luminance, counts
of contributing/occluded sources, and scene/voxel-collision/static-collision/rebase revision facts.

The sample point is `(Address + Offset) * voxelSize - worldOrigin`, with offset
in voxel units. `(0.5,0.5,0.5)` samples a cell center. A zero `Normal` samples
incident light without an orientation cosine; a nonzero normal is normalized
and selects surface irradiance. Sample just outside a solid face to avoid
self-occlusion. Light positions/directions must already be in local world space;
parent-relative descriptors must be transformed by the product's ordinary
scene composition before sampling. `DirectionalDistance` is a finite positive
shadow-query horizon chosen for the loaded world.

Point/spot light uses inverse-power decay and a smooth range cutoff, matching
the renderer's direct-light attenuation. Spot cones include penumbra; directional
lights use their opposite travel direction. Ambient light is unoccluded here,
even when it requests shadows ([its sky](#dark-caves-an-ambient-lights-sky)
is presentation only), so it cannot make sealed caves dark. Disabled/zero-intensity and out-of-range lights
contribute nothing.
`ShadowIntent.Requested` enables a ray against the current
voxel and retained static-mesh **collision** projection. This is an explicit
CPU lighting proxy, not GPU shadow-map or final-pixel readback: non-collidable
visual occluders, active entities, translucent shadow transmission, indirect
bounce, tone mapping and material response are not included. Keep the relevant
opaque cave geometry collision-resident. Unloaded geometry cannot occlude.

This direct-light sample gives dark enclosed rooms, local gradients, and
product-readable light at addresses. It does not store or propagate a
Minecraft-style sky/block-light lattice, or supply indirect light around corners.
The visual path is the renderer's ordinary lights and shadows.

### Persistence and cost

Persist the source light descriptors alongside the product's own voxel data in
its save envelope. On load, replace the chunks through `Voxel.ApplyResidency`,
restore the same descriptors through `Graphics`, then resample. Handles and derived light samples
are not persistence authority. `fixtures/csharp-lighting-sky` demonstrates a
source-generated JSON envelope that round-trips both values without a second
Engine voxel store.

Each sample visits the supplied lights once, with at most one accelerated
world-collision ray per contributing shadowed nonambient source. The bridge
copies the descriptor span for the call; work and temporary memory are O(L),
plus spatial query cost. Sampling Q addresses costs Q times that work. There is
no whole-world allocation/remesh on light placement, and no stored-light memory
per voxel. Products choose sampling frequency and spatial budget; cache only
while light descriptors and relevant scene/collision/origin facts are unchanged.
Rendering shadows has separate GPU cost (point shadows require six views); the
CPU readout is not a frame-rate or exact visual-brightness guarantee.

## Blend authored time-of-day skies

Open two ordinary retained 2:1 sRGB, clamp-wrapped panoramas once, then call:

```csharp
engine.CameraView.SetSkyBackgroundBlend(new(dayTexture, nightTexture, amount));
```

`amount` must be finite and in [0,1]. The product maps its own clock to this
value and chooses authored keyframes, horizon tint and stars in the
panoramas. A sun painted into a panorama stays where it was painted; the
atmosphere's sun disc (above) follows the directional light instead, so
moving the light with the same clock moves the sun in the sky. Blend consecutive pairs for dawn/day/dusk/night;
use coherent features and artwork to avoid double sun/moon images during a
crossfade. A blend is not a physical atmosphere simulation; distance fog is
`CameraView.SetFog`.
Update ordinary scene lights from the same product clock when illumination
should change too. With [the sky's light](#the-skys-light) on, the blend
lights the world as it moves.

The Engine retains both texture dependencies and one sky shader/geometry.
Changing the amount updates uniforms only, with no texture upload, mesh rebuild,
or new renderer resource. Cost is two panorama samples per visible sky pixel.
Different panorama resolutions are supported. The blend is retained
presentation state like the rest of the camera view.

`SetSkyBackground(texture)` selects a single panorama;
`SetBackgroundColor` selects an opaque clear color; `ClearSkyBackground` returns
to the Engine default. Selecting one replaces the other. Both selected blend
textures stay live until the selection changes; clear the sky before releasing
them. The Engine owns GPU lifetime and panorama orientation.

## Fixture

`fixtures/csharp-lighting-sky` uses the packaged SDK, a voxel room, a retained
torch light, a persistence round-trip and two deterministic authored panoramas.
Commands: `lighting.inspect`, `lighting.torch true|false`, `lighting.sky 0..1`,
`lighting.room`, `lighting.cave` (from the back wall toward the doorway),
`lighting.indirect sky|floor|off` (the probe volume over the room),
`lighting.fog <density>` (0 turns it off),
`lighting.exposure <exposure>` (ACES filmic), `lighting.atmosphere true|false`
(height fog, sun haze, disc and halo) and `lighting.panorama`.
`lighting.sky` also moves the fixture's sun from noon at 0 to a low dusk sun
at 1. Debug selection is explicit fixture
assistance; no downstream gameplay acceptance is implied.
