# Lit sprite shader comparison

This note records the Engine-owned runtime comparison for low-resolution
sprites that should respond to scene lighting. Asset Pipeline owns authoring
and conversion of source color, normal, and depth images. Rusty Engine admits
those checked resources and owns their bounded retained description, Three
realization, lifecycle, and browser evidence.

## Decision

There is no universal lit-sprite mode. Use the least expensive mode that
preserves the intended shape:

| Mode | Recommended use | Main failure mode |
|---|---|---|
| `unlit` | UI-like marks, effects, distant decoration, and deliberately stable pixel art | Does not belong to the scene's light or shadow language |
| `authoredNormal` | Hero characters and objects whose directional surface detail matters | Extra linear texture, authoring cost, and tangent-space orientation mistakes |
| `authoredDepth` | Assets with trustworthy height data but no hand-authored normal map | A single height field cannot represent overhangs or independently directed detail |
| `derivedGradient` | Bounded fallback for foliage, rough cutouts, and deliberately embossed color art | Color/value edges become false geometry and can shimmer between flipbook frames |
| `synthetic` | Round tokens, orbs, particles, and deliberately volume-like sprites | Imposes a curved volume on flat or irregular silhouettes |

The practical hybrid is an asset-selection policy, not another implicit shader
mode: use an authored tangent-space normal when supplied, an authored depth
map when that is the available trustworthy shape source, a deliberately chosen
derived or synthetic mode for suitable art, and unlit otherwise. The renderer
does not silently reinterpret missing resources because that would make a
retained frame's appearance depend on hidden availability.

Authored object-space normals are not admitted by the current ordinary path.
`MeshStandardMaterial.normalMap` and the billboard plane use tangent space, so
the map rotates with the camera-facing quad. Treating object-space vectors as
tangent-space data produced visibly incorrect light direction as the billboard
turned. Asset Pipeline should therefore export tangent-space normal maps and
linear authored depth maps for this contract.

## Why a camera-facing mesh quad

Engine retains sprites as `PlaneGeometry` meshes rather than Three `Sprite`
objects. The stock sprite material is useful for unlit billboards but does not
provide the normal-map, bump-map, standard-light, shadow, and bounded custom
normal behavior compared here. A mesh quad also preserves Engine's existing
retained handle, hierarchy, authored pivot, atlas frame, picking, depth,
render-order, spherical/cylindrical/world-aligned billboard, and disposal
semantics. It leaves future compatible batching possible without making Three
objects part of the neutral API.

Ordinary retained lit sprites are currently one mesh and one instance-owned
material each. They are not folded into task 6926's particle `Points` batch:
transparent object sorting, independent shadow policy, and different material
modes make that an unsafe equivalence. Three still reuses compiled shader
programs for matching `spriteMaterialVariantKey` values. That key contains
lighting mode, alpha family, depth policy, and normal/bump feature presence;
mutable tint, strength, texture identity, and shadow flags remain instance
state. The synthetic shader uses the fixed cache key
`rusty-sprite-synthetic-v1` and accepts bounded uniforms, never downstream
shader source.

## Renderer-neutral contract

`SpriteMaterialDescriptor` adds:

- `lighting`: the five closed modes above;
- one exact `normalTexture` or `depthTexture` only when its mode requires it;
- finite `normalStrength` in `0..=4` and `normalBias` in `-1..=1`;
- `opaque`, bounded alpha `mask`, or `blend` policy; and
- `none`, `cast`, `receive`, or `castAndReceive` shadow policy.

The descriptor is optional in TypeScript and Rust decoding, so old serialized
frames remain readable. Omitted legacy sprites preserve their previous
texture-aware transparency and default depth-write behavior. New Rust writers
always emit the material block, including its default, so explicit `blend`
remains distinguishable and intentionally disables depth writes. Legacy `lit`
and `shadowed` shading decode to the synthetic mode; legacy `custom` remains an
unlit compatibility fallback and does not admit shader text. New callers
should use the material descriptor.

Color textures use `srgb`. Normal and depth resources must use the new
`linear` color-space fact and must already be retained when the sprite is
admitted. The Three backend rejects a missing or sRGB lighting texture before
mutating retained handles or resources. Existing texture descriptors continue
to own `nearest` versus `linear` filtering and wrap policy. Existing sprite
atlas validation owns unique bounded frame numbers and UV rectangles; all
three maps use the same authored frame selection in the comparison.

## Orientation and temporal behavior

The same quad material works for every existing billboard policy:

- `spherical` follows the complete camera orientation;
- `cylindrical` follows camera yaw while retaining Y-up posture;
- `none` remains world-aligned.

The tangent basis follows the realized plane, so authored normal, authored
depth, derived, and synthetic lighting remain attached to the sprite while its
billboard rotates. Flipbook animation remains authority-driven: a frame update
changes the checked atlas rectangle, not renderer wall-clock state. Source
normal/depth atlases must use the same frame layout as color.

The browser comparison intentionally uses a preserved drawing buffer because
its API captures and downsamples immediately after deterministic route steps.
Without it, a later canvas read can observe a browser-cleared WebGL buffer and
falsely report temporal drift even though the composited frame is unchanged.

## Alpha, shadows, fog, and overlap

- `opaque` and `mask` write depth. Masked sprites use an explicit cutoff and
  give stable ordering and shadow silhouettes at the cost of hard pixel edges.
- `blend` disables depth writes and remains object-sorted. Two intersecting or
  internally layered transparent sprites can still sort incorrectly; split the
  art, prefer a mask, or set an explicit render order when that matters.
- Shadow casting and receiving are opt-in per sprite and are also bounded by
  the renderer's global shadow switch. Lit does not imply shadowed.
- Materials participate in scene fog. They are double-sided so a world-aligned
  cutout does not disappear from its back; strongly directional backside
  lighting can still expose the flat-plane approximation.
- Derived color gradients amplify alpha and palette boundaries. Keep nearest
  filtering for deliberate pixel steps, but do not mistake those steps for a
  faithful depth field.

## Evidence

The checked comparison fixture contains the same opaque cutout, soft alpha
silhouette, four-frame flipbook, foliage clump, and character token across all
five modes. One camera route moves through spherical and cylindrical billboard
orientations while ambient, directional, and point lights move; shadows, fog,
masked alpha, blended overlap, and atlas frame changes remain enabled.

The local headless Chromium run reported:

- 5 fixture recipes and 5 material modes;
- 30 meshes, including one overlapping soft-alpha sprite per mode;
- 25 instance-owned materials and 75 explicit color/normal/depth texture
  instances;
- 15 compiled shader programs and 46 draw calls including shadow passes; and
- 1.65 ms average JavaScript route-and-render submission across 24 steps in
  that local headless environment.

The timing is characterization, not a target-device frame budget. Structural
browser assertions bound residency, program count, and draw calls, require a
materially different moving-camera/light sample, then require the restored
route to return within one luminance level. Headless tests cover strict
contract validation, legacy defaults, material selection and cache identity,
atomic linear-texture rejection, tint updates, disposal, and deterministic
billboard orientation.

Run the focused evidence from the repository root:

```bash
cargo test -p render-model -p render-projection --locked
pnpm --dir render --filter @rusty-engine/render-contracts test
pnpm --dir render --filter @rusty-engine/renderer-three test
PLAYWRIGHT_RENDER_PORT=4191 pnpm --dir render exec playwright test \
  browser/lit-sprite-comparison.browser.spec.ts --config playwright.config.ts
```
