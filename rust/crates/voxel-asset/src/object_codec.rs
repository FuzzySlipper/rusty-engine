use std::collections::BTreeSet;

use core_assets::{AssetId, AssetKind};
use core_space::MAX_VOXEL_COORDINATE_ABS;
use sha2::{Digest, Sha256};

use crate::{
    frame::{canonicalize_frame, represented_voxel_count},
    VoxelAssetBounds, VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelObjectAsset,
    VoxelObjectCollisionPrimitive, VoxelObjectFrameCollision, VoxelObjectProvenance,
    VoxelObjectProvenanceKind, VOXEL_OBJECT_SCHEMA_VERSION,
};

/// Most voxels all of one object's frames may represent together. Admission
/// expands every frame's runs into cells before runtime limits apply.
pub const MAX_VOXEL_OBJECT_TOTAL_VOXELS: usize = 16_777_216;
const MAX_LOCAL_COORDINATE_ABS: f64 = MAX_VOXEL_COORDINATE_ABS as f64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelObjectDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelObjectError {
    diagnostics: Vec<VoxelObjectDiagnostic>,
}

impl VoxelObjectError {
    pub fn diagnostics(&self) -> &[VoxelObjectDiagnostic] {
        &self.diagnostics
    }

    fn one(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![diagnostic(code, path, message)],
        }
    }
}

impl std::fmt::Display for VoxelObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first = self
            .diagnostics
            .first()
            .expect("voxel object error always has a diagnostic");
        write!(
            formatter,
            "{} at {}: {}",
            first.code, first.path, first.message
        )
    }
}

impl std::error::Error for VoxelObjectError {}

pub fn decode_voxel_object(input: &str) -> Result<VoxelObjectAsset, VoxelObjectError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let object: VoxelObjectAsset =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            VoxelObjectError::one(
                "voxelObject.decode",
                json_path(&error.path().to_string()),
                error.inner().to_string(),
            )
        })?;
    deserializer.end().map_err(|error| {
        VoxelObjectError::one(
            "voxelObject.decode",
            "$",
            format!(
                "{} at line {}, column {}",
                error,
                error.line(),
                error.column()
            ),
        )
    })?;
    validate_voxel_object(&object)?;
    let mut canonical = object;
    canonicalize(&mut canonical);
    verify_stated_identities(canonical)
}

/// Compute a decoded object's frame and content identities once and refuse a
/// file whose stated identities do not match its content.
///
/// Content files can be edited by hand or written back by tooling and
/// reloaded, and every later consumer keys caches and expected-hash guards on
/// the stored identities, so they are checked here where the file enters.
fn verify_stated_identities(
    mut object: VoxelObjectAsset,
) -> Result<VoxelObjectAsset, VoxelObjectError> {
    let stated_content_hash = std::mem::take(&mut object.content_hash);
    let stated_default_hash = std::mem::take(&mut object.default_frame.voxel_data_hash);
    let stated_clip_hashes = object
        .clips
        .iter_mut()
        .map(|clip| {
            clip.frames
                .iter_mut()
                .map(|frame| std::mem::take(&mut frame.frame.voxel_data_hash))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&object).expect("voxel object serializes");
    object.content_hash = format!("sha256:{:x}", Sha256::digest(bytes));
    populate_frame_hashes(&mut object);

    let mut diagnostics = Vec::new();
    if stated_content_hash != object.content_hash {
        diagnostics.push(diagnostic(
            "voxelObject.contentHashMismatch",
            "contentHash",
            "contentHash does not match the canonical semantic object",
        ));
    }
    let mut frame_mismatch = |path: String| {
        diagnostics.push(diagnostic(
            "voxelFrame.voxelDataHashMismatch",
            path,
            "voxelDataHash does not match canonical sparse occupancy",
        ));
    };
    if stated_default_hash != object.default_frame.voxel_data_hash {
        frame_mismatch("defaultFrame.voxelDataHash".to_owned());
    }
    for (clip_index, (clip, stated)) in object.clips.iter().zip(&stated_clip_hashes).enumerate() {
        for (frame_index, (frame, stated)) in clip.frames.iter().zip(stated).enumerate() {
            if *stated != frame.frame.voxel_data_hash {
                frame_mismatch(format!(
                    "clips[{clip_index}].frames[{frame_index}].frame.voxelDataHash"
                ));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(object)
    } else {
        Err(VoxelObjectError { diagnostics })
    }
}

/// Serialize an object as it is. Every object the Engine holds came from
/// [`decode_voxel_object`] or [`with_computed_voxel_object_hashes`], which
/// already checked and canonicalized it and computed its identities.
pub fn encode_voxel_object(object: &VoxelObjectAsset) -> Result<String, VoxelObjectError> {
    let mut encoded = serde_json::to_string_pretty(object)
        .map_err(|error| VoxelObjectError::one("voxelObject.encode", "$", error.to_string()))?;
    encoded.push('\n');
    Ok(encoded)
}

/// Canonicalize the object and compute its frame and content identities.
///
/// This is where a voxel object's identities change. Decode verifies a file's
/// stated identities once; encode, runtime admission and queries reuse the
/// stored identities rather than recomputing them.
pub fn with_computed_voxel_object_hashes(
    mut object: VoxelObjectAsset,
) -> Result<VoxelObjectAsset, VoxelObjectError> {
    clear_hashes(&mut object);
    let diagnostics = semantic_diagnostics(&object);
    if !diagnostics.is_empty() {
        return Err(VoxelObjectError { diagnostics });
    }
    canonicalize(&mut object);
    let bytes = serde_json::to_vec(&object).expect("voxel object serializes");
    object.content_hash = format!("sha256:{:x}", Sha256::digest(bytes));
    populate_frame_hashes(&mut object);
    Ok(object)
}

/// Check an object's structure and the shape of its stored identities.
///
/// The identities themselves are not recomputed: decode verifies them once
/// where a file enters, and [`with_computed_voxel_object_hashes`] computes
/// them when content changes.
pub fn validate_voxel_object(object: &VoxelObjectAsset) -> Result<(), VoxelObjectError> {
    let mut diagnostics = semantic_diagnostics(object);
    validate_frame_hash_shape(&object.default_frame, "defaultFrame", &mut diagnostics);
    for (clip_index, clip) in object.clips.iter().enumerate() {
        for (frame_index, frame) in clip.frames.iter().enumerate() {
            validate_frame_hash_shape(
                &frame.frame,
                &format!("clips[{clip_index}].frames[{frame_index}].frame"),
                &mut diagnostics,
            );
        }
    }
    if !crate::codec::valid_sha256(&object.content_hash) {
        diagnostics.push(diagnostic(
            "voxelObject.contentHashMismatch",
            "contentHash",
            "contentHash must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VoxelObjectError { diagnostics })
    }
}

fn semantic_diagnostics(object: &VoxelObjectAsset) -> Vec<VoxelObjectDiagnostic> {
    let mut diagnostics = Vec::new();
    if object.schema_version != VOXEL_OBJECT_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "voxelObject.unsupportedSchema",
            "schemaVersion",
            format!(
                "expected schema {VOXEL_OBJECT_SCHEMA_VERSION}, found {}",
                object.schema_version
            ),
        ));
    }
    validate_asset_identity(
        &object.asset_id,
        AssetKind::VoxelObject,
        "voxelObject.invalidAssetId",
        "assetId",
        &mut diagnostics,
    );
    validate_grid(object, &mut diagnostics);
    let material_slots = validate_materials(
        &object.material_palette,
        &object.material_map,
        &mut diagnostics,
    );
    append_frame_semantics(
        &object.default_frame,
        "defaultFrame",
        &material_slots,
        &mut diagnostics,
    );
    validate_provenance(&object.provenance, &mut diagnostics);

    let mut clip_ids = BTreeSet::new();
    let mut total_voxels = represented_voxel_count(&object.default_frame);
    let mut union_bounds = object.default_frame.bounds;
    for (clip_index, clip) in object.clips.iter().enumerate() {
        let clip_path = format!("clips[{clip_index}]");
        if !valid_clip_id(&clip.id) || !clip_ids.insert(clip.id.as_str()) {
            diagnostics.push(diagnostic(
                "voxelObject.invalidClipId",
                format!("{clip_path}.id"),
                "clip ids must be unique scoped kebab-case values",
            ));
        }
        if let Some(name) = &clip.name {
            validate_string(name, format!("{clip_path}.name"), &mut diagnostics);
        }
        if !valid_positive(clip.frames_per_second) {
            diagnostics.push(diagnostic(
                "voxelObject.invalidFrameRate",
                format!("{clip_path}.framesPerSecond"),
                "framesPerSecond must be finite and greater than zero",
            ));
        }
        if clip.frames.is_empty() {
            diagnostics.push(diagnostic(
                "voxelObject.invalidClip",
                format!("{clip_path}.frames"),
                "each clip must contain at least one frame",
            ));
        }
        for (frame_index, animation_frame) in clip.frames.iter().enumerate() {
            let path = format!("{clip_path}.frames[{frame_index}]");
            if animation_frame
                .duration_seconds
                .is_some_and(|duration| !valid_positive(duration))
            {
                diagnostics.push(diagnostic(
                    "voxelObject.invalidFrameDuration",
                    format!("{path}.durationSeconds"),
                    "durationSeconds must be finite and greater than zero",
                ));
            }
            validate_frame_metadata(animation_frame, &path, &mut diagnostics);
            append_frame_semantics(
                &animation_frame.frame,
                &format!("{path}.frame"),
                &material_slots,
                &mut diagnostics,
            );
            total_voxels =
                total_voxels.saturating_add(represented_voxel_count(&animation_frame.frame));
            union_bounds = union(union_bounds, animation_frame.frame.bounds);
        }
    }
    if total_voxels > MAX_VOXEL_OBJECT_TOTAL_VOXELS {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "clips",
            format!(
                "object frames represent {total_voxels} aggregate voxels; limit is {MAX_VOXEL_OBJECT_TOTAL_VOXELS}"
            ),
        ));
    }
    if object.bounds != union_bounds {
        diagnostics.push(diagnostic(
            "voxelObject.invalidBounds",
            "bounds",
            format!(
                "declared union bounds {:?} do not equal frame union {:?}",
                object.bounds, union_bounds
            ),
        ));
    }
    if let Some(default_clip) = &object.default_clip {
        if !clip_ids.contains(default_clip.as_str()) {
            diagnostics.push(diagnostic(
                "voxelObject.missingDefaultClip",
                "defaultClip",
                format!("default clip `{default_clip}` is not present in clips"),
            ));
        }
    }
    let provenance_clip_ids = object
        .provenance
        .source_clips
        .iter()
        .map(|clip| clip.output_clip_id.as_str())
        .collect::<BTreeSet<_>>();
    match object.provenance.kind {
        VoxelObjectProvenanceKind::ConvertedAnimatedMesh
            if !provenance_clip_ids.is_empty() && provenance_clip_ids != clip_ids =>
        {
            diagnostics.push(diagnostic(
                "voxelObject.invalidProvenance",
                "provenance.sourceClips",
                "animated conversion provenance must name every output clip exactly once",
            ));
        }
        VoxelObjectProvenanceKind::Authored | VoxelObjectProvenanceKind::ConvertedStaticMesh
            if !object.provenance.source_clips.is_empty() =>
        {
            diagnostics.push(diagnostic(
                "voxelObject.invalidProvenance",
                "provenance.sourceClips",
                "only converted animated meshes may carry source clip schedules",
            ));
        }
        _ => {}
    }
    diagnostics
}

fn validate_grid(object: &VoxelObjectAsset, diagnostics: &mut Vec<VoxelObjectDiagnostic>) {
    if !object.grid.cell_size.is_finite() || object.grid.cell_size <= 0.0 {
        diagnostics.push(diagnostic(
            "voxelObject.invalidGrid",
            "grid.cellSize",
            "cellSize must be finite and greater than zero",
        ));
    }
    if object
        .grid
        .pivot
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_LOCAL_COORDINATE_ABS)
    {
        diagnostics.push(diagnostic(
            "voxelObject.invalidGrid",
            "grid.pivot",
            format!(
                "pivot components must be finite and stay within +/-{MAX_VOXEL_COORDINATE_ABS} cells"
            ),
        ));
    }
}

fn validate_frame_metadata(
    frame: &crate::VoxelObjectAnimationFrame,
    path: &str,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    let mut anchor_ids = BTreeSet::new();
    for (index, anchor) in frame.anchors.iter().enumerate() {
        let anchor_path = format!("{path}.anchors[{index}]");
        if !valid_frame_fact_id(&anchor.id) || !anchor_ids.insert(anchor.id.as_str()) {
            diagnostics.push(diagnostic(
                "voxelObject.invalidFrameAnchor",
                format!("{anchor_path}.id"),
                "anchor ids must be unique lowercase authored identities",
            ));
        }
        validate_local_point(
            anchor.position,
            format!("{anchor_path}.position"),
            diagnostics,
        );
    }
    if let Some(collision) = &frame.collision {
        validate_frame_collision(collision, path, diagnostics);
    }
}

fn validate_frame_collision(
    collision: &VoxelObjectFrameCollision,
    path: &str,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    if collision.body.is_none() && collision.hit_regions.is_empty() {
        diagnostics.push(diagnostic(
            "voxelObject.invalidFrameCollision",
            format!("{path}.collision"),
            "collision metadata must contain a body or at least one hit region",
        ));
    }
    if let Some(body) = &collision.body {
        validate_collision_primitive(body, &format!("{path}.collision.body"), diagnostics);
    }
    let mut region_ids = BTreeSet::new();
    for (index, region) in collision.hit_regions.iter().enumerate() {
        let region_path = format!("{path}.collision.hitRegions[{index}]");
        if !valid_frame_fact_id(&region.id) || !region_ids.insert(region.id.as_str()) {
            diagnostics.push(diagnostic(
                "voxelObject.invalidHitRegion",
                format!("{region_path}.id"),
                "hit region ids must be unique lowercase authored identities",
            ));
        }
        validate_collision_primitive(&region.primitive, &region_path, diagnostics);
    }
}

fn validate_collision_primitive(
    primitive: &VoxelObjectCollisionPrimitive,
    path: &str,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    match primitive {
        VoxelObjectCollisionPrimitive::Box {
            center,
            half_extents,
        } => {
            validate_local_point(*center, format!("{path}.center"), diagnostics);
            if half_extents
                .iter()
                .any(|value| !valid_positive_extent(*value))
            {
                diagnostics.push(diagnostic(
                    "voxelObject.invalidCollisionPrimitive",
                    format!("{path}.halfExtents"),
                    format!(
                        "box half extents must be finite and in (0,{MAX_VOXEL_COORDINATE_ABS}]"
                    ),
                ));
            }
        }
        VoxelObjectCollisionPrimitive::Capsule {
            center,
            radius,
            half_height,
        } => {
            validate_local_point(*center, format!("{path}.center"), diagnostics);
            if !valid_positive_extent(*radius) {
                diagnostics.push(diagnostic(
                    "voxelObject.invalidCollisionPrimitive",
                    format!("{path}.radius"),
                    format!("capsule radius must be finite and in (0,{MAX_VOXEL_COORDINATE_ABS}]"),
                ));
            }
            if !valid_positive_extent(*half_height) {
                diagnostics.push(diagnostic(
                    "voxelObject.invalidCollisionPrimitive",
                    format!("{path}.halfHeight"),
                    format!(
                        "capsule halfHeight must be finite and in (0,{MAX_VOXEL_COORDINATE_ABS}]"
                    ),
                ));
            }
        }
    }
}

fn validate_local_point(
    point: [f64; 3],
    path: impl Into<String>,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    if point
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_LOCAL_COORDINATE_ABS)
    {
        diagnostics.push(diagnostic(
            "voxelObject.invalidLocalPoint",
            path,
            format!(
                "local point components must be finite and stay within +/-{MAX_VOXEL_COORDINATE_ABS} cells"
            ),
        ));
    }
}

fn valid_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn valid_positive_extent(value: f64) -> bool {
    valid_positive(value) && value <= MAX_LOCAL_COORDINATE_ABS
}

fn validate_materials(
    palette: &[VoxelAssetMaterialBinding],
    mappings: &[VoxelAssetMaterialMapping],
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) -> BTreeSet<u16> {
    if palette.is_empty() {
        diagnostics.push(diagnostic(
            "voxelObject.invalidMaterialBinding",
            "materialPalette",
            "materialPalette must not be empty",
        ));
    }
    let mut palette_slots = BTreeSet::new();
    for (index, binding) in palette.iter().enumerate() {
        if binding.material_slot == 0 || !palette_slots.insert(binding.material_slot) {
            diagnostics.push(diagnostic(
                "voxelObject.duplicateMaterialBinding",
                format!("materialPalette[{index}].materialSlot"),
                "material slots must be unique and nonzero",
            ));
        }
        validate_asset_identity(
            &binding.material_asset_id,
            AssetKind::Material,
            "voxelObject.invalidMaterialReference",
            format!("materialPalette[{index}].materialAssetId"),
            diagnostics,
        );
        if let Some(name) = &binding.display_name {
            validate_string(
                name,
                format!("materialPalette[{index}].displayName"),
                diagnostics,
            );
        }
    }
    if mappings.is_empty() {
        diagnostics.push(diagnostic(
            "voxelObject.invalidMaterialMapping",
            "materialMap",
            "materialMap must not be empty",
        ));
    }
    let mut source_slots = BTreeSet::new();
    for (index, mapping) in mappings.iter().enumerate() {
        if !source_slots.insert(mapping.source_material_slot) {
            diagnostics.push(diagnostic(
                "voxelObject.duplicateMaterialMapping",
                format!("materialMap[{index}].sourceMaterialSlot"),
                "source material slots must be unique",
            ));
        }
        if !palette_slots.contains(&mapping.voxel_material_slot) {
            diagnostics.push(diagnostic(
                "voxelObject.unknownMaterial",
                format!("materialMap[{index}].voxelMaterialSlot"),
                "mapped voxel material has no materialPalette binding",
            ));
        }
        if let Some(name) = &mapping.source_material_name {
            validate_string(
                name,
                format!("materialMap[{index}].sourceMaterialName"),
                diagnostics,
            );
        }
    }
    palette_slots
}

fn validate_provenance(
    provenance: &VoxelObjectProvenance,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    validate_string(
        &provenance.source_path,
        "provenance.sourcePath",
        diagnostics,
    );
    validate_string(&provenance.converter, "provenance.converter", diagnostics);
    if let Some(path) = &provenance.license_path {
        validate_string(path, "provenance.licensePath", diagnostics);
    }
    if !crate::codec::valid_sha256(&provenance.source_sha256) {
        diagnostics.push(diagnostic(
            "voxelObject.invalidProvenance",
            "provenance.sourceSha256",
            "sourceSha256 must be a canonical SHA-256 identity",
        ));
    }
    if !crate::codec::valid_sha256(&provenance.settings_sha256) {
        diagnostics.push(diagnostic(
            "voxelObject.invalidProvenance",
            "provenance.settingsSha256",
            "settingsSha256 must be a canonical SHA-256 identity",
        ));
    }
    if provenance.source_byte_count == 0 {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "provenance.sourceByteCount",
            "sourceByteCount must be nonzero",
        ));
    }
    let mut output_ids = BTreeSet::new();
    for (index, clip) in provenance.source_clips.iter().enumerate() {
        let path = format!("provenance.sourceClips[{index}]");
        if !valid_clip_id(&clip.output_clip_id) || !output_ids.insert(clip.output_clip_id.as_str())
        {
            diagnostics.push(diagnostic(
                "voxelObject.invalidProvenance",
                format!("{path}.outputClipId"),
                "output clip ids must be unique scoped kebab-case values",
            ));
        }
        validate_string(
            &clip.source_clip_name,
            format!("{path}.sourceClipName"),
            diagnostics,
        );
        if clip.start_microseconds > clip.end_microseconds {
            diagnostics.push(diagnostic(
                "voxelObject.invalidProvenance",
                format!("{path}.startMicroseconds"),
                "source clip range must be ordered",
            ));
        }
        if clip.sample_rate_hz == 0 {
            diagnostics.push(diagnostic(
                "voxelObject.invalidProvenance",
                format!("{path}.sampleRateHz"),
                "sampleRateHz must be nonzero",
            ));
        }
    }
}

fn append_frame_semantics(
    frame: &crate::VoxelFrame,
    path: &str,
    material_slots: &BTreeSet<u16>,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    diagnostics.extend(
        crate::frame::semantic_diagnostics(frame, material_slots)
            .into_iter()
            .map(|item| diagnostic(item.code, prefixed(path, &item.path), item.message)),
    );
}

fn validate_frame_hash_shape(
    frame: &crate::VoxelFrame,
    path: &str,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    if !crate::codec::valid_sha256(&frame.voxel_data_hash) {
        diagnostics.push(diagnostic(
            "voxelFrame.voxelDataHashMismatch",
            prefixed(path, "voxelDataHash"),
            "voxelDataHash must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    }
}

fn canonicalize(object: &mut VoxelObjectAsset) {
    object.material_palette.sort_by(|left, right| {
        (
            left.material_slot,
            &left.material_asset_id,
            &left.display_name,
        )
            .cmp(&(
                right.material_slot,
                &right.material_asset_id,
                &right.display_name,
            ))
    });
    object.material_map.sort_by(|left, right| {
        (
            left.source_material_slot,
            left.voxel_material_slot,
            &left.source_material_name,
        )
            .cmp(&(
                right.source_material_slot,
                right.voxel_material_slot,
                &right.source_material_name,
            ))
    });
    object.clips.sort_by(|left, right| left.id.cmp(&right.id));
    object
        .provenance
        .source_clips
        .sort_by(|left, right| left.output_clip_id.cmp(&right.output_clip_id));
    canonicalize_frame(&mut object.default_frame);
    for clip in &mut object.clips {
        for frame in &mut clip.frames {
            frame.anchors.sort_by(|left, right| left.id.cmp(&right.id));
            if let Some(collision) = &mut frame.collision {
                collision
                    .hit_regions
                    .sort_by(|left, right| left.id.cmp(&right.id));
            }
            canonicalize_frame(&mut frame.frame);
        }
    }
}

fn clear_hashes(object: &mut VoxelObjectAsset) {
    object.content_hash.clear();
    object.default_frame.voxel_data_hash.clear();
    for clip in &mut object.clips {
        for frame in &mut clip.frames {
            frame.frame.voxel_data_hash.clear();
        }
    }
}

fn populate_frame_hashes(object: &mut VoxelObjectAsset) {
    let hash = |frame: &crate::VoxelFrame| {
        crate::frame::canonical_runs_hash(&frame.representation.sparse_runs)
    };
    object.default_frame.voxel_data_hash = hash(&object.default_frame);
    for clip in &mut object.clips {
        for frame in &mut clip.frames {
            frame.frame.voxel_data_hash = hash(&frame.frame);
        }
    }
}

fn union(left: VoxelAssetBounds, right: VoxelAssetBounds) -> VoxelAssetBounds {
    VoxelAssetBounds {
        min: std::array::from_fn(|axis| left.min[axis].min(right.min[axis])),
        max: std::array::from_fn(|axis| left.max[axis].max(right.max[axis])),
    }
}

fn valid_clip_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && !segment.contains("--")
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_frame_fact_id(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with(['-', '_', '/'])
        && !value.ends_with(['-', '_', '/'])
        && !value.contains("//")
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'/')
        })
}

fn validate_asset_identity(
    value: &str,
    expected_kind: AssetKind,
    code: &'static str,
    path: impl Into<String>,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    let path = path.into();
    match AssetId::parse(value) {
        Ok(id) if id.kind() == expected_kind => {}
        Ok(id) => diagnostics.push(diagnostic(
            code,
            path,
            format!("expected {expected_kind} identity, found {}", id.kind()),
        )),
        Err(error) => diagnostics.push(diagnostic(code, path, error.to_string())),
    }
}

fn validate_string(
    value: &str,
    path: impl Into<String>,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    if value.trim().is_empty() {
        diagnostics.push(diagnostic(
            "voxelObject.invalidString",
            path,
            "value must not be empty",
        ));
    }
}

fn prefixed(prefix: &str, path: &str) -> String {
    if path.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}.{path}")
    }
}

fn diagnostic(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> VoxelObjectDiagnostic {
    VoxelObjectDiagnostic {
        code,
        path: path.into(),
        message: message.into(),
    }
}

fn json_path(path: &str) -> String {
    if path.is_empty() {
        "$".to_string()
    } else {
        path.to_string()
    }
}
