# Content, assets, and scenes

## Purpose

Route durable content manifests, asset identity and catalogs, offline mesh
imports, prefab resolution, and authored scene admission and editing.

## Owns

- `content-store`: bounded source batches, content hashes, manifests, owner
  codecs, load/save plans, prefabs, and atomic write sets.
- `asset-catalog`: asset entries, versions, locks, dependencies, materials,
  fallbacks, and catalog validation.
- `asset-import`: bounded offline textual static-mesh, binary GLB, and explicit
  JSON glTF source-closure admission; deterministic GLB runtime packing; import
  planning, generated artifacts, sidecars, reimport decisions, and atomic
  publication.
- `authored-scene`: versioned scene documents, hierarchy, references, lights,
  validation, admission plans, and explicit edit commands.

## Does not own

- Product filesystem policy or a universal project schema.
- Live game orchestration, implicit entity updates, or renderer state.
- UI workflows for selecting, editing, or saving content.

## Primary paths

- [`content-store/src/lib.rs`](../../rust/crates/content-store/src/lib.rs)
- [`content-store/src/manifest.rs`](../../rust/crates/content-store/src/manifest.rs)
- [`asset-catalog/src/lib.rs`](../../rust/crates/asset-catalog/src/lib.rs)
- [`asset-import/src/lib.rs`](../../rust/crates/asset-import/src/lib.rs)
- [`asset-import/src/gltf_package.rs`](../../rust/crates/asset-import/src/gltf_package.rs)
- [`authored-scene/src/lib.rs`](../../rust/crates/authored-scene/src/lib.rs)
- [`authored-scene/src/admission.rs`](../../rust/crates/authored-scene/src/admission.rs)

## Public downstream surfaces

- Crate roots expose validated codecs, plans, diagnostics, and explicit
  mutation services.
- The `rusty-asset-import` binary is an offline producer, not a runtime asset
  loader.
- `plan_animated_glb_import` emits exact retained GLB bytes, a validated
  `AnimatedMeshAsset`, catalog entry, and provenance manifest. It consumes the
  canonical bounded GLB scene/skin/clip parser without sampling or voxelizing
  the source.
- `admit_gltf_source` accepts root JSON plus an immutable set of canonical
  resource identities and bytes. `gltf_relative_resource_uris` tells an
  explicit filesystem-owning adapter what to load; neither function performs
  I/O. `plan_animated_gltf_import` fingerprints that complete closure and packs
  it into the existing GLB runtime artifact before using the same importer.
- The CLI resolves those relative resource identities beneath the selected
  root's directory after canonicalization. Data URIs are decoded only by the
  bounded Rust admission path. Network, absolute, traversal, query/fragment,
  duplicate/colliding, missing, extra, unsupported MIME/extension, and quota
  failures produce no publication candidate.
- Consumers decide where content bodies live and how admitted scene entities
  become game-owned live state.
- `authored-scene` admission produces validated built-in `EntityDefinition`
  data and commits it through `EntityAuthoringService`; those built-ins enter
  the same typed store used by registered downstream components. A downstream
  project schema remains responsible for admitting its additional component
  families explicitly.

## Private or forbidden paths

- Do not perform ambient filesystem I/O from data components.
- Do not teach the generic component store to interpret one product's scene or
  project schema, and do not serialize runtime-only components merely because
  an authored scene mentions their owner.
- Do not let a content manifest become an unbounded catch-all for game policy.
- Do not make fallback rendering authoritative for collision or gameplay.
- Do not mutate generated artifacts or manifests without their validating
  owner codecs and atomic publication path.

## Acceptance gates and fixtures

```bash
cargo test -p content-store -p asset-catalog -p asset-import -p authored-scene --locked
cargo clippy -p content-store -p asset-catalog -p asset-import -p authored-scene --all-targets --locked -- -D warnings
./scripts/verify.sh
```

Focused regressions live in each crate's `tests/` directory.

## Common agent mistakes

- Treating storage layout as Engine-wide product policy.
- Combining scene admission with game-specific spawn behavior.
- Bypassing hashes, size limits, validation, or atomic write-set checks.
- Coupling a Rust content type to Studio or browser filesystem APIs.

## Follow-up routing

- Scene recipe generation: [Environment authoring](environment-authoring.md).
- Voxel-specific artifacts: [Voxel assets and conversion](voxel-assets-and-conversion.md).
- Authoring UI and external-project operations: [Studio](studio.md).
