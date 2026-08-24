//! Host-neutral Rusty Product Layout and current Compiled Composition schema.
//!
//! This crate validates immutable authoring and generated artifacts. It does
//! not load product files, evaluate TypeScript, schedule systems, mutate live
//! state, or choose a product host.

#![forbid(unsafe_code)]

mod composition;
mod diagnostic;
mod manifest;
mod path;

pub use composition::{
    decode_compiled_composition, encode_compiled_composition, validate_compiled_composition,
    CapabilityBinding, CompiledComposition, CompiledCompositionCandidate, GameplayDefinition,
    InputMapEntry, ScheduleEntry, Timeline, TimelineStep, MAX_CAPABILITY_BINDINGS,
    MAX_COMPILED_COMPOSITION_BYTES, MAX_GAMEPLAY_DEFINITIONS, MAX_INPUT_MAP_ENTRIES,
    MAX_OPAQUE_JSON_ARRAY_ENTRIES, MAX_OPAQUE_JSON_DEPTH, MAX_OPAQUE_JSON_NODES,
    MAX_OPAQUE_JSON_OBJECT_ENTRIES, MAX_OPAQUE_JSON_STRING_BYTES, MAX_SAFE_JSON_INTEGER,
    MAX_SCHEDULE_ENTRIES, MAX_TIMELINES, MAX_TIMELINE_STEPS,
};
pub use diagnostic::{ProductModelDiagnostic, ProductModelError, MAX_DIAGNOSTIC_MESSAGE_BYTES};
pub use manifest::{
    decode_product_manifest, validate_product_manifest, LifecycleMode, ProductManifest,
    ProductManifestCandidate, RealtimeClock, ReleaseChannel, WrapperCandidate, WrapperDeclaration,
    WrapperKind, MAX_COMPOSITION_ENTRYPOINTS, MAX_PRODUCT_MANIFEST_BYTES, MAX_WRAPPERS,
    MAX_WRAPPER_PERMISSIONS,
};
pub use path::ProductPath;
