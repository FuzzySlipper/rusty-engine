//! Durable semantic regions over successor voxel assets.
//!
//! Annotation layers are ordinary object-owned content. This crate keeps their
//! model, validation, queries, edits, and export together; it has no runtime
//! facade, event replay, renderer, project bundle, or filesystem authority.

#![forbid(unsafe_code)]

mod codec;
mod edit;
mod model;
mod query;

pub use codec::{
    decode_annotation_layer, encode_annotation_layer, finalize_annotation_draft,
    validate_annotation_layer, VoxelAnnotationError,
};
pub use edit::{
    VoxelAnnotationEditCommand, VoxelAnnotationEditError, VoxelAnnotationEditReceipt,
    VoxelAnnotationEditService, VoxelAnnotationEditTransaction,
};
pub use model::{
    VoxelAnnotationBounds, VoxelAnnotationContentHashes, VoxelAnnotationDiagnostic,
    VoxelAnnotationDiagnosticCode, VoxelAnnotationKind, VoxelAnnotationLayer,
    VoxelAnnotationLayerDraft, VoxelAnnotationProvenanceKind, VoxelAnnotationProvenanceRef,
    VoxelAnnotationRegion, VoxelAnnotationSelection, VoxelAnnotationSparseRun,
    VOXEL_ANNOTATION_SCHEMA_VERSION,
};
pub use query::{
    query_annotation_layer, VoxelAnnotationQuery, VoxelAnnotationQueryError,
    VoxelAnnotationQueryMode, VoxelAnnotationQueryReadout, VoxelAnnotationRegionReadout,
};
