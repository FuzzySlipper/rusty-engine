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
(`Origin` moves the root from there, and `Scale` makes a field unit other
than a metre; both are for the backdrop, below.)

`VegetationFocus.Following(camera)` centres the tiles on where that camera
stands, measured as each call finishes; `VegetationFocus.At(x, y, z)` on a
world point, which `SetFocus` moves. A tile is wanted while its nearest point
lies within the farthest layer's reach of the focus, horizontally, widened by
a sixteenth of a tile (see "Choosing again" below), and kept to 1.1 times that,
so a focus wavering on the edge samples nothing twice.

A product may make several fields. One lattice rarely suits every distance:
a fine one near the walking ground and a coarse one matching a distant
heightfield ring are two fields, and their layers hand over (below). A field
can also grow in the backdrop, over a backdrop heightfield (below).

`SetVisible(new(field, false))` stops drawing a field, shadows included,
for instance while the player is away in a separate space whose world
should not show. A hidden field keeps its tiles and patches and holds its
focus where it was, asking for and placing nothing new as its camera moves;
`SetVisible(new(field, true))` draws them again at once. The readout's
`Visible` says which.

## In the backdrop

A field made with `Layer = RenderLayer.Backdrop` grows in the
[backdrop](lighting-and-sky.md#the-backdrop), standing as a
[backdrop heightfield](heightfield-terrain.md#drawing) does: its point
(0, 0, 0) at `Origin` in the backdrop, one of its units spanning `Scale`
backdrop units. Everything the field takes or reports is in its own units:
tile size, focus, records, reach, fades, level starts, thinning distances and
a sprite's size. Grown over ground sampled in metres at 1:1000, a field takes
`Scale` 0.001 and stays in metres, so its layers read as a scene field's
would.

```csharp
VegetationField far = engine.Vegetation.Create(new VegetationFieldCreateRequest(
    default, TileSize: 256, TileCells: 8, VegetationFocus.Following(camera))
    with { Layer = RenderLayer.Backdrop, Origin = farGroundOrigin, Scale = 0.001f });
engine.Vegetation.SetLayer(new VegetationLayerRequest(far, Layer: 3, tree.Look,
    default(OptionalMaterial), Density: 0.002f, Reach: 20_000)
    with { Near = 3_000, NearFade = 300, Fade = 2_000 });
```

- **Focus.** `VegetationFocus.Following(camera)` measures where the camera
  stands in the backdrop under its view's link, again every call, so the tiles
  follow a world-origin rebase and a link change as the backdrop does.
  `VegetationFocus.At` takes a point of the field.
- **Sampling.** Tiles are asked for in field units: sample the far ground the
  backdrop heightfield draws, at the same points, and copies stand on its
  triangles when the lattices match.
- **Drawing.** Copies draw in the backdrop's pass, depth-tested against the
  backdrop heightfield, so a ridge hides what stands behind it. Fog reaches
  them at their world-equivalent distance, and the world draws over them.
  Sprite layers (`VegetationSprite`, either `BillboardMode`, `AuthoredFacing`,
  `Mask`) work as in the scene. Wind leans a copy by its height as in the
  scene, and its flutter is world-sized.
- **Handover.** A scene field's layer fading out at the near ground's edge
  hands over one for one to a backdrop layer of the same key, seed and
  density, as between scene fields, when a field metre is a world metre:
  `Scale` times the link's `Scale` is 1, and `Origin` is where the world's
  origin stands in the backdrop.
- **Selection.** Backdrop copies are selected copy by copy as the scene's
  are, from the backdrop camera: levels, crossfades, thinning and its
  scale-up, fades and the view test all apply, measured in field units. The
  backdrop's selection takes the instance room the world's leaves and has a
  GPU pass of its own, `backdrop-scatter-select`; `gpu.scatterSelection`
  stays the world's.
- **What it leaves out.** Occlusion culling does not test backdrop copies,
  and shadows from them are not supported: leave `CastsShadows` off.

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
affinity](world-streaming-contract.md#engine-call-affinity)). The
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

## Sprites

A layer or a level can draw a sprite atlas frame as a billboard: a flat that
stands upright at its copy's origin and turns about the vertical to face the
camera, so it never goes edge-on as a mesh or an impostor would. It turns as an
[upright sprite](csharp-capabilities.md#upright-sprites) does: toward the
camera's position (`BillboardMode.Cylindrical`, the default), or against the
camera's heading (`BillboardMode.CylindricalView`), so walking past a copy
does not turn it.

```csharp
using VegetationSprite flower = new(engine, atlasTexture, frameUvMin, frameUvMax,
    size: new(1, 2), pivot: new(.5f, 0), material with { AuthoredFacing = .3f },
    BillboardMode.CylindricalView);
engine.Vegetation.SetLayer(new VegetationLayerRequest(field, Layer: 3, flower.Look,
    default(OptionalMaterial), Density: .3f, Reach: 3000) with { Fade = 400 });
```

- **Everything a mesh copy takes.** `Look` is an ordinary mesh appearance, so
  spots, `Records`, `Fade`, `Near`, thinning, handover between fields and levels,
  budgets and the renderer's per-copy selection work as above. `VegetationLod.Mesh(flower.Look,
  start)` makes it a farther level of a mesh, which is the way to go from a mesh
  tree to a flat. A copy's scale and tint apply; `Align` does not, since a
  billboard always stands upright. Its random yaw counts only by the material's
  `AuthoredFacing`, the share of it the copy keeps, so copies need not all
  share one facing.
- **Material.** `SpriteMaterialDescriptor` gives the lighting, alpha mode,
  emission and wind, in every mode a sprite has, and a copy lights as the same
  sprite does. `Mask` and `Opaque` copies write depth and draw in the world pass,
  so they hide what stands behind them and fog reaches them by their depth. A
  `Blend` copy writes no depth and draws through no per-copy selection (the
  selection's copies are opaque), so its thinning and level crossfades do not
  apply and a patch's copies are not sorted: keep it for a few, near. Additive
  blending, softness and a tint mask are refused. `WindBend` leans it as a
  sprite's does, and `WindWholeTexels` steps its rows in whole texels with the
  bottom row planted.
- **Size.** `size` and `pivot` place the quad about the copy's origin; keep the
  pivot on the frame's centre line so the quad turns about its middle, which the
  selection's bounds expect.
- **Shadows.** A billboard casts none, whatever `CastsShadows` says, so leave it
  off: asked for, it still counts as a caster and redraws the shadow maps while
  the eye moves. Ray-traced passes do not see it either.
- **At a distance.** A masked frame only a pixel or two wide shimmers: nearest
  sampling lands on different texels from frame to frame, and the copy pops
  between covering a pixel and not. Multisampling softens that but does not
  hold it. Two things do: open the atlas with mipmaps (`NearestMipmapped`), whose
  alpha averages down, and let a layer's thinning trade far copies for larger
  ones (`maxScale`), so fewer copies stay wider than a pixel or two.

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

The depth pass and pyramid cost GPU time in the `occlusion` pass, and the
renderer CPU that encodes every opaque part once more. It pays where opaque
surfaces hide many copies at once. A copy hidden
behind terrain mostly costs vertex work only, since the depth test already
discards its pixels, so the gain follows the number of hidden copies more
than their area.

## Budgets and readout

- `MaximumInstances`: the layer's copies are given to its tiles nearest
  first; a tile that does not fit stays bare and is counted (`OverBudget`).
- `PlacementsPerCall` (64): the most tile layers of the field placed at once.
  Placing runs on the Engine's vegetation worker threads, off the update: a
  call hands tile layers to them, nearest first, and a later call takes each
  finished placement in whole, the same copies in the same order as placing
  in the call would make. Until then a tile layer draws what it had: nothing,
  or its copies from before its tile was sampled again or its layer changed.
  What is being placed or waits for room is counted (`Deferred`).
- Choosing again: a call chooses its tile layers again only when the field
  changed (a layer set or removed, a tile committed or invalidated, a
  placement taken in), or the focus moved a sixteenth of a tile from where it
  last chose. Otherwise it costs nothing. Choosing, it widens every reach by
  that much, so what it keeps covers any focus within it; the copies' fades
  are drawn from the camera itself.
- Device ceiling: every scatter copy held is a 64-byte row per mesh group
  in one buffer the renderer binds whole, so a device holds as many rows as
  its largest storage binding, at most 16.7 million (2^24, what a selection
  entry names): 2 million on a device binding 128 MiB (llvmpipe, and
  WebGPU's default), all 16.7 million on one binding 1 GiB or more (RADV
  binds 2 GiB). A patch that would pass it is refused whole and draws nothing until it is
  made again; the selection leaves out a run whose entries (two for each
  level part's copies) would pass the same binding. Both count in
  `gpu.scatterSelection.overLimit`; keep layer budgets (`MaximumInstances`)
  under the ceiling of the devices a product supports.

`Read` (and every call) returns the field's readout: tiles held and waiting
(`Tiles`, `PendingTiles`), patches and copies drawn, `OverBudget`, `Deferred`,
the placements the last call took in (`Placed`, `PlacedInstances`) and the
workers' time placing them (`PlacementMicroseconds`), the call's own time
choosing what is drawn and turning that into renderer work
(`SelectionMicroseconds`, `PublicationMicroseconds`),
`InvalidatedTiles`, `ReplacedPatches`, `SampleBytes`, and the calls that
had nothing to choose again and those that chose again (`SettlesSkipped`,
`SettlesReselected`). The renderer's side
of the upload is `engine.renderer` statistics' `scatter`: patches and copies
taken in since it was made, the CPU time turning them into drawable parts and
the GPU row bytes they upload; read it twice and subtract for a span.
`engine.renderer`'s `gpu.scatterSelection` reports the last world view's
selection: runs, parts, draws and copies selected, the commands its draws
take and the bytes of their mesh pool (`commands`, `poolBytes`), the copies
left undrawn past the device ceiling (`overLimit`), and (a frame later) copies
tested, drawn (a crossfading copy twice), drawn for shadows, thinned away,
outside the view and hidden behind its depth (`occluded`), with draws at
levels 1 to 3; its GPU time is the `scatter-select` pass. `gpu.occlusion`
reports the occlusion test: on, its pyramid's size and levels, and its
depth's samples per pixel; its GPU time is the `occlusion` pass.

Copies are never nodes, entities, colliders or pickable, and they cast
shadows only when the layer asks.
