# Voxel model conversion and flipbook workflow

Status: M12 working design and implementation schedule

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

The important gaps are behavioral rather than language-boundary gaps:

- static import now handles bounded scene hierarchy and multiple meshes, but animation sampling,
  skins, morph targets, and instance weights remain the separate #6238 responsibility;
- the durable format and Studio controls describe one volume, not a reusable object and clips; and
- retained voxel projection can replace changed chunk meshes, but no voxel-object resource,
  explicit clip sampler, or presentation-only playback owner exists.

The existing static workflow remains supported while these gaps close. M12 is an extension and
quality campaign, not a rewrite of conversion or Studio.

## Durable voxel-object schema 1

`voxel-asset` owns `VoxelObjectAsset` beside the unchanged `VoxelAsset` format. A voxel object has:

- a `voxel-object/...` identity;
- right-handed Y-up local coordinates, positive cell size, chunk size, and a finite possibly
  fractional local pivot;
- one required default `VoxelFrame`;
- shared material palette, source-material mapping, and source/settings/tool provenance;
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
UV sets, UTF-8 names, expanded vertices, and expanded indices. It rejects cycles, nodes reached from
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

Solid conversion first requires unique faces and exactly two oppositely directed uses of every
indexed edge. It keeps conservative boundary cells, then classifies voxel centers by deterministic
X-ray parity through the closed mesh. A bounded set of tiny row perturbations avoids vertex/edge
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
ambiguous; and requests whose conservative candidate or parity work exceeds ten million operations
must lower resolution or split the selected model.

## Intended animated conversion path

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

Rust owns source parsing, deformation, voxelization, canonical assets, validation, frame
resolution, and renderer-neutral values. Studio owns source and clip selection, forms, preview
scrubbing, transient playback, and explicit apply/discard. A downstream product owns gameplay
meaning and when to request playback.

Playback will be a named presentation mechanism or an explicit-time sampler. It will not create a
universal Engine scheduler, component-local update callback, ambient subscription, persisted
closure, or second gameplay authority. Browser/WebGL realization remains in the isolated renderer
backend and host; the asset and sampling mechanisms remain usable headlessly.

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
