# Runtime voxel surface textures

Status: selected VTX0 contract with VTX1 bounded PNG resource realization,
VTX2 tile-coordinate mesh streams, VTX3 canonical material/atlas bindings, and
VTX4 Three-backend sampling implemented. Studio authoring remains the VTX5
follow-on.

## Decision and ownership

Runtime voxel surfaces support three mappings through the existing material,
greedy mesh, retained projection, and renderer paths:

1. **color only** has no texture dependency or texture coordinates;
2. **repeating tile** repeats one complete texture at an authored cell-space
   period; and
3. **atlas tile** repeats only the content rectangle of one named atlas region.

Voxel cells and voxel-object frames continue to contain material slots, not
texture facts. Material definitions own the presentation binding. `svc-mesh`
owns the one greedy rectangle partition and tile-space vertex coordinates.
`render-model` and `render-projection` own the renderer-neutral resource and
material border. The isolated Three backend owns image decode, GPU resources,
sampling realization, and derived shader/material lifecycle. Studio submits
authoring intent to its Rust adapter and observes canonical readback.

This is distinct from [model-to-voxel texture sampling](voxel-model-conversion.md).
Conversion reads a source model texture offline to choose voxel material slots;
it does not install a runtime surface texture. Runtime surface assignment never
resamples source geometry or changes collision, navigation, occupancy, or voxel
authority.

## Tile-space coordinates

The vertex attribute is two signed `f32` values in **voxel-cell units**, not
normalized 0–1 UVs. Integers through `±16,777,216` are exact; a textured mesh
outside that inclusive coordinate range rejects with
`voxelTexture.tileCoordinateOutOfRange`. This bound does not constrain an
untextured world. Exact integers are not sufficient by themselves: material
admission also enforces
`max(abs(tileCoordinate), abs(tileOriginCells)) <= min(tileScaleCells, 1) *
2^22` on both axes. The actual encoded/interpolated coordinate and uniform
origin are bounded separately; a large matching origin cannot hide precision
already lost before subtraction. This joint bound preserves at least two representable `f32`
intervals across one cell or the authored repeat period, whichever is smaller.
A pair outside it rejects with
`voxelTexture.insufficientCoordinatePrecision`; it never reaches shader
interpolation with an already-quantized phase.

For canonical world chunks, grid points are projected after adding the chunk's
absolute voxel origin. Neighboring chunks therefore have the same phase even
when rebuilt independently. For voxel objects, coordinates are object-local and
the origin is zero; the texture moves with the object's transform. Translation,
rotation, and scale never rewrite tile coordinates. A non-uniform object scale
therefore stretches the rendered tile with the voxel cells rather than imposing
a second world-space mapping.

The outward-facing basis is fixed below. A signed axis means the corresponding
canonical X/Y/Z coordinate is multiplied by that sign.

| Face normal | U | V | Invariant |
|---|---|---|---|
| `+X` | `+Y` | `+Z` | `U × V = +X` |
| `-X` | `+Y` | `-Z` | `U × V = -X` |
| `+Y` | `+Z` | `+X` | `U × V = +Y` |
| `-Y` | `+Z` | `-X` | `U × V = -Y` |
| `+Z` | `+X` | `+Y` | `U × V = +Z` |
| `-Z` | `+X` | `-Y` | `U × V = -Z` |

The basis is executable in
[`svc-mesh::texture_mapping`](../../../rust/crates/svc-mesh/src/texture_mapping.rs).
Its determinant is positive for every already-outward-wound greedy quad, so an
opposite face does not silently mirror the material. Negative coordinates use
Euclidean remainder rather than truncating remainder.

An authored mapping carries `tileScaleCells = [su, sv]` and
`tileOriginCells = [ou, ov]`. Each scale is finite and from `1/256` through
`4096` cells. Each origin is finite and within the exact tile-coordinate range.
The repeated coordinate is:

```text
r = rem_euclid(tileCoordinate - tileOriginCells, tileScaleCells)
    / tileScaleCells
```

Thus `r` is in `[0, 1)`, an exact boundary maps to zero, and a merged `N×M`
rectangle repeats without adding vertices. Material changes remain greedy merge
boundaries; texture identity, atlas region, scale, and tint are material facts,
so unlike materials never become one draw group accidentally.

## Atlas repetition without face expansion

Ordinary texture wrapping cannot repeat a sub-rectangle of an atlas: GPU repeat
would wrap across the whole image and sample neighboring regions. The selected
implementation keeps the canonical greedy quad and uses one derived sampling
specialization:

1. interpolate the tile-space attribute across the existing quad;
2. compute `r` with the formula above;
3. remap `r` into the atlas region's safe texel-center rectangle; and
4. sample the physical atlas texture with clamp-to-edge.

Atlas rectangles are integer pixel rectangles. `contentMin` is inclusive,
`contentExtent` is nonzero, and the content rectangle excludes its extruded
gutter. The normalized safe interval is:

```text
safeMin = (contentMin + 0.5) / imageDimensions
safeMax = (contentMin + contentExtent - 0.5) / imageDimensions
atlasUv = mix(safeMin, safeMax, r)
```

The initial contract supports `nearest` and `linear` filtering and generates no
mipmaps. Nearest regions may have zero padding. Linear regions require at least
one pixel of edge-extruded padding on every side. A padded rectangle must remain
inside the image and may not overlap another region's padded rectangle. This is
the initial deterministic anti-bleed policy; mipmapped atlases require a later
version with measured per-level gutters rather than silently weakening it.

The specialization key is the canonical tuple
`(voxelSurfaceV1, mappingKind, filter, alphaMode)`. Texture identity, atlas
region, tile scale/origin, tint, and emission are uniforms/resource bindings,
not shader variants. Schema 1 has twelve possible specialization keys, below
the contract limit of 64. The backend retains the specialization on its owning
material, replaces dependent materials only after the complete candidate
texture/material closure validates and the new texture is ready, and disposes
the material and texture through the existing reference-counted lifecycle.
Reset/dispose clears all retained resources. No TypeScript remeshing,
per-voxel fallback, or second voxel renderer is allowed.

## Texture resource and material semantics

The VTX1 encoded format is non-interlaced RGBA8 PNG. The admitted byte identity is
`texture-resource/<sha256 hex>` and its `contentHash` is the SHA-256 of the exact
encoded bytes. The logical catalog identity remains `texture/<name>`. The
renderer-neutral descriptor carries identity, encoding, exact encoded length,
width, height, filter, color space, and content hash. Bytes may be supplied
inline for bounded fixtures or as an explicit content-addressed resource; the
two forms must decode to the same admitted RGBA8 pixels.

No path, URL, fetch request, browser image, canvas, blob, or Three object enters
the Rust contract. A caller publishes returned bytes and provides a resolver;
the resolver does not change their identity. PNG albedo is interpreted as sRGB
with straight alpha and converted by the backend's normal color-management
path. The texture sample multiplies material base color, texture tint, and
instance tint. Emission remains the existing linear emission color/intensity;
the initial surface texture does not create a separate emissive map.

`alphaMode` is `opaque`, `mask`, or `blend`. Opaque ignores sampled alpha, mask
uses one finite canonical cutoff, and blend uses sampled alpha multiplied by the
material/instance alpha. A missing texture on an old or explicit color-only
material is valid color-only behavior. A mapping that names a missing, stale,
malformed, or rejected texture is not a silent fallback: the complete candidate
frame rejects and the prior frame/resources remain live.

## Limits and typed rejection

These are admission limits, not suggestions. Count and byte aggregates are
checked before retaining or decoding the next item.

| Resource | Limit | Typed failure |
|---|---:|---|
| Image width or height | 4,096 pixels | `texture.dimensionQuotaExceeded` |
| Image texels | 16,777,216 | `texture.texelQuotaExceeded` |
| One encoded PNG | 16 MiB | `texture.encodedByteQuotaExceeded` |
| Retained texture identities | 256 | `texture.retainedResourceQuotaExceeded` |
| Aggregate encoded bytes | 128 MiB | `texture.aggregateEncodedByteQuotaExceeded` |
| Aggregate decoded RGBA8 bytes | 256 MiB | `texture.aggregateDecodedByteQuotaExceeded` |
| Regions in one atlas | 1,024 | `voxelAtlas.regionQuotaExceeded` |
| Regions in one retained frame | 4,096 | `voxelAtlas.aggregateRegionQuotaExceeded` |
| Region padding per side | 0–32 pixels | `voxelAtlas.invalidPadding` |
| Tile scale per axis | 1/256–4,096 cells | `voxelTexture.invalidTileScale` |
| Tile coordinate/origin magnitude | 16,777,216 cells, plus the joint coordinate/scale precision bound | `voxelTexture.tileCoordinateOutOfRange`, `voxelTexture.insufficientCoordinatePrecision` |
| Voxel surface specializations | 64 | `voxelTexture.specializationQuotaExceeded` |

Zero dimensions, unsupported PNG features/encoding, decoded-dimension mismatch,
nonfinite values, invalid alpha cutoff, wrong-kind identities, duplicate
identities, stale versions, content-hash drift, empty/out-of-bounds regions,
overlapping padded regions, and insufficient linear padding have distinct
typed errors under `texture.*`, `voxelTexture.*`, or `voxelAtlas.*`. Arithmetic
overflow reports the owning quota error. Validation completes before frame,
catalog, project, or GPU publication.

VTX1 implements the texture subset through `render-model`, strict TypeScript
decode, `renderer-host` preloading, and one retained Three `DataTexture` path.
Inline and content-addressed sources converge on the same decoded RGBA8 pixels;
filter, wrap, sRGB interpretation, tint, alpha, and existing emission remain on
the generic `MeshStandardMaterial` path. Retained identity replacement rebuilds
dependent materials only after decode succeeds and disposes the replaced GPU
resource exactly once. Legacy payload omission remains color-only.

VTX3 implements the canonical binding subset through `asset-catalog` and the
renderer-neutral material border. Texture entries own bounded dimensions and
filter/wrap facts; sprite-sheet entries may own schema-1 voxel atlases with
integer content rectangles, explicit padding, and the fixed half-texel inset.
Material entries optionally own an exact-pinned repeat or atlas-region mapping,
tile scale/origin, alpha mode, tint, and existing authority facts. Admission
checks the complete content-addressed dependency closure, region aggregates,
padded overlap, linear-filter gutter, and exact version/hash before projection.
`render-projection::project_catalog_material` returns immutable resolved
texture, atlas, region, version, and hash provenance. Omitted fields retain the
old color-only canonical JSON and behavior.

VTX4 implements the backend subset through the existing retained Three mesh
and `MeshStandardMaterial` path. It specializes Three's existing map UV varying
at shader compilation, applies Euclidean repeat and the atlas safe rectangle,
and retains normal base color/tint, emission, alpha, lighting, and instance
behavior. Final-frame preflight validates the exact texture version, content
hash, filter/wrap policy, retained payload, and atlas content bounds before any
backend mutation. A texture and every dependent material may therefore be
redefined atomically in either operation order; a stale or incomplete candidate
leaves the prior geometry, material, texture, readout, and renderer statistics
unchanged.

## Executable spike and measurements

`svc-mesh` tests exercise the actual greedy partition together with the VTX0
coordinate functions. They cover all six directions, negative origins,
right-handed winding, exact-coordinate limit/one-over, limiting
shader-equivalent phase precision and rejection, `1×1`, `1×7`, and `5×3`
surfaces, mixed material borders, neighboring chunk origins, and two atlas
regions. The atlas reference calculation uses the same safe-rectangle formula
selected above.

| Shape | Visible unit faces | Greedy quads | Vertices | Indices | Textured VTX0 spike |
|---|---:|---:|---:|---:|---|
| `1×1×1` | 6 | 6 | 24 | 36 | unchanged |
| `7×1×1` strip | 30 | 6 | 24 | 36 | unchanged |
| `5×3×1` rectangle | 46 | 6 | 24 | 36 | unchanged |
| two adjacent, different materials | 10 | 10 | 40 | 60 | unchanged |

VTX2 adds eight `f32` values (32 bytes) per greedy quad to voxel inline and
packed-v2 mesh payloads while preserving these exact quad/index counts. Legacy
color-only payloads omit the attribute and retain exact packed-v1 bytes.
Expanding to unit faces would turn the `5×3` top and bottom alone from two quads
into thirty and is rejected as the general atlas design.

The production corpus regression records the attribute cost independently of
resource headers:

| Corpus | Quads | Vertices | Indices | Position + normal bytes | Index bytes | Tile-coordinate bytes | Complete streams |
|---|---:|---:|---:|---:|---:|---:|---:|
| sparse one voxel | 6 | 24 | 36 | 576 | 144 | 192 | 912 |
| solid `4×4×4` | 6 | 24 | 36 | 576 | 144 | 192 | 912 |
| checkerboard `4×4×1` | 48 | 192 | 288 | 4,608 | 1,152 | 1,536 | 7,296 |
| `128×1×1` strip | 6 | 24 | 36 | 576 | 144 | 192 | 912 |
| two resident solid chunks | 10 | 40 | 60 | 960 | 240 | 320 | 1,520 |

The VTX4 browser proof renders a tracked 4-by-3 atlas whose one-pixel red
content region is surrounded by a blue extruded gutter and an adjacent green
pixel. One greedy quad spans 16-by-16 cell-space units using four vertices, six
indices, and 32 bytes of tile coordinates. The real Chromium framebuffer reads
red at the quad center with no green or blue bleed, while the immutable backend
readout records the exact half-texel safe UV `[0.375, 0.5]`. The same browser
frame exercises whole-texture repeat on ordinary and voxel-object geometry,
including frame replacement, reset, resize, and disposal. Headless replacement
tests prove that a coordinated texture/material redefinition keeps the same
geometry resource, four vertices, six indices, and object handle; no remesh or
per-cell resource is created.

Run the proof with:

```bash
cargo test -p svc-mesh --locked
cargo clippy -p svc-mesh --all-targets --locked -- -D warnings
```

## Follow-on contract touchpoints

- VTX1 extends existing `render-model` texture resources, retained projection,
  strict TypeScript decoding, renderer-host resource preload, and Three
  lifecycle with the PNG byte contract above. This slice is implemented.
- VTX2 consumes `svc-mesh::texture_mapping` from the one existing greedy
  `MeshPayload` path and versions inline/packed tile-space streams.
- VTX3 owns canonical material and atlas bindings in `asset-catalog` and the
  renderer-neutral material border; voxel cells retain slots only. This slice
  is implemented.
- VTX4 realizes the selected sampling specialization in the existing Three
  mesh/material path and supplies framebuffer/Chromium proof. This slice is
  implemented.
- VTX5 routes canonical texture/atlas authoring through the existing Studio
  adapter and project mutation boundary.

Model-conversion source sampling remains in `voxel-convert`; none of these
follow-ons should reuse its palette sampling structs as a runtime material
schema.
