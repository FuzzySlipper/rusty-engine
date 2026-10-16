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

## Budgets and readout

- `MaximumInstances`: the layer's copies are given to its tiles nearest
  first; a tile that does not fit stays bare and is counted (`OverBudget`).
- `PlacementsPerCall` (64): the most tile layers a call places. Placing runs
  on worker threads as the call finishes; what waits is counted (`Deferred`)
  and placed by the next calls, nearest first.

`Read` (and every call) returns the field's readout: tiles held and waiting
(`Tiles`, `PendingTiles`), patches and copies drawn, `OverBudget`, `Deferred`,
the last call's placements (`Placed`, `PlacedInstances`) and its time placing,
choosing what is drawn and turning that into renderer work
(`PlacementMicroseconds`, `SelectionMicroseconds`, `PublicationMicroseconds`),
`InvalidatedTiles`, `ReplacedPatches` and `SampleBytes`. The renderer's side
of the upload is `engine.renderer` statistics' `scatter`: patches and copies
taken in since it was made, the CPU time turning them into drawable parts and
the GPU row bytes they upload; read it twice and subtract for a span.

Copies are never nodes, entities, colliders or pickable, and they cast
shadows only when the layer asks.
