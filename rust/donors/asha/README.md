# Internalized Asha Rust donors

These crates are a bounded same-owner source transfer from
`git@github.com:FuzzySlipper/asha-engine.git` at commit
`a431974330589761c9e35fc4f8a55996a1b5ee48`.

The accepted family is limited to:

- `foundation`: `core-assets`, `core-ids`, `core-math`, `core-space`, `core-time`;
- `state`: `core-voxel`;
- `services`: `svc-volume`, `svc-spatial`, `svc-collision`, `svc-pathfinding`, `svc-rng`, `svc-mesh`.

Source directories and package names are retained so provenance and focused donor history remain
easy to compare. Rusty Engine owns the public composition boundary above them; this directory is not
an Asha runtime compatibility layer.

Production source is copied unchanged except for the explicitly bounded seams: `core-ids` retains
only the consumed `EntityId`, fixture-relative paths are local, and `svc-pathfinding`'s tests replace
their dev-only dependency on excluded `svc-levelgen` with the same tiny shell assembled directly as
a `VoxelWorld`. Fixture hashes and complete treatment are
recorded in [`docs/m9-extraction-contract.md`](../../../docs/m9-extraction-contract.md) and
[`docs/donor-provenance.md`](../../../docs/donor-provenance.md).

The donor repository and these package manifests declare no code license or notice. This README
records provenance without inventing a license assertion. Registry dependencies retain their own
published licenses through Cargo metadata and the workspace lockfile.
