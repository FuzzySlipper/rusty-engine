# Workspace math vocabulary (#8846)

**Question.** `render-wgpu` brought glam into an Engine that otherwise uses
`core-math` and plain arrays. Should glam become the workspace math library,
or stay private to the renderer?

**Decision: keep glam private to `render-wgpu`** (option 2). A named module
now owns the conversions, and the boundary check enforces the rule.

## What the crates need

| Crate family | What it uses | What it needs |
|---|---|---|
| `core-math` consumers: `entity-state`, `engine-spatial`, `authored-scene`, `csharp-engine-services`, `environment-authoring`, `svc-pathfinding` | f32 `Vec2`/`Vec3` from 209 lines of `core-math`: `new`, `dot`, `cross`, `length`, operators | Plain value types that serialize and cross the ABI as arrays. No matrices or quaternion algebra. |
| Spatial and collision: `svc-collision`, `svc-implicit`, `engine-spatial` world positions | f64 world coordinates. nalgebra through `parry3d-f64` and `rapier3d-f64`, and directly in `svc-implicit` | f64 and nalgebra at the collision seam. Promoting glam would not remove nalgebra there; it would add a third library at that seam. |
| Voxel and animation import (`voxel-convert`) | f64 quaternion `slerp` and a 4×4 multiply, written by hand | Import-time f64. Two small helpers do not justify a workspace library. |
| `render-wgpu` | glam `Mat4`, `Quat`, `Vec3` | f32 column-major matrices packed into GPU rows every frame. This is glam's purpose. |

Promotion would touch every `core-math` consumer and change nothing the
simulation side computes. The renderer is the only crate doing f32 matrix and
quaternion work, so glam stays there.

## What changed

- **`render-wgpu/src/convert.rs`.** The single seam between Engine arrays and
  glam:
  - `vec3` and `world_vec3` for f32 presentation values and f64 camera or pick
    values;
  - `rotation` and `transform_matrix`;
  - `array` and `world_array` for values written back into Engine descriptors,
    facts and readouts.

  `tables::transform_matrix` and `camera.rs`'s private f64 helper moved here.
  The inline conversions in `apply`, `animated`, `frame`, `ghost`, `labels`,
  `particles`, `pick` and `shadows` now call it.
- **Not moved:** GPU buffer packing (`to_cols_array` into uniform and instance
  rows) stays with the pass that owns the layout, and `glb.rs` decodes its
  glTF file straight into glam. Neither crosses the Engine seam. #8847 decides
  `glb.rs`'s future.
- **`scripts/dependency_boundary_check.py`.** `glam` is owned by `render-wgpu`
  in `EXTERNAL_DEPENDENCY_OWNERS`.
- **`docs/architecture.md`.** The Source owners table gains a "Math
  vocabulary" row.

## Evidence

- **No seam conversions left.** No `Vec3::from`, `from_array`, `DVec3` or
  `as_dvec3` remains in non-test `render-wgpu` code outside `convert.rs` and
  `glb.rs`. Checked with a scan of each file up to its test module.
- **Boundary check:** passes on the workspace (43 packages).
- **Rule enforced:** with glam injected into `core-math`'s dependencies in the
  cargo metadata, the check fails with "core-math depends on glam, which only
  render-wgpu may depend on".
- **Behaviour unchanged:** `cargo test -p render-wgpu` passes all 65 tests.
  That includes the screenshot suites, which need an adapter and compare with
  the reference PNGs.
- **Clippy:** `-D warnings` is clean for `render-wgpu`.
