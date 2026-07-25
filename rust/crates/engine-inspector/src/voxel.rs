use std::collections::BTreeMap;
use std::fmt::Write;

use engine_spatial::{MaterialVoxel, VoxelCollisionScene, VoxelEditRejection};
use serde::Serialize;
use voxel_asset::{decode_voxel_asset, validate_voxel_asset, VoxelAsset, VoxelAssetProvenanceKind};

use crate::{
    Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet, DiagnosticSeverity,
    RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelMaterialCount {
    pub material_slot: u16,
    pub voxel_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelAssetInspection {
    pub asset_id: String,
    pub schema_version: u32,
    pub cell_size: f64,
    pub chunk_size: u32,
    pub origin: [i64; 3],
    pub bounds_min: [i64; 3],
    pub bounds_max: [i64; 3],
    pub represented_voxel_count: usize,
    pub sparse_run_count: usize,
    pub material_counts: Vec<VoxelMaterialCount>,
    pub voxel_data_hash: String,
    pub content_hash: String,
    pub provenance_kind: String,
    pub provenance_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<VoxelStateInspection>,
    pub diagnostics: DiagnosticSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelChunkInspection {
    pub chunk: [i64; 3],
    pub content_hash: String,
    pub material_voxel_count: usize,
    pub has_collider: bool,
    pub vertices: u32,
    pub indices: usize,
    pub quads: u32,
    pub faces_culled: u32,
    pub material_group_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelStateInspection {
    pub voxel_size: f64,
    pub chunk_size: u32,
    pub source_revision: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub mesh_revision: u64,
    pub projections_coherent: bool,
    pub authority_hash: String,
    pub solid_voxel_count: usize,
    pub resident_chunk_count: usize,
    pub collider_chunk_count: usize,
    pub mesh_chunk_count: usize,
    pub navigation_cell_count: usize,
    pub navigation_hash: String,
    pub chunks: Vec<VoxelChunkInspection>,
    pub diagnostics: DiagnosticSet,
}

pub fn inspect_voxel_asset(asset: &VoxelAsset) -> VoxelAssetInspection {
    let mut counts = BTreeMap::<u16, usize>::new();
    let represented_voxel_count =
        asset
            .representation
            .sparse_runs
            .iter()
            .fold(0_usize, |count, run| {
                *counts.entry(run.material_slot).or_insert(0) += run.length as usize;
                count.saturating_add(run.length as usize)
            });
    let material_counts = counts
        .into_iter()
        .map(|(material_slot, voxel_count)| VoxelMaterialCount {
            material_slot,
            voxel_count,
        })
        .collect();

    let mut diagnostics = DiagnosticSet::new();
    let state = match validate_voxel_asset(asset) {
        Ok(()) => match collision_scene_from_asset(asset) {
            Ok(scene) => {
                let state = inspect_voxel_state(&scene);
                diagnostics.extend(state.diagnostics.diagnostics.clone());
                Some(state)
            }
            Err(message) => {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticDomain::VoxelState,
                        DiagnosticSeverity::Fatal,
                        "voxelState.projectionBuild",
                        DiagnosticLocation::path("representation.sparseRuns")
                            .with_asset(&asset.asset_id),
                        message,
                    )
                    .with_remedy(
                        RemedyAction::Inspect,
                        "correct the asset before building live voxel state",
                    ),
                );
                None
            }
        },
        Err(error) => {
            diagnostics.extend(error.diagnostics().iter().map(|source| {
                Diagnostic::new(
                    DiagnosticDomain::VoxelState,
                    DiagnosticSeverity::Error,
                    source.code,
                    DiagnosticLocation::path(&source.path).with_asset(&asset.asset_id),
                    &source.message,
                )
                .with_remedy(
                    RemedyAction::Inspect,
                    "correct the voxel asset authoring data",
                )
            }));
            None
        }
    };

    VoxelAssetInspection {
        asset_id: asset.asset_id.clone(),
        schema_version: asset.schema_version,
        cell_size: asset.grid.cell_size,
        chunk_size: asset.grid.chunk_size,
        origin: asset.grid.origin,
        bounds_min: asset.bounds.min,
        bounds_max: asset.bounds.max,
        represented_voxel_count,
        sparse_run_count: asset.representation.sparse_runs.len(),
        material_counts,
        voxel_data_hash: asset.voxel_data_hash.clone(),
        content_hash: asset.content_hash.clone(),
        provenance_kind: provenance_kind_label(asset.provenance.kind).to_string(),
        provenance_source: asset.provenance.source_path.clone(),
        state,
        diagnostics,
    }
}

pub fn inspect_voxel_asset_json(input: &str) -> Result<VoxelAssetInspection, DiagnosticSet> {
    let asset = decode_voxel_asset(input).map_err(|error| {
        let mut diagnostics = DiagnosticSet::new();
        diagnostics.extend(error.diagnostics().iter().map(|source| {
            Diagnostic::new(
                DiagnosticDomain::VoxelState,
                DiagnosticSeverity::Fatal,
                source.code,
                DiagnosticLocation::path(&source.path),
                &source.message,
            )
            .with_remedy(RemedyAction::RestoreArtifact, "fix the stored voxel asset")
        }));
        diagnostics
    })?;
    Ok(inspect_voxel_asset(&asset))
}

pub fn inspect_voxel_state(scene: &VoxelCollisionScene) -> VoxelStateInspection {
    let revisions = scene.projection_revisions();
    let projections_coherent = revisions.is_coherent_with(scene.source_revision());
    let mut diagnostics = DiagnosticSet::new();
    if !projections_coherent {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticDomain::VoxelState,
                DiagnosticSeverity::Error,
                "voxelState.projectionRevisionMismatch",
                DiagnosticLocation::default(),
                "collision, navigation, or mesh projection is stale",
            )
            .with_remedy(
                RemedyAction::Regenerate,
                "rebuild projections from voxel authority",
            ),
        );
    }

    let mut voxel_counts = BTreeMap::<[i64; 3], usize>::new();
    for voxel in scene.material_voxels() {
        let divisor = i64::from(scene.chunk_size());
        let chunk = [
            voxel.address[0].div_euclid(divisor),
            voxel.address[1].div_euclid(divisor),
            voxel.address[2].div_euclid(divisor),
        ];
        *voxel_counts.entry(chunk).or_insert(0) += 1;
    }
    let chunks = scene
        .mesh_chunks()
        .iter()
        .map(|chunk| VoxelChunkInspection {
            chunk: chunk.chunk,
            content_hash: format!("{:016x}", chunk.content_hash),
            material_voxel_count: voxel_counts.get(&chunk.chunk).copied().unwrap_or_default(),
            has_collider: scene.has_collider_chunk(chunk.chunk),
            vertices: chunk.vertices,
            indices: chunk.indices.len(),
            quads: chunk.quads,
            faces_culled: chunk.faces_culled,
            material_group_count: chunk.groups.len(),
        })
        .collect();

    VoxelStateInspection {
        voxel_size: scene.voxel_size(),
        chunk_size: scene.chunk_size(),
        source_revision: scene.source_revision().raw(),
        collision_revision: revisions.collision().raw(),
        navigation_revision: revisions.navigation().raw(),
        mesh_revision: revisions.mesh().raw(),
        projections_coherent,
        authority_hash: format!("{:016x}", scene.authority_hash()),
        solid_voxel_count: scene.solid_voxel_count(),
        resident_chunk_count: scene.resident_chunk_count(),
        collider_chunk_count: scene.collider_chunk_count(),
        mesh_chunk_count: scene.mesh_chunks().len(),
        navigation_cell_count: scene.navigation_cell_count(),
        navigation_hash: format!("{:016x}", scene.navigation_hash()),
        chunks,
        diagnostics,
    }
}

pub fn describe_voxel_edit_rejection(rejection: &VoxelEditRejection) -> String {
    match rejection {
        VoxelEditRejection::StaleRevision { expected, actual } => format!(
            "edit rejected: expected voxel revision {}, actual {}",
            expected.raw(),
            actual.raw()
        ),
        VoxelEditRejection::RevisionExhausted => {
            "edit rejected: voxel revision counter is exhausted".to_string()
        }
        VoxelEditRejection::EmptyTransaction => {
            "edit rejected: transaction contains no edits".to_string()
        }
        VoxelEditRejection::TooManyEdits { limit, actual } => {
            format!("edit rejected: {actual} edits exceed limit {limit}")
        }
        VoxelEditRejection::CoordinateOutOfBounds {
            edit_index,
            address,
            axis,
            limit,
        } => format!(
            "edit rejected: edit {edit_index} voxel [{},{},{}] exceeds axis {axis} limit +/-{limit}",
            address[0], address[1], address[2]
        ),
        VoxelEditRejection::InvalidMaterialSlot {
            edit_index,
            material_slot,
            maximum,
        } => format!(
            "edit rejected: edit {edit_index} material slot {material_slot} exceeds maximum {maximum}"
        ),
        VoxelEditRejection::DuplicateAddress {
            first_index,
            duplicate_index,
            address,
        } => format!(
            "edit rejected: edits {first_index} and {duplicate_index} repeat voxel [{},{},{}]",
            address[0], address[1], address[2]
        ),
        VoxelEditRejection::NoChanges => {
            "edit rejected: transaction would not change voxel authority".to_string()
        }
    }
}

impl VoxelAssetInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "voxel-asset id={} schema={} cellSize={} chunkSize={} origin={:?} bounds={:?}..{:?}\n",
            self.asset_id,
            self.schema_version,
            self.cell_size,
            self.chunk_size,
            self.origin,
            self.bounds_min,
            self.bounds_max
        );
        let _ = writeln!(
            output,
            "occupancy voxels={} runs={} voxelDataHash={} contentHash={}",
            self.represented_voxel_count,
            self.sparse_run_count,
            self.voxel_data_hash,
            self.content_hash
        );
        let materials = self
            .material_counts
            .iter()
            .map(|item| format!("{}={}", item.material_slot, item.voxel_count))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(output, "materials {materials}");
        let _ = writeln!(
            output,
            "provenance kind={} source={:?}",
            self.provenance_kind, self.provenance_source
        );
        if let Some(state) = &self.state {
            output.push_str(&state.summary_text());
        }
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

impl VoxelStateInspection {
    pub fn to_text(&self) -> String {
        let mut output = self.summary_text();
        output.push_str(&self.diagnostics.to_text());
        output
    }

    fn summary_text(&self) -> String {
        let mut output = format!(
            "voxel-state revision={} coherent={} authorityHash={} solids={} resident={} colliders={} meshChunks={} navigationCells={}\n",
            self.source_revision,
            self.projections_coherent,
            self.authority_hash,
            self.solid_voxel_count,
            self.resident_chunk_count,
            self.collider_chunk_count,
            self.mesh_chunk_count,
            self.navigation_cell_count
        );
        let _ = writeln!(
            output,
            "projection-revisions collision={} navigation={} mesh={} navigationHash={}",
            self.collision_revision,
            self.navigation_revision,
            self.mesh_revision,
            self.navigation_hash
        );
        for chunk in &self.chunks {
            let _ = writeln!(
                output,
                "chunk [{},{},{}] hash={} voxels={} collider={} vertices={} indices={} quads={} culled={} groups={}",
                chunk.chunk[0],
                chunk.chunk[1],
                chunk.chunk[2],
                chunk.content_hash,
                chunk.material_voxel_count,
                chunk.has_collider,
                chunk.vertices,
                chunk.indices,
                chunk.quads,
                chunk.faces_culled,
                chunk.material_group_count
            );
        }
        output
    }
}

fn collision_scene_from_asset(asset: &VoxelAsset) -> Result<VoxelCollisionScene, String> {
    let mut voxels = Vec::new();
    for run in &asset.representation.sparse_runs {
        for offset in 0..run.length {
            let local_x = run.start[0]
                .checked_add(i64::from(offset))
                .ok_or_else(|| "sparse run coordinate overflowed".to_string())?;
            let address = [
                asset.grid.origin[0]
                    .checked_add(local_x)
                    .ok_or_else(|| "mapped x address overflowed".to_string())?,
                asset.grid.origin[1]
                    .checked_add(run.start[1])
                    .ok_or_else(|| "mapped y address overflowed".to_string())?,
                asset.grid.origin[2]
                    .checked_add(run.start[2])
                    .ok_or_else(|| "mapped z address overflowed".to_string())?,
            ];
            voxels.push(MaterialVoxel {
                address,
                material_slot: run.material_slot,
            });
        }
    }
    VoxelCollisionScene::from_material_voxels(asset.grid.cell_size, asset.grid.chunk_size, voxels)
        .map_err(|error| error.to_string())
}

fn provenance_kind_label(kind: VoxelAssetProvenanceKind) -> &'static str {
    match kind {
        VoxelAssetProvenanceKind::Authored => "authored",
        VoxelAssetProvenanceKind::ConvertedStaticMesh => "convertedStaticMesh",
        VoxelAssetProvenanceKind::GeneratedEnvironment => "generatedEnvironment",
    }
}

#[cfg(test)]
mod tests {
    use voxel_asset::{
        with_computed_content_hash, VoxelAssetBounds, VoxelAssetGrid, VoxelAssetMaterialBinding,
        VoxelAssetMaterialMapping, VoxelAssetProvenance, VoxelAssetProvenanceKind,
        VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
        VOXEL_ASSET_SCHEMA_VERSION,
    };

    use super::*;

    fn asset() -> VoxelAsset {
        with_computed_content_hash(VoxelAsset {
            schema_version: VOXEL_ASSET_SCHEMA_VERSION,
            asset_id: "voxel-volume/two-cells".to_string(),
            grid: VoxelAssetGrid {
                coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
                cell_size: 1.0,
                chunk_size: 4,
                origin: [0, 0, 0],
            },
            bounds: VoxelAssetBounds {
                min: [0, 0, 0],
                max: [1, 0, 0],
            },
            representation: VoxelRepresentation {
                kind: VoxelRepresentationKind::SparseRuns,
                sparse_runs: vec![VoxelSparseRun {
                    start: [0, 0, 0],
                    length: 2,
                    material_slot: 1,
                }],
            },
            material_palette: vec![VoxelAssetMaterialBinding {
                material_slot: 1,
                material_asset_id: "material/stone".to_string(),
                display_name: None,
            }],
            material_map: vec![VoxelAssetMaterialMapping {
                source_material_slot: 0,
                source_material_name: Some("stone".to_string()),
                voxel_material_slot: 1,
            }],
            provenance: VoxelAssetProvenance {
                kind: VoxelAssetProvenanceKind::ConvertedStaticMesh,
                source_path: "source/two-cells.glb".to_string(),
                source_sha256: format!("sha256:{}", "1".repeat(64)),
                source_byte_count: 1,
                converter: "engine-inspector-test".to_string(),
                settings_sha256: format!("sha256:{}", "2".repeat(64)),
                license_path: None,
            },
            voxel_data_hash: String::new(),
            content_hash: String::new(),
        })
        .unwrap()
    }

    #[test]
    fn asset_report_rebuilds_real_projection_without_mutating_the_asset() {
        let asset = asset();
        let before = asset.content_hash.clone();
        let report = inspect_voxel_asset(&asset);
        assert_eq!(asset.content_hash, before);
        assert_eq!(report.represented_voxel_count, 2);
        let state = report.state.as_ref().unwrap();
        assert_eq!(state.solid_voxel_count, 2);
        assert_eq!(state.collider_chunk_count, 1);
        assert_eq!(state.chunks[0].quads, 10);
        assert!(state.projections_coherent);
        assert!(report.to_text().contains("chunk [0,0,0]"));
    }

    #[test]
    fn edit_rejection_names_the_exact_voxel_and_indices() {
        let text = describe_voxel_edit_rejection(&VoxelEditRejection::DuplicateAddress {
            first_index: 2,
            duplicate_index: 5,
            address: [-1, 3, 9],
        });
        assert!(text.contains("edits 2 and 5"));
        assert!(text.contains("voxel [-1,3,9]"));
    }
}
