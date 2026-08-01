use std::collections::{BTreeMap, BTreeSet};

use authored_scene::{
    composed_world_transforms, encode_scene, validate_scene, FlatSceneDocument, NodeMetadata,
    SceneMarker, SceneNodeKind, SceneNodeRecord, SceneTransform, CURRENT_SCENE_SCHEMA_VERSION,
};
use core_assets::{AssetId, AssetKind, AssetReference, AssetVersionReq};
use core_ids::SceneNodeId;
use core_math::Vec3;
use voxel_asset::{
    encode_voxel_asset, with_computed_content_hash, VoxelAsset, VoxelAssetBounds, VoxelAssetGrid,
    VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelAssetProvenance,
    VoxelAssetProvenanceKind, VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind,
    VoxelSparseRun, VOXEL_ASSET_SCHEMA_VERSION,
};

use crate::generation::{settings_bytes, sha256};
use crate::{
    generate_tunnel, GeneratedSpawnMarker, GeneratedTunnel, TunnelGenerationError,
    TunnelGeneratorConfig, MAX_GENERATED_TUNNEL_VOXELS,
};

pub const MAX_GENERATED_SPARSE_RUNS: usize = 65_536;
pub const MAX_GENERATED_MARKERS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironmentLimits {
    pub max_voxels: usize,
    pub max_sparse_runs: usize,
    pub max_markers: usize,
}

impl Default for EnvironmentLimits {
    fn default() -> Self {
        Self {
            max_voxels: MAX_GENERATED_TUNNEL_VOXELS,
            max_sparse_runs: MAX_GENERATED_SPARSE_RUNS,
            max_markers: MAX_GENERATED_MARKERS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentMarkerTarget {
    pub source_marker_id: String,
    pub node_id: SceneNodeId,
    pub marker_id: String,
    pub child_order: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentTarget {
    pub voxel_asset_id: String,
    pub voxel_node_id: SceneNodeId,
    pub voxel_parent_id: Option<SceneNodeId>,
    pub voxel_child_order: u32,
    pub voxel_label: Option<String>,
    pub voxel_transform: SceneTransform,
    pub marker_targets: Vec<EnvironmentMarkerTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentMaterializationRequest {
    pub expected_scene_revision: u64,
    pub config: TunnelGeneratorConfig,
    pub target: EnvironmentTarget,
    pub material_palette: Vec<VoxelAssetMaterialBinding>,
    pub limits: EnvironmentLimits,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentMarkerReadout {
    pub source_marker_id: &'static str,
    pub marker_id: String,
    pub node_id: SceneNodeId,
    pub local_transform: SceneTransform,
    pub world_transform: SceneTransform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedEnvironment {
    pub generation: GeneratedTunnel,
    pub asset: VoxelAsset,
    pub asset_json: String,
    pub scene: FlatSceneDocument,
    pub scene_json: String,
    pub voxel_world_transform: SceneTransform,
    pub markers: Vec<EnvironmentMarkerReadout>,
    pub revision_before: u64,
    pub revision_after: u64,
}

#[derive(Debug)]
pub enum EnvironmentMaterializationError {
    StaleSceneRevision {
        expected: u64,
        actual: u64,
    },
    SceneRevisionOverflow,
    InvalidSceneBefore {
        diagnostics: Vec<String>,
    },
    InvalidSceneAfter {
        diagnostics: Vec<String>,
    },
    ConflictingAssetDependency {
        asset_id: String,
    },
    InvalidTarget {
        path: &'static str,
        message: String,
    },
    RecipeMismatch,
    Generation(TunnelGenerationError),
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    InvalidVoxelAsset(String),
    SceneEncoding(String),
}

impl EnvironmentMaterializationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::StaleSceneRevision { .. } => "stale-scene-revision",
            Self::SceneRevisionOverflow => "scene-revision-overflow",
            Self::InvalidSceneBefore { .. } => "invalid-scene-before-materialization",
            Self::InvalidSceneAfter { .. } => "invalid-scene-after-materialization",
            Self::ConflictingAssetDependency { .. } => "conflicting-asset-dependency",
            Self::InvalidTarget { .. } => "invalid-environment-target",
            Self::RecipeMismatch => "environment-recipe-mismatch",
            Self::Generation(_) => "environment-generation-rejected",
            Self::ResourceLimit { .. } => "environment-resource-limit",
            Self::InvalidVoxelAsset(_) => "invalid-generated-voxel-asset",
            Self::SceneEncoding(_) => "generated-scene-encoding-failed",
        }
    }
}

impl std::fmt::Display for EnvironmentMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "environment materialization rejected: {self:?}")
    }
}

impl std::error::Error for EnvironmentMaterializationError {}

/// Build immutable asset and scene candidates. The input document is never
/// mutated, so every rejection is fail-atomic by construction.
pub fn materialize_environment(
    current_scene: &FlatSceneDocument,
    request: &EnvironmentMaterializationRequest,
) -> Result<MaterializedEnvironment, EnvironmentMaterializationError> {
    if current_scene.revision != request.expected_scene_revision {
        return Err(EnvironmentMaterializationError::StaleSceneRevision {
            expected: request.expected_scene_revision,
            actual: current_scene.revision,
        });
    }
    let before = validate_scene(current_scene);
    if !before.is_valid() {
        return Err(EnvironmentMaterializationError::InvalidSceneBefore {
            diagnostics: before
                .errors
                .iter()
                .map(|error| format!("{}: {error:?}", error.code()))
                .collect(),
        });
    }
    validate_target(current_scene, request)?;
    let generation =
        generate_tunnel(request.config).map_err(EnvironmentMaterializationError::Generation)?;
    enforce_limits(&generation, request.limits)?;
    let asset = build_voxel_asset(&generation, request)?;
    let asset_json = encode_voxel_asset(&asset)
        .map_err(|error| EnvironmentMaterializationError::InvalidVoxelAsset(error.to_string()))?;
    let scene = build_scene(current_scene, request, &generation, &asset)?;
    let scene_json = encode_scene(&scene)
        .map_err(|error| EnvironmentMaterializationError::SceneEncoding(error.to_string()))?;
    let world = composed_world_transforms(&scene);
    let voxel_world_transform = world[&request.target.voxel_node_id];
    let targets = request
        .target
        .marker_targets
        .iter()
        .map(|target| (target.source_marker_id.as_str(), target))
        .collect::<BTreeMap<_, _>>();
    let markers = generation
        .spawn_markers
        .iter()
        .map(|marker| {
            let target = targets[marker.source_id];
            let record = scene
                .nodes
                .iter()
                .find(|node| node.id == target.node_id)
                .expect("materialization installed every marker target");
            EnvironmentMarkerReadout {
                source_marker_id: marker.source_id,
                marker_id: target.marker_id.clone(),
                node_id: target.node_id,
                local_transform: record.transform,
                world_transform: world[&target.node_id],
            }
        })
        .collect();
    Ok(MaterializedEnvironment {
        generation,
        asset,
        asset_json,
        scene_json,
        voxel_world_transform,
        markers,
        revision_before: current_scene.revision,
        revision_after: scene.revision,
        scene,
    })
}

fn validate_target(
    scene: &FlatSceneDocument,
    request: &EnvironmentMaterializationRequest,
) -> Result<(), EnvironmentMaterializationError> {
    match AssetId::parse(&request.target.voxel_asset_id) {
        Ok(id) if id.kind() == AssetKind::VoxelVolume => {}
        _ => {
            return Err(invalid_target(
                "voxelAssetId",
                "expected a voxel-volume asset id",
            ))
        }
    }
    if request
        .target
        .voxel_parent_id
        .is_some_and(|parent| !scene.nodes.iter().any(|node| node.id == parent))
    {
        return Err(invalid_target(
            "voxelParentId",
            "parent node does not exist",
        ));
    }
    let target_ids = request
        .target
        .marker_targets
        .iter()
        .map(|target| target.node_id)
        .chain(std::iter::once(request.target.voxel_node_id))
        .collect::<Vec<_>>();
    if target_ids.iter().copied().collect::<BTreeSet<_>>().len() != target_ids.len() {
        return Err(invalid_target(
            "markerTargets",
            "target node ids must be unique",
        ));
    }
    let sources = request
        .target
        .marker_targets
        .iter()
        .map(|target| target.source_marker_id.as_str())
        .collect::<BTreeSet<_>>();
    let marker_ids = request
        .target
        .marker_targets
        .iter()
        .map(|target| target.marker_id.as_str())
        .collect::<BTreeSet<_>>();
    if sources != BTreeSet::from(["exit_hint", "player_start"])
        || marker_ids.len() != request.target.marker_targets.len()
    {
        return Err(invalid_target(
            "markerTargets",
            "tiny-enclosed requires unique player_start and exit_hint targets",
        ));
    }
    for node in &scene.nodes {
        if !target_ids.contains(&node.id) {
            continue;
        }
        let expected_voxel = node.id == request.target.voxel_node_id;
        if (expected_voxel && !matches!(node.kind, SceneNodeKind::VoxelVolume(_)))
            || (!expected_voxel && !matches!(node.kind, SceneNodeKind::Marker(_)))
        {
            return Err(invalid_target(
                "targetNodes",
                "target node has an incompatible kind",
            ));
        }
    }
    for node in &scene.nodes {
        let SceneNodeKind::Bootstrap(bindings) = &node.kind else {
            continue;
        };
        if bindings.generator.as_ref().is_some_and(|generator| {
            generator.provider_id != crate::TUNNEL_GENERATOR_ID
                || generator.preset_id != request.config.preset.label()
                || generator.seed != request.config.seed
        }) {
            return Err(EnvironmentMaterializationError::RecipeMismatch);
        }
    }
    Ok(())
}

fn enforce_limits(
    generation: &GeneratedTunnel,
    limits: EnvironmentLimits,
) -> Result<(), EnvironmentMaterializationError> {
    for (resource, actual, requested, ceiling) in [
        (
            "voxels",
            generation.voxels.len(),
            limits.max_voxels,
            MAX_GENERATED_TUNNEL_VOXELS,
        ),
        (
            "markers",
            generation.spawn_markers.len(),
            limits.max_markers,
            MAX_GENERATED_MARKERS,
        ),
    ] {
        let limit = requested.min(ceiling);
        if limit == 0 || actual > limit {
            return Err(EnvironmentMaterializationError::ResourceLimit {
                resource,
                actual,
                limit,
            });
        }
    }
    Ok(())
}

fn build_voxel_asset(
    generation: &GeneratedTunnel,
    request: &EnvironmentMaterializationRequest,
) -> Result<VoxelAsset, EnvironmentMaterializationError> {
    let sparse_runs = sparse_runs(&generation.voxels);
    let run_limit = request
        .limits
        .max_sparse_runs
        .min(MAX_GENERATED_SPARSE_RUNS);
    if run_limit == 0 || sparse_runs.len() > run_limit {
        return Err(EnvironmentMaterializationError::ResourceLimit {
            resource: "sparse-runs",
            actual: sparse_runs.len(),
            limit: run_limit,
        });
    }
    let bounds = voxel_bounds(&generation.voxels)
        .ok_or_else(|| invalid_target("generation", "generator produced no solid voxels"))?;
    let settings = settings_bytes(generation.config);
    let source_identity = format!(
        "{}|{}",
        generation.provenance.generator_id, generation.provenance.generator_version
    );
    let asset = VoxelAsset {
        schema_version: VOXEL_ASSET_SCHEMA_VERSION,
        asset_id: request.target.voxel_asset_id.clone(),
        grid: VoxelAssetGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: generation.config.voxel_size,
            chunk_size: generation.config.chunk_size,
            origin: [0, 0, 0],
        },
        bounds,
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs,
        },
        material_palette: request.material_palette.clone(),
        material_map: [
            (generation.config.wall_material, "wall"),
            (generation.config.floor_material, "floor"),
            (generation.config.accent_material, "accent"),
        ]
        .into_iter()
        .map(|(slot, name)| VoxelAssetMaterialMapping {
            source_material_slot: u32::from(slot),
            source_material_name: Some(name.to_string()),
            voxel_material_slot: slot,
        })
        .collect(),
        provenance: VoxelAssetProvenance {
            kind: VoxelAssetProvenanceKind::GeneratedEnvironment,
            source_path: format!(
                "generator/{}/{}",
                generation.provenance.generator_id, generation.provenance.preset
            ),
            source_sha256: sha256(source_identity.as_bytes()),
            source_byte_count: settings.len() as u64,
            converter: generation.provenance.generator_id.to_string(),
            settings_sha256: generation.provenance.settings_sha256.clone(),
            license_path: None,
        },
        voxel_data_hash: String::new(),
        content_hash: String::new(),
    };
    with_computed_content_hash(asset)
        .map_err(|error| EnvironmentMaterializationError::InvalidVoxelAsset(error.to_string()))
}

fn build_scene(
    current: &FlatSceneDocument,
    request: &EnvironmentMaterializationRequest,
    generation: &GeneratedTunnel,
    asset: &VoxelAsset,
) -> Result<FlatSceneDocument, EnvironmentMaterializationError> {
    let target_ids = request
        .target
        .marker_targets
        .iter()
        .map(|target| target.node_id)
        .chain(std::iter::once(request.target.voxel_node_id))
        .collect::<BTreeSet<_>>();
    let asset_id = AssetId::parse(&asset.asset_id)
        .map_err(|error| invalid_target("voxelAssetId", error.to_string()))?;
    let asset_reference = AssetReference::new(asset_id, AssetVersionReq::Any, None);
    let mut scene = current.clone();
    scene.nodes.retain(|node| !target_ids.contains(&node.id));
    for node in &mut scene.nodes {
        if let SceneNodeKind::Bootstrap(bindings) = &mut node.kind {
            if bindings.generator.as_ref().is_some_and(|generator| {
                generator.provider_id == crate::TUNNEL_GENERATOR_ID
                    && generator.preset_id == generation.config.preset.label()
                    && generator.seed == generation.config.seed
            }) {
                bindings.generator = None;
            }
        }
    }
    scene.nodes.push(SceneNodeRecord {
        id: request.target.voxel_node_id,
        parent: request.target.voxel_parent_id,
        child_order: request.target.voxel_child_order,
        transform: request.target.voxel_transform,
        renderable_transform: SceneTransform::IDENTITY,
        kind: SceneNodeKind::VoxelVolume(asset_reference),
        metadata: NodeMetadata {
            label: request.target.voxel_label.clone(),
            tags: vec!["generated-environment".to_string()],
        },
    });
    let targets = request
        .target
        .marker_targets
        .iter()
        .map(|target| (target.source_marker_id.as_str(), target))
        .collect::<BTreeMap<_, _>>();
    for marker in &generation.spawn_markers {
        let target = targets[marker.source_id];
        scene
            .nodes
            .push(marker_node(request.target.voxel_node_id, target, marker));
    }
    reconcile_dependencies(&mut scene)?;
    scene.schema_version = CURRENT_SCENE_SCHEMA_VERSION;
    scene.metadata.authoring_format_version = CURRENT_SCENE_SCHEMA_VERSION;
    scene.revision = scene
        .revision
        .checked_add(1)
        .ok_or(EnvironmentMaterializationError::SceneRevisionOverflow)?;
    scene.canonicalize();
    let report = validate_scene(&scene);
    if !report.is_valid() {
        return Err(EnvironmentMaterializationError::InvalidSceneAfter {
            diagnostics: report
                .errors
                .iter()
                .map(|error| format!("{}: {error:?}", error.code()))
                .collect(),
        });
    }
    Ok(scene)
}

fn reconcile_dependencies(
    scene: &mut FlatSceneDocument,
) -> Result<(), EnvironmentMaterializationError> {
    let mut references = BTreeMap::<String, AssetReference>::new();
    for reference in scene.nodes.iter().filter_map(|node| node.kind.asset()) {
        let asset_id = reference.id().as_str().to_string();
        if references
            .get(&asset_id)
            .is_some_and(|existing| existing != reference)
        {
            return Err(EnvironmentMaterializationError::ConflictingAssetDependency { asset_id });
        }
        references.insert(asset_id, reference.clone());
    }
    scene.dependencies = references.into_values().collect();
    Ok(())
}

fn marker_node(
    parent: SceneNodeId,
    target: &EnvironmentMarkerTarget,
    marker: &GeneratedSpawnMarker,
) -> SceneNodeRecord {
    let half_yaw = (marker.yaw_degrees as f32).to_radians() * 0.5;
    SceneNodeRecord {
        id: target.node_id,
        parent: Some(parent),
        child_order: target.child_order,
        transform: SceneTransform {
            translation: marker.local_position,
            rotation: authored_scene::Quat::new(0.0, half_yaw.sin(), 0.0, half_yaw.cos()),
            scale: Vec3::ONE,
        },
        renderable_transform: SceneTransform::IDENTITY,
        kind: SceneNodeKind::Marker(SceneMarker {
            marker_id: target.marker_id.clone(),
        }),
        metadata: NodeMetadata {
            label: Some(marker.kind.to_string()),
            tags: vec!["generated-marker".to_string()],
        },
    }
}

fn sparse_runs(voxels: &[crate::GeneratedVoxel]) -> Vec<VoxelSparseRun> {
    let mut cells = voxels.to_vec();
    cells.sort_by_key(|voxel| {
        (
            voxel.address[2],
            voxel.address[1],
            voxel.address[0],
            voxel.material_slot,
        )
    });
    let mut runs: Vec<VoxelSparseRun> = Vec::new();
    for voxel in cells {
        if let Some(last) = runs.last_mut() {
            if last.start[1] == voxel.address[1]
                && last.start[2] == voxel.address[2]
                && last.material_slot == voxel.material_slot
                && last.start[0] + i64::from(last.length) == voxel.address[0]
            {
                last.length += 1;
                continue;
            }
        }
        runs.push(VoxelSparseRun {
            start: voxel.address,
            length: 1,
            material_slot: voxel.material_slot,
        });
    }
    runs
}

fn voxel_bounds(voxels: &[crate::GeneratedVoxel]) -> Option<VoxelAssetBounds> {
    let first = voxels.first()?.address;
    let mut min = first;
    let mut max = first;
    for voxel in &voxels[1..] {
        for axis in 0..3 {
            min[axis] = min[axis].min(voxel.address[axis]);
            max[axis] = max[axis].max(voxel.address[axis]);
        }
    }
    Some(VoxelAssetBounds { min, max })
}

fn invalid_target(
    path: &'static str,
    message: impl Into<String>,
) -> EnvironmentMaterializationError {
    EnvironmentMaterializationError::InvalidTarget {
        path,
        message: message.into(),
    }
}
