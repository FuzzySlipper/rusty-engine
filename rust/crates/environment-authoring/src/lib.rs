//! Deterministic environment generation and explicit authored-content materialization.
//!
//! Generation produces one canonical cell set. Materialization turns that set
//! into a validated voxel asset and revisioned authored-scene candidate. Live
//! collision, navigation, meshing, and rendering consume those outputs through
//! their own named APIs; this crate owns no runtime session or provider router.

#![forbid(unsafe_code)]

mod generation;
mod materialization;

pub use generation::{
    generate_tunnel, GeneratedCollisionAabb, GeneratedSpawnMarker, GeneratedTunnel,
    GeneratedTunnelFrame, GeneratedTunnelProvenance, GeneratedVoxel, TunnelGenerationError,
    TunnelGeneratorConfig, TunnelPreset, MAX_GENERATED_TUNNEL_VOXELS, TUNNEL_GENERATOR_ID,
    TUNNEL_GENERATOR_VERSION,
};
pub use materialization::{
    materialize_environment, EnvironmentLimits, EnvironmentMarkerReadout, EnvironmentMarkerTarget,
    EnvironmentMaterializationError, EnvironmentMaterializationRequest, EnvironmentTarget,
    MaterializedEnvironment, MAX_GENERATED_MARKERS, MAX_GENERATED_SPARSE_RUNS,
};
