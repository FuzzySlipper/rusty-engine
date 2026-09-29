# C# runtime implicit surfaces

Entry page: [C# SDK guide](csharp-sdk.md).

## Runtime implicit surfaces

`engine.ImplicitSurfaces` constructs general-purpose scalar fields and generates
ordinary retained `MeshResource` objects. This is independent of the existing
voxel residency service and its cubic surface modes. C# owns the shape recipe,
material selection, and regeneration intent; Rust owns evaluation, dual
contouring, mesh attributes, and renderer admission. No retro style is built
into this service.

Create an `ImplicitField`, add boxes, spheres, ellipsoids, capsules or planes,
and compose their returned `ImplicitNode` values with union, intersection,
difference, smooth union, offset and affine TRS placement. Nodes belong to that
field: their opaque tokens are valid only with the field that produced them, and
discarded or disposed-field tokens are rejected. Values are negative inside; these constructive fields preserve a zero
surface but are not necessarily Euclidean distances. Smooth-union radii and
level-set offsets are in field-value units, especially after nonuniform scale.

`DisplaceWaves(ImplicitWaveRequest)` adds smooth seeded spectral noise to a
source field. `Frequency` selects cycles per coordinate unit on each axis;
`Amplitude` bounds the absolute change in field value. `Octaves` (1–8),
`Lacunarity` (at least 1), and `Gain` (0–1) control the normalized multiscale
sum. The same seed and parameters reproduce the field. This is a finite sum
of independently oriented waves, not lattice Perlin noise or simulated erosion.
Amplitude is not a world-space displacement guarantee. The operation preserves
neither connectivity nor a bounding shell; compose protected volumes afterward
and select extraction spacing appropriate to the finest wavelength. The scoped
`ImplicitRecipe.DisplaceWaves` helper uses this same Engine operation.

`Generate(ImplicitGenerateRequest)` takes an enclosure, sample spacing, crease
angle, UV scale, default material, and optional ordered material regions:

- The enclosure expands about its center into a cube with its longest side,
  preserving uniform world-space samples. To clip to a rectangular volume,
  explicitly intersect a box. Domain boundaries are not automatic caps.
- `MaxExtractionVertices` and `MaxExtractionTriangles` select raw extraction
  output budgets. Zero retains the default 262,144 each; positive values select
  the caller's budget. These are not peak-memory limits or limits on subsequent
  attribute/material splitting. Material-refinement budgets remain separate.
- Cell size is a maximum leaf sample spacing, not a minimum-feature guarantee.
  Thin features can disappear. Keep enough empty margin around closed shapes.
- `AddFrustum` authors a capped circular taper between distinct `Start` and `End`
  points. Non-negative `StartRadius`/`EndRadius` select the endpoint sizes; at
  least one must be positive. Equal radii give a cylinder, one zero radius a
  cone. Caps are perpendicular to the axis. Its negative-inside field preserves
  the zero surface but is not generally Euclidean signed distance, so offsets
  and blends retain the ordinary field-value interpretation.
- Zero crease angle gives flat facets; larger angles admit incident faces into
  area-weighted normals. Major-axis planar UV charts use world coordinates;
  UV scale is repeats per world unit, independent of extraction density.
  `ImplicitTextureMapping` optionally replaces that scalar chart: use
  `MajorAxis(scale, offset)` for independent U/V repeats and offsets, or
  `Basis(uAxis, vAxis, scale, offset)` for an orthonormal U/V orientation in
  the field's extraction coordinates. Its scale is repeats per projected world
  unit and its offset is added after scaling. `RecipeSurface.Placement` is
  applied after extraction and does not reproject UVs. Leaving the mapping at
  its default preserves the legacy major-axis UV output exactly.
- The short request constructor keeps `ImplicitMaterialBoundaryMode.Centroid`:
  each triangle uses the first region containing its centroid, or the default
  material. Select `MaterialBoundaryMode: ImplicitMaterialBoundaryMode.Interpolated`
  in the full request to split triangles at the zero contour of linearly
  interpolated vertex field samples. This gives exact cuts for affine fields
  such as planes, independently of triangle direction. Curved fields are
  polygonal approximations; regions hidden between vertices can be missed.
  Set `MaterialSampleSpacing` to a positive world-space edge length to subdivide
  the attributed surface before sampling material fields, independently of DC
  simplification. Zero retains the existing behavior. This option requires
  `Interpolated`; it does not change geometry extraction or recover missing
  grooves. Shared edges subdivide consistently and new normals/UVs interpolate
  the original attributes. Choose spacing below the narrowest desired motif;
  arbitrarily small, tangent or undersampled regions can still disappear.
  Refinement and subsequent clipping enforce the ordinary 262,144 vertex and
  triangle budgets per mesh, returning an error rather than silently dropping
  detail. `Generate` rejections include copied diagnostics on `EngineCallException`;
  catching that operation error allows the product callback to continue and keep
  its prior scene. Backend panics still fail the callback.
  Smaller spacing increases surface sampling and triangle cost; use
  bounded authored pieces. Cuts preserve the source
  surface and interpolate its existing normals/UVs instead of creating shading
  creases. First-region precedence remains unchanged. Added triangles count
  toward the ordinary mesh admission limits. Texture filtering/wrapping comes
  from ordinary materials; material groups remain indexed ranges.

Create an appearance with `engine.Graphics.CreateMeshAppearance(mesh)` and
include it in the product's complete appearance snapshot. A mesh may have
multiple appearances. Dispose appearances before their mesh, and materials
only after meshes using them have been released. The field may be disposed as
soon as generation completes: the mesh owns its copied result. Generate a new
mesh for explicit whole-region replacement.

For collision, `StaticMeshAsset` accepts `new MeshResourceReference(mesh)` in
its `MeshResource` field. Pass that asset and its instances to the existing
`Spatial.ReplaceCollision`, with empty raw vertex/triangle arrays when every
asset uses a reference. Spatial copies the geometry during admission; its
collider remains valid after the source graphics resource is released. Visual
and collision replacement are explicit independent product actions. A zero
reference retains the existing borrowed-array collision path.

For streaming authored collision cells, use `Spatial.ApplyCollisionResidency`
with stable asset and instance IDs. Its arrays are upserts; `RemovedAssets` and
`RemovedInstances` remove selected IDs before upserts are applied. Missing
removals are harmless. A whole delta commits atomically, and removing an asset
still referenced by a retained instance fails without changing the scene.
Unchanged geometry and prepared colliders are shared; admission does not copy
or rebuild every resident cell. Use current local-frame instance transforms;
`WorldOrigin` rebases these same retained colliders. The product owns cell
selection, unload/reload policy and its authored identity map. This path uses
ordinary static-mesh collision/query ownership, without a dense voxel volume
or replacement of the complete collision artifact.

`ReadGeneration(field)` reports the most recent successful generation's vertex,
triangle and material-group counts, actual sample spacing, octree depth,
elapsed service time, and reorientation/degenerate facet counts. Generation is
synchronous in the normal product callback; use load-time or explicit bounded
regeneration, not every frame. Partition large authored compositions and give
simple planar solids coarser sampling. Caller-selected output limits are checked
after extraction and do not bound peak memory or guarantee a latency deadline.

The backend uses Fidget 0.5 evaluation and dual-cell connectivity. Engine
triangulates its ordered cell-vertex polygons, avoiding folded fans around
sampled edge intersections at adaptive transitions. Shared-edge winding is
preserved; the retained reorientation counter is zero because individual faces
are no longer flipped against centroid gradients. Zero-area triangles are
omitted. The vendored `fidget-mesh` patch recovers finest-cell QEF vertices
that escape their cell to the mean of that cell's Hermite crossings, and prevents
those invalid solutions from driving collapse. `BoundedLeafVertices` counts these
adaptive recoveries. This bounds placement; it does not guarantee thin-feature
survival or self-intersection-free output. The patch and its source/license ship
in the runtime pack's `share/third-party/fidget-mesh` directory.
Large retained replacements remain ordinary deltas, sent as one output batch.
Complete committed snapshots serve every browser connection and recovery;
size alone does not replay/reconstruct the scene or re-enter product callbacks.

Implicit generation readouts also report boundary, non-manifold, and inconsistent
winding edges on extracted geometry before normal, UV, and material splitting.
An entrance into a carved solid can still have a closed rock surface around its
rim. These report-only counts diagnose index topology, not self-intersections,
feature survival, or final attributed-mesh watertightness. The adaptive path
[preserves distinct contour arcs on ambiguous faces](implicit-topology-diagnosis.md)
(Engine #7879). Zero boundary edges alone still does not establish manifold output;
vertex links and self-intersections are separate properties.


### Managed authoring vocabulary

`Rusty.Engine.Implicit` provides optional ordinary C# helpers. `ImplicitRecipe`
scopes an Engine field and composes typed nodes; `RecipeWriter` emits borrowed
`RecipeSurface` descriptions synchronously. The receiver generates and publishes
the mesh before the recipe is disposed. `ArchitecturalRecipes` supplies tunable
layered walls, masonry courses, passages, chambers, joins, and enclosed carving
with explicit portals. Products retain layouts, seeds, materials, artistic
choices, stage ordering, and publication policy. These helpers do not own a
renderer, evaluator, scene registry, or serialization format.

`PlanarRecipes.ConvexPrism` accepts either winding of a finite, strictly convex
XZ contour. It rejects concavity, self-intersections and degenerate edges before
adding field nodes. `PlanarRecipes.Walkway` unions square-capped segments and
preflights the whole centerline before construction. These are composition
helpers over the existing Engine field operations, not another evaluator.

`RoomRecipes.Shell` emits floor, wall and ceiling solids from one interior box,
wall thickness and named portal boxes. Optional floor platforms and ceiling
soffits union into their owning slabs. The returned `RecipeRoomContinuity`
contains expected wall/slab contacts (excluding doorway intervals), the same
portal boxes and an interior seed. Resolve its named `RecipeJoin` surfaces to
captured audit IDs, then use `Request` with the existing `ReadExpectedJoin` or
`ReadEnclosure` services. Products choose budgets and interpret completeness;
declarations express intent, not a guarantee of a clean extracted mesh.

The optional `RoomRecipes.Shell` placement transforms emitted surfaces and their
declarations together. Individual joins support placements preserving a rectangular
contact patch; room enclosure caps require axis-preserving placement
because the underlying cap contract uses axis-aligned boxes. Arbitrary rotated
caps are rejected rather than silently enlarged. Mesh generation and audit
capture must still happen synchronously before a recipe field is disposed.

The [architectural room example](../fixtures/csharp-architectural-room/README.md)
shows a recessed floor, stepped ceiling, windows, door and adjoining passage
through the ordinary packaged C# path. The source measurements → editable
suggestions → manual composition workflow in Loading Bay is a useful authoring
pattern; these helpers do not import source levels or prescribe their layout.

### Retained sampled densities

The same `ImplicitSurfaces` service owns `SampledVolume`. Create one with an
origin, positive uniform spacing, lattice-point counts (at least two per axis),
and initial scalar value. Values are finite floats, negative inside, in x-fastest
order: `((z * height) + y) * width + x`. The last sample lies at
`origin + spacing * (dimensions - 1)`. The retained limit is eight million points.

`WriteSampledVolume` replaces a bounded contiguous sample range;
`ReadSampledVolume` returns a managed copy and descriptor/revision. The generated
lease is released before returning. `SampleSampledVolume` trilinearly samples
inside the explicit domain and rejects outside positions. `RasterizeSampledVolume`
evaluates an analytic field onto the lattice in native batches, committing only
when all samples succeed. Successful writes and rasterization advance the
revision and invalidate the last generation readout; previously generated mesh
resources remain independent snapshots. Disposing the source field does not
invalidate stored densities.

`GenerateSampledVolume` extracts the selected isovalue through Engine's existing
uniform dual-contouring mesher. Its `Field` supplies only optional material
regions, independently of geometry. It reuses analytic generation's attribute,
material, resource, and collision paths. `SampledRecipeSurface` is an optional
synchronous description for this extraction. `ReadSampledVolumeGeneration`
reports actual lattice spacing and topology; octree depth and adaptive leaf
recoveries are zero because this path is uniform. Expected operation errors
return copied diagnostics without poisoning an otherwise handled product call.

Sampling resolution remains a real limit: crossings hidden between lattice
points are lost, and one vertex per active cell cannot represent arbitrary
within-cell topology. The mesher does not add padding or caps at volume edges;
include exterior samples around closed solids. Dense extraction has explicit
cell, temporary-memory, and mesh budgets, so not every retained volume can be
meshed in one request. Partition large products deliberately. This foundation
does not implement erosion, world streaming, or efficient sparse edits.



An interrupted development-host output subscription reattaches through a fresh
retained baseline, even after receiving numbered output. SSE cursors are local
to a host process; they are never reused after interruption against a potentially
replaced process. Input and product mutations are not replayed during this
recovery. The browser remains gated until the replacement projection is applied.

### Opt-in authored surface audit

`ImplicitSurfaces.CreateAudit()` creates an authoring-only collection. In a
`RecipeWriter` receiver, generate the ordinary mesh, then call
`CaptureAuditPiece(new(audit, pieceId, surface.Field, surface.Root, mesh,
surface.Placement, service.ReadGeneration(surface.Field).SampleSpacing))`.
Use stable unique `ulong` piece IDs and keep a product dictionary for labels.
The Engine copies geometry and retains an independent field snapshot; source
fields, meshes and materials can be disposed before running the audit.

`ReadAudit(new(audit, toleranceCells))` returns copied diagnostics, candidate
piece-pair and triangle-pair counts. Each diagnostic identifies both pieces,
classification, world-space bounds and approximate affected area. Coincident
and near-coincident exposed surfaces are distinct from buried surfaces; an
ordinary solid intersection need not be an exposed conflict. The report does
not change meshes or decide product acceptance. Dispose the collection when
authoring analysis ends; managed reports survive its disposal. Capture and
analysis are explicit synchronous operations, with no ordinary update cost.

The audit compares nearly parallel extracted facets and clips their projected
triangles to estimate contact area, with spatial AABB filtering for piece and
triangle candidates. It uses field **signs**, rather than treating constructive
field values as distances. Tolerance is relative to the coarser world-space
extraction spacing of each pair (largest placement stretch for nonuniform
scale). Facet normals must be within about 2.6 degrees of parallel; separation
within coordinate floating-point resolution is classified as coincident,
and the remainder up to that tolerance as near-coincident. Side probes start
at representable coordinate resolution and grow only when needed to straddle
an extracted facet. Exposure and burial sample patches at extraction spacing,
so pairwise area totals can count opposing internal faces separately.
This is sampled diagnostic evidence: curved
DC approximation, thin features or gaps between samples, open/clipped extraction
boundaries, and partial buried-face coverage can make bounds/areas approximate
or contacts unreported. A clean report is not a mesh-validity certificate.
Camera depth precision, extreme near/far ratios, shadows, transparency sorting,
texture aliasing and shader artifacts are outside this audit's guarantees.

The same collection supports three separate continuity queries:

- `ReadMeshIntegrity(new(audit, openings))` examines every captured triangle's
  exact-position connectivity. It reports open edges, edges with more than two
  incident faces, disconnected vertex fans and zero-area triangles. Duplicate
  vertex positions share connectivity; nearby positions are never welded.
  `ImplicitAuditOpenRegion` declares a world-space box for one stable piece ID;
  an edge is intentional only when both endpoints lie inside it. Use this first
  on malformed meshes; the overlap analysis requires nondegenerate facets.
- `ReadExpectedJoin(new(audit, pieceA, pieceB, center, halfU, halfV,
  searchDistance, toleranceCells, sampleSpacing, maxSamples))` samples an authored
  rectangular contact patch. Its perpendicular half axes set orientation and
  size. Along the patch normal, the query finds each named mesh's nearest
  intersection within the search radius. It reports separation above the
  extraction-relative tolerance, or `MissingJoinSurface` when either side is
  absent. Choose the patch and radius to identify the intended surfaces, avoiding
  unrelated faces of the same pieces. Width is maximum sampled separation;
  affected area is the sum of failed patch cells. Closed-mesh ray containment
  distinguishes solid overlaps from air gaps using extracted geometry, even
  when the source fields still meet. Inconsistent containment rays produce
  incomplete coverage. Open meshes provide facet-separation evidence without
  a closed-solid containment guarantee. This is not inferred intent.
- `ReadEnclosure(new(audit, minimum, maximum, interior, openings,
  sampleSpacing, maxSamples))` searches a bounded six-neighbor world grid from
  the declared interior point. Segment/triangle intersections block traversal.
  A leak returns one interior-to-outside `Path` and exit bounds; it does not
  enumerate every leak. Declare intentional door/window volumes as
  `ImplicitEnclosureOpening` entries: these virtually cap those openings while
  searching for other routes. `IntentionalOpening` means a declared virtual cap
  was encountered, not that a physical door was proved open. Capture moving
  doors at the pose being audited;
  use a separate collection for another pose. The query examines mesh barriers,
  not field distances or an assumed union of closed solids.

Continuity reports copy diagnostics and path points before releasing their
native lease. `Complete != 0` means the declared discrete query completed (or
found a witness); it is not proof below `Resolution`. For joins/enclosures,
`Sampled` counts patch samples/visited cells. An insufficient `maxSamples`
budget returns `IncompleteCoverage` and `Complete == 0`, never a clean result.
An enclosure seed whose connection to its grid cell crosses geometry also
returns incomplete coverage. Seeds must be authored in empty interior space.
Thin passages, diagonal connectivity, small missing triangles and contacts
between samples can be missed. Repeat at finer spacing when a feature is near
resolution; exact topology findings and sampled enclosure findings are distinct.
A leak path's reported width is the grid spacing, not measured clearance.
Enclosure diagnostics use zero piece IDs for collection-wide connectivity;
the declared region and returned path identify their scope.

These are synchronous authoring operations. The development host does not
interrupt a long callback, so full-scene capture and analysis simply hold the
runtime until they finish; this does not make analysis asynchronous or increase
geometric coverage. Keep analysis behind an explicit authoring switch or debug
command, and stop the owned host after the report is collected.

### Entity stats collections

`StatsComponent` holds product-named `StatId → Stat` and `TrackId → Track`
collections. Use it standalone or attach it like any ordinary class:

```csharp
var maximumId = StatId.Parse("maximum-health");
var healthId = TrackId.Parse("health");
var maximum = new Stat(100, minimum: 0);
var stats = new StatsComponent();
stats.AddStat(maximumId, maximum);
stats.AddTrack(healthId, new Track(maximum));
entities.Add(entity, stats);
entities.Get<StatsComponent>(entity).GetTrack(healthId).Spend(10);
foreach (var (id, track) in stats.Tracks) { /* UI reads track.ValueInt */ }
```

`Stats` and `Tracks` are live read-only dictionary views; their objects remain
mutable. Add methods reject duplicate IDs and retain the exact supplied objects.
`GetStat`/`GetTrack`, `TryGetStat`/`TryGetTrack`, and removal methods operate on
those collections directly. A track's maximum is registered only if the product
explicitly adds it. Removing a stat entry does not disconnect tracks referencing
that stat. IDs, labels, formulas, save schemas, and gameplay policy stay product-owned.

### Effects and owner-scoped inventory components

`EffectsComponent` is an ordinary class for Apply/Refresh/Replace/Remove/Expire
operations, with stacking and provenance checks. It works standalone or attached
through `entities.Add(entity, effects)`. Product code owns duration and timing.
Use `effects.Copy()` only when a detached preview is useful: the copy has its own
collection and shares immutable effect entries/definitions, without replaying
mutations. D20 uses this for action planning; Rifles keeps its own effect clock.

`InventoryStore` owns item quantities, containment and equipment records.
Use its direct Grant/Consume/TransferFungible, MaterializeUnique/TransferUnique/
DestroyUnique, and Equip/Unequip/Swap methods for ordinary operations. Redundant
static InventoryService/ItemService/EquipmentService forwarding APIs are removed.
`InventoryEdit` remains optional for grouped changes such as unequip → transfer →
equip. A failed edit leaves the store unchanged; `Publish` refuses if the store
changed directly after the edit began.

After registering an owner's inventory/equipment, attach live facades if useful:

```csharp
var inventory = new InventoryComponent(store, owner);
var equipment = new EquipmentComponent(store, owner);
entities.Add(owner, inventory);
entities.Add(owner, equipment);
inventory.MaterializeUnique(item);
equipment.Equip(item.Entity, slots);
```

These facades retain the store and owner ID, resolving current records on each
read or operation. Retaining the component is safe across edit publication;
individual returned views/lists describe the read that produced them. They do
not hold another ledger, grant writable access to internal maps, or synchronize
EntityStore parent relationships. Grouped operations still use the same store's
`Prepare()` edit. Inventory-only owners need no EntityStore attachment.

For metadata-bearing quantities, give each distinct stack a product-selected
`InventoryStackId` and use `Grant(owner, definition, stackId, quantity)`.
`InventoryView.Stacks` exposes the IDs; their scope is the owner inventory.
Selected-stack Consume, SplitFungible, TransferFungible and MergeFungible all
use the same store ledger and capacity checks. The product chooses compatible
merge targets and copies or retires its metadata using the returned IDs.
Splitting requires a new destination ID and leaves a positive source quantity;
merging retires the source ID. A full transfer can preserve its ID in an owner
where that ID is unused. Partial transfers require an explicit destination ID,
either new or selected for a compatible merge. `MaximumQuantity` limits each
stack; inventory capacity accounts for every stack and unique item together.
Every inventory mutation selects an explicit stack ID. Definition-only Grant,
Consume and TransferFungible overloads and the implicit-ID InventoryStack
constructor have been removed. Existing callers must choose stack IDs; the
Engine never derives one from definition text. Definition-level quantity reads
still aggregate all stacks of that definition.
`InventoryState.CaptureStacks()` and `InventoryState.Restore(...)` retain stack
IDs, definitions and quantities; restore and registration validate capacity.
Persist product metadata keyed by those owner/stack IDs, or map them to save-local
identities and re-grant on rebuild. No second quantity ledger or metadata-encoded
definition ID is needed.

### Explicit capture, restore and live inspection

Capture selected durable values into product-owned records on request. Save those
records through `ProductStateStore<T>.Save`; they should not contain live component
references, native leases, input state or presentation resources. `Load` reads and
decodes the current shape and returns a value; it never changes the live graph.
Build and validate replacement owners from that value, then install them at the
product boundary. A failed decode or candidate build leaves the old owners in place.
This does not promise rollback of independent native work already committed.

Rebuild shared references deliberately. For example, construct one maximum `Stat`,
put it in `StatsComponent.Stats`, and pass that same object to the restored `Track`.
D20 restores this graph through participant admission; its save contains numeric
values and product identities rather than a serialized component graph.

`JsonProductStateCodec<T>` is the ordinary JSON path over `ProductStateStore<T>`: supply
the save type plus a `JsonTypeInfo<T>` (source-generated contexts work under NativeAOT
without reflection) or `JsonSerializerOptions` for CoreCLR convenience, then Save/Load
with no byte-buffer plumbing and no version scaffolding. Absent keys report absent;
malformed bytes fail in deserialization; a JSON null document fails rather than decoding
to a missing value. Custom binary codecs stay available through the same small
`IProductStateCodec<T>` contract.

The direct Persistence requests, receipts and blobs also carry no product schema
version. Storage owns only its file layout marker and revision; specialized codecs
(such as voxel edit history) identify their own payload format during decoding.
The current layout replaces the old schema-bearing envelope without migration;
old development save files must be discarded or explicitly converted by their owner.

`ProductStateStore<T>.Delete(key, guard, expectedRevision)` and the direct
`Persistence.Delete(PersistenceDeleteRequest)` durably remove one scoped key.
`Deleted` reports the removed revision; `Missing` reports zero. Guards match Save:
`Any` accepts either state, `Exact` requires an existing matching revision, and
`Absent` requires no key. A mismatch returns `RevisionConflict` with the current
revision (zero when absent) without removing bytes. Loaded blobs remain readable.
Recreating a deleted key starts at revision one; revisions are not tombstone IDs.
Storage failures throw rather than returning a deletion receipt. After an I/O
failure, reload to determine whether removal occurred. Product slot catalogs and
metadata remain product-owned.


`StatsComponentCapture.Capture` reads a component's selected stat/track values as plain
data and `Rebuild` reconstructs an equivalent set with each track sharing its rebuilt
maximum `Stat` — later stat changes reach the same track. Authored stat sources are not
captured; re-supply them via `SetSources` in the optional `restoreStat` callback,
before any tracks are constructed. That callback runs once per distinct stat and
receives fresh modifier removal handles in capture order; retain them with the
product-owned temporary effects that will later remove those modifiers. Without
the callback, captured local modifiers remain attached for the rebuilt stat's lifetime.
Multiple names for the same Stat or Track are captured as aliases and rebuild to
the same instance. The callback receives the ordinal-first stat name, not each alias.

```csharp
StatsComponent restored = StatsComponentCapture.Rebuild(saved, (capture, stat, handles) =>
{
    stat.SetSources(StatId.Parse(capture.Id), sourcesByStat[capture.Id]);
    restoredModifierHandles[capture.Id] = handles; // product-owned removal associations
});
```

Effects rebuild by re-applying definitions
with fresh instance ids and product provenance through `EffectsComponent.Apply`;
inventory rebuilds by re-registering, granting stacks, materializing uniques under
product-mapped fresh entities, and equipping with re-supplied slot definitions.
Keep an item's saved instance key separate from its definition ID: two swords can
share a definition while remaining different items. The mechanics example assigns
save-local keys, maps each to a fresh runtime entity, and stores every equipment slot
per item (including multi-slot items). These keys are product-owned save data, not an
Engine identity registry. Definitions, provenance, and durable identity mapping remain
product choices.

For debug inspection, opt in on the existing product execution boundary:

```csharp
var debug = new EntityStoreDebugModule();
debug.RegisterStore("session", session.Entities);
debug.RegisterMechanicsProjections(maximumEntries: 16);
// Register this module with the product's debug-command catalog.
// After adopting a restored session:
debug.ReplaceStore("session", restoredSession.Entities);
// Before ending the registration's lifetime:
debug.UnregisterStore("session");
```

`RegisterProjection<T>(formatter)` also supports custom class/value projections
without a numeric descriptor. Descriptor-specific projections remain available
and take precedence for that descriptor. `entity.get` reports component types and
keys for `entity.component`. Every command reads the currently registered store
and current component fields, even when in-place edits leave structural revisions
unchanged. Mechanics projections limit entries and all projection output remains
bounded to 4096 characters. Returned debug metadata is an observation, not a save
or replay checkpoint. Registration is local and explicit; no mechanics discovery
or structural-version value cache is involved.

### Spatial debugging maps

`Spatial.ReadMap` reads a bounded (up to 1,024 cells), world-aligned X/Z map
from a live `SpatialSession`. Supply the minimum X/Z corner, cell size,
columns/rows, a collision Y interval, a separate navigation support Y interval,
and current product-owned dynamic colliders. Columns advance +X; rows advance
+Z. The copied receipt carries spatial publication identity and source,
collision and navigation revisions. It does not change navigation or gameplay.

Collision tests each full cell footprint against retained geometry and enabled,
non-trigger supplied colliders. Navigation samples the cell center against the
retained navigation projection, preserving support counts/heights and traversal
allowance. No navigation sample means unknown, not walkable. A collision miss
means no hit in the supplied/retained sources, not proof of loaded empty space
or character clearance. These are different observations, not a merged occupancy
truth. Multiple support heights remain explicit; this first view is not a full
stacked-floor visualizer.

`Rusty.Engine.Debugging.SpatialMapSnapshot.Capture` combines that read with
product-supplied `SpatialMapObservation` (stamp, player position/facing) and
`SpatialMapAnnotation` values (stable ID, label, relation, state, world position).
Call it at the existing serialized live-debug boundary for coherent product
facts. `ToAscii()` emits map layers and a legend; `ToJson()` emits the same
snapshot with compact row-major cell arrays whose `cellFields` names define
the columns. Both are NativeAOT-compatible. The helper retains copied facts,
not the session or collider inputs. Annotation limits, out-of-view counts, and
height intervals are explicit. The current view is omniscient.

Expose a product command through the ordinary generated debug catalog and use
`runtime-pack/bin/rusty-live-debug --origin http://127.0.0.1:PORT --command
"spatial.map ascii 12 1"` when the product implements that command (as
`rusty-doom` does). The CLI transports the command; the product supplies semantic
annotations and the Engine owns the spatial read. Rendering and fast-controller
experiments are independent of this capability.

### Source GLB material extensions

Live GLB admission preserves `KHR_materials_specular` and `KHR_materials_volume`
through the existing Engine loader, including required declarations. Optional
`FB_ngon_encoding` exporter hints over core triangles are accepted; declaring
that metadata as required is still unsupported. Material textures/factors stay
in the source resource and are realized by the Engine renderer. Unknown required
extensions still produce an import diagnostic without replacing the current view.

### GLB inspection and displayed-pose bounds

`Animation.SetMeshInspection(new(appearance, wireframe, matte, wholeVoxelNormals,
boundsRequest))` selects retained inspection for an animated-mesh appearance,
including GLBs without clips. Publish the appearance in the normal Graphics
snapshot. This appearance-level setting applies to every instance selecting that
appearance; use separate appearances for independent inspection. Inspection
updates preserve playback and target identity. Renderer
instances own temporary material/geometry clones; admitted source resources and
textures remain shared and unchanged. Matte keeps textures and sets PBR roughness
1, metalness 0 and environment intensity 0.35. Whole-voxel normals affect only
predominantly integer unit-face meshes; skinned/morph geometry stays authored.

A changed nonzero `boundsRequest` asks for the displayed pose's world-space bounds
once after the next renderer animation update. `Animation.ReadRealizationFactAt`
returns `MeshInspection` with the logical object, renderer generation,
`BoundsRequest`, `HasBounds`, `BoundsMin`, `BoundsMax`, and `VoxelNormalMeshes`.
No bounds means an empty/unmeasurable mesh, not a zero-sized box. Match the object
and request before consuming; cancel pending camera actions when the user moves
it. A replacement renderer replays the retained request once. Zero disables the
request. This is observation, not a second animation clock or automatic camera.

See [portable asset descriptors](portable-assets.md) for Engine-owned sprite/model semantics over loose files and bundles.

### Stopping the development supervisor

Packaged CoreCLR launches, and supervised or headless NativeAOT launches, run
a supervisor process and one runtime process with the selected loader;
see [packaging and development](architecture.md#packaging-and-development).
The runtime has its own Unix process group, so terminal Ctrl+C and
foreground-group signals reach only the supervisor. On SIGINT or SIGTERM, to
`rusty dev` or to a direct `rusty-product-host --product <Product> --loader
coreclr`, the supervisor closes the runtime's stdin. The runtime stops
updating, runs product disposal, flushes diagnostics and exits successfully.
A runtime that cannot dispose within ten seconds is killed with
`DEV_HOST_RUNTIME_SHUTDOWN_TIMEOUT`.

A direct launch does not treat its own stdin EOF as a stop request. If its
runtime crashes, the host stops with a named diagnostic and a nonzero exit; it
does not silently retry gameplay. Under `rusty dev`, the first crash restarts
the runtime once, and a second pauses until the next source restage; neither
replays a request. The development host has no callback deadline. A product
stuck in a callback shows `inFlightOperation` and its age in live-debug
telemetry, and the next source restage replaces it. `--debugger` disables the
30-second runtime startup deadline for managed breakpoints; shutdown remains
bounded.

NativeAOT hosting stays in process. Explicit finite `--exercise` and
`--performance-probe` runs, and contributor-only legacy raw-artifact launches,
retain their in-process path; they are not the ordinary packaged server lane.
