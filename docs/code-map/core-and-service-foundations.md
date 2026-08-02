# Core and service foundations

## Purpose

Route low-level identity, asset-reference, math, time, coordinate, voxel-value,
volume, spatial-index, collision, pathfinding, RNG, and mesh mechanisms.

## Owns

- `core-assets`, `core-ids`, `core-math`, `core-time`, `core-space`, and
  `core-voxel`: small typed foundations.
- `svc-volume`, `svc-spatial`, `svc-collision`, `svc-pathfinding`, `svc-rng`,
  and `svc-mesh`: focused reusable algorithms with explicit inputs and outputs.
- Stable coordinate, scalar, identifier, and material vocabulary used by
  higher-level owners.

## Does not own

- Cohesive game orchestration, content policy, rendering, persistence policy,
  or host integration.
- Hidden global state or service location.
- A reason to split every small type into another crate.

## Primary paths

- [`core-assets`](../../rust/crates/core-assets)
- [`core-ids`](../../rust/crates/core-ids)
- [`core-math`](../../rust/crates/core-math)
- [`core-time`](../../rust/crates/core-time)
- [`core-space`](../../rust/crates/core-space)
- [`core-voxel`](../../rust/crates/core-voxel)
- [`svc-volume`](../../rust/crates/svc-volume)
- [`svc-spatial`](../../rust/crates/svc-spatial)
- [`svc-collision`](../../rust/crates/svc-collision)
  - [`static_mesh.rs`](../../rust/crates/svc-collision/src/static_mesh.rs) owns
    bounded immutable triangle assets, exact-revision instance projection, and
    Parry-backed ray/AABB/sweep queries.
- [`svc-pathfinding`](../../rust/crates/svc-pathfinding)
- [`svc-rng`](../../rust/crates/svc-rng)
- [`svc-mesh`](../../rust/crates/svc-mesh)

## Public downstream surfaces

- Crate roots expose small, directly callable mechanisms.
- `engine-spatial` is the preferred cohesive owner when several spatial
  services must share canonical mutable state.
- New downstream use should depend only on the narrow foundations it actually
  needs.
- Static triangle instances retain only derived query state here. Callers own
  asset resolution, entity identity/transforms, persistence, and replacement
  timing.

## Private or forbidden paths

- Do not introduce entity, gameplay, scene, renderer, Studio, or browser
  concepts into foundations.
- Do not add ambient registries or callback-driven update frameworks.
- Do not turn a convenient helper into a universal policy abstraction.
- Keep the sole heavyweight collision dependency localized to `svc-collision`.

## Acceptance gates and fixtures

```bash
cargo test -p core-assets -p core-ids -p core-math -p core-time -p core-space -p core-voxel --locked
cargo test -p svc-volume -p svc-spatial -p svc-collision -p svc-pathfinding -p svc-rng -p svc-mesh --locked
./scripts/verify.sh
```

## Common agent mistakes

- Moving higher-level policy downward to avoid a dependency edge.
- Creating a new crate for a type that belongs to an existing cohesive owner.
- Using renderer or product coordinate conventions as canonical spatial truth.
- Adding a reusable abstraction before more than one concrete consumer needs
  it.

## Follow-up routing

- Cohesive spatial mutation: [Spatial mechanisms](spatial-mechanisms.md).
- Entity identity and components:
  [Entity state and state machines](entity-state-and-state-machines.md).
- Asset catalogs and storage:
  [Content, assets, and scenes](content-assets-and-scenes.md).
