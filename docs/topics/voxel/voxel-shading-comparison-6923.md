# Visual-voxel shading comparison (#6923 Phase 1)

The Phase 1 visual gate is the app-local
[`render/voxel-vignette-playtest`](../../../render/voxel-vignette-playtest)
comparison. It is deliberately presentation evidence, not a renderer feature.

It holds the #6925 shrine/tree/door transforms, scale, viewport, current light
settings, and free-flight camera route constant while replacing the entire
checked GLB catalogue through public application-host lifecycle. Terrain is
staged but not requested or instantiated. Public retained-light controls let
the owner vary ambient and directional intensity and toggle a camera-following
point light without reaching into the Three backend. The six selectable routes are original
PBR/no normals, producer normals/current PBR, producer normals/matte PBR,
accepted `KHR_materials_unlit`, direct-VOX occupancy axis normals, and
direct-VOX occupancy adjacency-donor normals. The last two share their own
direct-VOX `COLOR_0` plus matte-PBR representation, so the comparison reports
that material/palette boundary rather than pretending it is constant.

The detailed evidence inventory, producer commands, material boundaries, and
observation instructions live beside the app in
[`comparison-provenance.md`](../../../render/voxel-vignette-playtest/comparison-provenance.md).
Owner visual judgment was the terminal Phase 1 gate. The decision below names
the one reusable observed host gap and rejects a voxel-specific control.

## Decision

The 2026-08-24 owner visual gate selected direct-VOX axis normals plus matte
PBR (variant 5) for the high-density microvoxel baseline. Occupancy-adjacency
normals (variant 6) were mathematically correct but visually harsher: diagonal
normal changes between face regions formed dark edge bands under local light.
Because the donor treatment originated with much coarser voxel grids, it
remains an offline experiment rather than a provider default and may be
revisited only with a concrete coarse-grid consumer.

No voxel-specific renderer profile, shader, post-process, or runtime VOX
reader is added. The comparison did expose one neutral host gap: authored
retained lights could request shadows, but Application Host could neither
disable its compatibility neutral rigs nor enable the existing renderer-host
shadow policy. `RustyApplicationRendererOptions.lighting` now forwards that
bounded host policy without exposing Three. Omission preserves prior behavior.

The campaign-level representation and import disposition is recorded in
[High-density voxel and pixel-art runtime decision](high-density-voxel-runtime-6911.md).
