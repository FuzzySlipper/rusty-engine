use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};
use voxel_asset::{
    conversion_settings_sha256, encode_voxel_asset, validate_conversion_request,
    with_computed_content_hash, VoxelAsset, VoxelAssetBounds, VoxelAssetGrid, VoxelAssetProvenance,
    VoxelAssetProvenanceKind, VoxelConversionRequest, VoxelCoordinateSystem, VoxelRepresentation,
    VoxelRepresentationKind, VoxelSparseRun, VOXEL_ASSET_SCHEMA_VERSION,
};

use crate::{
    material::MaterialSamplingContext,
    voxelize::{voxelize, MaterialEvidence, MAX_GEOMETRIC_VOXELIZATION_WORK},
    ConversionError, ImportedStaticMesh,
};

pub const CONVERTER_ID: &str = "rusty-engine.mesh-to-voxel.v2";
pub const MAX_SURFACE_SAMPLE_WORK: u64 = MAX_GEOMETRIC_VOXELIZATION_WORK;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionReceipt {
    pub asset: VoxelAsset,
    pub canonical_json: String,
    pub source_sha256: String,
    pub settings_sha256: String,
    pub content_hash: String,
    pub source_vertices: usize,
    pub source_triangles: usize,
    pub voxelization_work: u64,
    pub output_voxels: usize,
    pub sparse_runs: usize,
    pub bounds: VoxelAssetBounds,
}

pub fn convert_glb(
    request: &VoxelConversionRequest,
    source: &[u8],
) -> Result<ConversionReceipt, ConversionError> {
    let source_sha256 = sha256(source);
    let mesh = crate::import_static_glb(source)?;
    convert_imported_mesh(request, &mesh, source_sha256, source.len() as u64)
}

pub(crate) fn convert_imported_mesh(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
    source_sha256: String,
    source_byte_count: u64,
) -> Result<ConversionReceipt, ConversionError> {
    convert_imported_mesh_with_material_sampling(
        request,
        mesh,
        source_sha256,
        source_byte_count,
        None,
    )
}

pub(crate) fn convert_imported_mesh_with_material_sampling(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
    source_sha256: String,
    source_byte_count: u64,
    material_sampling: Option<&MaterialSamplingContext<'_>>,
) -> Result<ConversionReceipt, ConversionError> {
    validate_conversion_request(request, source_byte_count)?;
    if source_sha256 != request.expected_source_sha256 {
        return Err(ConversionError::one(
            "conversion.sourceHashMismatch",
            "expectedSourceSha256",
            format!(
                "expected {}, computed {source_sha256}",
                request.expected_source_sha256
            ),
        ));
    }

    validate_material_map(request, mesh)?;
    let (cells, voxelization_work) = convert_cells(request, mesh, material_sampling)?;
    let bounds = bounds_for_cells(&cells).expect("conversion rejects empty output");
    let sparse_runs = sparse_runs(&cells);
    let settings_sha256 = conversion_settings_sha256(&request.settings);
    let asset = with_computed_content_hash(VoxelAsset {
        schema_version: VOXEL_ASSET_SCHEMA_VERSION,
        asset_id: request.asset_id.clone(),
        grid: VoxelAssetGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: request.settings.cell_size,
            chunk_size: request.settings.chunk_size,
            origin: request.settings.origin,
        },
        bounds,
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs,
        },
        material_palette: request.settings.material_palette.clone(),
        material_map: request.settings.material_map.clone(),
        provenance: VoxelAssetProvenance {
            kind: VoxelAssetProvenanceKind::ConvertedStaticMesh,
            source_path: request.source_path.clone(),
            source_sha256: source_sha256.clone(),
            source_byte_count,
            converter: CONVERTER_ID.to_string(),
            settings_sha256: settings_sha256.clone(),
            license_path: request.license_path.clone(),
        },
        voxel_data_hash: String::new(),
        content_hash: String::new(),
    })
    .map_err(asset_error)?;
    let canonical_json = encode_voxel_asset(&asset).map_err(asset_error)?;
    let content_hash = asset.content_hash.clone();
    let output_voxels = asset
        .representation
        .sparse_runs
        .iter()
        .map(|run| run.length as usize)
        .sum();

    Ok(ConversionReceipt {
        source_vertices: mesh.positions.len(),
        source_triangles: mesh.triangles.len(),
        voxelization_work,
        output_voxels,
        sparse_runs: asset.representation.sparse_runs.len(),
        bounds,
        asset,
        canonical_json,
        source_sha256,
        settings_sha256,
        content_hash,
    })
}

pub(crate) fn replace_settings_identity(
    mut receipt: ConversionReceipt,
    settings_sha256: String,
) -> Result<ConversionReceipt, ConversionError> {
    receipt.asset.provenance.settings_sha256 = settings_sha256.clone();
    receipt.asset = with_computed_content_hash(receipt.asset).map_err(asset_error)?;
    receipt.canonical_json = encode_voxel_asset(&receipt.asset).map_err(asset_error)?;
    receipt.settings_sha256 = settings_sha256;
    receipt.content_hash = receipt.asset.content_hash.clone();
    Ok(receipt)
}

fn validate_material_map(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
) -> Result<(), ConversionError> {
    let imported = mesh
        .materials
        .iter()
        .map(|material| {
            (
                material.source_material_slot,
                &material.source_material_name,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let requested = request
        .settings
        .material_map
        .iter()
        .enumerate()
        .map(|(index, mapping)| (mapping.source_material_slot, (index, mapping)))
        .collect::<BTreeMap<_, _>>();

    for (slot, name) in &imported {
        let Some((index, mapping)) = requested.get(slot) else {
            return Err(ConversionError::one(
                "conversion.materialMapMismatch",
                "settings.materialMap",
                format!("source material slot {slot} ({name:?}) has no mapping"),
            ));
        };
        if let Some(expected_name) = &mapping.source_material_name {
            if name.as_deref() != Some(expected_name.as_str()) {
                return Err(ConversionError::one(
                    "conversion.materialMapMismatch",
                    format!("settings.materialMap[{index}].sourceMaterialName"),
                    format!("expected source material name {expected_name:?}, imported {name:?}"),
                ));
            }
        }
    }
    let extras = requested
        .keys()
        .filter(|slot| !imported.contains_key(slot))
        .copied()
        .collect::<Vec<_>>();
    if !extras.is_empty() {
        return Err(ConversionError::one(
            "conversion.materialMapMismatch",
            "settings.materialMap",
            format!("material mappings reference absent source slots {extras:?}"),
        ));
    }
    Ok(())
}

fn convert_cells(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
    material_sampling: Option<&MaterialSamplingContext<'_>>,
) -> Result<(BTreeMap<[i64; 3], u16>, u64), ConversionError> {
    let material_map = request
        .settings
        .material_map
        .iter()
        .map(|mapping| (mapping.source_material_slot, mapping.voxel_material_slot))
        .collect::<BTreeMap<_, _>>();
    let voxelization = voxelize(request, mesh)?;
    let cells = voxelization
        .cells
        .into_iter()
        .map(|(coordinate, evidence)| {
            let fallback = resolve_static_material(evidence, &material_map)?;
            Ok((
                coordinate,
                match material_sampling {
                    Some(sampling) => sampling.resolve(mesh, evidence, fallback)?,
                    None => fallback,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ConversionError>>()?;
    Ok((cells, voxelization.work))
}

fn resolve_static_material(
    evidence: MaterialEvidence,
    material_map: &BTreeMap<u32, u16>,
) -> Result<u16, ConversionError> {
    material_map
        .get(&evidence.source_material_slot)
        .copied()
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.materialMapMismatch",
                "settings.materialMap",
                format!(
                    "surface evidence references unmapped source material slot {}",
                    evidence.source_material_slot
                ),
            )
        })
}

fn sparse_runs(cells: &BTreeMap<[i64; 3], u16>) -> Vec<VoxelSparseRun> {
    let mut rows = BTreeMap::<(i64, i64), Vec<(i64, u16)>>::new();
    for (coordinate, material) in cells {
        rows.entry((coordinate[1], coordinate[2]))
            .or_default()
            .push((coordinate[0], *material));
    }
    let mut runs = Vec::new();
    for ((y, z), row) in &mut rows {
        row.sort_unstable();
        for (x, material) in row.iter().copied() {
            let extend = runs.last_mut().is_some_and(|prior: &mut VoxelSparseRun| {
                prior.start[1] == *y
                    && prior.start[2] == *z
                    && prior.material_slot == material
                    && prior.start[0] + i64::from(prior.length) == x
            });
            if extend {
                runs.last_mut().expect("checked last run").length += 1;
            } else {
                runs.push(VoxelSparseRun {
                    start: [x, *y, *z],
                    length: 1,
                    material_slot: material,
                });
            }
        }
    }
    runs
}

fn bounds_for_cells(cells: &BTreeMap<[i64; 3], u16>) -> Option<VoxelAssetBounds> {
    bounds_for_coordinates(&cells.keys().copied().collect::<Vec<_>>())
}

fn bounds_for_coordinates(coordinates: &[[i64; 3]]) -> Option<VoxelAssetBounds> {
    let first = *coordinates.first()?;
    let mut min = first;
    let mut max = first;
    for coordinate in coordinates.iter().skip(1) {
        for axis in 0..3 {
            min[axis] = min[axis].min(coordinate[axis]);
            max[axis] = max[axis].max(coordinate[axis]);
        }
    }
    Some(VoxelAssetBounds { min, max })
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn asset_error(error: voxel_asset::VoxelAssetError) -> ConversionError {
    let first = error
        .diagnostics()
        .first()
        .expect("asset error has diagnostic");
    ConversionError::one(first.code, first.path.clone(), first.message.clone())
}
