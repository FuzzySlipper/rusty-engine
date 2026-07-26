# Voxel model conversion and flipbook workflow

Status: current M12 design and implementation record

Rusty Engine now distinguishes two durable voxel meanings:

- a **voxel volume** is grid content that a product may admit as authoritative environment state and
  derive collision, navigation, and visible mesh from; and
- a **voxel object** is a reusable local-space model with one default arrangement and optional
  named frame-swap clips. Its animated frames are presentation data unless a caller explicitly
  selects one stable frame or another asset as a collision proxy.

This distinction is the center of the mesh-conversion work. Animated poses must not masquerade as
world edits or trigger collision/navigation rebuilds every time a visible frame changes.

## Survey of the starting point

The static path is already substantial:

| Owner | Implemented behavior |
|---|---|
| `voxel-asset` | Strict schema-1 voxel volumes, sparse runs, bounds, palette/material bindings, provenance, canonical bytes, semantic hashes, and bounded conversion inputs. |
| `voxel-convert` | Bounded embedded GLB default-scene import with composed node transforms, multiple mesh instances/primitives, stable source identities, retained UV sets, explicit group/node selection, conservative triangle/cell coverage, closed-mesh interior classification, per-cell material evidence, contain/cover/stretch and origin policy, plan/preview/apply, bounded queries, stale guards, and fail-atomic installation. |
| `engine-spatial` | Canonical material-voxel authority plus collision, navigation, and deterministic chunk meshes. |
| `render-projection` | Stable retained voxel-instance/chunk handles and changed-payload projection. |
| Studio | Project/host GLB selection, conversion settings, private Rust plan and renderer-visible preview, guarded apply/discard, canonical project persistence, import/export, and reopen integration proof. |

Static and animated import now share one bounded scene/primitive identity family, and the offline
converter assembles static defaults and sampled animated clips into durable voxel objects on one
fixed grid. The remaining first-party authoring gap is Studio: its controls still describe one
volume rather than a reusable object, clip ranges, and frame preview.

The existing static workflow remains supported while these gaps close. M12 is an extension and
quality campaign, not a rewrite of conversion or Studio.

## Durable voxel-object schema 1

`voxel-asset` owns `VoxelObjectAsset` beside the unchanged `VoxelAsset` format. A voxel object has:

- a `voxel-object/...` identity;
- right-handed Y-up local coordinates, positive cell size, chunk size, and a finite possibly
  fractional local pivot;
- one required default `VoxelFrame`;
- shared material palette, source-material mapping, and source/settings/tool provenance, including
  exact source animation indices, names, ranges, rates, endpoint policies, and output clip IDs for
  converted animated objects;
- zero or more uniquely named clips with a default frame rate;
- ordered frames with an optional per-frame duration override;
- the exact union bounds of every stored frame;
- a resolved occupancy hash for every frame and one semantic object content hash; and
- explicit per-frame, aggregate-frame, aggregate-voxel, timing, coordinate, string, and artifact
  limits.

Schema 1 stores every frame as a complete canonical sparse arrangement. This is intentional. The
old VoxelForge model stored a base plus per-frame set/remove overrides, which is useful evidence that
delta storage can work. It did not provide measurements for the converted animated corpus this
pipeline will use. Full frames make validation, random access, preview, load failure, and runtime
resolution local and obvious. M12H will measure real artifacts before a delta, reference, or packed
schema is justified. The public resolved-frame meaning does not depend on the storage choice.

The generic content store recognizes voxel objects as durable asset data and can encode their
canonical owner bytes. It does not decide a product path, scene attachment, playback, or collision
policy.

## Bounded model-scene import

`voxel-convert` parses the selected GLB default scene into one reusable imported-model family before
flattening static geometry. Mesh-local indexed primitives retain source node, mesh, primitive, and
material indices plus every bounded `TEXCOORD_n` set. Reachable nodes retain their parent/child
identity, local transform, and composed model transform. A later animation sampler can therefore
deform the same primitive family instead of reparsing through another authority.

Traversal is deterministic root order followed by source child order. It accepts multiple scenes
but reads only the explicit default scene, multiple roots, transform hierarchies, multiple mesh
definitions, and repeated mesh instances. The flattened static mesh duplicates geometry only at
the explicit instance boundary. `meshPrimitive` remains backwards compatible: absent means the
whole model, `group/<n>` selects one deterministic flattened primitive group, and `node/<n>` selects
all groups attached to one exact source node. Canonical metadata exposes the source indices so
equal names never become identity.

The importer bounds source bytes, document nodes/edges/depth, meshes, mesh instances, primitives,
and the union of distinct UV sets across the selected model before allocating flattened attributes,
along with UTF-8 names, expanded vertices, and expanded indices. It rejects cycles, nodes reached from
more than one root/parent, external buffer resources, unsupported primitive modes, implicit or bad
indices, morph/skin/animation inputs in the static path, non-finite transforms/geometry/UVs, and
degenerate transformed triangles with classified source paths. Texture image URIs are never opened
or resolved by this host-neutral importer; only embedded geometry buffers and material identity are
consumed.

The licensed hierarchy corpus is the existing CC0 Kenney wall geometry plus the checked
`kenney-wall-hierarchy.fixture.json` scene overlay. It creates a transformed parent/child branch and
a second transformed mesh node using two mesh definitions and both source primitives. Tests prove
composed transforms, unique node/mesh/primitive identities, UV preservation, deterministic whole
model import, node/group selection, and bounded failure before partial geometry is returned. The
adjacent Kenney license continues to govern the unchanged source geometry.

## Geometric voxelization

Surface conversion now tests every bounded triangle candidate against the closed half-cell box with
a separating-axis test. It therefore retains a cell whenever the actual triangle intersects that
cell, including slanted faces and features thinner than the old point-sampling interval. Candidate
cell volume is charged before iteration against a ten-million-operation geometric-work ceiling.
The conversion receipt reports the measured work; the old `MAX_SURFACE_SAMPLE_WORK` name remains a
compatibility alias for `MAX_GEOMETRIC_VOXELIZATION_WORK`.

This semantic algorithm change is provenance-visible as
`rusty-engine.mesh-to-voxel.v2`; existing v1 assets remain valid data, while a re-conversion records
the geometric owner that produced the new candidate.

Every accepted surface cell retains the closest point's barycentric weights, source triangle, and
source material until palette resolution. Conflicts choose the least squared center-to-triangle
distance, then the lower source material slot, then the lower deterministic triangle ordinal. A
texture binding names a hash-pinned imported `TEXCOORD_n`; barycentric UV interpolation and
clamp-to-edge nearest-texel sampling happen independently for each cell. The binding's `sampleUv`
still chooses the representative/fallback source-material mapping for schema compatibility, but it
does not replace per-cell UV evidence. Missing coordinates or UV hash drift fail before output.

Solid conversion first assigns one topology vertex to every exactly coincident finite position,
normalizing signed zero but using no scale-dependent epsilon. UV, hard-normal, and material seams
can therefore retain distinct attribute vertices without opening an otherwise closed mesh. It then
requires unique geometric faces and exactly two oppositely directed uses of every geometric edge.
It keeps conservative boundary cells, then classifies voxel centers by deterministic X-ray parity
through the closed mesh. A bounded set of tiny row perturbations avoids vertex/edge
ambiguity; an odd crossing count after all attempts is rejected rather than guessed. Interior cells
inherit material and barycentric evidence from their nearest positive-X exit surface. Multiple
closed shells therefore preserve exterior cavities instead of filling the source AABB.

The checked CC0 geometric corpus covers a slanted two-material thin sheet, a hollow cube with an
explicit inner shell, and a non-axis-aligned four-material tetrahedron. Goldens pin cell/evidence
hashes and exact work at several resolutions. Tests also prove connected surface coverage, a 27-cell
empty cavity, scale-sensitive detail growth, topology failure before raster work, and candidate-work
rejection before a large cell loop.

Feature limits remain explicit: surface occupancy is conservative at cell granularity, so a
sub-cell feature is retained but not geometrically reconstructed; solid occupancy represents voxel
centers, so a void narrower than one target cell may have no empty center; the tiny parity
perturbation makes geometry within roughly `2.4e-7` target-cell units of a row boundary numerically
ambiguous; near-coincident positions are not welded because an epsilon could merge real features at
another model scale; and requests whose conservative candidate or parity work exceeds ten million
operations must lower resolution or split the selected model.

## Animated GLB sampling

```text
bounded GLB bytes
  -> deterministic scene/mesh import
  -> explicit clip and sample schedule
  -> deformed indexed mesh snapshots
  -> one fixed conversion grid, pivot, palette, and material policy
  -> complete canonical voxel frames
  -> voxel-object asset with named clips
  -> Studio save/reopen
  -> explicit runtime object admission and presentation playback
```

`voxel-convert` now imports every uniquely named clip from the selected embedded GLB together with
its reachable node channels, base transforms and instance morph weights, referenced skins, joint
tables, inverse bind matrices, `JOINTS_0`/`WEIGHTS_0`, and position morph targets. This extends the
same `ImportedModelScene`; each sampled `ImportedStaticMesh` therefore retains the static path's
node/mesh/primitive groups, indices, material slots, UV sets, and source SHA identity. It does not
define a renderer animation object or a second geometry authority.

Animation time uses integer microsecond ticks. Source key times are rounded once to the nearest
microsecond and must remain strictly increasing after quantization. A rate schedule always includes
tick zero. `IncludeClipEnd` emits regular rounded rate ticks strictly before the duration and then
the exact quantized duration once; `ExcludeLoopSeam` omits that final duplicate endpoint. A
zero-duration clip has one sample at zero under either policy. Sampling clamps each channel before
its first and after its last key while the clip duration is the greatest channel endpoint.

`STEP`, `LINEAR`, and `CUBICSPLINE` follow the glTF channel rules. Linear rotations use normalized
shortest-path spherical interpolation. Cubic values use their in/out tangents scaled by the
quantized segment duration and sampled rotations are normalized. Every sample starts from the
authored node TRS and instance/mesh morph weights, applies channel values, composes parents before
children, applies position morph deltas, then applies four-weight linear skinning. The resulting
positions are finite ordinary object-space indexed geometry ready for the static voxelizer.

Anchor policy is explicit per request. `PreserveSourceSpace` retains authored root motion.
`LockNodeToBindPose` left-multiplies every sampled position by the selected reachable node's bind
model transform times the inverse of its sampled model transform. It therefore removes that
node's motion without recentering the bind pose or choosing an implicit root by name.

Import is bounded to 64 clips, 4,096 channels, one million input keys, four million sampled channel
components, 128 skins, 256 joints per skin, 64 morph targets per primitive, and four million stored
morph-position deltas. Accessor counts are checked before their buffers are collected. Requests are
bounded to 240 Hz, 4,096 snapshots, one hour of quantized source time, and ten million vertex
deformation work units. Clip absence, stale source SHA, unreachable or duplicate targets, collapsed
time keys, bad accessors, bad joints/weights, non-finite values, invalid anchors, and exceeded limits
return source-locatable classified diagnostics before a partial receipt is returned.

The real CC0 Kenney retro-character GLB proves three named clips, a 45-joint skin, bind-pose and
known-time deformation, identity preservation, loop scheduling, and repeat determinism. The
adjacent checked `morph-animation.fixture.json` corpus is also CC0 and proves position morphs,
linear/step/cubic interpolation, endpoint equality, and root-motion locking with exact coordinates.

Deliberate limits are narrow and visible: offline deformation accepts one four-influence joint set
(`JOINTS_0`/`WEIGHTS_0`), only reachable default-scene joints, and TRS animation rather than channels
targeting matrix-authored nodes. Morph normal/tangent deltas are irrelevant to voxel positions and
are not projected into the mesh snapshot. Additional joint sets must first be justified by a real
conversion asset rather than silently changing work accounting.

## Static and animated object conversion

`voxel-convert` admits a static `mesh/...` source or a complete `mesh-animation/...` source through
the same hash-pinned source receipt. Animated admission retains the authority-bearing clip model and
also exposes one bind-pose mesh view for common groups, materials, UVs, and conversion settings. A
static source produces only the required default frame. An animated request selects source clip
names, absolute microsecond ranges, rates, endpoint policy, output clip IDs and names, an optional
default clip, and one explicit animation anchor policy.

Before voxelizing any frame, conversion transforms the bind pose and every selected sample and
computes their exact union source envelope. M12C then maps every mesh through that immutable
envelope with the same resolution, fit/origin policy, object-local `[0,0,0]` volume origin, pivot,
palette, material map, texture policy, and deterministic surface-conflict rule. Individual frames
cannot silently recompute their own scale or offset, which prevents breathing and grid drift. Every
frame remains a complete schema-1 sparse arrangement; the durable object's bounds are the exact
union of the converted default and clip frames.

Frame duration is derived from adjacent integer-microsecond sample timestamps. An
`ExcludeLoopSeam` clip holds its final sample through the exact selected range end, so its output
duration equals the requested range. `IncludeClipEnd` retains the exact endpoint pose and gives it
one nominal sample interval because schema 1 intentionally has no zero-duration frame. A zero-length
range likewise produces one sample held for one nominal interval. Consecutive
frames with equal resolved occupancy hashes are merged only while their summed duration remains
representable; their complete source timestamp list remains in the conversion readout and timing is
unchanged. Non-consecutive matches remain separate complete frames.

Object plans bind the source snapshot, path, target identity, license, mesh settings, pivot, anchor,
clip schedule, expected output hash, measured counts, bounds, and artifact size into one private
prepared candidate. Preview and apply require the exact plan hash; apply may also pin the output
hash. Preview reports sampled/stored frame counts, clip timings, union and per-frame bounds, voxel
and run counts, deduplicated source timestamps, truncation, and bounded samples from an explicitly
selected default or clip frame. Failed or stale apply is checked before the fail-atomic installer
touches an existing asset.

The same crate supplies stale-safe bounded object-info, frame-info, and frame-window queries plus
the `voxel-object-convert` offline CLI. Conversion additionally caps aggregate deformation work at
ten million units and aggregate geometric voxelization work at fifty million units; the durable
schema continues to cap clips, sampled/stored frames, aggregate voxels, strings, coordinates,
timing, and the 64 MiB artifact.

Focused unoptimized local evidence on the checked Kenney character produced a three-sample run plus
default frame in about 89 ms and a 30,642-byte canonical object. A 24 Hz low-resolution idle range
sampled 25 poses, merged all equal quantized clip frames into one stored frame without changing the
selected duration, and produced 5,458 bytes in about 589 ms. Timing is recorded as observational
evidence rather than a machine-dependent CI threshold. Tests also cover repeatability, static
objects, clip/range selection, anchor identity, palette stability, strict decoding, budgets, stale
apply/install, selectable preview, object/frame/window queries, and the real CLI.

Rust owns source parsing, deformation, voxelization, canonical assets, validation, frame
resolution, bounded runtime admission, explicit-time playback, and renderer-neutral values. Studio
owns source and clip selection, forms, preview scrubbing, transient playback, and explicit
apply/discard. A downstream product owns gameplay meaning and when to request playback.

Playback is a named, explicit-time sampler. It does not create a universal Engine scheduler,
component-local update callback, ambient subscription, persisted closure, or second gameplay
authority. Browser/WebGL realization remains in the isolated renderer backend and host; the asset
and sampling mechanisms remain usable headlessly.

## Runtime admission, playback, and shared realization

`voxel-object-runtime` is the host-neutral live admission owner. It strictly decodes the canonical
object, resolves every complete frame under caller-selected runtime work limits, meshes local cells
around the durable fractional pivot, and deduplicates equal occupancy hashes into reusable mesh
payloads. `svc-mesh` supplies the deterministic visible-face mechanism through a bounded
standalone-cell entry point; neither crate imports a renderer, browser, filesystem, collision
world, navigation world, or scheduler.

Runtime readouts expose the exact object/content identity, frame and clip counts, per-clip frame
indices and integer-microsecond durations, and unique mesh count. Admission bounds frame count,
aggregate resolved cells, and aggregate unique visible faces before unbounded live allocations.
Equal input produces byte-for-byte equal mesh streams independent of source cell order.

`VoxelObjectPlayer` accepts caller-provided integer microsecond time. Its named `play`, `pause`,
`resume`, and `stop` operations support once, repeat, and ping-pong selection plus an exact rational
speed. Per-frame duration overrides take precedence over the clip rate. A durable posture stores
clip, mode, rate, status, and elapsed caller time; restoring it attaches a fresh transient time
anchor, so renderer clocks remain disposable and are never serialized.

Collision is deliberately outside playback. `VisualOnly`, one explicit stable default or clip
frame, and an external static-mesh proxy are the only runtime policies. Sampling a different visible
frame does not mutate the selected collision cells and has no path to collision or navigation
rebuilds.

The Rust render contract defines one deduplicated voxel-object resource, stable instances, explicit
frame swaps, and explicit resource release. `render-projection` converts admitted meshes and keeps
instance handles stable across frame changes. The TypeScript retained projection mirrors this
lifecycle, while the shared Three backend uploads one geometry per unique mesh, shares it across
instances, swaps only the selected geometry, and disposes all object GPU resources on explicit
release. Headless Rust and TypeScript tests prove admission, timing, reload, invalid-frame,
collision isolation, and cleanup; the focused Chromium/WebGL gate proves the same frame swap through
the public shared renderer surface.

## Implementation schedule

Den parent task `rusty-engine#6234` owns the campaign:

| Task | Slice | Dependency |
|---|---|---|
| #6235 | Reusable frames and canonical voxel-object asset | none |
| #6236 | Bounded multi-node/multi-mesh GLB scene import | #6235 |
| #6237 | Conservative surface coverage and real solid interior classification | #6236 |
| #6238 | Deterministic skin/morph/clip sampling | #6236 |
| #6239 | Temporally stable animated voxel conversion | #6237 and #6238 |
| #6240 | Runtime admission, meshing, projection, and playback | #6235 |
| #6241 | Complete Studio object/flipbook authoring | #6239 and #6240 |
| #6242 | Exact-revision runtime consumer and quality/performance closeout | #6241 |

The fork after #6235 is deliberate: renderer/runtime work can prove a hand-authored object while
import and voxelization improve independently. Animated conversion waits for both accurate geometry
and sampled poses; Studio waits for a real converter and a shared runtime renderer path.

## Evidence and exclusions

The Asha portability report and current conversion donors establish bounded static import,
conversion planning, validation, and offline ownership. They do not contain the voxel flipbook
workflow. VoxelForge contributes product evidence for frame clips, per-frame timing, serialization,
editing, and CPU skeletal sampling. Its C# object graph, Assimp loader, MCP/bridge services, editor
session, and rendering topology are not transferred.

M12 explicitly excludes Asha RuntimeSession/replay/provider/project-bundle structure, a universal
animation or gameplay graph, collision derived from every visible pose, browser paths in Rust, and
an operational dependency on a sibling consumer checkout.
