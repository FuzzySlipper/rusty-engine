# Smooth voxel surfaces and densities

Entry page: [C# SDK guide](csharp-sdk.md).

A Spatial session draws its voxels as cubes (`GreedyCubes`) or as a surface
reconstructed from them (`MarchingCubes`, `DualContouring`). The product picks
the session's mode, the mode and character of each material, and may give
voxels a density that places the surface between voxel centres. Collision,
raycasts, character movement and collision navigation follow what is drawn.

Three routes reach a smooth, editable world, and they can be mixed:

- **Per-material surfaces** in a voxel session: ground materials dual
  contoured, building materials as cubes or as crisp dual-contoured blocks.
  Edits stay ordinary voxel edits.
- **Densities** in the same voxel session: each voxel may carry a float that
  moves the surface between voxel centres, written by region or blended with
  sphere and box brushes (blasts, fills, smoothing). Chunks stream, rebase and
  rebuild incrementally like any voxel chunk.
- **Sampled volumes** (`ImplicitSurfaces`): a dense scalar lattice with a
  material per sample, meshed whole or in blocks into ordinary `MeshResource`s
  that the product draws and admits as static-mesh collision. See
  [implicit surfaces](csharp-implicit-surfaces.md#retained-sampled-densities).

## Surface modes and characters

`SpatialSessionConfig.VoxelSurfaceMode` is the mode of every material without
its own surface. `Voxel.ConfigureMaterialSurfaces(new(session, mode,
materials))` replaces the session's mode and each listed slot's
`VoxelMaterialSurface(slot, mode, character)` at any time: every chunk is
remeshed, its collision rebuilt, and the next collision-navigation publication
derives everything. The returned `VoxelSceneReadout` reports the rebuild.

A `SurfaceCharacter(placement, creaseAngleDegrees, roughness)` shapes a
reconstructed material:

- `VertexPlacement.Sharp` (default) minimizes the error to the crossing planes,
  keeping corners and edges at any angle. `Smooth` uses the mean of the
  crossings: rounded, blob-like surfaces. `Blocky` keeps the material on the
  voxel grid: its crossings sit on voxel faces whatever the densities, and the
  normals of a cell it wins snap to the axes, so a cube of material meshes as
  an exact cube and a neighbouring material meets its planes. Use Sharp to
  shape sharp features with densities instead.
- `CreaseAngleDegrees` shades a vertex smooth where the facet bends less than
  this from the vertex's interpolated normal, and flat beyond it: 0 is every
  facet flat, 180 everything smooth. `SurfaceCharacter.Default` is Sharp,
  180, no roughness.
- `Roughness` (0 to 0.5 of a cell) displaces each vertex by a deterministic
  amount chosen by its cell, so independently meshed chunks agree.

Where materials meet in one cell, the sharper placement (Smooth, Sharp,
Blocky, then a cube material) and the smaller roughness place the shared
vertex. A cube material beside a reconstructed one keeps all its faces that
are not against another cube, since the smooth surface need not cover them,
and the smooth surface meets the cube on its face planes. Marching-cubes and
dual-contoured materials are each closed against empty space and cubes but
are not stitched to each other.

Voxel cell states (orientation and variant) are cube-face facts and are
refused on reconstructed materials.

## Textures on reconstructed surfaces

Reconstructed surfaces carry tile coordinates like cube faces, projected along
the dominant axis of each polygon in the same absolute cell space and face
basis as a cube face of that direction. Groups are split per material and
face, so textured, atlas-tiled and direction-specific materials (a grass top
over dirt sides) draw on smooth surfaces with the same bindings as on cubes. A
Blocky material's planar faces texture exactly like cubes. On curved surfaces
the chart changes where the dominant axis does, which shows as a texture seam
on slopes near 45°.

For rock and other curved ground, give the material a nonzero
`TriplanarSharpness` (`AuthoredMaterialAppearanceRequest`, or
`MaterialRequest` for a mesh). The shader then samples the texture, and its
normal map, from three axis planes in each face direction's texture basis,
over absolute cells (object cells on a voxel object), and blends them by
the surface normal: weights are |normal| to the sharpness, so 1 blends widely
and larger values keep each plane longer, nearer box projection; 4 is a
reasonable start. There is no seam, at the price of three samples per map, in
a shader variant only triplanar materials compile. An axis-aligned face takes
its one plane whole, so cubes and Blocky faces keep their tiles exactly. The
planes follow the voxel grid, not a block's orientation state, and a
direction-specific material on a smooth surface is chosen by the polygon's
dominant axis as before, then blended across its planes. On a retained mesh
the planes run over its object-space positions in metres.

## Densities

A density is signed, negative inside, in voxel units. A voxel without one
reads -0.5 when solid and 0.5 when empty, which puts the surface on the cube
face between them, so a session without densities draws exactly as before.
Solidity is authoritative: a density's magnitude places the surface, and its
sign follows whether the voxel is solid.

`Voxel.ApplyDensityEdits(new VoxelDensityTransaction(session, edits,
densities, materials))` applies edits in order:

- `VoxelDensityEdit.Region(min, size, densityOffset, densityCount,
  materialOffset, materialCount)` replaces a box of densities (x-fastest) with
  a range of the transaction's densities. A negative density makes its voxel
  solid with the matching material, or keeps the voxel's own material when no
  materials are given; an empty voxel turning solid needs one.
- `VoxelDensityEdit.Sphere(center, radius, operation, materialSlot, strength)`
  and `Box(min, max, ...)` blend a brush in the scene's local frame. `Add`
  takes the union (newly solid voxels take the material), `Subtract` carves,
  `Smooth` moves densities inside the brush toward their neighbours' mean by
  `strength`, and `Paint` changes the material of solid voxels inside it.
  Add and Subtract also update voxels within two voxels of the brush so the
  surface beside it is placed exactly.

Only the touched chunks' meshes and colliders, and navigation cells around
voxels whose solidity changed, are rebuilt; a failed rebuild restores the
scene. The receipt reports changed voxels, solidity changes, rebuilt chunks
and meshing time. `Voxel.ReadDensities(new(session, min, sizeX, sizeY,
sizeZ))` reads densities and materials back (borrowed until the next Voxel
call). Residency operations may carry a chunk's densities through
`DensityOffset`/`DensityCount` into the transaction's `Densities`; they must
be negative exactly where the chunk's material slots are solid. A classic
voxel edit keeps the edited voxel's density magnitude.

## Collision follows the drawn surface

In a session with any reconstructed material each chunk collides with what it
draws: a cuboid for every cube-material voxel and for every reconstructed
voxel that no surface passes through (all 26 neighbours solid, or beside a
non-collidable solid), plus the chunk's reconstructed triangles. Each
triangle belongs to a voxel (the solid end of its dual-contoured edge, or the
first solid corner of its marched cell), so `CastRay` and picking still name
a voxel, a face (the axis nearest the surface normal) and the normal itself.
Cuboids are merged into boxes; a ray into a box names the voxel under the
impact. Character casts, overlaps, Dynamics and collision navigation use the
same shapes. Point and box queries in the shell between the surface and the
interior cuboids classify by the side of the nearest surface triangle.
Non-collidable materials contribute neither cuboids nor triangles. A chunk's
parts are rebuilt with its mesh and kept when unchanged.

Collision navigation samples a support's height and slope from the hit normal,
and where a curved or filleted floor (the rounded foot of a dual-contoured
riser) rises under the standing capsule's rim it rests the capsule on the
surface, within one step height. A stair of one-voxel risers drawn by dual
contouring is a run of rounded steps: its cell-centre heights vary by a few
centimetres, so a step height tuned to exactly one voxel can refuse an edge.
The voxel navigation projection (`navigation_step`) stays a voxel-cell
projection; collision navigation follows the surface.

## Cost

Reconstructed chunks mesh from the chunk and a one-voxel halo of its
neighbours, in parallel when a change rebuilds several chunks. On a 180-chunk
dungeon of 16³ one-metre chunks (`svc-mesh` example `smooth_chunk_meshing`,
release build) dual contouring costs about 0.3 ms per chunk, marching cubes
0.5 ms and cubes 0.4 ms. A chunk's surface depends on all 26 neighbours, so
admitting a world chunk by chunk remeshes each chunk several times; admitting
that dungeon in slices of six chunks (`engine-spatial` example
`smooth_residency_load`) takes about 0.43 s with dual contouring, 0.63 s with
marching cubes and 0.40 s with cubes, collision and navigation included. Reconstructed vertices are
split per texture face and crease, so a smooth chunk draws more vertices than
its cell count suggests. `VoxelSceneReadout.MeshMicroseconds`, and the same
field on edit, residency and density receipts, report the meshing time of the
chunks the last change rebuilt, summed over chunks.
