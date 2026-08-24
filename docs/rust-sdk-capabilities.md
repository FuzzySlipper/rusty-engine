# Rust SDK capability index

The `rusty-engine` crate is the complete namespace-preserving dependency for
ordinary downstream games. Every public Rust library in this workspace is an
unconditional normal dependency and is re-exported without wrappers. The
namespace remains the Cargo package name with hyphens converted to underscores.

This is a dependency and navigation facade, not a runtime. The owning crates
remain independently meaningful and no owner may depend back on `rusty-engine`.

| Cargo package | Downstream namespace |
|---|---|
| `asset-catalog` | `rusty_engine::asset_catalog` |
| `asset-import` | `rusty_engine::asset_import` |
| `authored-scene` | `rusty_engine::authored_scene` |
| `content-store` | `rusty_engine::content_store` |
| `core-assets` | `rusty_engine::core_assets` |
| `core-ids` | `rusty_engine::core_ids` |
| `core-math` | `rusty_engine::core_math` |
| `core-space` | `rusty_engine::core_space` |
| `core-time` | `rusty_engine::core_time` |
| `core-voxel` | `rusty_engine::core_voxel` |
| `developer-command` | `rusty_engine::developer_command` |
| `developer-command-standard` | `rusty_engine::developer_command_standard` |
| `engine-inspector` | `rusty_engine::engine_inspector` |
| `engine-spatial` | `rusty_engine::engine_spatial` |
| `entity-state` | `rusty_engine::entity_state` |
| `environment-authoring` | `rusty_engine::environment_authoring` |
| `gameplay-continuous-mechanics` | `rusty_engine::gameplay_continuous_mechanics` |
| `gameplay-mechanics` | `rusty_engine::gameplay_mechanics` |
| `gameplay-resolution` | `rusty_engine::gameplay_resolution` |
| `gameplay-rules` | `rusty_engine::gameplay_rules` |
| `gameplay-standard` | `rusty_engine::gameplay_standard` |
| `product-model` | `rusty_engine::product_model` |
| `render-host-contracts` | `rusty_engine::render_host_contracts` |
| `render-model` | `rusty_engine::render_model` |
| `render-presentation` | `rusty_engine::render_presentation` |
| `render-projection` | `rusty_engine::render_projection` |
| `renderer-webview-host` | `rusty_engine::renderer_webview_host` |
| `runtime-input` | `rusty_engine::runtime_input` |
| `runtime-schedule` | `rusty_engine::runtime_schedule` |
| `runtime-timeline` | `rusty_engine::runtime_timeline` |
| `runtime-mutation` | `rusty_engine::runtime_mutation` |
| `runtime-standard-capabilities` | `rusty_engine::runtime_standard_capabilities` |
| `runtime-lifecycle` | `rusty_engine::runtime_lifecycle` |
| `state-machine` | `rusty_engine::state_machine` |
| `svc-collision` | `rusty_engine::svc_collision` |
| `svc-mesh` | `rusty_engine::svc_mesh` |
| `svc-pathfinding` | `rusty_engine::svc_pathfinding` |
| `svc-rng` | `rusty_engine::svc_rng` |
| `svc-spatial` | `rusty_engine::svc_spatial` |
| `svc-volume` | `rusty_engine::svc_volume` |
| `voxel-annotation` | `rusty_engine::voxel_annotation` |
| `voxel-asset` | `rusty_engine::voxel_asset` |
| `voxel-convert` | `rusty_engine::voxel_convert` |
| `voxel-object-runtime` | `rusty_engine::voxel_object_runtime` |

`scripts/check-rust-sdk-coverage.py` derives the library set from Cargo
metadata and checks this table, the facade manifest, and the exact `pub use`
surface together. A new library cannot silently remain absent downstream.
