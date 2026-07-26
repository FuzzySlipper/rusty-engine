# Stored voxel asset and offline conversion boundary

Status: current provider format and offline conversion boundary; downstream product proof is
historical evidence.

This is the successor's smallest durable border between a real static mesh and admitted voxel
content. It is deliberately an authoring/build path, not a runtime protocol:

```text
GLB bytes + explicit conversion request
  -> offline Rust converter
  -> canonical schema-1 voxel-volume JSON
  -> downstream asset/project admission
  -> downstream material-voxel authority
```

This document describes authoritative `voxel-volume/...` assets. Reusable local-space models and
animated frame-swap clips use the separate voxel-object meaning documented in
[voxel-model-conversion.md](voxel-model-conversion.md); visible object frames do not implicitly
become environment edits or collision/navigation authority.

Runtime consumers never read GLB, invoke conversion, or discover a provider. A
content hash detects artifact drift and makes reproducibility inspectable; it is not a gameplay
revision, replay certificate, action precondition, or runtime lifecycle owner.

## Real source and provenance

The selected source is the repository-local fixture
`fixtures/voxel-conversion/kenney-wall-a.glb`, copied unchanged from the pinned Asha donor:

- donor commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`;
- source size: 3,352 bytes;
- SHA-256: `6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00`;
- parsed evidence: one embedded static GLB mesh, 48 positions, 36 indices, two triangle groups, and
  the `wall_lines`/`concrete` material slots;
- license: Kenney Retro Urban Kit 2.0, CC0, recorded in the adjacent
  `KENNEY-RETRO-URBAN-KIT-LICENSE.txt`.

The adjacent license is copied unchanged with the fixture. Their hashes above preserve the donor
identity while conversion, tests, CI, and product verification use only this repository-local path.
The fixture, license, and audited conversion/import paths were also byte-unchanged at inspected Asha
head `6462a6de20d48ea1a3b7456826804bd9507860a5`.

## Schema 1

`voxel-asset` owns the strict serde shape, semantic validation, canonical encoding, and conversion
input values. The artifact records:

| Field family | Meaning |
|---|---|
| `assetId` | A strict `voxel-volume/...` identity compatible with the M5 asset catalog. |
| `grid` | Right-handed Y-up coordinates, positive cell size, chunk size `1..=64`, and the engine-cell address of local `[0,0,0]`. |
| `bounds` | Exact inclusive local bounds derived from represented cells. |
| `representation` | Bounded sparse runs along +X; omitted cells are empty. |
| `materialMap` | Explicit source material slot/name to runtime voxel material slot `1..=4095`. |
| `provenance` | Source path/hash/byte count, converter id, canonical settings hash, and optional license path. |
| `contentHash` | SHA-256 of the canonical semantic artifact with only this field cleared. |

World addresses are `grid.origin + local cell coordinate`. Runs are sorted by coordinate and
adjacent same-material runs are merged. Material mappings are sorted by source slot. Object field
order comes from concrete structs; canonical JSON is pretty-printed with LF endings and one trailing
newline. Bounds, mappings, runs, provenance, and content hash are all revalidated on decode.

Hard format limits match the already-proven spatial boundary rather than Asha's much broader
authoring ceilings:

- 16 MiB artifact bytes;
- 1,000,000 represented solid voxels;
- 4,095 source-material mappings and runtime material slots `1..=4095`;
- absolute mapped engine coordinate 1,000,000;
- 4,096 UTF-8 bytes per provenance/material string.

Validation returns classified `code`, `path`, and `message` diagnostics. Unknown JSON fields,
unsupported schemas, wrong asset kinds, invalid grids/bounds, duplicate or overlapping runs,
unmapped materials, bad provenance, excessive resources, and hash drift all fail closed.

## Deterministic conversion input

One `VoxelConversionRequest` fixes the source path and expected SHA-256 before parsing. Its settings
fix resolution, cell size, chunk size, engine origin, fit policy (`contain`, `cover`, or `stretch`),
origin policy (`sourceOrigin`, `targetMin`, or `centered`), mode (`surface` or `solid`), the complete
material map, and a maximum output count. Material-map order does not affect the settings hash.

Preflight rejects empty or greater-than-64-MiB sources, resolution axes outside `1..=256`, grids over
16,777,216 candidate cells, mapped coordinates outside the engine bound, output budgets outside
`1..=1,000,000`, duplicate source slots, and invalid material slots. The parser adds limits of
2,000,000 positions and 6,000,000 indices. Conversion must never partially replace a known-good
artifact.

## Implemented conversion

`voxel-convert` is a separate workspace crate with no downstream-runtime dependency. Its GLB importer
reads one explicit default scene backed by an embedded BIN chunk. It traverses bounded roots and
children deterministically, composes finite affine node transforms, admits multiple mesh nodes and
primitive groups, and retains exact source node/mesh/primitive/material identities plus bounded UV
sets. The ordinary static converter receives one explicitly flattened model-space mesh; the
higher-level import receipt can select the whole model, `group/<n>`, or one exact `node/<n>` subset.
The separate hash-pinned conversion-plan transform still positions the resulting selected model as
an authoring choice rather than replacing source-scene semantics.

Static import rejects animation, skinning, morph targets, instance weights, hierarchy cycles or
multiply-parented nodes, external buffers, non-triangle modes, implicit or invalid indices,
non-finite transforms/positions/UVs, degenerate transformed geometry, excessive hierarchy or
expanded geometry, and ambiguous/missing selections. External image URIs are not fetched or treated
as geometry dependencies. The cohesive imported scene retains mesh-local primitives and node
transforms so the later animation owner can deform the same family without a second parser.

Surface mode maps the source through the explicit fit/origin settings and conservatively tests
bounded triangle candidate cells with a triangle/box separating-axis test. It retains closest-point
barycentric, triangle, and material evidence until deterministic conflict resolution and per-cell
palette sampling. Imported UV attributes have selected-geometry SHA-256 identities; texture-bound
cells interpolate the requested `TEXCOORD_n` and sample the bounded palette texture independently.

Solid mode first requires a closed, consistently wound geometric manifold. Topology identity unifies
vertices only when all three finite positions are exactly coincident (with signed zero normalized),
so face-local render vertices split by UV, normal, or material seams do not create false openings.
There is deliberately no scale-dependent epsilon weld: near-coincident positions remain distinct so
real gaps are not silently closed. Solid conversion then retains conservative boundary evidence and
uses bounded perturbed X-ray parity to classify actual interior cell centers. Interior cells inherit
the nearest positive-X exit material evidence, so multiple closed shells leave cavities empty. The
shared ten-million-operation geometric work meter charges conservative candidate tests,
ray/triangle tests, and interior classification; receipts expose the exact count. The checked
Kenney artifact remains configured for surface mode as an authored output choice; its coincident
face-seam vertices are accepted by solid conversion and are not an unsupported-topology boundary.

The higher-level conversion owner imports a hash-pinned source with deterministic bounds, groups,
and material-slot metadata, then separates `plan`, bounded `preview`, and guarded `apply`. Plans add
an affine transform, default material fallback, and optional nearest-texel palette sampling while
retaining the stored conversion settings above. Preview and apply reject stale plan/output hashes;
apply installs only a complete canonical candidate. Bounded model-info and voxel-window queries make
the resulting asset inspectable without a runtime facade or replay session.

The checked request at `content/conversion/kenney-wall-a.request.json` produces
`content/assets/kenney-wall-a.voxel.json`:

| Result | Value |
|---|---:|
| Imported geometry | 48 positions / 12 triangles / 2 material groups |
| Converted authority | 8 voxels / 4 sparse runs / local bounds `[0,0,0]..[1,1,1]` |
| Geometric work | 48 candidate/intersection operations |
| Converter | `rusty-engine.mesh-to-voxel.v2` |
| Settings SHA-256 | `550002c71046096dcb2ce72653a73fb3755a41c070a36f5afee195313d86c297` |
| Content hash | `1afe2e2d29272f2ac35ae577cc038f1e7fed75b03459bd07c441547abb1eb058` |
| Artifact file SHA-256 | `9dd9a24b4c6728450173f0ea4ff7279310f0611042919d16d552482fc649d6d0` |

Run the direct authoring tool with:

```bash
cargo run -q -p voxel-convert --bin voxel-convert -- \
  --request content/conversion/kenney-wall-a.request.json \
  --source fixtures/voxel-conversion/kenney-wall-a.glb \
  --output content/assets/kenney-wall-a.voxel.json
```

The CLI reads at most 1 MiB plus one byte for its request and 64 MiB plus one byte for its source, so
the filesystem entrypoint enforces the same bounds before retaining complete inputs. The tool then
completes parsing, conversion, validation, and canonical encoding before touching the target. It
writes and syncs a same-directory pending file, then atomically renames it into place.
Stale identity, malformed source, unsupported topology, material-map drift, excess work/output,
invalid artifact content, and I/O failure return nonzero with a classified path; conversion failure
cannot replace a prior good target.

The artifact is consumer-neutral. The extracted reference demo embeds it on a
`voxel-volume/...` catalog entry, validates grid compatibility during its own admission, expands the
sparse runs, and feeds `VoxelCollisionScene::from_material_voxels`. Engine's provider tests prove
the artifact itself remains strict and byte-reproducible; the consumer owns its schema and admission
policy.

The M7B.2 implementation is pinned by
`b3481fadf1586c2cfea167d569af0bd6333af6b5`.

## Historical product, persistence, and workload proof

The proof below was established before the walking product moved to
[`rusty-engine-demo`](https://github.com/FuzzySlipper/rusty-engine-demo). Its game schema, browser
gate, and persistence code are no longer Engine commands or files; they remain useful evidence that
the provider artifact works through an ordinary downstream path.

`content/projects/converted-wall.project.json` is a normal schema-v7 stored product. It declares
the canonical asset in the catalog, references it from a material environment, and combines its
four lower/four upper cells with an authored floor. Complete M5 admission expands the artifact
before constructing the existing M7A scene; the runtime never retains a conversion request or
invokes `voxel-convert`. A focused comparison builds the equivalent explicit authored cells and
proves identical material authority, collision, navigation hash, and mesh.

The admitted product starts with 94 solids and a nine-cell probe path. Real Chromium proves the
converted upper wall reaches the retained Three mesh and blocks the actual player. One ordinary
expected-revision transaction clears its four upper cells. The coherent result has 90 solids,
revision 1, a seven-cell probe path, changed authority/navigation/mesh hashes, and a traversable
route. No converter-specific runtime API or projection exists.

Snapshot schema 9 reopens the live revision and concrete authority exactly. The explicit project
save path instead materializes 90 ordinary static material voxels, clears the environment's asset
reference, re-runs complete admission, and installs through the existing M6 store. Reset and a fresh
host reopen identical authority/navigation/mesh at live revision zero, with no request, receipt,
event, history, annotation, or replay field.

Run the bounded conversion measurement with:

```bash
cargo run --release -q -p voxel-convert --bin voxel-conversion-workload -- 256
```

On the checked source after geometric voxelization, 256 full parse-and-convert passes were
byte-identical and averaged 54.7 us (272 us maximum, 18,276 conversions/s) on the current provider
host. Each conversion charged 48 geometric operations and retained the same eight-cell output. The
source/request/output sizes were 3,352/1,080/2,109 bytes. The historical M7B closeout also measured
256 full M7A projection rebuilds at 525.4 us average and 1,172 us maximum, retaining a 90,756-byte
mesh payload; that projection number was not rerun by M12C.

The M7B.3 product/persistence/workload implementation is pinned by
`2cdad99c0d012643fe157fa6db51495a31327d98`. M7C annotations/history remains unscheduled absent a
named undo, provenance, collaboration, or diagnostic consumer.

## Donor audit and exclusions

The useful donor evidence is narrow:

- `svc-mesh-import`: bounded host-provided GLB 2.0 parsing, one static embedded-BIN mesh, indexed
  triangle groups, finite positions, stable material slots, source SHA-256, and classified failure;
- `svc-voxel-conversion`: explicit fit/origin/resolution/material settings, deterministic coordinate
  mapping, bounded output, closed-topology validation for solid mode, and coordinate-ordered output;
- `protocol-voxel-asset` plus `svc-voxel-asset`: schema versioning, exact grid/bounds, sparse +X runs,
  material validation, canonical bytes, and content drift detection;
- `asset-import`: offline-only execution, deterministic artifacts, useful diagnostics, and
  write-then-install discipline.

Rusty Engine does not import those crates. It rejects the conversion source registry, model readout,
provider interface, catalog/lock graph, manifest/sidecar system, plan/preview/apply sequence,
evidence URI graph, command registry, runtime bridge, replay records, asset lifecycle, Studio
facade, texture sampling protocol, and project-bundle integration. The selected consumer needs one
direct CLI call and one admitted output artifact, so those abstractions have no owner here.
