# Stored voxel asset and offline conversion boundary

Status: M7B.1 format boundary implemented; conversion algorithm follows in M7B.2.

This is the successor's smallest durable border between a real static mesh and admitted voxel
content. It is deliberately an authoring/build path, not a runtime protocol:

```text
GLB bytes + explicit conversion request
  -> offline Rust converter
  -> canonical schema-1 voxel-volume JSON
  -> ordinary schema-7 project asset/environment reference
  -> existing M5 admission
  -> existing M7A material-voxel authority
```

The runtime never reads GLB, invokes conversion, discovers a provider, or executes TypeScript. A
content hash detects artifact drift and makes reproducibility inspectable; it is not a gameplay
revision, replay certificate, action precondition, or runtime lifecycle owner.

## Real source and provenance

The selected source is the pinned Asha donor fixture
`../asha-engine/harness/fixtures/voxel-conversion/kenney-wall-a.glb`:

- donor commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`;
- source size: 3,352 bytes;
- SHA-256: `6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00`;
- parsed evidence: one embedded static GLB mesh, 48 positions, 36 indices, two triangle groups, and
  the `wall_lines`/`concrete` material slots;
- license: Kenney Retro Urban Kit 2.0, CC0, recorded in the adjacent
  `KENNEY-RETRO-URBAN-KIT-LICENSE.txt`.

The fixture and license are referenced rather than duplicated. CI already checks out the exact
pinned sibling donor used by every other retained dependency. The fixture, license, and audited
conversion/import paths are byte-unchanged at current Asha head
`6462a6de20d48ea1a3b7456826804bd9507860a5`.

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
fix resolution, cell size, chunk size, engine origin, fit policy (`contain` or `stretch`), origin
policy (`targetMin` or `centered`), mode (`surface` or `solid`), the complete material map, and a
maximum output count. Material-map order does not affect the settings hash.

Preflight rejects empty or greater-than-8-MiB sources, resolution axes outside `1..=256`, grids over
16,777,216 candidate cells, mapped coordinates outside the engine bound, output budgets outside
`1..=1,000,000`, duplicate source slots, and invalid material slots. The parser adds limits of
250,000 positions and 750,000 indices in M7B.2. Conversion must never partially replace a known-good
artifact.

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
