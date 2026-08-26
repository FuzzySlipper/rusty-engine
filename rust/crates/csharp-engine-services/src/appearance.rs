use crate::composition::{borrowed_slice, borrowed_utf8, CsharpEngineServicesError, ABI_OK};
use csharp_engine_abi::*;
use render_model::*;
use render_projection::{
    Appearance, RuntimeAppearanceCatalog, RuntimeAppearanceFact, RuntimeAppearanceProjector,
};
use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

// Renderer resources cross into the product-development host after C# has
// selected them. Keep the pre-split per-resource ceiling at the selection
// boundary so a successful product call is already host-admissible.
const MAX_RENDER_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_INLINE_MESH_RESOURCE_BYTES: u32 = MAX_RENDER_RESOURCE_BYTES as u32;

/// Immutable renderer content selected through the Engine appearance API.
/// Host bundle realization remains the runtime's responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsharpRenderResource {
    kind: CsharpRenderResourceKind,
    identity: String,
    content_hash: String,
    path: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsharpRenderResourceKind {
    Texture,
    Mesh,
}

impl CsharpRenderResource {
    fn admit_texture(path: String, bytes: Vec<u8>) -> Result<Self, CsharpEngineServicesError> {
        let path = renderer_path(path, ".png")?;
        let descriptor = TextureDescriptor::admit_png_rgba8_resource(
            "texture/csharp-product".to_owned(),
            &bytes,
            TextureFilter::Nearest,
            TextureWrap::Clamp,
            1,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_TEXTURE",
                format!("renderer resource is not an admitted PNG: {error:?}"),
            )
        })?;
        let content_hash = descriptor
            .content_hash
            .expect("resource-backed texture has a content hash");
        let identity = match descriptor
            .payload
            .expect("resource-backed texture has a payload")
            .source
        {
            render_model::TexturePayloadSource::Resource { resource } => resource,
            render_model::TexturePayloadSource::Inline { .. } => unreachable!("resource admission"),
        };
        admit_bundle_resource(&path, &bytes)?;
        Ok(Self {
            kind: CsharpRenderResourceKind::Texture,
            identity,
            content_hash,
            path,
            bytes,
        })
    }

    fn admit_mesh(path: String, bytes: Vec<u8>) -> Result<Self, CsharpEngineServicesError> {
        let path = renderer_path(path, ".rmesh")?;
        validate_mesh_resource_header(&bytes).map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_MESH",
                format!("mesh resource header is invalid: {error:?}"),
            )
        })?;
        let content_hash = mesh_resource_content_hash(&bytes);
        let identity = format!(
            "mesh-resource/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("Engine mesh hash uses SHA-256")
        );
        admit_bundle_resource(&path, &bytes)?;
        Ok(Self {
            kind: CsharpRenderResourceKind::Mesh,
            identity,
            content_hash,
            path,
            bytes,
        })
    }

    pub const fn kind(&self) -> CsharpRenderResourceKind {
        self.kind
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn renderer_path(path: String, extension: &str) -> Result<String, CsharpEngineServicesError> {
    if !path.starts_with("content/") || !path.ends_with(extension) {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_RENDER_RESOURCE_PATH",
            "renderer resource must use its fixed content path and media extension",
        ));
    }
    normalize_bundle_path(&path)
}

fn admit_bundle_resource(path: &str, bytes: &[u8]) -> Result<(), CsharpEngineServicesError> {
    // The old host conversion admitted this same path and byte body through a
    // product-dev bundle entry after media validation. The service owns that
    // selection-time decision now; the runtime conversion merely represents it.
    normalize_bundle_path(path)?;
    if bytes.len() > MAX_RENDER_RESOURCE_BYTES {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_RENDER_RESOURCE_SIZE",
            "renderer resource exceeds the maximum byte length",
        ));
    }
    Ok(())
}

fn normalize_bundle_path(value: &str) -> Result<String, CsharpEngineServicesError> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_RENDER_RESOURCE_PATH",
            "renderer resource path must be a bounded normalized relative ASCII path",
        ));
    }
    Ok(value.to_owned())
}

#[derive(Clone)]
pub(crate) struct RuntimeAppearanceState {
    projector: RuntimeAppearanceProjector,
    appearances: BTreeMap<u64, String>,
    next_appearance: u64,
    pub(crate) render_resources: Vec<CsharpRenderResource>,
    resource_paths: BTreeMap<String, u64>,
    resource_identities: BTreeMap<String, u64>,
}

pub(crate) struct RuntimeAppearanceCall {
    pub(crate) state: RuntimeAppearanceState,
    pub(crate) frame: Option<render_model::RenderFrameDiff>,
}

/// Engine-owned appearance admission and retained projection for trusted C# products.
/// `Create` selects the immutable renderer resources the browser host will serve; calls stage
/// both resource selection, newly admitted appearances, and snapshots so a failure cannot partly
/// advance renderer-visible state.
pub(crate) struct RuntimeAppearanceBridge {
    pub(crate) state: RuntimeAppearanceState,
    content_resources: BTreeMap<String, Arc<[u8]>>,
    selection_sealed: bool,
    staged: Option<RuntimeAppearanceCall>,
    callback_error: Option<CsharpEngineServicesError>,
}

impl RuntimeAppearanceBridge {
    pub(crate) fn new(
        catalog: RuntimeAppearanceCatalog,
        content_resources: BTreeMap<String, Arc<[u8]>>,
    ) -> Self {
        Self {
            state: RuntimeAppearanceState {
                projector: RuntimeAppearanceProjector::new(catalog),
                appearances: BTreeMap::new(),
                next_appearance: 1,
                render_resources: Vec::new(),
                resource_paths: BTreeMap::new(),
                resource_identities: BTreeMap::new(),
            },
            content_resources,
            selection_sealed: false,
            staged: None,
            callback_error: None,
        }
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeAppearanceCall {
            state: self.state.clone(),
            frame: None,
        });
        self.callback_error = None;
    }

    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<Option<RuntimeAppearanceCall>, CsharpEngineServicesError> {
        if let Some(error) = self.callback_error.take() {
            self.staged = None;
            return Err(error);
        }
        Ok(self.staged.take())
    }

    pub(crate) fn commit(&mut self, staged: Option<RuntimeAppearanceCall>) {
        if let Some(staged) = staged {
            self.state = staged.state;
        }
    }

    pub(crate) fn seal_resource_selection(&mut self) {
        self.selection_sealed = true;
        self.content_resources.clear();
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeAppearanceCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_CALL",
                "appearance service was called outside a product call",
            )
        })
    }

    fn open_resource(
        &mut self,
        request: &NativeRenderResourceRequest,
    ) -> Result<NativeRenderResourceInfo, CsharpEngineServicesError> {
        // SAFETY: the borrowed path is copied before the direct callback returns.
        let requested_path = unsafe {
            borrowed_utf8(request.path.bytes, request.path.len, "resource path")?.to_owned()
        };
        if let Some(handle) = self
            .staged
            .as_ref()
            .and_then(|staged| staged.state.resource_paths.get(&requested_path))
            .copied()
        {
            return self.resource_info(handle);
        }
        if self.selection_sealed {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_SELECTION_CLOSED",
                format!(
                    "renderer resource `{requested_path}` was not selected during product Create"
                ),
            ));
        }
        let relative_path = requested_path
            .strip_prefix("content/")
            .unwrap_or(&requested_path)
            .to_owned();
        let bytes = self
            .content_resources
            .get(&relative_path)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_UNKNOWN",
                    format!("product content has no renderer resource `{requested_path}`"),
                )
            })?;
        let browser_path = format!("content/{relative_path}");
        let resource = match () {
            _ if relative_path.ends_with(".png") => {
                CsharpRenderResource::admit_texture(browser_path.clone(), bytes.to_vec())
            }
            _ if relative_path.ends_with(".rmesh") => {
                CsharpRenderResource::admit_mesh(browser_path.clone(), bytes.to_vec())
            }
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_KIND",
                    format!(
                        "renderer resource `{requested_path}` must be an RGBA PNG or packed .rmesh file"
                    ),
                ))
            }
        }?;
        let handle =
            self.stage_resource(resource, [browser_path, relative_path, requested_path])?;
        self.resource_info(handle)
    }

    fn stage_resource(
        &mut self,
        resource: CsharpRenderResource,
        paths: impl IntoIterator<Item = String>,
    ) -> Result<u64, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let handle = if let Some(handle) = staged
            .state
            .resource_identities
            .get(resource.identity())
            .copied()
        {
            handle
        } else {
            let handle = u64::try_from(staged.state.render_resources.len())
                .map_err(|_| {
                    CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_HANDLE",
                        "renderer resource handle overflowed",
                    )
                })?
                .checked_add(1)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_HANDLE",
                        "renderer resource handle overflowed",
                    )
                })?;
            let identity = resource.identity().to_owned();
            staged.state.render_resources.push(resource);
            staged.state.resource_identities.insert(identity, handle);
            handle
        };
        for path in paths {
            staged.state.resource_paths.insert(path, handle);
        }
        Ok(handle)
    }

    fn resource_info(
        &self,
        handle: u64,
    ) -> Result<NativeRenderResourceInfo, CsharpEngineServicesError> {
        let resource = self.resource(handle)?;
        Ok(NativeRenderResourceInfo {
            handle: NativeRenderResourceHandle { value: handle },
            kind: match resource.kind() {
                CsharpRenderResourceKind::Texture => 1,
                CsharpRenderResourceKind::Mesh => 2,
            },
            byte_length: u32::try_from(resource.bytes().len()).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_SIZE",
                    "renderer resource byte length exceeded u32",
                )
            })?,
        })
    }

    fn resource(&self, handle: u64) -> Result<&CsharpRenderResource, CsharpEngineServicesError> {
        let index = usize::try_from(handle.saturating_sub(1)).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "invalid resource handle",
            )
        })?;
        let state = self
            .staged
            .as_ref()
            .map(|staged| &staged.state)
            .unwrap_or(&self.state);
        state.render_resources.get(index).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "unknown resource handle",
            )
        })
    }

    fn allocate_appearance(
        &mut self,
        appearance: Appearance,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let handle = staged.state.next_appearance;
        staged.state.next_appearance = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_APPEARANCE_HANDLE", "appearance handle overflow")
        })?;
        let identity = format!("appearance/native-{handle}");
        staged
            .state
            .projector
            .insert_appearance(identity.clone(), appearance);
        staged.state.appearances.insert(handle, identity);
        Ok(NativeAppearanceHandle { value: handle })
    }

    fn create_primitive(
        &mut self,
        request: NativePrimitiveAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let geometry = match request.geometry {
            1 => Geometry::Cube,
            2 => Geometry::Sphere,
            3 => Geometry::Quad,
            4 => Geometry::Point,
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_PRIMITIVE_GEOMETRY",
                    "unknown primitive geometry",
                ))
            }
        };
        self.allocate_appearance(Appearance::Primitive {
            geometry,
            material: Material {
                color: native_color(request.color),
                wireframe: request.wireframe != 0,
            },
        })
    }

    unsafe fn create_static_mesh(
        &mut self,
        request: &NativeStaticMeshAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let resource = self.resource(request.resource.value)?.clone();
        if resource.kind() != CsharpRenderResourceKind::Mesh {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_RESOURCE",
                "static mesh appearance requires a mesh resource",
            ));
        }
        let native_groups = borrowed_slice(request.groups, request.groups_len, "mesh groups")?;
        let mut groups = Vec::with_capacity(native_groups.len());
        let mut slots = BTreeMap::new();
        for group in native_groups {
            let material_slot = u16::try_from(group.material_slot).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_SLOT",
                    "mesh material slot exceeded u16",
                )
            })?;
            groups.push(MeshGroupDescriptor {
                material_slot,
                start: group.start,
                count: group.count,
            });
            slots.insert(material_slot, ());
        }
        let handle = self.staged_mut()?.state.next_appearance;
        let mesh_id = format!("mesh/native-{handle}");
        let mut material_slots = Vec::with_capacity(slots.len());
        let mut materials = Vec::with_capacity(slots.len());
        for slot in slots.keys().copied() {
            let material = format!("material/native-{handle}-{slot}");
            material_slots.push(MeshMaterialSlot {
                slot,
                material: material.clone(),
            });
            materials.push(render_material(material, request.color));
        }
        let uvs = (request.uvs_byte_offset != 0).then_some(request.uvs_byte_offset);
        let colors = (request.colors_byte_offset != 0).then_some(request.colors_byte_offset);
        let mut attributes = vec![
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
        ];
        if uvs.is_some() {
            attributes.push(MeshAttribute {
                name: MeshAttributeName::Uv,
                components: 2,
                kind: MeshAttributeKind::F32,
            });
        }
        if colors.is_some() {
            attributes.push(MeshAttribute {
                name: MeshAttributeName::Color,
                components: 4,
                kind: MeshAttributeKind::F32,
            });
        }
        let encoding = match request.encoding {
            1 => MeshResourceEncoding::PackedStreamsLeV1,
            2 => MeshResourceEncoding::PackedStreamsLeV2,
            3 => MeshResourceEncoding::PackedStreamsLeV3,
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_ENCODING",
                    "unknown packed mesh encoding",
                ))
            }
        };
        let byte_length = u32::try_from(resource.bytes().len()).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_SIZE",
                "mesh resource byte length exceeded u32",
            )
        })?;
        let asset = StaticMeshAsset {
            asset: mesh_id.clone(),
            payload: MeshPayloadDescriptor {
                layout: MeshBufferLayout {
                    vertex_count: request.vertex_count,
                    index_count: request.index_count,
                    index_width: MeshIndexWidth::U32,
                    attributes,
                },
                groups,
                bounds: MeshBoundsDescriptor {
                    min: native_vec3_array(request.bounds_min),
                    max: native_vec3_array(request.bounds_max),
                },
                source: MeshPayloadSource::Resource {
                    resource: resource.identity().to_owned(),
                    content_hash: resource.content_hash().to_owned(),
                    byte_length,
                    encoding,
                    positions_byte_offset: request.positions_byte_offset,
                    normals_byte_offset: request.normals_byte_offset,
                    uvs_byte_offset: uvs,
                    colors_byte_offset: colors,
                    indices_byte_offset: request.indices_byte_offset,
                },
                provenance: MeshProvenance::StaticAsset,
            },
            material_slots: material_slots.clone(),
            collision: MeshCollisionPolicy::VisualOnly,
        };
        {
            let resources = self.staged_mut()?.state.projector.resources_mut();
            resources.materials.extend(materials);
            resources.static_meshes.push(asset);
        }
        self.allocate_appearance(Appearance::StaticMesh {
            asset: mesh_id,
            material_overrides: Vec::new(),
        })
    }

    fn create_static_mesh_from_content(
        &mut self,
        request: &NativeStaticMeshContentAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        // SAFETY: the borrowed path is copied before the direct callback returns.
        let requested_path = unsafe {
            borrowed_utf8(
                request.path.bytes,
                request.path.len,
                "static mesh content path",
            )?
            .to_owned()
        };
        if self.selection_sealed {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_SELECTION_CLOSED",
                format!(
                    "static mesh content `{requested_path}` was not selected during product Create"
                ),
            ));
        }
        let relative_path = requested_path
            .strip_prefix("content/")
            .unwrap_or(&requested_path)
            .to_owned();
        let bytes = self.content_resources.get(&relative_path).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_CONTENT_UNKNOWN",
                format!("product content has no static mesh document `{requested_path}`"),
            )
        })?;
        let asset = serde_json::from_slice::<StaticMeshAsset>(bytes).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_STATIC_MESH_CONTENT_JSON", error.to_string())
        })?;
        asset.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_STATIC_MESH_CONTENT", format!("{error:?}"))
        })?;
        if !matches!(asset.payload.source, MeshPayloadSource::Inline { .. }) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_CONTENT_INLINE",
                "static mesh content must use an inline payload",
            ));
        }
        let mut packed = pack_mesh_resources(&[asset.payload], MAX_INLINE_MESH_RESOURCE_BYTES)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_STATIC_MESH_PACK", format!("{error:?}"))
            })?;
        let payload = packed.payloads.pop().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_PACK",
                "inline mesh pack returned no payload",
            )
        })?;
        let packed_resource = packed.resources.pop().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_PACK",
                "inline mesh pack returned no resource",
            )
        })?;
        let content_hash = packed_resource
            .content_hash
            .strip_prefix("sha256:")
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_PACK",
                    "packed mesh resource had an invalid content hash",
                )
            })?;
        let browser_path = format!("content/engine-mesh/{content_hash}.rmesh");
        let resource =
            CsharpRenderResource::admit_mesh(browser_path.clone(), packed_resource.bytes)?;
        self.stage_resource(resource, [browser_path])?;
        self.create_retained_static_mesh(payload, asset.material_slots, request.color)
    }

    fn create_retained_static_mesh(
        &mut self,
        payload: MeshPayloadDescriptor,
        source_material_slots: Vec<MeshMaterialSlot>,
        color: NativeColor,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let handle = self.staged_mut()?.state.next_appearance;
        let mesh_id = format!("mesh/native-{handle}");
        let mut material_slots = Vec::with_capacity(source_material_slots.len());
        let mut materials = Vec::with_capacity(source_material_slots.len());
        for source_slot in source_material_slots {
            let material = format!("material/native-{handle}-{}", source_slot.slot);
            material_slots.push(MeshMaterialSlot {
                slot: source_slot.slot,
                material: material.clone(),
            });
            materials.push(render_material(material, color));
        }
        {
            let resources = self.staged_mut()?.state.projector.resources_mut();
            resources.materials.extend(materials);
            resources.static_meshes.push(StaticMeshAsset {
                asset: mesh_id.clone(),
                payload,
                material_slots,
                collision: MeshCollisionPolicy::VisualOnly,
            });
        }
        self.allocate_appearance(Appearance::StaticMesh {
            asset: mesh_id,
            material_overrides: Vec::new(),
        })
    }

    fn create_sprite(
        &mut self,
        request: NativeSpriteAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let resource = self.resource(request.texture.value)?.clone();
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_RESOURCE",
                "sprite appearance requires a texture resource",
            ));
        }
        let handle = self.staged_mut()?.state.next_appearance;
        let texture_id = format!("texture/native-{handle}");
        let atlas_id = format!("sprite/native-{handle}");
        let texture = TextureDescriptor::admit_png_rgba8_resource(
            texture_id.clone(),
            resource.bytes(),
            TextureFilter::Nearest,
            TextureWrap::Clamp,
            1,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_TEXTURE", format!("{error:?}"))
        })?;
        let atlas = SpriteAtlasDescriptor {
            id: atlas_id.clone(),
            texture: texture_id,
            frames: vec![SpriteFrameRect {
                frame: 0,
                uv_min: native_vec2(request.uv_min),
                uv_max: native_vec2(request.uv_max),
                size: None,
            }],
        };
        {
            let resources = self.staged_mut()?.state.projector.resources_mut();
            resources.textures.push(texture);
            resources.sprite_atlases.push(atlas);
        }
        let billboard = match request.billboard {
            0 => BillboardMode::None,
            1 => BillboardMode::Spherical,
            2 => BillboardMode::Cylindrical,
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_BILLBOARD",
                    "unknown sprite billboard mode",
                ))
            }
        };
        self.allocate_appearance(Appearance::Sprite {
            sprite: SpriteInstanceDescriptor {
                asset: atlas_id,
                frame: 0,
                pivot: native_vec2(request.pivot),
                size: native_vec2(request.size),
                size_mode: SpriteSizeMode::World,
                billboard,
                tint: native_color(request.tint),
                render_order: request.render_order,
                depth: SpriteDepthPolicy::Default,
                shading: SpriteShading::Unlit,
                material: SpriteMaterialDescriptor::default(),
                visible: true,
                transform: Transform::IDENTITY,
                attachment: SpriteAttachment::default(),
                metadata: RenderMetadata::default(),
            },
        })
    }

    unsafe fn stage_snapshot(
        &mut self,
        facts: *const NativeAppearanceFact,
        fact_count: usize,
    ) -> Result<(), CsharpEngineServicesError> {
        if fact_count > 0 && facts.is_null() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VISUAL_FACTS_POINTER",
                "C# visual snapshot had facts without a facts pointer",
            ));
        }
        // SAFETY: a non-empty snapshot was checked above and the callback is synchronous.
        let facts = if fact_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(facts, fact_count) }
        };
        let appearances = self.staged_mut()?.state.appearances.clone();
        let mut owned = Vec::with_capacity(facts.len());
        for fact in facts {
            let appearance = appearances.get(&fact.appearance.value).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_APPEARANCE_HANDLE",
                    "visual fact used an unknown appearance handle",
                )
            })?;
            owned.push(RuntimeAppearanceFact {
                object_id: fact.object_id,
                appearance: appearance.clone(),
                transform: Transform {
                    translation: [
                        fact.transform.translation.x,
                        fact.transform.translation.y,
                        fact.transform.translation.z,
                    ],
                    rotation: [
                        fact.transform.rotation.x,
                        fact.transform.rotation.y,
                        fact.transform.rotation.z,
                        fact.transform.rotation.w,
                    ],
                    scale: [
                        fact.transform.scale.x,
                        fact.transform.scale.y,
                        fact.transform.scale.z,
                    ],
                },
                visible: fact.visible != 0,
            });
        }
        let staged = self.staged_mut()?;
        let frame = staged
            .state
            .projector
            .project(&owned)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_VISUAL_SNAPSHOT", format!("{error:?}"))
            })?
            .frame;
        staged.frame = Some(frame);
        Ok(())
    }
}

pub(crate) fn create(
    catalog: RuntimeAppearanceCatalog,
    content_resources: BTreeMap<String, Arc<[u8]>>,
) -> RuntimeAppearanceBridge {
    RuntimeAppearanceBridge::new(catalog, content_resources)
}

pub(crate) unsafe extern "C" fn open_render_resource(
    context: *mut c_void,
    request: *const NativeRenderResourceRequest,
    result: *mut NativeRenderResourceInfo,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    match bridge.open_resource(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn create_primitive_appearance(
    context: *mut c_void,
    request: NativePrimitiveAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    appearance_result(context, result, |bridge| bridge.create_primitive(request))
}

pub(crate) unsafe extern "C" fn create_static_mesh_appearance(
    context: *mut c_void,
    request: *const NativeStaticMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    appearance_result(context, result, |bridge| unsafe {
        bridge.create_static_mesh(&*request)
    })
}

pub(crate) unsafe extern "C" fn create_static_mesh_from_content_appearance(
    context: *mut c_void,
    request: *const NativeStaticMeshContentAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    appearance_result(context, result, |bridge| {
        bridge.create_static_mesh_from_content(unsafe { &*request })
    })
}

pub(crate) unsafe extern "C" fn create_sprite_appearance(
    context: *mut c_void,
    request: NativeSpriteAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    appearance_result(context, result, |bridge| bridge.create_sprite(request))
}

fn appearance_result(
    context: *mut c_void,
    result: *mut NativeAppearanceHandle,
    action: impl FnOnce(
        &mut RuntimeAppearanceBridge,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError>,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    match action(bridge) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn publish_appearance_snapshot(
    context: *mut c_void,
    facts: *const NativeAppearanceFact,
    fact_count: usize,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context points at a box retained by `CsharpProductRuntime`.
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    // SAFETY: callback inputs are copied/validated before this method returns.
    match unsafe { bridge.stage_snapshot(facts, fact_count) } {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

fn native_vec2(value: NativeVec2) -> [f32; 2] {
    [value.x, value.y]
}

fn native_vec3_array(value: NativeVec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn native_color(value: NativeColor) -> [f32; 4] {
    [value.r, value.g, value.b, value.a]
}

fn render_material(id: String, color: NativeColor) -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        schema_version: 1,
        id,
        color: native_color(color),
        texture: None,
        roughness: 1.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: MaterialAlphaModeDescriptor::Opaque,
        double_sided: false,
        voxel_surface: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RGBA_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 0, 68,
        255, 25, 26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
        130,
    ];

    fn resource_request(path: &'static str) -> NativeRenderResourceRequest {
        NativeRenderResourceRequest {
            path: NativeUtf8Slice {
                bytes: path.as_ptr(),
                len: path.len(),
            },
        }
    }

    #[test]
    fn selected_resources_are_transactional_deduplicated_and_create_time_only() {
        let mut content_resources = BTreeMap::new();
        content_resources.insert("selected.png".to_owned(), Arc::from(RGBA_PNG));
        content_resources.insert(
            "unselected.png".to_owned(),
            Arc::from(&b"not an RGBA PNG"[..]),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);

        bridge.begin_call();
        bridge
            .open_resource(&resource_request("selected.png"))
            .expect("selected RGBA texture");
        assert_eq!(
            bridge.staged.as_ref().unwrap().state.render_resources.len(),
            1
        );
        bridge.discard_call();
        assert!(bridge.state.render_resources.is_empty());

        bridge.begin_call();
        let selected = bridge
            .open_resource(&resource_request("selected.png"))
            .expect("selected RGBA texture");
        let alias = bridge
            .open_resource(&resource_request("content/selected.png"))
            .expect("selected resource alias");
        assert_eq!(selected.handle.value, alias.handle.value);
        assert_eq!(
            bridge.staged.as_ref().unwrap().state.render_resources.len(),
            1
        );
        let staged = bridge.take_staged_call().expect("staged call");
        bridge.commit(staged);

        bridge.begin_call();
        assert!(bridge
            .open_resource(&resource_request("unselected.png"))
            .is_err());
        bridge.discard_call();
        assert_eq!(bridge.state.render_resources.len(), 1);

        bridge.seal_resource_selection();
        bridge.begin_call();
        assert_eq!(
            bridge
                .open_resource(&resource_request("selected.png"))
                .expect("already selected resource")
                .handle
                .value,
            selected.handle.value
        );
        assert_eq!(
            bridge
                .open_resource(&resource_request("unselected.png"))
                .expect_err("new selection is closed")
                .code(),
            "CSHARP_RENDER_RESOURCE_SELECTION_CLOSED"
        );
    }

    #[test]
    fn inline_static_mesh_content_packs_one_selected_resource_for_retained_appearances() {
        let document = StaticMeshAsset {
            asset: "mesh/test".to_owned(),
            payload: MeshPayloadDescriptor {
                layout: MeshBufferLayout {
                    vertex_count: 3,
                    index_count: 3,
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
                groups: vec![MeshGroupDescriptor {
                    material_slot: 0,
                    start: 0,
                    count: 3,
                }],
                bounds: MeshBoundsDescriptor {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 0.0],
                },
                source: MeshPayloadSource::Inline {
                    positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                    uvs: None,
                    colors: None,
                    indices: vec![0, 1, 2],
                },
                provenance: MeshProvenance::StaticAsset,
            },
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/test".to_owned(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        };
        let mut content_resources = BTreeMap::new();
        content_resources.insert(
            "mesh.json".to_owned(),
            Arc::from(serde_json::to_vec(&document).expect("mesh JSON")),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let request = NativeStaticMeshContentAppearanceRequest {
            path: NativeUtf8Slice {
                bytes: b"mesh.json".as_ptr(),
                len: b"mesh.json".len(),
            },
            color: NativeColor {
                r: 0.2,
                g: 0.3,
                b: 0.4,
                a: 1.0,
            },
        };

        bridge.begin_call();
        bridge
            .create_static_mesh_from_content(&request)
            .expect("first retained appearance");
        bridge
            .create_static_mesh_from_content(&request)
            .expect("second retained appearance");
        let staged = bridge.take_staged_call().expect("staged static meshes");
        bridge.commit(staged);

        assert_eq!(bridge.state.render_resources.len(), 1);
        let resources = bridge.state.projector.resources_mut();
        assert_eq!(resources.static_meshes.len(), 2);
        assert_eq!(resources.materials.len(), 2);
    }
}
