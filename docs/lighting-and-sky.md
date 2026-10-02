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
  player (or camera) in a large world, as for a directional light's shadow.
  Outside the square nothing is occluded.
- It is one more shadow layer: every shown part casts into it, and it
  re-renders only when a light or part changes. Each lit fragment takes 25
  more comparison samples. Meshing is unchanged.
- It is presentation only. `Voxel.SampleDirectLighting` (below) still treats
  ambient light as unoccluded.

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

Materials with the same features share pipelines and batch together. A new
feature set compiles once, when its first material is defined: about 8 ms on
an RX 9070 XT, then about 0.4 ms for each further pass it draws in. Doom's E1M1
uses two. The shader is composed from importable WGSL modules in
`render-wgpu/src/shaders/` (`types`, `view`, `material`, `surface`,
`lighting`, `tonemap`, `finish`).

## Exposure, tone mapping and fog

Everything drawn in the world (lit and unlit meshes, voxel surfaces, GLB
parts, sprites and particles) ends in one finish step: exposure, then the tone
mapping operator, then distance fog. The background, a clear colour or sky
panorama, is never finished. Both settings are retained camera-view state,
like the sky, and products change them at runtime, for a cave, underwater or
at night:

```csharp
engine.CameraView.SetToneMapping(new(ToneMappingOperator.AcesFilmic, exposure));
engine.CameraView.SetFog(new(FogMode.Linear, fogColor, Start: 3, End: 30, Density: 0));
engine.CameraView.SetFog(new(FogMode.Off, default, 0, 0, 0));
```

- **Exposure** multiplies lit colour; it defaults to 1.
- **Operators.** `None` (the default) clamps at the target's range, so bright
  lights clip to white or a saturated primary. `Neutral` (Khronos PBR Neutral)
  keeps base colours true and compresses only highlights. `AcesFilmic` adds
  film-like contrast and rolls highlights toward white. Choose one before
  tuning light intensities: the same lights read differently under each.
- **Fog.** `Linear` has none before `start` and is full at `end` (metres from
  the camera). `Exponential` leaves `exp(-density × distance)` of the colour,
  and `ExponentialSquared` leaves `exp(-(density × distance)²)`, which is
  clearer near and denser far. The colour is linear RGB; alpha is ignored.
  Fog blends after tone mapping, so a fog colour equal to the background
  colour fades distant geometry exactly into it. Over a sky panorama, pick a
  colour that matches its horizon.
- **Captures.** `RenderOutput.CaptureImage` uses its request's own exposure
  and tone mapping, and keeps the scene's fog as it keeps the scene's lights,
  whichever background it selects.

Changing either setting recompiles nothing: both are values in the frame
uniform, so a product may update them every frame. Blended surfaces are
finished before they blend, as three.js does, so there is no bloom; that
needs an HDR intermediate the renderer does not have. Fog and tone mapping do
not change `Voxel.SampleDirectLighting`, which reports linear light.

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
