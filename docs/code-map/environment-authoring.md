# Environment authoring

## Purpose

Route reusable environment-recipe planning, pure preview, and deterministic
materialization into authored scene and voxel mechanisms.

## Owns

- Typed environment recipes and validation.
- Deterministic generation plans and previews.
- Revision-bound materialization into successor-owned authored scene and voxel
  operations.

## Does not own

- A universal procedural-generation framework.
- Product UI, editor interaction, game-specific level meaning, or scheduling.
- Independent scene, entity, voxel, or persistence authority.

## Primary paths

- [`environment-authoring/src/lib.rs`](../../rust/crates/environment-authoring/src/lib.rs)
- [`environment-authoring/src/generation.rs`](../../rust/crates/environment-authoring/src/generation.rs)
- [`environment-authoring/src/materialization.rs`](../../rust/crates/environment-authoring/src/materialization.rs)
- [`environment-authoring/tests`](../../rust/crates/environment-authoring/tests)

## Public downstream surfaces

- Downstream Rust can validate recipes, inspect pure plans/previews, and choose
  when to materialize them.
- Materialization uses named scene and spatial owners; it does not introduce a
  parallel world representation.

## Private or forbidden paths

- Do not embed callbacks, scripts, or renderer resources in recipes.
- Do not make preview state authoritative.
- Do not widen a concrete recipe into a universal authored-behavior language.
- Do not move product-specific generation policy upstream without multiple
  concrete consumers.

## Acceptance gates and fixtures

```bash
cargo test -p environment-authoring --locked
cargo clippy -p environment-authoring --all-targets --locked -- -D warnings
./scripts/verify.sh
```

## Common agent mistakes

- Combining recipe validation, preview, and committed materialization into one
  implicit mutation path.
- Hiding scene or voxel ownership behind a generator abstraction.
- Treating a downstream content recipe as a reusable Engine mechanism before
  a second consumer exists.

## Follow-up routing

- Scene and persistence contracts:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Spatial materialization: [Spatial mechanisms](spatial-mechanisms.md).
- Studio presentation of authoring operations: [Studio](studio.md).

