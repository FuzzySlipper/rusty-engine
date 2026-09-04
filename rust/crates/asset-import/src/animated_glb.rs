use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use asset_catalog::{validate_catalog, AssetCatalog, CatalogEntry};
use core_assets::{AssetHash, AssetId, AssetKind};
use gltf::buffer::Source as BufferSource;
use gltf::image::Source as ImageSource;
use render_model::{
    animation_rig_fingerprint, AnimatedMeshAsset, AnimatedMeshEmbeddedMaterialSlot,
    AnimatedMeshRuntimeFormat, AnimationBindRestConvention, AnimationClipDescriptor,
    AnimationRigFingerprintJoint, AnimationRigJoint, AnimationRigSignature,
    AnimationRootConvention, MeshBoundsDescriptor,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voxel_convert::{
    import_animated_mesh_source_for_visual_metadata, import_mesh_source, AnimationChannelValues,
    AnimationProperty, MeshSourceFormat, MeshSourceImportRequest, MAX_CONVERSION_SOURCE_BYTES,
};

use crate::{
    gltf_package::glb_json_document, ImportCode, ImportContext, ImportDiagnostic, SourceUri,
};

pub const SUPPORTED_ANIMATED_GLB_VERSION: u32 = 2;
pub const MAX_ANIMATED_GLB_MATERIALS: usize = 256;
pub const MAX_ANIMATED_GLB_TEXTURES: usize = 256;
pub const MAX_ANIMATED_GLB_IMAGES: usize = 256;
pub const MAX_ANIMATED_GLB_JOINTS: usize = 4_096;
pub const MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES: usize = 32 * 1024 * 1024;
/// Three's admitted animated-mesh path realizes TEXCOORD_0 through TEXCOORD_3.
pub const MAX_ANIMATED_GLB_TEXTURE_COORD_SET: u64 = 3;
/// Keeps authored UV transforms finite and prevents extreme values from
/// crossing the retained renderer boundary. Negative offset, rotation, and
/// scale remain valid within this absolute bound.
pub const MAX_ANIMATED_GLB_TEXTURE_TRANSFORM_COMPONENT: f64 = 1_000_000.0;

/// The admitted GLB's embedded animation classification. Both variants retain
/// the existing GLB mesh resource and `AnimatedMeshAsset` wire lifecycle; this
/// readout avoids describing a zero-clip resource as animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlbAnimationKind {
    Static,
    Animated,
}

impl GlbAnimationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Static => "staticGlb",
            Self::Animated => "animatedGlb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedGlbImportReceipt {
    /// `Static` means the admitted GLB has zero embedded clips.
    pub animation_kind: GlbAnimationKind,
    pub source_hash: String,
    pub source_byte_count: u64,
    pub node_count: u32,
    pub mesh_count: u32,
    pub primitive_count: u32,
    pub material_count: u32,
    pub texture_count: u32,
    pub image_count: u32,
    pub skin_count: u32,
    pub joint_count: u32,
    pub clip_count: u32,
    pub channel_count: u32,
    pub keyframe_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimatedGlb {
    pub animated_mesh: AnimatedMeshAsset,
    pub catalog: AssetCatalog,
    pub runtime_resource_path: String,
    pub runtime_resource_bytes: Vec<u8>,
    pub receipt: AnimatedGlbImportReceipt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedGlbImportOutcome {
    pub assets: Option<ImportedAnimatedGlb>,
    pub diagnostics: Vec<ImportDiagnostic>,
}

impl AnimatedGlbImportOutcome {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(ImportDiagnostic::is_error)
    }
}

pub fn import_animated_glb_asset(
    source_uri: &SourceUri,
    source: &[u8],
    context: &ImportContext,
) -> AnimatedGlbImportOutcome {
    let mut diagnostics = Vec::new();
    // Animated mesh IDs are content identities, not filename-derived labels.
    // Hashing before source-name inspection means a source can retain its
    // authored spelling (including spaces or uppercase characters) without
    // weakening the canonical AssetId grammar.
    let source_hash_hex = sha256_hex(source);
    let Some((asset_id, name, runtime_resource_path)) =
        source_identity(source_uri, &source_hash_hex, &mut diagnostics)
    else {
        return failed(diagnostics);
    };
    if !context.settings.is_valid()
        || context.settings.scale != 1.0
        || context.settings.generate_collision
        || context.settings.material_namespace.is_some()
    {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::InvalidImportSettings,
            asset_id.as_str(),
            "animated GLB import retains exact source bytes and requires scale=1, collision disabled, and no material namespace",
            "apply scale on authored instances and keep GLB-owned materials/collision policy outside the import",
        ));
        return failed(diagnostics);
    }
    let parsed = match parse_and_preflight(source, source_uri.value()) {
        Ok(parsed) => parsed,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return failed(diagnostics);
        }
    };
    let source_hash = format!("sha256:{source_hash_hex}");
    let animated_request = MeshSourceImportRequest {
        source_asset_id: asset_id.as_str().to_owned(),
        asset_version: 1,
        source_path: source_uri.value().to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes: source.to_vec(),
        expected_source_sha256: Some(source_hash.clone()),
        mesh_primitive: None,
    };
    let (
        animation_kind,
        source_bounds,
        source_material_slots,
        clips,
        rig,
        channel_count,
        keyframe_count,
    ) = if parsed.document.animations().next().is_some() {
        let imported = match import_animated_mesh_source_for_visual_metadata(&animated_request) {
            Ok(imported) => imported,
            Err(error) => {
                diagnostics.extend(error.diagnostics().iter().map(|item| {
                    ImportDiagnostic::error(
                        map_conversion_code(item.code, &item.message),
                        item.path.clone(),
                        item.message.clone(),
                        conversion_remedy(item.code),
                    )
                }));
                return failed(diagnostics);
            }
        };
        let clips = imported
            .model
            .clips
            .iter()
            .map(|clip| AnimationClipDescriptor {
                id: clip.name.clone(),
                name: Some(clip.name.clone()),
                duration_seconds: Some(
                    clip.duration_microseconds as f32
                        / voxel_convert::ANIMATION_TIMESTAMP_TICKS_PER_SECOND as f32,
                ),
            })
            .collect::<Vec<_>>();
        let channel_count = imported
            .model
            .clips
            .iter()
            .map(|clip| clip.channels.len())
            .sum::<usize>();
        let keyframe_count = imported
            .model
            .clips
            .iter()
            .flat_map(|clip| &clip.channels)
            .map(|channel| channel.timestamps_microseconds.len() as u64)
            .sum::<u64>();
        let rig = match derive_animation_rig_signature(&imported.model) {
            Ok(rig) => rig,
            Err(message) => {
                // Embedded clips remain independently usable. We retain no
                // approximate signature, so a later clip-pack association
                // fails closed while this primary resource can still serve
                // its own decoded animations.
                diagnostics.push(ImportDiagnostic::warning(
                    ImportCode::InvalidAnimation,
                    asset_id.as_str(),
                    message,
                    "export one named skin joint forest with a supported designated root-motion policy before using this GLB as a clip-pack endpoint",
                ));
                None
            }
        };
        (
            GlbAnimationKind::Animated,
            imported.source.receipt.metadata.source_bounds,
            imported
                .source
                .receipt
                .metadata
                .material_slots
                .iter()
                .map(|material| material.source_material_slot)
                .collect::<Vec<_>>(),
            clips,
            rig,
            channel_count,
            keyframe_count,
        )
    } else {
        // `voxel-convert` retains separate static and animated source parsers.
        // This uses the existing bounded static scene parser only to validate
        // and measure a zero-clip GLB; publication remains the same GLB mesh
        // descriptor/resource lifecycle below.
        let static_request = MeshSourceImportRequest {
            source_asset_id: format!("mesh/{source_hash_hex}"),
            ..animated_request
        };
        let imported = match import_mesh_source(&static_request) {
            Ok(imported) => imported,
            Err(error) => {
                diagnostics.extend(error.diagnostics().iter().map(|item| {
                    ImportDiagnostic::error(
                        map_conversion_code(item.code, &item.message),
                        item.path.clone(),
                        item.message.clone(),
                        conversion_remedy(item.code),
                    )
                }));
                return failed(diagnostics);
            }
        };
        (
            GlbAnimationKind::Static,
            imported.receipt.metadata.source_bounds,
            imported
                .receipt
                .metadata
                .material_slots
                .iter()
                .map(|material| material.source_material_slot)
                .collect::<Vec<_>>(),
            Vec::new(),
            None,
            0,
            0,
        )
    };
    let bounds = match render_bounds(source_bounds) {
        Ok(bounds) => bounds,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return failed(diagnostics);
        }
    };
    let clip_count = clips.len();
    let embedded_material_slots =
        embedded_material_slots(source_material_slots, parsed.document.materials().count());
    let default_clip = clips
        .iter()
        .find(|clip| clip.id == "idle")
        .or_else(|| clips.first())
        .map(|clip| clip.id.clone());
    let animated_mesh = AnimatedMeshAsset {
        asset: asset_id.as_str().to_owned(),
        runtime_format: AnimatedMeshRuntimeFormat::Glb,
        content_hash: Some(source_hash.clone()),
        clips,
        rig,
        clip_packs: Vec::new(),
        default_clip,
        embedded_material_slots,
        material_slots: Vec::new(),
        bounds,
    };
    if let Err(error) = animated_mesh.validate() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::InvalidDescriptor,
            asset_id.as_str(),
            format!("generated animated mesh descriptor is invalid: {error:?}"),
            "repair source clip names, durations, transforms, or bounds",
        ));
        return failed(diagnostics);
    }
    let asset_hash = AssetHash::parse(&source_hash_hex)
        .expect("SHA-256 is a valid lowercase hexadecimal asset hash");
    let catalog = AssetCatalog::from_entries(vec![CatalogEntry::new(asset_id, 1)
        .with_hash(asset_hash)
        .with_source(runtime_resource_path.clone())
        .with_label(name)]);
    let validation = validate_catalog(&catalog);
    if !validation.is_ok() {
        diagnostics.extend(validation.diagnostics().into_iter().map(|item| {
            ImportDiagnostic::error(
                ImportCode::InvalidDescriptor,
                item.path,
                item.message,
                "repair generated animated asset identity or provenance",
            )
        }));
        return failed(diagnostics);
    }
    let primitive_count = parsed
        .document
        .meshes()
        .map(|mesh| mesh.primitives().count())
        .sum::<usize>();
    let joint_count = parsed
        .document
        .skins()
        .map(|skin| skin.joints().count())
        .sum::<usize>();
    AnimatedGlbImportOutcome {
        assets: Some(ImportedAnimatedGlb {
            animated_mesh,
            catalog: catalog.canonical(),
            runtime_resource_path,
            runtime_resource_bytes: source.to_vec(),
            receipt: AnimatedGlbImportReceipt {
                animation_kind,
                source_hash,
                source_byte_count: source.len() as u64,
                node_count: count_u32(parsed.document.nodes().count()),
                mesh_count: count_u32(parsed.document.meshes().count()),
                primitive_count: count_u32(primitive_count),
                material_count: count_u32(parsed.document.materials().count()),
                texture_count: count_u32(parsed.document.textures().count()),
                image_count: count_u32(parsed.document.images().count()),
                skin_count: count_u32(parsed.document.skins().count()),
                joint_count: count_u32(joint_count),
                clip_count: count_u32(clip_count),
                channel_count: count_u32(channel_count),
                keyframe_count,
            },
        }),
        diagnostics,
    }
}

/// Derives the exact renderer compatibility signature from the already
/// bounded voxel-convert model. Only named skin joints participate: container
/// ancestry stays out of the skeleton and is never promoted into a joint.
fn derive_animation_rig_signature(
    model: &voxel_convert::ImportedAnimatedModel,
) -> Result<Option<AnimationRigSignature>, String> {
    if model.skins.is_empty() {
        return Ok(None);
    }
    let scene_nodes = model
        .scene
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect::<BTreeMap<_, _>>();
    let mut inverse_binds = BTreeMap::<u32, [f64; 16]>::new();
    let mut joint_nodes = BTreeSet::new();
    for skin in &model.skins {
        for (node_index, inverse_bind) in skin
            .joint_node_indices
            .iter()
            .copied()
            .zip(skin.inverse_bind_matrices.iter().copied())
        {
            joint_nodes.insert(node_index);
            if let Some(prior) = inverse_binds.insert(node_index, inverse_bind) {
                if prior != inverse_bind {
                    return Err(format!(
                        "named skin joint node {node_index} has conflicting inverse-bind matrices"
                    ));
                }
            }
        }
    }
    let mut names = BTreeMap::new();
    for node_index in &joint_nodes {
        let node = scene_nodes.get(node_index).ok_or_else(|| {
            format!("skin joint node {node_index} is absent from the imported scene")
        })?;
        let name = node.source_node_name.clone().ok_or_else(|| {
            format!(
                "skin joint node {node_index} has no name; joint identities are never synthesized"
            )
        })?;
        if let Some(prior) = names.insert(name.clone(), *node_index) {
            return Err(format!(
                "named skin joints {prior} and {node_index} both use the identity `{name}`"
            ));
        }
    }
    let ids_by_node = names
        .iter()
        .map(|(name, node)| (*node, name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut fingerprint_joints = Vec::with_capacity(joint_nodes.len());
    let mut joints = Vec::with_capacity(joint_nodes.len());
    for node_index in &joint_nodes {
        let node = scene_nodes[node_index];
        let id = ids_by_node[node_index].clone();
        let parent = node
            .parent_node_index
            .filter(|parent| joint_nodes.contains(parent))
            .map(|parent| ids_by_node[&parent].clone());
        let inverse_bind = inverse_binds
            .get(node_index)
            .copied()
            .ok_or_else(|| format!("skin joint node {node_index} has no inverse-bind matrix"))?;
        fingerprint_joints.push(AnimationRigFingerprintJoint {
            id: id.clone(),
            parent: parent.clone(),
            local_rest_matrix: node.local_transform,
            inverse_bind_matrix: inverse_bind,
        });
        joints.push(AnimationRigJoint { id, parent });
    }
    joints.sort_by(|left, right| left.id.cmp(&right.id));
    let roots = joints
        .iter()
        .filter(|joint| joint.parent.is_none())
        .map(|joint| joint.id.clone())
        .collect::<Vec<_>>();
    let default_root = roots
        .first()
        .cloned()
        .ok_or_else(|| "the named skin joint forest has no structural root".to_owned())?;

    // Root motion is a product meaning, not something an importer can infer
    // from a node name or from the fact that a node happens to be a root.  A
    // single unambiguous changing root retains the historical authored-motion
    // convention.  As soon as more than one root changes (or clips disagree),
    // retain every joint translation as authored pose and leave selection to a
    // future explicit product policy.
    let mut changing_horizontal_roots = BTreeSet::new();
    let mut translated_joints = BTreeSet::new();
    let mut every_clip_has_changing_root = true;
    for clip in &model.clips {
        let mut clip_changing_roots = BTreeSet::new();
        for channel in &clip.channels {
            if channel.property != AnimationProperty::Translation {
                continue;
            }
            let Some(joint_id) = ids_by_node.get(&channel.target_node_index) else {
                // Animation channels outside a named skin joint are ordinary
                // GLB animation data, not a root-motion declaration.
                continue;
            };
            let AnimationChannelValues::Translations(values) = &channel.values else {
                return Err(format!(
                    "clip `{}` has malformed translation values",
                    clip.name
                ));
            };
            if values.is_empty()
                || values
                    .iter()
                    .any(|value| !value.iter().all(|item| item.is_finite()))
            {
                return Err(format!(
                    "clip `{}` has non-finite joint translation values",
                    clip.name
                ));
            }
            translated_joints.insert(joint_id.clone());
            if !roots.contains(joint_id) {
                // Child translation is always authored pose data. The
                // renderer validates the eventual clip pack independently;
                // deriving metadata must not reduce primary compatibility.
                continue;
            }
            let origin = values[0];
            if values.iter().any(|value| {
                (value[0] - origin[0]).abs() > 1e-6 || (value[2] - origin[2]).abs() > 1e-6
            }) {
                clip_changing_roots.insert(joint_id.clone());
            }
        }
        // A clip may contain several structural-root translations. They are
        // still valid authored pose channels; only one changing root across
        // every clip is sufficiently unambiguous for the legacy motion mode.
        every_clip_has_changing_root &= !clip_changing_roots.is_empty();
        changing_horizontal_roots.extend(clip_changing_roots);
    }
    let designated_motion_root_ids =
        if changing_horizontal_roots.len() == 1 && every_clip_has_changing_root {
            changing_horizontal_roots
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
    let designated_motion_roots = designated_motion_root_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let authored_pose_translation_joint_ids = translated_joints
        .difference(&designated_motion_roots)
        .cloned()
        .collect::<Vec<_>>();
    let root_joint_id = designated_motion_root_ids
        .first()
        .cloned()
        .unwrap_or(default_root);
    let root_convention = if designated_motion_root_ids.is_empty() {
        AnimationRootConvention::InPlace
    } else {
        AnimationRootConvention::AuthoredRootTranslation
    };
    let bind_rest_hash = animation_rig_fingerprint(&fingerprint_joints).map_err(|_| {
        "named skin joint matrices cannot produce the canonical renderer fingerprint".to_owned()
    })?;
    let signature = AnimationRigSignature {
        joints,
        bind_rest_hash,
        bind_rest_convention: AnimationBindRestConvention::LocalMatrixV1,
        root_convention,
        root_joint_id,
        structural_root_ids: roots,
        designated_motion_root_ids,
        authored_pose_translation_joint_ids,
    };
    signature.validate().map_err(|_| {
        "derived named skin joint forest is not a valid renderer rig signature".to_owned()
    })?;
    Ok(Some(signature))
}

/// Derive dense Engine-facing slots from the admitted source parser's used
/// material slots. Only explicit GLB material indices participate: primitives
/// without a `material` property use Three's default material and have no
/// source material index suitable for an Engine override slot.
fn embedded_material_slots(
    source_material_slots: Vec<u32>,
    glb_material_count: usize,
) -> Vec<AnimatedMeshEmbeddedMaterialSlot> {
    let glb_material_count =
        u32::try_from(glb_material_count).expect("animated GLB material admission bound fits u32");
    source_material_slots
        .into_iter()
        .filter(|source_material_slot| *source_material_slot < glb_material_count)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(
            |(slot, source_material_slot)| AnimatedMeshEmbeddedMaterialSlot {
                slot: u16::try_from(slot)
                    .expect("animated GLB material admission bound keeps dense slots in u16"),
                source_material_slot: u16::try_from(source_material_slot)
                    .expect("animated GLB material admission bound keeps source slots in u16"),
            },
        )
        .collect()
}

fn parse_and_preflight(source: &[u8], locus: &str) -> Result<gltf::Gltf, ImportDiagnostic> {
    if source.is_empty() || source.len() as u64 > MAX_CONVERSION_SOURCE_BYTES {
        return Err(ImportDiagnostic::error(
            ImportCode::SourceTooLarge,
            locus,
            format!(
                "animated GLB byte count {} is outside 1..={MAX_CONVERSION_SOURCE_BYTES}",
                source.len()
            ),
            "supply one bounded binary GLB source",
        ));
    }
    let json_document = glb_json_document(source, locus)?;
    for extension in extension_names(&json_document, "extensionsRequired")? {
        if !is_admitted_extension(&extension) {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                "source.extensionsRequired",
                format!("required GLB extension `{extension}` is not admitted"),
                "export core glTF 2.0 data or an admitted material extension",
            ));
        }
    }
    validate_webp_texture_extensions(&json_document)?;
    let parsed = gltf::Gltf::from_slice(source).map_err(|error| {
        ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            locus,
            format!("invalid GLB 2.0 source: {error}"),
            "export a valid binary glTF 2.0 file",
        )
    })?;
    if parsed.blob.is_none() {
        return Err(ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            locus,
            "animated import requires a GLB with one embedded BIN chunk",
            "embed all buffers and select binary glTF export",
        ));
    }
    let document = &parsed.document;
    for extension in document.extensions_used() {
        if !is_admitted_extension(extension) {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                "source.extensionsUsed",
                format!("GLB extension `{extension}` is not admitted"),
                "export core glTF 2.0 data or an admitted material extension",
            ));
        }
    }
    for extension in document.extensions_required() {
        if !is_admitted_extension(extension) {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                "source.extensionsRequired",
                format!("required GLB extension `{extension}` is not admitted"),
                "export core glTF 2.0 data or an admitted material extension",
            ));
        }
    }
    validate_texture_transforms(&json_document)?;
    for buffer in document.buffers() {
        if let BufferSource::Uri(uri) = buffer.source() {
            return Err(ImportDiagnostic::error(
                ImportCode::ExternalResource,
                format!("source.buffers[{}]", buffer.index()),
                format!("animated GLB import never resolves external buffer URI `{uri}`"),
                "embed every buffer in the GLB BIN chunk",
            ));
        }
    }
    if document.cameras().next().is_some() {
        return Err(ImportDiagnostic::error(
            ImportCode::UnsupportedFeature,
            "source.cameras",
            "animated mesh resources do not import authored cameras",
            "remove cameras from the exported actor resource",
        ));
    }
    bounded_count(
        "source.materials",
        document.materials().count(),
        MAX_ANIMATED_GLB_MATERIALS,
    )?;
    bounded_count(
        "source.textures",
        document.textures().count(),
        MAX_ANIMATED_GLB_TEXTURES,
    )?;
    bounded_count(
        "source.images",
        document.images().count(),
        MAX_ANIMATED_GLB_IMAGES,
    )?;
    let joint_count = document
        .skins()
        .map(|skin| skin.joints().count())
        .try_fold(0usize, |total, count| total.checked_add(count))
        .ok_or_else(|| resource_limit("source.skins", "joint count overflowed"))?;
    bounded_count("source.skins.joints", joint_count, MAX_ANIMATED_GLB_JOINTS)?;
    let mut embedded_image_bytes = 0usize;
    for image in document.images() {
        match image.source() {
            ImageSource::Uri { .. } => {
                return Err(ImportDiagnostic::error(
                    ImportCode::ExternalResource,
                    format!("source.images[{}]", image.index()),
                    "animated GLB import never resolves external image URIs",
                    "embed PNG or JPEG images in the GLB",
                ));
            }
            ImageSource::View { view, mime_type } => {
                if !matches!(mime_type, "image/png" | "image/jpeg" | "image/webp") {
                    return Err(ImportDiagnostic::error(
                        ImportCode::UnsupportedFeature,
                        format!("source.images[{}].mimeType", image.index()),
                        format!("embedded image MIME type `{mime_type}` is not admitted"),
                        "embed a PNG, JPEG, or WebP image",
                    ));
                }
                if view.length() == 0 || view.length() > MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES {
                    return Err(resource_limit(
                        &format!("source.images[{}]", image.index()),
                        &format!(
                            "embedded image byte count {} is outside 1..={MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES}",
                            view.length()
                        ),
                    ));
                }
                embedded_image_bytes =
                    embedded_image_bytes
                        .checked_add(view.length())
                        .ok_or_else(|| {
                            resource_limit("source.images", "image byte count overflowed")
                        })?;
            }
        }
    }
    if embedded_image_bytes > MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES {
        return Err(resource_limit(
            "source.images",
            &format!(
                "embedded image bytes {embedded_image_bytes} exceed {MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES}"
            ),
        ));
    }
    Ok(parsed)
}

fn is_admitted_extension(extension: &str) -> bool {
    matches!(
        extension,
        "EXT_texture_webp" | "KHR_materials_unlit" | "KHR_texture_transform"
    )
}

/// `gltf`'s `allow_empty_texture` feature is necessary because the glTF 2.0
/// representation of `EXT_texture_webp` deliberately omits the core texture
/// source. Keep that parser accommodation narrow at the Engine boundary: an
/// otherwise empty texture remains invalid unless the extension supplies one
/// indexed embedded WebP image.
fn validate_webp_texture_extensions(document: &serde_json::Value) -> Result<(), ImportDiagnostic> {
    let images = document
        .get("images")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let Some(textures) = document
        .get("textures")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };

    for (texture_index, texture) in textures.iter().enumerate() {
        let Some(texture) = texture.as_object() else {
            // The glTF parser provides the ordinary structural diagnostic.
            continue;
        };
        let extension = texture
            .get("extensions")
            .and_then(serde_json::Value::as_object)
            .and_then(|extensions| extensions.get("EXT_texture_webp"));
        let texture_locus = format!("source.textures[{texture_index}]");
        let Some(extension) = extension else {
            if !texture.contains_key("source") {
                return Err(ImportDiagnostic::error(
                    ImportCode::InvalidContainer,
                    format!("{texture_locus}.source"),
                    "texture omits core source without EXT_texture_webp source",
                    "provide a core image source or one EXT_texture_webp image source",
                ));
            }
            continue;
        };
        let source = extension
            .as_object()
            .and_then(|extension| extension.get("source"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < images.len())
            .ok_or_else(|| {
                ImportDiagnostic::error(
                    ImportCode::InvalidContainer,
                    format!("{texture_locus}.extensions.EXT_texture_webp.source"),
                    "EXT_texture_webp source must be one valid image index",
                    "reference one declared embedded WebP image",
                )
            })?;
        let mime_type = images[source]
            .as_object()
            .and_then(|image| image.get("mimeType"))
            .and_then(serde_json::Value::as_str);
        if mime_type != Some("image/webp") {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                format!("source.images[{source}].mimeType"),
                "EXT_texture_webp must reference an image/webp image",
                "embed a WebP image for the extension texture source",
            ));
        }
    }
    Ok(())
}

fn extension_names(
    document: &serde_json::Value,
    property: &str,
) -> Result<Vec<String>, ImportDiagnostic> {
    let Some(extensions) = document.get(property) else {
        return Ok(Vec::new());
    };
    let extensions = extensions.as_array().ok_or_else(|| {
        ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            format!("source.{property}"),
            format!("{property} must be an array"),
            "repair the embedded glTF JSON document",
        )
    })?;
    extensions
        .iter()
        .map(|extension| {
            extension.as_str().map(str::to_owned).ok_or_else(|| {
                ImportDiagnostic::error(
                    ImportCode::InvalidContainer,
                    format!("source.{property}"),
                    format!("{property} names must be strings"),
                    "repair the embedded glTF JSON document",
                )
            })
        })
        .collect()
}

fn validate_texture_transforms(document: &serde_json::Value) -> Result<(), ImportDiagnostic> {
    let Some(materials) = document
        .get("materials")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for (material_index, material) in materials.iter().enumerate() {
        let texture_infos = [
            (
                "pbrMetallicRoughness.baseColorTexture",
                material.pointer("/pbrMetallicRoughness/baseColorTexture"),
            ),
            (
                "pbrMetallicRoughness.metallicRoughnessTexture",
                material.pointer("/pbrMetallicRoughness/metallicRoughnessTexture"),
            ),
            ("normalTexture", material.get("normalTexture")),
            ("occlusionTexture", material.get("occlusionTexture")),
            ("emissiveTexture", material.get("emissiveTexture")),
        ];
        for (property, texture_info) in texture_infos {
            let Some(transform) =
                texture_info.and_then(|value| value.pointer("/extensions/KHR_texture_transform"))
            else {
                continue;
            };
            let path = format!(
                "source.materials[{material_index}].{property}.extensions.KHR_texture_transform"
            );
            validate_texture_transform(transform, &path)?;
        }
    }
    Ok(())
}

fn validate_texture_transform(
    transform: &serde_json::Value,
    path: &str,
) -> Result<(), ImportDiagnostic> {
    let object = transform
        .as_object()
        .ok_or_else(|| invalid_texture_transform(path, "texture transform must be an object"))?;
    for property in object.keys() {
        if !matches!(
            property.as_str(),
            "offset" | "rotation" | "scale" | "texCoord" | "extensions" | "extras"
        ) {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                format!("{path}.{property}"),
                format!("texture transform property `{property}` is not supported"),
                "use offset, rotation, scale, texCoord, extensions, or extras only",
            ));
        }
    }
    if let Some(offset) = object.get("offset") {
        validate_texture_transform_pair(offset, &format!("{path}.offset"))?;
    }
    if let Some(rotation) = object.get("rotation") {
        validate_texture_transform_number(rotation, &format!("{path}.rotation"))?;
    }
    if let Some(scale) = object.get("scale") {
        validate_texture_transform_pair(scale, &format!("{path}.scale"))?;
    }
    if let Some(tex_coord) = object.get("texCoord") {
        let Some(tex_coord) = tex_coord.as_u64() else {
            return Err(invalid_texture_transform(
                &format!("{path}.texCoord"),
                "texture coordinate override must be a non-negative integer",
            ));
        };
        if tex_coord > MAX_ANIMATED_GLB_TEXTURE_COORD_SET {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                format!("{path}.texCoord"),
                format!(
                    "texture coordinate set {tex_coord} exceeds renderer support through TEXCOORD_{MAX_ANIMATED_GLB_TEXTURE_COORD_SET}"
                ),
                "author the texture against TEXCOORD_0 through TEXCOORD_3",
            ));
        }
    }
    Ok(())
}

fn validate_texture_transform_pair(
    value: &serde_json::Value,
    path: &str,
) -> Result<(), ImportDiagnostic> {
    let Some(values) = value.as_array().filter(|values| values.len() == 2) else {
        return Err(invalid_texture_transform(
            path,
            "texture transform vector must contain exactly two numbers",
        ));
    };
    for (index, value) in values.iter().enumerate() {
        validate_texture_transform_number(value, &format!("{path}[{index}]"))?;
    }
    Ok(())
}

fn validate_texture_transform_number(
    value: &serde_json::Value,
    path: &str,
) -> Result<(), ImportDiagnostic> {
    let Some(value) = value.as_f64() else {
        return Err(invalid_texture_transform(
            path,
            "texture transform component must be a finite number",
        ));
    };
    if !value.is_finite() {
        return Err(ImportDiagnostic::error(
            ImportCode::NonFiniteValue,
            path,
            "texture transform component must be finite",
            "author a finite UV transform",
        ));
    }
    if value.abs() > MAX_ANIMATED_GLB_TEXTURE_TRANSFORM_COMPONENT {
        return Err(resource_limit(
            path,
            &format!(
                "texture transform component {value} exceeds absolute bound {MAX_ANIMATED_GLB_TEXTURE_TRANSFORM_COMPONENT}"
            ),
        ));
    }
    Ok(())
}

fn invalid_texture_transform(path: &str, message: &str) -> ImportDiagnostic {
    ImportDiagnostic::error(
        ImportCode::InvalidContainer,
        path,
        message,
        "repair the KHR_texture_transform texture-info payload",
    )
}

fn source_identity(
    source_uri: &SourceUri,
    source_hash_hex: &str,
    diagnostics: &mut Vec<ImportDiagnostic>,
) -> Option<(AssetId, String, String)> {
    let value = source_uri.value();
    let path = value.strip_prefix("file://").unwrap_or(value);
    let Some(file_name) = Path::new(path).file_name().and_then(|value| value.to_str()) else {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            value,
            "animated GLB source URI has no UTF-8 filename",
            "select a named binary GLB source",
        ));
        return None;
    };
    let name = file_name
        .strip_suffix(".glb")
        .or_else(|| file_name.strip_suffix(".GLB"));
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::UnsupportedFeature,
            value,
            "animated import accepts binary .glb source files only",
            "select a named binary GLB source",
        ));
        return None;
    };
    // AssetId intentionally remains strict while source filenames are
    // ordinary provenance/display metadata. The source hash is deterministic
    // for the exact copied bytes, survives a project move or filename rename,
    // and prevents collisions that a lossy filename normalization would hide.
    let asset_text = format!("mesh-animation/{source_hash_hex}");
    match AssetId::parse(&asset_text) {
        Ok(asset_id) if asset_id.kind() == AssetKind::AnimatedMesh => {
            Some((asset_id, name.to_owned(), format!("{name}.glb")))
        }
        Ok(_) => unreachable!("mesh-animation prefix always selects animated mesh kind"),
        Err(error) => {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                value,
                format!("Engine-derived animated mesh identity is invalid: {error}"),
                "repair the Engine-derived source identity",
            ));
            None
        }
    }
}

fn render_bounds(
    bounds: voxel_convert::MeshSourceBounds,
) -> Result<MeshBoundsDescriptor, ImportDiagnostic> {
    let min = bounds.min.map(|value| value as f32);
    let max = bounds.max.map(|value| value as f32);
    if !min.iter().chain(max.iter()).all(|value| value.is_finite()) {
        return Err(ImportDiagnostic::error(
            ImportCode::NonFiniteValue,
            "source.bounds",
            "animated GLB bounds do not fit finite renderer coordinates",
            "normalize the source geometry into finite f32 coordinates",
        ));
    }
    Ok(MeshBoundsDescriptor { min, max })
}

fn bounded_count(path: &str, count: usize, limit: usize) -> Result<(), ImportDiagnostic> {
    if count > limit {
        return Err(resource_limit(
            path,
            &format!("count {count} exceeds {limit}"),
        ));
    }
    Ok(())
}

fn resource_limit(path: &str, message: &str) -> ImportDiagnostic {
    ImportDiagnostic::error(
        ImportCode::ResourceLimit,
        path,
        message,
        "reduce the authored resource below the documented animated GLB limit",
    )
}

fn map_conversion_code(code: &str, message: &str) -> ImportCode {
    match code {
        "conversion.resourceLimit" => ImportCode::ResourceLimit,
        "conversion.unsupportedFeature" | "conversion.unsupportedSource" => {
            ImportCode::UnsupportedFeature
        }
        "conversion.invalidAnimation"
        | "conversion.invalidSkin"
        | "conversion.invalidMorphTarget"
        | "conversion.invalidDeformation" => ImportCode::InvalidAnimation,
        "conversion.invalidGeometry" if message.contains("finite") => ImportCode::NonFiniteValue,
        "conversion.invalidSource" => ImportCode::InvalidContainer,
        _ => ImportCode::MalformedSource,
    }
}

fn conversion_remedy(code: &str) -> &'static str {
    match code {
        "conversion.resourceLimit" => "reduce the GLB below the documented import limits",
        "conversion.unsupportedFeature" | "conversion.unsupportedSource" => {
            "remove unsupported GLB features and embed every resource"
        }
        "conversion.invalidAnimation"
        | "conversion.invalidSkin"
        | "conversion.invalidMorphTarget"
        | "conversion.invalidDeformation" => {
            "repair animation channels, skin bindings, or morph targets"
        }
        _ => "repair the GLB container, hierarchy, geometry, or transforms",
    }
}

fn count_u32(value: usize) -> u32 {
    u32::try_from(value).expect("admission limits keep counts within u32")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn failed(diagnostics: Vec<ImportDiagnostic>) -> AnimatedGlbImportOutcome {
    AnimatedGlbImportOutcome {
        assets: None,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_transform_validation_covers_every_core_texture_info() {
        let transform = serde_json::json!({
            "offset": [-1_000_000.0, 1_000_000.0],
            "rotation": 0.25,
            "scale": [-2.0, 3.0],
            "texCoord": 3,
            "extras": {"fixture": true}
        });
        let mut document = serde_json::json!({
            "materials": [{
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"extensions": {"KHR_texture_transform": transform.clone()}},
                    "metallicRoughnessTexture": {"extensions": {"KHR_texture_transform": transform.clone()}}
                },
                "normalTexture": {"extensions": {"KHR_texture_transform": transform.clone()}},
                "occlusionTexture": {"extensions": {"KHR_texture_transform": transform.clone()}},
                "emissiveTexture": {"extensions": {"KHR_texture_transform": transform}}
            }]
        });
        validate_texture_transforms(&document).expect("all core texture-info transforms");

        for pointer in [
            "/materials/0/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform",
            "/materials/0/pbrMetallicRoughness/metallicRoughnessTexture/extensions/KHR_texture_transform",
            "/materials/0/normalTexture/extensions/KHR_texture_transform",
            "/materials/0/occlusionTexture/extensions/KHR_texture_transform",
            "/materials/0/emissiveTexture/extensions/KHR_texture_transform",
        ] {
            let original = document.pointer(pointer).unwrap().clone();
            *document.pointer_mut(pointer).unwrap() = serde_json::json!({"texCoord": 4});
            let diagnostic = validate_texture_transforms(&document)
                .expect_err("each texture-info location is validated");
            assert_eq!(diagnostic.code, ImportCode::UnsupportedFeature);
            *document.pointer_mut(pointer).unwrap() = original;
        }
    }
}
