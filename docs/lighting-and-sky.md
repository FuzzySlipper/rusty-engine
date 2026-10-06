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
shadow setup, but a layer draws only the parts inside its view and, for a
point or spot light with a `Range`, the parts that range reaches. Give lamps a
range: a lamp without one casts from everything within 500 m of it.

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

- The sky is looked at straight down over a 64 m square centred on the
  light's position and snapped to its texels, so move the light with the
  player (or camera) in a large world.
  Outside the square nothing is occluded.
- It is one more shadow layer: every shown part inside the square casts into
  it, and it re-renders only when the square moves or a part in it changes. Each lit fragment takes 25
  more comparison samples. Meshing is unchanged.
- It is presentation only. `Voxel.SampleDirectLighting` (below) still treats
  ambient light as unoccluded.

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
stay the same, so the two modes swap on one product setting.

## The standard shader

Retained meshes, voxel surfaces and GLB parts all draw with the Engine's
standard shader: metallic-roughness lighting (Lambert diffuse plus GGX
specular) from the scene's lights and requested shadows, in linear light,
then finished by the product's exposure, tone mapping and fog (below).
`MaterialRequest.Metalness` (and a GLB's metallic factor) runs from 0, a
dielectric, to 1, a metal. A metal has no diffuse; its specular takes the base
colour, and it reflects ambient light (and a hemisphere light along the
reflection) as a uniform environment, so its look depends on those lights.
Dielectrics take ambient light as diffuse only. There is no sky or
environment-map reflection. A material compiles only the features its contents
use:

| Feature | A material has it when |
| --- | --- |
| Unlit | it is a GLB `KHR_materials_unlit` material or an Engine primitive |
| Alpha mask | its alpha mode is `MaterialAlphaMode.Mask`, or its voxel surface is masked |
| Voxel surface | it carries a voxel surface mapping |
| Normal map | it is a GLB material with a normal texture, or `MaterialRequest.NormalMap` names one |
| Emissive, occlusion map | it is a GLB material with that texture |
| Triplanar | its `TriplanarSharpness` is nonzero ([three planes](smooth-voxel-surfaces.md#textures-on-reconstructed-surfaces)) |
| Stochastic tiling | its `StochasticTiling` is nonzero (three blended hex tiles per sample) |

Materials with the same features share pipelines and batch together. A new
feature set compiles once, when its first material is defined: about 8 ms on
an RX 9070 XT, then about 0.4 ms for each further pass it draws in. Doom's E1M1
uses two. The shader is composed from importable WGSL modules in
`render-shaders/src/shaders/` (`types`, `view`, `material`, `surface`,
`lighting`, `tonemap`, `finish`, `shade`).

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
distance fog, over the background. The background, a clear colour or sky panorama, is drawn
first and never finished. These settings are retained camera-view state, like
the sky, and products change them at runtime, for a cave, underwater or at
night:

```csharp
engine.CameraView.SetToneMapping(new(ToneMappingOperator.AcesFilmic, exposure));
engine.CameraView.SetFog(new(FogMode.Linear, fogColor, Start: 3, End: 30, Density: 0));
engine.CameraView.SetFog(new(FogMode.Off, default, 0, 0, 0));
engine.CameraView.SetBloom(new(Threshold: 1, Intensity: 0.6f));
engine.CameraView.SetAutoExposure(new(Enabled: true, Speed: 1.5f, MinExposure: 0.25f, MaxExposure: 4));
engine.CameraView.SetColorGrading(new(Temperature: -0.15f, Tint: 0, Contrast: 0.1f, Saturation: 0.2f));
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
  emission and highlights above white glow. The background does not bloom.
- **Auto exposure** scales the exposure toward the one that brings the
  world's log-average luminance (weighted by coverage, background excluded)
  to middle grey, clamped to `MinExposure..MaxExposure`, closing
  `1 - e^(-Speed × t)` of the gap in `t` seconds of presentation time, so it
  holds while the simulation is paused. The tone mapping exposure multiplies
  it, so it stays the product's brightness choice. The background is not
  exposed: choose a range that keeps the world in step with its sky (a dusk
  world raised to middle grey reads as day under a dusk sky). The first
  frame after it is enabled takes its target at once. It is off by default;
  `Enabled: false` turns it off. It adapts once a frame, at the frame's first
  world view.
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
- **Captures.** `RenderOutput.CaptureImage` uses its request's own exposure
  and tone mapping, without auto exposure or colour grading, and keeps the
  scene's fog and bloom as it keeps the scene's lights, whichever background
  it selects.

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
0.05 ms at 1280×720 and 0.1 ms at 1920×1080 (colour grading adds about
0.005 ms), and bloom with auto exposure about 0.15 ms; on llvmpipe each takes 5 to 17 ms, a large share of a light
scene's frame.

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
value and chooses authored keyframes, sun/moon positions/colors, horizon tint,
and stars in the panoramas. Blend consecutive pairs for dawn/day/dusk/night;
use coherent features and artwork to avoid double sun/moon images during a
crossfade. A blend is not a physical atmosphere simulation; distance fog is
`CameraView.SetFog`.
Update ordinary scene lights from the same product clock when illumination
should change too; sky presentation does not create environment lighting.

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
`lighting.room`, `lighting.fog <density>` (0 turns it off),
`lighting.exposure <exposure>` (ACES filmic), and `lighting.panorama`. Debug selection is explicit fixture
assistance; no downstream gameplay acceptance is implied.
