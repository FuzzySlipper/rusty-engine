use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use render_model::{
    LightDescriptor, LightShadowIntent, MaterialUvStrategy, RenderDiff, RenderFrameDiff,
    RenderHandle, RenderMaterialDescriptor, RenderMetadata, TextureDescriptor, TextureFilter,
    TexturePayloadSource, TextureWrap, Transform, VoxelAtlasPaddingDescriptor,
    VoxelAtlasRegionDescriptor, VoxelSurfaceAlphaModeDescriptor, VoxelSurfaceDescriptor,
    VoxelSurfaceMappingDescriptor,
};
use render_projection::{VoxelObjectProjectionInstance, VoxelObjectRenderProjector};
use serde::Serialize;
use svc_mesh::SurfaceMode;
use voxel_asset::decode_voxel_object;
use voxel_object_runtime::{
    admit_voxel_object_with_options, AdmittedVoxelObject, VoxelObjectAdmissionOptions,
};

#[derive(Debug)]
struct ModelArgument {
    label: String,
    path: PathBuf,
    include_clip_frames: bool,
    texture: Option<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonReport {
    schema_version: u32,
    entries: Vec<ComparisonEntry>,
    resources: Vec<ComparisonResource>,
    texture_resources: Vec<ComparisonResource>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonEntry {
    model: String,
    source_path: String,
    frame: u32,
    frame_id: String,
    mode: String,
    textured_source: bool,
    unsupported_reason: Option<String>,
    build_milliseconds: f64,
    metrics: Option<ComparisonMetrics>,
    projection: Option<render_model::RenderFrameDiff>,
    resource_ids: Vec<String>,
    texture_resource_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonMetrics {
    occupied_voxels: usize,
    vertices: u32,
    triangles: u32,
    material_partitions: usize,
    sampled_cells: u64,
    qef_rank_deficient: u32,
    qef_fallbacks: u32,
    packed_bytes: usize,
    retained_resource_count: usize,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonResource {
    resource: String,
    content_hash: String,
    byte_length: usize,
    base64: String,
}

struct ProjectedEntry {
    entry: ComparisonEntry,
    mesh_resources: Vec<ComparisonResource>,
    texture_resources: Vec<ComparisonResource>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (output, models) = arguments()?;
    let mut entries = Vec::new();
    let mut resources = BTreeMap::new();
    let mut texture_resources = BTreeMap::new();
    for model in models {
        let source = fs::read_to_string(&model.path)?;
        let object = decode_voxel_object(&source)?;
        for mode in [
            SurfaceMode::GreedyCubes,
            SurfaceMode::MarchingCubes,
            SurfaceMode::DualContouring,
        ] {
            if model.texture.is_some() && !mode.supports_voxel_tile_coordinates() {
                entries.push(ComparisonEntry {
                    model: model.label.clone(),
                    source_path: model.path.display().to_string(),
                    frame: 0,
                    frame_id: "default".to_string(),
                    mode: mode.as_str().to_string(),
                    textured_source: true,
                    unsupported_reason: Some(
                        "reconstructed surfaces reject voxel repeat/atlas textures because stable UV projection is not defined"
                            .to_string(),
                    ),
                    build_milliseconds: 0.0,
                    metrics: None,
                    projection: None,
                    resource_ids: Vec::new(),
                    texture_resource_ids: Vec::new(),
                });
                continue;
            }
            let started = Instant::now();
            let admitted = admit_voxel_object_with_options(
                &object,
                VoxelObjectAdmissionOptions {
                    surface_mode: mode,
                    ..VoxelObjectAdmissionOptions::default()
                },
            )?;
            let build_milliseconds = started.elapsed().as_secs_f64() * 1_000.0;
            let frames = selected_frames(&admitted, model.include_clip_frames);
            for frame in frames {
                let projected = project_entry(&model, &admitted, mode, frame, build_milliseconds)?;
                for resource in projected.mesh_resources {
                    resources
                        .entry(resource.resource.clone())
                        .or_insert(resource);
                }
                for resource in projected.texture_resources {
                    texture_resources
                        .entry(resource.resource.clone())
                        .or_insert(resource);
                }
                entries.push(projected.entry);
            }
        }
    }
    let report = ComparisonReport {
        schema_version: 1,
        entries,
        resources: resources.into_values().collect(),
        texture_resources: texture_resources.into_values().collect(),
    };
    fs::write(output, serde_json::to_vec(&report)?)?;
    Ok(())
}

fn project_entry(
    model: &ModelArgument,
    object: &AdmittedVoxelObject,
    mode: SurfaceMode,
    frame: u32,
    build_milliseconds: f64,
) -> Result<ProjectedEntry, Box<dyn std::error::Error>> {
    let runtime_frame = object.frame(frame).ok_or("selected frame is unavailable")?;
    let mesh = object
        .mesh(runtime_frame.mesh_index)
        .ok_or("selected mesh is unavailable")?;
    let instance = VoxelObjectProjectionInstance {
        instance_id: format!("{}-{}-{frame}", model.label, mode.as_str()),
        object,
        frame,
        transform: Transform::IDENTITY,
        visible: true,
        material_overrides: Vec::new(),
        metadata: RenderMetadata {
            source_entity: None,
            source_scene_node: None,
            tags: vec!["voxel-surface-comparison".to_string()],
            label: Some(format!("{} {}", model.label, mode.as_str())),
        },
    };
    let texture = model
        .texture
        .as_ref()
        .map(|path| comparison_texture(path))
        .transpose()?;
    let mut projector = VoxelObjectRenderProjector::with_packed_mesh_resources();
    let projected = projector
        .project(
            &[instance],
            &materials(object, texture.as_ref().map(|value| &value.0)),
        )
        .map_err(|error| format!("voxel projection failed: {error:?}"))?;
    let mut operations = Vec::new();
    if let Some((descriptor, _)) = &texture {
        operations.push(RenderDiff::DefineTexture {
            texture: descriptor.clone(),
        });
    }
    operations.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(42),
        parent: None,
        light: LightDescriptor::Ambient {
            color: [1.0; 3],
            intensity: 2.0,
            enabled: true,
            shadow_intent: LightShadowIntent::Disabled,
        },
    });
    operations.extend(projected.frame.ops.iter().cloned());
    let projection = RenderFrameDiff::try_from_ops(operations)
        .map_err(|error| format!("comparison frame failed validation: {error:?}"))?;
    let packed_bytes = projected
        .mesh_resources
        .iter()
        .map(|resource| resource.bytes.len())
        .sum();
    let resources = projected
        .mesh_resources
        .into_iter()
        .map(|resource| ComparisonResource {
            byte_length: resource.bytes.len(),
            base64: BASE64.encode(resource.bytes),
            resource: resource.resource,
            content_hash: resource.content_hash,
        })
        .collect::<Vec<_>>();
    let resource_ids = resources
        .iter()
        .map(|resource| resource.resource.clone())
        .collect();
    let texture_resources = texture
        .into_iter()
        .map(|(_, resource)| resource)
        .collect::<Vec<_>>();
    let texture_resource_ids = texture_resources
        .iter()
        .map(|resource| resource.resource.clone())
        .collect::<Vec<_>>();
    let retained_resource_count = resources.len() + texture_resources.len();
    Ok(ProjectedEntry {
        entry: ComparisonEntry {
            model: model.label.clone(),
            source_path: model.path.display().to_string(),
            frame,
            frame_id: match &runtime_frame.source {
                voxel_object_runtime::VoxelObjectFrameSource::Default => "default".to_string(),
                voxel_object_runtime::VoxelObjectFrameSource::Clip { clip, frame } => {
                    format!("{clip}/{frame}")
                }
            },
            mode: mode.as_str().to_string(),
            textured_source: model.texture.is_some(),
            unsupported_reason: None,
            build_milliseconds,
            metrics: Some(ComparisonMetrics {
                occupied_voxels: runtime_frame.cells.len(),
                vertices: mesh.stats.vertices,
                triangles: mesh.stats.triangles,
                material_partitions: mesh.groups.len(),
                sampled_cells: mesh.stats.sampled_cells,
                qef_rank_deficient: mesh.stats.qef_rank_deficient,
                qef_fallbacks: mesh.stats.qef_fallbacks,
                packed_bytes,
                retained_resource_count,
                bounds_min: mesh.bounds.min,
                bounds_max: mesh.bounds.max,
            }),
            projection: Some(projection),
            resource_ids,
            texture_resource_ids,
        },
        mesh_resources: resources,
        texture_resources,
    })
}

fn materials(
    object: &AdmittedVoxelObject,
    texture: Option<&TextureDescriptor>,
) -> BTreeMap<String, RenderMaterialDescriptor> {
    object
        .source()
        .material_palette
        .iter()
        .map(|binding| {
            let slot = binding.material_slot;
            let color = [
                f32::from((slot.wrapping_mul(67) % 191) as u8) / 255.0 + 0.2,
                f32::from((slot.wrapping_mul(101) % 191) as u8) / 255.0 + 0.2,
                f32::from((slot.wrapping_mul(149) % 191) as u8) / 255.0 + 0.2,
                1.0,
            ];
            let material = RenderMaterialDescriptor {
                schema_version: if texture.is_some() { 2 } else { 1 },
                id: binding.material_asset_id.clone(),
                color,
                texture: texture.map(|descriptor| descriptor.id.clone()),
                roughness: 0.85,
                texture_tint: [1.0; 4],
                emission_color: [0.0; 3],
                emission_intensity: 0.0,
                uv_strategy: if texture.is_some() {
                    MaterialUvStrategy::Atlas
                } else {
                    MaterialUvStrategy::Flat
                },
                voxel_surface: texture.map(|descriptor| atlas_surface(descriptor, slot)),
            };
            (material.id.clone(), material)
        })
        .collect()
}

fn comparison_texture(
    path: &Path,
) -> Result<(TextureDescriptor, ComparisonResource), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let descriptor = TextureDescriptor::admit_png_rgba8_resource(
        "texture/surface-comparison-atlas".to_string(),
        &bytes,
        TextureFilter::Nearest,
        TextureWrap::Clamp,
        1,
    )
    .map_err(|error| format!("comparison texture was rejected: {error:?}"))?;
    let payload = descriptor
        .payload
        .as_ref()
        .ok_or("texture payload missing")?;
    let TexturePayloadSource::Resource { resource } = &payload.source else {
        return Err("comparison texture must use a resource payload".into());
    };
    let resource = resource.clone();
    let content_hash = payload.content_hash.clone();
    Ok((
        descriptor,
        ComparisonResource {
            resource,
            content_hash,
            byte_length: bytes.len(),
            base64: BASE64.encode(bytes),
        },
    ))
}

fn atlas_surface(texture: &TextureDescriptor, slot: u16) -> VoxelSurfaceDescriptor {
    let texture_hash = texture
        .content_hash
        .clone()
        .expect("admitted texture has a content hash");
    let (id, content_min) = if slot.is_multiple_of(2) {
        ("cool-arrow", [9, 1])
    } else {
        ("warm-arrow", [1, 1])
    };
    VoxelSurfaceDescriptor {
        schema_version: 1,
        filter: TextureFilter::Nearest,
        wrap: TextureWrap::Clamp,
        alpha_mode: VoxelSurfaceAlphaModeDescriptor::Opaque,
        mapping: VoxelSurfaceMappingDescriptor::Atlas {
            atlas: "sprite-sheet/surface-comparison".to_string(),
            atlas_version: 1,
            atlas_content_hash: texture_hash.clone(),
            texture: texture.id.clone(),
            texture_version: texture.version,
            texture_content_hash: texture_hash,
            region: VoxelAtlasRegionDescriptor {
                id: id.to_string(),
                content_min,
                content_extent: [6, 6],
                padding: VoxelAtlasPaddingDescriptor {
                    left: 1,
                    right: 1,
                    bottom: 1,
                    top: 1,
                },
                inset: "halfTexel".to_string(),
            },
            tile_scale_cells: [1.0, 1.0],
            tile_origin_cells: [0.0, 0.0],
        },
    }
}

fn selected_frames(object: &AdmittedVoxelObject, include_clip_frames: bool) -> Vec<u32> {
    let mut frames = vec![0];
    if include_clip_frames {
        if let Some(clip) = object.clips().first() {
            frames.extend(clip.frame_indices.iter().take(3).copied());
        }
        frames.sort_unstable();
        frames.dedup();
    }
    frames
}

fn arguments() -> Result<(PathBuf, Vec<ModelArgument>), Box<dyn std::error::Error>> {
    let mut values = env::args().skip(1);
    let mut output = None;
    let mut models = Vec::new();
    while let Some(argument) = values.next() {
        let value = values.next().ok_or("argument requires a value")?;
        match argument.as_str() {
            "--output" => output = Some(PathBuf::from(value)),
            "--model" => models.push(parse_model(&value, false)?),
            "--animated-model" => models.push(parse_model(&value, true)?),
            "--textured-model" => models.push(parse_textured_model(&value)?),
            _ => return Err(format!("unknown argument {argument}").into()),
        }
    }
    let output = output.ok_or("--output is required")?;
    if models.is_empty() {
        return Err("at least one --model is required".into());
    }
    Ok((output, models))
}

fn parse_model(
    value: &str,
    include_clip_frames: bool,
) -> Result<ModelArgument, Box<dyn std::error::Error>> {
    let (label, path) = value
        .split_once('=')
        .ok_or("model argument must be label=/absolute/path")?;
    if label.is_empty() || !Path::new(path).is_absolute() {
        return Err("model label must be non-empty and path must be absolute".into());
    }
    Ok(ModelArgument {
        label: label.to_string(),
        path: PathBuf::from(path),
        include_clip_frames,
        texture: None,
    })
}

fn parse_textured_model(value: &str) -> Result<ModelArgument, Box<dyn std::error::Error>> {
    let (label, paths) = value
        .split_once('=')
        .ok_or("textured model must be label=/absolute/object@/absolute/texture")?;
    let (object, texture) = paths
        .split_once('@')
        .ok_or("textured model must include @/absolute/texture")?;
    if label.is_empty() || !Path::new(object).is_absolute() || !Path::new(texture).is_absolute() {
        return Err("textured model label must be non-empty and both paths absolute".into());
    }
    Ok(ModelArgument {
        label: label.to_string(),
        path: PathBuf::from(object),
        include_clip_frames: false,
        texture: Some(PathBuf::from(texture)),
    })
}
