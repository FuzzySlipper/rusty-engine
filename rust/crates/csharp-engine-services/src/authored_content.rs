use std::{collections::BTreeMap, ffi::c_void};

use asset_catalog::{
    AdmittedAssetCatalog, AssetCatalog, AssetContext, AtlasInset, AtlasPadding,
    AtlasRegionDefinition, CatalogAdmissionError, CatalogDiagnostic, CatalogEntry,
    CatalogResolveError, FallbackOutcome, FallbackVisual, MaterialAuthority, MaterialDefinition,
    MaterialStyle, RenderMaterial, ResolvedVoxelSurface, ResolvedVoxelSurfaceMapping,
    StructuralClass, TextureDefinition, TextureFilter, TextureWrap, UvStrategy, VoxelAlphaMode,
    VoxelAtlasDefinition, VoxelSurfaceBinding, VoxelSurfaceMapping, VoxelSurfaceResolutionError,
};
use content_store::{
    decode_prefab_registry, resolve_prefab as resolve_prefab_owner, PrefabDefinition,
    PrefabDiagnostic, PrefabOverride, PrefabOverrideValue, PrefabPart, PrefabPartRoleBinding,
    PrefabPartSource, PrefabRegistry, PrefabRegistryValidationContext, PrefabTransform,
    PrefabVariantDelta, ValidatedPrefabRegistry,
};
use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use core_ids::{PrefabId, PrefabPartId};
use csharp_engine_abi::*;

use crate::{
    composition::{borrowed_slice, borrowed_utf8, ABI_OK},
    content::RuntimeContentBridge,
};

const SERVICE: &[u8] = b"AuthoredContent";
const MAX_ENTRIES: usize = 4096;
const MAX_DEPENDENCIES: usize = 16384;
const MAX_PAYLOAD_ROWS: usize = 16384;
const MAX_TEXT: usize = 4096;
const MAX_DIAGNOSTICS: usize = 128;
const MAX_PREFAB_DEFINITIONS: usize = 4096;
const MAX_PREFAB_ROWS: usize = 16384;
const MAX_ENTITY_DEFINITION_IDS: usize = 16384;

pub(crate) struct RuntimeAuthoredContentBridge {
    catalogs: BTreeMap<u64, AdmittedAssetCatalog>,
    next_catalog: u64,
    leases: BTreeMap<u64, CatalogLease>,
    next_lease: u64,
    resolved_leases: BTreeMap<u64, ResolvedLease>,
    next_resolved_lease: u64,
    diagnostics: BTreeMap<u64, DiagnosticLease>,
    next_diagnostic: u64,
    material_leases: BTreeMap<u64, MaterialResolutionLease>,
    next_material_lease: u64,
    surface_leases: BTreeMap<u64, SurfaceResolutionLease>,
    next_surface_lease: u64,
    fallback_leases: BTreeMap<u64, FallbackLease>,
    next_fallback_lease: u64,
    prefab_registries: BTreeMap<u64, ValidatedPrefabRegistry>,
    next_prefab_registry: u64,
    prefab_leases: BTreeMap<u64, PrefabRegistryLease>,
    next_prefab_lease: u64,
    resolved_prefab_leases: BTreeMap<u64, ResolvedPrefabLease>,
    next_resolved_prefab_lease: u64,
    content: Option<*const RuntimeContentBridge>,
}
struct Text {
    values: Vec<String>,
}
impl Text {
    fn copy(&mut self, value: &str) -> NativeUtf8Slice {
        self.values.push(value.to_owned());
        let value = self.values.last().unwrap();
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }
}

#[test]
fn admits_prefab_registry_content_inside_the_owner() {
    use std::{collections::BTreeMap, sync::Arc};

    fn slice(value: &'static [u8]) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    let source = PrefabRegistry {
        schema_version: 1,
        definitions: vec![PrefabDefinition {
            id: PrefabId::new(1),
            schema_version: 1,
            display_name: "Base".into(),
            parts: vec![PrefabPart {
                id: PrefabPartId::new(10),
                namespace: "body/root".into(),
                display_name: "Body".into(),
                parent: None,
                transform: PrefabTransform::IDENTITY,
                source: PrefabPartSource::Scene {
                    asset: "scene/test".into(),
                },
            }],
            part_roles: vec![PrefabPartRoleBinding {
                role: "body/root".into(),
                part: PrefabPartId::new(10),
            }],
            variant: None,
        }],
    };
    let source_context = PrefabRegistryValidationContext::from_asset_ids(
        [AssetId::parse("scene/test").unwrap()],
        [] as [String; 0],
    );
    let encoded = content_store::encode_prefab_registry(
        &ValidatedPrefabRegistry::new(source, &source_context).unwrap(),
    )
    .unwrap();
    let mut resources = BTreeMap::new();
    resources.insert("prefabs.json".to_owned(), Arc::from(encoded.into_bytes()));
    let mut content = RuntimeContentBridge::new(resources);
    let content_api = crate::content::api(&mut content);
    let mut reference = NativeContentReferenceHandle::default();
    assert_eq!(
        unsafe {
            (content_api.open_reference)(
                content_api.context,
                &NativeContentOpenRequest {
                    path: slice(b"prefabs.json"),
                },
                &mut reference,
            )
        },
        ABI_OK
    );
    let mut bridge = RuntimeAuthoredContentBridge::new();
    bridge.bind_content(&content);
    let api = api(&mut bridge);
    let entries = [NativeAuthoredCatalogEntryInput {
        id: slice(b"scene/test"),
        version: 1,
        has_hash: false,
        hash: slice(b""),
        has_source_path: false,
        source_path: slice(b""),
        has_label: false,
        label: slice(b""),
    }];
    let mut catalog = NativeAuthoredCatalogHandle::default();
    let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe {
            (api.admit_catalog)(
                api.context,
                &NativeAuthoredCatalogAdmitRequest {
                    entries: entries.as_ptr(),
                    entries_len: entries.len(),
                    dependencies: std::ptr::null(),
                    dependencies_len: 0,
                },
                &mut catalog,
                &mut receipt,
            )
        },
        ABI_OK
    );
    let mut registry = NativeAuthoredPrefabRegistryHandle::default();
    assert_eq!(
        unsafe {
            (api.admit_prefab_registry_from_content)(
                api.context,
                &NativeAuthoredPrefabRegistryFromContentRequest {
                    content: reference,
                    catalog,
                    entity_definition_ids: std::ptr::null(),
                    entity_definition_ids_len: 0,
                },
                &mut registry,
                &mut receipt,
            )
        },
        ABI_OK
    );
    let mut readout: NativeAuthoredPrefabRegistryReadoutLease = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { (api.read_prefab_registry)(api.context, registry, &mut readout) },
        ABI_OK
    );
    assert_eq!((readout.definitions_len, readout.parts_len), (1, 1));
    assert_eq!(
        unsafe { (api.destroy_prefab_registry_readout_lease)(api.context, readout.handle) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (api.destroy_prefab_registry)(api.context, registry) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (api.destroy_catalog)(api.context, catalog) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (content_api.destroy_reference)(content_api.context, reference) },
        ABI_OK
    );
}
struct CatalogLease {
    _text: Text,
    entries: Vec<NativeAuthoredCatalogEntryReadout>,
    dependencies: Vec<NativeAuthoredCatalogDependencyReadout>,
    materials: Vec<NativeAuthoredMaterialReadout>,
    textures: Vec<NativeAuthoredTextureReadout>,
    voxel_atlases: Vec<NativeAuthoredVoxelAtlasReadout>,
    atlas_regions: Vec<NativeAuthoredAtlasRegionReadout>,
    voxel_surfaces: Vec<NativeAuthoredVoxelSurfaceReadout>,
    hash: String,
}
struct ResolvedLease {
    _text: Text,
    entry: Vec<NativeAuthoredCatalogEntryReadout>,
    dependencies: Vec<NativeAuthoredCatalogDependencyReadout>,
    materials: Vec<NativeAuthoredMaterialReadout>,
    textures: Vec<NativeAuthoredTextureReadout>,
    voxel_atlases: Vec<NativeAuthoredVoxelAtlasReadout>,
    atlas_regions: Vec<NativeAuthoredAtlasRegionReadout>,
    voxel_surfaces: Vec<NativeAuthoredVoxelSurfaceReadout>,
}
struct MaterialResolutionLease {
    _text: Text,
    materials: Vec<NativeAuthoredMaterialReadout>,
    voxel_surfaces: Vec<NativeAuthoredVoxelSurfaceReadout>,
}
struct SurfaceResolutionLease {
    _text: Text,
    surfaces: Vec<NativeAuthoredVoxelSurfaceReadout>,
}
struct FallbackLease {
    _text: Text,
    outcomes: Vec<NativeAuthoredFallbackReadout>,
}
struct PayloadReadouts {
    materials: Vec<NativeAuthoredMaterialReadout>,
    textures: Vec<NativeAuthoredTextureReadout>,
    voxel_atlases: Vec<NativeAuthoredVoxelAtlasReadout>,
    atlas_regions: Vec<NativeAuthoredAtlasRegionReadout>,
    voxel_surfaces: Vec<NativeAuthoredVoxelSurfaceReadout>,
}
struct DiagnosticLease {
    _text: Text,
    values: Vec<NativeEngineDiagnostic>,
}
struct PrefabRegistryLease {
    _text: Text,
    definitions: Vec<NativeAuthoredPrefabDefinitionReadout>,
    parts: Vec<NativeAuthoredPrefabPartReadout>,
    roles: Vec<NativeAuthoredPrefabRoleReadout>,
    removed_roles: Vec<NativeAuthoredPrefabRemovedRoleReadout>,
    overrides: Vec<NativeAuthoredPrefabOverrideReadout>,
}
struct ResolvedPrefabLease {
    _text: Text,
    variant_id: String,
    parts: Vec<NativeAuthoredPrefabPartReadout>,
    roles: Vec<NativeAuthoredPrefabRoleReadout>,
}
#[derive(Debug)]
enum AuthoredError {
    Validation(Vec<CatalogDiagnostic>),
    PrefabValidation(Vec<PrefabDiagnostic>),
    Simple {
        code: &'static str,
        message: String,
        source: String,
    },
}
impl AuthoredError {
    fn simple(message: impl Into<String>) -> Self {
        Self::Simple {
            code: "AUTHORED_CONTENT_INPUT",
            message: message.into(),
            source: "catalog".into(),
        }
    }
}

impl RuntimeAuthoredContentBridge {
    pub(crate) fn new() -> Self {
        Self {
            catalogs: BTreeMap::new(),
            next_catalog: 1,
            leases: BTreeMap::new(),
            next_lease: 1,
            resolved_leases: BTreeMap::new(),
            next_resolved_lease: 1,
            diagnostics: BTreeMap::new(),
            next_diagnostic: 1,
            material_leases: BTreeMap::new(),
            next_material_lease: 1,
            surface_leases: BTreeMap::new(),
            next_surface_lease: 1,
            fallback_leases: BTreeMap::new(),
            next_fallback_lease: 1,
            prefab_registries: BTreeMap::new(),
            next_prefab_registry: 1,
            prefab_leases: BTreeMap::new(),
            next_prefab_lease: 1,
            resolved_prefab_leases: BTreeMap::new(),
            next_resolved_prefab_lease: 1,
            content: None,
        }
    }
    pub(crate) fn bind_content(&mut self, content: &RuntimeContentBridge) {
        self.content = Some(content);
    }
    fn retain(
        &mut self,
        catalog: AdmittedAssetCatalog,
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        let value = self.next_catalog;
        self.next_catalog = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("catalog handle exhausted"))?;
        self.catalogs.insert(value, catalog);
        Ok(NativeAuthoredCatalogHandle { value })
    }
    fn admit_rows(
        &mut self,
        entries: &[NativeAuthoredCatalogEntryInput],
        dependencies: &[NativeAuthoredCatalogDependencyInput],
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        let values = Self::base_entries(entries, dependencies, false)?;
        self.retain(
            AdmittedAssetCatalog::admit(AssetCatalog::from_entries(values))
                .map_err(admission_error)?,
        )
    }
    fn base_entries(
        entries: &[NativeAuthoredCatalogEntryInput],
        dependencies: &[NativeAuthoredCatalogDependencyInput],
        allow_material: bool,
    ) -> Result<Vec<CatalogEntry>, AuthoredError> {
        if entries.len() > MAX_ENTRIES || dependencies.len() > MAX_DEPENDENCIES {
            return Err(AuthoredError::simple("catalog input exceeds engine bounds"));
        }
        let mut values = Vec::with_capacity(entries.len());
        for row in entries {
            let id = parse_id(row.id, "entry id").map_err(AuthoredError::simple)?;
            if !allow_material && id.kind() == AssetKind::Material {
                return Err(AuthoredError::simple(
                    "material requires its payload and is not admitted by AuthoredContent base",
                ));
            }
            let mut entry = CatalogEntry::new(id, row.version);
            if row.has_hash {
                entry.hash =
                    Some(parse_hash(row.hash, "entry hash").map_err(AuthoredError::simple)?);
            }
            if row.has_source_path {
                entry.source_path = Some(
                    parse_text(row.source_path, "source path").map_err(AuthoredError::simple)?,
                );
            }
            if row.has_label {
                entry.label = Some(parse_text(row.label, "label").map_err(AuthoredError::simple)?);
            }
            values.push(entry);
        }
        for row in dependencies {
            let entry_id =
                parse_id(row.entry_id, "dependency entry id").map_err(AuthoredError::simple)?;
            let dependency = parse_reference_parts(
                row.reference_id,
                row.reference_version_kind,
                row.reference_version,
                row.reference_has_hash,
                row.reference_hash,
            )
            .map_err(AuthoredError::simple)?;
            let Some(entry) = values.iter_mut().find(|entry| entry.id == entry_id) else {
                return Err(AuthoredError::simple(
                    "dependency entry is absent from request",
                ));
            };
            entry.dependencies.push(dependency);
        }
        Ok(values)
    }
    fn admit_payload_rows(
        &mut self,
        request: NativeAuthoredCatalogPayloadAdmitRequest,
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        let entries =
            unsafe { borrowed_slice(request.entries, request.entries_len, "catalog entries") }
                .map_err(|error| AuthoredError::simple(error.to_string()))?;
        let dependencies = unsafe {
            borrowed_slice(
                request.dependencies,
                request.dependencies_len,
                "catalog dependencies",
            )
        }
        .map_err(|error| AuthoredError::simple(error.to_string()))?;
        let materials = unsafe {
            borrowed_slice(
                request.materials,
                request.materials_len,
                "material payloads",
            )
        }
        .map_err(|error| AuthoredError::simple(error.to_string()))?;
        let textures =
            unsafe { borrowed_slice(request.textures, request.textures_len, "texture payloads") }
                .map_err(|error| AuthoredError::simple(error.to_string()))?;
        let atlases = unsafe {
            borrowed_slice(
                request.voxel_atlases,
                request.voxel_atlases_len,
                "voxel atlas payloads",
            )
        }
        .map_err(|error| AuthoredError::simple(error.to_string()))?;
        let regions = unsafe {
            borrowed_slice(
                request.atlas_regions,
                request.atlas_regions_len,
                "atlas region payloads",
            )
        }
        .map_err(|error| AuthoredError::simple(error.to_string()))?;
        let surfaces = unsafe {
            borrowed_slice(
                request.voxel_surfaces,
                request.voxel_surfaces_len,
                "voxel surface payloads",
            )
        }
        .map_err(|error| AuthoredError::simple(error.to_string()))?;
        if [
            materials.len(),
            textures.len(),
            atlases.len(),
            regions.len(),
            surfaces.len(),
        ]
        .into_iter()
        .any(|count| count > MAX_PAYLOAD_ROWS)
        {
            return Err(AuthoredError::simple(
                "catalog payload input exceeds engine bounds",
            ));
        }
        let mut values = Self::base_entries(entries, dependencies, true)?;
        let mut seen = std::collections::BTreeSet::new();
        for row in materials {
            let id = parse_id(row.entry_id, "material entry id").map_err(AuthoredError::simple)?;
            if !seen.insert(("material", id.as_str().to_owned())) {
                return Err(AuthoredError::simple("duplicate material payload row"));
            }
            let texture = parse_optional_reference(
                row.has_texture,
                row.texture_id,
                row.texture_version_kind,
                row.texture_version,
                row.texture_has_hash,
                row.texture_hash,
            )?;
            let entry = find_entry_mut(&mut values, &id)?;
            entry.material = Some(MaterialDefinition {
                authority: MaterialAuthority {
                    solid: row.solid,
                    collidable: row.collidable,
                    occludes: row.occludes,
                    structural_class: structural_class(row.structural_class)?,
                },
                style: MaterialStyle {
                    color: rgba(row.color),
                    texture,
                    roughness: row.roughness,
                    texture_tint: rgba(row.texture_tint),
                    emission_color: rgba(row.emission_color),
                    emissive: row.emissive,
                    uv_strategy: uv_strategy(row.uv_strategy)?,
                    voxel_surface: None,
                },
            });
        }
        seen.clear();
        for row in textures {
            let id = parse_id(row.entry_id, "texture entry id").map_err(AuthoredError::simple)?;
            if !seen.insert(("texture", id.as_str().to_owned())) {
                return Err(AuthoredError::simple("duplicate texture payload row"));
            }
            let entry = find_entry_mut(&mut values, &id)?;
            entry.texture = Some(TextureDefinition {
                width: row.width,
                height: row.height,
                filter: texture_filter(row.filter)?,
                wrap: texture_wrap(row.wrap)?,
            });
        }
        seen.clear();
        for row in atlases {
            let id = parse_id(row.entry_id, "atlas entry id").map_err(AuthoredError::simple)?;
            if !seen.insert(("atlas", id.as_str().to_owned())) {
                return Err(AuthoredError::simple("duplicate voxel atlas payload row"));
            }
            let texture = parse_reference_parts(
                row.texture_id,
                row.texture_version_kind,
                row.texture_version,
                row.texture_has_hash,
                row.texture_hash,
            )
            .map_err(AuthoredError::simple)?;
            let entry = find_entry_mut(&mut values, &id)?;
            entry.voxel_atlas = Some(VoxelAtlasDefinition {
                schema_version: row.schema_version,
                texture,
                regions: Vec::new(),
            });
        }
        seen.clear();
        for row in regions {
            let atlas_id = parse_id(row.atlas_entry_id, "atlas region owner id")
                .map_err(AuthoredError::simple)?;
            let region_id = parse_text(row.id, "atlas region id").map_err(AuthoredError::simple)?;
            if !seen.insert(("region", format!("{}:{region_id}", atlas_id.as_str()))) {
                return Err(AuthoredError::simple("duplicate atlas region payload row"));
            }
            let entry = find_entry_mut(&mut values, &atlas_id)?;
            let atlas = entry
                .voxel_atlas
                .as_mut()
                .ok_or_else(|| AuthoredError::simple("atlas region owner has no atlas payload"))?;
            atlas.regions.push(AtlasRegionDefinition {
                id: region_id,
                content_min: [row.content_min_x, row.content_min_y],
                content_extent: [row.content_extent_x, row.content_extent_y],
                padding: AtlasPadding {
                    left: row.padding_left,
                    right: row.padding_right,
                    bottom: row.padding_bottom,
                    top: row.padding_top,
                },
                inset: atlas_inset(row.inset)?,
            });
        }
        seen.clear();
        for row in surfaces {
            let id = parse_id(row.material_entry_id, "voxel surface material id")
                .map_err(AuthoredError::simple)?;
            if !seen.insert(("surface", id.as_str().to_owned())) {
                return Err(AuthoredError::simple("duplicate voxel surface payload row"));
            }
            let mapping = match row.mapping_kind {
                NativeAuthoredVoxelSurfaceMappingKind::Repeat => VoxelSurfaceMapping::Repeat {
                    texture: parse_reference_parts(
                        row.texture_id,
                        row.texture_version_kind,
                        row.texture_version,
                        row.texture_has_hash,
                        row.texture_hash,
                    )
                    .map_err(AuthoredError::simple)?,
                    tile_scale_cells: [row.tile_scale_x, row.tile_scale_y],
                    tile_origin_cells: [row.tile_origin_x, row.tile_origin_y],
                },
                NativeAuthoredVoxelSurfaceMappingKind::Atlas => VoxelSurfaceMapping::Atlas {
                    atlas: parse_reference_parts(
                        row.atlas_id,
                        row.atlas_version_kind,
                        row.atlas_version,
                        row.atlas_has_hash,
                        row.atlas_hash,
                    )
                    .map_err(AuthoredError::simple)?,
                    region: parse_text(row.region, "voxel surface region")
                        .map_err(AuthoredError::simple)?,
                    tile_scale_cells: [row.tile_scale_x, row.tile_scale_y],
                    tile_origin_cells: [row.tile_origin_x, row.tile_origin_y],
                },
            };
            let entry = find_entry_mut(&mut values, &id)?;
            let material = entry.material.as_mut().ok_or_else(|| {
                AuthoredError::simple("voxel surface owner has no material payload")
            })?;
            material.style.voxel_surface = Some(VoxelSurfaceBinding {
                schema_version: row.schema_version,
                mapping,
                alpha_mode: voxel_alpha(row.alpha_mode, row.alpha_cutoff)?,
            });
        }
        self.retain(
            AdmittedAssetCatalog::admit(AssetCatalog::from_entries(values))
                .map_err(admission_error)?,
        )
    }
    fn admit_content(
        &mut self,
        reference: NativeContentReferenceHandle,
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        let content = unsafe {
            self.content
                .ok_or_else(|| AuthoredError::simple("authored content is not composed"))?
                .as_ref()
        }
        .ok_or_else(|| AuthoredError::simple("authored content is not composed"))?;
        let bytes = content
            .retained_bytes(reference)
            .ok_or_else(|| AuthoredError::simple("unknown content reference"))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| AuthoredError::simple("catalog content was not UTF-8"))?;
        self.retain(AdmittedAssetCatalog::reopen(text).map_err(admission_error)?)
    }
    fn prefab_context(
        &self,
        catalog: NativeAuthoredCatalogHandle,
        entity_definition_ids: &[NativeAuthoredPrefabEntityDefinitionInput],
    ) -> Result<PrefabRegistryValidationContext, AuthoredError> {
        if entity_definition_ids.len() > MAX_ENTITY_DEFINITION_IDS {
            return Err(AuthoredError::simple(
                "prefab entity-definition input exceeds engine bounds",
            ));
        }
        let catalog = self
            .catalogs
            .get(&catalog.value)
            .ok_or_else(|| AuthoredError::simple("unknown catalog handle"))?;
        let entity_definition_ids = entity_definition_ids
            .iter()
            .map(|row| parse_text(row.stable_id, "entity definition id"))
            .collect::<Result<Vec<_>, _>>()
            .map_err(AuthoredError::simple)?;
        Ok(PrefabRegistryValidationContext::from_asset_ids(
            catalog.catalog().iter().map(|entry| entry.id.clone()),
            entity_definition_ids,
        ))
    }
    fn retain_prefab_registry(
        &mut self,
        registry: ValidatedPrefabRegistry,
    ) -> Result<NativeAuthoredPrefabRegistryHandle, AuthoredError> {
        let value = self.next_prefab_registry;
        self.next_prefab_registry = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("prefab registry handle exhausted"))?;
        self.prefab_registries.insert(value, registry);
        Ok(NativeAuthoredPrefabRegistryHandle { value })
    }
    fn admit_prefab_rows(
        &mut self,
        request: NativeAuthoredPrefabRegistryAdmitRequest,
        definitions: &[NativeAuthoredPrefabDefinitionInput],
        parts: &[NativeAuthoredPrefabPartInput],
        roles: &[NativeAuthoredPrefabRoleInput],
        removed_roles: &[NativeAuthoredPrefabRemovedRoleInput],
        overrides: &[NativeAuthoredPrefabOverrideInput],
        entity_definition_ids: &[NativeAuthoredPrefabEntityDefinitionInput],
    ) -> Result<NativeAuthoredPrefabRegistryHandle, AuthoredError> {
        if definitions.len() > MAX_PREFAB_DEFINITIONS
            || [
                parts.len(),
                roles.len(),
                removed_roles.len(),
                overrides.len(),
            ]
            .into_iter()
            .any(|count| count > MAX_PREFAB_ROWS)
        {
            return Err(AuthoredError::simple("prefab input exceeds engine bounds"));
        }
        let context = self.prefab_context(request.catalog, entity_definition_ids)?;
        let mut registry = PrefabRegistry {
            schema_version: request.schema_version,
            definitions: definitions
                .iter()
                .map(prefab_definition)
                .collect::<Result<Vec<_>, _>>()
                .map_err(AuthoredError::simple)?,
        };
        for row in parts {
            let definition = find_prefab_definition_mut(&mut registry, row.prefab_id)?;
            definition
                .parts
                .push(prefab_part(row).map_err(AuthoredError::simple)?);
        }
        for row in roles {
            let definition = find_prefab_definition_mut(&mut registry, row.prefab_id)?;
            definition.part_roles.push(PrefabPartRoleBinding {
                role: parse_text(row.role, "prefab role").map_err(AuthoredError::simple)?,
                part: PrefabPartId::new(row.part_id),
            });
        }
        for row in removed_roles {
            let definition = find_prefab_definition_mut(&mut registry, row.prefab_id)?;
            let variant = definition.variant.as_mut().ok_or_else(|| {
                AuthoredError::simple("removed role belongs to a non-variant prefab")
            })?;
            variant
                .removed_roles
                .push(parse_text(row.role, "removed prefab role").map_err(AuthoredError::simple)?);
        }
        for row in overrides {
            let definition = find_prefab_definition_mut(&mut registry, row.prefab_id)?;
            let variant = definition
                .variant
                .as_mut()
                .ok_or_else(|| AuthoredError::simple("override belongs to a non-variant prefab"))?;
            variant
                .overrides
                .push(prefab_override(row).map_err(AuthoredError::simple)?);
        }
        self.retain_prefab_registry(
            ValidatedPrefabRegistry::new(registry, &context)
                .map_err(|report| AuthoredError::PrefabValidation(report.diagnostics))?,
        )
    }
    fn admit_prefab_content(
        &mut self,
        request: NativeAuthoredPrefabRegistryFromContentRequest,
        entity_definition_ids: &[NativeAuthoredPrefabEntityDefinitionInput],
    ) -> Result<NativeAuthoredPrefabRegistryHandle, AuthoredError> {
        let context = self.prefab_context(request.catalog, entity_definition_ids)?;
        let content = unsafe {
            self.content
                .ok_or_else(|| AuthoredError::simple("authored content is not composed"))?
                .as_ref()
        }
        .ok_or_else(|| AuthoredError::simple("authored content is not composed"))?;
        let bytes = content
            .retained_bytes(request.content)
            .ok_or_else(|| AuthoredError::simple("unknown content reference"))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| AuthoredError::simple("prefab registry content was not UTF-8"))?;
        self.retain_prefab_registry(decode_prefab_registry(text, &context).map_err(|error| {
            AuthoredError::Simple {
                code: "AUTHORED_PREFAB_CONTENT",
                message: error.message,
                source: error.path,
            }
        })?)
    }
    fn read_prefab_registry(
        &mut self,
        handle: NativeAuthoredPrefabRegistryHandle,
    ) -> Option<NativeAuthoredPrefabRegistryReadoutLease> {
        let registry = self.prefab_registries.get(&handle.value)?.clone();
        let value = self.next_prefab_lease;
        self.next_prefab_lease = value.checked_add(1)?;
        let mut text = Text { values: vec![] };
        let mut definitions = vec![];
        let mut parts = vec![];
        let mut roles = vec![];
        let mut removed_roles = vec![];
        let mut overrides = vec![];
        for definition in &registry.as_registry().definitions {
            definitions.push(prefab_definition_row(&mut text, definition));
            for part in &definition.parts {
                parts.push(prefab_part_row(&mut text, definition.id, part, None, true));
            }
            for role in &definition.part_roles {
                roles.push(prefab_role_row(
                    &mut text,
                    definition.id,
                    role.part,
                    &role.role,
                ));
            }
            if let Some(variant) = &definition.variant {
                for role in &variant.removed_roles {
                    removed_roles.push(NativeAuthoredPrefabRemovedRoleReadout {
                        prefab_id: definition.id.raw(),
                        role: text.copy(role),
                    });
                }
                for item in &variant.overrides {
                    overrides.push(prefab_override_row(&mut text, definition.id, item));
                }
            }
        }
        let lease = PrefabRegistryLease {
            _text: text,
            definitions,
            parts,
            roles,
            removed_roles,
            overrides,
        };
        let result = NativeAuthoredPrefabRegistryReadoutLease {
            handle: NativeAuthoredPrefabRegistryReadoutLeaseHandle { value },
            schema_version: registry.as_registry().schema_version,
            definitions: lease.definitions.as_ptr(),
            definitions_len: lease.definitions.len(),
            parts: lease.parts.as_ptr(),
            parts_len: lease.parts.len(),
            roles: lease.roles.as_ptr(),
            roles_len: lease.roles.len(),
            removed_roles: lease.removed_roles.as_ptr(),
            removed_roles_len: lease.removed_roles.len(),
            overrides: lease.overrides.as_ptr(),
            overrides_len: lease.overrides.len(),
        };
        self.prefab_leases.insert(value, lease);
        Some(result)
    }
    fn resolve_prefab_registry(
        &mut self,
        request: NativeAuthoredPrefabResolveRequest,
        instance_overrides: &[NativeAuthoredPrefabInstanceOverrideInput],
    ) -> Result<NativeAuthoredResolvedPrefabLease, AuthoredError> {
        if instance_overrides.len() > MAX_PREFAB_ROWS {
            return Err(AuthoredError::simple(
                "prefab instance overrides exceed engine bounds",
            ));
        }
        let registry = self
            .prefab_registries
            .get(&request.registry.value)
            .ok_or_else(|| AuthoredError::simple("unknown prefab registry handle"))?;
        let overrides = instance_overrides
            .iter()
            .map(prefab_instance_override)
            .collect::<Result<Vec<_>, _>>()
            .map_err(AuthoredError::simple)?;
        let resolved = resolve_prefab_owner(registry, PrefabId::new(request.prefab_id), &overrides)
            .map_err(|error| AuthoredError::Simple {
                code: "AUTHORED_PREFAB_RESOLUTION",
                message: error.to_string(),
                source: request.prefab_id.to_string(),
            })?;
        let value = self.next_resolved_prefab_lease;
        self.next_resolved_prefab_lease = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("resolved prefab lease exhausted"))?;
        let mut text = Text { values: vec![] };
        let mut parts = vec![];
        let mut roles = vec![];
        for part in &resolved.parts {
            parts.push(resolved_prefab_part_row(
                &mut text,
                resolved.requested,
                part,
            ));
            for role in &part.roles {
                roles.push(prefab_role_row(
                    &mut text,
                    resolved.requested,
                    part.id,
                    role,
                ));
            }
        }
        let variant_id = resolved.variant_id.unwrap_or_default();
        let lease = ResolvedPrefabLease {
            _text: text,
            variant_id,
            parts,
            roles,
        };
        let result = NativeAuthoredResolvedPrefabLease {
            handle: NativeAuthoredResolvedPrefabLeaseHandle { value },
            requested_id: resolved.requested.raw(),
            base_id: resolved.base.raw(),
            has_variant: !lease.variant_id.is_empty(),
            variant_id: NativeUtf8Slice {
                bytes: lease.variant_id.as_ptr(),
                len: lease.variant_id.len(),
            },
            parts: lease.parts.as_ptr(),
            parts_len: lease.parts.len(),
            roles: lease.roles.as_ptr(),
            roles_len: lease.roles.len(),
        };
        self.resolved_prefab_leases.insert(value, lease);
        Ok(result)
    }
    fn read_catalog(
        &mut self,
        handle: NativeAuthoredCatalogHandle,
    ) -> Option<NativeAuthoredCatalogReadoutLease> {
        let catalog = self.catalogs.get(&handle.value)?.clone();
        self.lease_for(catalog.catalog(), catalog.canonical_hash())
    }
    fn resolve(
        &mut self,
        request: NativeAuthoredCatalogResolveRequest,
    ) -> Result<NativeAuthoredResolvedEntryLease, AuthoredError> {
        let catalog = self
            .catalogs
            .get(&request.catalog.value)
            .ok_or_else(|| AuthoredError::simple("unknown catalog handle"))?;
        let reference = parse_reference_parts(
            request.reference_id,
            request.reference_version_kind,
            request.reference_version,
            request.reference_has_hash,
            request.reference_hash,
        )
        .map_err(AuthoredError::simple)?;
        let entry = catalog
            .catalog()
            .resolve_reference(&reference)
            .map_err(|error| match error {
                CatalogResolveError::Missing { .. } => AuthoredError::Simple {
                    code: "AUTHORED_CONTENT_REFERENCE_MISSING",
                    message: "catalog reference is missing".into(),
                    source: reference.id().as_str().into(),
                },
                CatalogResolveError::Stale { .. } => AuthoredError::Simple {
                    code: "AUTHORED_CONTENT_REFERENCE_STALE",
                    message: "catalog reference is stale".into(),
                    source: reference.id().as_str().into(),
                },
            })?
            .clone();
        let value = self.next_resolved_lease;
        self.next_resolved_lease = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("resolved entry lease exhausted"))?;
        let mut text = Text { values: vec![] };
        let dependencies = entry
            .dependencies
            .iter()
            .map(|dependency| dependency_row(&mut text, &entry, dependency))
            .collect();
        let rows = vec![entry_row(&mut text, &entry)];
        let payloads = payload_rows(&mut text, std::iter::once(&entry));
        let lease = ResolvedLease {
            _text: text,
            entry: rows,
            dependencies,
            materials: payloads.materials,
            textures: payloads.textures,
            voxel_atlases: payloads.voxel_atlases,
            atlas_regions: payloads.atlas_regions,
            voxel_surfaces: payloads.voxel_surfaces,
        };
        let out = NativeAuthoredResolvedEntryLease {
            handle: NativeAuthoredResolvedEntryLeaseHandle { value },
            entry: lease.entry.as_ptr(),
            entry_len: lease.entry.len(),
            dependencies: lease.dependencies.as_ptr(),
            dependencies_len: lease.dependencies.len(),
            materials: lease.materials.as_ptr(),
            materials_len: lease.materials.len(),
            textures: lease.textures.as_ptr(),
            textures_len: lease.textures.len(),
            voxel_atlases: lease.voxel_atlases.as_ptr(),
            voxel_atlases_len: lease.voxel_atlases.len(),
            atlas_regions: lease.atlas_regions.as_ptr(),
            atlas_regions_len: lease.atlas_regions.len(),
            voxel_surfaces: lease.voxel_surfaces.as_ptr(),
            voxel_surfaces_len: lease.voxel_surfaces.len(),
        };
        self.resolved_leases.insert(value, lease);
        Ok(out)
    }
    fn resolve_material(
        &mut self,
        request: NativeAuthoredMaterialResolveRequest,
    ) -> Result<NativeAuthoredMaterialResolutionLease, AuthoredError> {
        let material_id =
            parse_id(request.material_id, "material id").map_err(AuthoredError::simple)?;
        if material_id.kind() != AssetKind::Material {
            return Err(AuthoredError::simple(
                "material resolution requires a material asset id",
            ));
        }
        let catalog = self
            .catalogs
            .get(&request.catalog.value)
            .ok_or_else(|| AuthoredError::simple("unknown catalog handle"))?;
        let entry = catalog
            .catalog()
            .get(&material_id)
            .ok_or_else(|| AuthoredError::simple("material asset is missing"))?;
        let material = entry
            .material
            .as_ref()
            .ok_or_else(|| AuthoredError::simple("asset has no material payload"))?;
        let render = catalog
            .catalog()
            .render_material(&material_id)
            .map_err(|error| voxel_resolution_error(error, material_id.as_str()))?;
        let value = self.next_material_lease;
        self.next_material_lease = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("material resolution lease exhausted"))?;
        let mut text = Text { values: vec![] };
        let materials = vec![resolved_material_row(&mut text, entry, material, &render)];
        let voxel_surfaces = render
            .voxel_surface
            .as_ref()
            .map(|surface| resolved_surface_row(&mut text, entry, surface))
            .into_iter()
            .collect();
        let lease = MaterialResolutionLease {
            _text: text,
            materials,
            voxel_surfaces,
        };
        let out = NativeAuthoredMaterialResolutionLease {
            handle: NativeAuthoredMaterialResolutionLeaseHandle { value },
            materials: lease.materials.as_ptr(),
            materials_len: lease.materials.len(),
            voxel_surfaces: lease.voxel_surfaces.as_ptr(),
            voxel_surfaces_len: lease.voxel_surfaces.len(),
        };
        self.material_leases.insert(value, lease);
        Ok(out)
    }
    fn resolve_voxel_surface(
        &mut self,
        request: NativeAuthoredMaterialResolveRequest,
    ) -> Result<NativeAuthoredVoxelSurfaceResolutionLease, AuthoredError> {
        let material_id =
            parse_id(request.material_id, "material id").map_err(AuthoredError::simple)?;
        let catalog = self
            .catalogs
            .get(&request.catalog.value)
            .ok_or_else(|| AuthoredError::simple("unknown catalog handle"))?;
        let entry = catalog
            .catalog()
            .get(&material_id)
            .ok_or_else(|| AuthoredError::simple("material asset is missing"))?;
        let _material = entry
            .material
            .as_ref()
            .ok_or_else(|| AuthoredError::simple("material has no voxel surface payload"))?;
        let render = catalog
            .catalog()
            .render_material(&material_id)
            .map_err(|error| voxel_resolution_error(error, entry.id.as_str()))?;
        let resolved = render
            .voxel_surface
            .as_ref()
            .ok_or_else(|| AuthoredError::simple("material has no voxel surface payload"))?;
        let value = self.next_surface_lease;
        self.next_surface_lease = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("voxel surface resolution lease exhausted"))?;
        let mut text = Text { values: vec![] };
        let surfaces = vec![resolved_surface_row(&mut text, entry, resolved)];
        let lease = SurfaceResolutionLease {
            _text: text,
            surfaces,
        };
        let out = NativeAuthoredVoxelSurfaceResolutionLease {
            handle: NativeAuthoredVoxelSurfaceResolutionLeaseHandle { value },
            surfaces: lease.surfaces.as_ptr(),
            surfaces_len: lease.surfaces.len(),
        };
        self.surface_leases.insert(value, lease);
        Ok(out)
    }
    fn resolve_fallback(
        &mut self,
        request: NativeAuthoredFallbackResolveRequest,
    ) -> Result<NativeAuthoredFallbackLease, AuthoredError> {
        let kind = asset_kind(request.kind)?;
        let context = fallback_context(request.context)?;
        let value = self.next_fallback_lease;
        self.next_fallback_lease = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("fallback lease exhausted"))?;
        let mut text = Text { values: vec![] };
        let outcome = fallback_row(&mut text, asset_catalog::fallback_for(kind, context));
        let lease = FallbackLease {
            _text: text,
            outcomes: vec![outcome],
        };
        let out = NativeAuthoredFallbackLease {
            handle: NativeAuthoredFallbackLeaseHandle { value },
            outcomes: lease.outcomes.as_ptr(),
            outcomes_len: lease.outcomes.len(),
        };
        self.fallback_leases.insert(value, lease);
        Ok(out)
    }
    fn lease_for(
        &mut self,
        catalog: &AssetCatalog,
        canonical_hash: &str,
    ) -> Option<NativeAuthoredCatalogReadoutLease> {
        let value = self.next_lease;
        self.next_lease = value.checked_add(1)?;
        let mut text = Text { values: vec![] };
        let mut dependencies = vec![];
        let entries = catalog
            .iter()
            .map(|entry| {
                for dependency in &entry.dependencies {
                    dependencies.push(dependency_row(&mut text, entry, dependency));
                }
                entry_row(&mut text, entry)
            })
            .collect::<Vec<_>>();
        let hash = canonical_hash.to_owned();
        let payloads = payload_rows(&mut text, catalog.iter());
        let lease = CatalogLease {
            _text: text,
            entries,
            dependencies,
            materials: payloads.materials,
            textures: payloads.textures,
            voxel_atlases: payloads.voxel_atlases,
            atlas_regions: payloads.atlas_regions,
            voxel_surfaces: payloads.voxel_surfaces,
            hash,
        };
        let out = NativeAuthoredCatalogReadoutLease {
            handle: NativeAuthoredCatalogReadoutLeaseHandle { value },
            canonical_hash: NativeUtf8Slice {
                bytes: lease.hash.as_ptr(),
                len: lease.hash.len(),
            },
            entry_count: u32::try_from(lease.entries.len()).ok()?,
            entries: lease.entries.as_ptr(),
            entries_len: lease.entries.len(),
            dependencies: lease.dependencies.as_ptr(),
            dependencies_len: lease.dependencies.len(),
            materials: lease.materials.as_ptr(),
            materials_len: lease.materials.len(),
            textures: lease.textures.as_ptr(),
            textures_len: lease.textures.len(),
            voxel_atlases: lease.voxel_atlases.as_ptr(),
            voxel_atlases_len: lease.voxel_atlases.len(),
            atlas_regions: lease.atlas_regions.as_ptr(),
            atlas_regions_len: lease.atlas_regions.len(),
            voxel_surfaces: lease.voxel_surfaces.as_ptr(),
            voxel_surfaces_len: lease.voxel_surfaces.len(),
        };
        self.leases.insert(value, lease);
        Some(out)
    }
    fn diagnostic(&mut self, error: AuthoredError) -> Option<NativeEngineDiagnosticLease> {
        let value = self.next_diagnostic;
        self.next_diagnostic = value.checked_add(1)?;
        let mut text = Text { values: vec![] };
        let facts = match error {
            AuthoredError::Validation(values) => values
                .into_iter()
                .take(MAX_DIAGNOSTICS)
                .map(|value| (value.code, value.message, value.path))
                .collect(),
            AuthoredError::PrefabValidation(values) => values
                .into_iter()
                .take(MAX_DIAGNOSTICS)
                .map(|value| (value.code.as_str().to_owned(), value.message, value.path))
                .collect(),
            AuthoredError::Simple {
                code,
                message,
                source,
            } => vec![(code.to_owned(), message, source)],
        };
        let values = facts
            .into_iter()
            .map(|(code, message, source)| NativeEngineDiagnostic {
                code: text.copy(&code),
                message: text.copy(&message),
                source: text.copy(&source),
            })
            .collect();
        let lease = DiagnosticLease {
            _text: text,
            values,
        };
        let out = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value },
            diagnostics: lease.values.as_ptr(),
            diagnostics_len: lease.values.len(),
        };
        self.diagnostics.insert(value, lease);
        Some(out)
    }
}
fn payload_rows<'a>(
    text: &mut Text,
    entries: impl IntoIterator<Item = &'a CatalogEntry>,
) -> PayloadReadouts {
    let mut result = PayloadReadouts {
        materials: vec![],
        textures: vec![],
        voxel_atlases: vec![],
        atlas_regions: vec![],
        voxel_surfaces: vec![],
    };
    for entry in entries {
        if let Some(material) = &entry.material {
            result.materials.push(material_row(text, entry, material));
            if let Some(surface) = &material.style.voxel_surface {
                result
                    .voxel_surfaces
                    .push(surface_row(text, entry, surface));
            }
        }
        if let Some(texture) = &entry.texture {
            result.textures.push(NativeAuthoredTextureReadout {
                entry_id: text.copy(entry.id.as_str()),
                width: texture.width,
                height: texture.height,
                filter: native_texture_filter(texture.filter),
                wrap: native_texture_wrap(texture.wrap),
            });
        }
        if let Some(atlas) = &entry.voxel_atlas {
            result.voxel_atlases.push(NativeAuthoredVoxelAtlasReadout {
                entry_id: text.copy(entry.id.as_str()),
                schema_version: atlas.schema_version,
                texture: native_reference(text, &atlas.texture),
            });
            result
                .atlas_regions
                .extend(
                    atlas
                        .regions
                        .iter()
                        .map(|region| NativeAuthoredAtlasRegionReadout {
                            atlas_entry_id: text.copy(entry.id.as_str()),
                            id: text.copy(&region.id),
                            content_min_x: region.content_min[0],
                            content_min_y: region.content_min[1],
                            content_extent_x: region.content_extent[0],
                            content_extent_y: region.content_extent[1],
                            padding_left: region.padding.left,
                            padding_right: region.padding.right,
                            padding_bottom: region.padding.bottom,
                            padding_top: region.padding.top,
                            inset: native_atlas_inset(region.inset),
                        }),
                );
        }
    }
    result
}
fn material_row(
    text: &mut Text,
    entry: &CatalogEntry,
    material: &MaterialDefinition,
) -> NativeAuthoredMaterialReadout {
    NativeAuthoredMaterialReadout {
        entry_id: text.copy(entry.id.as_str()),
        solid: material.authority.solid,
        collidable: material.authority.collidable,
        occludes: material.authority.occludes,
        structural_class: native_structural_class(material.authority.structural_class),
        color: native_color(material.style.color),
        has_texture: material.style.texture.is_some(),
        texture: material
            .style
            .texture
            .as_ref()
            .map(|value| native_reference(text, value))
            .unwrap_or_else(|| empty_reference(text)),
        roughness: material.style.roughness,
        texture_tint: native_color(material.style.texture_tint),
        emission_color: native_color(material.style.emission_color),
        emissive: material.style.emissive,
        uv_strategy: native_uv_strategy(material.style.uv_strategy),
        has_voxel_surface: material.style.voxel_surface.is_some(),
    }
}
fn resolved_material_row(
    text: &mut Text,
    entry: &CatalogEntry,
    material: &MaterialDefinition,
    render: &RenderMaterial,
) -> NativeAuthoredMaterialReadout {
    let collision = material.collision_projection();
    NativeAuthoredMaterialReadout {
        entry_id: text.copy(entry.id.as_str()),
        solid: collision.solid,
        collidable: collision.collidable,
        occludes: collision.occludes,
        structural_class: native_structural_class(collision.structural_class),
        color: native_color(render.color),
        has_texture: render.texture.is_some(),
        texture: render
            .texture
            .as_ref()
            .map(|value| native_reference(text, value))
            .unwrap_or_else(|| empty_reference(text)),
        roughness: render.roughness,
        texture_tint: native_color(render.texture_tint),
        emission_color: native_color(render.emission_color),
        emissive: render.emissive,
        uv_strategy: native_uv_strategy(render.uv_strategy),
        has_voxel_surface: render.voxel_surface.is_some(),
    }
}
fn surface_row(
    text: &mut Text,
    entry: &CatalogEntry,
    surface: &VoxelSurfaceBinding,
) -> NativeAuthoredVoxelSurfaceReadout {
    let (mapping_kind, texture, atlas, region, scale, origin) = match &surface.mapping {
        VoxelSurfaceMapping::Repeat {
            texture,
            tile_scale_cells,
            tile_origin_cells,
        } => (
            NativeAuthoredVoxelSurfaceMappingKind::Repeat,
            native_reference(text, texture),
            empty_reference(text),
            text.copy(""),
            *tile_scale_cells,
            *tile_origin_cells,
        ),
        VoxelSurfaceMapping::Atlas {
            atlas,
            region,
            tile_scale_cells,
            tile_origin_cells,
        } => (
            NativeAuthoredVoxelSurfaceMappingKind::Atlas,
            empty_reference(text),
            native_reference(text, atlas),
            text.copy(region),
            *tile_scale_cells,
            *tile_origin_cells,
        ),
    };
    NativeAuthoredVoxelSurfaceReadout {
        material_entry_id: text.copy(entry.id.as_str()),
        schema_version: surface.schema_version,
        mapping_kind,
        texture,
        atlas,
        region,
        tile_scale_x: scale[0],
        tile_scale_y: scale[1],
        tile_origin_x: origin[0],
        tile_origin_y: origin[1],
        alpha_mode: native_voxel_alpha_kind(surface.alpha_mode),
        alpha_cutoff: voxel_alpha_cutoff(surface.alpha_mode),
        has_resolved_mapping: false,
        resolved_texture_version: 0,
        resolved_atlas_version: 0,
        resolved_filter: NativeAuthoredTextureFilter::Nearest,
        resolved_wrap: NativeAuthoredTextureWrap::Clamp,
        resolved_texture: empty_reference(text),
        has_resolved_region: false,
        resolved_region_id: text.copy(""),
        resolved_region_min_x: 0,
        resolved_region_min_y: 0,
        resolved_region_extent_x: 0,
        resolved_region_extent_y: 0,
        resolved_region_padding_left: 0,
        resolved_region_padding_right: 0,
        resolved_region_padding_bottom: 0,
        resolved_region_padding_top: 0,
        resolved_region_inset: NativeAuthoredAtlasInset::HalfTexel,
    }
}
fn resolved_surface_row(
    text: &mut Text,
    entry: &CatalogEntry,
    resolved: &ResolvedVoxelSurface,
) -> NativeAuthoredVoxelSurfaceReadout {
    let (
        mapping_kind,
        texture,
        atlas,
        region,
        scale,
        origin,
        texture_version,
        atlas_version,
        resolved_region,
    ) = match &resolved.mapping {
        ResolvedVoxelSurfaceMapping::Repeat {
            texture,
            texture_version,
            tile_scale_cells,
            tile_origin_cells,
        } => (
            NativeAuthoredVoxelSurfaceMappingKind::Repeat,
            native_reference(text, texture),
            empty_reference(text),
            text.copy(""),
            *tile_scale_cells,
            *tile_origin_cells,
            *texture_version,
            0,
            None,
        ),
        ResolvedVoxelSurfaceMapping::Atlas {
            atlas,
            atlas_version,
            texture,
            texture_version,
            region,
            tile_scale_cells,
            tile_origin_cells,
        } => (
            NativeAuthoredVoxelSurfaceMappingKind::Atlas,
            native_reference(text, texture),
            native_reference(text, atlas),
            text.copy(&region.id),
            *tile_scale_cells,
            *tile_origin_cells,
            *texture_version,
            *atlas_version,
            Some(region),
        ),
    };
    let (has_region, region_id, min, extent, padding, inset) = match resolved_region {
        Some(region) => (
            true,
            text.copy(&region.id),
            region.content_min,
            region.content_extent,
            region.padding,
            native_atlas_inset(region.inset),
        ),
        None => (
            false,
            text.copy(""),
            [0, 0],
            [0, 0],
            AtlasPadding::ZERO,
            NativeAuthoredAtlasInset::HalfTexel,
        ),
    };
    NativeAuthoredVoxelSurfaceReadout {
        material_entry_id: text.copy(entry.id.as_str()),
        schema_version: resolved.schema_version,
        mapping_kind,
        texture,
        atlas,
        region,
        tile_scale_x: scale[0],
        tile_scale_y: scale[1],
        tile_origin_x: origin[0],
        tile_origin_y: origin[1],
        alpha_mode: native_voxel_alpha_kind(resolved.alpha_mode),
        alpha_cutoff: voxel_alpha_cutoff(resolved.alpha_mode),
        has_resolved_mapping: true,
        resolved_texture_version: texture_version,
        resolved_atlas_version: atlas_version,
        resolved_filter: native_texture_filter(resolved.filter),
        resolved_wrap: native_texture_wrap(resolved.wrap),
        resolved_texture: native_reference(
            text,
            match &resolved.mapping {
                ResolvedVoxelSurfaceMapping::Repeat { texture, .. } => texture,
                ResolvedVoxelSurfaceMapping::Atlas { texture, .. } => texture,
            },
        ),
        has_resolved_region: has_region,
        resolved_region_id: region_id,
        resolved_region_min_x: min[0],
        resolved_region_min_y: min[1],
        resolved_region_extent_x: extent[0],
        resolved_region_extent_y: extent[1],
        resolved_region_padding_left: padding.left,
        resolved_region_padding_right: padding.right,
        resolved_region_padding_bottom: padding.bottom,
        resolved_region_padding_top: padding.top,
        resolved_region_inset: inset,
    }
}
fn empty_reference(text: &mut Text) -> NativeAuthoredAssetReference {
    NativeAuthoredAssetReference {
        id: text.copy(""),
        version_kind: NativeAssetVersionRequirementKind::Any,
        version: 0,
        has_hash: false,
        hash: text.copy(""),
    }
}
fn native_color(value: asset_catalog::Rgba) -> NativeColor {
    NativeColor {
        r: value.r,
        g: value.g,
        b: value.b,
        a: value.a,
    }
}
fn native_structural_class(value: StructuralClass) -> NativeAuthoredStructuralClass {
    match value {
        StructuralClass::Decorative => NativeAuthoredStructuralClass::Decorative,
        StructuralClass::Solid => NativeAuthoredStructuralClass::Solid,
        StructuralClass::Structural => NativeAuthoredStructuralClass::Structural,
    }
}
fn native_uv_strategy(value: UvStrategy) -> NativeAuthoredUvStrategy {
    match value {
        UvStrategy::Flat => NativeAuthoredUvStrategy::Flat,
        UvStrategy::Planar => NativeAuthoredUvStrategy::Planar,
        UvStrategy::Atlas => NativeAuthoredUvStrategy::Atlas,
    }
}
fn native_texture_filter(value: TextureFilter) -> NativeAuthoredTextureFilter {
    match value {
        TextureFilter::Nearest => NativeAuthoredTextureFilter::Nearest,
        TextureFilter::Linear => NativeAuthoredTextureFilter::Linear,
    }
}
fn native_texture_wrap(value: TextureWrap) -> NativeAuthoredTextureWrap {
    match value {
        TextureWrap::Clamp => NativeAuthoredTextureWrap::Clamp,
        TextureWrap::Repeat => NativeAuthoredTextureWrap::Repeat,
    }
}
fn native_atlas_inset(value: AtlasInset) -> NativeAuthoredAtlasInset {
    match value {
        AtlasInset::HalfTexel => NativeAuthoredAtlasInset::HalfTexel,
    }
}
fn native_voxel_alpha_kind(value: VoxelAlphaMode) -> NativeAuthoredVoxelAlphaModeKind {
    match value {
        VoxelAlphaMode::Opaque => NativeAuthoredVoxelAlphaModeKind::Opaque,
        VoxelAlphaMode::Mask { .. } => NativeAuthoredVoxelAlphaModeKind::Mask,
        VoxelAlphaMode::Blend => NativeAuthoredVoxelAlphaModeKind::Blend,
    }
}
fn voxel_alpha_cutoff(value: VoxelAlphaMode) -> f32 {
    match value {
        VoxelAlphaMode::Mask { cutoff } => cutoff,
        _ => 0.0,
    }
}
fn fallback_row(text: &mut Text, value: FallbackOutcome) -> NativeAuthoredFallbackReadout {
    match value {
        FallbackOutcome::UseFallback { reason, visual } => NativeAuthoredFallbackReadout {
            outcome: NativeAuthoredFallbackOutcomeKind::UseFallback,
            visual: match visual {
                FallbackVisual::MagentaSquare => NativeAuthoredFallbackVisual::MagentaSquare,
                FallbackVisual::GreyMaterial => NativeAuthoredFallbackVisual::GreyMaterial,
            },
            reason: text.copy(reason),
        },
        FallbackOutcome::FailClosed { reason } => NativeAuthoredFallbackReadout {
            outcome: NativeAuthoredFallbackOutcomeKind::FailClosed,
            visual: NativeAuthoredFallbackVisual::None,
            reason: text.copy(reason),
        },
        FallbackOutcome::Skip { reason } => NativeAuthoredFallbackReadout {
            outcome: NativeAuthoredFallbackOutcomeKind::Skip,
            visual: NativeAuthoredFallbackVisual::None,
            reason: text.copy(reason),
        },
    }
}
fn voxel_resolution_error(error: VoxelSurfaceResolutionError, source: &str) -> AuthoredError {
    let (code, message) = match error {
        VoxelSurfaceResolutionError::MissingAsset => (
            "AUTHORED_CONTENT_VOXEL_SURFACE_MISSING_ASSET",
            "voxel surface references an asset absent from the catalog",
        ),
        VoxelSurfaceResolutionError::StaleReference => (
            "AUTHORED_CONTENT_VOXEL_SURFACE_STALE_REFERENCE",
            "voxel surface reference no longer satisfies its catalog pin",
        ),
        VoxelSurfaceResolutionError::MissingTextureDefinition => (
            "AUTHORED_CONTENT_VOXEL_SURFACE_MISSING_TEXTURE_DEFINITION",
            "voxel surface target has no texture definition",
        ),
        VoxelSurfaceResolutionError::MissingAtlasDefinition => (
            "AUTHORED_CONTENT_VOXEL_SURFACE_MISSING_ATLAS_DEFINITION",
            "voxel surface target has no atlas definition",
        ),
        VoxelSurfaceResolutionError::MissingAtlasRegion => (
            "AUTHORED_CONTENT_VOXEL_SURFACE_MISSING_ATLAS_REGION",
            "voxel surface atlas has no requested region",
        ),
    };
    AuthoredError::Simple {
        code,
        message: message.into(),
        source: source.into(),
    }
}
fn entry_row(text: &mut Text, entry: &CatalogEntry) -> NativeAuthoredCatalogEntryReadout {
    NativeAuthoredCatalogEntryReadout {
        id: text.copy(entry.id.as_str()),
        kind: native_kind(entry.kind()),
        version: entry.version,
        has_hash: entry.hash.is_some(),
        hash: text.copy(entry.hash.as_ref().map_or("", AssetHash::as_str)),
        has_source_path: entry.source_path.is_some(),
        source_path: text.copy(entry.source_path.as_deref().unwrap_or("")),
        has_label: entry.label.is_some(),
        label: text.copy(entry.label.as_deref().unwrap_or("")),
        dependency_count: u32::try_from(entry.dependencies.len()).unwrap_or(u32::MAX),
    }
}
fn dependency_row(
    text: &mut Text,
    entry: &CatalogEntry,
    reference: &AssetReference,
) -> NativeAuthoredCatalogDependencyReadout {
    NativeAuthoredCatalogDependencyReadout {
        entry_id: text.copy(entry.id.as_str()),
        reference: native_reference(text, reference),
    }
}
fn native_reference(text: &mut Text, value: &AssetReference) -> NativeAuthoredAssetReference {
    NativeAuthoredAssetReference {
        id: text.copy(value.id().as_str()),
        version_kind: match value.version() {
            AssetVersionReq::Any => NativeAssetVersionRequirementKind::Any,
            AssetVersionReq::Exact(_) => NativeAssetVersionRequirementKind::Exact,
            AssetVersionReq::AtLeast(_) => NativeAssetVersionRequirementKind::AtLeast,
        },
        version: match value.version() {
            AssetVersionReq::Any => 0,
            AssetVersionReq::Exact(v) | AssetVersionReq::AtLeast(v) => v,
        },
        has_hash: value.hash().is_some(),
        hash: text.copy(value.hash().map_or("", AssetHash::as_str)),
    }
}
fn native_kind(kind: AssetKind) -> NativeAssetKind {
    match kind {
        AssetKind::Material => NativeAssetKind::Material,
        AssetKind::StaticMesh => NativeAssetKind::StaticMesh,
        AssetKind::AnimatedMesh => NativeAssetKind::AnimatedMesh,
        AssetKind::Sprite => NativeAssetKind::Sprite,
        AssetKind::SpriteSheet => NativeAssetKind::SpriteSheet,
        AssetKind::Texture => NativeAssetKind::Texture,
        AssetKind::AudioClip => NativeAssetKind::AudioClip,
        AssetKind::Font => NativeAssetKind::Font,
        AssetKind::VoxelVolume => NativeAssetKind::VoxelVolume,
        AssetKind::VoxelObject => NativeAssetKind::VoxelObject,
        AssetKind::Script => NativeAssetKind::Script,
        AssetKind::Scene => NativeAssetKind::Scene,
    }
}
fn admission_error(error: CatalogAdmissionError) -> AuthoredError {
    match error {
        CatalogAdmissionError::Validation(report) => {
            AuthoredError::Validation(report.diagnostics())
        }
        CatalogAdmissionError::Codec(error) => AuthoredError::Simple {
            code: "AUTHORED_CONTENT_CODEC",
            message: error.message,
            source: error.path,
        },
        CatalogAdmissionError::RevisionExhausted => {
            AuthoredError::simple("catalog revision exhausted")
        }
    }
}
fn parse_text(value: NativeUtf8Slice, field: &'static str) -> Result<String, String> {
    let value = unsafe { borrowed_utf8(value.bytes, value.len, field) }
        .map_err(|error| error.to_string())?;
    if value.len() > MAX_TEXT {
        return Err(format!("{field} exceeds engine bound"));
    }
    Ok(value.to_owned())
}
fn parse_id(value: NativeUtf8Slice, field: &'static str) -> Result<AssetId, String> {
    AssetId::parse(&parse_text(value, field)?).map_err(|error| error.to_string())
}
fn parse_hash(value: NativeUtf8Slice, field: &'static str) -> Result<AssetHash, String> {
    AssetHash::parse(&parse_text(value, field)?).map_err(|error| error.to_string())
}
fn find_entry_mut<'a>(
    values: &'a mut [CatalogEntry],
    id: &AssetId,
) -> Result<&'a mut CatalogEntry, AuthoredError> {
    values
        .iter_mut()
        .find(|entry| entry.id == *id)
        .ok_or_else(|| AuthoredError::simple("payload owner is absent from catalog entries"))
}
fn parse_optional_reference(
    present: bool,
    id: NativeUtf8Slice,
    version_kind: NativeAssetVersionRequirementKind,
    version: u32,
    has_hash: bool,
    hash: NativeUtf8Slice,
) -> Result<Option<AssetReference>, AuthoredError> {
    present
        .then(|| parse_reference_parts(id, version_kind, version, has_hash, hash))
        .transpose()
        .map_err(AuthoredError::simple)
}
fn rgba(value: NativeColor) -> asset_catalog::Rgba {
    asset_catalog::Rgba {
        r: value.r,
        g: value.g,
        b: value.b,
        a: value.a,
    }
}
fn structural_class(
    value: NativeAuthoredStructuralClass,
) -> Result<StructuralClass, AuthoredError> {
    match value {
        NativeAuthoredStructuralClass::Decorative => Ok(StructuralClass::Decorative),
        NativeAuthoredStructuralClass::Solid => Ok(StructuralClass::Solid),
        NativeAuthoredStructuralClass::Structural => Ok(StructuralClass::Structural),
    }
}
fn uv_strategy(value: NativeAuthoredUvStrategy) -> Result<UvStrategy, AuthoredError> {
    match value {
        NativeAuthoredUvStrategy::Flat => Ok(UvStrategy::Flat),
        NativeAuthoredUvStrategy::Planar => Ok(UvStrategy::Planar),
        NativeAuthoredUvStrategy::Atlas => Ok(UvStrategy::Atlas),
    }
}
fn texture_filter(value: NativeAuthoredTextureFilter) -> Result<TextureFilter, AuthoredError> {
    match value {
        NativeAuthoredTextureFilter::Nearest => Ok(TextureFilter::Nearest),
        NativeAuthoredTextureFilter::Linear => Ok(TextureFilter::Linear),
    }
}
fn texture_wrap(value: NativeAuthoredTextureWrap) -> Result<TextureWrap, AuthoredError> {
    match value {
        NativeAuthoredTextureWrap::Clamp => Ok(TextureWrap::Clamp),
        NativeAuthoredTextureWrap::Repeat => Ok(TextureWrap::Repeat),
    }
}
fn atlas_inset(value: NativeAuthoredAtlasInset) -> Result<AtlasInset, AuthoredError> {
    match value {
        NativeAuthoredAtlasInset::HalfTexel => Ok(AtlasInset::HalfTexel),
    }
}
fn voxel_alpha(
    value: NativeAuthoredVoxelAlphaModeKind,
    cutoff: f32,
) -> Result<VoxelAlphaMode, AuthoredError> {
    match value {
        NativeAuthoredVoxelAlphaModeKind::Opaque => Ok(VoxelAlphaMode::Opaque),
        NativeAuthoredVoxelAlphaModeKind::Mask => Ok(VoxelAlphaMode::Mask { cutoff }),
        NativeAuthoredVoxelAlphaModeKind::Blend => Ok(VoxelAlphaMode::Blend),
    }
}
fn asset_kind(value: NativeAssetKind) -> Result<AssetKind, AuthoredError> {
    match value {
        NativeAssetKind::Material => Ok(AssetKind::Material),
        NativeAssetKind::StaticMesh => Ok(AssetKind::StaticMesh),
        NativeAssetKind::AnimatedMesh => Ok(AssetKind::AnimatedMesh),
        NativeAssetKind::Sprite => Ok(AssetKind::Sprite),
        NativeAssetKind::SpriteSheet => Ok(AssetKind::SpriteSheet),
        NativeAssetKind::Texture => Ok(AssetKind::Texture),
        NativeAssetKind::AudioClip => Ok(AssetKind::AudioClip),
        NativeAssetKind::Font => Ok(AssetKind::Font),
        NativeAssetKind::VoxelVolume => Ok(AssetKind::VoxelVolume),
        NativeAssetKind::VoxelObject => Ok(AssetKind::VoxelObject),
        NativeAssetKind::Script => Ok(AssetKind::Script),
        NativeAssetKind::Scene => Ok(AssetKind::Scene),
    }
}
fn fallback_context(value: NativeAuthoredFallbackContext) -> Result<AssetContext, AuthoredError> {
    match value {
        NativeAuthoredFallbackContext::DebugOverlay => Ok(AssetContext::DebugOverlay),
        NativeAuthoredFallbackContext::CosmeticSurface => Ok(AssetContext::CosmeticSurface),
        NativeAuthoredFallbackContext::CollisionCritical => Ok(AssetContext::CollisionCritical),
        NativeAuthoredFallbackContext::BackgroundDecoration => {
            Ok(AssetContext::BackgroundDecoration)
        }
    }
}
fn parse_reference_parts(
    id: NativeUtf8Slice,
    version_kind: NativeAssetVersionRequirementKind,
    version: u32,
    has_hash: bool,
    hash: NativeUtf8Slice,
) -> Result<AssetReference, String> {
    let version = match version_kind {
        NativeAssetVersionRequirementKind::Any => AssetVersionReq::Any,
        NativeAssetVersionRequirementKind::Exact => AssetVersionReq::Exact(version),
        NativeAssetVersionRequirementKind::AtLeast => AssetVersionReq::AtLeast(version),
    };
    Ok(AssetReference::new(
        parse_id(id, "asset reference id")?,
        version,
        if has_hash {
            Some(parse_hash(hash, "asset reference hash")?)
        } else {
            None
        },
    ))
}
fn prefab_definition(
    row: &NativeAuthoredPrefabDefinitionInput,
) -> Result<PrefabDefinition, String> {
    Ok(PrefabDefinition {
        id: PrefabId::new(row.id),
        schema_version: row.schema_version,
        display_name: parse_text(row.display_name, "prefab display name")?,
        parts: vec![],
        part_roles: vec![],
        variant: if row.has_variant {
            Some(PrefabVariantDelta {
                variant_id: parse_text(row.variant_id, "prefab variant id")?,
                base: PrefabId::new(row.variant_base),
                removed_roles: vec![],
                overrides: vec![],
            })
        } else {
            None
        },
    })
}
fn find_prefab_definition_mut(
    registry: &mut PrefabRegistry,
    id: u64,
) -> Result<&mut PrefabDefinition, AuthoredError> {
    registry
        .definitions
        .iter_mut()
        .find(|definition| definition.id.raw() == id)
        .ok_or_else(|| AuthoredError::simple("prefab row refers to an absent definition"))
}
fn prefab_part(row: &NativeAuthoredPrefabPartInput) -> Result<PrefabPart, String> {
    Ok(PrefabPart {
        id: PrefabPartId::new(row.id),
        namespace: parse_text(row.namespace, "prefab part namespace")?,
        display_name: parse_text(row.display_name, "prefab part display name")?,
        parent: row.has_parent.then(|| PrefabPartId::new(row.parent_id)),
        transform: prefab_transform(row.transform),
        source: prefab_source(row.source_kind, row.source)?,
    })
}
fn prefab_source(
    kind: NativeAuthoredPrefabPartSourceKind,
    source: NativeUtf8Slice,
) -> Result<PrefabPartSource, String> {
    let source = parse_text(source, "prefab part source")?;
    Ok(match kind {
        NativeAuthoredPrefabPartSourceKind::Scene => PrefabPartSource::Scene { asset: source },
        NativeAuthoredPrefabPartSourceKind::EntityDefinition => {
            PrefabPartSource::EntityDefinition { stable_id: source }
        }
        NativeAuthoredPrefabPartSourceKind::VoxelObject => {
            PrefabPartSource::VoxelObject { asset: source }
        }
    })
}
fn prefab_transform(value: NativeTransform) -> PrefabTransform {
    PrefabTransform {
        translation: [
            value.translation.x,
            value.translation.y,
            value.translation.z,
        ],
        rotation: [
            value.rotation.x,
            value.rotation.y,
            value.rotation.z,
            value.rotation.w,
        ],
        scale: [value.scale.x, value.scale.y, value.scale.z],
    }
}
fn native_prefab_transform(value: PrefabTransform) -> NativeTransform {
    NativeTransform {
        translation: NativeVec3 {
            x: value.translation[0],
            y: value.translation[1],
            z: value.translation[2],
        },
        rotation: NativeQuat {
            x: value.rotation[0],
            y: value.rotation[1],
            z: value.rotation[2],
            w: value.rotation[3],
        },
        scale: NativeVec3 {
            x: value.scale[0],
            y: value.scale[1],
            z: value.scale[2],
        },
    }
}
fn prefab_override(row: &NativeAuthoredPrefabOverrideInput) -> Result<PrefabOverride, String> {
    Ok(PrefabOverride {
        target_role: parse_text(row.target_role, "prefab override target role")?,
        value: prefab_override_value(row.kind, row.transform, row.value, row.active)?,
    })
}
fn prefab_instance_override(
    row: &NativeAuthoredPrefabInstanceOverrideInput,
) -> Result<PrefabOverride, String> {
    Ok(PrefabOverride {
        target_role: parse_text(row.target_role, "prefab override target role")?,
        value: prefab_override_value(row.kind, row.transform, row.value, row.active)?,
    })
}
fn prefab_override_value(
    kind: NativeAuthoredPrefabOverrideKind,
    transform: NativeTransform,
    value: NativeUtf8Slice,
    active: bool,
) -> Result<PrefabOverrideValue, String> {
    Ok(match kind {
        NativeAuthoredPrefabOverrideKind::Transform => PrefabOverrideValue::Transform {
            transform: prefab_transform(transform),
        },
        NativeAuthoredPrefabOverrideKind::EntityDefinition => {
            PrefabOverrideValue::EntityDefinition {
                stable_id: parse_text(value, "entity-definition override value")?,
            }
        }
        NativeAuthoredPrefabOverrideKind::Asset => PrefabOverrideValue::Asset {
            asset: parse_text(value, "asset override value")?,
        },
        NativeAuthoredPrefabOverrideKind::Material => PrefabOverrideValue::Material {
            asset: parse_text(value, "material override value")?,
        },
        NativeAuthoredPrefabOverrideKind::Activation => PrefabOverrideValue::Activation { active },
    })
}
fn native_prefab_source(source: &PrefabPartSource) -> (NativeAuthoredPrefabPartSourceKind, &str) {
    match source {
        PrefabPartSource::Scene { asset } => (NativeAuthoredPrefabPartSourceKind::Scene, asset),
        PrefabPartSource::EntityDefinition { stable_id } => (
            NativeAuthoredPrefabPartSourceKind::EntityDefinition,
            stable_id,
        ),
        PrefabPartSource::VoxelObject { asset } => {
            (NativeAuthoredPrefabPartSourceKind::VoxelObject, asset)
        }
    }
}
fn prefab_definition_row(
    text: &mut Text,
    definition: &PrefabDefinition,
) -> NativeAuthoredPrefabDefinitionReadout {
    let (has_variant, variant_id, variant_base) = definition
        .variant
        .as_ref()
        .map(|variant| (true, variant.variant_id.as_str(), variant.base.raw()))
        .unwrap_or((false, "", 0));
    NativeAuthoredPrefabDefinitionReadout {
        id: definition.id.raw(),
        schema_version: definition.schema_version,
        display_name: text.copy(&definition.display_name),
        has_variant,
        variant_id: text.copy(variant_id),
        variant_base,
    }
}
fn prefab_part_row(
    text: &mut Text,
    prefab: PrefabId,
    part: &PrefabPart,
    material: Option<&str>,
    active: bool,
) -> NativeAuthoredPrefabPartReadout {
    let (source_kind, source) = native_prefab_source(&part.source);
    NativeAuthoredPrefabPartReadout {
        prefab_id: prefab.raw(),
        id: part.id.raw(),
        namespace: text.copy(&part.namespace),
        display_name: text.copy(&part.display_name),
        has_parent: part.parent.is_some(),
        parent_id: part.parent.map(PrefabPartId::raw).unwrap_or_default(),
        transform: native_prefab_transform(part.transform),
        source_kind,
        source: text.copy(source),
        has_material: material.is_some(),
        material: text.copy(material.unwrap_or("")),
        active,
    }
}
fn prefab_role_row(
    text: &mut Text,
    prefab: PrefabId,
    part: PrefabPartId,
    role: &str,
) -> NativeAuthoredPrefabRoleReadout {
    NativeAuthoredPrefabRoleReadout {
        prefab_id: prefab.raw(),
        part_id: part.raw(),
        role: text.copy(role),
    }
}
fn prefab_override_row(
    text: &mut Text,
    prefab: PrefabId,
    item: &PrefabOverride,
) -> NativeAuthoredPrefabOverrideReadout {
    let (kind, transform, value, active) = match &item.value {
        PrefabOverrideValue::Transform { transform } => (
            NativeAuthoredPrefabOverrideKind::Transform,
            native_prefab_transform(*transform),
            "",
            false,
        ),
        PrefabOverrideValue::EntityDefinition { stable_id } => (
            NativeAuthoredPrefabOverrideKind::EntityDefinition,
            native_prefab_transform(PrefabTransform::IDENTITY),
            stable_id.as_str(),
            false,
        ),
        PrefabOverrideValue::Asset { asset } => (
            NativeAuthoredPrefabOverrideKind::Asset,
            native_prefab_transform(PrefabTransform::IDENTITY),
            asset.as_str(),
            false,
        ),
        PrefabOverrideValue::Material { asset } => (
            NativeAuthoredPrefabOverrideKind::Material,
            native_prefab_transform(PrefabTransform::IDENTITY),
            asset.as_str(),
            false,
        ),
        PrefabOverrideValue::Activation { active } => (
            NativeAuthoredPrefabOverrideKind::Activation,
            native_prefab_transform(PrefabTransform::IDENTITY),
            "",
            *active,
        ),
    };
    NativeAuthoredPrefabOverrideReadout {
        prefab_id: prefab.raw(),
        target_role: text.copy(&item.target_role),
        kind,
        transform,
        value: text.copy(value),
        active,
    }
}
fn resolved_prefab_part_row(
    text: &mut Text,
    prefab: PrefabId,
    part: &content_store::ResolvedPrefabPart,
) -> NativeAuthoredPrefabPartReadout {
    let (source_kind, source) = native_prefab_source(&part.source);
    NativeAuthoredPrefabPartReadout {
        prefab_id: prefab.raw(),
        id: part.id.raw(),
        namespace: text.copy(&part.namespace),
        display_name: text.copy(&part.display_name),
        has_parent: part.parent.is_some(),
        parent_id: part.parent.map(PrefabPartId::raw).unwrap_or_default(),
        transform: native_prefab_transform(part.transform),
        source_kind,
        source: text.copy(source),
        has_material: part.material.is_some(),
        material: text.copy(part.material.as_deref().unwrap_or("")),
        active: part.active,
    }
}
pub(crate) fn api(bridge: &mut RuntimeAuthoredContentBridge) -> NativeAuthoredContentApi {
    NativeAuthoredContentApi {
        context: (bridge as *mut RuntimeAuthoredContentBridge).cast(),
        admit_catalog,
        admit_catalog_from_content,
        admit_catalog_payload,
        destroy_catalog,
        read_catalog,
        destroy_catalog_readout_lease,
        resolve_reference,
        destroy_resolved_entry_lease,
        resolve_material,
        destroy_material_resolution_lease,
        resolve_voxel_surface,
        destroy_voxel_surface_resolution_lease,
        resolve_fallback,
        destroy_fallback_lease,
        admit_prefab_registry,
        admit_prefab_registry_from_content,
        destroy_prefab_registry,
        read_prefab_registry,
        destroy_prefab_registry_readout_lease,
        resolve_prefab,
        destroy_resolved_prefab_lease,
        destroy_operation_diagnostic_lease,
    }
}
fn receipt(
    bridge: &mut RuntimeAuthoredContentBridge,
    operation: &[u8],
    error: AuthoredError,
) -> NativeOperationErrorReceipt {
    NativeOperationErrorReceipt {
        service: NativeUtf8Slice {
            bytes: SERVICE.as_ptr(),
            len: SERVICE.len(),
        },
        operation: NativeUtf8Slice {
            bytes: operation.as_ptr(),
            len: operation.len(),
        },
        status: 0,
        diagnostics: bridge
            .diagnostic(error)
            .unwrap_or(NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle::default(),
                diagnostics: std::ptr::null(),
                diagnostics_len: 0,
            }),
    }
}
unsafe extern "C" fn admit_catalog(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogAdmitRequest,
    result: *mut NativeAuthoredCatalogHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let entries = match unsafe {
        borrowed_slice(
            request.entries,
            request.entries_len,
            "authored catalog entries",
        )
    } {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let dependencies = match unsafe {
        borrowed_slice(
            request.dependencies,
            request.dependencies_len,
            "authored catalog dependencies",
        )
    } {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.admit_rows(entries, dependencies) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitCatalog", error) };
            0
        }
    }
}
unsafe extern "C" fn admit_catalog_from_content(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogFromContentRequest,
    result: *mut NativeAuthoredCatalogHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    let request = unsafe { *request };
    match bridge.admit_content(request.content) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitCatalogFromContent", error) };
            0
        }
    }
}
unsafe extern "C" fn admit_catalog_payload(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogPayloadAdmitRequest,
    result: *mut NativeAuthoredCatalogHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.admit_payload_rows(unsafe { *request }) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitCatalogPayload", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_catalog(
    context: *mut c_void,
    handle: NativeAuthoredCatalogHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.catalogs.remove(&handle.value).is_some())
}
unsafe extern "C" fn read_catalog(
    context: *mut c_void,
    handle: NativeAuthoredCatalogHandle,
    result: *mut NativeAuthoredCatalogReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.read_catalog(handle) {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_catalog_readout_lease(
    context: *mut c_void,
    handle: NativeAuthoredCatalogReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn resolve_reference(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogResolveRequest,
    result: *mut NativeAuthoredResolvedEntryLease,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.resolve(unsafe { *request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"ResolveReference", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_resolved_entry_lease(
    context: *mut c_void,
    handle: NativeAuthoredResolvedEntryLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.resolved_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn resolve_material(
    context: *mut c_void,
    request: *const NativeAuthoredMaterialResolveRequest,
    result: *mut NativeAuthoredMaterialResolutionLease,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.resolve_material(unsafe { *request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"ResolveMaterial", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_material_resolution_lease(
    context: *mut c_void,
    handle: NativeAuthoredMaterialResolutionLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.material_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn resolve_voxel_surface(
    context: *mut c_void,
    request: *const NativeAuthoredMaterialResolveRequest,
    result: *mut NativeAuthoredVoxelSurfaceResolutionLease,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.resolve_voxel_surface(unsafe { *request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"ResolveVoxelSurface", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_voxel_surface_resolution_lease(
    context: *mut c_void,
    handle: NativeAuthoredVoxelSurfaceResolutionLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.surface_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn resolve_fallback(
    context: *mut c_void,
    request: *const NativeAuthoredFallbackResolveRequest,
    result: *mut NativeAuthoredFallbackLease,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.resolve_fallback(unsafe { *request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"ResolveFallback", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_fallback_lease(
    context: *mut c_void,
    handle: NativeAuthoredFallbackLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.fallback_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn admit_prefab_registry(
    context: *mut c_void,
    request: *const NativeAuthoredPrefabRegistryAdmitRequest,
    result: *mut NativeAuthoredPrefabRegistryHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let definitions = match unsafe {
        borrowed_slice(
            request.definitions,
            request.definitions_len,
            "prefab definitions",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let parts = match unsafe { borrowed_slice(request.parts, request.parts_len, "prefab parts") } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let roles = match unsafe { borrowed_slice(request.roles, request.roles_len, "prefab roles") } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let removed_roles = match unsafe {
        borrowed_slice(
            request.removed_roles,
            request.removed_roles_len,
            "prefab removed roles",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let overrides = match unsafe {
        borrowed_slice(request.overrides, request.overrides_len, "prefab overrides")
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let entity_definition_ids = match unsafe {
        borrowed_slice(
            request.entity_definition_ids,
            request.entity_definition_ids_len,
            "prefab entity definition ids",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.admit_prefab_rows(
        request,
        definitions,
        parts,
        roles,
        removed_roles,
        overrides,
        entity_definition_ids,
    ) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitPrefabRegistry", error) };
            0
        }
    }
}
unsafe extern "C" fn admit_prefab_registry_from_content(
    context: *mut c_void,
    request: *const NativeAuthoredPrefabRegistryFromContentRequest,
    result: *mut NativeAuthoredPrefabRegistryHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let entity_definition_ids = match unsafe {
        borrowed_slice(
            request.entity_definition_ids,
            request.entity_definition_ids_len,
            "prefab entity definition ids",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.admit_prefab_content(request, entity_definition_ids) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitPrefabRegistryFromContent", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_prefab_registry(
    context: *mut c_void,
    handle: NativeAuthoredPrefabRegistryHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.prefab_registries.remove(&handle.value).is_some())
}
unsafe extern "C" fn read_prefab_registry(
    context: *mut c_void,
    handle: NativeAuthoredPrefabRegistryHandle,
    result: *mut NativeAuthoredPrefabRegistryReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.read_prefab_registry(handle) {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_prefab_registry_readout_lease(
    context: *mut c_void,
    handle: NativeAuthoredPrefabRegistryReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.prefab_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn resolve_prefab(
    context: *mut c_void,
    request: *const NativeAuthoredPrefabResolveRequest,
    result: *mut NativeAuthoredResolvedPrefabLease,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let instance_overrides = match unsafe {
        borrowed_slice(
            request.instance_overrides,
            request.instance_overrides_len,
            "prefab instance overrides",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.resolve_prefab_registry(request, instance_overrides) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"ResolvePrefab", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_resolved_prefab_lease(
    context: *mut c_void,
    handle: NativeAuthoredResolvedPrefabLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(
        handle.value != 0
            && bridge
                .resolved_prefab_leases
                .remove(&handle.value)
                .is_some(),
    )
}
unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.diagnostics.remove(&handle.value).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(value: &'static [u8]) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    #[test]
    fn admits_typed_and_retained_content_catalogs_through_named_callbacks() {
        use std::{collections::BTreeMap, sync::Arc};

        let source = AssetCatalog::from_entries(vec![CatalogEntry::new(
            AssetId::parse("scene/test").unwrap(),
            2,
        )
        .with_hash(AssetHash::parse("aabb").unwrap())
        .with_label("Test")]);
        let canonical = AdmittedAssetCatalog::admit(source)
            .unwrap()
            .canonical_json()
            .as_bytes()
            .to_vec();
        let mut resources = BTreeMap::new();
        resources.insert("catalog.json".to_owned(), Arc::from(canonical));
        let mut content = RuntimeContentBridge::new(resources);
        let content_api = crate::content::api(&mut content);
        let mut reference = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                (content_api.open_reference)(
                    content_api.context,
                    &NativeContentOpenRequest {
                        path: slice(b"catalog.json"),
                    },
                    &mut reference,
                )
            },
            ABI_OK
        );

        let mut bridge = RuntimeAuthoredContentBridge::new();
        bridge.bind_content(&content);
        let api = super::api(&mut bridge);

        let mut from_content = NativeAuthoredCatalogHandle::default();
        let mut receipt = NativeOperationErrorReceipt {
            service: slice(b""),
            operation: slice(b""),
            status: 0,
            diagnostics: NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle::default(),
                diagnostics: std::ptr::null(),
                diagnostics_len: 0,
            },
        };
        assert_eq!(
            unsafe {
                (api.admit_catalog_from_content)(
                    api.context,
                    &NativeAuthoredCatalogFromContentRequest { content: reference },
                    &mut from_content,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.diagnostics.handle.value, 0);
        let mut readout = NativeAuthoredCatalogReadoutLease {
            handle: NativeAuthoredCatalogReadoutLeaseHandle::default(),
            canonical_hash: slice(b""),
            entry_count: 0,
            entries: std::ptr::null(),
            entries_len: 0,
            dependencies: std::ptr::null(),
            dependencies_len: 0,
            materials: std::ptr::null(),
            materials_len: 0,
            textures: std::ptr::null(),
            textures_len: 0,
            voxel_atlases: std::ptr::null(),
            voxel_atlases_len: 0,
            atlas_regions: std::ptr::null(),
            atlas_regions_len: 0,
            voxel_surfaces: std::ptr::null(),
            voxel_surfaces_len: 0,
        };
        assert_eq!(
            unsafe { (api.read_catalog)(api.context, from_content, &mut readout) },
            ABI_OK
        );
        assert_eq!(readout.entry_count, 1);
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    (*readout.entries).id.bytes,
                    (*readout.entries).id.len,
                ))
            },
            "scene/test"
        );
        assert_eq!(
            unsafe { (api.destroy_catalog_readout_lease)(api.context, readout.handle) },
            ABI_OK
        );
        let mut resolved = NativeAuthoredResolvedEntryLease {
            handle: NativeAuthoredResolvedEntryLeaseHandle::default(),
            entry: std::ptr::null(),
            entry_len: 0,
            dependencies: std::ptr::null(),
            dependencies_len: 0,
            materials: std::ptr::null(),
            materials_len: 0,
            textures: std::ptr::null(),
            textures_len: 0,
            voxel_atlases: std::ptr::null(),
            voxel_atlases_len: 0,
            atlas_regions: std::ptr::null(),
            atlas_regions_len: 0,
            voxel_surfaces: std::ptr::null(),
            voxel_surfaces_len: 0,
        };
        assert_eq!(
            unsafe {
                (api.resolve_reference)(
                    api.context,
                    &NativeAuthoredCatalogResolveRequest {
                        catalog: from_content,
                        reference_id: slice(b"scene/test"),
                        reference_version_kind: NativeAssetVersionRequirementKind::Exact,
                        reference_version: 2,
                        reference_has_hash: true,
                        reference_hash: slice(b"aabb"),
                    },
                    &mut resolved,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(resolved.entry_len, 1);
        assert_eq!(
            unsafe { (api.destroy_resolved_entry_lease)(api.context, resolved.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_catalog)(api.context, from_content) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (content_api.destroy_reference)(content_api.context, reference) },
            ABI_OK
        );

        let entries = [NativeAuthoredCatalogEntryInput {
            id: slice(b"scene/test"),
            version: 2,
            has_hash: true,
            hash: slice(b"aabb"),
            has_source_path: false,
            source_path: slice(b""),
            has_label: true,
            label: slice(b"Test"),
        }];
        let mut typed = NativeAuthoredCatalogHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_catalog)(
                    api.context,
                    &NativeAuthoredCatalogAdmitRequest {
                        entries: entries.as_ptr(),
                        entries_len: entries.len(),
                        dependencies: std::ptr::null(),
                        dependencies_len: 0,
                    },
                    &mut typed,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { (api.destroy_catalog)(api.context, typed) }, ABI_OK);

        let material = [NativeAuthoredCatalogEntryInput {
            id: slice(b"material/test"),
            ..entries[0]
        }];
        assert_eq!(
            unsafe {
                (api.admit_catalog)(
                    api.context,
                    &NativeAuthoredCatalogAdmitRequest {
                        entries: material.as_ptr(),
                        entries_len: material.len(),
                        dependencies: std::ptr::null(),
                        dependencies_len: 0,
                    },
                    &mut typed,
                    &mut receipt,
                )
            },
            0
        );
        assert_ne!(receipt.diagnostics.handle.value, 0);
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
    }

    #[test]
    fn admits_and_projects_complete_payload_catalog_through_one_owner() {
        fn color(r: f32, g: f32, b: f32, a: f32) -> NativeColor {
            NativeColor { r, g, b, a }
        }
        fn reference(
            id: &'static [u8],
            hash: &'static [u8],
        ) -> (
            NativeUtf8Slice,
            NativeAssetVersionRequirementKind,
            u32,
            bool,
            NativeUtf8Slice,
        ) {
            (
                slice(id),
                NativeAssetVersionRequirementKind::Exact,
                1,
                true,
                slice(hash),
            )
        }

        let entries = [
            NativeAuthoredCatalogEntryInput {
                id: slice(b"material/stone"),
                version: 1,
                has_hash: true,
                hash: slice(b"aabb"),
                has_source_path: false,
                source_path: slice(b""),
                has_label: true,
                label: slice(b"Stone"),
            },
            NativeAuthoredCatalogEntryInput {
                id: slice(b"texture/stone"),
                version: 1,
                has_hash: true,
                hash: slice(b"ccdd"),
                has_source_path: false,
                source_path: slice(b""),
                has_label: false,
                label: slice(b""),
            },
            NativeAuthoredCatalogEntryInput {
                id: slice(b"sprite-sheet/stone"),
                version: 1,
                has_hash: true,
                hash: slice(b"eeff"),
                has_source_path: false,
                source_path: slice(b""),
                has_label: false,
                label: slice(b""),
            },
        ];
        let dependencies = [
            NativeAuthoredCatalogDependencyInput {
                entry_id: slice(b"material/stone"),
                reference_id: slice(b"texture/stone"),
                reference_version_kind: NativeAssetVersionRequirementKind::Exact,
                reference_version: 1,
                reference_has_hash: true,
                reference_hash: slice(b"ccdd"),
            },
            NativeAuthoredCatalogDependencyInput {
                entry_id: slice(b"material/stone"),
                reference_id: slice(b"sprite-sheet/stone"),
                reference_version_kind: NativeAssetVersionRequirementKind::Exact,
                reference_version: 1,
                reference_has_hash: true,
                reference_hash: slice(b"eeff"),
            },
            NativeAuthoredCatalogDependencyInput {
                entry_id: slice(b"sprite-sheet/stone"),
                reference_id: slice(b"texture/stone"),
                reference_version_kind: NativeAssetVersionRequirementKind::Exact,
                reference_version: 1,
                reference_has_hash: true,
                reference_hash: slice(b"ccdd"),
            },
        ];
        let (texture_id, texture_version_kind, texture_version, texture_has_hash, texture_hash) =
            reference(b"texture/stone", b"ccdd");
        let materials = [NativeAuthoredMaterialInput {
            entry_id: slice(b"material/stone"),
            solid: true,
            collidable: true,
            occludes: true,
            structural_class: NativeAuthoredStructuralClass::Solid,
            color: color(0.5, 0.5, 0.5, 1.0),
            has_texture: true,
            texture_id,
            texture_version_kind,
            texture_version,
            texture_has_hash,
            texture_hash,
            roughness: 1.0,
            texture_tint: color(1.0, 1.0, 1.0, 1.0),
            emission_color: color(0.0, 0.0, 0.0, 1.0),
            emissive: 0.0,
            uv_strategy: NativeAuthoredUvStrategy::Atlas,
        }];
        let textures = [NativeAuthoredTextureInput {
            entry_id: slice(b"texture/stone"),
            width: 16,
            height: 16,
            filter: NativeAuthoredTextureFilter::Linear,
            wrap: NativeAuthoredTextureWrap::Clamp,
        }];
        let atlases = [NativeAuthoredVoxelAtlasInput {
            entry_id: slice(b"sprite-sheet/stone"),
            schema_version: 1,
            texture_id,
            texture_version_kind,
            texture_version,
            texture_has_hash,
            texture_hash,
        }];
        let regions = [NativeAuthoredAtlasRegionInput {
            atlas_entry_id: slice(b"sprite-sheet/stone"),
            id: slice(b"stone"),
            content_min_x: 2,
            content_min_y: 2,
            content_extent_x: 8,
            content_extent_y: 8,
            padding_left: 1,
            padding_right: 1,
            padding_bottom: 1,
            padding_top: 1,
            inset: NativeAuthoredAtlasInset::HalfTexel,
        }];
        let (atlas_id, atlas_version_kind, atlas_version, atlas_has_hash, atlas_hash) =
            reference(b"sprite-sheet/stone", b"eeff");
        let surfaces = [NativeAuthoredVoxelSurfaceInput {
            material_entry_id: slice(b"material/stone"),
            schema_version: 1,
            mapping_kind: NativeAuthoredVoxelSurfaceMappingKind::Atlas,
            texture_id: slice(b""),
            texture_version_kind: NativeAssetVersionRequirementKind::Any,
            texture_version: 0,
            texture_has_hash: false,
            texture_hash: slice(b""),
            atlas_id,
            atlas_version_kind,
            atlas_version,
            atlas_has_hash,
            atlas_hash,
            region: slice(b"stone"),
            tile_scale_x: 1.0,
            tile_scale_y: 1.0,
            tile_origin_x: 0.0,
            tile_origin_y: 0.0,
            alpha_mode: NativeAuthoredVoxelAlphaModeKind::Opaque,
            alpha_cutoff: 0.0,
        }];
        let mut bridge = RuntimeAuthoredContentBridge::new();
        let api = super::api(&mut bridge);
        let request = NativeAuthoredCatalogPayloadAdmitRequest {
            entries: entries.as_ptr(),
            entries_len: entries.len(),
            dependencies: dependencies.as_ptr(),
            dependencies_len: dependencies.len(),
            materials: materials.as_ptr(),
            materials_len: materials.len(),
            textures: textures.as_ptr(),
            textures_len: textures.len(),
            voxel_atlases: atlases.as_ptr(),
            voxel_atlases_len: atlases.len(),
            atlas_regions: regions.as_ptr(),
            atlas_regions_len: regions.len(),
            voxel_surfaces: surfaces.as_ptr(),
            voxel_surfaces_len: surfaces.len(),
        };
        let mut catalog = NativeAuthoredCatalogHandle::default();
        let mut receipt = NativeOperationErrorReceipt {
            service: slice(b""),
            operation: slice(b""),
            status: 0,
            diagnostics: NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle::default(),
                diagnostics: std::ptr::null(),
                diagnostics_len: 0,
            },
        };
        assert_eq!(
            unsafe {
                (api.admit_catalog_payload)(api.context, &request, &mut catalog, &mut receipt)
            },
            ABI_OK
        );
        let mut readout = NativeAuthoredCatalogReadoutLease {
            handle: NativeAuthoredCatalogReadoutLeaseHandle::default(),
            canonical_hash: slice(b""),
            entry_count: 0,
            entries: std::ptr::null(),
            entries_len: 0,
            dependencies: std::ptr::null(),
            dependencies_len: 0,
            materials: std::ptr::null(),
            materials_len: 0,
            textures: std::ptr::null(),
            textures_len: 0,
            voxel_atlases: std::ptr::null(),
            voxel_atlases_len: 0,
            atlas_regions: std::ptr::null(),
            atlas_regions_len: 0,
            voxel_surfaces: std::ptr::null(),
            voxel_surfaces_len: 0,
        };
        assert_eq!(
            unsafe { (api.read_catalog)(api.context, catalog, &mut readout) },
            ABI_OK
        );
        assert_eq!(
            (
                readout.entry_count,
                readout.materials_len,
                readout.textures_len,
                readout.voxel_atlases_len,
                readout.atlas_regions_len,
                readout.voxel_surfaces_len
            ),
            (3, 1, 1, 1, 1, 1)
        );
        assert_eq!(
            unsafe { (api.destroy_catalog_readout_lease)(api.context, readout.handle) },
            ABI_OK
        );
        let material_request = NativeAuthoredMaterialResolveRequest {
            catalog,
            material_id: slice(b"material/stone"),
        };
        let mut material = NativeAuthoredMaterialResolutionLease {
            handle: NativeAuthoredMaterialResolutionLeaseHandle::default(),
            materials: std::ptr::null(),
            materials_len: 0,
            voxel_surfaces: std::ptr::null(),
            voxel_surfaces_len: 0,
        };
        assert_eq!(
            unsafe {
                (api.resolve_material)(api.context, &material_request, &mut material, &mut receipt)
            },
            ABI_OK
        );
        assert_eq!(
            (material.materials_len, material.voxel_surfaces_len),
            (1, 1)
        );
        assert_eq!(
            unsafe { (api.destroy_material_resolution_lease)(api.context, material.handle) },
            ABI_OK
        );
        let mut surface = NativeAuthoredVoxelSurfaceResolutionLease {
            handle: NativeAuthoredVoxelSurfaceResolutionLeaseHandle::default(),
            surfaces: std::ptr::null(),
            surfaces_len: 0,
        };
        assert_eq!(
            unsafe {
                (api.resolve_voxel_surface)(
                    api.context,
                    &material_request,
                    &mut surface,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { (*surface.surfaces).has_resolved_mapping }, true);
        assert_eq!(unsafe { (*surface.surfaces).has_resolved_region }, true);
        assert_eq!(unsafe { (*surface.surfaces).resolved_texture_version }, 1);
        assert_eq!(unsafe { (*surface.surfaces).resolved_atlas_version }, 1);
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    (*surface.surfaces).resolved_texture.id.bytes,
                    (*surface.surfaces).resolved_texture.id.len,
                ))
            },
            "texture/stone"
        );
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    (*surface.surfaces).resolved_region_id.bytes,
                    (*surface.surfaces).resolved_region_id.len,
                ))
            },
            "stone"
        );
        assert_eq!(
            unsafe { (api.destroy_voxel_surface_resolution_lease)(api.context, surface.handle) },
            ABI_OK
        );
        let mut fallback = NativeAuthoredFallbackLease {
            handle: NativeAuthoredFallbackLeaseHandle::default(),
            outcomes: std::ptr::null(),
            outcomes_len: 0,
        };
        assert_eq!(
            unsafe {
                (api.resolve_fallback)(
                    api.context,
                    &NativeAuthoredFallbackResolveRequest {
                        kind: NativeAssetKind::Material,
                        context: NativeAuthoredFallbackContext::CosmeticSurface,
                    },
                    &mut fallback,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (*fallback.outcomes).outcome },
            NativeAuthoredFallbackOutcomeKind::UseFallback
        );
        assert_eq!(
            unsafe { (api.destroy_fallback_lease)(api.context, fallback.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_catalog)(api.context, catalog) },
            ABI_OK
        );
    }

    #[test]
    fn admits_and_resolves_typed_prefab_registry_through_authored_content() {
        fn transform(x: f32) -> NativeTransform {
            NativeTransform {
                translation: NativeVec3 { x, y: 0.0, z: 0.0 },
                rotation: NativeQuat {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                scale: NativeVec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            }
        }
        let mut bridge = RuntimeAuthoredContentBridge::new();
        let api = super::api(&mut bridge);
        let mut receipt = NativeOperationErrorReceipt {
            service: slice(b""),
            operation: slice(b""),
            status: 0,
            diagnostics: NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle::default(),
                diagnostics: std::ptr::null(),
                diagnostics_len: 0,
            },
        };
        let catalog_rows = [NativeAuthoredCatalogEntryInput {
            id: slice(b"scene/test"),
            version: 1,
            has_hash: false,
            hash: slice(b""),
            has_source_path: false,
            source_path: slice(b""),
            has_label: false,
            label: slice(b""),
        }];
        let mut catalog = NativeAuthoredCatalogHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_catalog)(
                    api.context,
                    &NativeAuthoredCatalogAdmitRequest {
                        entries: catalog_rows.as_ptr(),
                        entries_len: catalog_rows.len(),
                        dependencies: std::ptr::null(),
                        dependencies_len: 0,
                    },
                    &mut catalog,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        let definitions = [
            NativeAuthoredPrefabDefinitionInput {
                id: 1,
                schema_version: 1,
                display_name: slice(b"Base"),
                has_variant: false,
                variant_id: slice(b""),
                variant_base: 0,
            },
            NativeAuthoredPrefabDefinitionInput {
                id: 2,
                schema_version: 1,
                display_name: slice(b"Night"),
                has_variant: true,
                variant_id: slice(b"night"),
                variant_base: 1,
            },
        ];
        let parts = [NativeAuthoredPrefabPartInput {
            prefab_id: 1,
            id: 10,
            namespace: slice(b"body/root"),
            display_name: slice(b"Body"),
            has_parent: false,
            parent_id: 0,
            transform: transform(0.0),
            source_kind: NativeAuthoredPrefabPartSourceKind::Scene,
            source: slice(b"scene/test"),
        }];
        let roles = [NativeAuthoredPrefabRoleInput {
            prefab_id: 1,
            role: slice(b"body/root"),
            part_id: 10,
        }];
        let variant_overrides = [NativeAuthoredPrefabOverrideInput {
            prefab_id: 2,
            target_role: slice(b"body/root"),
            kind: NativeAuthoredPrefabOverrideKind::Activation,
            transform: transform(0.0),
            value: slice(b""),
            active: false,
        }];
        let request = NativeAuthoredPrefabRegistryAdmitRequest {
            schema_version: 1,
            catalog,
            definitions: definitions.as_ptr(),
            definitions_len: definitions.len(),
            parts: parts.as_ptr(),
            parts_len: parts.len(),
            roles: roles.as_ptr(),
            roles_len: roles.len(),
            removed_roles: std::ptr::null(),
            removed_roles_len: 0,
            overrides: variant_overrides.as_ptr(),
            overrides_len: variant_overrides.len(),
            entity_definition_ids: std::ptr::null(),
            entity_definition_ids_len: 0,
        };
        let mut registry = NativeAuthoredPrefabRegistryHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_prefab_registry)(api.context, &request, &mut registry, &mut receipt)
            },
            ABI_OK
        );
        let mut inspection: NativeAuthoredPrefabRegistryReadoutLease =
            unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.read_prefab_registry)(api.context, registry, &mut inspection) },
            ABI_OK
        );
        assert_eq!(
            (
                inspection.definitions_len,
                inspection.parts_len,
                inspection.roles_len,
                inspection.overrides_len
            ),
            (2, 1, 1, 1)
        );
        assert_eq!(
            unsafe { (api.destroy_prefab_registry_readout_lease)(api.context, inspection.handle) },
            ABI_OK
        );
        let instance_overrides = [NativeAuthoredPrefabInstanceOverrideInput {
            target_role: slice(b"body/root"),
            kind: NativeAuthoredPrefabOverrideKind::Transform,
            transform: transform(3.0),
            value: slice(b""),
            active: false,
        }];
        let mut resolved: NativeAuthoredResolvedPrefabLease = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.resolve_prefab)(
                    api.context,
                    &NativeAuthoredPrefabResolveRequest {
                        registry,
                        prefab_id: 2,
                        instance_overrides: instance_overrides.as_ptr(),
                        instance_overrides_len: instance_overrides.len(),
                    },
                    &mut resolved,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(
            (resolved.requested_id, resolved.base_id, resolved.parts_len),
            (2, 1, 1)
        );
        assert!(resolved.has_variant);
        assert_eq!(unsafe { (*resolved.parts).transform.translation.x }, 3.0);
        assert!(!unsafe { (*resolved.parts).active });
        assert_eq!(
            unsafe { (api.destroy_resolved_prefab_lease)(api.context, resolved.handle) },
            ABI_OK
        );
        let bad_parts = [NativeAuthoredPrefabPartInput {
            source: slice(b"scene/missing"),
            ..parts[0]
        }];
        let mut rejected = NativeAuthoredPrefabRegistryHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_prefab_registry)(
                    api.context,
                    &NativeAuthoredPrefabRegistryAdmitRequest {
                        parts: bad_parts.as_ptr(),
                        ..request
                    },
                    &mut rejected,
                    &mut receipt,
                )
            },
            0
        );
        assert_ne!(receipt.diagnostics.handle.value, 0);
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_prefab_registry)(api.context, registry) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_catalog)(api.context, catalog) },
            ABI_OK
        );
    }
}
