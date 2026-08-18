# Ghost-Plate Relief Rendering for `rusty-engine`

## Status

Experimental rendering design for interactive actors and objects in a voxel-first-person world.

The technique uses an ordinary animated 3D model as its source, resolves that model into a low-resolution “FMV plate,” then deforms the original mesh into a shallow view-dependent relief carrying that plate.

The environment remains conventional voxel geometry and owns the full-screen perspective, depth, occlusion, and optic flow. Only actor-sized regions commit crimes against projective geometry.

## Core visual thesis

> Photograph the model, crush the model into the shape of that photograph, paste the photograph back onto it, then allow the player to move slightly around the resulting haunted plate.

The visible actor is not intended to behave like a correct 3D model. It is intended to behave like a directional sprite that has acquired just enough depth to occupy the world.

This creates a useful gameplay grammar:

* Voxel geometry represents settled world matter.
* Ghost-plate entities represent actors, loot, chests, interactable objects, and other causally privileged things.
* Large environmental objects can remain voxel geometry while only their interactive component uses the ghost-plate material.

The discontinuity is intentional. It should be made coherent and recognizable rather than hidden.

---

# 1. Representation overview

Each ghost actor has four related representations.

## 1.1 Canonical actor

The ordinary model and animation state used for:

* gameplay animation;
* collision and hitboxes;
* navigation;
* IK;
* physics;
* authoritative pose;
* shadow casting;
* optional AO or world-light probes.

This representation is not drawn into the main color pass.

## 1.2 Held appearance actor

A second visual pose that updates at an intentionally limited cadence, such as 8 to 15 Hz.

It owns:

* the pose photographed into the current plate;
* the visible relief mesh;
* the stepped FMV-like performance;
* source-view geometry correspondence.

Its root transform can move smoothly every display frame even while its local skeletal pose remains held.

## 1.3 Ghost plate

A low-resolution offscreen render of the held appearance actor from a quantized actor-relative camera.

Initial suggested resolution:

```text
96 × 128
```

Possible later resolutions:

```text
64 × 96      distant actors
96 × 128     normal gameplay
128 × 160    close actors
```

The plate may contain:

* final stylized color;
* alpha or coverage;
* source depth;
* optional source normal;
* optional semantic region ID;
* optional stable surface or object ID.

## 1.4 Visible relief

The held appearance mesh is deformed along rays from the ghost camera so its depth is compressed while its projection from that ghost camera remains unchanged.

The plate is then projected back onto this deformed mesh.

From the ghost camera’s source angle, the result should reconstruct the plate almost exactly. From nearby angles, it exhibits shallow parallax and limited disocclusion.

---

# 2. Architectural ownership split

The clean boundary is:

> Application and renderer coordination decide which depiction exists. GPU shaders decide how that depiction lies about geometry.

For `rusty-engine`, this is best divided into three layers.

| Layer                         | Responsibilities                                                                                                                                                        |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust engine                   | Authoritative actor state, root transform, animation identity/time, visual-style component, LOD eligibility, lifecycle                                                  |
| Three.js renderer coordinator | View-sector selection, hysteresis, appearance sampling, pose snapshots, capture scheduling, render-target pooling, ghost-camera matrices, transitions                   |
| GPU shaders                   | Skinning integration, ray-preserving depth compression, source projection, plate sampling, source-depth rejection, optional depth banding, dither and world integration |

## 2.1 Rust engine responsibilities

Rust should own declarative intent, not per-view rendering details.

Suggested component shape:

```rust
pub struct GhostPlateStyle {
    pub enabled: bool,

    pub plate_width: u16,
    pub plate_height: u16,

    pub azimuth_sectors: u16,
    pub elevation_bands: u8,
    pub sector_hysteresis_degrees: f32,

    pub appearance_hz: f32,
    pub depth_retention: f32,

    pub capture_fov_y_degrees: f32,
    pub capture_distance: f32,
    pub capture_height_offset: f32,

    pub shell_mode: GhostShellMode,
    pub transition_mode: GhostTransitionMode,

    pub style_profile_id: String,
}
```

Possible enums:

```rust
pub enum GhostShellMode {
    WholeMesh,
    SourceVisibleOnly,
}

pub enum GhostTransitionMode {
    HardCut,
    GeometryMorph,
    PlateDither,
}
```

Rust should send the renderer:

* actor identity;
* model or visual-archetype identity;
* current root transform;
* animation clip/state and time, or a bone-palette handle;
* current morph-target values if used;
* the `GhostPlateStyle`;
* visibility and LOD information;
* equipment or material variation identifiers.

Rust should not receive:

* plate pixels;
* depth textures;
* warped vertex positions;
* source visibility results;
* GPU timing details unless exposed as renderer telemetry.

No GPU readback is required.

## 2.2 Three.js renderer responsibilities

View-sector selection is visual and camera-dependent, so it should remain inside the renderer rather than becoming gameplay state.

Suggested renderer-side state:

```ts
interface GhostVisualState {
  actorId: string;

  physicalRoot: THREE.Object3D;
  appearanceRoot: THREE.Object3D;
  displayMeshes: THREE.SkinnedMesh[];

  currentSector: ViewSector;
  pendingSector: ViewSector | null;

  heldAnimationTime: number;
  nextAppearanceSampleTime: number;

  currentPlate: GhostPlateHandle;
  previousPlate: GhostPlateHandle | null;

  ghostCameraLocal: THREE.Matrix4;
  ghostViewWorld: THREE.Matrix4;
  ghostViewWorldInverse: THREE.Matrix4;
  ghostProjection: THREE.Matrix4;

  anchorDepth: number;
  depthRetention: number;

  transitionProgress: number;
  dirtyFlags: GhostDirtyFlags;
}
```

The renderer coordinator owns:

* deciding when a plate is stale;
* evaluating the actor-relative camera angle;
* quantizing that angle into a view sector;
* applying angular hysteresis;
* selecting the held animation sample;
* updating the held appearance rig;
* scheduling the capture;
* allocating and releasing plate textures;
* feeding matrices and parameters to materials;
* deciding which actors receive capture budget each frame.

## 2.3 Shader responsibilities

The shader should receive already-decided state:

* held appearance pose;
* ghost view and inverse view matrices;
* ghost projection matrix;
* real camera matrices;
* plate texture;
* source depth texture;
* anchor depth;
* global and semantic depth retention;
* transition parameters.

The shader should not decide:

* which sector is active;
* when animation samples advance;
* when capture occurs;
* whether an actor is important;
* texture lifetime;
* update budgeting;
* savegame state.

---

# 3. Recommended object layout

## 3.1 Canonical physical rig

The canonical rig advances at normal simulation or display cadence.

It can be used as:

* the authoritative actor pose;
* the collision source;
* the shadow caster;
* the source for copying appearance samples.

It should not write into the main camera color or depth pass.

Do not simply set the canonical actor’s `visible` flag to `false` if the standard Three.js shadow system must still render it. Instead, keep it in a shadow-only path or use a material that writes neither color nor main-camera depth while remaining eligible for shadow rendering.

## 3.2 Held appearance rig

The appearance rig has an independent skeleton or independent bone palette.

It advances only when an appearance sample is taken.

Its root transform follows the canonical actor smoothly, but its local bone pose remains held between samples.

This separation is important. Temporarily rewinding the canonical skeleton to capture a plate and then restoring it would create brittle render-order dependencies.

`SkeletonUtils.clone()` can correctly clone a hierarchy containing `SkinnedMesh` objects and bones while reusing geometry and material references. That is a reasonable starting point for creating the appearance rig, after which its materials can be replaced with ghost-specific materials.

Two animation ownership paths are possible.

### Renderer-evaluated animation

The Three.js backend owns an independent appearance `AnimationMixer`.

On an appearance tick:

```text
evaluate appearance mixer at held time
update appearance matrices
capture plate
hold pose
```

### Engine-evaluated animation

Rust or another engine system owns bone evaluation.

On an appearance tick, copy or upload a held bone palette to the appearance rig.

Only the held bone palette needs to cross the boundary at the appearance cadence. Root transforms can continue crossing every frame.

## 3.3 Capture puppet

For the first spike, the held appearance rig can be copied into a small dedicated capture scene.

For production, use one of these approaches:

1. A dedicated capture clone per visible ghost actor.
2. A pool of reusable capture puppets keyed by rig or model type.
3. Shared held skeleton data between display and capture meshes, provided bind transforms remain controlled.

A dedicated capture scene avoids traversing and rendering the entire main scene for every plate update.

The capture scene should contain only:

* the actor and attached equipment;
* fixed capture lighting, if used;
* a transparent background;
* the ghost camera.

---

# 4. Render pass graph

```text
Canonical actor
    ├── gameplay / collision / IK
    ├── canonical shadow pass
    └── pose sample
            │
            ▼
Held appearance rig
    ├── ghost capture pass
    │       ├── raw color
    │       └── source depth
    │
    ├── plate stylization pass
    │       └── finished ghost plate
    │
    └── ghost display pass
            ├── ray-preserving mesh deformation
            ├── finished plate projection
            ├── source-shell rejection
            └── world fog / tint / contact integration
```

## 4.1 Ghost capture pass

Render the held appearance actor from the quantized ghost camera.

Initial output:

* raw color and alpha;
* source depth.

Start with a simple unlit or deliberately fixed-light capture material. The capture should resolve the actor into one coherent image rather than letting several independent real-time effects reinterpret it later.

Useful capture choices:

* transparent clear color;
* no MSAA;
* no mipmaps;
* nearest-neighbor texture filtering;
* tight near and far planes around the actor;
* fixed framing per actor archetype.

Do not dynamically auto-fit the capture camera to every pose. That would make the actor’s apparent scale pump as limbs move. Use a stable archetype-specific capture volume with enough margin for weapons and animation extremes.

## 4.2 Plate stylization pass

Render a fullscreen triangle or quad over the tiny capture target.

Possible operations:

* palette quantization;
* value-band reduction;
* edge or outline construction;
* alpha cleanup;
* dithering;
* exposure treatment;
* cluster cleanup;
* semantic feature emphasis;
* minimum-thickness repair.

The crucial structural rule is that these operations resolve into one final plate before the actor is drawn into the world.

The first prototype should use an extremely modest stylization pass:

```text
raw capture
    ↓
palette reduction
    ↓
optional outline
    ↓
finished plate
```

Do not begin by rebuilding every previous effect.

## 4.3 Ghost display pass

Render the held appearance mesh into the main scene.

The vertex shader:

1. applies morph targets and skinning;
2. calculates the undeformed source position;
3. projects it into the ghost camera;
4. compresses source-camera depth;
5. preserves source-camera screen position;
6. transforms the warped position back into world space;
7. projects it through the real player camera.

The fragment shader:

1. retrieves the source plate coordinate;
2. optionally rejects surfaces hidden in the source plate;
3. samples the finished plate;
4. applies alpha testing;
5. adds restrained world tint, fog, and contact integration.

## 4.4 Canonical shadow pass

The undeformed canonical model should initially cast the actor’s shadow.

This allows the visible actor to remain shallow while its shadow insists that a full body occupies the world.

The visible ghost mesh should not cast its own standard shadow during the first prototype.

---

# 5. Three.js render-target setup

Assumption for the initial spike:

```text
THREE.WebGLRenderer
```

Three.js render targets support depth textures and multiple color attachments, and `WebGLRenderer` renders into a selected target using `setRenderTarget()`. `ShaderMaterial` and `onBeforeCompile()` are WebGLRenderer-specific paths. If `rusty-engine` currently uses `WebGPURenderer`, the same design should instead be expressed through NodeMaterial or TSL position and fragment nodes.

Conceptual target setup:

```ts
const rawCapture = new THREE.WebGLRenderTarget(width, height, {
  minFilter: THREE.NearestFilter,
  magFilter: THREE.NearestFilter,
  generateMipmaps: false,
  depthBuffer: true,
  stencilBuffer: false,
  samples: 0,
});

rawCapture.depthTexture = new THREE.DepthTexture(width, height);
rawCapture.depthTexture.minFilter = THREE.NearestFilter;
rawCapture.depthTexture.magFilter = THREE.NearestFilter;
rawCapture.depthTexture.generateMipmaps = false;

const stylizedPlate = new THREE.WebGLRenderTarget(width, height, {
  minFilter: THREE.NearestFilter,
  magFilter: THREE.NearestFilter,
  generateMipmaps: false,
  depthBuffer: false,
  stencilBuffer: false,
  samples: 0,
});
```

This is illustrative rather than a drop-in constructor contract. Match the exact creation path to the pinned Three.js version used by `rusty-engine`.

Color-space rules:

* finished color plates should follow the engine’s existing linear/sRGB convention;
* depth, normals, masks, and semantic IDs are non-color data;
* non-color textures should not receive color-space conversion;
* nearest filtering and disabled mipmaps prevent low-resolution plate texels from becoming blurred.

Three.js exposes nearest-neighbor filtering and explicit texture color-space metadata through `Texture`.

When performing a capture:

```ts
const previousTarget = renderer.getRenderTarget();
// Also save viewport, scissor, clear color, and clear alpha if the engine mutates them.

renderer.setRenderTarget(rawCapture);
renderer.setViewport(0, 0, width, height);
renderer.setScissorTest(false);
renderer.setClearColor(0x000000, 0);
renderer.clear(true, true, true);

renderer.render(captureScene, ghostCamera);

renderer.setRenderTarget(previousTarget);
// Restore all other renderer state.
```

The capture system should be wrapped in a renderer-state guard so a failed or early-returning capture cannot poison the main render.

## 5.1 Initial attachment strategy

Start with:

* one RGBA color attachment;
* one depth texture;
* one stylized color target.

Later, the current RenderTarget API’s `count` and `textures[]` support can be used for multiple color attachments such as:

* raw color;
* linear depth;
* normal;
* semantic mask.

Do not start with MRT unless source-depth precision or semantic masks become immediately necessary.

## 5.2 Linear depth versus hardware depth

Two source-depth paths are possible.

### Hardware depth texture

Advantages:

* simplest initial setup;
* automatically captures the front surface.

Disadvantages:

* nonlinear;
* comparison depends on the exact projection and depth convention;
* requires care if reversed depth is enabled.

### Explicit linear-depth attachment

A capture shader writes positive ghost-camera depth into an `R16F` or similar attachment.

Advantages:

* easier source-shell comparison;
* stable epsilon in actor-space units;
* simpler debugging.

Disadvantages:

* requires another color attachment or capture pass.

Recommendation:

1. Use the normal depth texture for the first whole-mesh prototype.
2. Add explicit linear depth when implementing source-visible shell rejection.

---

# 6. Ray-preserving deformation math

Three.js camera space looks down negative Z.

Let the undeformed, skinned vertex in ghost-camera space be:

```text
p = (x, y, z)
```

Define positive depth:

```text
d = -z
```

Choose an anchor plane at positive depth:

```text
d₀
```

The anchor should generally pass through the torso, pelvis, or actor bounds center.

Let `k` be depth retention:

```text
k = 1.0   ordinary 3D
k = 0.3   shallow relief
k = 0.1   strongly flattened
k = 0.02  almost a plate
```

Compress depth around the anchor:

```text
d′ = d₀ + k(d - d₀)
```

Then scale X and Y by the same depth ratio:

```text
s = d′ / d

x′ = xs
y′ = ys
z′ = -d′
```

This preserves:

```text
x′ / d′ = x / d
y′ / d′ = y / d
```

Therefore the vertex retains the same projected position from the ghost camera.

## 6.1 GLSL-like vertex pseudocode

```glsl
// `transformed` is the local-space position after morphing and skinning.

vec4 originalWorld =
    modelMatrix * vec4(transformed, 1.0);

vec4 originalGhost4 =
    uGhostViewWorld * originalWorld;

vec3 originalGhost = originalGhost4.xyz;

float d = max(-originalGhost.z, 0.0001);
float k = clamp(uDepthRetention, 0.0, 1.0);

float warpedDepth =
    uAnchorDepth + k * (d - uAnchorDepth);

float rayScale = warpedDepth / d;

vec3 warpedGhost = vec3(
    originalGhost.x * rayScale,
    originalGhost.y * rayScale,
    -warpedDepth
);

vec4 warpedWorld =
    uGhostViewWorldInverse * vec4(warpedGhost, 1.0);

vec4 realClip =
    projectionMatrix * viewMatrix * warpedWorld;

gl_Position = realClip;
```

The CPU or renderer coordinator should calculate and upload:

```text
uGhostViewWorld
uGhostViewWorldInverse
uGhostProjection
uAnchorDepth
uDepthRetention
```

Do not invert the matrix per vertex.

## 6.2 Ghost camera transform

Store the selected ghost camera in actor-local space.

Every frame:

```text
ghostCameraWorld = actorRootWorld × ghostCameraLocal
```

Then derive:

```text
ghostViewWorld = inverse(ghostCameraWorld)
```

This lets the actor’s root translate and rotate smoothly after capture while the plate remains attached to the actor.

The held local pose and actor-relative ghost camera remain unchanged until the next appearance update.

---

# 7. Source plate coordinates

Before deforming the vertex, project its undeformed world position through the ghost camera:

```glsl
vec4 sourceClip =
    uGhostProjection * originalGhost4;

vec2 sourceUv =
    sourceClip.xy / sourceClip.w * 0.5 + 0.5;
```

There are two useful interpolation modes.

## 7.1 Projective-surface mapping

Pass `sourceClip` to the fragment shader and divide there:

```glsl
vec2 sourceUv =
    vSourceClip.xy / vSourceClip.w * 0.5 + 0.5;
```

This behaves like ordinary projective texturing and may look more attached to the surface from off-source angles.

## 7.2 Plate-locked mapping

For the strongest FMV quality, make the plate coordinates interpolate linearly in screen space.

At the vertex:

```glsl
vSourceUvTimesW = sourceUv * realClip.w;
vRealW = realClip.w;
```

At the fragment:

```glsl
vec2 sourceUv =
    vSourceUvTimesW / vRealW;
```

This cancels the default perspective correction and makes the source plate reconstruct more exactly when the real camera matches the ghost camera.

Suggested experiment toggle:

```text
ProjectiveSurface
PlateLocked
```

`PlateLocked` is the recommended starting mode.

It preserves the actor’s image composition more aggressively and is therefore more likely to produce the intended “filmed thing with shallow volume” result.

---

# 8. Fragment shader behavior

Minimal fragment pseudocode:

```glsl
vec2 uv = resolveSourceUv();

if (
    uv.x < 0.0 || uv.x > 1.0 ||
    uv.y < 0.0 || uv.y > 1.0
) {
    discard;
}

vec4 plate = texture2D(uGhostPlate, uv);

if (plate.a < uAlphaThreshold) {
    discard;
}

vec3 color = plate.rgb;

// Keep integration restrained.
color *= uWorldTint;
color = mix(color, uFogColor, computeFogFactor());

gl_FragColor = vec4(color, 1.0);
```

Prefer opaque rendering with fragment discard over normal alpha blending:

* writes ordinary scene depth;
* avoids transparent-object sorting problems;
* preserves crisp pixel edges;
* makes the entity behave as a solid actor in world occlusion.

Do not set `transparent = true` unless the final style genuinely requires partial transparency.

---

# 9. Source-visible shell rejection

The whole-mesh version will allow back surfaces to sample colors belonging to front surfaces.

This may be interesting, but it may also look like a broken projector.

A source-visible shell mode can reject fragments that were hidden from the ghost camera when the plate was captured.

## 9.1 Depth comparison

At capture time, store the source front depth.

At display time, calculate the undeformed fragment’s source depth and compare it with the captured value:

```glsl
float capturedDepth =
    texture2D(uSourceDepth, sourceUv).r;

float fragmentSourceDepth =
    resolveInterpolatedSourceDepth();

if (
    fragmentSourceDepth >
    capturedDepth + uSourceDepthEpsilon
) {
    discard;
}
```

Use nearest-neighbor sampling.

A depth epsilon is necessary because:

* the source depth map is low resolution;
* triangles interpolate depth continuously;
* rasterization rules differ slightly around silhouettes;
* equipment and adjacent surfaces may nearly coincide.

## 9.2 Mask dilation

If source-shell rejection creates edge holes:

1. increase the depth epsilon slightly;
2. dilate the source coverage mask by one plate texel;
3. retain the nearest valid depth around silhouette pixels;
4. fade rejection strength near plate boundaries;
5. temporarily render the whole mesh around the edge.

## 9.3 Whole mesh versus shell

Expose this as a debug slider or mode:

```text
0.0  whole mesh
1.0  strict source-visible shell
```

Intermediate values can soften rejection or allow a bounded amount of hidden geometry to emerge.

---

# 10. Semantic depth retention

Once the global deformation works, selected body parts can retain different amounts of depth.

Suggested starting profile:

| Region                    | Depth retained |
| ------------------------- | -------------: |
| Face                      |   0.03 to 0.08 |
| Hair                      |   0.10 to 0.20 |
| Torso                     |   0.15 to 0.25 |
| Legs                      |   0.20 to 0.30 |
| Arms                      |   0.25 to 0.40 |
| Hands                     |   0.35 to 0.55 |
| Weapon                    |   0.50 to 0.80 |
| Long horns or protrusions |   0.50 to 0.80 |

Two implementation routes are practical.

## 10.1 Per-vertex attribute

Add a float attribute to the appearance geometry:

```text
ghostFlattenWeight
```

Interpretation:

```text
0.0  retain real depth
1.0  use full configured flattening
```

Shader:

```glsl
float semanticK =
    mix(1.0, uDepthRetention, ghostFlattenWeight);
```

This is simple and art-directable.

## 10.2 Bone-weight-derived retention

Assign a depth multiplier per bone and calculate a weighted value from the existing skin weights.

This avoids another vertex attribute but requires a GPU-accessible table of per-bone ghost-depth values.

Start with the per-vertex attribute only after the global deformation has proven worthwhile.

---

# 11. Depth banding

A later variant can quantize compressed depth into several strata:

```glsl
float offset =
    k * (d - uAnchorDepth);

float bandedOffset =
    round(offset / uDepthBandSize) *
    uDepthBandSize;

float finalOffset =
    mix(offset, bandedOffset, uDepthBandStrength);

float warpedDepth =
    uAnchorDepth + finalOffset;
```

Important limitation:

A vertex shader can snap vertices to depth bands, but triangles spanning different bands will still interpolate between those vertices. The result is faceted relief, not perfectly separate animation cels.

True independent depth sheets require one of:

* appearance-only geometry subdivision;
* duplicated vertices at depth boundaries;
* per-triangle depth assignment;
* preprocessing into separate shell layers;
* a future mesh-processing stage capable of splitting primitives.

Do not treat vertex-level banding as literal discrete layers.

---

# 12. View-sector selection

## 12.1 Actor-relative azimuth

Calculate the vector from actor to real camera, transformed into actor-local space.

Project onto the actor’s horizontal plane:

```text
azimuth = atan2(localView.x, localView.z)
```

Quantize:

```text
sectorSize = 360° / sectorCount
sector = round(azimuth / sectorSize)
```

Initial values:

```text
16 sectors  = 22.5° each
24 sectors  = 15° each
```

Start with 16.

## 12.2 Hysteresis

Do not switch sectors exactly at the midpoint.

For a 22.5° sector, retain the current sector until the view passes its boundary by another 2 to 4 degrees.

This prevents:

* rapid A/B chatter;
* repeated plate captures;
* visible instability while the player makes small mouse movements.

## 12.3 Elevation

Ignore elevation sectors during the first spike.

Use a fixed capture-camera height representative of normal gameplay.

Later options:

```text
low
level
high
```

The shallow relief should tolerate modest pitch variation before an additional elevation band becomes necessary.

## 12.4 Ghost lens

The ghost camera does not need the same FOV as the real first-person camera.

Suggested experiment:

```text
real camera: 75° to 90°
ghost camera: 35° to 55°
```

A narrower ghost lens may help the actor feel separately photographed.

Keep the lens fixed per archetype. Do not continuously match it to the player camera.

---

# 13. Animation sampling

The actor has two simultaneous animation times.

```text
physicalAnimationTime
appearanceAnimationTime
```

The canonical actor advances normally.

The appearance actor samples and holds:

```text
appearanceAnimationTime =
    floor(time * appearanceHz) / appearanceHz
```

For example:

```text
appearanceHz = 12
```

At each appearance tick:

1. evaluate the held appearance pose;
2. update morph targets;
3. update bone matrices;
4. capture a new plate if the pose changed enough;
5. retain the resulting pose and plate until the next tick.

Root translation and actor world rotation remain smooth every display frame.

This creates the useful FMV structure:

* world movement is smooth;
* the performance is held;
* plate, silhouette, lighting, and geometry all update together.

Avoid a state where animation is sampled at 12 Hz but lighting, outline, palette, or plate content continue changing at display rate.

---

# 14. Sector transitions

Start with hard sector changes.

A hard change is useful because it reveals whether the base depictions are strong enough. A complicated transition can conceal defects while adding another instrument to the late-night orchestra.

## 14.1 Recommended first transition

When the sector threshold is crossed:

1. wait until the next appearance sample;
2. freeze the held pose;
3. capture the new sector using that same pose;
4. switch ghost camera, plate, and warp together;
5. resume normal appearance sampling.

This avoids needing two simultaneous bone palettes.

## 14.2 Geometry morph

The vertex shader can calculate old and new warped positions:

```glsl
vec3 oldPosition = warpWithGhostCameraA();
vec3 newPosition = warpWithGhostCameraB();

vec3 finalPosition =
    mix(oldPosition, newPosition, uTransitionT);
```

This works most cleanly when both plates use the same held pose.

The resulting rubbery rematerialization may be a feature.

## 14.3 Plate dither

Sample both plates and select using an ordered pattern:

```glsl
float threshold =
    bayer4x4(sourcePixelCoordinate);

vec4 plate =
    threshold < uTransitionT
        ? plateB
        : plateA;
```

Base the dither on plate coordinates rather than screen coordinates so the replacement pattern remains attached to the actor.

Do not alpha-blend plates unless translucency is specifically desired.

---

# 15. Three.js material strategy

This is multipass mesh rendering plus vertex deformation. It does not require a hardware mesh-shader stage.

## 15.1 Fast spike

Patch a `MeshBasicMaterial` or similarly simple built-in material using `onBeforeCompile()`.

Advantages:

* existing skinning;
* existing morph-target support;
* existing clipping and fog chunks;
* less initial shader boilerplate.

Replace or inject around:

* the point after morphing and skinning;
* the built-in projection calculation;
* the final diffuse-color output.

When using `onBeforeCompile()`, provide a stable `customProgramCacheKey()` for shader permutations. Both APIs are specific to WebGLRenderer.

## 15.2 Dedicated production material

Once the experiment works, move the effect into a dedicated renderer-backend material.

Possible paths:

* `ShaderMaterial` using the necessary Three.js shader chunks;
* an engine-owned material wrapper pinned to the project’s Three.js version;
* NodeMaterial/TSL if the backend migrates to WebGPU.

Avoid `RawShaderMaterial` initially because it requires manually recreating more of Three.js’s built-in object, camera, morph, and skinning machinery.

Modern Three.js no longer uses the old `material.skinning = true` flag. Skinning is inferred from the rendered object and geometry, but a custom shader still needs the appropriate transformed-position path or shader chunks.

## 15.3 Normals

The visible ghost material should not use normals generated from the warped relief as though they were physically correct.

Options:

* use no conventional lighting;
* pass undeformed world normals for a mild world-light term;
* use only actor-center ambient tint;
* use the source plate’s captured lighting;
* use a contact shadow for grounding.

The plate should remain the dominant source of internal shading.

---

# 16. Multi-part actors and equipment

A character may consist of:

* body mesh;
* head mesh;
* hair;
* clothing;
* weapon;
* accessories;
* bone-attached rigid props.

All parts should:

* render into the same source plate;
* use the same ghost camera;
* use the same anchor depth;
* sample the same finished plate;
* participate in the same source-depth comparison.

Using world-space ghost matrices makes this straightforward.

Each submesh:

1. calculates its undeformed world position using its own `modelMatrix`;
2. transforms into the shared ghost camera;
3. performs the same ray-preserving compression;
4. transforms back to world space.

Rigid equipment can use higher depth retention than the body.

---

# 17. Culling and bounds

GPU vertex deformation is invisible to Three.js CPU-side frustum culling.

For the spike:

```ts
ghostMesh.frustumCulled = false;
```

For production:

* calculate a conservative actor-specific bounding sphere;
* include relief deformation and weapons;
* update bounds only when the held appearance pose changes;
* avoid recomputing full skinned bounds every display frame.

Three.js notes that animated `SkinnedMesh` bounds are not continuously updated automatically and may need recomputation as animation changes.

---

# 18. Capture scheduling and performance

A plate update costs approximately:

* one tiny skinned capture draw;
* one tiny fullscreen stylization pass;
* possible additional attachment writes.

The visible ghost still costs one normal skinned draw per display frame.

## 18.1 Update priority

Prioritize actors by:

1. on-screen visibility;
2. screen-space size;
3. sector change;
4. elapsed appearance age;
5. distance;
6. gameplay importance.

## 18.2 Capture budget

Use a maximum capture count or GPU-time budget per frame.

Example initial policy:

```text
near actor sector change   capture immediately
near actor pose update     high priority
mid-distance pose update   budgeted
far actor                  reduced appearance Hz
offscreen actor            no capture
```

If an update is delayed, keep showing the old plate. A small amount of visual latency is preferable to a frame-time spike and is compatible with the FMV aesthetic.

## 18.3 Render-target lifetime

Initial implementation:

* current plate per active ghost actor;
* optional previous plate during transition;
* shared intermediate stylization target.

Later optimization:

* pool plate targets by resolution;
* pack finished plates into an atlas or texture array;
* pool capture puppets by rig;
* use lower resolution and cadence by distance.

Do not add an atlas until per-actor targets become an observed problem.

---

# 19. Debug controls

The effect needs unusually strong inspection tools because several coordinate systems can fail while still producing something vaguely actor-shaped.

Expose:

```text
Ghost effect enabled
Canonical mesh visible
Ghost camera frustum visible
Source plate inset
Source depth inset
Depth retention
Anchor depth
Ghost camera FOV
Ghost camera distance
Sector count
Current sector
Sector hysteresis
Appearance Hz
Whole mesh / source shell
Depth epsilon
Projective / plate-locked UV mode
Depth band strength
Freeze pose
Freeze sector
Freeze plate
World integration strength
```

Useful visualizations:

* undeformed mesh wireframe;
* warped mesh wireframe;
* source UV as RGB;
* source-depth error heatmap;
* discarded shell fragments;
* actor-local ghost camera axes;
* ghost plate overlaid on the source-camera view;
* plate texel grid.

---

# 20. Validation tests

## 20.1 CPU projection-invariance test

Implement the warp math in Rust or TypeScript and test random points in front of a perspective camera.

For each point:

1. project original point through ghost projection;
2. apply depth compression;
3. project warped point through ghost projection;
4. compare NDC X and Y.

Expected:

```text
abs(originalNdc.xy - warpedNdc.xy) < small epsilon
```

Test:

* several depth-retention values;
* several anchor depths;
* points around the actor bounds;
* non-square aspect ratios;
* different ghost-camera FOVs.

This catches sign and matrix-order mistakes before GLSL becomes involved.

## 20.2 Exact source-view GPU test

Set the real camera equal to the ghost camera.

Render:

* source capture;
* ghost relief.

Compare them side by side or with a difference shader.

Expected:

* silhouette agreement within roughly one plate texel;
* no large-scale texture swimming;
* no unexplained scale shift;
* no root offset.

If this fails, likely causes include:

* source and display pose mismatch;
* incorrect matrix composition;
* Y-axis texture inversion;
* perspective-interpolation mismatch;
* wrong anchor-depth sign;
* capture camera framing changing between passes.

## 20.3 Motion test

Record a standard automated route:

1. circle-strafe at ordinary gameplay speed;
2. approach and retreat;
3. look up and down;
4. pass close to the actor;
5. cross several sector boundaries;
6. repeat under bright and dark lighting.

Do not judge the effect primarily from slow orbit GIFs.

---

# 21. Likely failure modes

## Plate swims across the mesh

Likely causes:

* source and display poses differ;
* source UV interpolation is unsuitable;
* ghost camera matrices are not actor-relative;
* plate was captured before skeleton matrices updated.

Try plate-locked UV interpolation and freeze all state except camera movement.

## Actor does not match the plate from the source angle

Likely causes:

* incorrect camera-space Z convention;
* wrong matrix multiplication order;
* real and ghost camera projections differ unexpectedly;
* ghost camera moved after capture;
* capture framing changed dynamically.

## Back of actor displays front-face colors

Cause:

* whole-mesh projection without source visibility rejection.

Add source-depth shell rejection.

## Edge holes appear

Likely causes:

* low-resolution source depth;
* depth epsilon too strict;
* nearest-depth mismatch at silhouettes.

Dilate the source mask or loosen the epsilon.

## Actor becomes rubbery off-angle

Likely causes:

* depth retention too high;
* sector width too wide;
* large triangles in the appearance geometry.

Reduce retained depth, add sectors, or subdivide the appearance-only mesh.

## Actor disappears near the edge of view

Likely cause:

* CPU frustum bounds do not include shader-deformed geometry.

Disable frustum culling during the spike.

## Shadow feels disconnected

Possible responses:

* reduce canonical-shadow sharpness;
* use a contact shadow plus softer canonical shadow;
* compress only the visible actor while keeping feet near canonical depth;
* reduce depth mismatch near the ground.

## Near-zero depth causes z-fighting

At extremely low retention, multiple surfaces collapse toward the same plane.

Mitigations:

* maintain a minimum depth retention such as `0.02`;
* enable source-shell rejection;
* retain more depth for overlapping limbs and equipment;
* add tiny semantic depth offsets.

---

# 22. Recommended implementation sequence

## Phase 0: math-only test

Implement the ray-preserving warp on CPU and verify projection invariance.

No Three.js scene changes.

## Phase 1: static mesh warp

Use:

* one static mesh;
* one ghost camera;
* one real camera;
* no capture texture;
* a checkerboard or ordinary texture;
* global depth-retention slider.

Acceptance criterion:

The mesh silhouette is unchanged from the ghost camera and becomes shallow when viewed off-axis.

## Phase 2: raw ghost plate

Add:

* low-resolution color capture;
* projection of that capture onto the warped mesh;
* no stylization;
* no skinning;
* no source-depth rejection.

Acceptance criterion:

At the source view, the relief reconstructs the captured image.

## Phase 3: skinned held appearance rig

Add:

* independent appearance skeleton;
* held animation sampling;
* smooth root motion;
* canonical shadow proxy.

Acceptance criterion:

The actor moves smoothly through the world while its performance updates at a held cadence.

## Phase 4: view sectors

Add:

* 16 actor-relative azimuth sectors;
* hysteresis;
* hard plate and ghost-camera switching;
* debug sector display.

Acceptance criterion:

Circle-strafing produces deliberate directional depiction changes without sector chatter.

## Phase 5: minimal stylization

Add only:

* palette reduction;
* optional outline;
* alpha cleanup.

Acceptance criterion:

The plate feels more image-like without breaking the source-view reconstruction.

## Phase 6: source-visible shell

Add:

* source-depth texture;
* source-depth comparison;
* depth-error visualization;
* shell-strength control.

Acceptance criterion:

Back geometry no longer incorrectly carries front-image colors.

## Phase 7: semantic depth

Add:

* per-vertex flatten weight;
* greater depth for hands and weapons;
* flatter face and torso.

Acceptance criterion:

Important protrusions gain parallax while the actor’s central image composition remains stable.

## Phase 8: transitions and scheduling

Add:

* capture budget;
* priority queue;
* previous plate;
* geometry morph or plate-space ordered dither;
* distance-based plate size and appearance rate.

Do not begin Phase 8 until the static and single-actor result is visually compelling.

---

# 23. Minimal viable spike

The smallest version worth testing is:

```text
1 animated actor
1 canonical rig
1 held appearance rig
1 ghost camera
16 azimuth sectors
96 × 128 plate
12 Hz appearance sampling
0.15 to 0.30 depth retention
plate-locked source mapping
hard sector changes
canonical shadow
no semantic depth
no MRT
no compute pass
no layered depth
no fancy transition
```

Required controls:

```text
Depth retention
Ghost FOV
Sector count
Appearance Hz
Source-view freeze
Canonical/ghost comparison
```

Primary test values:

```text
depth retention: 0.30
sector count: 16
appearance rate: 12 Hz
ghost FOV: 45°

depth retention: 0.15
sector count: 16
appearance rate: 10 Hz
ghost FOV: 40°

depth retention: 0.05
sector count: 24
appearance rate: 12 Hz
ghost FOV: 45°
```

---

# 24. Definition of success

The experiment succeeds when:

* the source-camera view reads as a coherent low-resolution actor image;
* nearby camera movement produces restrained parallax rather than billboard rotation;
* the actor feels intentionally flatter than the voxel world;
* root movement remains comfortable and conventionally spatial;
* the canonical shadow grounds the actor;
* sector changes read as depiction changes rather than broken geometry;
* the style comes primarily from one resolved plate rather than several independent effects;
* no CPU readback or per-vertex CPU deformation is required.

The representation does not need to look correct from every angle.

It needs to maintain:

> arbitrary player movement with controlled, view-conditioned abstraction.

That is the ghost curse worth testing.
