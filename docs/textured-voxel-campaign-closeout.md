# Textured voxel campaign closeout

Status: implemented provider capability with explicit Studio consumer evidence.

This document is the durable entry point for the VTX0–VTX6 campaign. It records
the completed ownership chain, measurable costs, proof boundaries, and stopping
point without requiring Den access. The detailed formulas, quotas, and error
identities remain owned by
[runtime voxel surface textures](topics/voxel/voxel-surface-textures.md).

## Delivered capability

Rusty Engine supports color-only voxel materials, whole-texture repeat, and
repeat within one padded atlas region. All three use the existing canonical
voxel material slots, greedy mesh partition, renderer-neutral retained frame,
Three mesh/material path, and Studio project mutation boundary.

The owner chain is deliberately narrow:

```text
voxel cells / voxel-object frames (material slots only)
        |
        +--> asset-catalog (texture, atlas, material, exact dependency closure)
        +--> svc-mesh (one greedy partition and cell-space tile coordinates)
        +--> render-model / render-projection (bounded neutral resources and facts)
        +--> renderer-three (PNG decode, shader specialization, GPU lifecycle)
        +--> Studio (observational form and viewport)
                    |
                    +--> downstream Rust adapter (admission, assignment, persistence)
```

No ordinary Rust crate gained a filesystem path, URL, fetch request, browser
image, DOM/WebGL object, or Three dependency. Studio never parses an atlas,
meshes voxels, or publishes project truth. The downstream adapter decides its
project paths and storage policy, but consumes Engine's public catalog, mesh,
projection, and protocol mechanisms.

## Distinctions that must remain visible

- **Conversion-time source sampling** is an offline `voxel-convert` operation.
  It samples a source model texture to choose canonical voxel material slots.
  It does not install a runtime texture.
- **Runtime repeat** binds one complete admitted texture to a canonical material
  and repeats it at an authored cell-space scale and origin.
- **Runtime atlas repeat** binds the same kind of material to one named atlas
  content rectangle. A shader remaps Euclidean repeat phase into the
  half-texel-safe rectangle; it does not expand the greedy quad.
- **Color-only compatibility** is omission, not fallback from a failed texture.
  Old material JSON remains color-only. A present malformed, missing, stale, or
  incomplete texture closure rejects before publication.

## Closed initial contract

- The encoded image format is non-interlaced RGBA8 PNG with exact SHA-256 byte
  identity. Albedo uses sRGB with straight alpha.
- Tile coordinates are signed `f32` cell-space values in the documented
  right-handed basis for all six face directions. Canonical world chunks add
  their absolute voxel origin; voxel objects remain object-local.
- Tile scale is `1/256` through `4096` cells. Coordinates, origins, and their
  joint precision are bounded before interpolation; negative phase uses
  Euclidean remainder.
- Atlas rectangles are integer pixel content bounds. Nearest filtering permits
  zero padding. Linear filtering requires one replicated texel on every side.
  Atlas textures clamp, use a half-texel inset, and generate no mipmaps.
- Tint, existing emission, and `opaque`/`mask`/`blend` alpha policy remain
  canonical material facts. The specialization key contains only schema,
  mapping kind, filter, and alpha mode; authored texture identity, region,
  scale/origin, tint, and emission are bindings or uniforms.
- Complete texture/material/atlas candidates validate before retained-frame,
  project, disk, or GPU publication. Rejected replacement preserves the prior
  frame, project hash, bytes, resources, and renderer statistics.

The exact count and byte ceilings are listed in the owning texture topic. In
particular, one PNG is at most 16 MiB, retained encoded PNG bytes total at most
128 MiB, decoded RGBA8 bytes total at most 256 MiB, and the renderer host admits
at most 256 texture identities. Packed VTX2 geometry retains its separate
256 MiB complete-resource ceiling.

## Measured geometry and resource evidence

The public `rusty-engine-voxels` consumer owns a deterministic 16×8 RGBA8 atlas.
Its two asymmetric 6×6 content regions each have a replicated one-pixel gutter.
The current deterministic stored-DEFLATE PNG is 588 bytes with content hash
`sha256:ac1a8a3685fe0b5b42c585f4f5cf8e246721a09497644eacddf36372a377fd99`.

| Consumer corpus | Visible unit faces | Greedy quads | Vertices | Indices | Position + normal | Tile coordinates | Index bytes | Complete VTX2 streams |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `48×32×1` wall | 3,232 | 6 | 24 | 36 | 576 B | 192 B | 144 B | 912 B |
| `48×1×32` floor | 3,232 | 6 | 24 | 36 | 576 B | 192 B | 144 B | 912 B |
| one resident `16×16×1` chunk side | 560 | 5 | 20 | 30 | 480 B | 160 B | 120 B | 760 B |
| four adjacent material slots | 18 | 18 | 72 | 108 | 1,728 B | 576 B | 432 B | 2,736 B |

The wall and floor have exactly the same six quads, 24 vertices, and 36 indices
as their untextured greedy baseline. Texture support adds 32 bytes per quad; it
does not create a unit-face fallback. Two resident chunks meshed independently
record tile extents `[-16, 0]` and `[0, 16]` with an exact shared phase at zero.
The corpus also records every face direction at negative coordinates and four
material groups in one mesh.

The provider's real Chromium framebuffer proof retains four vertices and six
indices for each asymmetric atlas quad. It samples eight positions across two
repeat periods in standard and rotated bases; the rotated case uses scale
`[0.5, 2]` and origin `[0.25, -0.5]`. Two active proof materials share the one
`(voxelSurfaceV1, atlas, linear, mask)` shader specialization shape.

The public Studio consumer retains one texture resource while replacing repeat
with atlas, saving, reloading, and reopening in a second host/adapter process.
The observed close/reopen/close active-viewport texture counts are `0 → 1 → 0`;
closing destroys the viewport and its renderer rather than leaving an empty
renderer component mounted. Provider
lifecycle tests separately prove two instances share one texture, a successful
redefinition disposes the replaced GPU texture exactly once, final disposal is
idempotent and disposes the replacement exactly once, and the geometry resource
and object handle do not change during material replacement. These are resource
counts and disposal events, not driver VRAM or GPU-time measurements.

## Historical consumer and regeneration owners

The completed campaign recorded a reverse consumer identity in the now-retired
`studio/voxel-consumer-source.json` manifest. That exact-revision workflow is
historical evidence, not the current downstream integration contract. Current
downstream work follows the
[downstream renderer and Studio boundary](topics/development/downstream-renderer-and-studio.md).
The campaign proof drove normal Studio controls in real Chromium, closed the
project to zero resources, and started a fresh Studio host/adapter process
against the same persisted project.

The consumer owns these checked inputs and regeneration commands:

```bash
node scripts/generate-textured-voxel-fixture.mjs --check
cargo run --locked --bin textured-voxel-evidence -- --check
./scripts/verify.sh
./scripts/verify-studio.sh
```

The Engine-owned reverse proof is:

```bash
./scripts/verify-studio-voxel-integration.sh \
  /absolute/path/to/the-exact-rusty-engine-voxels-checkout
```

## Exact review ledger

Each row names the independently reviewed containing revision and its terminal
owning gate. Earlier corrective rounds remain review history; they are not the
certification target.

| Work | Exact reviewed revision | Review round | Terminal gate |
|---|---|---:|---:|
| VTX0 tile-space contract (#6464) | `4ac1e53736d296a2e6301fba84204cf625b16ea3` | 3779 | 2457 |
| VTX1 texture admission and renderer realization (#6465) | `8327b565a5df45aa093a9a6a385401a9fc4be575` | 3785 | 2465 |
| VTX2 greedy tile-coordinate streams (#6466) | `72f85c341f0eb803ea19e08cc0e4fafa005ee781` | 3781 | 2459 |
| VTX3 canonical texture/atlas material bindings (#6467) | `9a9f15b999bd207778e1c9f741e2bd4212c20eca` | 3788 | 2468 |
| VTX4 tiled/atlas Three realization (#6468) | `c8a3363ee0e2490b4c411773a98e19bbfb392fab` | 3797 | 2477 |
| VTX5 Studio authoring and persistence (#6469) | `ca3d86223f668fcf886d690fe0eede7d8cf96a1b` | 3830 | 2513 |
| Public Studio consumer (#6470) | `b7d56a919cc91973637b312fea541814818f1c18` | 3831 | 2514 |

## Deliberate stopping point

The campaign does not claim mipmapped atlas filtering, compressed GPU-native
texture formats, arbitrary source-image formats, per-face material identities,
normal/emissive texture maps, URL/fetch import, measured GPU time, exact VRAM,
or broad art-style coverage. It also does not add a second mesher, renderer, or
Studio-owned material authority.

Add another format, atlas mip policy, or renderer specialization only when a
concrete consumer supplies a bounded corpus and shows why this closed contract
is insufficient. Add broader art and simultaneous-instance measurements in the
downstream repository; promote only a reusable mechanism gap back to Engine.
