# #6923 comparison provenance

## Constant scene and observation route

All six variants use the accepted #6925 shrine, tree, and door; terrain is
deliberately neither requested nor instantiated after the owner found it
visually uninformative. The variants retain the
same checked transforms and `1/128` display scale; camera start
`[0, 1.6, 13]` at pitch `-8°`, yaw `0°`; free-flight route; `4:3`–`16:9`
presentation frame; clear/fog colour `0x89a8b8`; and the same live retained
ambient, directional, and optional camera-following point lights. Compare near, oblique, walking-distance, distant, and
moving-camera views using the selector, then record shimmer, inversion, or
temporal instability as observations—not inferred renderer metrics.

The Application Host neutral world/viewmodel rigs are disabled for this gate.
Its bounded shadow backend is enabled with one active retained-light slot, used
by the directional request. The camera point light remains non-shadowing and
has an owner-adjustable `1..12` world-unit range so it can demonstrate local
falloff rather than illuminate the entire vignette.

## Geometry

The original source, normal derivative, and matte derivative retain the
accepted scene composition and matching source mesh/material bindings. The
unlit producer receipts additionally establish that their BIN geometry/UV
payload remains byte-identical to their original source. The staging manifest
is the authoritative exact-byte inventory for the app-local copies.

## Producer-normal variant

The four normal GLBs were emitted repeatedly, byte-identically, from the
matching accepted `.vox` members with pinned Vengi `v0.5.0.0`, revision
`4d5fbc999`, binary SHA-256 prefix `cfde…45db`:

```text
/home/dev/asset-pipeline/external/vengi-src/build/install-voxconvert/usr/bin/vengi-voxconvert \
  -set voxformat_withnormals true -set voxformat_mergequads true \
  -set voxformat_quads false -set voxformat_withtexcoords true \
  -set voxformat_withmaterials true -set voxformat_withcolor true \
  --input <accepted-member.vox> --output <variant.glb> -f
```

This is a producer-side `NORMAL` comparison route. The Asset Pipeline
whole-voxel normal baker was intentionally not substituted: it rejects these
merged-quad #6925 GLBs (terrain qualified `0/44` triangles). No donor code is
used by this comparison surface.

## Occupancy axis/adjacency A/B

Variants 5 and 6 are direct accepted-VOX occupancy/palette compilations from
the Asset Pipeline A/B evidence run. The axis control assigns each emitted
face its axis normal. The adjacency route sums occupied six-neighbour exposed
face directions, uses the resulting `{-1, 0, 1}` vector when it points outward
of the emitted face, and otherwise falls back to that face normal. Their
manifest records the shared coordinate conversion (`VOX (x,y,z)` to glTF
`(x,z,-y)`), repeat-byte identity, compiler provenance, and independent
reparse checks.

They deliberately use normalised per-vertex `COLOR_0` under one matte PBR
material rather than preserving the prior GLB texture/material partitioning.
They therefore isolate the two occupancy normal treatments within the same
compiler and palette/material representation; they are not texture-preserving
derivatives of variants 1–4. Their exact byte inventory and source manifest
pointer are staged with the other variants.

## Material model and lighting

Variant 2 preserves current PBR material values. Variant 3 is an explicit
producer derivative whose receipts prove `roughnessFactor: 1` and
`metallicFactor: 0`, retaining the normal route and recording no KHR-unlit or
environment behavior. Variant 4 uses the accepted `KHR_materials_unlit`
derivative; retained light changes do not affect it and are not evidence of a
lighting/shadow result.

Variants 5 and 6 use explicit matte PBR with their VOX palette in `COLOR_0`.
Their lighting is the same retained setup as variants 1–3. HUD changes use the
public `updateLight` contract and are applied consistently to the active
variant; the camera point position follows the published camera pose.

The comparison proves the bounded retained-light and shadow route exercised
here, not general PBR quality or a cross-backend lighting guarantee. It does
not create a visual-voxel profile, runtime voxel admission, or collision
authority. The owner selection that governed Phase 2 is recorded below.

## Owner decision

The 2026-08-24 owner inspection selected variant 5, direct-VOX axis normals
with matte PBR and palette `COLOR_0`, as the baseline for these microvoxel
models. Variant 6 is not mislabeled: its emitted shrine has 26 normal keys and
107,040 greedy quads versus variant 5's six normal keys and 101,441 quads. In
the retained-lit comparison those additional diagonal face-region normals
produced harsh dark voxel-edge bands rather than perceptual smoothing.

The adjacency rule came from a substantially coarser-grid use case, so the
result is retained as bounded evidence rather than declared universally bad.
It needs a separate coarse-grid visual consumer before reconsideration. No
voxel-specific shader, post-process, silent material mutation, or normal mode
is promoted from this task. The only provider addition is neutral Application
Host forwarding of the renderer-host's existing default-light and bounded
shadow policy, prompted by discovering that the comparison could not otherwise
disable compatibility lights or activate requested shadows.
