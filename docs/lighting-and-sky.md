# Voxel lighting and product-driven skies

Use retained `Graphics` lights for cave and torch illumination. They already
support point/spot attenuation, directional light, and requested shadows. Set
`RustyEngineProductDefaultWorldLights` to `disabled` for a dark unlit world;
the default neutral rig otherwise continues to illuminate it. Emissive material
color makes a surface visible but does not emit light onto other surfaces. Pair
a torch's emissive appearance with a retained point light when it should light
its surroundings. Updating or disabling that light updates the existing owner.

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
lights use their opposite travel direction. Ambient light is unoccluded, so it
cannot make sealed caves dark. Disabled/zero-intensity and out-of-range lights
contribute nothing. `ShadowIntent.Requested` enables a ray against the current
voxel and retained static-mesh **collision** projection. This is an explicit
CPU lighting proxy, not GPU shadow-map or final-pixel readback: non-collidable
visual occluders, active entities, translucent shadow transmission, indirect
bounce, tone mapping and material response are not included. Keep the relevant
opaque cave geometry collision-resident. Unloaded geometry cannot occlude.

This direct-light alternative provides dark enclosed rooms, local gradients,
and product-readable light at addresses. It does not store or propagate a
Minecraft-style sky/block-light lattice, or supply indirect light around corners.
The visual path remains the existing renderer's ordinary lights and shadows.

### Persistence and cost

Persist the source light descriptors alongside `Voxel.ExportHistory` in the
product's save envelope. On load, use `Voxel.RestoreHistory`, restore the same
descriptors through `Graphics`, then resample. Handles and derived light samples
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
crossfade. A blend is not a physical atmosphere simulation or fog control.
Update ordinary scene lights from the same product clock when illumination
should change too; sky presentation does not create environment lighting.

The Engine retains both texture dependencies and one sky shader/geometry.
Changing the amount updates uniforms only, with no texture upload, mesh rebuild,
or new renderer resource. Cost is two panorama samples per visible sky pixel.
Different panorama resolutions are supported. The blend survives a fresh host
attachment through the ordinary retained presentation baseline.

`SetSkyBackground(texture)` still selects the original single-panorama path;
`SetBackgroundColor` selects an opaque clear color; `ClearSkyBackground` returns
to the Engine default. Selecting one replaces the other. Both selected blend
textures stay live until the selection changes; clear the sky before releasing
them. The Engine owns GPU lifetime and panorama orientation.

## Fixture

`fixtures/csharp-lighting-sky` uses the packaged SDK, a voxel room, a retained
torch light, a persistence round-trip and two deterministic authored panoramas.
Commands: `lighting.inspect`, `lighting.torch true|false`, `lighting.sky 0..1`,
`lighting.room`, and `lighting.panorama`. Debug selection is explicit fixture
assistance; no downstream gameplay acceptance is implied.
