use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use voxel_asset::{VoxelAsset, VoxelAssetBounds};

use crate::ConversionError;

pub const MAX_MODEL_WINDOW_CELLS: u64 = 1_000_000;
pub const MAX_MODEL_WINDOW_SAMPLES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelModelInfoRequest {
    pub expected_content_hash: String,
    pub include_material_counts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelModelMaterialCount {
    pub material_slot: u16,
    pub voxel_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelModelInfoReadout {
    pub asset_id: String,
    pub content_hash: String,
    pub voxel_data_hash: String,
    pub bounds: VoxelAssetBounds,
    pub voxel_count: usize,
    pub sparse_run_count: usize,
    pub material_counts: Vec<VoxelModelMaterialCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelModelWindowRequest {
    pub expected_content_hash: String,
    pub bounds: VoxelAssetBounds,
    pub include_empty: bool,
    pub material_filter: Vec<u16>,
    pub max_samples: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelModelWindowSample {
    pub coordinate: [i64; 3],
    pub material_slot: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelModelWindowReadout {
    pub asset_id: String,
    pub content_hash: String,
    pub requested_bounds: VoxelAssetBounds,
    pub model_bounds: VoxelAssetBounds,
    pub scanned_cell_count: u64,
    pub samples: Vec<VoxelModelWindowSample>,
    pub samples_truncated: bool,
}

pub fn query_model_info(
    asset: &VoxelAsset,
    request: &VoxelModelInfoRequest,
) -> Result<VoxelModelInfoReadout, ConversionError> {
    validate_snapshot(asset, &request.expected_content_hash)?;
    let occupied = occupied_voxels(asset);
    let material_counts = if request.include_material_counts {
        let mut counts = BTreeMap::<u16, usize>::new();
        for material in occupied.values() {
            *counts.entry(*material).or_default() += 1;
        }
        counts
            .into_iter()
            .map(|(material_slot, voxel_count)| VoxelModelMaterialCount {
                material_slot,
                voxel_count,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(VoxelModelInfoReadout {
        asset_id: asset.asset_id.clone(),
        content_hash: asset.content_hash.clone(),
        voxel_data_hash: asset.voxel_data_hash.clone(),
        bounds: asset.bounds,
        voxel_count: occupied.len(),
        sparse_run_count: asset.representation.sparse_runs.len(),
        material_counts,
    })
}

pub fn query_model_window(
    asset: &VoxelAsset,
    request: &VoxelModelWindowRequest,
) -> Result<VoxelModelWindowReadout, ConversionError> {
    validate_snapshot(asset, &request.expected_content_hash)?;
    let volume = bounds_volume(request.bounds)?;
    if volume > MAX_MODEL_WINDOW_CELLS {
        return Err(ConversionError::one(
            "conversion.queryQuotaExceeded",
            "bounds",
            format!("requested window has {volume} cells; limit is {MAX_MODEL_WINDOW_CELLS}"),
        ));
    }
    let max_samples = request.max_samples as usize;
    if !(1..=MAX_MODEL_WINDOW_SAMPLES).contains(&max_samples) {
        return Err(ConversionError::one(
            "conversion.queryQuotaExceeded",
            "maxSamples",
            format!("maxSamples must be in 1..={MAX_MODEL_WINDOW_SAMPLES}"),
        ));
    }
    let material_filter = request
        .material_filter
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if material_filter.len() != request.material_filter.len() || material_filter.contains(&0) {
        return Err(ConversionError::one(
            "conversion.invalidQueryBounds",
            "materialFilter",
            "materialFilter must contain unique non-zero material slots",
        ));
    }
    let occupied = occupied_voxels(asset);
    let mut samples = Vec::new();
    let mut matching_count = 0usize;
    if request.include_empty {
        for z in request.bounds.min[2]..=request.bounds.max[2] {
            for y in request.bounds.min[1]..=request.bounds.max[1] {
                for x in request.bounds.min[0]..=request.bounds.max[0] {
                    let coordinate = [x, y, z];
                    let material = occupied.get(&coordinate).copied();
                    if !material_filter.is_empty()
                        && !material.is_some_and(|slot| material_filter.contains(&slot))
                    {
                        continue;
                    }
                    matching_count += 1;
                    if samples.len() < max_samples {
                        samples.push(VoxelModelWindowSample {
                            coordinate,
                            material_slot: material,
                        });
                    }
                }
            }
        }
    } else {
        let mut matching = occupied
            .iter()
            .filter(|(coordinate, material_slot)| {
                contains(request.bounds, **coordinate)
                    && (material_filter.is_empty() || material_filter.contains(material_slot))
            })
            .map(|(coordinate, material_slot)| (*coordinate, *material_slot))
            .collect::<Vec<_>>();
        matching.sort_by_key(|(coordinate, _)| (coordinate[2], coordinate[1], coordinate[0]));
        for (coordinate, material_slot) in matching {
            matching_count += 1;
            if samples.len() < max_samples {
                samples.push(VoxelModelWindowSample {
                    coordinate,
                    material_slot: Some(material_slot),
                });
            }
        }
    }
    Ok(VoxelModelWindowReadout {
        asset_id: asset.asset_id.clone(),
        content_hash: asset.content_hash.clone(),
        requested_bounds: request.bounds,
        model_bounds: asset.bounds,
        scanned_cell_count: volume,
        samples,
        samples_truncated: matching_count > max_samples,
    })
}

pub(crate) fn occupied_voxels(asset: &VoxelAsset) -> BTreeMap<[i64; 3], u16> {
    let mut occupied = BTreeMap::new();
    for run in &asset.representation.sparse_runs {
        for offset in 0..run.length {
            occupied.insert(
                [run.start[0] + i64::from(offset), run.start[1], run.start[2]],
                run.material_slot,
            );
        }
    }
    occupied
}

fn validate_snapshot(asset: &VoxelAsset, expected: &str) -> Result<(), ConversionError> {
    if expected != asset.content_hash {
        return Err(ConversionError::one(
            "conversion.staleAuthoritySnapshot",
            "expectedContentHash",
            "query expected a different voxel asset content hash",
        ));
    }
    Ok(())
}

fn bounds_volume(bounds: VoxelAssetBounds) -> Result<u64, ConversionError> {
    let mut volume = 1u64;
    for axis in 0..3 {
        if bounds.min[axis] > bounds.max[axis] {
            return Err(ConversionError::one(
                "conversion.invalidQueryBounds",
                "bounds",
                "bounds must be inclusive and ordered on every axis",
            ));
        }
        let length = bounds.max[axis]
            .checked_sub(bounds.min[axis])
            .and_then(|difference| difference.checked_add(1))
            .and_then(|length| u64::try_from(length).ok())
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidQueryBounds",
                    "bounds",
                    "bounds volume overflowed",
                )
            })?;
        volume = volume.checked_mul(length).ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidQueryBounds",
                "bounds",
                "bounds volume overflowed",
            )
        })?;
    }
    Ok(volume)
}

fn contains(bounds: VoxelAssetBounds, coordinate: [i64; 3]) -> bool {
    (0..3).all(|axis| (bounds.min[axis]..=bounds.max[axis]).contains(&coordinate[axis]))
}
