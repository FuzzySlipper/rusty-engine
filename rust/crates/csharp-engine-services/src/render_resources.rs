//! Renderer resource admission and the resource registry. Graphics, audio,
//! video and offline output read the same admitted bodies, and implicit
//! surfaces and spatial collision reach generated meshes here; the decoders
//! and slot bookkeeping stay private.

use crate::appearance::{
    animation_result, appearance_operation, appearance_void, RuntimeAppearanceBridge,
};
use crate::composition::{borrowed_slice, CsharpEngineServicesError};
use crate::content::RetainedContent;
use asset_import::{
    admit_glb_source, glb_relative_resource_uris, import_animated_glb_asset, GlbSourceClosure,
    GltfResource, ImportContext, SourceUri,
};
use csharp_engine_abi::{
    NativeMeshMaterialBinding, NativeMeshPartitionHandle, NativeMeshPartitionPartRequest,
    NativeMeshPartitionReadout, NativeMeshPartitionRequest, NativeMeshResourceCreateRequest,
    NativeMeshResourceHandle, NativeOperationErrorReceipt, NativeRenderResourceHandle,
    NativeRenderResourceInfo, NativeRenderResourceKind, NativeTextureColorSpace,
    NativeTextureFilter, NativeTextureWrap, NativeVec3,
};
use render_model::{
    pack_mesh_resources, partition_mesh_spatially, validate_mesh_resource_header,
    AnimatedMeshAsset, MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
    MeshBufferLayout, MeshCollisionPolicy, MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot,
    MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, PackedMeshResource, StaticMeshAsset,
    TextureDescriptor, TextureFilter, TextureWrap, MAX_MESH_RESOURCE_BYTES,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    sync::Arc,
};

/// Immutable renderer content selected through the Engine appearance API.
/// Host bundle realization remains the runtime's responsibility.
#[derive(Debug, Clone, PartialEq)]
pub struct CsharpRenderResource {
    kind: CsharpRenderResourceKind,
    identity: String,
    content_hash: String,
    path: String,
    bytes: Arc<[u8]>,
    texture: Option<TextureDescriptor>,
    animated_mesh: Option<AnimatedMeshAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsharpRenderResourceKind {
    Texture,
    Mesh,
    Font,
    Audio,
    Video,
    AnimatedMesh,
    AnimationClipPack,
    Shader,
    StarCatalogue,
}

impl CsharpRenderResource {
    /// A PNG texture from its content identity and the dimensions its first
    /// admission read.
    fn admit_texture(
        path: String,
        bytes: Arc<[u8]>,
        content_hash: String,
        dimensions: [u32; 2],
        filter: NativeTextureFilter,
        wrap: NativeTextureWrap,
        color_space: NativeTextureColorSpace,
    ) -> Self {
        let digest = &content_hash["sha256:".len()..];
        let mut identity = format!("texture/csharp-product-{digest}");
        if filter != NativeTextureFilter::Nearest || wrap != NativeTextureWrap::Clamp {
            identity.push_str(&format!("-f{}-w{}", filter as u32, wrap as u32));
        }
        if color_space == NativeTextureColorSpace::Linear {
            identity.push_str("-linear");
        }
        let resource_identity = format!("texture-resource/{digest}");
        let mut descriptor = TextureDescriptor::png_rgba8_resource(
            identity,
            content_hash.clone(),
            bytes.len() as u32,
            dimensions,
            match filter {
                NativeTextureFilter::Nearest => TextureFilter::Nearest,
                NativeTextureFilter::Linear => TextureFilter::Linear,
            },
            match wrap {
                NativeTextureWrap::Clamp => TextureWrap::Clamp,
                NativeTextureWrap::Repeat => TextureWrap::Repeat,
            },
            1,
        );
        if color_space == NativeTextureColorSpace::Linear {
            if let Some(payload) = descriptor.payload.as_mut() {
                payload.color_space = render_model::TextureColorSpace::Linear;
            }
        }
        Self {
            kind: CsharpRenderResourceKind::Texture,
            identity: resource_identity,
            content_hash,
            path,
            bytes,
            texture: Some(descriptor),
            animated_mesh: None,
        }
    }

    fn admit_mesh(
        path: String,
        bytes: Arc<[u8]>,
        content_hash: String,
    ) -> Result<Self, CsharpEngineServicesError> {
        validate_mesh_resource_header(&bytes).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_RENDER_RESOURCE_MESH", format!("{error:?}"))
        })?;
        Ok(Self {
            kind: CsharpRenderResourceKind::Mesh,
            identity: format!("mesh-resource/{}", &content_hash["sha256:".len()..]),
            content_hash,
            path,
            bytes,
            texture: None,
            animated_mesh: None,
        })
    }

    fn from_packed_mesh(path: String, packed: PackedMeshResource) -> Self {
        Self {
            kind: CsharpRenderResourceKind::Mesh,
            identity: packed.resource,
            content_hash: packed.content_hash,
            path,
            bytes: Arc::from(packed.bytes),
            texture: None,
            animated_mesh: None,
        }
    }

    fn admit_font(
        path: String,
        bytes: Arc<[u8]>,
        content_hash: String,
    ) -> Result<Self, CsharpEngineServicesError> {
        if bytes.get(..4) != Some(b"wOF2") {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_FONT",
                "font resource is not an admitted WOFF2 body",
            ));
        }
        Ok(Self {
            kind: CsharpRenderResourceKind::Font,
            identity: format!("font/{}", &content_hash["sha256:".len()..]),
            content_hash,
            path,
            bytes,
            texture: None,
            animated_mesh: None,
        })
    }

    /// A product shader: UTF-8 WGSL that composes with the standard shader
    /// family (render-shaders), checked now so its error names the content
    /// file and line.
    fn admit_shader(
        path: String,
        relative_path: &str,
        bytes: Arc<[u8]>,
        content_hash: String,
        keywords: &[String],
    ) -> Result<Self, CsharpEngineServicesError> {
        let source = std::str::from_utf8(&bytes).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_SHADER",
                format!("shader `{relative_path}` is not UTF-8 text"),
            )
        })?;
        render_shaders::check_product_shader(relative_path, source, keywords)
            .map_err(|error| CsharpEngineServicesError::new("CSHARP_SHADER", error))?;
        // Each keyword set is its own variant, so its own resource.
        let identity = format!(
            "shader/{}{}",
            &content_hash["sha256:".len()..],
            keywords
                .iter()
                .map(|keyword| format!("+{keyword}"))
                .collect::<String>()
        );
        Ok(Self {
            kind: CsharpRenderResourceKind::Shader,
            identity,
            content_hash,
            path,
            bytes,
            texture: None,
            animated_mesh: None,
        })
    }

    /// A star catalogue: `rusty.stars.v1` JSON (`render_model::stars`),
    /// checked now and packed into the binary resource the renderer reads.
    fn admit_star_catalogue(
        path: String,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<Self, CsharpEngineServicesError> {
        use sha2::{Digest, Sha256};

        let stars = render_model::parse_star_catalogue(bytes).map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_STAR_CATALOGUE",
                format!("star catalogue `{relative_path}` is not admitted: {error:?}"),
            )
        })?;
        let packed = render_model::pack_star_catalogue(&stars);
        let content_hash = format!("sha256:{:x}", Sha256::digest(&packed));
        let identity = format!(
            "stars/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("SHA-256 prefix")
        );
        Ok(Self {
            kind: CsharpRenderResourceKind::StarCatalogue,
            identity,
            content_hash,
            path,
            bytes: Arc::from(packed),
            texture: None,
            animated_mesh: None,
        })
    }

    pub(crate) fn admit_audio(
        path: String,
        bytes: Arc<[u8]>,
        content_hash: String,
    ) -> Result<Self, CsharpEngineServicesError> {
        let container = render_model::AudioContainer::identify(&bytes).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_CONTAINER",
                render_model::AUDIO_CONTAINER_POLICY,
            )
        })?;
        require_extension(&path, container.extension())?;
        Ok(Self {
            kind: CsharpRenderResourceKind::Audio,
            identity: format!("audio-resource/{}", &content_hash["sha256:".len()..]),
            content_hash,
            path,
            bytes,
            texture: None,
            animated_mesh: None,
        })
    }

    pub(crate) fn admit_video(
        path: String,
        bytes: Arc<[u8]>,
        content_hash: String,
    ) -> Result<Self, CsharpEngineServicesError> {
        require_extension(&path, ".webm")?;
        if bytes.get(..4) != Some(&[0x1a, 0x45, 0xdf, 0xa3])
            || !bytes[..bytes.len().min(1024)]
                .windows(4)
                .any(|window| window == b"webm")
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VIDEO_RESOURCE_WEBM",
                "video resource must be an admitted WebM EBML body",
            ));
        }
        Ok(Self {
            kind: CsharpRenderResourceKind::Video,
            identity: format!("video-resource/{}", &content_hash["sha256:".len()..]),
            content_hash,
            path,
            bytes,
            texture: None,
            animated_mesh: None,
        })
    }

    fn admit_animated_mesh(
        path: String,
        bytes: Vec<u8>,
    ) -> Result<Self, CsharpEngineServicesError> {
        require_extension(&path, ".glb")?;
        let relative_path = path
            .strip_prefix("content/")
            .expect("renderer path retains content prefix");
        let outcome = import_animated_glb_asset(
            &SourceUri::RelativePath(relative_path.to_owned()),
            &bytes,
            &ImportContext::default(),
        );
        let imported = outcome.assets.ok_or_else(|| {
            let detail = outcome
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GLB_ADMISSION",
                if detail.is_empty() {
                    "animated GLB admission produced no asset".to_owned()
                } else {
                    detail
                },
            )
        })?;
        let content_hash = imported
            .animated_mesh
            .content_hash
            .clone()
            .expect("animated import assigns the content digest");
        let identity = format!(
            "animated-mesh-resource/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("SHA-256 prefix")
        );
        Ok(Self {
            kind: CsharpRenderResourceKind::AnimatedMesh,
            identity,
            content_hash,
            path,
            bytes: Arc::from(imported.runtime_resource_bytes),
            texture: None,
            animated_mesh: Some(imported.animated_mesh),
        })
    }

    pub const fn kind(&self) -> CsharpRenderResourceKind {
        self.kind
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub(crate) fn asset_identity(&self) -> &str {
        self.texture
            .as_ref()
            .map(|texture| texture.id.as_str())
            .unwrap_or(&self.identity)
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
    /// Shares the immutable body with Engine host delivery without copying it.
    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }
    pub(crate) fn texture(&self) -> Option<&TextureDescriptor> {
        self.texture.as_ref()
    }
    pub(crate) fn animated_mesh(&self) -> Option<&AnimatedMeshAsset> {
        self.animated_mesh.as_ref()
    }
    /// A shader resource's renderer definition, its errors naming `path`.
    pub(crate) fn shader(&self) -> Option<render_model::ShaderDescriptor> {
        (self.kind == CsharpRenderResourceKind::Shader).then(|| render_model::ShaderDescriptor {
            id: self.identity.clone(),
            path: self
                .path
                .strip_prefix("content/")
                .unwrap_or(&self.path)
                .to_owned(),
            source: String::from_utf8_lossy(&self.bytes).into_owned(),
            keywords: self
                .identity
                .split('+')
                .skip(1)
                .map(str::to_owned)
                .collect(),
        })
    }

    /// A mesh body for tests that only need a large admitted resource.
    #[cfg(test)]
    pub(crate) fn test_mesh(bytes: Arc<[u8]>) -> Self {
        Self {
            kind: CsharpRenderResourceKind::Mesh,
            identity: "mesh-resource/performance-probe".to_owned(),
            content_hash: format!("sha256:{}", "ab".repeat(32)),
            path: "content/performance-probe.rmesh".to_owned(),
            bytes,
            texture: None,
            animated_mesh: None,
        }
    }
}

/// A renderer path with the sampling it was opened with.
type RendererPath = (String, NativeTextureFilter, NativeTextureWrap);

/// Admitted renderer resources by handle. Monotonic slots keep stale handles
/// invalid while released payloads leave memory.
#[derive(Clone, Default)]
pub(crate) struct RenderResourceRegistry {
    entries: BTreeMap<usize, CsharpRenderResource>,
    next: usize,
    /// Bodies released during the current call, and during the call before
    /// it. The renderer reads a body after it receives the call's output, so
    /// a texture defined and released within one call (#8771) stays servable
    /// for one more call instead of forcing a renderer rebaseline.
    released_this_call: Vec<CsharpRenderResource>,
    released_last_call: Vec<CsharpRenderResource>,
    paths: BTreeMap<RendererPath, u64>,
    /// The renderer paths mapped to each handle, so removing it costs its own.
    handle_paths: BTreeMap<u64, Vec<RendererPath>>,
    /// Live resources by asset identity (a texture's descriptor id).
    identities: BTreeMap<String, u64>,
    /// Live resources by body identity. Textures of one image with different
    /// sampling share a body, so an identity can name several handles.
    bodies: BTreeMap<String, Vec<u64>>,
    /// Live animated meshes, and how many clip packs are live, for the
    /// animation readout.
    animated: BTreeSet<u64>,
    clip_packs: usize,
    /// Each explicit resource admission gives the caller one releasable owner.
    open_counts: BTreeMap<u64, u32>,
    /// How many Engine holders (appearances, materials, atlases and the like)
    /// hold each resource.
    holds: BTreeMap<u64, u32>,
    /// Resources that may have no owner left: freshly staged, or released by
    /// their last holder. [`Self::take_unowned`] looks only at these, so a
    /// release costs the released resources, not every live holder.
    unheld: Vec<u64>,
}

impl RenderResourceRegistry {
    /// Starts a product call. Bodies released by the previous call stay
    /// servable through this one.
    pub(crate) fn begin_call(&mut self) {
        self.released_last_call = std::mem::take(&mut self.released_this_call);
    }

    /// Released bodies the renderer may still read for recent output.
    pub(crate) fn recently_released(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        self.released_this_call
            .iter()
            .chain(&self.released_last_call)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        #[cfg(test)]
        crate::appearance::RESOURCE_INVENTORY_READS.with(|count| count.set(count.get() + 1));
        self.entries.values()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// No live resource, and no renderer path naming one.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.paths.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn first(&self) -> Option<&CsharpRenderResource> {
        self.entries.values().next()
    }

    pub(crate) fn get(&self, handle: u64) -> Option<&CsharpRenderResource> {
        self.entries
            .get(&usize::try_from(handle.saturating_sub(1)).ok()?)
    }

    /// The live resource with this asset identity.
    pub(crate) fn by_asset_identity(&self, identity: &str) -> Option<&CsharpRenderResource> {
        self.get(*self.identities.get(identity)?)
    }

    /// A live resource whose body has this identity.
    pub(crate) fn by_identity(&self, identity: &str) -> Option<&CsharpRenderResource> {
        self.get(*self.bodies.get(identity)?.first()?)
    }

    /// Live animated meshes.
    pub(crate) fn animated_meshes(&self) -> impl Iterator<Item = &AnimatedMeshAsset> {
        self.animated
            .iter()
            .filter_map(|handle| self.get(*handle)?.animated_mesh())
    }

    /// How many animation clip packs are live.
    pub(crate) fn clip_pack_count(&self) -> usize {
        self.clip_packs
    }

    pub(crate) fn resource(
        &self,
        handle: u64,
    ) -> Result<&CsharpRenderResource, CsharpEngineServicesError> {
        let index = usize::try_from(handle.saturating_sub(1)).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "invalid resource handle",
            )
        })?;
        self.entries.get(&index).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "unknown resource handle",
            )
        })
    }

    /// The imported descriptor of a live animated resource, which clip-pack
    /// association extends in place.
    pub(crate) fn animated_mesh_mut(&mut self, handle: u64) -> Option<&mut AnimatedMeshAsset> {
        self.entries
            .get_mut(&usize::try_from(handle.saturating_sub(1)).ok()?)?
            .animated_mesh
            .as_mut()
    }

    /// Stages a resource under the handle of an identical live asset, or a
    /// fresh one, and maps each renderer path to it.
    pub(crate) fn stage(
        &mut self,
        resource: CsharpRenderResource,
        paths: impl IntoIterator<Item = String>,
    ) -> Result<u64, CsharpEngineServicesError> {
        let (filter, wrap) = resource.texture().map_or(
            (NativeTextureFilter::Nearest, NativeTextureWrap::Clamp),
            |texture| {
                (
                    match texture.filter {
                        TextureFilter::Nearest => NativeTextureFilter::Nearest,
                        TextureFilter::Linear => NativeTextureFilter::Linear,
                    },
                    match texture.wrap {
                        TextureWrap::Clamp => NativeTextureWrap::Clamp,
                        TextureWrap::Repeat => NativeTextureWrap::Repeat,
                    },
                )
            },
        );
        let handle = if let Some(handle) = self.identities.get(resource.asset_identity()).copied() {
            handle
        } else {
            let handle = u64::try_from(self.next)
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
            self.identities
                .insert(resource.asset_identity().to_owned(), handle);
            self.bodies
                .entry(resource.identity().to_owned())
                .or_default()
                .push(handle);
            match resource.kind() {
                CsharpRenderResourceKind::AnimatedMesh => {
                    self.animated.insert(handle);
                }
                CsharpRenderResourceKind::AnimationClipPack => self.clip_packs += 1,
                _ => {}
            }
            self.entries.insert(self.next, resource);
            self.next += 1;
            self.unheld.push(handle);
            handle
        };
        for path in paths {
            let key = (path, filter, wrap);
            if self.paths.insert(key.clone(), handle) != Some(handle) {
                self.handle_paths.entry(handle).or_default().push(key);
            }
        }
        Ok(handle)
    }

    /// Stages a resource and gives the caller one releasable owner of it.
    pub(crate) fn admit(
        &mut self,
        resource: CsharpRenderResource,
    ) -> Result<u64, CsharpEngineServicesError> {
        let handle = self.stage(resource, [])?;
        self.acquire_owner(handle)?;
        Ok(handle)
    }

    fn acquire_owner(&mut self, handle: u64) -> Result<(), CsharpEngineServicesError> {
        if !self.entries.contains_key(&resource_slot(handle)?) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "unknown resource handle",
            ));
        }
        let count = self.open_counts.entry(handle).or_default();
        *count = count.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_OWNER",
                "renderer resource owner count overflowed",
            )
        })?;
        Ok(())
    }

    /// Releases one caller-owned admission. True means it is the last: the
    /// caller removes the resource once nothing else holds it.
    pub(crate) fn release_owner(&mut self, handle: u64) -> Result<bool, CsharpEngineServicesError> {
        if !self.entries.contains_key(&resource_slot(handle)?) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "renderer resource is not live",
            ));
        }
        let count = self.open_counts.get(&handle).copied().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_OWNER",
                "renderer resource has no caller-owned admission",
            )
        })?;
        if count > 1 {
            self.open_counts.insert(handle, count - 1);
            return Ok(false);
        }
        Ok(true)
    }

    /// Removes a live resource. Its body stays servable for recent output.
    pub(crate) fn remove(
        &mut self,
        handle: u64,
    ) -> Result<CsharpRenderResource, CsharpEngineServicesError> {
        let index = resource_slot(handle)?;
        let resource = self.entries.remove(&index).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "renderer resource is not live",
            )
        })?;
        self.identities.remove(resource.asset_identity());
        if let Some(handles) = self.bodies.get_mut(resource.identity()) {
            handles.retain(|candidate| *candidate != handle);
            if handles.is_empty() {
                self.bodies.remove(resource.identity());
            }
        }
        match resource.kind() {
            CsharpRenderResourceKind::AnimatedMesh => {
                self.animated.remove(&handle);
            }
            CsharpRenderResourceKind::AnimationClipPack => self.clip_packs -= 1,
            _ => {}
        }
        for key in self.handle_paths.remove(&handle).unwrap_or_default() {
            // A path a later resource took over stays with it.
            if self.paths.get(&key) == Some(&handle) {
                self.paths.remove(&key);
            }
        }
        self.open_counts.remove(&handle);
        self.released_this_call.push(resource.clone());
        Ok(resource)
    }

    /// One more Engine holder holds each of these resources.
    pub(crate) fn hold(&mut self, handles: &BTreeSet<u64>) {
        for handle in handles {
            *self.holds.entry(*handle).or_default() += 1;
        }
    }

    /// One Engine holder no longer holds these resources.
    pub(crate) fn unhold(&mut self, handles: &BTreeSet<u64>) {
        for handle in handles {
            if let Some(count) = self.holds.get_mut(handle) {
                *count -= 1;
                if *count == 0 {
                    self.holds.remove(handle);
                    self.unheld.push(*handle);
                }
            }
        }
    }

    /// Whether an Engine holder holds this resource.
    pub(crate) fn held(&self, handle: u64) -> bool {
        self.holds.contains_key(&handle)
    }

    /// A live resource that neither a caller-owned admission nor an Engine
    /// holder keeps.
    pub(crate) fn take_unowned(&mut self) -> Option<u64> {
        while let Some(handle) = self.unheld.pop() {
            let live = resource_slot(handle).is_ok_and(|slot| self.entries.contains_key(&slot));
            if live && !self.open_counts.contains_key(&handle) && !self.held(handle) {
                return Some(handle);
            }
        }
        None
    }

    pub(crate) fn info(
        &self,
        handle: u64,
    ) -> Result<NativeRenderResourceInfo, CsharpEngineServicesError> {
        let resource = self.resource(handle)?;
        Ok(NativeRenderResourceInfo {
            handle: NativeRenderResourceHandle { value: handle },
            kind: match resource.kind() {
                CsharpRenderResourceKind::Texture => NativeRenderResourceKind::Texture,
                CsharpRenderResourceKind::Mesh => NativeRenderResourceKind::StaticMesh,
                CsharpRenderResourceKind::Font => NativeRenderResourceKind::Font,
                CsharpRenderResourceKind::Shader => NativeRenderResourceKind::Shader,
                CsharpRenderResourceKind::StarCatalogue => NativeRenderResourceKind::StarCatalogue,
                CsharpRenderResourceKind::Audio => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_KIND",
                        "audio resources are exposed by the Audio service, not Appearance",
                    ));
                }
                CsharpRenderResourceKind::Video => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_KIND",
                        "video resources are exposed by the Video service, not Appearance",
                    ));
                }
                CsharpRenderResourceKind::AnimatedMesh => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_KIND",
                        "animated GLB resources are exposed by the Animation service, not Appearance",
                    ));
                }
                CsharpRenderResourceKind::AnimationClipPack => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_KIND",
                        "animation clip-pack GLB resources are exposed by the Animation service, not Appearance",
                    ));
                }
            },
            byte_length: u32::try_from(resource.bytes().len()).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_SIZE",
                    "renderer resource byte length exceeded u32",
                )
            })?,
        })
    }
}

/// Billboards and particles name textures and fonts by asset identity.
impl render_presentation::PresentationAssetLookup for RenderResourceRegistry {
    fn presentation_asset_kind(&self, id: &str) -> Option<render_model::RenderAssetKind> {
        match self.by_asset_identity(id)?.kind() {
            CsharpRenderResourceKind::Texture => Some(render_model::RenderAssetKind::Texture),
            CsharpRenderResourceKind::Font => Some(render_model::RenderAssetKind::Font),
            _ => None,
        }
    }
}

fn resource_slot(handle: u64) -> Result<usize, CsharpEngineServicesError> {
    usize::try_from(handle.checked_sub(1).ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_RENDER_RESOURCE_HANDLE", "invalid resource handle")
    })?)
    .map_err(|_| {
        CsharpEngineServicesError::new(
            "CSHARP_RENDER_RESOURCE_HANDLE",
            "renderer resource handle overflowed",
        )
    })
}

struct ImportedStaticContent {
    source: Arc<[u8]>,
    payload: MeshPayloadDescriptor,
    material_slots: Vec<MeshMaterialSlot>,
    resource: CsharpRenderResource,
}

struct ImportedAnimatedContent {
    source: Arc<[u8]>,
    dependencies: Vec<(String, Arc<[u8]>)>,
    resource: CsharpRenderResource,
}

/// Derived assets reused for the immutable admitted source lifetime. Separate
/// from staged state: importing does not mutate product-visible ownership.
#[derive(Default)]
pub(crate) struct RenderResourceImports {
    static_content: BTreeMap<String, ImportedStaticContent>,
    /// The dimensions of each PNG body admitted so far, by content identity,
    /// so reopening one reads neither its chunks nor its hash again.
    png_dimensions: BTreeMap<String, [u32; 2]>,
    animated: BTreeMap<String, ImportedAnimatedContent>,
}

impl RenderResourceImports {
    /// Admits a PNG texture, packed mesh, WOFF2 font, WGSL shader or star
    /// catalogue by its extension, under the content's identity.
    pub(crate) fn file(
        &mut self,
        content: RetainedContent,
        filter: NativeTextureFilter,
        wrap: NativeTextureWrap,
        color_space: NativeTextureColorSpace,
        shader_keywords: &[String],
    ) -> Result<CsharpRenderResource, CsharpEngineServicesError> {
        let relative_path = content.path;
        let bytes = content.bytes;
        let browser_path = format!("content/{relative_path}");
        let content_hash = || content.identity.content_hash(&bytes);
        match () {
            _ if relative_path.ends_with(".png") => {
                let content_hash = content_hash();
                let dimensions = match self.png_dimensions.get(&content_hash) {
                    Some(dimensions) => *dimensions,
                    None => {
                        #[cfg(test)]
                        tests::PNG_ADMISSIONS.with(|count| count.set(count.get() + 1));
                        let dimensions = render_model::admit_png_rgba8(&bytes).map_err(|error| {
                            CsharpEngineServicesError::new(
                                "CSHARP_RENDER_RESOURCE_TEXTURE",
                                format!("renderer resource is not an admitted PNG: {error:?}"),
                            )
                        })?;
                        self.png_dimensions.insert(content_hash.clone(), dimensions);
                        dimensions
                    }
                };
                Ok(CsharpRenderResource::admit_texture(
                    browser_path,
                    bytes,
                    content_hash,
                    dimensions,
                    filter,
                    wrap,
                    color_space,
                ))
            }
            _ if relative_path.ends_with(".rmesh") => {
                CsharpRenderResource::admit_mesh(browser_path, Arc::clone(&bytes), content_hash())
            }
            _ if relative_path.ends_with(".woff2") => {
                CsharpRenderResource::admit_font(browser_path, Arc::clone(&bytes), content_hash())
            }
            _ if relative_path.ends_with(".stars.json") => {
                CsharpRenderResource::admit_star_catalogue(browser_path, &relative_path, &bytes)
            }
            _ if relative_path.ends_with(".wgsl") => CsharpRenderResource::admit_shader(
                browser_path,
                &relative_path,
                Arc::clone(&bytes),
                content_hash(),
                shader_keywords,
            ),
            _ => Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_KIND",
                format!(
                    "renderer resource `{relative_path}` must be an RGBA PNG, packed .rmesh, WOFF2, WGSL or .stars.json file"
                ),
            )),
        }
    }

    /// Packs a static mesh JSON document into one mesh resource, or admits a
    /// binary static mesh (`RSTATMSH`) whose resource is already packed, and
    /// returns it with the payload and material slots that read it.
    pub(crate) fn static_mesh(
        &mut self,
        content: RetainedContent,
    ) -> Result<
        (
            MeshPayloadDescriptor,
            Vec<MeshMaterialSlot>,
            CsharpRenderResource,
        ),
        CsharpEngineServicesError,
    > {
        if !self
            .static_content
            .get(&content.path)
            .is_some_and(|imported| Arc::ptr_eq(&imported.source, &content.bytes))
        {
            let bytes = content.bytes.as_ref();
            let (asset, payload, packed_resource) = if bytes.starts_with(STATIC_MESH_MAGIC) {
                decode_static_mesh(bytes, content.identity.content_hash(bytes))?
            } else {
                let asset = serde_json::from_slice::<StaticMeshAsset>(bytes).map_err(|error| {
                    CsharpEngineServicesError::new(
                        "CSHARP_STATIC_MESH_CONTENT_JSON",
                        error.to_string(),
                    )
                })?;
                let mut packed = pack_mesh_resources(
                    std::slice::from_ref(&asset.payload),
                    MAX_MESH_RESOURCE_BYTES,
                )
                .map_err(|error| {
                    CsharpEngineServicesError::new("CSHARP_STATIC_MESH_PACK", format!("{error:?}"))
                })?;
                let payload = packed.payloads.pop().expect("one packed inline payload");
                let packed_resource = packed.resources.pop().expect("one packed inline resource");
                (asset, payload, packed_resource)
            };
            let content_hash = &packed_resource.content_hash["sha256:".len()..];
            let browser_path = format!("content/engine-mesh/{content_hash}.rmesh");
            let resource = CsharpRenderResource::from_packed_mesh(browser_path, packed_resource);
            self.static_content.insert(
                content.path.clone(),
                ImportedStaticContent {
                    source: Arc::clone(&content.bytes),
                    payload,
                    material_slots: asset.material_slots,
                    resource,
                },
            );
        }
        let imported = &self.static_content[&content.path];
        Ok((
            imported.payload.clone(),
            imported.material_slots.clone(),
            imported.resource.clone(),
        ))
    }

    /// Imports an animated GLB and its relative dependencies as a primary
    /// animated mesh, or as a clip pack.
    pub(crate) fn animated(
        &mut self,
        content: RetainedContent,
        clip_pack: bool,
    ) -> Result<CsharpRenderResource, CsharpEngineServicesError> {
        // Live imports belong only to reference/resource owners. Immutable
        // startup sources retain the existing import cache across opens.
        let mut resource = if content.transient {
            let packed = pack_animated_glb_closure(&content.path, &content.bytes, &content.files)?;
            CsharpRenderResource::admit_animated_mesh(
                format!("content/{}", content.path),
                packed.bytes,
            )?
        } else {
            let key = content.path.clone();
            let matches_source = self.animated.get(&key).is_some_and(|imported| {
                Arc::ptr_eq(&imported.source, &content.bytes)
                    && imported.dependencies.iter().all(|(path, bytes)| {
                        content
                            .files
                            .get(path)
                            .ok()
                            .flatten()
                            .is_some_and(|current| Arc::ptr_eq(&current, bytes))
                    })
            });
            if !matches_source {
                let packed =
                    pack_animated_glb_closure(&content.path, &content.bytes, &content.files)?;
                let resource = CsharpRenderResource::admit_animated_mesh(
                    format!("content/{}", content.path),
                    packed.bytes,
                )?;
                self.animated.insert(
                    key.clone(),
                    ImportedAnimatedContent {
                        source: content.bytes,
                        dependencies: packed.dependencies,
                        resource,
                    },
                );
            }
            self.animated[&key].resource.clone()
        };
        if clip_pack {
            if resource
                .animated_mesh
                .as_ref()
                .expect("imported GLB descriptor")
                .clips
                .is_empty()
            {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CLIP_PACK_ADMISSION",
                    "animation clip-pack GLB must contain clips",
                ));
            }
            resource.kind = CsharpRenderResourceKind::AnimationClipPack;
            resource.identity = format!(
                "clip-pack-resource/{}",
                &resource.content_hash["sha256:".len()..]
            );
        }
        Ok(resource)
    }

    /// The cached import of a source path, if any.
    #[cfg(test)]
    pub(crate) fn cached(&self, path: &str) -> Option<&CsharpRenderResource> {
        self.static_content
            .get(path)
            .map(|imported| &imported.resource)
            .or_else(|| self.animated.get(path).map(|imported| &imported.resource))
    }
}

pub(crate) type CollisionMeshGeometry = (Vec<[f64; 3]>, Vec<[u32; 3]>);

/// Generated mesh resources and the partitions cut from them. Each resource
/// retains its typed definition, which the renderer catalog shares, and the
/// materials its slots bind.
#[derive(Clone)]
pub(crate) struct GeneratedMeshes {
    resources: BTreeMap<u64, GeneratedMesh>,
    next_resource: u64,
    partitions: BTreeMap<u64, GeneratedMeshPartition>,
    next_partition: u64,
    /// Live meshes and prepared partitions binding each material, with how
    /// many.
    material_users: BTreeMap<u64, u32>,
}

#[derive(Clone)]
struct GeneratedMesh {
    definition: Arc<StaticMeshAsset>,
    material_handles: BTreeSet<u64>,
}

#[derive(Clone)]
struct GeneratedMeshPartition {
    parts: Vec<Option<Arc<StaticMeshAsset>>>,
    material_handles: BTreeSet<u64>,
}

/// A bound generated mesh that has passed every admission check.
struct BoundGeneratedMesh {
    handle: u64,
    next: u64,
    definition: StaticMeshAsset,
    material_handles: BTreeSet<u64>,
}

impl Default for GeneratedMeshes {
    fn default() -> Self {
        Self {
            resources: BTreeMap::new(),
            next_resource: 1,
            partitions: BTreeMap::new(),
            next_partition: 1,
            material_users: BTreeMap::new(),
        }
    }
}

impl GeneratedMeshes {
    pub(crate) fn len(&self) -> usize {
        self.resources.len()
    }

    /// The catalog asset of a live generated mesh.
    pub(crate) fn asset(&self, handle: u64) -> Option<&str> {
        self.resources
            .get(&handle)
            .map(|mesh| mesh.definition.asset.as_str())
    }

    /// A live mesh or a prepared partition binds this material.
    pub(crate) fn uses_material(&self, material: u64) -> bool {
        self.material_users.contains_key(&material)
    }

    /// Count one more (`add`) or one fewer user of each material.
    fn count_materials(&mut self, materials: &BTreeSet<u64>, add: bool) {
        for material in materials {
            if add {
                *self.material_users.entry(*material).or_default() += 1;
            } else if let Some(count) = self.material_users.get_mut(material) {
                *count -= 1;
                if *count == 0 {
                    self.material_users.remove(material);
                }
            }
        }
    }

    pub(crate) fn remove(&mut self, handle: u64) {
        if let Some(mesh) = self.resources.remove(&handle) {
            self.count_materials(&mesh.material_handles, false);
        }
    }

    fn remove_partition(&mut self, partition: u64) {
        if let Some(partition) = self.partitions.remove(&partition) {
            self.count_materials(&partition.material_handles, false);
        }
    }

    fn bind(
        &self,
        payload: MeshPayloadDescriptor,
        slots: BTreeSet<u16>,
        bindings: &[NativeMeshMaterialBinding],
        material_id: impl Fn(u64) -> Option<String>,
    ) -> Result<BoundGeneratedMesh, CsharpEngineServicesError> {
        let invalid =
            |message: &str| CsharpEngineServicesError::new("CSHARP_MESH_ADMISSION", message);
        let mut bound_slots = BTreeSet::new();
        let mut material_slots = Vec::new();
        let mut material_handles = BTreeSet::new();
        for binding in bindings {
            let slot = u16::try_from(binding.material_slot)
                .map_err(|_| invalid("mesh material slot exceeds u16"))?;
            if !slots.contains(&slot) || !bound_slots.insert(slot) {
                return Err(invalid(
                    "mesh bindings must name each used slot exactly once",
                ));
            }
            let material = material_id(binding.material.value)
                .ok_or_else(|| invalid("mesh binding requires a live material"))?;
            material_slots.push(MeshMaterialSlot { slot, material });
            material_handles.insert(binding.material.value);
        }
        if slots != bound_slots {
            return Err(invalid(
                "mesh bindings must cover every group material slot",
            ));
        }
        let handle = self.next_resource;
        let next = handle
            .checked_add(1)
            .ok_or_else(|| invalid("mesh handle overflow"))?;
        let definition = StaticMeshAsset {
            asset: format!("mesh/runtime-{handle}"),
            payload,
            material_slots,
            collision: MeshCollisionPolicy::VisualOnly,
        };
        definition
            .validate()
            .map_err(|error| invalid(&format!("invalid mesh resource: {error:?}")))?;
        Ok(BoundGeneratedMesh {
            handle,
            next,
            definition,
            material_handles,
        })
    }

    fn insert(&mut self, bound: BoundGeneratedMesh) -> Arc<StaticMeshAsset> {
        // Retain the typed definition. Delivery owns its eventual encoding;
        // do not serialize and discard an extra copy merely to measure it.
        let definition = Arc::new(bound.definition);
        self.count_materials(&bound.material_handles, true);
        self.resources.insert(
            bound.handle,
            GeneratedMesh {
                definition: Arc::clone(&definition),
                material_handles: bound.material_handles,
            },
        );
        self.next_resource = bound.next;
        definition
    }

    fn partition(
        &mut self,
        request: NativeMeshPartitionRequest,
    ) -> Result<NativeMeshPartitionHandle, CsharpEngineServicesError> {
        let invalid =
            |message: &str| CsharpEngineServicesError::new("CSHARP_MESH_PARTITION", message);
        let source = self
            .resources
            .get(&request.source.value)
            .ok_or_else(|| invalid("partition source mesh is not live"))?;
        let definition = &source.definition;
        let payloads = partition_mesh_spatially(
            &definition.payload,
            native_vec3_array(request.origin),
            native_vec3_array(request.cell_size),
        )
        .map_err(invalid)?;
        let parts = payloads
            .into_iter()
            .map(|payload| {
                let used: BTreeSet<_> = payload.groups.iter().map(|g| g.material_slot).collect();
                Some(Arc::new(StaticMeshAsset {
                    asset: String::new(),
                    payload,
                    material_slots: definition
                        .material_slots
                        .iter()
                        .filter(|m| used.contains(&m.slot))
                        .cloned()
                        .collect(),
                    collision: MeshCollisionPolicy::VisualOnly,
                }))
            })
            .collect();
        let material_handles = source.material_handles.clone();
        let handle = self.next_partition;
        self.next_partition = handle
            .checked_add(1)
            .ok_or_else(|| invalid("mesh partition handle overflow"))?;
        self.count_materials(&material_handles, true);
        self.partitions.insert(
            handle,
            GeneratedMeshPartition {
                parts,
                material_handles,
            },
        );
        Ok(NativeMeshPartitionHandle { value: handle })
    }

    fn read_partition(
        &self,
        partition: NativeMeshPartitionHandle,
    ) -> Result<NativeMeshPartitionReadout, CsharpEngineServicesError> {
        let prepared = self.partitions.get(&partition.value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MESH_PARTITION", "mesh partition is not live")
        })?;
        Ok(NativeMeshPartitionReadout {
            part_count: prepared.parts.len() as u32,
        })
    }

    fn take_part(
        &mut self,
        request: NativeMeshPartitionPartRequest,
    ) -> Result<(NativeMeshResourceHandle, Arc<StaticMeshAsset>), CsharpEngineServicesError> {
        let invalid =
            |message: &str| CsharpEngineServicesError::new("CSHARP_MESH_PARTITION", message);
        let prepared = self
            .partitions
            .get_mut(&request.partition.value)
            .ok_or_else(|| invalid("mesh partition is not live"))?;
        let part = prepared
            .parts
            .get_mut(request.index as usize)
            .ok_or_else(|| invalid("mesh partition index is out of range"))?;
        let handle = self.next_resource;
        let next = handle
            .checked_add(1)
            .ok_or_else(|| invalid("mesh resource handle overflow"))?;
        let mut definition = part
            .take()
            .ok_or_else(|| invalid("mesh partition part was already taken"))?;
        let material_handles = prepared.material_handles.clone();
        Arc::make_mut(&mut definition).asset = format!("mesh/runtime-{handle}");
        self.count_materials(&material_handles, true);
        self.resources.insert(
            handle,
            GeneratedMesh {
                definition: Arc::clone(&definition),
                material_handles,
            },
        );
        self.next_resource = next;
        Ok((NativeMeshResourceHandle { value: handle }, definition))
    }

    fn copy_inline(
        &self,
        resource: NativeMeshResourceHandle,
    ) -> Result<CollisionMeshGeometry, CsharpEngineServicesError> {
        let mesh = self.resources.get(&resource.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_COLLISION_MESH_STALE",
                "collision mesh reference does not name a live Graphics mesh",
            )
        })?;
        let MeshPayloadSource::Inline {
            positions, indices, ..
        } = &mesh.definition.payload.source
        else {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_COLLISION_MESH_NONINLINE",
                "collision mesh references require an inline Graphics mesh",
            ));
        };
        let (position_chunks, position_remainder) = positions.as_chunks::<3>();
        let (index_chunks, index_remainder) = indices.as_chunks::<3>();
        if !position_remainder.is_empty() || !index_remainder.is_empty() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_COLLISION_MESH_INVALID",
                "inline Graphics mesh had incomplete collision geometry",
            ));
        }
        Ok((
            position_chunks
                .iter()
                .map(|position| {
                    [
                        f64::from(position[0]),
                        f64::from(position[1]),
                        f64::from(position[2]),
                    ]
                })
                .collect(),
            index_chunks.to_vec(),
        ))
    }
}

/// Admits a generated mesh in the staged Graphics call. Its streams are
/// copied before this returns, each binding names a live material, and its
/// definition joins the renderer catalog.
pub(crate) unsafe fn create_generated_mesh(
    bridge: &mut RuntimeAppearanceBridge,
    request: &NativeMeshResourceCreateRequest,
) -> Result<NativeMeshResourceHandle, CsharpEngineServicesError> {
    let (payload, slots, bindings) = generated_mesh_payload(request)?;
    let state = &mut *bridge.staged_mut()?.state;
    let bound = state
        .generated_meshes
        .bind(payload, slots, bindings, |material| {
            state.material_id(material).map(str::to_owned)
        })?;
    let handle = bound.handle;
    let definition = state.generated_meshes.insert(bound);
    state.project_static_mesh(definition);
    Ok(NativeMeshResourceHandle { value: handle })
}

/// Copies staged inline mesh geometry for Engine spatial or authoring
/// consumers. The Graphics resource remains borrowed for this call and
/// may be released as soon as the copy completes.
pub(crate) fn copy_inline_mesh_geometry(
    bridge: &RuntimeAppearanceBridge,
    resource: NativeMeshResourceHandle,
) -> Result<CollisionMeshGeometry, CsharpEngineServicesError> {
    let staged = bridge.staged_ref().map_err(|_| {
        CsharpEngineServicesError::new(
            "CSHARP_COLLISION_MESH_UNBOUND",
            "collision mesh references require a current staged Graphics call",
        )
    })?;
    staged.state.generated_meshes.copy_inline(resource)
}

pub(crate) unsafe extern "C" fn create_mesh_resource(
    context: *mut c_void,
    request: *const NativeMeshResourceCreateRequest,
    result: *mut NativeMeshResourceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| unsafe {
            create_generated_mesh(bridge, &*request)
        })
    })
}

pub(crate) unsafe extern "C" fn partition_mesh(
    context: *mut c_void,
    request: NativeMeshPartitionRequest,
    result: *mut NativeMeshPartitionHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            bridge
                .staged_mut()?
                .state
                .generated_meshes
                .partition(request)
        })
    })
}

pub(crate) unsafe extern "C" fn read_mesh_partition(
    context: *mut c_void,
    partition: NativeMeshPartitionHandle,
    result: *mut NativeMeshPartitionReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            bridge
                .staged_ref()?
                .state
                .generated_meshes
                .read_partition(partition)
        })
    })
}

pub(crate) unsafe extern "C" fn take_mesh_partition_part(
    context: *mut c_void,
    request: NativeMeshPartitionPartRequest,
    result: *mut NativeMeshResourceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            let state = &mut *bridge.staged_mut()?.state;
            let (handle, definition) = state.generated_meshes.take_part(request)?;
            state.project_static_mesh(definition);
            Ok(handle)
        })
    })
}

pub(crate) unsafe extern "C" fn destroy_mesh_partition(
    context: *mut c_void,
    partition: NativeMeshPartitionHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| {
            bridge
                .staged_mut()?
                .state
                .generated_meshes
                .remove_partition(partition.value);
            Ok(())
        })
    })
}

fn native_vec3_array(value: NativeVec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

/// Copies a generated mesh request's streams into a typed inline payload.
/// Returns the group slots the bindings must cover, and the bindings.
unsafe fn generated_mesh_payload(
    request: &NativeMeshResourceCreateRequest,
) -> Result<
    (
        MeshPayloadDescriptor,
        BTreeSet<u16>,
        &[NativeMeshMaterialBinding],
    ),
    CsharpEngineServicesError,
> {
    let invalid = |message: &str| CsharpEngineServicesError::new("CSHARP_MESH_ADMISSION", message);
    // Counts are stored as u32 in the mesh layout. This is a representation
    // constraint, not a byte budget on trusted product geometry.
    let vertex_count = u32::try_from(request.positions_len)
        .map_err(|_| invalid("mesh vertex count exceeds the u32 layout representation"))?;
    let index_count = u32::try_from(request.indices_len)
        .map_err(|_| invalid("mesh index count exceeds the u32 layout representation"))?;
    if request.positions_len < 3
        || request.normals_len != request.positions_len
        || (request.uvs_len != 0 && request.uvs_len != request.positions_len)
        || (request.colors_len != 0 && request.colors_len != request.positions_len)
        || (request.layer_weights_len != 0 && request.layer_weights_len != request.positions_len)
        || request.indices_len < 3
        || !request.indices_len.is_multiple_of(3)
        || request.groups_len == 0
        || request.bindings_len == 0
    {
        return Err(invalid("mesh requires at least 3 vertices, matching normals/optional UV/color/layer weight streams, complete triangle indices, and nonempty groups/bindings"));
    }
    // Layer weights travel in the colour stream, as a voxel chunk's do.
    if request.colors_len != 0 && request.layer_weights_len != 0 {
        return Err(invalid(
            "a mesh has vertex colours or layer weights, not both",
        ));
    }
    let positions = borrowed_slice(request.positions, request.positions_len, "mesh positions")?;
    let normals = borrowed_slice(request.normals, request.normals_len, "mesh normals")?;
    let uvs = borrowed_slice(request.uvs, request.uvs_len, "mesh UVs")?;
    let colors = borrowed_slice(request.colors, request.colors_len, "mesh colors")?;
    let weights = borrowed_slice(
        request.layer_weights,
        request.layer_weights_len,
        "mesh layer weights",
    )?;
    let indices = borrowed_slice(request.indices, request.indices_len, "mesh indices")?;
    let groups = borrowed_slice(request.groups, request.groups_len, "mesh groups")?;
    let bindings = borrowed_slice(request.bindings, request.bindings_len, "mesh bindings")?;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in positions {
        for (axis, value) in native_vec3_array(*position).into_iter().enumerate() {
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    let groups = groups
        .iter()
        .map(|group| {
            if !group.start.is_multiple_of(3) || group.count == 0 || !group.count.is_multiple_of(3)
            {
                return Err(invalid(
                    "mesh groups must contain complete nonempty triangles",
                ));
            }
            Ok(MeshGroupDescriptor {
                material_slot: u16::try_from(group.material_slot)
                    .map_err(|_| invalid("mesh material slot exceeds u16"))?,
                start: group.start,
                count: group.count,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let slots = groups
        .iter()
        .map(|group| group.material_slot)
        .collect::<BTreeSet<_>>();
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
    if !uvs.is_empty() {
        attributes.push(MeshAttribute {
            name: MeshAttributeName::Uv,
            components: 2,
            kind: MeshAttributeKind::F32,
        });
    }
    if !colors.is_empty() || !weights.is_empty() {
        attributes.push(MeshAttribute {
            name: MeshAttributeName::Color,
            components: 4,
            kind: MeshAttributeKind::F32,
        });
    }
    let payload = MeshPayloadDescriptor {
        texture_space: None,
        distance_field: None,
        layout: MeshBufferLayout {
            vertex_count,
            index_count,
            index_width: MeshIndexWidth::U32,
            attributes,
        },
        groups,
        bounds: MeshBoundsDescriptor { min, max },
        source: MeshPayloadSource::Inline {
            positions: positions
                .iter()
                .flat_map(|value| native_vec3_array(*value))
                .collect(),
            normals: normals
                .iter()
                .flat_map(|value| native_vec3_array(*value))
                .collect(),
            uvs: (!uvs.is_empty())
                .then(|| uvs.iter().flat_map(|value| [value.x, value.y]).collect()),
            colors: if !weights.is_empty() {
                Some(
                    weights
                        .iter()
                        .flat_map(|value| [value.x, value.y, value.z, value.w])
                        .collect(),
                )
            } else {
                (!colors.is_empty()).then(|| {
                    colors
                        .iter()
                        .flat_map(|value| [value.r, value.g, value.b, value.a])
                        .collect()
                })
            },
            indices: indices.to_vec(),
        },
        provenance: MeshProvenance::Generated,
        layer_weights: !weights.is_empty(),
        layer_palette: Vec::new(),
        vertex_occlusion: false,
        morph: None,
    };
    Ok((payload, slots, bindings))
}

/// A recorded-audio, video or animated resource's path names its media type.
fn require_extension(path: &str, extension: &str) -> Result<(), CsharpEngineServicesError> {
    if !path.ends_with(extension) {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_RENDER_RESOURCE_PATH",
            format!("renderer resource `{path}` must use the {extension} extension"),
        ));
    }
    Ok(())
}

struct PackedAnimatedSource {
    bytes: Vec<u8>,
    dependencies: Vec<(String, Arc<[u8]>)>,
}

fn pack_animated_glb_closure(
    root_path: &str,
    root_bytes: &[u8],
    content_resources: &crate::content::ContentFiles,
) -> Result<PackedAnimatedSource, CsharpEngineServicesError> {
    let resource_uris = glb_relative_resource_uris(root_bytes).map_err(|diagnostic| {
        CsharpEngineServicesError::new(
            "CSHARP_ANIMATION_GLB_CLOSURE",
            format!(
                "{}: {} ({:?})",
                diagnostic.locus, diagnostic.message, diagnostic.code
            ),
        )
    })?;
    let directory = root_path
        .rsplit_once('/')
        .map_or("", |(directory, _)| directory);
    let mut dependencies = Vec::new();
    let resources = resource_uris
        .into_iter()
        .map(|uri| {
            let content_path = if directory.is_empty() {
                uri.clone()
            } else {
                format!("{directory}/{uri}")
            };
            let bytes = content_resources.get(&content_path)?.ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GLB_CLOSURE",
                    format!("animated GLB dependency `{content_path}` is missing"),
                )
            })?;
            let resource = GltfResource {
                uri,
                bytes: bytes.to_vec(),
            };
            dependencies.push((content_path, bytes));
            Ok(resource)
        })
        .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
    admit_glb_source(&GlbSourceClosure {
        root_glb: root_bytes.to_vec(),
        resources,
    })
    .map(|packed| PackedAnimatedSource {
        bytes: packed.glb_bytes,
        dependencies,
    })
    .map_err(|diagnostic| {
        CsharpEngineServicesError::new(
            "CSHARP_ANIMATION_GLB_CLOSURE",
            format!(
                "{}: {} ({:?})",
                diagnostic.locus, diagnostic.message, diagnostic.code
            ),
        )
    })
}

/// The binary static mesh: the JSON form's descriptor, with its payload read
/// from the Engine's packed mesh resource that follows it, so the streams need
/// no parse and no repacking. Little-endian:
///
/// | field | encoding |
/// |---|---|
/// | magic | `RSTATMSH` (8 bytes) |
/// | descriptor length | u32 |
/// | descriptor | UTF-8 JSON static mesh document whose `payload.source` is a `resource` source naming the bytes below |
/// | packed mesh resource | `payload.source.byteLength` bytes, to the end of the file (`RMSHLE01`–`03`) |
///
/// The resource is named by the file's content identity, which the Engine
/// computed: the descriptor's `resource` and `contentHash` are replaced, so a
/// stale value written by a tool cannot name another mesh's resource.
/// Admission checks the descriptor and the resource header against the bytes,
/// not the streams, which the renderer reads when it draws.
const STATIC_MESH_MAGIC: &[u8; 8] = b"RSTATMSH";

type DecodedStaticMesh = (StaticMeshAsset, MeshPayloadDescriptor, PackedMeshResource);

/// `file_hash` is the content identity of the whole file (`sha256:<hex>`).
fn decode_static_mesh(
    bytes: &[u8],
    file_hash: String,
) -> Result<DecodedStaticMesh, CsharpEngineServicesError> {
    let malformed =
        |detail: String| CsharpEngineServicesError::new("CSHARP_STATIC_MESH_CONTENT_JSON", detail);
    let header = STATIC_MESH_MAGIC.len() + 4;
    let length = bytes
        .get(STATIC_MESH_MAGIC.len()..header)
        .map(|length| u32::from_le_bytes(length.try_into().expect("four bytes")) as usize)
        .ok_or_else(|| malformed("binary static mesh is truncated".to_owned()))?;
    let descriptor = bytes
        .get(header..header.saturating_add(length))
        .ok_or_else(|| malformed("binary static mesh descriptor is truncated".to_owned()))?;
    let mut asset = serde_json::from_slice::<StaticMeshAsset>(descriptor)
        .map_err(|error| malformed(error.to_string()))?;
    let resource = &bytes[header + length..];
    let pack_error =
        |detail: String| CsharpEngineServicesError::new("CSHARP_STATIC_MESH_PACK", detail);
    validate_mesh_resource_header(resource).map_err(|error| pack_error(format!("{error:?}")))?;
    let MeshPayloadSource::Resource {
        resource: identity,
        content_hash,
        byte_length,
        ..
    } = &mut asset.payload.source
    else {
        return Err(pack_error(
            "binary static mesh payload must name its packed resource".to_owned(),
        ));
    };
    if *byte_length as usize != resource.len() {
        return Err(pack_error(format!(
            "binary static mesh descriptor names {byte_length} resource bytes, not {}",
            resource.len()
        )));
    }
    *identity = format!("mesh-resource/{}", &file_hash["sha256:".len()..]);
    *content_hash = file_hash;
    let packed = PackedMeshResource {
        resource: identity.clone(),
        content_hash: content_hash.clone(),
        bytes: resource.to_vec(),
    };
    asset
        .payload
        .validate()
        .map_err(|error| pack_error(format!("{error:?}")))?;
    let payload = asset.payload.clone();
    Ok((asset, payload, packed))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::appearance::tests::RGBA_PNG;
    use csharp_engine_abi::{NativeColor, NativeMaterialHandle, NativeMeshGroup};

    const CHARACTER_GLB: &[u8] = include_bytes!(
        "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
    );

    thread_local! {
        pub(super) static PNG_ADMISSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    /// PNG bodies whose chunks were read on this thread so far.
    pub(crate) fn png_admissions() -> usize {
        PNG_ADMISSIONS.with(std::cell::Cell::get)
    }

    pub(crate) fn external_image_glb(source: &[u8], uri: &str) -> Vec<u8> {
        assert_eq!(&source[..4], b"glTF");
        let json_length = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
        let old_json_end = 20 + json_length;
        let mut root: serde_json::Value =
            serde_json::from_slice(&source[20..old_json_end]).unwrap();
        let image = root["images"][0].as_object_mut().unwrap();
        image.remove("bufferView");
        image.remove("mimeType");
        image.insert("uri".to_owned(), serde_json::Value::String(uri.to_owned()));
        let mut json = serde_json::to_vec(&root).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len() + source.len() - old_json_end;
        let mut rewritten = Vec::with_capacity(total);
        rewritten.extend_from_slice(b"glTF");
        rewritten.extend_from_slice(&2u32.to_le_bytes());
        rewritten.extend_from_slice(&(total as u32).to_le_bytes());
        rewritten.extend_from_slice(&(json.len() as u32).to_le_bytes());
        rewritten.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
        rewritten.extend_from_slice(&json);
        rewritten.extend_from_slice(&source[old_json_end..]);
        rewritten
    }

    /// Widening this module's crate surface is a reviewed change: another
    /// module may reach only what this list names, test-only items included.
    #[test]
    fn crate_surface_is_the_reviewed_list() {
        let source = include_str!("render_resources.rs");
        let module = source
            .split("#[cfg(test)]\npub(crate) mod tests")
            .next()
            .expect("module source precedes its tests");
        let surface = module
            .lines()
            .filter_map(|line| {
                let line = line.trim_start();
                let (visibility, item) = line
                    .strip_prefix("pub(crate) ")
                    .map(|item| ("pub(crate)", item))
                    .or_else(|| line.strip_prefix("pub ").map(|item| ("pub", item)))?;
                let end = item
                    .find(['(', '<', ':', '{', '=', ';'])
                    .unwrap_or(item.len());
                Some(format!("{visibility} {}", item[..end].trim_end()))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            surface,
            [
                "pub struct CsharpRenderResource",
                "pub enum CsharpRenderResourceKind",
                "pub(crate) fn admit_audio",
                "pub(crate) fn admit_video",
                "pub const fn kind",
                "pub fn identity",
                "pub(crate) fn asset_identity",
                "pub fn content_hash",
                "pub fn path",
                "pub fn bytes",
                "pub fn shared_bytes",
                "pub(crate) fn texture",
                "pub(crate) fn animated_mesh",
                "pub(crate) fn shader",
                "pub(crate) fn test_mesh",
                "pub(crate) struct RenderResourceRegistry",
                "pub(crate) fn begin_call",
                "pub(crate) fn recently_released",
                "pub(crate) fn iter",
                "pub(crate) fn len",
                "pub(crate) fn is_empty",
                "pub(crate) fn first",
                "pub(crate) fn get",
                "pub(crate) fn by_asset_identity",
                "pub(crate) fn by_identity",
                "pub(crate) fn animated_meshes",
                "pub(crate) fn clip_pack_count",
                "pub(crate) fn resource",
                "pub(crate) fn animated_mesh_mut",
                "pub(crate) fn stage",
                "pub(crate) fn admit",
                "pub(crate) fn release_owner",
                "pub(crate) fn remove",
                "pub(crate) fn hold",
                "pub(crate) fn unhold",
                "pub(crate) fn held",
                "pub(crate) fn take_unowned",
                "pub(crate) fn info",
                "pub(crate) struct RenderResourceImports",
                "pub(crate) fn file",
                "pub(crate) fn static_mesh",
                "pub(crate) fn animated",
                "pub(crate) fn cached",
                "pub(crate) type CollisionMeshGeometry",
                "pub(crate) struct GeneratedMeshes",
                "pub(crate) fn len",
                "pub(crate) fn asset",
                "pub(crate) fn uses_material",
                "pub(crate) fn remove",
                "pub(crate) unsafe fn create_generated_mesh",
                "pub(crate) fn copy_inline_mesh_geometry",
                "pub(crate) unsafe extern \"C\" fn create_mesh_resource",
                "pub(crate) unsafe extern \"C\" fn partition_mesh",
                "pub(crate) unsafe extern \"C\" fn read_mesh_partition",
                "pub(crate) unsafe extern \"C\" fn take_mesh_partition_part",
                "pub(crate) unsafe extern \"C\" fn destroy_mesh_partition",
            ]
        );
    }

    #[test]
    fn identity_and_kind_indexes_follow_staging_and_removal() {
        let mut registry = RenderResourceRegistry::default();
        let texture = |filter| {
            CsharpRenderResource::admit_texture(
                "content/atlas.png".to_owned(),
                Arc::from(RGBA_PNG),
                crate::content::ContentIdentity::default().content_hash(RGBA_PNG),
                render_model::admit_png_rgba8(RGBA_PNG).expect("png"),
                filter,
                NativeTextureWrap::Clamp,
                NativeTextureColorSpace::Srgb,
            )
        };
        let nearest = registry
            .admit(texture(NativeTextureFilter::Nearest))
            .unwrap();
        let linear = registry
            .admit(texture(NativeTextureFilter::Linear))
            .unwrap();
        assert_ne!(nearest, linear, "one image, two samplings");
        let body = registry.get(nearest).unwrap().identity().to_owned();
        assert_eq!(registry.get(linear).unwrap().identity(), body);
        let asset = registry.get(linear).unwrap().asset_identity().to_owned();
        let animated = || {
            CsharpRenderResource::admit_animated_mesh(
                "content/character.glb".to_owned(),
                CHARACTER_GLB.to_vec(),
            )
            .expect("animated mesh")
        };
        let mesh = registry.admit(animated()).unwrap();
        let mut pack = animated();
        pack.kind = CsharpRenderResourceKind::AnimationClipPack;
        pack.identity.push_str("-pack");
        let pack = registry.admit(pack).unwrap();
        assert_eq!(
            (
                registry.animated_meshes().count(),
                registry.clip_pack_count()
            ),
            (1, 1)
        );

        assert_eq!(
            registry
                .by_asset_identity(&asset)
                .map(|r| r.asset_identity()),
            Some(asset.as_str())
        );
        registry.remove(nearest).unwrap();
        assert_eq!(
            registry.by_identity(&body).map(|r| r.asset_identity()),
            Some(asset.as_str()),
            "the other sampling still serves the body"
        );
        registry.remove(linear).unwrap();
        assert!(registry.by_identity(&body).is_none());
        assert!(registry.by_asset_identity(&asset).is_none());
        registry.remove(mesh).unwrap();
        registry.remove(pack).unwrap();
        assert_eq!(
            (
                registry.animated_meshes().count(),
                registry.clip_pack_count()
            ),
            (0, 0)
        );
    }

    #[test]
    fn a_released_body_stays_servable_through_the_next_call() {
        // #8771: output of the releasing call can still name the body; the
        // renderer reads it after the call, so it stays until the call after next.
        let mut registry = RenderResourceRegistry::default();
        registry.begin_call();
        let texture = open_file(
            &mut RenderResourceImports::default(),
            content("atlas.png", Arc::from(RGBA_PNG)),
        )
        .expect("texture");
        let handle = registry.admit(texture).expect("admitted texture");
        assert!(registry.release_owner(handle).expect("last owner"));
        registry.remove(handle).expect("release in the same call");
        assert_eq!(
            registry.recently_released().count(),
            1,
            "after the releasing call"
        );
        registry.begin_call();
        assert_eq!(
            registry.recently_released().count(),
            1,
            "through the next call"
        );
        registry.begin_call();
        assert_eq!(registry.recently_released().count(), 0, "gone after that");
    }

    #[test]
    fn reopening_content_reuses_its_identity_and_png_facts_under_any_filename() {
        let packed = pack_mesh_resources(
            std::slice::from_ref(
                &serde_json::from_slice::<StaticMeshAsset>(&static_mesh_fixture(
                    "triangle.static-mesh.json",
                ))
                .unwrap()
                .payload,
            ),
            MAX_MESH_RESOURCE_BYTES,
        )
        .unwrap();
        let mut font = b"wOF2".to_vec();
        font.resize(64, 0);
        let mut imports = RenderResourceImports::default();
        for (path, bytes) in [
            ("textures/ß #1 ткань.png", RGBA_PNG.to_vec()),
            (
                "meshes/треугольник.rmesh",
                packed.resources[0].bytes.clone(),
            ),
            ("fonts/日本語.woff2", font),
        ] {
            let retained = content(path, Arc::from(bytes));
            let (hashes, pngs) = (crate::content::tests::body_hashes(), png_admissions());
            let first = open_file(&mut imports, retained.clone()).expect(path);
            let second = open_file(&mut imports, retained).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.path(), format!("content/{path}"));
            assert_eq!(
                crate::content::tests::body_hashes(),
                hashes + 1,
                "{path} is hashed once, by its content identity"
            );
            assert_eq!(
                png_admissions(),
                pngs + usize::from(path.ends_with(".png")),
                "{path}'s PNG chunks are read once"
            );
            if path.ends_with(".rmesh") {
                assert_eq!(first.identity(), packed.resources[0].resource);
            }
        }
        // Other content with the same body reuses the facts of its identity.
        let alias = content("textures/alias.png", Arc::from(RGBA_PNG));
        let pngs = png_admissions();
        let texture = open_file(&mut imports, alias).unwrap();
        assert_eq!(png_admissions(), pngs);
        let texture = texture.texture().unwrap();
        assert_eq!((texture.width, texture.height), (2, 1));
        // A changed body is a new identity, admitted afresh.
        let mut damaged = RGBA_PNG.to_vec();
        damaged[20] ^= 1;
        assert_eq!(
            open_file(&mut imports, content("damaged.png", Arc::from(damaged)))
                .unwrap_err()
                .code(),
            "CSHARP_RENDER_RESOURCE_TEXTURE"
        );
    }

    #[test]
    fn animated_embedded_image_is_packed_without_external_dependencies() {
        let embedded = external_image_glb(CHARACTER_GLB,
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg==");
        let packed = pack_animated_glb_closure(
            "character.glb",
            &embedded,
            &crate::content::ContentFiles::snapshot(BTreeMap::new()),
        )
        .expect("embedded image is packed without companion files");
        assert!(packed.dependencies.is_empty());
        assert_ne!(packed.bytes, embedded);
        let imported = asset_import::import_animated_glb_asset(
            &asset_import::SourceUri::RelativePath("character.glb".to_owned()),
            &packed.bytes,
            &asset_import::ImportContext::default(),
        );
        assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
        let unchanged = pack_animated_glb_closure(
            "character.glb",
            CHARACTER_GLB,
            &crate::content::ContentFiles::snapshot(BTreeMap::new()),
        )
        .unwrap();
        assert_eq!(unchanged.bytes, CHARACTER_GLB);
    }

    fn content(path: &str, bytes: Arc<[u8]>) -> RetainedContent {
        RetainedContent {
            path: path.to_owned(),
            identity: Default::default(),
            bytes,
            transient: false,
            files: crate::content::ContentFiles::snapshot(BTreeMap::new()),
        }
    }

    fn open_file(
        imports: &mut RenderResourceImports,
        content: RetainedContent,
    ) -> Result<CsharpRenderResource, CsharpEngineServicesError> {
        imports.file(
            content,
            NativeTextureFilter::Nearest,
            NativeTextureWrap::Clamp,
            NativeTextureColorSpace::Srgb,
            &[],
        )
    }

    fn static_mesh_fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/csharp-static-mesh")
                .join(name),
        )
        .unwrap()
    }

    fn import_static_mesh(
        bytes: Vec<u8>,
    ) -> Result<
        (
            MeshPayloadDescriptor,
            Vec<MeshMaterialSlot>,
            CsharpRenderResource,
        ),
        String,
    > {
        RenderResourceImports::default()
            .static_mesh(content("mesh", Arc::from(bytes)))
            .map_err(|error| error.code().to_owned())
    }

    #[test]
    fn a_binary_static_mesh_admits_the_mesh_its_json_form_packs_to() {
        let (json_payload, json_slots, json_resource) =
            import_static_mesh(static_mesh_fixture("triangle.static-mesh.json")).unwrap();
        // Written from the documented layouts by an independent script.
        let binary = static_mesh_fixture("triangle.rstatmsh");
        let (mut payload, slots, resource) = import_static_mesh(binary.clone()).unwrap();
        assert_eq!(resource.bytes(), json_resource.bytes());
        // The binary form's resource is named by the file's own identity.
        let file_hash = render_model::mesh_resource_content_hash(&binary);
        assert_eq!(
            resource.identity(),
            format!("mesh-resource/{}", &file_hash["sha256:".len()..])
        );
        assert_eq!(resource.content_hash(), file_hash);
        payload.source = json_payload.source.clone();
        assert_eq!((payload, slots), (json_payload, json_slots));
    }

    #[test]
    fn a_binary_static_mesh_is_admitted_from_its_descriptor_and_resource_header() {
        let binary = static_mesh_fixture("triangle.rstatmsh");
        let refused = |bytes: Vec<u8>| import_static_mesh(bytes).unwrap_err();
        assert_eq!(
            refused(binary[..10].to_vec()),
            "CSHARP_STATIC_MESH_CONTENT_JSON"
        );
        let mut long = binary.clone();
        long[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(refused(long), "CSHARP_STATIC_MESH_CONTENT_JSON");
        assert_eq!(
            refused(binary[..binary.len() - 4].to_vec()),
            "CSHARP_STATIC_MESH_PACK"
        );
        // The streams are not decoded on admission: a changed stream is
        // admitted, under the changed file's identity.
        let (_, _, original) = import_static_mesh(binary.clone()).unwrap();
        let mut changed = binary;
        *changed.last_mut().unwrap() ^= 1;
        let (_, _, admitted) = import_static_mesh(changed.clone()).unwrap();
        assert_ne!(admitted.identity(), original.identity());
        assert_eq!(
            admitted.bytes(),
            &changed[changed.len() - admitted.bytes().len()..]
        );
    }

    #[test]
    fn binary_static_meshes_are_named_by_their_content_not_their_descriptor_hash() {
        // A tool that left the descriptor's hash stale: same descriptor,
        // different streams.
        let first = static_mesh_fixture("triangle.rstatmsh");
        let mut second = first.clone();
        let indices = second.len() - 12;
        for (slot, index) in [0_u32, 2, 1].into_iter().enumerate() {
            second[indices + slot * 4..indices + slot * 4 + 4]
                .copy_from_slice(&index.to_le_bytes());
        }
        let mut imports = RenderResourceImports::default();
        let mut registry = RenderResourceRegistry::default();
        let mut handles = Vec::new();
        for (path, bytes, expected) in [
            ("first.rstatmsh", first, [0, 1, 2]),
            ("second.rstatmsh", second, [0, 2, 1]),
        ] {
            let (payload, _, resource) = imports
                .static_mesh(content(path, Arc::from(bytes)))
                .unwrap();
            let decoded =
                render_model::decode_mesh_resource_payload(&payload, resource.bytes()).unwrap();
            let MeshPayloadSource::Inline { indices, .. } = decoded.source else {
                unreachable!("decoded streams are inline");
            };
            assert_eq!(indices, expected);
            let MeshPayloadSource::Resource {
                resource: named, ..
            } = &payload.source
            else {
                unreachable!("an admitted binary mesh names its resource");
            };
            assert_eq!(named, resource.identity());
            let path = resource.path().to_owned();
            handles.push(registry.stage(resource, [path]).unwrap());
        }
        assert_ne!(handles[0], handles[1], "each file is its own resource");
        for (handle, expected) in handles.iter().zip([[0_u32, 1, 2], [0, 2, 1]]) {
            let resource = registry.get(*handle).unwrap();
            let tail = &resource.bytes()[resource.bytes().len() - 12..];
            let drawn = tail
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| u32::from_le_bytes(*chunk))
                .collect::<Vec<_>>();
            assert_eq!(drawn, expected);
        }
    }

    /// A town-sized static mesh (rusty-dagger's Charing ships 6.9 MB of JSON),
    /// imported from each encoding.
    /// `cargo test --release -p csharp-engine-services --lib measure_static_mesh -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing measurement"]
    fn measure_static_mesh_encodings() {
        let quads = 25_000_u32;
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        for quad in 0..quads {
            // Fractional values, as imported geometry has, for a realistic JSON size.
            let (x, z) = ((quad % 160) as f32 * 1.2537, (quad / 160) as f32 * 1.2491);
            for (dx, dz) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                positions.extend([x + dx, ((quad % 7) as f32) * 0.4831 + 0.0173, z + dz]);
                uvs.extend([dx * 0.9817 + 0.0091, dz * 0.9817 + 0.0091]);
            }
            let base = quad * 4;
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let vertices = positions.len() / 3;
        let normals = [0.0_f32, 1.0, 0.0].repeat(vertices);
        let attribute = |name, components| MeshAttribute {
            name,
            components,
            kind: MeshAttributeKind::F32,
        };
        let asset = StaticMeshAsset {
            asset: "mesh/charing".to_owned(),
            payload: MeshPayloadDescriptor {
                texture_space: None,
                distance_field: None,
                layout: MeshBufferLayout {
                    vertex_count: vertices as u32,
                    index_count: indices.len() as u32,
                    index_width: MeshIndexWidth::U32,
                    attributes: vec![
                        attribute(MeshAttributeName::Position, 3),
                        attribute(MeshAttributeName::Normal, 3),
                        attribute(MeshAttributeName::Uv, 2),
                    ],
                },
                groups: vec![MeshGroupDescriptor {
                    material_slot: 0,
                    start: 0,
                    count: indices.len() as u32,
                }],
                bounds: MeshBoundsDescriptor {
                    min: [0.0, 0.0, 0.0],
                    max: [202.0, 3.0, 201.0],
                },
                source: MeshPayloadSource::Inline {
                    positions,
                    normals,
                    uvs: Some(uvs),
                    colors: None,
                    indices,
                },
                provenance: MeshProvenance::StaticAsset,
                layer_weights: false,
                layer_palette: Vec::new(),
                vertex_occlusion: false,
                morph: None,
            },
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/charing".to_owned(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        };
        let json = serde_json::to_vec(&asset).unwrap();
        let mut packed = pack_mesh_resources(
            std::slice::from_ref(&asset.payload),
            MAX_MESH_RESOURCE_BYTES,
        )
        .unwrap();
        let descriptor = serde_json::to_vec(&StaticMeshAsset {
            payload: packed.payloads.pop().unwrap(),
            ..asset
        })
        .unwrap();
        let mut binary = STATIC_MESH_MAGIC.to_vec();
        binary.extend((descriptor.len() as u32).to_le_bytes());
        binary.extend(descriptor);
        binary.extend(packed.resources.pop().unwrap().bytes);
        for (label, bytes) in [("JSON", json), ("binary", binary)] {
            let size = bytes.len();
            let started = std::time::Instant::now();
            import_static_mesh(bytes).unwrap();
            println!("{label}: {size} bytes, imported in {:?}", started.elapsed());
        }
    }

    #[test]
    fn repeated_animated_opens_reuse_import_but_changed_dependency_reimports() {
        let root: Arc<[u8]> = Arc::from(external_image_glb(CHARACTER_GLB, "texture.png"));
        let mut files = BTreeMap::from([
            ("character.glb".to_owned(), root),
            ("texture.png".to_owned(), Arc::from(RGBA_PNG)),
        ]);
        let content = |files: &BTreeMap<String, Arc<[u8]>>| RetainedContent {
            path: "character.glb".to_owned(),
            identity: Default::default(),
            bytes: Arc::clone(&files["character.glb"]),
            transient: false,
            files: crate::content::ContentFiles::snapshot(files.clone()),
        };
        let mut imports = RenderResourceImports::default();
        let mut registry = RenderResourceRegistry::default();
        for clip_pack in [false, true] {
            let first = registry
                .admit(imports.animated(content(&files), clip_pack).unwrap())
                .unwrap();
            let key = "character.glb";
            let bytes = imports.animated[key].resource.shared_bytes();
            let second = registry
                .admit(imports.animated(content(&files), clip_pack).unwrap())
                .unwrap();
            assert_eq!(first, second);
            assert!(Arc::ptr_eq(
                &bytes,
                &imports.animated[key].resource.shared_bytes()
            ));
            assert_eq!(registry.open_counts[&first], 2);
            assert!(!registry.release_owner(first).unwrap());
            assert!(registry.get(second).is_some());
            assert!(registry.release_owner(second).unwrap());
            registry.remove(second).unwrap();
            assert!(registry.get(second).is_none());
        }
        let old = imports.animated["character.glb"].resource.shared_bytes();
        // A new admitted dependency must not match by root path alone. Even an
        // identical replacement buffer requires fresh closure admission.
        files.insert("texture.png".to_owned(), Arc::from(RGBA_PNG));
        imports.animated(content(&files), false).unwrap();
        assert!(!Arc::ptr_eq(
            &old,
            &imports.animated["character.glb"].resource.shared_bytes()
        ));
        files.remove("texture.png");
        assert!(imports.animated(content(&files), false).is_err());
    }

    const MATERIAL: NativeMaterialHandle = NativeMaterialHandle { value: 1 };

    /// Admits through the lane without a Graphics call: material 1 is live.
    fn admit_generated(
        meshes: &mut GeneratedMeshes,
        request: &NativeMeshResourceCreateRequest,
    ) -> Result<NativeMeshResourceHandle, CsharpEngineServicesError> {
        let (payload, slots, bindings) = unsafe { generated_mesh_payload(request) }?;
        let bound = meshes.bind(payload, slots, bindings, |material| {
            (material == MATERIAL.value).then(|| "material/runtime-1".to_owned())
        })?;
        let handle = bound.handle;
        meshes.insert(bound);
        Ok(NativeMeshResourceHandle { value: handle })
    }

    fn triangle_positions() -> [NativeVec3; 3] {
        [
            NativeVec3::default(),
            NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ]
    }

    const TRIANGLE_NORMALS: [NativeVec3; 3] = [NativeVec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    }; 3];

    #[test]
    fn generated_mesh_layer_weights_travel_as_flagged_colours() {
        let mut meshes = GeneratedMeshes::default();
        let positions = triangle_positions();
        let normals = TRIANGLE_NORMALS;
        let weight = |x, y| csharp_engine_abi::NativeVec4 {
            x,
            y,
            z: 0.0,
            w: 0.0,
        };
        let weights = [weight(1.0, 0.0), weight(0.0, 1.0), weight(0.5, 0.5)];
        let colors = [NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }; 3];
        let indices = [0, 1, 2];
        let groups = [NativeMeshGroup {
            material_slot: 3,
            start: 0,
            count: 3,
        }];
        let bindings = [NativeMeshMaterialBinding {
            material_slot: 3,
            material: MATERIAL,
        }];
        let request = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: std::ptr::null(),
            colors_len: 0,
            indices: indices.as_ptr(),
            indices_len: indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
            layer_weights: weights.as_ptr(),
            layer_weights_len: weights.len(),
        };
        let resource = admit_generated(&mut meshes, &request).unwrap();
        let payload = &meshes.resources[&resource.value].definition.payload;
        assert!(payload.layer_weights);
        let MeshPayloadSource::Inline { colors: stream, .. } = &payload.source else {
            panic!("generated streams are retained inline");
        };
        assert_eq!(
            stream.as_deref(),
            Some(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0][..])
        );
        // A mesh has colours or layer weights, not both.
        let both = NativeMeshResourceCreateRequest {
            colors: colors.as_ptr(),
            colors_len: colors.len(),
            ..request
        };
        assert_eq!(
            admit_generated(&mut meshes, &both).unwrap_err().code(),
            "CSHARP_MESH_ADMISSION"
        );
        assert_eq!(meshes.len(), 1);
    }

    #[test]
    fn generated_mesh_copies_streams_and_refuses_atomically() {
        let mut meshes = GeneratedMeshes::default();
        let mut positions = triangle_positions();
        let normals = TRIANGLE_NORMALS;
        let mut colors = [
            NativeColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            NativeColor {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            NativeColor {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
        ];
        let mut indices = [0, 1, 2];
        let groups = [NativeMeshGroup {
            material_slot: 3,
            start: 0,
            count: 3,
        }];
        let bindings = [NativeMeshMaterialBinding {
            material_slot: 3,
            material: MATERIAL,
        }];
        let request = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: colors.as_ptr(),
            colors_len: colors.len(),
            indices: indices.as_ptr(),
            indices_len: indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
            layer_weights: std::ptr::null(),
            layer_weights_len: 0,
        };
        let resource = admit_generated(&mut meshes, &request).unwrap();
        positions[1].x = 99.0;
        colors[0].r = 0.25;
        indices[1] = 99;
        assert_eq!(positions[1].x, 99.0);
        assert_eq!(colors[0].r, 0.25);
        assert_eq!(indices[1], 99);
        match &meshes.resources[&resource.value].definition.payload.source {
            MeshPayloadSource::Inline {
                positions,
                colors,
                indices,
                ..
            } => {
                assert_eq!(positions[3], 1.0);
                assert_eq!(
                    colors.as_deref(),
                    Some(&[1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0][..])
                );
                assert_eq!(indices, &[0, 1, 2]);
            }
            _ => panic!("generated streams are retained inline"),
        }
        let (collision_positions, triangles) = meshes.copy_inline(resource).unwrap();
        assert_eq!(collision_positions[1], [1.0, 0.0, 0.0]);
        assert_eq!(triangles, [[0, 1, 2]]);
        // Bad indices fail before admission, preserving the prior owner/allocator.
        assert!(admit_generated(&mut meshes, &request).is_err());
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes.next_resource, 2);
        let unrepresentable = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: usize::MAX,
            normals: normals.as_ptr(),
            normals_len: usize::MAX,
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: std::ptr::null(),
            colors_len: 0,
            indices: indices.as_ptr(),
            indices_len: 3,
            groups: groups.as_ptr(),
            groups_len: 1,
            bindings: bindings.as_ptr(),
            bindings_len: 1,
            layer_weights: std::ptr::null(),
            layer_weights_len: 0,
        };
        assert_eq!(
            admit_generated(&mut meshes, &unrepresentable)
                .expect_err("mesh count must fit its layout representation")
                .code(),
            "CSHARP_MESH_ADMISSION"
        );
        // A valid mesh above the former 64 MiB copied-stream cap is admitted.
        // It has no byte-budget preflight or throwaway JSON-size encoding.
        let mut large_positions = vec![NativeVec3::default(); 2_900_000];
        large_positions[1].x = 1.0;
        large_positions[2].y = 1.0;
        let large_normals = vec![TRIANGLE_NORMALS[0]; 2_900_000];
        let large_indices = [0u32, 1, 2];
        let large_request = NativeMeshResourceCreateRequest {
            positions: large_positions.as_ptr(),
            positions_len: large_positions.len(),
            normals: large_normals.as_ptr(),
            normals_len: large_normals.len(),
            indices: large_indices.as_ptr(),
            indices_len: large_indices.len(),
            ..unrepresentable
        };
        assert!(
            std::mem::size_of_val(large_positions.as_slice())
                + std::mem::size_of_val(large_normals.as_slice())
                > 64 * 1024 * 1024
        );
        let large_resource = admit_generated(&mut meshes, &large_request).unwrap();
        meshes.remove(large_resource.value);
        assert_eq!(
            meshes.copy_inline(large_resource).unwrap_err().code(),
            "CSHARP_COLLISION_MESH_STALE"
        );
    }

    #[test]
    fn generated_mesh_accepts_more_than_256_groups_and_bindings() {
        const GROUP_COUNT: usize = 300;

        let mut meshes = GeneratedMeshes::default();
        let positions = triangle_positions();
        let normals = TRIANGLE_NORMALS;
        let indices = (0..GROUP_COUNT)
            .flat_map(|_| [0, 1, 2])
            .collect::<Vec<u32>>();
        let groups = (0..GROUP_COUNT)
            .map(|group_index| NativeMeshGroup {
                material_slot: group_index as u32,
                start: (group_index * 3) as u32,
                count: 3,
            })
            .collect::<Vec<_>>();
        let bindings = (0..GROUP_COUNT)
            .map(|group_index| NativeMeshMaterialBinding {
                material_slot: group_index as u32,
                material: MATERIAL,
            })
            .collect::<Vec<_>>();
        let request = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: std::ptr::null(),
            colors_len: 0,
            indices: indices.as_ptr(),
            indices_len: indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
            layer_weights: std::ptr::null(),
            layer_weights_len: 0,
        };

        let resource = admit_generated(&mut meshes, &request).unwrap();
        assert_eq!(resource.value, 1);
        let definition = &meshes.resources[&resource.value].definition;
        assert_eq!(definition.payload.groups.len(), GROUP_COUNT);
        assert_eq!(definition.material_slots.len(), GROUP_COUNT);

        let malformed = NativeMeshResourceCreateRequest {
            bindings_len: bindings.len() - 1,
            ..request
        };
        assert_eq!(
            admit_generated(&mut meshes, &malformed)
                .expect_err("missing material coverage must fail atomically")
                .code(),
            "CSHARP_MESH_ADMISSION"
        );
        assert_eq!(meshes.len(), 1);
    }
}
