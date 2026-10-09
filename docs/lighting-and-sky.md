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

A blended material's parts cast no shadow (a lake does not shadow its bed,
glass throws no block of dark) unless the material sets
`TranslucentShadow`, when they cast as opaque parts do. A layer re-renders
only when its view changes (the light moves, turns or changes range; a
cascade when the camera moves) or a part in it is added, removed, moved or
posed. A lamp that flickers by changing colour or intensity
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
lights the budget left out, the layers and casters re-rendered last frame, and
the atlas's GPU bytes (`atlasBytes`, every allocated depth page). Its
`gpu.passes` time a frame's shadow-layer rendering as `shadows`, in the frames
that render layers.

A point or spot light's layer re-renders only when a caster in it moves. A
caster that moves while the layer draws it (a posed character, a door, a
carried prop) becomes dynamic for that layer. The layer keeps its static
casters' depth in a static cache and, while only dynamic casters move,
restores that depth and redraws just them, so one walking resident among
thirty props draws one caster per face, not thirty-one, and the shadows
are the same. A static caster that moves, joins or leaves renders the static
depth once more. The cache is a depth array the size of the atlas, built
the first time a layer has a dynamic caster (`staticCacheBytes`). A
directional light's cascades, which follow the camera, and casters a
product shader moves with time redraw whole as before.

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
most 262,144 probes) on a world lattice, traces 64 rays from each through
the world's shown opaque triangles, and keeps what arrives as low-order
spherical harmonics (four RGB coefficients a probe). A ray that leaves the scene sees the sky:
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

- The volume is kept in 16 m bricks, each with its own tree over the
  triangles in its cell. The bake runs on a worker thread, never the render
  thread, one brick after another, and each brick uploads alone while the
  old probes keep drawing. A retained change rebakes only what it reaches:
  a part's move, mesh, visibility or material rebuilds its cells' trees and
  rebakes those cells and the ones around them; a point or spot light the
  cells within its range plus one; the sun, the sky, an ambient or
  hemisphere light, a material or a texture definition every brick. Bakes
  start once the scene has been still for a quarter second, so a world
  built over many frames bakes once; while something inside keeps moving
  every frame they start every two seconds instead. Moving the request's
  centre scrolls the volume: probes still covered keep their values, only
  the newly covered bricks (and a brick the old box only partly covered)
  bake, and until they do they draw the nearest old probe's value.
  `engine.renderer` reports `gpu.indirectLight`: probes, probes filled from
  neighbours, triangles in the trees, bricks and bricks pending, the last
  brick's and the slowest brick's milliseconds, the last batch's
  milliseconds and brick count, the bakes so far, the last frame's upload
  bytes, and the volume's GPU bytes. A 16 m brick at 2 m spacing (8³
  probes, 64 rays, two bounces) bakes in about 14 ms on one core in the
  lighting exploration's cave and dungeon, the slowest in 26 to 28 ms, and
  in 3 to 4 ms with half the machine's cores; at 1 m spacing a brick holds
  eight times the probes and takes about eight times as long. The first
  bake of a whole volume costs its bricks together plus their trees (the
  cave's 40 bricks in 118 ms on 10 threads, the dungeon's 32 in 229 ms with
  200 k triangles). Starting a bake while the sky's light is on reads its
  irradiance back from the device (144 bytes, a short wait on the render
  thread).
- Walls thinner than the spacing keep the light beyond them out. Each
  probe's bake also casts a ray along each axis to see whether a wall lies
  within the next spacing, and the shader moves its trilinear sample to its
  own side of a wall across its cell, by as much of the cell's face as the
  probes that met the wall cover there. A sample reads only probes on its
  side of a closed wall, still reads through an opening such as a door, and
  changes smoothly from cell to cell: a closed room with 1 m walls under
  2 m probes stays dark in daylight, and away from walls the volume reads as
  plain trilinear sampling does. On a software
  adapter the one-read encoding (below) keeps no walls, so there a wall
  thinner than the spacing still lets about a fifth of the light beyond it
  through: keep the spacing at or under such walls' thickness if that
  matters on software rendering.
- It is one volume. A product moves it with the player in a large world by
  re-requesting it at a new centre; keep the spacing, extent, bounces and
  ambient the same so the move scrolls instead of starting over.
- Per lit fragment the shader adds three trilinear reads of one small 3D
  texture (RGBA16F, 32 bytes a probe with its cell's walls), six for metals
  and under the sky's light, plus one unfiltered read of the cell's walls:
  0.01 to 0.05 ms of world pass at 1080p on an RX 9070 XT, the walls about
  0.02 ms of it. The walls cost each probe three short rays in the bake. On a
  software adapter (llvmpipe) each filtered read costs about a twentieth of
  a cheap world pass, so there the Engine uploads a one-texel encoding
  instead, the ambient coefficient in colour and the vertical coefficient's
  luminance (sky above, torch-lit floor below; no horizontal direction), and
  the shader reads it once: 4 to 5 percent of llvmpipe's world pass on the
  lighting exploration's cave and dungeon. `rusty-scene-render
  --indirect-light cx,cy,cz,ex,ey,ez,spacing,bounces[,floor]` bakes a volume
  before its frames and reports it under `gpu.indirectLight`.

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
`Describe()` returns the catalogue a product's own menu is built from, the
same one the Engine's [video options](#video-options) panel draws. Each
`RendererSettingOptionReadout` gives:
- the setting's id, label, group and description;
- its kind (`Toggle`, `Choice`, or `Range` with min, max, step and unit);
- its Engine default, requested and drawn values, as text (`true`, `4x`,
  `none`, `0.75`);
- its refusal;
- whether a change needs a restart (none does);
- its measured cost.

`Choices` lists each choice setting's values and labels by option id.
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

## Video options

A player chooses renderer settings in the game's options menu: the Engine's
video options panel
([the product project](csharp-product-project.md#video-options)), which
draws a catalogue of every setting this pair has. The player's choices apply
over the game's own: over the manifest at startup and over every
`RendererSettings.Set`, so a game that sets its settings again does not undo
them, and a choice the player forgets returns to the game's value. `Read()`
reports `Requested` with the player's choices in it. The choices are kept for
the install in `engine-video-options.json` under the persistence root, read
at startup; a game run without a persistence root keeps them for the session
only, and a damaged file is reported and ignored.

| Option | Group | Values |
| --- | --- | --- |
| `renderScale` | Display | 0.5 to 1 in steps of 0.05 |
| `antialiasing` | Display | `off`, `2x`, `4x` |
| `vsync` | Display | on or off |
| `shadows` | Quality | on or off |
| `shadowBudget` | Quality | `4`, `8`, `16`, `32`, `none` |
| `ambientOcclusion` | Lighting | `disabled`, `screenSpace`, `distanceField` |
| `ambientOcclusionStrength` | Lighting | 0 to 2 |
| `ambientOcclusionRadius` | Lighting | 0.25 to 2 m |
| `volumetricFog` | Lighting | `off`, `low`, `high` |
| `volumetricClouds` | Lighting | `off`, `low`, `high` |
| `clusteredLighting` | Advanced | on or off |
| `gpuCulling` | Advanced | on or off |

The presets Low, Medium, High and Ultra set render scale, antialiasing,
shadows, their budget, the occlusion mode, and volumetric fog and clouds
(off, off, low, high), over the player's other choices. A setting the device refuses is shown with the reason, from the
same refusals the readout carries. The panel's route,
`/__rusty/product/runtime/video-options`, answers `GET` with the catalogue
(each option's `value` drawn, `requested`, `gameDefault`, `chosen` and
`refused`, whether a change needs a `restart` (none does) and its measured
`cost`, which the panel shows under the description) and `POST` with one change: `{"choose": {"id", "value"}}`,
`{"forget": id}`, `{"forget": null}` for every choice, or
`{"preset": id}`. A scene snapshot records the player's choices, and
`rusty-scene-render` applies them, with its own feature flags as choices
over them.

A new renderer setting joins the catalogue in the commit that adds it:
its entry in `RENDERER_SETTING_OPTIONS`, its field and value mapping in
`RendererSettingsOverrides` and `renderer_setting_value`
(`render-model/src/settings_options.rs`), its `restart`, its measured
`cost` and its `gallery` experiment there too, and its refusal, if it has
one, in the runtime's video options. The panel and the feature gallery then
offer it to every game on that pair.

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

`RustyEngineProductAmbientOcclusionRadius` (0.75 m by default) is how far
it looks; the pass costs the same at any radius (about 0.24 ms at 720p on an
RX 9070 XT). A wider radius reaches past contact into the creases a world is
built from: in an overcast CraftSurvive meadow, 0.75 m darkens about 8% of
pixels by more than 8 levels, 1.25 m about 12% and 2 m about 14%, with no
halos. The default suits props and rooms; a world built on 1 m voxels or
pieces reads its corners better at 1 to 1.25 m.

A voxel session can instead, or as well, bake occlusion into its surfaces
at mesh time from the voxels around each vertex
([vertex occlusion](smooth-voxel-surfaces.md#vertex-occlusion)): no pass,
no view dependence, and it reaches into every corner the lattice knows,
at the scale of a voxel rather than of the screen.

What each path is for:

| Path | For | Not for |
| --- | --- | --- |
| Screen-space (`enabled`) | contact under anything drawn: props, foliage, characters, building pieces, terrain creases in view | occluders off screen; it darkens nothing a frame cannot see |
| Distance field (`distanceField`) | voxel worlds that want occlusion from geometry out of view, out to half a chunk | props and foliage, which have no field |
| Vertex occlusion (per voxel session) | voxel corners, crevices and cave walls at no per-frame cost, steady as the camera turns | anything that is not a voxel surface; coarse distant chunks |

They combine: vertex occlusion bakes the lattice's corners and a pass adds
the contact around what stands on them. For a voxel world the Engine
recommends screen-space occlusion at about a voxel's radius, with vertex
occlusion on the sessions whose corners carry the look (cubic interiors,
dungeons, built structures).

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
| Occlusion map | it is a GLB material with an occlusion texture, or `MaterialRequest.OcclusionMap` names one (opened with `TextureColorSpace.Linear`; its red channel scales the ambient, hemisphere and sky light the surface takes, by `OcclusionStrength`: 1, or 0, applies it fully) |
| ORM map | `MaterialRequest.OrmMap` names a packed occlusion, roughness, metalness texture (glTF's R, G, B; opened with `TextureColorSpace.Linear`; a material takes it or `OcclusionMap`, not both). Red is occlusion as above; green multiplies `Roughness` and blue `Metalness` per texel, so one texture mixes matte and glossy or metal and dielectric regions. It reads as the base texture does: the mesh uv or the triplanar planes, with the same repeats, offset and stochastic tiling |
| Triplanar | its `TriplanarSharpness` is nonzero ([three planes](smooth-voxel-surfaces.md#textures-on-reconstructed-surfaces)) |
| Flat shading | `MaterialRequest.FlatShading` is set (or a static mesh asset's material says `flatShading`): each triangle shades from its own plane, taken from the world position's screen derivatives and turned to face the mesh's normal, so a low-poly prop with welded smooth normals reads as faceted; its normal map is ignored. A GLB's authored materials are never flat by themselves: replace a slot's material with a flat one through `Animation.UpdateAnimatedMeshMaterials` or bind it on a static mesh |
| Stochastic tiling | its `StochasticTiling` is nonzero (three blended hex tiles per sample) |
| Water | `MaterialRequest.Water` (or an authored material's `Water`) has a depth scale, on a blended material: the surface is tinted by the depth of the scene behind it, foams along the shore, ripples and reflects at grazing angles ([water](#water)) |
| Wind | `MaterialRequest.WindBend` or `WindFlutter` is nonzero (or a static mesh asset's material has `wind`): the part sways in [the scene's wind](#wind) in the world and shadow passes alike. `WindBend` is how far each metre of a vertex's height above the part's origin leans with the wind at unit strength, in metres, so a trunk or stalk bends from its root; `WindFlutter` is how far a vertex circles at unit strength, in metres, times its colour's alpha, so leaves and grass tips flutter while their roots (alpha 0) hold; a mesh without vertex colours flutters whole. Masked leaves resolve anti-aliased under multisampling: a masked material covers its pixel's samples by its alpha sharpened about the cutoff, and masked sprites do the same |

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
- **Vertices.** A shader may define `fn displace(vertex: Vertex) ->
  vec3<f32>`, which the world and shadow passes' vertex stages call for the
  vertex's world position, so a banner waves, a flag ripples or a creature
  breathes in its image and its shadow alike. `Vertex` holds the world
  position and normal as the part's transform (and the wind feature, when
  the material has it) placed them, the mesh's own position and normal, the
  uv, the vertex colour, the part's origin in the world and its part index.
  It may read `frame.time` and the scene's wind (`frame.wind`: xy its unit
  direction over the ground, z its strength, w its gust share), and
  `rusty::wind::wind_displace` is the wind feature's own sway. While a
  displacing material draws, the shadow maps redraw as presentation time
  moves, as they do for a caster stage. The standard stages still cull by
  the mesh's bounds, so keep a displacement within a metre or so of them.
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

## Water

A blended material with `MaterialRequest.Water` set (`MaterialWater`; an
authored voxel material takes the same on
`AuthoredMaterialAppearanceRequest.Water`) draws as a water surface. The
blend pass copies the opaque pass's depth before the surface draws, so each
pixel knows what lies behind it along its view ray:

- **Depth tint.** The view through the water turns from `ShallowColor` to
  `DeepColor` (linear RGB, multiplied by the material's colour and texture)
  by `e` every `DepthScale` metres of water along the ray, and turns opaque
  with it, so a bed shows through the shallows and a lake reads deep.
- **Foam.** Where the scene lies within `ShorelineWidth` metres below the
  surface (a bank, a post, a swimmer) the surface foams white where
  `FoamTexture` (any texture, read as a mask and scrolled by `FoamScroll`
  repeats per second) exceeds `FoamThreshold` (0 to 1); without a texture,
  bands roll in toward the shore.
- **Ripples.** The material's normal map, scrolled by `NormalScrollA`, and
  `RippleTexture` as a second normal map (opened with
  `TextureColorSpace.Linear`) scrolled by `NormalScrollB`, both read over the
  ground every `WaveScale` metres and scaled by the normal map's
  `NormalScale`, ripple the surface; without either it ripples procedurally.
  The two water textures take a product shader's two texture slots. The sun's specular and, with [the sky's light](#the-skys-light),
  the sky's reflection follow the ripples.
- **Fresnel.** The alpha rises toward the reflectance at grazing angles, so
  a lake seen along its length mirrors the sky while the shallows under the
  eye stay clear. Water is a dielectric (metalness 0) that still reflects
  the ambient and hemisphere light along the reflection, weighted by the
  same Fresnel term metals use, so it mirrors its surroundings without the
  sky's light; with the sky's light on, the prefiltered sky takes over as
  for every surface.

The water needs no vertex motion; a product that wants waves gives the
material a [displace stage](#product-shaders) or a wind bend. A water view
costs one depth copy per view that draws water, and the blended parts and
particles draw in a second pass after it; views without water draw as
before. Blended materials cast no shadow unless `TranslucentShadow` is
set, so a lake does not shadow its own bed.

```csharp
Material lake = engine.Graphics.CreateMaterial(new MaterialRequest(new Color(1, 1, 1, 0.35f), default, 0.08f, white, Vector3.Zero, 0, true, MaterialAlphaMode.Blend, 0) with
{
    NormalMap = ripples, NormalScale = 0.6f,
    Water = new MaterialWater(new Color(0.18f, 0.55f, 0.5f, 1), new Color(0.01f, 0.08f, 0.2f, 1), DepthScale: 1.8f, ShorelineWidth: 0.35f, FoamThreshold: 0.55f,
        FoamScroll: new Vector2(0.03f, 0.02f), NormalScrollA: new Vector2(0.04f, 0.03f), NormalScrollB: new Vector2(-0.02f, 0.035f), WaveScale: 3f,
        FoamTexture: foam, RippleTexture: ripples),
});
```

## Wind

`engine.CameraView.SetWind(new(Direction: new Vector2(1, 0.3f), Strength: 1.5f,
Gust: 0.6f))` blows a wind over the scene: materials with a wind bend or
flutter ([the standard shader](#the-standard-shader)) sway in it, and
product shaders read it in a displace stage ([product
shaders](#product-shaders)). `Direction` is over the ground (world x, z;
any length); `Strength` (0 to 16) scales every material's bend and flutter,
and 0 stills the scene (the default, and `SetWind` with strength 0); `Gust`
(0 to 1) is the share of the lean that rises and falls in slow gusts rather
than holding steady. The sway moves with presentation time, so it holds
while the simulation is paused and costs nothing per frame on the C# side;
each part's lean is phased by where it stands, so neighbouring trees differ,
and each vertex's flutter by where it is. The wind is retained with the
other environment settings and survives a runtime restart. A swaying
material's shadow sways too: while one draws, the shadow maps redraw every
frame, so give the wind feature to trees, grass and banners rather than to
whole terrains.

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
world view's `world`, `particles` (soft sprites and billboards, in the frames that draw them), `bloom-exposure` and `finish` passes
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

## Clouds

A cloud layer drifts over the sky panorama, lit by the sun:

```csharp
engine.CameraView.SetClouds(new(Coverage: .6f, Drift: new Vector2(8, 3),
    Altitude: 1200, Scale: 500, Color: Vector3.One));
```

- `Coverage` (0 to 1) is how much of the sky is cloud. 0 clears the layer,
  draws no `clouds` pass and leaves the sky exactly as without it (the
  default).
- `Drift` is the layer's velocity over the ground (world x, z) in metres per
  second, at most 1000. Pass the [wind](#wind)'s direction to have clouds
  follow it. Clouds move with presentation time, so they hold while the
  simulation is paused.
- `Altitude` and `Scale` (above 0, at most 100 km) are the layer's height
  and the size of one cloud, in metres. `Altitude` is above the world's
  zero, not above the camera. The view ray meets a plane at that height, so
  clouds look larger overhead and shrink toward the horizon, where they
  fade into the panorama. A camera at or above the altitude sees no flat
  layer overhead.
- `Color` (linear, 0 to 16 a channel) tints the light the clouds take.
- The clouds take the brightest directional light's colour and direction,
  less where more cloud lies between them and the sun, and brighter at
  their thin edges toward it. They also take a greyed share of the
  panorama's colour behind them. Moving the sun and blending the
  panoramas with the product clock ([below](#blend-authored-time-of-day-skies))
  warms them at dusk and darkens them at night with no other call.
- The layer draws over a sky panorama (or a blend), after the sun's disc,
  which it covers. Over a clear colour it draws nothing.
- It shades the ground. Every directional light (sun or moon) is met by the
  layer where the ray toward it crosses the layer's altitude, and a cloud
  there lets through a fifth of the light, specular included. The shade
  takes the sky's own noise along that ray, so from any height the cloud
  seen in front of the sun is the cloud that shades you (a thin wisp of
  the sky's finest detail casts none). Under a broken
  sky the shade lies in patches that drift with `Drift`. Under a full
  overcast every point is shaded, so the sky's light and the fills carry
  more of the scene. A light at or below the horizon is not shaded.
- A heavy layer greys the panorama behind it: with the square of the
  coverage the sky loses up to 60 % of its colour toward a slightly darker
  grey. Coverage 0 leaves the sky exactly as without clouds.
- The [sky's light](#the-skys-light) and reflections do not see the layer:
  they still take the panorama's own colours.
- The layer is retained camera-view state like the wind, and survives a
  runtime restart.
- `engine.renderer` times it as the `clouds` pass, in the frames that draw
  it. It is one pass over the screen: two four-octave noise samples and two
  panorama reads a pixel. At 1920×1080 it takes about 0.36 ms on an RX 9070
  XT and 15 to 18 ms on llvmpipe.
  The shade on the ground is drawn in the world pass instead: a two-octave
  sample per lit fragment facing a directional light, only while the layer
  has coverage. On a CraftSurvive meadow at 1920×1080 it added about 0.2 ms
  to the world pass on an RX 9070 XT.

### Cloud regions

A cloud region is more cloud in one place: a weather front's storm over the
plain, a squall the player can see coming.

```csharp
engine.CameraView.SetCloudRegion(new(Id: 1, Center: new(4000, -2500), Radius: 3000,
    Coverage: .95f, Darkness: .6f, Drift: new(6, 2)));
engine.CameraView.RemoveCloudRegion(new(1));
```

- Within two thirds of its `Radius` (metres) of `Center` (world x, z) the sky
  holds at least the region's `Coverage`; past that it fades to the layer's
  own. It drifts at `Drift` metres per second from where it was placed, so
  move it by placing it again. `Darkness` (0 to 1) darkens its clouds'
  undersides and lets even less sun through them.
- The flat layer, the volumetric clouds and the shade on the ground all read
  it, so a storm overhead shades the ground under it. A region needs no layer
  coverage: with none set, it draws under a default layer 1500 m up with
  600 m clouds. At most 32 regions; they are retained camera-view state.

### Volumetric clouds

With the renderer's `VolumetricClouds` setting `Low` or `High`
(`RustyEngineProductVolumetricClouds`, `RendererSettings`, the player's
[video options](#video-options)), the cloud layer is raymarched through a
slab from its altitude up by six tenths of it, instead of drawn as a sheet:
billows of 3D noise at the cloud size, kept where they rise above the sky's
coverage (the layer's, raised by its regions), with flat bottoms thinning
toward the top. Each sample is lit by the sun through the cloud between it and
the sun (Beer's law with a multiple-scattering term, a forward lobe toward the
sun and darker edges), and by the panorama behind it; a region's darkness
darkens it. The ground's shade samples the same density a third of the way
up the slab, so a cloud and its shadow agree.

- `Low` takes 12 steps a ray and `High` 24, with two steps toward the sun; a
  per-pixel offset turns the steps' banding into fine noise. The clouds reach
  30 km and fade toward the horizon as the flat layer does.
- It is timed as the `clouds` pass. On an RX 9070 XT at 1280×720 a view
  filled with sky costs about 1.0 ms (`Low`) and 1.9 ms (`High`); a
  CraftSurvive view with a strip of sky about 0.2 to 0.5 ms. Off draws the
  flat layer exactly as before; a software adapter refuses it
  (`SoftwareAdapter`) and draws the flat layer.
- Seen low across the sky, rays cross many clouds, so a given coverage looks
  fuller than the flat layer's. There is no temporal filtering, and the
  sky's light and reflections still see only the panorama.

## Wet surfaces

After rain, the scene's lit surfaces can read as wet:

```csharp
engine.CameraView.SetWetness(new(Wetness: .8f, Puddles: .5f));
```

- `Wetness` (0 to 1) darkens a surface's diffuse colour toward 55 % (metals
  keep theirs) and lowers its roughness toward 0.15, so the sun and sky
  glint off it. Up-facing surfaces take the full wetness, walls half of it,
  and undersides none.
- Only surfaces under the open sky get wet. Where an ambient light has a sky
  layer ([the sky's occlusion](#the-skys-light)), what it shades (the ground
  under a roof, a cave) stays dry. Without such a light every surface counts
  as open.
- `Puddles` (0 to 1) gathers standing water in patches about 2.5 m across on
  flat ground, as much as the surface is wet: darker still, nearly smooth
  and flat.
- A material can keep dry: `new MaterialRequest(...) with { KeepDry = true }`
  shades it as on a dry day beside surfaces that wet, such as a sheltered
  sign, an awning's underside or a waxed hull. Its product shader, if any,
  receives the dry surface.
- It applies to standard-lit world materials before their shade stage, so
  a product `shade(Surface)` receives the wetted surface. Water and unlit
  materials, sprites and particles are left as they are. Product WGSL reads
  the values as `frame.weather.x` (wetness) and `.y` (puddles).
- 0 (the default) draws exactly as dry. The setting is retained camera-view
  state like the wind. It costs nothing while 0; while wet, each lit
  fragment reads the sky layer once more.

## Precipitation

Rain or snow falls around the camera:

```csharp
engine.CameraView.SetPrecipitation(new(Drops: 30_000, PrecipitationShape.Streak,
    Velocity: new(1.5f, -14, 0.5f), Size: .03f, StreakSeconds: .05f,
    Color: new(1.6f, 1.7f, 1.9f, .6f), Additive: false, Radius: 16, Height: 10));
```

- `Drops` (at most 200,000; 0 stops it) fall at `Velocity` (world metres per
  second, wind included) through a box `Radius` metres to each side of the
  camera and `Height` metres above and below it. The box is tiled across the
  world and wraps as the camera moves, so the drops stay put in the world and
  the density stays even. Set `Drops` each update to change how hard it falls.
- `PrecipitationShape.Streak` draws thin streaks as long as a drop travels in
  `StreakSeconds`, stretched along the velocity (rain).
  `PrecipitationShape.Flake` draws round flakes facing the camera (snow, ash,
  glitter). `Size` is a drop's width in metres.
- `Color` is linear radiance and alpha: scale it with the time of day, as the
  drops take no light of their own. `Additive` adds it to the frame (glints),
  otherwise it blends over it.
- No drop falls where an ambient light's sky layer says the sky is closed
  overhead, so it does not rain under a roof or in a cave. Drops hide behind
  the world and fade softly into it, and fade out toward the box's sides.
- Nothing is simulated: each drop is one instanced quad placed by its index
  and the presentation time, so it holds while the simulation is paused. It
  draws in its own pass after the world, timed as `precipitation`.

## Volumetric fog

Fog that light passes through: shafts where the sun reaches past a wall or a
tree, a torch's glow in a misty cave, a valley bank, a wall of dust. It draws
while the renderer's `VolumetricFog` setting is `Low` or `High`
(`RustyEngineProductVolumetricFog`, `RendererSettings`, or the player's
[video options](#video-options)). Within the medium's reach (`Distance`)
it replaces the analytic distance fog (`SetFog`), which begins only where it
ends, so no surface is fogged by both; the analytic fog keeps fogging the
far view and the backdrop beyond. Refused or off, the analytic fog fogs the
whole view as before.

```csharp
engine.CameraView.SetVolumetricFog(new(Density: .02f, Albedo: new(.9f, .9f, .92f),
    Anisotropy: .6f, BaseHeight: 0, FalloffHeight: 25, Distance: 96, Ambient: 1));
engine.CameraView.SetFogVolume(new(Id: 1, FogVolumeShape.Ellipsoid, Center: new(40, 3, -20),
    HalfExtents: new(30, 6, 30), YawDegrees: 0, Density: .15f, Albedo: new(.85f, .8f, .7f),
    Emission: Vector3.Zero, Edge: .4f, NoiseScale: 6, NoiseStrength: .6f,
    NoiseVelocity: new(2, 0, 1)));
engine.CameraView.RemoveFogVolume(new(1));
```

- **The medium.** `SetVolumetricFog` fills the air: `Density` (0 to 1,
  extinction per metre at `BaseHeight`; 0 fills none, so only fog volumes
  draw) thinning by e every `FalloffHeight` metres up (0: one density
  everywhere); `Albedo` the colour it scatters; `Anisotropy` (-0.9 to 0.9)
  scatters forward when positive, so fog glows looking toward a light;
  `Distance` (8 to 1000 m, 96 by default) how far the fog grid reaches;
  `Ambient` (0 to 4) how much ambient and sky light it scatters.
- **Fog volumes.** `SetFogVolume` places or replaces a box or ellipsoid by
  `Id` (at most 64): `Density` (0 to 4) at its heart, fading to nothing over
  `Edge` of its half extent; `Emission` light it gives off per unit of
  density (a glowing storm, an arcane haze); 3D noise cells `NoiseScale`
  metres across thin it by `NoiseStrength` as they drift at `NoiseVelocity`.
  Positions are in the renderer's world space, as an indirect light
  volume's. `RemoveFogVolume` takes one away.
- **The light.** Every light of the view lights it through its shadows
  (sun cascades, point and spot shadows), the sun and moon through the cloud
  layer, plus the ambient and hemisphere lights and the sky's light. Light
  shafts come from the shadows: fog in a wall's shadow stays dark.
- **How it draws.** A froxel grid over each world view (`Low` 96×54×48 cells,
  `High` 160×90×64, spaced quadratically in distance out to `Distance`) is
  lit and integrated front to back in two compute dispatches, timed as
  `volumetric-fog`; the finish pass dims each surface by the fog in front of
  it and adds the light it scatters, before exposure, and the background is
  seen through the grid's whole depth. A surface reads the grid a cell nearer
  than itself, so fog behind it does not leak through. On an RX 9070 XT at
  1280×720 over a CraftSurvive meadow it costs about 0.11 ms (`Low`) and
  0.25 ms (`High`). There is no temporal filtering: a thin shaft narrower
  than a cell is blurred to the cell.
- It needs compute shaders on a GPU: a device without them, or a software
  adapter, refuses the setting (`NoComputeShaders`, `SoftwareAdapter`) and
  draws the analytic fog alone. Off, or on with no medium and no volumes, it
  draws exactly as without it. The medium and the volumes are retained
  camera-view state, kept by a rebaseline and recorded by a scene snapshot.

## The backdrop

A second scene can stand behind the world, drawn by a camera linked to each
world view: distant ranges on the skyline at the scale of a world map, or a
miniature of space behind a ship. This is the multi-camera "3D skybox".
Appearances placed in the `Backdrop` render layer, and voxel scene
presentations moved there, stand in **backdrop units**. The product links
the layer to the world's cameras:

```csharp
engine.CameraView.SetBackdrop(new(Anchor: Vector3.Zero, Origin: Vector3.Zero, Scale: 1000));
engine.Graphics.PublishSnapshot([new AppearanceFact(id, false, 0, at, ranges, true, RenderLayer.Backdrop)]);
engine.VoxelScenePresentation.SetLayer(new(mapTerrain, RenderLayer.Backdrop));
```

**The link**
- `Scale` is world metres per backdrop unit: 1 draws the backdrop at world
  scale (a far field beyond the world's far plane), 1000 a 1:1000 miniature.
- `Anchor` is a world point in the cameras' frame, and `Origin` is where it
  lies in the backdrop. The product moves either whenever it wants, for
  example to recentre on a region.
- Each world view draws the backdrop with its own rotation and field of view
  from `Origin + (eye − Anchor) / Scale`, so walking 100 m at 1:1000 moves
  the backdrop camera 0.1 units, and turning turns both alike.
- On a world-origin rebase, move the anchor by the receipt's `LocalDelta` as
  you move the camera; the backdrop then stays where it was.
- `ClearBackdrop` unlinks it.

**Drawing**
- The backdrop draws over the sky, the sun and the clouds and before the
  world. The world then clears depth and draws over it, so world geometry
  always covers it, however near the backdrop's own depth is.
- Its camera's depth range is fitted to what the backdrop shows. The
  world's depth precision is unchanged, and nearer backdrop than a
  ten-thousandth of its far plane is clipped (the world covers it).
- It is lit by the world's ambient, hemisphere and directional lights, and
  by the sky's light, without their shadows. Lights placed in the backdrop
  light only the backdrop.
- Distance fog, height fog and the atmosphere's haze are reckoned at the
  backdrop's **world-equivalent** distance and height, and the cloud layer
  shades it where it would stand in the world. A backdrop range 5 km out at
  1:1000 therefore draws as the same range would at world scale
  (`tests/backdrop.rs` checks this pixel for pixel across turns and walks),
  and fog matches at the join.
- Volumetric fog over the world covers the backdrop, as it covers the
  background.
- Static meshes, animated meshes, sprites and voxel presentations can stand
  in it, opaque, masked, blended and water alike. Particles stay the
  world's. The backdrop casts no shadows, is never pickable, and adds
  nothing to collision. A voxel presentation in it draws at full resolution
  and grows no scatters (its viewer stands in the world). Its session still
  owns its collision, apart from the walking session's.

**Cost**
- Without a link, or with nothing shown in the layer, nothing is drawn and
  nothing changes.
- With one it is a pass of its own, timed as `backdrop`: the backdrop's
  parts listed on the CPU, drawn and finished. The world view's background
  is submitted before it. On an RX 9070 XT at 1280×720 the fixture's ranges
  (a 64×64 heightfield) and its dual-contoured mesa cost about 0.05 ms.
- Perspective views only: an orthographic view draws no backdrop. Neither
  the sky's light nor reflections see it.

## Image effects

A product can run its own WGSL over each primary view's finished picture:
raindrops on the view, a lightning flash, a shimmer or blur.

```csharp
var shader = engine.Graphics.OpenResource(new RenderResourceRequest("flash.wgsl")).Handle;
engine.CameraView.SetImageEffect(new ImageEffectRequest(shader, new Vector4(flash, shimmer, 0, 0)));
```

The `.wgsl` defines `fn image_effect(pixel: ImagePixel) -> vec4<f32>` and
imports what it needs from `rusty::image`:

- `ImagePixel` carries `uv` (0 to 1 across and down the picture), `color`
  (the finished colour there, alpha its coverage), `depth` (the world's
  depth there, 0 at the near plane, 1 at the far plane and over the sky)
  and `time` (the presentation time in seconds, modulo a day).
- `picture_at(uv)` samples the finished picture anywhere, filtered, so an
  effect can refract, shift or blur it. `picture_size()` is its size in
  pixels.
- `effect_parameter(0..3)` reads `Parameter0` to `Parameter3`, and
  `effect_texture_a(uv)` / `effect_texture_b(uv)` read `TextureA` and
  `TextureB` (white when unset): a droplet normal map, a noise.

The result replaces the pixel. The picture is finished first (fog, grade,
tone mapping, bloom), so the effect works in display values, and the
product UI draws over it. The picture is the render scale's when there is
one, and the effect upscales it.

While an effect is set, each primary view draws into a picture of its own,
and one full-screen pass runs the effect into the view, timed as
`image-effect`. `Shader` handle 0 removes the effect, and the views draw
straight into the target as before. A shader that does not compose is
refused when it is opened, with its file and line. A shader may define both
`shade` and `image_effect`.

## Fixture

`fixtures/csharp-lighting-sky` uses the packaged SDK, a voxel room, a retained
torch light, a persistence round-trip and two deterministic authored panoramas.
Commands: `lighting.inspect`, `lighting.torch true|false`, `lighting.sky 0..1`,
`lighting.room`, `lighting.cave` (from the back wall toward the doorway),
`lighting.indirect sky|floor|off` (the probe volume over the room),
`lighting.dig <x> <y> <z>` (clear one of the room's voxels: its chunk
re-meshes and only the probe bricks around it rebake),
`lighting.follow <x> <y> <z>` (move the volume's centre),
`lighting.fog <density>` (0 turns it off),
`lighting.exposure <exposure>` (ACES filmic), `lighting.atmosphere true|false`
(height fog, sun haze, disc and halo), `lighting.panorama` and
`lighting.torch.flame true|false` (a fire by the wall: a soft additive flame
flipbook, soft additive embers and soft alpha smoke, with the camera on it;
`lighting.torch.softness 0` gives them hard depth edges again and 1 the
authored softness; [particle bursts](csharp-lifecycle.md#particle-bursts)
describes the two options), `lighting.facets true|false` (a welded
low-poly sphere and the fixture's low-poly tree twice: their authored smooth
normals on the left, the flat-shading material on the right, by the torch),
and `lighting.wind <strength>` (the tree under a material with a wind bend,
a clump of grass cards whose vertex alpha weights their flutter, and a
banner a product displace stage (`content/wave.wgsl`) waves, with the
camera on them; 0 stills the wind, below 0 clears the scene),
`lighting.water 1|0|-1` (a dual-contoured sand bank sloping into a water
slab with a cube pier, seen from the bank under the sun: 1 the water
feature with the fixture's foam and ripple textures, 0 the same slab as a
plain blended material, below 0 clears the scene), and
`lighting.vertexocclusion <strength>` (the room's cube vertices darkened by
the voxels around them, with the camera on the room's far corner), and
`lighting.flash <strength> <shimmer>` (an image effect from
`content/flash.wgsl`),
`lighting.precipitation <drops> <shape>` (rain, shape 0, or snow, shape 1,
around the camera; none in the room under its roof),
`lighting.wetness <wetness> <puddles>` (the room's surfaces wet after rain),
`lighting.clouds <coverage>` (a drifting cloud layer over the panorama, with
the camera up toward it; 0 clears it, and `lighting.sky` moves the sun that
lights it from noon to dusk),
`lighting.backdrop <scale>` (ranges ringing the room 1.5 to 6 km out, a
heightfield mesh, and a dual-contoured mesa, drawn behind the world at
1:`scale` and seen out of the doorway; 0 removes them).
`generate-particles.py` regenerates its three authored sprites and
`generate-water.py` the foam and ripple textures.
`lighting.sky` also moves the fixture's sun from noon at 0 to a low dusk sun
at 1. Debug selection is explicit fixture
assistance; no downstream gameplay acceptance is implied.
