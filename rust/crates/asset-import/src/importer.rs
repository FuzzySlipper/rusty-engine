use std::collections::BTreeSet;

use asset_catalog::{
    validate_catalog, AssetCatalog, CatalogEntry, MaterialAuthority, MaterialDefinition,
    MaterialStyle, Rgba, UvStrategy,
};
use core_assets::{AssetId, AssetReference, AssetVersionReq};
use render_model::{
    MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout,
    MeshCollisionPolicy, MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot,
    MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, StaticMeshAsset,
};

use crate::fingerprint::fingerprint_hash;
use crate::{
    ImportCode, ImportDiagnostic, ImportSettings, SourceCollision, SourceMaterial, SourceMesh,
};

#[derive(Debug, Clone, Default)]
pub struct ImportContext {
    pub available_textures: Option<BTreeSet<String>>,
    pub settings: ImportSettings,
}

impl ImportContext {
    pub fn with_textures(textures: impl IntoIterator<Item = String>) -> Self {
        Self {
            available_textures: Some(textures.into_iter().collect()),
            settings: ImportSettings::default(),
        }
    }

    fn texture_is_missing(&self, name: &str) -> bool {
        self.available_textures
            .as_ref()
            .is_some_and(|textures| !textures.contains(name))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAssets {
    pub static_mesh: StaticMeshAsset,
    pub catalog: AssetCatalog,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportOutcome {
    pub assets: Option<ImportedAssets>,
    pub diagnostics: Vec<ImportDiagnostic>,
}

impl ImportOutcome {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(ImportDiagnostic::is_error)
    }
}

pub fn import(source: &SourceMesh) -> ImportOutcome {
    import_with_context(source, &ImportContext::default())
}

pub fn import_with_context(source: &SourceMesh, context: &ImportContext) -> ImportOutcome {
    let mut diagnostics = Vec::new();
    let mesh_id = format!("mesh/{}", source.name);
    if AssetId::parse(&mesh_id).is_err() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{mesh_id}#name"),
            "source name does not form a valid scoped-kebab asset id",
            "use lowercase kebab-case path segments",
        ));
    }
    if !context.settings.is_valid() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::InvalidImportSettings,
            &mesh_id,
            "import scale must be finite and greater than zero; material namespace must be scoped kebab-case",
            "correct the sidecar or project override settings",
        ));
    }
    if !source.positions.len().is_multiple_of(3) {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::AttributeLengthMismatch,
            &mesh_id,
            "positions are not three floats per vertex",
            "supply one xyz position per vertex",
        ));
    }
    if source.normals.len() != source.positions.len() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::AttributeLengthMismatch,
            &mesh_id,
            "normal and position stream lengths differ",
            "supply one xyz normal per vertex",
        ));
    }
    if !source.indices.len().is_multiple_of(3) {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::UnsupportedTopology,
            &mesh_id,
            "indices are not a triangle list",
            "triangulate the mesh before import",
        ));
    }
    if source
        .positions
        .iter()
        .chain(&source.normals)
        .any(|value| !value.is_finite())
    {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::NonFiniteValue,
            &mesh_id,
            "vertex streams contain NaN or infinity",
            "repair non-finite geometry values",
        ));
    }
    let vertex_count = source.positions.len() / 3;
    if let Some(index) = source
        .indices
        .iter()
        .copied()
        .find(|index| *index as usize >= vertex_count)
    {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::IndexOutOfRange,
            &mesh_id,
            format!("index {index} exceeds vertex count {vertex_count}"),
            "repair the source index buffer",
        ));
    }
    let mut slots = BTreeSet::new();
    for material in &source.materials {
        if !slots.insert(material.slot) {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::DuplicateMaterialSlot,
                &mesh_id,
                format!("material slot {} occurs more than once", material.slot),
                "assign each material a unique slot",
            ));
        }
        if material.name.is_empty()
            || material
                .color
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::InvalidDescriptor,
                format!("{mesh_id}#material/{}", material.slot),
                "material needs a non-empty name and finite normalized RGBA color",
                "repair the source material name and color",
            ));
        }
    }
    for group in &source.groups {
        if !slots.contains(&group.material_slot) {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::GroupSlotUnbound,
                &mesh_id,
                format!("group references undeclared slot {}", group.material_slot),
                "declare every material slot used by a group",
            ));
        }
        if group
            .start
            .checked_add(group.count)
            .is_none_or(|end| end > source.indices.len() as u32)
        {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::InvalidGroupRange,
                &mesh_id,
                "group range exceeds the index buffer",
                "repair group start/count values",
            ));
        }
    }
    if diagnostics.iter().any(ImportDiagnostic::is_error) {
        return ImportOutcome {
            assets: None,
            diagnostics,
        };
    }

    let positions: Vec<_> = source
        .positions
        .iter()
        .map(|value| value * context.settings.scale)
        .collect();
    let static_mesh = StaticMeshAsset {
        asset: mesh_id.clone(),
        payload: MeshPayloadDescriptor {
            layout: MeshBufferLayout {
                vertex_count: vertex_count as u32,
                index_count: source.indices.len() as u32,
                index_width: MeshIndexWidth::U32,
                attributes: vec![
                    MeshAttribute {
                        name: MeshAttributeName::Position,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                    MeshAttribute {
                        name: MeshAttributeName::Normal,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                ],
            },
            groups: source
                .groups
                .iter()
                .map(|group| MeshGroupDescriptor {
                    material_slot: group.material_slot,
                    start: group.start,
                    count: group.count,
                })
                .collect(),
            bounds: bounds_of(&positions),
            source: MeshPayloadSource::Inline {
                positions,
                normals: source.normals.clone(),
                uvs: None,
                indices: source.indices.clone(),
            },
            provenance: MeshProvenance::StaticAsset,
        },
        material_slots: source
            .materials
            .iter()
            .map(|material| MeshMaterialSlot {
                slot: material.slot,
                material: material_id(material, &context.settings),
            })
            .collect(),
        collision: collision_policy(source, &context.settings),
    };
    if let Err(error) = static_mesh.validate() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::InvalidDescriptor,
            &mesh_id,
            format!("generated static mesh is invalid: {error:?}"),
            "repair source geometry, groups, material names, or collision policy",
        ));
        return ImportOutcome {
            assets: None,
            diagnostics,
        };
    }

    let mut entries = Vec::new();
    let mut ids = BTreeSet::new();
    let mut mesh_dependencies = Vec::new();
    for material in &source.materials {
        let mut material_dependencies = Vec::new();
        if let Some(texture) = &material.texture {
            if context.texture_is_missing(texture) {
                diagnostics.push(ImportDiagnostic::warning(
                    ImportCode::MissingTexture,
                    format!("material/{}#texture", material.name),
                    format!("texture `{texture}` is unavailable"),
                    "provide the external texture resource or remove the reference",
                ));
            }
            let texture_id = format!("texture/{texture}");
            let texture_hash = fingerprint_hash(texture_id.as_bytes());
            let Some(id) = parse_generated_id(&texture_id, &mut diagnostics) else {
                continue;
            };
            push_unique(
                &mut entries,
                &mut ids,
                &mut diagnostics,
                CatalogEntry::new(id.clone(), 1)
                    .with_hash(texture_hash.clone())
                    .with_label(texture),
            );
            material_dependencies.push(AssetReference::new(
                id,
                AssetVersionReq::Exact(1),
                Some(texture_hash),
            ));
        }

        let material_id_text = material_id(material, &context.settings);
        let material_hash =
            fingerprint_hash(material_fingerprint(material, &context.settings).as_bytes());
        let Some(id) = parse_generated_id(&material_id_text, &mut diagnostics) else {
            continue;
        };
        let texture_ref = material_dependencies.first().cloned();
        let definition = MaterialDefinition {
            authority: MaterialAuthority::DECORATIVE,
            style: MaterialStyle {
                color: rgba(material.color),
                texture: texture_ref,
                roughness: 1.0,
                texture_tint: Rgba::WHITE,
                emission_color: rgba(material.color),
                emissive: 0.0,
                uv_strategy: if material.texture.is_some() {
                    UvStrategy::Planar
                } else {
                    UvStrategy::Flat
                },
                voxel_surface: None,
            },
        };
        push_unique(
            &mut entries,
            &mut ids,
            &mut diagnostics,
            CatalogEntry::new(id.clone(), 1)
                .with_hash(material_hash.clone())
                .with_label(&material.name)
                .with_dependencies(material_dependencies)
                .with_material(definition),
        );
        mesh_dependencies.push(AssetReference::new(
            id,
            AssetVersionReq::Exact(1),
            Some(material_hash),
        ));
    }
    let mesh_hash = fingerprint_hash(
        &serde_json::to_vec(&static_mesh).expect("static mesh descriptor serializes"),
    );
    if let Some(id) = parse_generated_id(&mesh_id, &mut diagnostics) {
        push_unique(
            &mut entries,
            &mut ids,
            &mut diagnostics,
            CatalogEntry::new(id, 1)
                .with_hash(mesh_hash)
                .with_label(&source.name)
                .with_dependencies(mesh_dependencies),
        );
    }
    if diagnostics.iter().any(ImportDiagnostic::is_error) {
        return ImportOutcome {
            assets: None,
            diagnostics,
        };
    }
    let catalog = AssetCatalog::from_entries(entries).canonical();
    let validation = validate_catalog(&catalog);
    if !validation.is_ok() {
        diagnostics.extend(validation.diagnostics().into_iter().map(|item| {
            ImportDiagnostic::error(
                ImportCode::InvalidDescriptor,
                item.path,
                item.message,
                "repair generated asset dependencies",
            )
        }));
        return ImportOutcome {
            assets: None,
            diagnostics,
        };
    }
    ImportOutcome {
        assets: Some(ImportedAssets {
            static_mesh,
            catalog,
        }),
        diagnostics,
    }
}

fn bounds_of(positions: &[f32]) -> MeshBoundsDescriptor {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in positions.chunks_exact(3) {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }
    if positions.is_empty() {
        min = [0.0; 3];
        max = [0.0; 3];
    }
    MeshBoundsDescriptor { min, max }
}

fn collision_policy(source: &SourceMesh, settings: &ImportSettings) -> MeshCollisionPolicy {
    match &source.collision {
        SourceCollision::VisualOnly if settings.generate_collision => {
            MeshCollisionPolicy::AabbFallback
        }
        SourceCollision::VisualOnly => MeshCollisionPolicy::VisualOnly,
        SourceCollision::AabbFallback => MeshCollisionPolicy::AabbFallback,
        SourceCollision::Proxy(name) => MeshCollisionPolicy::Proxy {
            proxy_asset: format!("mesh/{name}"),
        },
    }
}

fn material_id(material: &SourceMaterial, settings: &ImportSettings) -> String {
    match &settings.material_namespace {
        Some(namespace) => format!("material/{namespace}/{}", material.name),
        None => format!("material/{}", material.name),
    }
}

fn material_fingerprint(material: &SourceMaterial, settings: &ImportSettings) -> String {
    format!(
        "{}:{:08x?}:{:?}",
        material_id(material, settings),
        material.color.map(f32::to_bits),
        material.texture
    )
}

fn rgba(value: [f32; 4]) -> Rgba {
    Rgba {
        r: value[0],
        g: value[1],
        b: value[2],
        a: value[3],
    }
}

fn parse_generated_id(text: &str, diagnostics: &mut Vec<ImportDiagnostic>) -> Option<AssetId> {
    match AssetId::parse(text) {
        Ok(id) => Some(id),
        Err(error) => {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                text,
                error.to_string(),
                "use lowercase scoped kebab-case names",
            ));
            None
        }
    }
}

fn push_unique(
    entries: &mut Vec<CatalogEntry>,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ImportDiagnostic>,
    entry: CatalogEntry,
) {
    if !ids.insert(entry.id.as_str().to_owned()) {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::DuplicateAssetId,
            entry.id.as_str(),
            "two source declarations resolve to the same asset id",
            "give materials and textures distinct names",
        ));
    } else {
        entries.push(entry);
    }
}
