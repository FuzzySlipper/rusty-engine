# Offline depth-splat retained-renderer proof

## Decision

The first-surface depth-splat assets do not need a new runtime asset family.
Each direction is one ordinary retained static mesh, whether it contains two
triangles or thousands of pixel quads. Direction choice, animation policy, and
campaign meaning remain downstream concerns.

The proof exposed three generic gaps rather than a splat-specific blocker:

- the mesh contract named a normalized color attribute but could not carry,
  pack, decode, or upload its stream;
- generic retained materials could not request alpha masking or double-sided
  rendering; and
- the public browser surface had no bounded linear-fog option.

Those seams are now completed generically. Packed mesh V1 and V2 bytes remain
unchanged; V3 is used when a normalized RGBA vertex-color stream is present.
No particle path, runtime GLB loader, downstream shader text, per-sample render
object, or splat contract was added.

## Checked source slice

The checked fixture was derived offline from Asset Pipeline task 6977, run
`depth-splat-20260815-001`, subject `spatial-wizard`, direction `dir-00`. The
fixture records the exact SHA-256 of each source GLB and contains the generated
content-addressed retained resources, not a runtime dependency on an Asset
Pipeline checkout.

| Variant | Vertices | Triangles | Source GLB bytes | Uploaded mesh bytes |
|---|---:|---:|---:|---:|
| quad | 4 | 2 | 10,028 | 152 |
| flat | 6,064 | 3,032 | 213,564 | 278,944 |
| physical | 6,064 | 3,032 | 213,588 | 278,944 |
| compressed | 6,064 | 3,032 | 213,588 | 278,944 |
| tangent | 6,170 | 3,032 | 217,020 | 283,184 |

The five GLBs total 867,788 bytes. Their retained mesh streams total 1,120,168
uploaded bytes and 1,120,200 packed bytes across two resources. The conventional
quad uses one 8,377-byte nearest-filter RGBA8 PNG, decoded to 36,864 bytes. The
larger retained representation is expected: normalized 16-bit source colors and
16-bit source indices are admitted as the renderer-neutral f32/u32 contract.

## Browser evidence

The isolated comparison mounts through `@rusty-engine/renderer-host` with its
public mesh and texture resource manifests. The complete comparison retains one
instance per depiction, seven handles and seven geometries including the floor
and occluder, one texture, seven draw calls, and 12,154 submitted triangles.
Isolating the tangent depiction keeps the same resident resources while reducing
submission to two draw calls and 3,044 triangles.

The Chromium fixture verifies:

- nearest texture sampling, alpha mask, normalized vertex RGBA, and
  double-sided material realization;
- ordinary depth testing and a foreground occluder;
- bounded host-owned linear fog across a moving camera route;
- retained transform and visibility updates without resource churn;
- projection-only pick hints, including source metadata but no gameplay action;
- hidden objects do not remain pickable; and
- explicit surface disposal after the comparison.

The browser test prints the measured camera-route time from its current WebGL
backend. Treat that timing as machine evidence, not a product budget or a GPU
completion claim.

## Regeneration

The committed retained fixture is regenerated explicitly from a selected Asset
Pipeline subject directory:

```bash
node render/scripts/generate-depth-splat-fixture.mjs \
  /absolute/path/to/depth-splat-run/spatial-wizard \
  fixtures/render/depth-splat
```

The generator deliberately accepts the current bounded one-node, one-primitive
GLB shape and fails on transforms or unsupported accessor forms. It is fixture
plumbing, not the beginning of a runtime GLB contract.
