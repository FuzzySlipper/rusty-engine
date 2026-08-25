//! Complete namespace-preserving facade for ordinary downstream games.
//!
//! This crate is a leaf over the focused Engine owners. It introduces no
//! runtime, service location, wrappers, or replacement data types. Consumers
//! reach the exact owning APIs through stable owner namespaces.

#![forbid(unsafe_code)]

pub use asset_catalog;
pub use asset_import;
pub use authored_scene;
pub use content_store;
pub use core_assets;
pub use core_ids;
pub use core_math;
pub use core_space;
pub use core_time;
pub use core_voxel;
pub use developer_command;
pub use developer_command_standard;
pub use engine_inspector;
pub use engine_spatial;
pub use entity_state;
pub use environment_authoring;
pub use gameplay_continuous_mechanics;
pub use gameplay_mechanics;
pub use gameplay_resolution;
pub use gameplay_rules;
pub use gameplay_standard;
pub use product_assembly;
pub use product_dev_host;
pub use product_kernel;
pub use product_materializer;
pub use product_model;
pub use render_host_contracts;
pub use render_model;
pub use render_presentation;
pub use render_projection;
pub use renderer_webview_host;
pub use runtime_composition;
pub use runtime_input;
pub use runtime_lifecycle;
pub use runtime_mutation;
pub use runtime_schedule;
pub use runtime_standard_capabilities;
pub use runtime_timeline;
pub use runtime_ui;
pub use state_machine;
pub use svc_collision;
pub use svc_mesh;
pub use svc_pathfinding;
pub use svc_rng;
pub use svc_spatial;
pub use svc_volume;
pub use voxel_annotation;
pub use voxel_asset;
pub use voxel_convert;
pub use voxel_object_runtime;
