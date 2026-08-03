use std::path::Path;

use core_assets::AssetKind;
use voxel_asset::{
    encode_voxel_object, resolve_voxel_frame, validate_voxel_object,
    with_computed_voxel_object_hashes, VoxelAssetBounds, VoxelConversionRequest, VoxelFrame,
    VoxelObjectAnimationFrame, VoxelObjectAsset, VoxelObjectClip, VoxelObjectGrid,
    VoxelObjectProvenance, VoxelObjectProvenanceKind, VoxelObjectSourceClipProvenance,
    MAX_VOXEL_OBJECT_FRAME_DURATION_SECONDS, MAX_VOXEL_OBJECT_TOTAL_FRAMES,
    MAX_VOXEL_OBJECT_TOTAL_VOXELS, VOXEL_OBJECT_SCHEMA_VERSION,
};

use super::model::*;
use super::validation::*;
use crate::{
    animation::{preflight_animation_bind_pose, preflight_animation_clip_range},
    convert::{convert_imported_mesh_with_material_sampling_in_bounds, replace_settings_identity},
    material::{material_sampling_context, resolve_material_map},
    planning::{transform_mesh, transform_mesh_owned, transform_position},
    sample_animation_bind_pose, sample_animation_clip_range,
    store::install_canonical_asset,
    voxelize::VoxelizationSourceBounds,
    AnimationBindPoseRequest, AnimationEndPolicy, AnimationSampleRangeReceipt,
    AnimationSampleRangeRequest, ConversionError, ImportedAnimatedMeshSource, ImportedMeshSource,
    ImportedStaticMesh,
};

struct SampledClip {
    request: VoxelObjectClipConversionRequest,
    sampling: AnimationSampleRangeReceipt,
}

struct ConvertedFrame {
    frame: VoxelFrame,
    voxelization_work: u64,
    voxel_count: usize,
    sparse_run_count: usize,
}

struct StoredFrame {
    frame: VoxelFrame,
    duration_microseconds: u64,
    source_timestamps_microseconds: Vec<u64>,
    voxel_count: usize,
    sparse_run_count: usize,
}

pub fn plan_static_voxel_object_conversion(
    request: &VoxelObjectConversionPlanRequest,
    source: &ImportedMeshSource,
) -> Result<PreparedVoxelObjectConversion, ConversionError> {
    validate_request(request, source, AssetKind::StaticMesh)?;
    if !request.clips.is_empty() || request.default_clip.is_some() {
        return Err(ConversionError::one(
            "conversion.unsupportedSource",
            "clips",
            "static voxel-object conversion cannot declare animation clips",
        ));
    }
    let default_mesh = transform_mesh(&source.mesh, request.settings.mesh.transform)?;
    let output = build_candidate(
        request,
        source,
        default_mesh,
        Vec::new(),
        VoxelObjectProvenanceKind::ConvertedStaticMesh,
        0,
    )?;
    prepare_plan(request, output)
}

pub fn plan_animated_voxel_object_conversion(
    request: &VoxelObjectConversionPlanRequest,
    source: &ImportedAnimatedMeshSource,
) -> Result<PreparedVoxelObjectConversion, ConversionError> {
    validate_request(request, &source.source, AssetKind::AnimatedMesh)?;
    let clips = canonical_clip_requests(request)?;
    let bind_request = AnimationBindPoseRequest {
        expected_source_sha256: source.source.receipt.source.source_sha256.clone(),
        anchor_policy: request.settings.anchor_policy,
    };
    let bind_estimate = preflight_animation_bind_pose(&source.model, &bind_request)?;
    let mut retained_snapshot_bytes = bind_estimate.materialized_snapshot_bytes;
    let mut deformation_work = bind_estimate.deformation_work;
    let mut total_sampled_frames = 1usize;
    let mut planned_clips = Vec::with_capacity(clips.len());
    for clip_request in &clips {
        let source_clip = source
            .model
            .clips
            .iter()
            .find(|clip| clip.name == clip_request.source_clip_name)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.clipNotFound",
                    "clips.sourceClipName",
                    format!(
                        "animation clip {:?} is not present",
                        clip_request.source_clip_name
                    ),
                )
            })?;
        let end_microseconds = clip_request
            .end_microseconds
            .unwrap_or(source_clip.duration_microseconds);
        let sampling_request = AnimationSampleRangeRequest {
            expected_source_sha256: source.source.receipt.source.source_sha256.clone(),
            clip_name: clip_request.source_clip_name.clone(),
            sample_rate_hz: clip_request.sample_rate_hz,
            start_microseconds: clip_request.start_microseconds,
            end_microseconds,
            end_policy: clip_request.end_policy,
            anchor_policy: request.settings.anchor_policy,
        };
        let estimate = preflight_animation_clip_range(&source.model, &sampling_request)?;
        retained_snapshot_bytes = retained_snapshot_bytes
            .checked_add(estimate.materialized_snapshot_bytes)
            .ok_or_else(|| {
                aggregate_snapshot_storage_limit("retained snapshot storage overflowed")
            })?;
        if retained_snapshot_bytes > MAX_VOXEL_OBJECT_CONVERSION_RETAINED_SNAPSHOT_BYTES {
            return Err(aggregate_snapshot_storage_limit(&format!(
                "bind pose and selected clips require {retained_snapshot_bytes} estimated retained bytes; limit is {MAX_VOXEL_OBJECT_CONVERSION_RETAINED_SNAPSHOT_BYTES}"
            )));
        }
        deformation_work = deformation_work
            .checked_add(estimate.deformation_work)
            .ok_or_else(|| aggregate_limit("animation deformation work overflowed"))?;
        if deformation_work > MAX_VOXEL_OBJECT_CONVERSION_DEFORMATION_WORK {
            return Err(aggregate_limit(&format!(
                "aggregate animation deformation work {deformation_work} exceeds {MAX_VOXEL_OBJECT_CONVERSION_DEFORMATION_WORK}"
            )));
        }
        total_sampled_frames = total_sampled_frames
            .checked_add(estimate.snapshot_count)
            .ok_or_else(|| aggregate_limit("sample frame count overflowed"))?;
        if total_sampled_frames > MAX_VOXEL_OBJECT_TOTAL_FRAMES {
            return Err(aggregate_limit(&format!(
                "selected clips contain {total_sampled_frames} samples; limit is {MAX_VOXEL_OBJECT_TOTAL_FRAMES}"
            )));
        }
        planned_clips.push((clip_request.clone(), sampling_request));
    }

    let bind = sample_animation_bind_pose(&source.model, &bind_request)?;
    debug_assert_eq!(
        bind.estimated_materialized_snapshot_bytes,
        bind_estimate.materialized_snapshot_bytes
    );
    let default_mesh = transform_mesh_owned(bind.mesh, request.settings.mesh.transform)?;
    let mut sampled_clips = Vec::with_capacity(planned_clips.len());
    for (clip_request, sampling_request) in planned_clips {
        let sampling = sample_animation_clip_range(&source.model, &sampling_request)?;
        sampled_clips.push(SampledClip {
            request: clip_request,
            sampling,
        });
    }
    let output = build_candidate(
        request,
        &source.source,
        default_mesh,
        sampled_clips,
        VoxelObjectProvenanceKind::ConvertedAnimatedMesh,
        deformation_work,
    )?;
    prepare_plan(request, output)
}

fn build_candidate(
    request: &VoxelObjectConversionPlanRequest,
    source: &ImportedMeshSource,
    default_mesh: ImportedStaticMesh,
    sampled_clips: Vec<SampledClip>,
    provenance_kind: VoxelObjectProvenanceKind,
    deformation_work: u64,
) -> Result<VoxelObjectConversionReceipt, ConversionError> {
    let fixed_bounds = match request.settings.source_bounds {
        Some(bounds) => VoxelizationSourceBounds::from_explicit(bounds.min, bounds.max)?,
        None => {
            let mut bounds = VoxelizationSourceBounds::for_mesh(&default_mesh)?;
            for sampled in &sampled_clips {
                for snapshot in &sampled.sampling.snapshots {
                    for position in &snapshot.mesh.positions {
                        bounds.include_position(transform_position(
                            *position,
                            request.settings.mesh.transform,
                        )?)?;
                    }
                }
            }
            bounds
        }
    };

    let mut effective_settings = request.settings.mesh.conversion.clone();
    effective_settings.material_map = resolve_material_map(&request.settings.mesh, source)?;
    let material_sampling = material_sampling_context(&request.settings.mesh, source)?;
    let settings_sha256 =
        object_settings_sha256(&request.settings, &request.clips, &request.default_clip);
    let conversion_request = VoxelConversionRequest {
        asset_id: "voxel-volume/object-conversion-frame".to_owned(),
        source_path: source.receipt.source_path.clone(),
        expected_source_sha256: source.receipt.source.source_sha256.clone(),
        license_path: request.license_path.clone(),
        settings: effective_settings.clone(),
    };
    let convert_frame = |mesh: &ImportedStaticMesh| -> Result<ConvertedFrame, ConversionError> {
        let receipt = replace_settings_identity(
            convert_imported_mesh_with_material_sampling_in_bounds(
                &conversion_request,
                mesh,
                source.receipt.source.source_sha256.clone(),
                source.receipt.source_byte_count,
                Some(&material_sampling),
                fixed_bounds,
            )?,
            settings_sha256.clone(),
        )?;
        Ok(ConvertedFrame {
            frame: VoxelFrame::from(&receipt.asset),
            voxelization_work: receipt.voxelization_work,
            voxel_count: receipt.output_voxels,
            sparse_run_count: receipt.sparse_runs,
        })
    };

    let converted_default = convert_frame(&default_mesh)?;
    let mut voxelization_work = converted_default.voxelization_work;
    let mut aggregate_voxels = converted_default.voxel_count;
    let mut sampled_frames = 1usize;
    let mut stored_frames = 1usize;
    let mut union_bounds = converted_default.frame.bounds;
    let mut object_clips = Vec::with_capacity(sampled_clips.len());
    let mut clip_readouts = Vec::with_capacity(sampled_clips.len());
    let mut source_clip_provenance = Vec::with_capacity(sampled_clips.len());

    for sampled in sampled_clips {
        let durations = sample_durations(&sampled.sampling)?;
        let sampled_frame_count = sampled.sampling.snapshots.len();
        let mut converted = Vec::with_capacity(sampled_frame_count);
        for (snapshot, duration) in sampled.sampling.snapshots.into_iter().zip(durations) {
            let source_timestamp_microseconds = snapshot.timestamp_microseconds;
            let mesh = transform_mesh_owned(snapshot.mesh, request.settings.mesh.transform)?;
            let frame = convert_frame(&mesh)?;
            voxelization_work = voxelization_work
                .checked_add(frame.voxelization_work)
                .ok_or_else(|| aggregate_limit("voxelization work overflowed"))?;
            if voxelization_work > MAX_VOXEL_OBJECT_CONVERSION_VOXELIZATION_WORK {
                return Err(aggregate_limit(&format!(
                    "aggregate voxelization work {voxelization_work} exceeds {MAX_VOXEL_OBJECT_CONVERSION_VOXELIZATION_WORK}"
                )));
            }
            aggregate_voxels = aggregate_voxels
                .checked_add(frame.voxel_count)
                .ok_or_else(|| aggregate_limit("aggregate voxel count overflowed"))?;
            if aggregate_voxels > MAX_VOXEL_OBJECT_TOTAL_VOXELS {
                return Err(aggregate_limit(&format!(
                    "sampled frames contain {aggregate_voxels} aggregate voxels; limit is {MAX_VOXEL_OBJECT_TOTAL_VOXELS}"
                )));
            }
            sampled_frames += 1;
            union_bounds = union(union_bounds, frame.frame.bounds);
            converted.push(StoredFrame {
                frame: frame.frame,
                duration_microseconds: duration,
                source_timestamps_microseconds: vec![source_timestamp_microseconds],
                voxel_count: frame.voxel_count,
                sparse_run_count: frame.sparse_run_count,
            });
        }
        let stored = deduplicate_consecutive_frames(converted);
        let duration_microseconds = stored.iter().try_fold(0u64, |total, frame| {
            total
                .checked_add(frame.duration_microseconds)
                .ok_or_else(|| aggregate_limit("clip duration overflowed"))
        })?;
        let frames = stored
            .iter()
            .map(|frame| VoxelObjectAnimationFrame {
                duration_seconds: Some(frame.duration_microseconds as f64 / 1_000_000.0),
                anchors: Vec::new(),
                collision: None,
                frame: frame.frame.clone(),
            })
            .collect::<Vec<_>>();
        let frame_readouts = stored
            .iter()
            .enumerate()
            .map(|(index, frame)| VoxelObjectConvertedFrameReadout {
                stored_frame_index: index as u32,
                source_timestamps_microseconds: frame.source_timestamps_microseconds.clone(),
                duration_microseconds: frame.duration_microseconds,
                bounds: frame.frame.bounds,
                voxel_count: frame.voxel_count,
                sparse_run_count: frame.sparse_run_count,
                voxel_data_hash: frame.frame.voxel_data_hash.clone(),
            })
            .collect::<Vec<_>>();
        stored_frames += stored.len();
        object_clips.push(VoxelObjectClip {
            id: sampled.request.output_clip_id.clone(),
            name: sampled.request.output_name.clone(),
            frames_per_second: f64::from(sampled.request.sample_rate_hz),
            frames,
        });
        clip_readouts.push(VoxelObjectConvertedClipReadout {
            output_clip_id: sampled.request.output_clip_id.clone(),
            source_clip_name: sampled.sampling.clip_name.clone(),
            source_animation_index: sampled.sampling.source_animation_index,
            start_microseconds: sampled.sampling.start_microseconds,
            end_microseconds: sampled.sampling.end_microseconds,
            sample_rate_hz: sampled.sampling.sample_rate_hz,
            end_policy: sampled.sampling.end_policy,
            sampled_frame_count,
            stored_frame_count: stored.len(),
            duration_microseconds,
            frames: frame_readouts,
        });
        source_clip_provenance.push(VoxelObjectSourceClipProvenance {
            output_clip_id: sampled.request.output_clip_id,
            source_clip_name: sampled.sampling.clip_name,
            source_animation_index: sampled.sampling.source_animation_index,
            start_microseconds: sampled.sampling.start_microseconds,
            end_microseconds: sampled.sampling.end_microseconds,
            sample_rate_hz: sampled.sampling.sample_rate_hz,
            included_clip_end: sampled.sampling.end_policy == AnimationEndPolicy::IncludeClipEnd,
        });
    }

    let asset = with_computed_voxel_object_hashes(VoxelObjectAsset {
        schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
        asset_id: request.target_asset_id.clone(),
        grid: VoxelObjectGrid {
            coordinate_system: voxel_asset::VoxelCoordinateSystem::RightHandedYUp,
            cell_size: effective_settings.cell_size,
            chunk_size: effective_settings.chunk_size,
            pivot: request.settings.pivot,
        },
        bounds: union_bounds,
        default_frame: converted_default.frame,
        clips: object_clips,
        default_clip: request.default_clip.clone(),
        material_palette: effective_settings.material_palette,
        material_map: effective_settings.material_map,
        provenance: VoxelObjectProvenance {
            kind: provenance_kind,
            source_path: source.receipt.source_path.clone(),
            source_sha256: source.receipt.source.source_sha256.clone(),
            source_byte_count: source.receipt.source_byte_count,
            converter: VOXEL_OBJECT_CONVERTER_ID.to_owned(),
            settings_sha256: settings_sha256.clone(),
            license_path: request.license_path.clone(),
            source_clips: source_clip_provenance,
        },
        content_hash: String::new(),
    })
    .map_err(object_error)?;
    let canonical_json = encode_voxel_object(&asset).map_err(object_error)?;
    let artifact_bytes = canonical_json.len();
    let content_hash = asset.content_hash.clone();
    Ok(VoxelObjectConversionReceipt {
        source_vertices: source.mesh.positions.len(),
        source_triangles: source.mesh.triangles.len(),
        source_sha256: source.receipt.source.source_sha256.clone(),
        settings_sha256,
        content_hash,
        deformation_work,
        voxelization_work,
        sampled_frames,
        stored_frames,
        aggregate_voxels,
        artifact_bytes,
        bounds: asset.bounds,
        clips: clip_readouts,
        asset,
        canonical_json,
    })
}

fn aggregate_snapshot_storage_limit(message: &str) -> ConversionError {
    ConversionError::one("conversion.resourceLimit", "clips.snapshotStorage", message)
}

fn prepare_plan(
    request: &VoxelObjectConversionPlanRequest,
    output: VoxelObjectConversionReceipt,
) -> Result<PreparedVoxelObjectConversion, ConversionError> {
    let clips = canonical_clip_requests(request)?;
    let clip_summaries = output
        .clips
        .iter()
        .map(|clip| VoxelObjectClipPlanSummary {
            output_clip_id: clip.output_clip_id.clone(),
            source_clip_name: clip.source_clip_name.clone(),
            source_animation_index: clip.source_animation_index,
            start_microseconds: clip.start_microseconds,
            end_microseconds: clip.end_microseconds,
            sample_rate_hz: clip.sample_rate_hz,
            sampled_frame_count: clip.sampled_frame_count,
            stored_frame_count: clip.stored_frame_count,
            duration_microseconds: clip.duration_microseconds,
        })
        .collect::<Vec<_>>();
    let settings_sha256 =
        object_settings_sha256(&request.settings, &request.clips, &request.default_clip);
    let plan_id = object_plan_id(request, &settings_sha256);
    let mut plan = VoxelObjectConversionPlan {
        plan_id,
        source: request.source.clone(),
        source_path: request.source_path.clone(),
        target_asset_id: request.target_asset_id.clone(),
        license_path: request.license_path.clone(),
        settings: request.settings.clone(),
        clips,
        default_clip: request.default_clip.clone(),
        planner: VOXEL_OBJECT_CONVERSION_PLANNER_ID.to_owned(),
        expected_source_sha256: request.source.source_sha256.clone(),
        settings_sha256,
        expected_output_content_hash: output.content_hash.clone(),
        plan_hash: String::new(),
        estimated_sampled_frames: output.sampled_frames,
        estimated_stored_frames: output.stored_frames,
        estimated_aggregate_voxels: output.aggregate_voxels,
        estimated_artifact_bytes: output.artifact_bytes,
        estimated_bounds: output.bounds,
        clip_summaries,
    };
    plan.plan_hash = voxel_object_conversion_plan_hash(&plan);
    Ok(PreparedVoxelObjectConversion { plan, output })
}

pub fn voxel_object_conversion_plan_hash(plan: &VoxelObjectConversionPlan) -> String {
    let mut canonical = plan.clone();
    canonical.plan_hash.clear();
    sha256_json(&canonical)
}

pub fn preview_voxel_object_conversion(
    request: &VoxelObjectConversionPreviewRequest,
    prepared: &PreparedVoxelObjectConversion,
) -> Result<VoxelObjectConversionPreview, ConversionError> {
    validate_prepared(&request.plan_id, &request.expected_plan_hash, prepared)?;
    let max_samples = request.max_samples as usize;
    if !(1..=MAX_VOXEL_OBJECT_PREVIEW_SAMPLES).contains(&max_samples) {
        return Err(ConversionError::one(
            "conversion.queryQuotaExceeded",
            "maxSamples",
            format!("maxSamples must be in 1..={MAX_VOXEL_OBJECT_PREVIEW_SAMPLES}"),
        ));
    }
    let (frame, duration_microseconds, source_timestamps_microseconds) =
        selected_frame(prepared, &request.frame)?;
    let mut cells = resolve_voxel_frame(
        frame,
        prepared
            .output
            .asset
            .material_palette
            .iter()
            .map(|binding| binding.material_slot),
    )
    .map_err(frame_error)?;
    cells.sort_by_key(|cell| (cell.coordinate[2], cell.coordinate[1], cell.coordinate[0]));
    let selected_frame = VoxelObjectSelectedFramePreview {
        selection: request.frame.clone(),
        bounds: frame.bounds,
        voxel_count: cells.len(),
        sparse_run_count: frame.representation.sparse_runs.len(),
        voxel_data_hash: frame.voxel_data_hash.clone(),
        duration_microseconds,
        source_timestamps_microseconds,
        sample_voxels: cells
            .iter()
            .take(max_samples)
            .map(|cell| VoxelObjectPreviewVoxel {
                coordinate: cell.coordinate,
                material_slot: cell.material_slot,
            })
            .collect(),
        samples_truncated: cells.len() > max_samples,
    };
    Ok(VoxelObjectConversionPreview {
        plan_id: prepared.plan.plan_id.clone(),
        plan_hash: prepared.plan.plan_hash.clone(),
        output_hash: prepared.output.content_hash.clone(),
        sampled_frame_count: prepared.output.sampled_frames,
        stored_frame_count: prepared.output.stored_frames,
        aggregate_voxel_count: prepared.output.aggregate_voxels,
        artifact_bytes: prepared.output.artifact_bytes,
        union_bounds: prepared.output.bounds,
        clips: prepared.output.clips.clone(),
        selected_frame,
    })
}

pub fn apply_voxel_object_conversion(
    request: &VoxelObjectConversionApplyRequest,
    prepared: &PreparedVoxelObjectConversion,
) -> Result<AppliedVoxelObjectConversion, ConversionError> {
    validate_prepared(&request.plan_id, &request.expected_plan_hash, prepared)?;
    validate_voxel_object(&prepared.output.asset).map_err(object_error)?;
    if request
        .expected_output_hash
        .as_ref()
        .is_some_and(|expected| expected != &prepared.output.content_hash)
    {
        return Err(ConversionError::one(
            "conversion.staleOutput",
            "expectedOutputHash",
            "apply request expected a different prepared voxel-object output",
        ));
    }
    Ok(AppliedVoxelObjectConversion {
        plan_id: prepared.plan.plan_id.clone(),
        plan_hash: prepared.plan.plan_hash.clone(),
        output_hash: prepared.output.content_hash.clone(),
        conversion: prepared.output.clone(),
    })
}

pub fn apply_voxel_object_conversion_and_install(
    request: &VoxelObjectConversionApplyRequest,
    prepared: &PreparedVoxelObjectConversion,
    output_path: &Path,
) -> Result<AppliedVoxelObjectConversion, ConversionError> {
    let applied = apply_voxel_object_conversion(request, prepared)?;
    install_canonical_asset(&applied.conversion.canonical_json, output_path)?;
    Ok(applied)
}

fn sample_durations(sampling: &AnimationSampleRangeReceipt) -> Result<Vec<u64>, ConversionError> {
    let nominal = (1_000_000u64 + u64::from(sampling.sample_rate_hz / 2))
        / u64::from(sampling.sample_rate_hz);
    let mut durations = Vec::with_capacity(sampling.snapshots.len());
    for pair in sampling.snapshots.windows(2) {
        durations.push(
            pair[1]
                .timestamp_microseconds
                .checked_sub(pair[0].timestamp_microseconds)
                .ok_or_else(|| aggregate_limit("sample timestamps moved backwards"))?,
        );
    }
    let last = match sampling.end_policy {
        AnimationEndPolicy::IncludeClipEnd => nominal.max(1),
        AnimationEndPolicy::ExcludeLoopSeam => sampling
            .end_microseconds
            .checked_sub(
                sampling
                    .snapshots
                    .last()
                    .expect("range sampler always emits one sample")
                    .timestamp_microseconds,
            )
            .filter(|duration| *duration > 0)
            .unwrap_or(nominal.max(1)),
    };
    durations.push(last);
    Ok(durations)
}

fn deduplicate_consecutive_frames(frames: Vec<StoredFrame>) -> Vec<StoredFrame> {
    let limit_microseconds = (MAX_VOXEL_OBJECT_FRAME_DURATION_SECONDS * 1_000_000.0) as u64;
    let mut stored: Vec<StoredFrame> = Vec::with_capacity(frames.len());
    for frame in frames {
        let merge = stored.last().is_some_and(|prior| {
            prior.frame.voxel_data_hash == frame.frame.voxel_data_hash
                && prior
                    .duration_microseconds
                    .checked_add(frame.duration_microseconds)
                    .is_some_and(|duration| duration <= limit_microseconds)
        });
        if merge {
            let prior = stored.last_mut().expect("checked prior frame");
            prior.duration_microseconds += frame.duration_microseconds;
            prior
                .source_timestamps_microseconds
                .extend(frame.source_timestamps_microseconds);
        } else {
            stored.push(frame);
        }
    }
    stored
}

fn selected_frame<'a>(
    prepared: &'a PreparedVoxelObjectConversion,
    selection: &VoxelObjectFrameSelection,
) -> Result<(&'a VoxelFrame, Option<u64>, Vec<u64>), ConversionError> {
    match selection {
        VoxelObjectFrameSelection::Default => {
            Ok((&prepared.output.asset.default_frame, None, vec![0]))
        }
        VoxelObjectFrameSelection::Clip {
            clip_id,
            frame_index,
        } => {
            let clip = prepared
                .output
                .asset
                .clip(clip_id)
                .ok_or_else(|| unknown_frame(clip_id, *frame_index))?;
            let frame = clip
                .frames
                .get(*frame_index as usize)
                .ok_or_else(|| unknown_frame(clip_id, *frame_index))?;
            let readout = prepared
                .output
                .clips
                .iter()
                .find(|clip| clip.output_clip_id == *clip_id)
                .and_then(|clip| clip.frames.get(*frame_index as usize))
                .ok_or_else(|| unknown_frame(clip_id, *frame_index))?;
            Ok((
                &frame.frame,
                Some(readout.duration_microseconds),
                readout.source_timestamps_microseconds.clone(),
            ))
        }
    }
}

fn validate_prepared(
    plan_id: &str,
    expected_plan_hash: &str,
    prepared: &PreparedVoxelObjectConversion,
) -> Result<(), ConversionError> {
    let plan = &prepared.plan;
    let output = &prepared.output;
    if plan_id != plan.plan_id
        || expected_plan_hash != plan.plan_hash
        || voxel_object_conversion_plan_hash(plan) != plan.plan_hash
        || object_plan_id_from_plan(plan) != plan.plan_id
        || object_settings_sha256(&plan.settings, &plan.clips, &plan.default_clip)
            != plan.settings_sha256
        || plan.source.source_sha256 != plan.expected_source_sha256
        || output.asset.asset_id != plan.target_asset_id
        || output.asset.provenance.source_path != plan.source_path
        || output.asset.provenance.source_sha256 != plan.expected_source_sha256
        || output.asset.provenance.settings_sha256 != plan.settings_sha256
        || output.asset.provenance.license_path != plan.license_path
        || output.source_sha256 != plan.expected_source_sha256
        || output.settings_sha256 != plan.settings_sha256
        || output.content_hash != plan.expected_output_content_hash
        || output.asset.content_hash != plan.expected_output_content_hash
        || output.sampled_frames != plan.estimated_sampled_frames
        || output.stored_frames != plan.estimated_stored_frames
        || output.aggregate_voxels != plan.estimated_aggregate_voxels
        || output.artifact_bytes != plan.estimated_artifact_bytes
        || output.bounds != plan.estimated_bounds
    {
        return Err(ConversionError::one(
            "conversion.stalePlan",
            "plan",
            "request does not match the prepared voxel-object conversion plan",
        ));
    }
    Ok(())
}

fn union(left: VoxelAssetBounds, right: VoxelAssetBounds) -> VoxelAssetBounds {
    VoxelAssetBounds {
        min: std::array::from_fn(|axis| left.min[axis].min(right.min[axis])),
        max: std::array::from_fn(|axis| left.max[axis].max(right.max[axis])),
    }
}

fn unknown_frame(clip_id: &str, frame_index: u32) -> ConversionError {
    ConversionError::one(
        "conversion.frameNotFound",
        "frame",
        format!("clip {clip_id:?} has no stored frame {frame_index}"),
    )
}

fn frame_error(error: voxel_asset::VoxelFrameError) -> ConversionError {
    let first = error
        .diagnostics()
        .first()
        .expect("voxel frame errors contain diagnostics");
    ConversionError::one(first.code, first.path.clone(), first.message.clone())
}

fn object_error(error: voxel_asset::VoxelObjectError) -> ConversionError {
    let first = error
        .diagnostics()
        .first()
        .expect("voxel object errors contain diagnostics");
    ConversionError::one(first.code, first.path.clone(), first.message.clone())
}
