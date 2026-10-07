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
  shape sharp features with densities instead. Dual-contoured Blocky faces
  without roughness that lie in one plane are merged into rectangles, as
  cube faces are, wherever every corner they use belongs only to such faces
  and lies inside the chunk: faces beside another surface or at the chunk's
  border keep the corners they share, so nothing meets a T-junction.
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
over dirt sides) draw on smooth surfaces with the same bindings as on cubes;
neighbouring face groups that resolve to one material are drawn as one. A
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
the planes run over its object-space positions in metres, repeating once per
metre unless `MaterialRequest.TextureScale` sets another repeat.

## Blending terrain layers

Where two materials meet on a reconstructed surface their textures change at
a polygon edge. To blend them instead, name up to four material slots as
terrain layers and draw them with one terrain layer material:

```csharp
engine.Voxel.ConfigureTerrainLayers(new VoxelTerrainLayerRequest(
    session, new uint[] { SandSlot, RockSlot }, TransitionCells: 2));
Material terrain = engine.Graphics.CreateTerrainLayerMaterial(
    new TerrainLayerMaterialRequest(sand, new[] { rock }, Contrast: 2));
// Bind `terrain` to both SandSlot and RockSlot in the voxel scene presentation.
```

Several physical slots can draw as one layer, so grass and the dirt under it
keep their own identities and still blend as one texture. `Layers` gives the
layer (0 to 3) of each slot at the same index, for up to 16 distinct slots:

```csharp
engine.Voxel.ConfigureTerrainLayers(new VoxelTerrainLayerRequest(
    session,
    Slots: new uint[] { GrassSlot, DirtSlot, StoneSlot, SandSlot, SnowSlot, GravelSlot },
    TransitionCells: 2,
    Layers: new uint[] { 0, 0, 1, 2, 3, 3 }));
// Bind the four-layer material to all six slots.
```

The product still decides which material each voxel is (biome, slope, height
or noise); the Engine only blends what it chose:

- **Weights.** Each reconstructed vertex weighs the solid voxels of the layer
  slots within `TransitionCells` voxels of it (1 to 4, and less than the chunk
  size less one), nearer ones more. The weights come from absolute voxel
  positions and the voxels on both sides of a chunk seam, so a seam vertex has
  the same weights in both chunks, and a world-origin rebase changes none. An
  edit within reach of a chunk border remeshes the neighbour as well. A vertex
  with no layer voxel in reach, and every cube face, takes its own slot's
  layer whole. Without `Layers`, the slots are layers 0 to 3 in order. A
  duplicate slot, a layer past 3, or `Layers` of another length than `Slots`
  is refused and the previous layers stay. No slots removes the weights.
- **Material.** `CreateTerrainLayerMaterial` takes the base material as layer 0
  and 1 to 3 more (`Layers`), all voxel surface materials. Each layer keeps
  its own texture, repeat or atlas tiling and normal map, as they are when the
  material is made; the base gives the rest, triplanar sharpness included. The
  material keeps those textures while it lives. `Contrast` (1 or more) raises
  the weights to that power before normalizing them: 1 blends them as they
  are, higher narrows each transition toward the dominant layer. Width comes
  from `TransitionCells`, sharpness from `Contrast`, and texture scale and
  triplanar sharpness from the layer materials, each independently.
- **What stays.** Geometry, groups, material slots, collision, navigation and
  voxel readouts are unchanged: blending is drawn only. A slot outside the layers, bound to an
  ordinary material, draws as before.
- **Cost.** Meshing reads up to (2 × `TransitionCells`)³ voxels per vertex
  and looks each voxel's slot up in the set.
  The material samples every layer's texture and normal map (four layers: 4×
  the samples, 12× with triplanar planes), in a shader variant only terrain
  layer materials compile. Layer materials are opaque.

## Vertex occlusion

Corners, the foot of a wall and the inside of a crevice read deeper when the
ambient light there is less. `engine.Voxel.ConfigureVertexOcclusion(new(session,
Strength: 1))` darkens every vertex of the session's surfaces by the solid
voxels around it at mesh time, with no screen-space pass: a reconstructed
vertex looks out along a fan of directions over its normal (straight out,
and two rings at 45 and 75 degrees from it) at three quarters of a voxel and
one and three quarters, and counts the solid voxels it meets, nearer ones
weighing more; a cube face's corner takes the classic voxel rule from the
two voxels beside it across the face's plane and the one diagonal, and
greedy faces merge only with equal corners, so a merged quad keeps each
corner's own value. `Strength` scales the darkening (0, the default, turns it
off and costs nothing; 1 applies it fully). The value rides the chunk's
vertex colour alpha and scales the ambient, hemisphere, sky and probe light
the standard shader gives the surface, as the occlusion map and
[screen-space occlusion](lighting-and-sky.md#contact-darkening-screen-space-ambient-occlusion)
do; the sun and lamps are unchanged. Values come from absolute voxel
positions and the same voxels on both sides of a chunk seam, so neighbouring
chunks give a shared vertex the same value and a world-origin rebase changes
none; an edit within three voxels of a neighbouring chunk remeshes it, as a
terrain transition does. Geometry, material slots and collision are
unchanged. With terrain layers, the fourth layer's weight becomes the
remainder of the first three, so the alpha can carry the occlusion. Coarse
(distant) chunks draw without it. Cost: about 0.15 ms more per 16³ chunk
dual contoured on the `smooth_chunk_meshing` example (0.31 to 0.47 ms),
0.07 ms marched; cube chunks mesh into more quads where corners differ.

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

`Voxel.StampImplicit(new VoxelImplicitStampRequest(session, field, node,
boundsMin, boundsMax, operation, strength, materialSlot))` stamps any shape an
implicit field can describe (`ImplicitRecipe`: capsules, smooth unions,
offsets, placed and wave-roughened shapes) as one of these brushes. The Engine
samples the node at the centre of every voxel whose centre lies within the
bounds (grown by the two-voxel margin for Add and Subtract), divides each value
by the field's gradient so it reads as a distance in voxels even where the
field grows faster or slower than distance (a scaled or wave-displaced shape),
clamps it to four voxels either side of the surface, and applies it as a brush
of that shape: an implicit sphere stamps exactly what the sphere brush does.
The bounds must enclose the shape; anything outside them is untouched. The
field stays owned by ImplicitSurfaces and is only read during the call, so a
recipe can be disposed right after. On `fixtures/csharp-voxel-stamp`, carving
a smooth union of three capsules (42,369 voxels changed, 32 chunks rebuilt)
takes about 37 ms, 12 of them meshing; in a CraftSurvive dungeon a rounded
chamber took 5.8 ms by stamp against 10.5 ms for the same cut read, computed
and written back as a region from C#.

Only the touched chunks' meshes and colliders are rebuilt; a failed rebuild
restores the scene. The receipt reports changed voxels, solidity changes, rebuilt chunks
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
Cuboids are merged into boxes, and a merged Blocky rectangle is owned by the
voxels under it; a ray into either names the voxel under the impact. Character casts, overlaps, Dynamics and collision navigation use the
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

## Distance level of detail

`VoxelScenePresentation.SetLevelOfDetail(new(presentation, coarseDistance))`
draws a presentation's distant chunks from coarse meshes. Each update, the
Engine measures every chunk's cube from the camera of the lowest-ordered
primary view; a chunk farther than `coarseDistance` metres (by 10% more when it
switches, so a camera at the boundary does not flip it) is drawn coarse. Zero
draws every chunk at full resolution. Set the distance whenever the view calls
for another (a map view, open ground, caves): a call projects only the chunks
that change level, and repeating a distance costs nothing. Only the drawing
changes: collision, raycasts, picking and navigation keep every chunk's full
mesh.

- A coarse mesh is the chunk's reconstructed materials meshed from a lattice
  twice as coarse. Each sample stands for a 2 × 2 × 2 block of voxels, solid
  only where the whole block is, with its majority material and smallest
  density, so the coarse surface lies on or inside the fine one. Tile
  coordinates, triplanar textures and terrain layer weights stay in voxel units
  and match the fine mesh. Cube materials keep their full-resolution faces.
- Coarse chunks meet each other without seams, as fine chunks do. Against a
  fine neighbour every open edge of a coarse mesh carries a skirt reaching one
  coarse cell into the solid and as far toward the neighbour, which closes both
  the step and the gap where the two surfaces stop short of each other.
- A coarse mesh reads two voxels into each neighbour, so a change to the
  voxels of the chunk or of any neighbour rebuilds it; a neighbour remeshed
  only for its seams changes nothing. An edit rebuilds it with the same
  projection. A chunk admitted or evicted within reach, as a stream does
  column after column, rebuilds it once the stream pauses: with the second
  consecutive projection that admits or evicts nothing within reach (the
  first is the update's own level-of-detail settle). A streamed chunk is
  thus meshed coarse once its neighbours have arrived rather than once per
  neighbour. Until then a chunk newly drawn coarse shows its full mesh and
  one already coarse keeps its coarse mesh. Chunks rebuilt together are
  meshed in parallel.
- Sessions whose materials are all cubes, and chunks with an odd edge, are
  always drawn at full resolution. `VoxelScenePresentationReadout.
  CoarseChunkCount` reports how many chunks are drawn coarse, and
  `CoarseMeshMicroseconds` the time the last projection spent meshing coarse
  chunks, summed over chunks.

## Scatter

Grass, bushes, stones and flowers grow on a presentation's ground through
`engine.VoxelScenePresentation.SetScatter(new VoxelSceneScatterRequest(
presentation, scatter, appearance, material, slots, density, radius))`. The
product names what grows (a static-mesh `Appearance` drawn with a
`Material`), on which material slots (all when none are named), how densely
(copies per square metre of ground) and how far from the camera of the
lowest-ordered primary view; `with` sets the rest: `Fade` (the last metres of
the radius over which copies shrink into the ground, a quarter of it by
default), `ScaleMin` and `ScaleMax`, `TintLow` and `TintHigh` (linear colours
each copy's colour is drawn between), `SlopeLimitDegrees` (35 by default),
`Align` (0 stands copies upright, 1 along the ground's normal),
`CastsShadows` (off by default), `MaximumInstances` and `Seed`. `scatter` is the
product's key: setting it again replaces that scatter, and
`RemoveScatter(new(presentation, scatter))` removes its copies. The Engine owns
the rest:

- **Where.** On every full-resolution chunk within the radius, the Engine
  samples the chunk's drawn triangles: a jittered grid over the ground in
  absolute coordinates gives each grid cell one candidate spot, kept on every
  upward triangle of a named slot, within the slope limit, whose footprint
  holds it (`svc_mesh::scatter`). So the same ground grows the same copies
  whichever chunk meshed it and every time it comes back into reach, ground is
  covered evenly whatever its triangles' sizes, and a ledge over the ground
  grows its own. Each copy's scale, tint and turn about its up axis come from
  a hash of its spot.
- **Drawing.** A chunk's copies of one scatter are one scatter patch, a child
  of the chunk's node: one instanced draw per mesh group, culled by the
  patch's bounds (the GPU cull, when on, culls each copy). Copies are rows of
  the patch, never nodes, entities, colliders or pickable, and the indirect
  light volume does not see them. They shrink toward their origin over the
  fade distance from the camera, in the vertex stage and the shadow pass
  alike, so nothing pops at the edge of the radius. The material's wind
  sways them: bend grows with height above each copy's own origin.
- **Following the ground.** A patch is placed when its chunk comes within
  the radius and removed when the chunk is 10% farther (so a camera at the
  boundary does not rebuild it), when the chunk is drawn coarse (distant
  chunks grow nothing), or when the scatter changes; an edit or remesh places
  the chunk's patches again, and a world-origin rebase moves them with their
  chunk. Nearer chunks come first within `MaximumInstances`; a chunk the budget
  leaves bare stays bare until the camera moves.
- **Lifetime.** The scatter keeps a copy of the material from when it was
  set, and holds the appearance: disposing the appearance while a scatter
  grows it is refused. Every mesh slot draws with the scatter's material.

`VoxelScenePresentationReadout.ScatterPatchCount`, `ScatterInstanceCount` and
`ScatterOverBudgetCount` report the patches, the copies they hold and the
chunks a budget left bare. Sampling costs about 0.012 ms per 16³ dual
contoured chunk at 4 spots per square metre on the `smooth_chunk_meshing`
example (meshing the chunk costs 0.17 ms), and runs only for chunks coming
into reach, not at mesh time: the samples are not stored with the chunk.
Drawing is the cost that matters: masked grass cards are overdraw. On
`fixtures/csharp-voxel-scatter`, which grows grass clumps and bushes on a dual
contoured hill, 8,725 visible copies in 35 patch draws take a 1280 × 720 frame
from 0.68 to 0.96 ms on an RX 9070 XT and from 13 to 64 ms on llvmpipe.

## Cost

Reconstructed chunks mesh from the chunk and a one-voxel halo of its
neighbours. A change builds each rebuilt chunk's mesh and collider together, in
parallel when it rebuilds several chunks. On a 180-chunk dungeon of 16³
one-metre chunks (`svc-mesh` example `smooth_chunk_meshing`, release build)
dual contouring costs about 0.18 ms per chunk, marching cubes 0.25 ms and cubes
0.14 ms: a cube chunk's face tests read its 26 neighbours through references
resolved once per chunk. One residency, edit or density call meshes each chunk
it touches once, after all its writes. A chunk's surface depends on all 26
neighbours, though, so each later call remeshes the resident neighbours of what
it changes. Admitting that dungeon in one call (`engine-spatial` example
`smooth_residency_load`) takes about 27 ms with dual contouring, 39 ms with
marching cubes and 22 ms with cubes, collision included. In slices of six
chunks it builds 714 chunk meshes (474 with cubes) and takes about 0.12 s, 0.18
s and 0.04 s in all, at most about 7, 9 and 2 ms per slice. A product filling a
space behind a loading wait trades the longest call against the total through
its slice size; larger, contiguous slices leave fewer resident neighbours to
remesh. Reconstructed vertices are split per texture face and crease, so a
smooth chunk draws more vertices than its cell count suggests. A coarse mesh
takes about 0.15 ms per chunk with dual contouring and 0.2 ms with marching
cubes, and draws about a third of the triangles (35% and 30% on that dungeon,
skirts included; a quarter without). `VoxelSceneReadout.MeshMicroseconds`, and
the same field on edit, residency and density receipts, report the meshing time
of the chunks the last change rebuilt, summed over chunks.
