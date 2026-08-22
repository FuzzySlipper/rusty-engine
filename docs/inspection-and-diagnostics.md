# Inspection and diagnostics

Rusty Engine's diagnostics are observational views over the owners that already validate and hold
state. They are not a runtime spine, repair authority, event route, replay log, or proof system.

The current implementation has two layers:

```text
asset-catalog  authored-scene  entity-state  gameplay-mechanics  engine-spatial  content-store
      \              |             |                 |                  |             /
                         engine-inspector (read-only leaf)
                                      |
                                rusty-inspect CLI
```

No runtime crate depends on `engine-inspector`. The optional
`developer-command-standard` tooling composition leaf and the complete
`rusty-engine` facade may consume it; neither is a runtime owner and no owner
depends back. The inspector may combine immutable views because that dependency
direction cannot give it authority over the owners it observes.

## Structured diagnostic contract

`engine-inspector` uses one small output shape:

- severity is `info`, `warning`, `error`, or `fatal`; only `fatal` means the artifact cannot be
  loaded at all;
- domain identifies the direct owner: catalog, entity state, gameplay mechanics, scene, voxel
  state, persistence, or import;
- code remains an owner-local stable string instead of entering an engine-wide code registry;
- location can name a local path, asset, entity, scene node, or voxel chunk;
- remedies are advisory text and never authorize a mutation.

There is deliberately no render-handle-to-session source trace, global runtime-source taxonomy,
automatic repair verb, or generated cross-language diagnostics union. Renderer and presentation
diagnostics stay with their existing render owners and readouts.

Semantic authoring errors remain explainable. `authored-scene::decode_scene_unvalidated` and
`content-store::decode_manifest_unvalidated` decode only strict stored shapes for inspection; normal
runtime calls continue to use `decode_scene` and `decode_manifest`, which validate before returning.

## Inspection coverage

| Area | Library view | CLI | Notable readout |
| --- | --- | --- | --- |
| Catalog | `inspect_catalog` | `catalog` | kinds, dependency counts, validation, optional lock drift |
| Entity state | `inspect_entity_state` | `entity-state` | lifecycle, source, registered component identities/counts, per-entity presence, relationships, focused queries |
| Gameplay mechanics | `inspect_mechanics_entity_structural`, `inspect_mechanics_entity_from_evidence`, `inspect_damage_receipt` | `mechanics` | structural codec/presence and stored facts; separately supplied stat/effect/inventory evidence can assemble the compatible enriched report; bounded receipt stages and costs |
| Scene | `inspect_scene` | `scene` | hierarchy/kind counts, validation, optional catalog cross-check |
| Voxel state | `inspect_voxel_asset`, `inspect_voxel_state` | `voxel` | occupancy, materials, chunks, mesh/collision/navigation coherence |
| Persistence | `inspect_content_manifest` | `content` | artifact roles/classes and dependency-ordered load plan |
| Imports | `inspect_import_source`, `inspect_import_manifest` | `import-source`, `import-manifest` | real offline-import diagnostics and artifact identities |

The voxel report rebuilds the ordinary `VoxelCollisionScene` from a validated voxel asset. Its
chunk, collision, mesh, navigation, revision, and authority-hash readouts therefore describe the
same projections gameplay queries use rather than a diagnostic-only model.

The mechanics command takes an entity-state snapshot, a strict
`MechanicsCatalogDefinition`, and an entity ID. It admits the caller-supplied
catalog and uses the canonical gameplay registry for reconstruction before
projecting stored state through `inspect_mechanics_snapshot_structural_json_v2`.
The legacy enriched JSON wire shape is available only through
`inspect_mechanics_snapshot_json_v1_from_evidence`, which requires the caller's
already-produced evaluations, effect activations, and inventory view. The catalog
version remains downstream compatibility authority; the fingerprint is diagnostic
evidence only. Component revisions in the report are live instance-local evidence
and therefore restart at restored slot revisions rather than pretending to be durable
history.

Run `cargo run -p engine-inspector --bin rusty-inspect -- --help` for command syntax. Exit status is
zero for a clean inspection, one for an empty focused query, two for read/decode/validation/import
failure, and three for command misuse.

## Donor disposition

The useful portions of Asha's `protocol-diagnostics`, `scene-diagnostics`, `voxel-diagnostics`, and
`state-inspector` are adapted here: structured severity and remedies, stable text, catalog and scene
classification, entity/category summaries, live voxel projection counts, and contextual edit
rejections.

The following concepts are intentionally removed:

- ProjectBundle manifest, cache, composition, and generator routing; `content-store` manifests and
  direct owner load plans are the successor persistence model;
- save/load and session-state equivalence reports, round-trip goldens, source-trace reconstruction,
  and replay-divergence explanations;
- the Asha-wide diagnostic-code/scope tables and developer-console sources tied to Authority,
  Projection, and RuntimeHost topology;
- duplicate renderer-resource accounting in Rust; renderer lifecycle diagnostics, readouts, and
  disposal coverage remain with `render-presentation`, `renderer-host`, and `renderer-three`;
- the old `core-entity` artifact and its replay hash; the CLI decodes `entity-state` snapshots.

These are removed-concept decisions, not deferred ports.

## Verification

The closure gate is:

```bash
cargo test -p engine-inspector
cargo clippy -p engine-inspector --all-targets -- -D warnings
./scripts/check-asha-equivalence.sh --final
./scripts/verify.sh
```

The repository-wide standalone audit rejects external Cargo paths and sibling-repository coupling;
it also verifies that `engine-inspector` remains a dependency leaf.
