use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use voxel_asset::{
    represented_voxel_count, resolve_voxel_frame, validate_voxel_object, VoxelAssetBounds,
    VoxelFrame, VoxelObjectAsset,
};

use crate::{ConversionError, VoxelModelMaterialCount, VoxelObjectFrameSelection};

pub const MAX_VOXEL_OBJECT_WINDOW_CELLS: u64 = 1_000_000;
pub const MAX_VOXEL_OBJECT_WINDOW_SAMPLES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectInfoRequest {
    pub expected_content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectQueryClipReadout {
    pub clip_id: String,
    pub frame_count: usize,
    pub duration_microseconds: u64,
    pub aggregate_voxel_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectInfoReadout {
    pub asset_id: String,
    pub content_hash: String,
    pub bounds: VoxelAssetBounds,
    pub pivot: [f64; 3],
    pub default_frame_voxel_count: usize,
    pub clip_count: usize,
    pub total_stored_frame_count: usize,
    pub total_stored_voxel_count: usize,
    pub artifact_source_sha256: String,
    pub settings_sha256: String,
    pub clips: Vec<VoxelObjectQueryClipReadout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectFrameRequest {
    pub expected_content_hash: String,
    pub frame: VoxelObjectFrameSelection,
    pub include_material_counts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectFrameReadout {
    pub asset_id: String,
    pub content_hash: String,
    pub selection: VoxelObjectFrameSelection,
    pub bounds: VoxelAssetBounds,
    pub voxel_data_hash: String,
    pub voxel_count: usize,
    pub sparse_run_count: usize,
    pub duration_microseconds: Option<u64>,
    pub material_counts: Vec<VoxelModelMaterialCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectWindowRequest {
    pub expected_content_hash: String,
    pub frame: VoxelObjectFrameSelection,
    pub bounds: VoxelAssetBounds,
    pub include_empty: bool,
    pub material_filter: Vec<u16>,
    pub max_samples: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectWindowSample {
    pub coordinate: [i64; 3],
    pub material_slot: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectWindowReadout {
    pub asset_id: String,
    pub content_hash: String,
    pub selection: VoxelObjectFrameSelection,
    pub requested_bounds: VoxelAssetBounds,
    pub frame_bounds: VoxelAssetBounds,
    pub scanned_cell_count: u64,
    pub samples: Vec<VoxelObjectWindowSample>,
    pub samples_truncated: bool,
}

pub fn query_voxel_object_info(
    object: &VoxelObjectAsset,
    request: &VoxelObjectInfoRequest,
) -> Result<VoxelObjectInfoReadout, ConversionError> {
    validate_snapshot(object, &request.expected_content_hash)?;
    let clips = object
        .clips
        .iter()
        .map(|clip| {
            let duration_microseconds = clip
                .frames
                .iter()
                .try_fold(0u64, |total, frame| {
                    total.checked_add(frame_duration_microseconds(
                        frame.duration_seconds,
                        clip.frames_per_second,
                    ))
                })
                .ok_or_else(|| invalid_object("clip duration overflowed"))?;
            Ok(VoxelObjectQueryClipReadout {
                clip_id: clip.id.clone(),
                frame_count: clip.frames.len(),
                duration_microseconds,
                aggregate_voxel_count: clip
                    .frames
                    .iter()
                    .map(|frame| represented_voxel_count(&frame.frame))
                    .sum(),
            })
        })
        .collect::<Result<Vec<_>, ConversionError>>()?;
    let total_stored_frame_count = 1 + clips.iter().map(|clip| clip.frame_count).sum::<usize>();
    let default_frame_voxel_count = represented_voxel_count(&object.default_frame);
    let total_stored_voxel_count = default_frame_voxel_count
        + clips
            .iter()
            .map(|clip| clip.aggregate_voxel_count)
            .sum::<usize>();
    Ok(VoxelObjectInfoReadout {
        asset_id: object.asset_id.clone(),
        content_hash: object.content_hash.clone(),
        bounds: object.bounds,
        pivot: object.grid.pivot,
        default_frame_voxel_count,
        clip_count: clips.len(),
        total_stored_frame_count,
        total_stored_voxel_count,
        artifact_source_sha256: object.provenance.source_sha256.clone(),
        settings_sha256: object.provenance.settings_sha256.clone(),
        clips,
    })
}

pub fn query_voxel_object_frame(
    object: &VoxelObjectAsset,
    request: &VoxelObjectFrameRequest,
) -> Result<VoxelObjectFrameReadout, ConversionError> {
    validate_snapshot(object, &request.expected_content_hash)?;
    let (frame, duration_microseconds) = select_frame(object, &request.frame)?;
    let cells = resolve_cells(object, frame)?;
    let material_counts = if request.include_material_counts {
        let mut counts = BTreeMap::<u16, usize>::new();
        for cell in &cells {
            *counts.entry(cell.material_slot).or_default() += 1;
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
    Ok(VoxelObjectFrameReadout {
        asset_id: object.asset_id.clone(),
        content_hash: object.content_hash.clone(),
        selection: request.frame.clone(),
        bounds: frame.bounds,
        voxel_data_hash: frame.voxel_data_hash.clone(),
        voxel_count: cells.len(),
        sparse_run_count: frame.representation.sparse_runs.len(),
        duration_microseconds,
        material_counts,
    })
}

pub fn query_voxel_object_window(
    object: &VoxelObjectAsset,
    request: &VoxelObjectWindowRequest,
) -> Result<VoxelObjectWindowReadout, ConversionError> {
    validate_snapshot(object, &request.expected_content_hash)?;
    let (frame, _) = select_frame(object, &request.frame)?;
    let volume = bounds_volume(request.bounds)?;
    if volume > MAX_VOXEL_OBJECT_WINDOW_CELLS {
        return Err(quota(format!(
            "requested window has {volume} cells; limit is {MAX_VOXEL_OBJECT_WINDOW_CELLS}"
        )));
    }
    let max_samples = request.max_samples as usize;
    if !(1..=MAX_VOXEL_OBJECT_WINDOW_SAMPLES).contains(&max_samples) {
        return Err(quota(format!(
            "maxSamples must be in 1..={MAX_VOXEL_OBJECT_WINDOW_SAMPLES}"
        )));
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
    let occupied = resolve_cells(object, frame)?
        .into_iter()
        .map(|cell| (cell.coordinate, cell.material_slot))
        .collect::<BTreeMap<_, _>>();
    let mut samples = Vec::new();
    let mut matching_count = 0usize;
    if request.include_empty {
        for z in request.bounds.min[2]..=request.bounds.max[2] {
            for y in request.bounds.min[1]..=request.bounds.max[1] {
                for x in request.bounds.min[0]..=request.bounds.max[0] {
                    let coordinate = [x, y, z];
                    let material_slot = occupied.get(&coordinate).copied();
                    if !material_filter.is_empty()
                        && !material_slot.is_some_and(|slot| material_filter.contains(&slot))
                    {
                        continue;
                    }
                    matching_count += 1;
                    if samples.len() < max_samples {
                        samples.push(VoxelObjectWindowSample {
                            coordinate,
                            material_slot,
                        });
                    }
                }
            }
        }
    } else {
        let mut matching = occupied
            .into_iter()
            .filter(|(coordinate, material)| {
                contains(request.bounds, *coordinate)
                    && (material_filter.is_empty() || material_filter.contains(material))
            })
            .collect::<Vec<_>>();
        matching.sort_by_key(|(coordinate, _)| (coordinate[2], coordinate[1], coordinate[0]));
        matching_count = matching.len();
        samples.extend(matching.into_iter().take(max_samples).map(
            |(coordinate, material_slot)| VoxelObjectWindowSample {
                coordinate,
                material_slot: Some(material_slot),
            },
        ));
    }
    Ok(VoxelObjectWindowReadout {
        asset_id: object.asset_id.clone(),
        content_hash: object.content_hash.clone(),
        selection: request.frame.clone(),
        requested_bounds: request.bounds,
        frame_bounds: frame.bounds,
        scanned_cell_count: volume,
        samples,
        samples_truncated: matching_count > max_samples,
    })
}

fn validate_snapshot(object: &VoxelObjectAsset, expected: &str) -> Result<(), ConversionError> {
    validate_voxel_object(object).map_err(|error| {
        let first = error
            .diagnostics()
            .first()
            .expect("voxel object error contains diagnostics");
        ConversionError::one(first.code, first.path.clone(), first.message.clone())
    })?;
    if expected != object.content_hash {
        return Err(ConversionError::one(
            "conversion.staleAuthoritySnapshot",
            "expectedContentHash",
            "query expected a different voxel-object content hash",
        ));
    }
    Ok(())
}

fn select_frame<'a>(
    object: &'a VoxelObjectAsset,
    selection: &VoxelObjectFrameSelection,
) -> Result<(&'a VoxelFrame, Option<u64>), ConversionError> {
    match selection {
        VoxelObjectFrameSelection::Default => Ok((&object.default_frame, None)),
        VoxelObjectFrameSelection::Clip {
            clip_id,
            frame_index,
        } => {
            let clip = object
                .clip(clip_id)
                .ok_or_else(|| frame_not_found(clip_id, *frame_index))?;
            let frame = clip
                .frames
                .get(*frame_index as usize)
                .ok_or_else(|| frame_not_found(clip_id, *frame_index))?;
            Ok((
                &frame.frame,
                Some(frame_duration_microseconds(
                    frame.duration_seconds,
                    clip.frames_per_second,
                )),
            ))
        }
    }
}

fn resolve_cells(
    object: &VoxelObjectAsset,
    frame: &VoxelFrame,
) -> Result<Vec<voxel_asset::VoxelFrameCell>, ConversionError> {
    resolve_voxel_frame(
        frame,
        object
            .material_palette
            .iter()
            .map(|binding| binding.material_slot),
    )
    .map_err(|error| {
        let first = error
            .diagnostics()
            .first()
            .expect("voxel frame error contains diagnostics");
        ConversionError::one(first.code, first.path.clone(), first.message.clone())
    })
}

fn frame_duration_microseconds(duration: Option<f64>, frames_per_second: f64) -> u64 {
    (duration.unwrap_or(1.0 / frames_per_second) * 1_000_000.0).round() as u64
}

fn bounds_volume(bounds: VoxelAssetBounds) -> Result<u64, ConversionError> {
    let mut volume = 1u64;
    for axis in 0..3 {
        let length = bounds.max[axis]
            .checked_sub(bounds.min[axis])
            .and_then(|difference| difference.checked_add(1))
            .and_then(|length| u64::try_from(length).ok())
            .filter(|_| bounds.min[axis] <= bounds.max[axis])
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidQueryBounds",
                    "bounds",
                    "bounds must be inclusive, ordered, and non-overflowing",
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

fn quota(message: String) -> ConversionError {
    ConversionError::one("conversion.queryQuotaExceeded", "bounds", message)
}

fn frame_not_found(clip_id: &str, frame_index: u32) -> ConversionError {
    ConversionError::one(
        "conversion.frameNotFound",
        "frame",
        format!("clip {clip_id:?} has no stored frame {frame_index}"),
    )
}

fn invalid_object(message: &str) -> ConversionError {
    ConversionError::one("conversion.invalidPreparedOutput", "object", message)
}
