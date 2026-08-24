//! Host-neutral Rusty Product Layout and current Compiled Composition schema.
//!
//! This crate validates immutable authoring and generated artifacts. It does
//! not load product files, evaluate TypeScript, schedule systems, mutate live
//! state, or choose a product host.

#![forbid(unsafe_code)]

mod admission;
mod capability_catalog;
mod composition;
mod contract;
mod diagnostic;
mod manifest;
mod path;

pub use admission::{
    admit_checked_product_composition, admit_product_composition, AdmittedCapabilityBinding,
    AdmittedCapabilityReference, AdmittedDefinitionReference, AdmittedGameplayDefinition,
    AdmittedInputMapEntry, AdmittedIntentDescriptor, AdmittedIntentReference,
    AdmittedProductComposition, AdmittedScheduleFragment, AdmittedSchedulePhase,
    AdmittedScheduleSystem, AdmittedTimeline, AdmittedTimelineStep,
};
pub use capability_catalog::{
    engine_capability_descriptors, link_admitted_product_composition,
    validate_engine_capability_descriptors, CapabilityAccess, CapabilityAvailability,
    CapabilityBudget, CapabilityKind, CapabilityMetadata, CapabilityProvenance, CapabilityUse,
    CapabilityUses, EngineCapability, EngineCapabilityDescriptor, LinkedCapabilityBinding,
    LinkedCapabilityTarget, LinkedProductComposition, ProductKernelCapabilityDescriptor,
    ProductKernelCapabilityIndex, MAX_CAPABILITY_PROVENANCE_BYTES, MAX_PRODUCT_KERNEL_CAPABILITIES,
};
pub use composition::{
    decode_compiled_composition, encode_compiled_composition, validate_compiled_composition,
    CapabilityBinding, CompiledComposition, CompiledCompositionCandidate, ControllerAxis,
    ControllerButton, GameplayDefinition, InputAxis, InputEdge, InputMapEntry, InputTrigger,
    IntentValueKind, KeyboardControl, PointerButton, ProductIntentDescriptor, ScheduleCadence,
    ScheduleComposition, ScheduleCompositionMode, SchedulePhase, SchedulePhaseDeclaration,
    SchedulePlacement, ScheduleSystem, Timeline, TimelineStep, MAX_CAPABILITY_BINDINGS,
    MAX_COMPILED_COMPOSITION_BYTES, MAX_GAMEPLAY_DEFINITIONS, MAX_INPUT_CHORD_CONTROLS,
    MAX_INPUT_MAP_ENTRIES, MAX_INTENT_DESCRIPTORS, MAX_OPAQUE_JSON_ARRAY_ENTRIES,
    MAX_OPAQUE_JSON_DEPTH, MAX_OPAQUE_JSON_NODES, MAX_OPAQUE_JSON_OBJECT_ENTRIES,
    MAX_OPAQUE_JSON_STRING_BYTES, MAX_SAFE_JSON_INTEGER, MAX_SCHEDULE_ACCESS_DECLARATIONS,
    MAX_SCHEDULE_DEPENDENCIES, MAX_SCHEDULE_ENTRIES, MAX_TIMELINES, MAX_TIMELINE_STEPS,
    SCHEDULE_PHASE_COUNT,
};
pub use contract::encode_product_model_contract_descriptor;
pub use diagnostic::{ProductModelDiagnostic, ProductModelError, MAX_DIAGNOSTIC_MESSAGE_BYTES};
pub use manifest::{
    decode_product_manifest, validate_product_manifest, LifecycleMode, ProductManifest,
    ProductManifestCandidate, RealtimeClock, ReleaseChannel, WrapperCandidate, WrapperDeclaration,
    WrapperKind, MAX_COMPOSITION_ENTRYPOINTS, MAX_IDENTITY_BYTES, MAX_PRODUCT_MANIFEST_BYTES,
    MAX_REALTIME_CATCH_UP_STEPS, MAX_REALTIME_HZ, MAX_WRAPPERS, MAX_WRAPPER_PERMISSIONS,
};
pub use path::ProductPath;
