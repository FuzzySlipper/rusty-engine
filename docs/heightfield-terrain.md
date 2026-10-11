# Heightfield terrain

`engine.Heightfield` draws a product's 2.5D ground (distant ranges, a world
map, an open far field) as rings of grid tiles about a focus. The product owns
the height function and the world map and samples tiles when the Engine asks;
the Engine owns residency, meshing (on its own threads), GPU resources, level
of detail and the geomorph that hides it, and picks the drawn surface. A
heightfield is drawn, not collided: collision and navigation stay with Spatial.

```csharp
using Rusty.Engine.Heightfields;

HeightfieldRing[] rings = [new(1, 48), new(4, 400), new(32, 4000)];
Heightfield ground = engine.Heightfield.Create(new HeightfieldCreateRequest(
    TileCells: 32, rings,
    Materials: new HeightfieldMaterial[] { new(0, groundMaterial), new(1, waterMaterial) },
    RenderLayer.Scene, Origin: Vector3.Zero, Scale: 1, HeightfieldFocus.Following(camera)));
var streamer = new HeightfieldStreamer(engine, ground, new MySampler());
// Each update:
streamer.Update();
```

`fixtures/csharp-heightfield` flies a camera over hills, ranges and a lake in
three rings, in either colour mode.

## Rings

A ring is a cell width and a reach, in heightfield units, finest first; every
tile is `TileCells` cells a side (1 to 256), so each ring's tile is
`CellSize × TileCells`. Up to eight rings.

- **Nesting.** Each ring's tile must be a whole number (2 or more) of the
  finer ring's tile; the cell widths need not divide (31.25 m inside 250 m
  works with the same `TileCells`). A ring covers a square of its coarser
  ring's tiles about the focus (the coarsest, a square of its own), out to
  its reach rounded up to whole tiles.
- **Geomorph.** Every vertex of a ring that has a coarser one also carries
  the coarser ring's drawn surface where it stands (its height and normal).
  The renderer moves it there with distance from the focus (the larger of the
  x and z distances): whole within the morph's start, wholly the coarser
  surface from its end. The end is as far as the ring reaches from whichever
  tile the focus stands in; the band is the coarser ring's tile, or less
  where that would reach the finer ring. So tiles that join or leave a ring as
  the focus moves are drawn exactly as the coarser surface they replace, the
  finer ring's edge lies on the coarser ring's grid lines, and the coarser
  ring is unmorphed wherever a finer ring ends. Nothing pops and the product
  sinks nothing.
- **Refusals.** A ring whose tile is not a whole number of the finer ring's,
  or that does not reach far enough past the finer ring to morph onto it, is
  refused with `CSHARP_HEIGHTFIELD` and names the ring. A ring must reach
  (in whole tiles of the ring above it) farther than the finer ring reaches
  (in whole tiles of its own) plus one of its own tiles.

## Sampling

`ReadRequests` lists the tiles the rings want that have no current samples:
coarsest ring first (so the ground is covered soonest), nearest the focus
first within a ring. A `HeightfieldTileRequest` gives the tile's ring and
index, its first vertex's place (`OriginX`, `OriginZ`), `CellSize`,
`TileCells` and a `Revision`. `CommitTile` takes:

- `Heights`: `(TileCells + 3)²` heights in heightfield units, from vertex
  (−1, −1), x fastest: the tile's vertices and one more each side, for normals
  that match across tile edges. NaN is no surface: cells touching it are not
  drawn (the map's edge).
- `Colors` (linear RGBA, drawn as given and multiplying the material's
  colour), or `LayerWeights` (a terrain layer material's first four layers,
  as on [voxel ground](smooth-voxel-surfaces.md#blending-terrain-layers)), or
  neither: `(TileCells + 1)²`, from vertex (0, 0). Every colour the
  heightfield takes is linear: decode a palette's sRGB colours with
  `Color.FromSrgb` ([colours](csharp-lifecycle.md#colours)).
- `Materials`: a material index per vertex, `(TileCells + 1)²`; 0 when
  empty. Every index used must be bound when the heightfield is made. A
  triangle draws with the index most of its corners have, or the highest of
  three different ones, so the product decides which wins where three meet.
  Each index draws whole with its own material: a lake, a road or a glacier
  is a hard edge. Only terrain layer materials blend, and only their layers.

Samples whose revision is no longer the tile's are ignored and the receipt
says `Accepted: false`: the tile was invalidated since, or it left the rings
and came back (revisions are unique to the heightfield). `Invalidate` asks
again for every held tile overlapping a rectangle; each keeps drawing until
its new samples land. A wrong count, colours and weights together, a value
outside 0 to 1, or an unbound index is refused.

**`HeightfieldStreamer`** (`Rusty.Engine.Heightfields`) does this for a
product: each `Update` commits at most `commitsPerUpdate` finished tiles (8
by default), then starts sampling new requests on the thread pool, at most
twice the processors at once. Its `IHeightfieldSampler.Sample` runs on
workers, several at a time: it reads only product data the product does not
change under it and calls no Engine service ([call
affinity](world-streaming-contract.md#engine-call-affinity-8611)). Every
Engine call stays in `Update`. A sampler that throws rethrows from the next
`Update`, and its tile is asked for again. `Committed`, `Ignored`, `InFlight`
and `SampleMilliseconds` report its work.

## Drawing

- **Meshing.** Tiles committed during a call are meshed as it finishes, in
  parallel: a vertex per sample, normals from the heights about it, each cell
  split along the diagonal whose ends are nearer in height (a ridge or a gully
  is not cut across), and a skirt hanging below each edge (as deep as the
  coarser ring's cell) to cover what a tile waiting for its samples would
  leave open. A tile below the coarsest ring meshes once its coarser tile has
  samples; new samples remesh the tile and the finer tiles within it.
- **What draws.** A ring draws the tiles of its region; a coarser tile keeps
  drawing until every finer tile within it is meshed, then gives way to them
  all at once, so the ground has no holes while a ring loads (after a teleport
  the coarsest ring covers everything first) and nothing overlaps. Tiles are
  kept a region tile beyond the region, so a focus wavering on a boundary
  samples nothing twice; beyond that they are let go.
- **Textures.** A tile's uvs are the heightfield's own x and z, which a voxel
  surface material reads as tile coordinates (`TileScale` cells a repeat),
  and triplanar materials project the heightfield's own positions: textures
  run on across tiles and rings.
- **Placement.** A heightfield draws in `RenderLayer.Scene` or
  `RenderLayer.Backdrop`. Its point (0, 0, 0) stands at `Origin`, one of its
  units spanning `Scale` layer units, never turned. With a `Session`, it
  stands at the world point `Anchor` resolved into that session's frame, again
  after every world-origin rebase, as a [backdrop link](lighting-and-sky.md#the-backdrop)
  does. A backdrop heightfield sampled in metres at 1:1000 takes `Scale`
  0.001; fog and shadows treat it as the backdrop does.
- **Shadows.** It casts unless made with `ShadowCasting.None`; it always
  receives.

## Focus

`HeightfieldFocus.Following(camera)` centres the rings on where that camera
stands, measured as each call finishes (so the requests of a heightfield made
during a call come with the next one); a backdrop heightfield measures where
the camera stands in the backdrop under its view's link. `HeightfieldFocus.At(x,
z)` centres them on a point of the heightfield, which `SetFocus` moves. Both
the tiles drawn and the geomorph follow the same focus.

## Pick

`Pick` meets a ray in the heightfield's layer with the surface as drawn now,
morph included: the hit in the layer, its normal, its distance in layer units,
and where it is in the heightfield (`X`, `Z`, `Height`) and on which ring.
`MaxDistance` 0 reaches as far as the heightfield does. It walks only the cells
the ray crosses in the tiles its bounds meet.

## Readout and cost

`Read` (and `SetFocus`, `Invalidate`) return the focus, the tiles held, drawn
and waiting, whether every tile the rings want is drawn (`Settled`), the
drawn geometry, the samples and meshes held in bytes, and the last call's
meshing time and tiles.

- **Meshing** a 16-cell tile takes about 0.04 ms of one core (release build,
  RX 9070 XT machine); tiles mesh in parallel as a call finishes. Sampling is
  the product's own cost, on its workers.
- **Memory.** A vertex costs 48 bytes on the GPU plus 12 for its morph target
  (rings with a coarser one), an index 4. The Engine also keeps each tile's
  samples (4 bytes a height, 16 a colour or weight set) and its mesh on the
  CPU, for remeshing finer tiles and for picking. A tile carries a skirt of
  `4 × TileCells` quads: small tiles pay proportionally more for it.
- **Tile size.** Rings are counted in their coarser ring's tiles, so a ring
  holds more than its reach where the coarser tile is large. Larger tiles
  mean fewer draws and fewer skirts but coarser rounding of every ring.
- **Measured** with the fixture's continent (390 km sampled in metres at
  1:1000 in the backdrop) on an RX 9070 XT at 1280 × 577, sampled by
  `HeightfieldStreamer` on a 20-thread processor:

  | rings | tile cells | tiles | vertices | backdrop pass | settled after a 130 km jump |
  | --- | --- | --- | --- | --- | --- |
  | 31.25 m to 4 km, 250 m to 32 km, 1 km to 195 km | 16 | 1,705 | 711,000 | 0.13 ms | 1.8 s |
  | the same | 32 | 945 | 1,271,000 | 0.22 ms | 1.2 s |
  | 31.25 m halving to 1 km, reach doubling from 4 km | 32 | 725 | 975,000 | 0.13 ms | 0.8 s |

  The settle time is the fixture's synthetic ground; a product's sampler
  sets its own.
