use std::collections::BTreeSet;

use core_assets::{AssetId, AssetKind};
use sha2::{Digest, Sha256};

use crate::{
    frame::{canonicalize_frame, represented_voxel_count},
    validate_voxel_frame, VoxelAssetBounds, VoxelAssetMaterialBinding, VoxelAssetMaterialMapping,
    VoxelObjectAsset, VoxelObjectProvenance, VOXEL_OBJECT_SCHEMA_VERSION,
};

pub const MAX_VOXEL_OBJECT_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_VOXEL_OBJECT_CLIPS: usize = 256;
pub const MAX_VOXEL_OBJECT_FRAMES_PER_CLIP: usize = 4_096;
pub const MAX_VOXEL_OBJECT_TOTAL_FRAMES: usize = 8_192;
pub const MAX_VOXEL_OBJECT_TOTAL_VOXELS: usize = 16_777_216;
pub const MAX_VOXEL_OBJECT_FRAMES_PER_SECOND: f64 = 240.0;
pub const MAX_VOXEL_OBJECT_FRAME_DURATION_SECONDS: f64 = 60.0;

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
    if input.len() > MAX_VOXEL_OBJECT_ARTIFACT_BYTES {
        return Err(VoxelObjectError::one(
            "voxelObject.resourceLimit",
            "$",
            format!(
                "artifact has {} bytes; limit is {MAX_VOXEL_OBJECT_ARTIFACT_BYTES}",
                input.len()
            ),
        ));
    }
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
    Ok(canonical)
}

pub fn encode_voxel_object(object: &VoxelObjectAsset) -> Result<String, VoxelObjectError> {
    validate_voxel_object(object)?;
    let mut canonical = object.clone();
    canonicalize(&mut canonical);
    let mut encoded = serde_json::to_string_pretty(&canonical)
        .map_err(|error| VoxelObjectError::one("voxelObject.encode", "$", error.to_string()))?;
    encoded.push('\n');
    if encoded.len() > MAX_VOXEL_OBJECT_ARTIFACT_BYTES {
        return Err(VoxelObjectError::one(
            "voxelObject.resourceLimit",
            "$",
            format!(
                "encoded artifact has {} bytes; limit is {MAX_VOXEL_OBJECT_ARTIFACT_BYTES}",
                encoded.len()
            ),
        ));
    }
    Ok(encoded)
}

pub fn canonicalize_voxel_object(
    object: &VoxelObjectAsset,
) -> Result<VoxelObjectAsset, VoxelObjectError> {
    validate_voxel_object(object)?;
    let mut canonical = object.clone();
    canonicalize(&mut canonical);
    Ok(canonical)
}

pub fn with_computed_voxel_object_hashes(
    mut object: VoxelObjectAsset,
) -> Result<VoxelObjectAsset, VoxelObjectError> {
    clear_hashes(&mut object);
    let diagnostics = semantic_diagnostics(&object);
    if !diagnostics.is_empty() {
        return Err(VoxelObjectError { diagnostics });
    }
    canonicalize(&mut object);
    populate_frame_hashes(&mut object);
    object.content_hash = computed_content_hash(&object);
    validate_voxel_object(&object)?;
    Ok(object)
}

pub fn validate_voxel_object(object: &VoxelObjectAsset) -> Result<(), VoxelObjectError> {
    let mut diagnostics = semantic_diagnostics(object);
    validate_frame_hash(
        &object.default_frame,
        "defaultFrame",
        &object.material_palette,
        &mut diagnostics,
    );
    for (clip_index, clip) in object.clips.iter().enumerate() {
        for (frame_index, frame) in clip.frames.iter().enumerate() {
            validate_frame_hash(
                &frame.frame,
                &format!("clips[{clip_index}].frames[{frame_index}].frame"),
                &object.material_palette,
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
    } else if object.content_hash != computed_content_hash(object) {
        diagnostics.push(diagnostic(
            "voxelObject.contentHashMismatch",
            "contentHash",
            "contentHash does not match the canonical semantic object",
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

    if object.clips.len() > MAX_VOXEL_OBJECT_CLIPS {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "clips",
            format!("clip count must be 0..={MAX_VOXEL_OBJECT_CLIPS}"),
        ));
    }
    let mut clip_ids = BTreeSet::new();
    let mut total_frames = 0usize;
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
        if !clip.frames_per_second.is_finite()
            || !(f64::EPSILON..=MAX_VOXEL_OBJECT_FRAMES_PER_SECOND)
                .contains(&clip.frames_per_second)
        {
            diagnostics.push(diagnostic(
                "voxelObject.invalidFrameRate",
                format!("{clip_path}.framesPerSecond"),
                format!(
                    "framesPerSecond must be finite and in (0,{MAX_VOXEL_OBJECT_FRAMES_PER_SECOND}]"
                ),
            ));
        }
        if clip.frames.is_empty() || clip.frames.len() > MAX_VOXEL_OBJECT_FRAMES_PER_CLIP {
            diagnostics.push(diagnostic(
                "voxelObject.resourceLimit",
                format!("{clip_path}.frames"),
                format!("each clip must contain 1..={MAX_VOXEL_OBJECT_FRAMES_PER_CLIP} frames"),
            ));
        }
        total_frames = total_frames.saturating_add(clip.frames.len());
        for (frame_index, animation_frame) in clip.frames.iter().enumerate() {
            let path = format!("{clip_path}.frames[{frame_index}]");
            if animation_frame.duration_seconds.is_some_and(|duration| {
                !duration.is_finite()
                    || !(f64::EPSILON..=MAX_VOXEL_OBJECT_FRAME_DURATION_SECONDS).contains(&duration)
            }) {
                diagnostics.push(diagnostic(
                    "voxelObject.invalidFrameDuration",
                    format!("{path}.durationSeconds"),
                    format!(
                        "durationSeconds must be finite and in (0,{MAX_VOXEL_OBJECT_FRAME_DURATION_SECONDS}]"
                    ),
                ));
            }
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
    if total_frames > MAX_VOXEL_OBJECT_TOTAL_FRAMES {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "clips",
            format!(
                "object contains {total_frames} animation frames; limit is {MAX_VOXEL_OBJECT_TOTAL_FRAMES}"
            ),
        ));
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
    if !(1..=64).contains(&object.grid.chunk_size) {
        diagnostics.push(diagnostic(
            "voxelObject.invalidGrid",
            "grid.chunkSize",
            "chunkSize must be in 1..=64",
        ));
    }
    if object
        .grid
        .pivot
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        diagnostics.push(diagnostic(
            "voxelObject.invalidGrid",
            "grid.pivot",
            "pivot components must be finite and stay within +/-1,000,000 cells",
        ));
    }
}

fn validate_materials(
    palette: &[VoxelAssetMaterialBinding],
    mappings: &[VoxelAssetMaterialMapping],
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) -> BTreeSet<u16> {
    if palette.is_empty() || palette.len() > crate::MAX_MATERIAL_MAPPINGS {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "materialPalette",
            format!(
                "materialPalette must contain 1..={} entries",
                crate::MAX_MATERIAL_MAPPINGS
            ),
        ));
    }
    let mut palette_slots = BTreeSet::new();
    for (index, binding) in palette.iter().enumerate() {
        if !(1..=4_095).contains(&binding.material_slot)
            || !palette_slots.insert(binding.material_slot)
        {
            diagnostics.push(diagnostic(
                "voxelObject.duplicateMaterialBinding",
                format!("materialPalette[{index}].materialSlot"),
                "material slots must be unique and in 1..=4095",
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
    if mappings.is_empty() || mappings.len() > crate::MAX_MATERIAL_MAPPINGS {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "materialMap",
            format!(
                "materialMap must contain 1..={} entries",
                crate::MAX_MATERIAL_MAPPINGS
            ),
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
    if provenance.source_byte_count == 0
        || provenance.source_byte_count > crate::MAX_CONVERSION_SOURCE_BYTES
    {
        diagnostics.push(diagnostic(
            "voxelObject.resourceLimit",
            "provenance.sourceByteCount",
            format!(
                "sourceByteCount must be in 1..={}",
                crate::MAX_CONVERSION_SOURCE_BYTES
            ),
        ));
    }
}

fn append_frame_semantics(
    frame: &crate::VoxelFrame,
    path: &str,
    material_slots: &BTreeSet<u16>,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    let mut without_hash = frame.clone();
    without_hash.voxel_data_hash = crate::frame::computed_voxel_data_hash(&without_hash);
    if let Err(error) = validate_voxel_frame(&without_hash, material_slots.iter().copied()) {
        diagnostics.extend(
            error.diagnostics().iter().map(|item| {
                diagnostic(item.code, prefixed(path, &item.path), item.message.clone())
            }),
        );
    }
}

fn validate_frame_hash(
    frame: &crate::VoxelFrame,
    path: &str,
    palette: &[VoxelAssetMaterialBinding],
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    if let Err(error) =
        validate_voxel_frame(frame, palette.iter().map(|binding| binding.material_slot))
    {
        diagnostics.extend(
            error.diagnostics().iter().map(|item| {
                diagnostic(item.code, prefixed(path, &item.path), item.message.clone())
            }),
        );
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
    canonicalize_frame(&mut object.default_frame);
    for clip in &mut object.clips {
        for frame in &mut clip.frames {
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
    object.default_frame.voxel_data_hash =
        crate::frame::computed_voxel_data_hash(&object.default_frame);
    for clip in &mut object.clips {
        for frame in &mut clip.frames {
            frame.frame.voxel_data_hash = crate::frame::computed_voxel_data_hash(&frame.frame);
        }
    }
}

fn computed_content_hash(object: &VoxelObjectAsset) -> String {
    let mut canonical = object.clone();
    clear_hashes(&mut canonical);
    canonicalize(&mut canonical);
    let bytes = serde_json::to_vec(&canonical).expect("voxel object serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn union(left: VoxelAssetBounds, right: VoxelAssetBounds) -> VoxelAssetBounds {
    VoxelAssetBounds {
        min: std::array::from_fn(|axis| left.min[axis].min(right.min[axis])),
        max: std::array::from_fn(|axis| left.max[axis].max(right.max[axis])),
    }
}

fn valid_clip_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= crate::MAX_STRING_BYTES
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

fn validate_asset_identity(
    value: &str,
    expected_kind: AssetKind,
    code: &'static str,
    path: impl Into<String>,
    diagnostics: &mut Vec<VoxelObjectDiagnostic>,
) {
    let path = path.into();
    if value.len() > crate::MAX_STRING_BYTES {
        diagnostics.push(diagnostic(
            code,
            path,
            format!(
                "identity has {} UTF-8 bytes; limit is {}",
                value.len(),
                crate::MAX_STRING_BYTES
            ),
        ));
        return;
    }
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
    if value.trim().is_empty() || value.len() > crate::MAX_STRING_BYTES {
        diagnostics.push(diagnostic(
            "voxelObject.invalidString",
            path,
            format!(
                "value must contain 1..={} UTF-8 bytes",
                crate::MAX_STRING_BYTES
            ),
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
