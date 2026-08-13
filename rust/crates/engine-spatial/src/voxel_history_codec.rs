use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    validate_voxel_address, validate_voxel_material_slot,
    voxel_history::{material_voxels, VoxelEditHistoryParts},
    CollisionSceneError, MaterialVoxel, SurfaceMeshLimits, SurfaceMeshOptions, SurfaceMode,
    VoxelCollisionScene, VoxelEditHistory, VoxelEditHistoryEntry, VoxelEditHistoryError,
    VoxelEditHistoryLimits, VoxelSourceRevision, MAX_RESIDENT_VOXEL_CHUNKS,
    MAX_VOXEL_COORDINATE_ABS,
};

pub const VOXEL_EDIT_HISTORY_SCHEMA_VERSION: u32 = 3;
pub const MAX_VOXEL_EDIT_HISTORY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct VoxelEditHistoryDocument {
    schema_version: u32,
    voxel_size: f64,
    chunk_size: u32,
    base_voxels: Vec<MaterialVoxel>,
    base_resident_chunks: Vec<[i64; 3]>,
    mesh_options: SurfaceMeshOptionsDocument,
    base_hash: u64,
    entries: Vec<VoxelEditHistoryEntry>,
    cursor_index: usize,
    next_transaction_id: u64,
    source_revision: u64,
    content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SurfaceMeshOptionsDocument {
    mode: String,
    max_source_faces: u64,
    max_sampled_cells: u64,
    max_vertices: u32,
    max_indices: u32,
    max_temporary_field_bytes: u64,
    max_material_partitions: u32,
}

#[derive(Debug)]
pub struct VoxelEditHistoryRestore {
    pub history: VoxelEditHistory,
    pub scene: VoxelCollisionScene,
}

#[derive(Debug)]
pub enum VoxelEditHistoryCodecError {
    ResourceLimit {
        limit: usize,
        actual: usize,
    },
    Decode {
        path: String,
        message: String,
    },
    UnsupportedSchema(u32),
    InvalidContentHash,
    NonCanonicalBase,
    InvalidResidentChunk {
        chunk: [i64; 3],
    },
    InvalidMeshOptions,
    InvalidCursor {
        cursor: usize,
        entries: usize,
    },
    EntryQuotaExceeded {
        limit: usize,
        actual: usize,
    },
    DeltaQuotaExceeded {
        limit: usize,
        actual: usize,
    },
    InvalidTransactionChain {
        entry_index: usize,
    },
    InvalidDelta {
        entry_index: usize,
        delta_index: usize,
    },
    History(VoxelEditHistoryError),
    Scene(CollisionSceneError),
}

impl std::fmt::Display for VoxelEditHistoryCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelEditHistoryCodecError {}

pub fn encode_voxel_edit_history(
    history: &VoxelEditHistory,
) -> Result<String, VoxelEditHistoryCodecError> {
    validate_history(history)?;
    let mut document = document_for(history);
    document.content_hash = document_hash(&document);
    let mut encoded = serde_json::to_string_pretty(&document).map_err(|error| {
        VoxelEditHistoryCodecError::Decode {
            path: "$".to_string(),
            message: error.to_string(),
        }
    })?;
    encoded.push('\n');
    if encoded.len() > MAX_VOXEL_EDIT_HISTORY_BYTES {
        return Err(VoxelEditHistoryCodecError::ResourceLimit {
            limit: MAX_VOXEL_EDIT_HISTORY_BYTES,
            actual: encoded.len(),
        });
    }
    Ok(encoded)
}

pub fn decode_voxel_edit_history(
    input: &str,
    limits: VoxelEditHistoryLimits,
) -> Result<VoxelEditHistoryRestore, VoxelEditHistoryCodecError> {
    if input.len() > MAX_VOXEL_EDIT_HISTORY_BYTES {
        return Err(VoxelEditHistoryCodecError::ResourceLimit {
            limit: MAX_VOXEL_EDIT_HISTORY_BYTES,
            actual: input.len(),
        });
    }
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let document: VoxelEditHistoryDocument = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| VoxelEditHistoryCodecError::Decode {
            path: json_path(&error.path().to_string()),
            message: error.inner().to_string(),
        })?;
    deserializer
        .end()
        .map_err(|error| VoxelEditHistoryCodecError::Decode {
            path: "$".to_string(),
            message: error.to_string(),
        })?;
    if document.schema_version != VOXEL_EDIT_HISTORY_SCHEMA_VERSION {
        return Err(VoxelEditHistoryCodecError::UnsupportedSchema(
            document.schema_version,
        ));
    }
    if !valid_sha256(&document.content_hash) || document.content_hash != document_hash(&document) {
        return Err(VoxelEditHistoryCodecError::InvalidContentHash);
    }
    validate_document_shape(&document, limits)?;

    let history = VoxelEditHistory::from_parts(
        VoxelEditHistoryParts {
            base_voxel_size: document.voxel_size,
            base_chunk_size: document.chunk_size,
            base_voxels: document.base_voxels,
            base_resident_chunks: document.base_resident_chunks,
            base_mesh_options: decode_mesh_options(&document.mesh_options)?,
            base_hash: document.base_hash,
            entries: document.entries,
            cursor_index: document.cursor_index,
            next_transaction_id: document.next_transaction_id,
            source_revision: VoxelSourceRevision::new(document.source_revision),
        },
        limits,
    );
    validate_history(&history)?;
    let materials = history
        .materials_at_cursor(history.cursor_index)
        .map_err(VoxelEditHistoryCodecError::History)?;
    let scene = VoxelCollisionScene::from_material_voxels_at_revision_with_residents(
        history.base_voxel_size,
        history.base_chunk_size,
        material_voxels(&materials),
        history.base_resident_chunks.iter().copied(),
        crate::SceneBuildRevision::initial(history.source_revision),
        history.base_mesh_options,
        None,
    )
    .map_err(VoxelEditHistoryCodecError::Scene)?;
    Ok(VoxelEditHistoryRestore { history, scene })
}

fn validate_history(history: &VoxelEditHistory) -> Result<(), VoxelEditHistoryCodecError> {
    let document = document_for(history);
    validate_document_shape(&document, history.limits)?;
    let base_scene = VoxelCollisionScene::from_material_voxels_at_revision_with_residents(
        history.base_voxel_size,
        history.base_chunk_size,
        history.base_voxels.iter().copied(),
        history.base_resident_chunks.iter().copied(),
        crate::SceneBuildRevision::initial(VoxelSourceRevision::INITIAL),
        history.base_mesh_options,
        None,
    )
    .map_err(VoxelEditHistoryCodecError::Scene)?;
    if base_scene.authority_hash() != history.base_hash {
        return Err(VoxelEditHistoryCodecError::NonCanonicalBase);
    }
    history
        .materials_at_cursor(history.entries.len())
        .map_err(VoxelEditHistoryCodecError::History)?;
    Ok(())
}

fn validate_document_shape(
    document: &VoxelEditHistoryDocument,
    limits: VoxelEditHistoryLimits,
) -> Result<(), VoxelEditHistoryCodecError> {
    if document.entries.len() > limits.max_entries {
        return Err(VoxelEditHistoryCodecError::EntryQuotaExceeded {
            limit: limits.max_entries,
            actual: document.entries.len(),
        });
    }
    let deltas = document
        .entries
        .iter()
        .map(|entry| entry.deltas.len())
        .sum::<usize>();
    if deltas > limits.max_retained_deltas {
        return Err(VoxelEditHistoryCodecError::DeltaQuotaExceeded {
            limit: limits.max_retained_deltas,
            actual: deltas,
        });
    }
    if document.cursor_index > document.entries.len() {
        return Err(VoxelEditHistoryCodecError::InvalidCursor {
            cursor: document.cursor_index,
            entries: document.entries.len(),
        });
    }
    if document
        .base_voxels
        .windows(2)
        .any(|pair| pair[0].address >= pair[1].address)
    {
        return Err(VoxelEditHistoryCodecError::NonCanonicalBase);
    }
    if document.base_resident_chunks.len() > MAX_RESIDENT_VOXEL_CHUNKS
        || document
            .base_resident_chunks
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(VoxelEditHistoryCodecError::NonCanonicalBase);
    }
    for &chunk in &document.base_resident_chunks {
        validate_resident_chunk(chunk, document.chunk_size)?;
    }
    decode_mesh_options(&document.mesh_options)?;
    for (entry_index, entry) in document.entries.iter().enumerate() {
        let expected_parent = entry_index
            .checked_sub(1)
            .and_then(|index| document.entries.get(index))
            .map(|entry| entry.transaction_id);
        let transaction_is_ordered = entry_index == 0
            || document.entries[entry_index - 1].transaction_id < entry.transaction_id;
        if entry.parent_transaction_id != expected_parent
            || !transaction_is_ordered
            || entry.deltas.is_empty()
        {
            return Err(VoxelEditHistoryCodecError::InvalidTransactionChain { entry_index });
        }
        for (delta_index, delta) in entry.deltas.iter().enumerate() {
            let ordered = delta_index == 0 || entry.deltas[delta_index - 1].address < delta.address;
            let valid_authority = validate_voxel_address(delta.address).is_ok()
                && delta
                    .before_material
                    .is_none_or(|material| validate_voxel_material_slot(material).is_ok())
                && delta
                    .after_material
                    .is_none_or(|material| validate_voxel_material_slot(material).is_ok());
            if !ordered || delta.before_material == delta.after_material || !valid_authority {
                return Err(VoxelEditHistoryCodecError::InvalidDelta {
                    entry_index,
                    delta_index,
                });
            }
        }
    }
    if document.next_transaction_id
        <= document
            .entries
            .last()
            .map_or(0, |entry| entry.transaction_id)
    {
        return Err(VoxelEditHistoryCodecError::InvalidTransactionChain {
            entry_index: document.entries.len(),
        });
    }
    Ok(())
}

fn document_for(history: &VoxelEditHistory) -> VoxelEditHistoryDocument {
    VoxelEditHistoryDocument {
        schema_version: VOXEL_EDIT_HISTORY_SCHEMA_VERSION,
        voxel_size: history.base_voxel_size,
        chunk_size: history.base_chunk_size,
        base_voxels: history.base_voxels.clone(),
        base_resident_chunks: history.base_resident_chunks.clone(),
        mesh_options: encode_mesh_options(history.base_mesh_options),
        base_hash: history.base_hash,
        entries: history.entries.clone(),
        cursor_index: history.cursor_index,
        next_transaction_id: history.next_transaction_id,
        source_revision: history.source_revision.raw(),
        content_hash: String::new(),
    }
}

fn document_hash(document: &VoxelEditHistoryDocument) -> String {
    let mut canonical = VoxelEditHistoryDocument {
        schema_version: document.schema_version,
        voxel_size: document.voxel_size,
        chunk_size: document.chunk_size,
        base_voxels: document.base_voxels.clone(),
        base_resident_chunks: document.base_resident_chunks.clone(),
        mesh_options: document.mesh_options.clone(),
        base_hash: document.base_hash,
        entries: document.entries.clone(),
        cursor_index: document.cursor_index,
        next_transaction_id: document.next_transaction_id,
        source_revision: document.source_revision,
        content_hash: String::new(),
    };
    canonical
        .base_voxels
        .sort_unstable_by_key(|voxel| voxel.address);
    let bytes = serde_json::to_vec(&canonical).expect("history document serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn encode_mesh_options(options: SurfaceMeshOptions) -> SurfaceMeshOptionsDocument {
    SurfaceMeshOptionsDocument {
        mode: options.mode.as_str().to_string(),
        max_source_faces: options.limits.max_source_faces,
        max_sampled_cells: options.limits.max_sampled_cells,
        max_vertices: options.limits.max_vertices,
        max_indices: options.limits.max_indices,
        max_temporary_field_bytes: options.limits.max_temporary_field_bytes,
        max_material_partitions: options.limits.max_material_partitions,
    }
}

fn decode_mesh_options(
    options: &SurfaceMeshOptionsDocument,
) -> Result<SurfaceMeshOptions, VoxelEditHistoryCodecError> {
    let mode = match options.mode.as_str() {
        "greedyCubes" => SurfaceMode::GreedyCubes,
        "marchingCubes" => SurfaceMode::MarchingCubes,
        "dualContouring" => SurfaceMode::DualContouring,
        _ => return Err(VoxelEditHistoryCodecError::InvalidMeshOptions),
    };
    let limits = SurfaceMeshLimits {
        max_source_faces: options.max_source_faces,
        max_sampled_cells: options.max_sampled_cells,
        max_vertices: options.max_vertices,
        max_indices: options.max_indices,
        max_temporary_field_bytes: options.max_temporary_field_bytes,
        max_material_partitions: options.max_material_partitions,
    };
    if limits.max_source_faces == 0
        || limits.max_sampled_cells == 0
        || limits.max_vertices == 0
        || limits.max_indices == 0
        || limits.max_temporary_field_bytes == 0
        || limits.max_material_partitions == 0
    {
        return Err(VoxelEditHistoryCodecError::InvalidMeshOptions);
    }
    Ok(SurfaceMeshOptions { mode, limits })
}

fn validate_resident_chunk(
    chunk: [i64; 3],
    chunk_size: u32,
) -> Result<(), VoxelEditHistoryCodecError> {
    let extent = i64::from(chunk_size);
    for coordinate in chunk {
        let Some(minimum) = coordinate.checked_mul(extent) else {
            return Err(VoxelEditHistoryCodecError::InvalidResidentChunk { chunk });
        };
        let Some(maximum) = minimum.checked_add(extent.saturating_sub(1)) else {
            return Err(VoxelEditHistoryCodecError::InvalidResidentChunk { chunk });
        };
        if minimum < -MAX_VOXEL_COORDINATE_ABS || maximum > MAX_VOXEL_COORDINATE_ABS {
            return Err(VoxelEditHistoryCodecError::InvalidResidentChunk { chunk });
        }
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn json_path(path: &str) -> String {
    if path.is_empty() {
        "$".to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VoxelEdit;

    #[test]
    fn decode_rejects_invalid_material_in_unapplied_redo_tail() {
        let mut scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            8,
            [MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
            }],
        )
        .unwrap();
        let mut history = VoxelEditHistory::new(&scene);
        history
            .apply(
                &mut scene,
                &[VoxelEdit::Set {
                    address: [1, 0, 0],
                    material_slot: 2,
                }],
            )
            .unwrap();
        history.undo_one(&mut scene).unwrap();

        let mut document = document_for(&history);
        document.entries[0].deltas[0].after_material = Some(0);
        document.content_hash = document_hash(&document);
        let encoded = serde_json::to_string(&document).unwrap();
        assert!(matches!(
            decode_voxel_edit_history(&encoded, VoxelEditHistoryLimits::default()),
            Err(VoxelEditHistoryCodecError::InvalidDelta { .. })
        ));
    }
}
