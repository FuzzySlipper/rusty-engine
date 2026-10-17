# Vegetation fields

`engine.Vegetation` grows grass, plants and placed things such as trees over
ground the product samples, in square tiles about a camera. The product
decides what grows where: ground classes, cover, densities and its own
placement records. The Engine keeps the tiles resident about the focus,
places each layer's copies on worker threads within budgets, and draws them as
retained instanced scatter patches. Nothing here needs voxel or collision
residency: a field grows as far over distant heightfield ground as over
walking ground.

```csharp
using Rusty.Engine.VegetationTiles;

VegetationField field = engine.Vegetation.Create(new VegetationFieldCreateRequest(
    default, TileSize: 32, TileCells: 16, VegetationFocus.Following(camera)));
engine.Vegetation.SetLayer(new VegetationLayerRequest(field, Layer: 1, grassLook, grassMaterial,
    Density: 2.5f, Reach: 256) with { Grounds = VegetationLayerRequest.Ground(0), Seed = 100 });
var streamer = new VegetationStreamer(engine, field, new MyGround(), beforeCommit: MaskEdits);
// Each update:
streamer.Update();
```

`fixtures/csharp-heightfield` grows grass and trees over its hills with two
fields that hand over to each other (`vegetation.inspect`,
`vegetation.tune <density> <reach>`, `vegetation.invalidate <metres>`).

## Fields and tiles

A field covers a spatial session's world (`default`: the scene's own space)
in square tiles of `TileSize` metres, each sampled `TileCells` cells a side
(1 to 256). With a session, the field's root stands at the world's origin in
that session's frame and follows every world-origin rebase. Tile x and z are
world metres, and a tile's first vertex stands at a multiple of `TileSize`.

`VegetationFocus.Following(camera)` centres the tiles on where that camera
stands, measured as each call finishes; `VegetationFocus.At(x, y, z)` on a
world point, which `SetFocus` moves. A tile is wanted while its nearest point
lies within the farthest layer's reach of the focus, horizontally, and kept to
1.1 times that, so a focus wavering on the edge samples nothing twice.

A product may make several fields. One lattice rarely suits every distance:
a fine one near the walking ground and a coarse one matching a distant
heightfield ring are two fields, and their layers hand over (below).

## Sampling

`ReadRequests` lists the wanted tiles without current samples, nearest the
focus first. A `VegetationTileRequest` gives the tile's column and row, its
first vertex (`OriginX`, `OriginZ`), `CellSize`, `TileCells` and a
`Revision`. `CommitTile` takes, as [heightfield tiles](heightfield-terrain.md#sampling)
are sampled:

- `Heights`: `(TileCells + 3)²` world heights from vertex (−1, −1), x
  fastest; NaN is no ground. The extra ring gives normals that match across
  tile edges.
- `Grounds`: a ground class (0 to 63) per vertex, `(TileCells + 1)²`; class 0
  when empty.
- `Cover`: a weight 0 to 1 per vertex, `(TileCells + 1)²`: the share of every
  layer's spots kept there (1 when empty). It thins a biome edge or a trodden
  yard without a hard line.
- `Records`: things the product places itself: a layer, x and z from the
  tile's origin, the world y, a yaw and a scale. Precomputed world-generation
  tiles stream through here, read when their tile is asked for and never all
  held at once.

Samples for a revision that is no longer the tile's are ignored
(`Accepted: false`). `Invalidate(rectangle)` asks again for every held tile
overlapping it: an edit, a moved sink, a felled forest. Each keeps drawing
its copies until its new samples land, and only those tiles are placed again
(`InvalidatedTiles`, `ReplacedPatches`).

**`VegetationStreamer`** (`Rusty.Engine.VegetationTiles`) samples requested
tiles with an `IVegetationSampler` on the thread pool, at most twice the
processors at once, and commits at most `commitsPerUpdate` (16) finished ones
an update. The sampler runs on workers: it reads only product data nothing
changes under it and calls no Engine service ([call
affinity](world-streaming-contract.md#engine-call-affinity-8611)). The
optional `beforeCommit` runs on the update's thread just before each commit,
where the product may read state its workers may not, such as player edits:
zero `Cover` where a pit was dug, set a class where a floor was built.

## Where copies stand

A layer's spots come from the absolute jittered grid voxel scatter uses
([scatter](smooth-voxel-surfaces.md#scatter)): one candidate a cell of
`1 / √Density` metres, drawn from the cell and the layer's key and seed. A spot
grows where its tile's surface is no steeper than `SlopeLimitDegrees`, the
nearest vertex's class has its bit in `Grounds`, and the nearest vertex's
cover keeps it. Its height is the surface a heightfield of the same lattice
draws, each cell split along the diagonal whose ends are nearer in height,
so copies on a lattice matching a heightfield ring stand exactly on its
triangles. A finer vegetation lattice than the drawn ground stands on the
sampled ground instead: sample what the product draws there.

Each copy is turned at random about the vertical, leaned `Align` of the way
onto the ground's normal, scaled between `ScaleMin` and `ScaleMax` and tinted
between `TintLow` and `TintHigh`. A record keeps its own yaw and scale.

## Layers, reach and handover

`SetLayer` names a layer within its field; setting the key again replaces it
and its copies are placed again as calls allow, drawing as they were until
then. Copies stand on tiles within `Reach` metres of the focus and shrink into
their spots over its last `Fade` metres. Nearer than `Near` they are not
drawn, growing back over the next `NearFade` metres; a tile wholly nearer than
`Near` is not placed at all. Distances are 3D from the camera to each copy.

The same spots grow wherever a layer of the same key, seed and density grows:
in another field, or as a voxel scene scatter of the same key. So a near
source fading out over a band and a farther layer whose near band is that band
hand their copies over one for one, with nothing doubled and no bare ring.
For a voxel scatter of radius `R` and fade `f`, set the layer's `Near` to
`R − f` and `NearFade` to `f`; for a nearer field's layer of reach `R` and
fade `f`, the same.

## Levels of detail and thinning

`SetLayerDetail(new VegetationLayerDetailRequest(field, layer, lods, lodBand,
thinning, maxScale))` gives a set layer up to three farther levels and its
thinning; setting the layer again keeps them.

- **Levels.** `VegetationLod.Mesh(appearance, start)`: from `start` metres a
  copy draws that mesh in place of the level before. Within `lodBand` metres
  before the start, it draws at both levels, which share each pixel's samples
  by an ordered dither (complementary, so no pixel is covered twice). No
  level shrinks or fades into the ground. A level's appearance and material
  are held while the layer grows them. A `default` material (also the
  layer's own) draws each mesh slot with the mesh's own material, so a tree's
  bark and leaves keep theirs.
- **Impostors.** `VegetationLod.ImpostorOf(appearance, start)`: from
  `start`, a copy draws as a card facing the camera that shows the mesh from
  the nearest of 64 views the renderer bakes of it (a hemi-octahedral grid
  over the upper half of its bounding sphere, 64 texels a view, 2 MiB an
  impostor), lit by
  the baked normals turned with the copy. The bake draws each mesh group
  with its own material once per mesh and set of materials, when a layer
  first needs it; the atlases are derived from the mesh and its materials
  alone and are not saved. A card casts no shadow: a copy at an impostor
  level casts at the last mesh level, so keep the shadow distance short of
  where that costs too much. `engine.renderer`'s `gpu.impostors` reports
  the impostors baked, their atlases' bytes and the last bake's time.
- **Thinning.** Up to four `VegetationThinningPoint(distance, share)`, in
  increasing distance: the share of copies kept, linear between points and
  level beyond them. A copy is kept while its rank (drawn from its spot,
  the same for a voxel scatter's copy of the spot) is below the share. Kept
  copies grow by the inverse square root of the share, at most `maxScale`
  (1 to 4), so the ground stays as covered while the count falls. The
  product chooses the profile per layer: a prairie thins slowly to a far
  reach; a forest floor keeps everything to a short one.

The renderer selects copies one by one on the GPU each frame (with compute
and indirect draws): within the fade, past the near cut, kept by thinning,
inside the view, at the level their distance reaches. Copies outside the
view, past reach or thinned away cost no vertex work, and the CPU's draw
lists hold no copy at all. Every tile's copies of one mesh, material and
pipeline draw together, so the draws follow the layers' meshes and levels,
not the tiles. The meshes they draw are copied into one shared vertex and
index buffer, so the draws of one pass, pipeline and material (the variants
of a species with the same bark and leaves) are one multi-draw command. Every world shadow layer takes the casting copies the same
way without the view test: the sun's cascades those within their reach (the
light's `range`), and a point, spot or ambient light's layer those inside
it. Without compute, copies draw at their first level (the readout's
`scatterSelection.refused` says why).

### Occlusion

With the renderer setting `OcclusionCulling` (`RendererSettings.Set(...
with { OcclusionCulling = true })`, or the player's video option *Vegetation
occlusion*), each world view first draws its occluders depth-only at its
world pass's size and samples per pixel, reduces that depth (each pixel's
farthest sample) to a farthest-depth pyramid, and drops from the camera's
list every copy whose bounds lie wholly behind it. The occluders are its
opaque parts other than scattered copies whose materials keep every sample
they cover: a masked material never occludes, since the world pass may
share its samples out differently, nor does one with a product shader, whose
shade stage may discard. Sampling as the world pass does, the depth keeps
every opening a world-pass sample sees through, however narrow. With GPU culling on, the culled candidates do not occlude.
The depth is the frame's own, so nothing the selection keeps appears a
frame late, and camera motion, origin rebases and edits need nothing more.
Scattered copies never hide each other, and the shadows' list is not
tested.

The depth pass and pyramid cost about 0.14 ms of GPU at 1280×720 with 4×
antialiasing on an RX 9070 XT, in the `occlusion` pass, and the renderer
CPU that encodes every opaque part once more: 0.6 to 0.85 ms in a
CraftSurvive forest. It pays where
opaque surfaces hide many copies at once. In CraftSurvive, looking past a
near crown hid 75% of the drawn copies and the world pass fell from 3.65 to
3.31 ms; inside a dense forest (25% hidden), up a long slope with forest
beyond it (19%) and from a raised vista (3%) the change stayed within the
run-to-run spread. The picture was the same in every case. A copy hidden
behind terrain mostly costs vertex work only, since the depth test already
discards its pixels, so the gain follows the number of hidden copies more
than their area.

## Budgets and readout

- `MaximumInstances`: the layer's copies are given to its tiles nearest
  first; a tile that does not fit stays bare and is counted (`OverBudget`).
- `PlacementsPerCall` (64): the most tile layers a call places. Placing runs
  on worker threads as the call finishes; what waits is counted (`Deferred`)
  and placed by the next calls, nearest first.
- Choosing again: a call chooses its tile layers again only when the field
  changed (a layer set or removed, a tile committed or invalidated), work
  was left waiting, or the focus moved a sixteenth of a tile from where it
  last chose. Otherwise it costs nothing. Choosing, it widens every reach by
  that much, so what it keeps covers any focus within it; the copies' fades
  are drawn from the camera itself.

`Read` (and every call) returns the field's readout: tiles held and waiting
(`Tiles`, `PendingTiles`), patches and copies drawn, `OverBudget`, `Deferred`,
the last call's placements (`Placed`, `PlacedInstances`) and its time placing,
choosing what is drawn and turning that into renderer work
(`PlacementMicroseconds`, `SelectionMicroseconds`, `PublicationMicroseconds`),
`InvalidatedTiles`, `ReplacedPatches`, `SampleBytes`, and the calls that
had nothing to choose again and those that chose again (`SettlesSkipped`,
`SettlesReselected`). The renderer's side
of the upload is `engine.renderer` statistics' `scatter`: patches and copies
taken in since it was made, the CPU time turning them into drawable parts and
the GPU row bytes they upload; read it twice and subtract for a span.
`engine.renderer`'s `gpu.scatterSelection` reports the last world view's
selection: runs, parts, draws and copies selected, the commands its draws
take and the bytes of their mesh pool (`commands`, `poolBytes`), and (a
frame later) copies
tested, drawn (a crossfading copy twice), drawn for shadows, thinned away,
outside the view and hidden behind its depth (`occluded`), with draws at
levels 1 to 3; its GPU time is the `scatter-select` pass. `gpu.occlusion`
reports the occlusion test: on, its pyramid's size and levels, and its
depth's samples per pixel; its GPU time is the `occlusion` pass.

Copies are never nodes, entities, colliders or pickable, and they cast
shadows only when the layer asks.
