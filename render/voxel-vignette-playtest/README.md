# Voxel shading comparison visual gate

This is the bounded Phase 1 visual gate for Den #6923. It uses the exact
#6925 shrine, tree, and door assets through the public
`@rusty-engine/application-host` API. It exposes exactly six complete asset
catalogues. The visually uninformative terrain is staged for provenance but
is neither requested nor instantiated:

1. original PBR GLBs with no `NORMAL` attribute;
2. deterministic pinned-producer `NORMAL` GLBs with their current PBR values;
3. those producer-normal GLBs with an explicit producer-side matte PBR
   derivative (`roughnessFactor: 1`, `metallicFactor: 0`);
4. the accepted producer-side `KHR_materials_unlit` derivative;
5. a direct-VOX occupancy compile with axis face-normal control;
6. the otherwise-matched direct-VOX occupancy compile with exposed-face
   adjacency-donor normals.

Owner verdict (2026-08-24): route 5 is the accepted baseline for this
high-density microvoxel corpus. Route 6 is visibly harsher because its 26
face-region normal keys create dark edge bands under local lighting; retain it
only as an experiment for substantially coarser voxel grids. The app opens on
route 5. No voxel-specific shader or material profile is promoted.

The selector atomically replaces the full public-host resource catalog and
retained scene. Geometry placement, camera route, viewport bounds, clear/fog,
scale, and current light settings are identical across variants. Bounded HUD
controls publish ordinary retained `updateLight` frames for ambient,
directional, and camera-following point lights. This comparison explicitly
disables the host's neutral default rigs and enables a one-light retained
shadow budget, so zero ambient is actually zero ambient and the directional
request casts/receives ordinary Three shadow-map shadows. The point light has
an adjustable `1..12` world-unit range. The two occupancy routes are
truthfully different GLB compilations: their accepted VOX palette is `COLOR_0`
under one explicit matte PBR material, rather than the prior GLB palette
texture/material partitioning. It does not import Three or mutate loaded
materials.

Before serving, stage and verify the checked ignored GLBs:

```bash
./scripts/stage-voxel-vignette-comparison-assets.sh
./scripts/stage-voxel-vignette-comparison-assets.sh --check
pnpm --dir render run check:voxel-vignette
```

The exact hashes, byte counts, paths, and receipt/provenance pointers are in
[`comparison-staging-manifest.tsv`](comparison-staging-manifest.tsv). The
normal source is deliberately recorded as the verified ephemeral producer
output; after staging it is no longer a runtime dependency. Recreate it only
with the pinned command in [`comparison-provenance.md`](comparison-provenance.md).
The copied GLBs remain ignored and local experimental evidence.

Serve from this directory so its app-local `.den-serve.json` supplies the
public `0.0.0.0` Vite host and persistent comparison route:

```bash
den-serve up rusty-engine-voxel-vignette-comparison -repo "$PWD/render/voxel-vignette-playtest"
```

The owner visual gate is complete. Preserve the comparison as decision evidence;
do not turn route 6 into a provider default without a separate coarse-grid
consumer proof.
