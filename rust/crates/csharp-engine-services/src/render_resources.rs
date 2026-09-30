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
    NativeRenderResourceInfo, NativeRenderResourceKind, NativeTextureFilter, NativeTextureWrap,
    NativeVec3,
};
use render_model::{
    mesh_resource_content_hash, pack_mesh_resources, partition_mesh_spatially,
    validate_mesh_resource_header, AnimatedMeshAsset, MeshAttribute, MeshAttributeKind,
    MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout, MeshCollisionPolicy,
    MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot, MeshPayloadDescriptor,
    MeshPayloadSource, MeshProvenance, PackedMeshResource, StaticMeshAsset, TextureDescriptor,
    TextureFilter, TexturePayloadSource, TextureWrap, MAX_MESH_RESOURCE_BYTES,
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
}

impl CsharpRenderResource {
    fn admit_texture(
        path: String,
        bytes: Arc<[u8]>,
        filter: NativeTextureFilter,
        wrap: NativeTextureWrap,
    ) -> Result<Self, CsharpEngineServicesError> {
        let path = renderer_path(path, ".png")?;
        let mut descriptor = TextureDescriptor::admit_png_rgba8_resource(
            "texture/csharp-product".to_owned(),
            &bytes,
            match filter {
                NativeTextureFilter::Nearest => TextureFilter::Nearest,
                NativeTextureFilter::Linear => TextureFilter::Linear,
            },
            match wrap {
                NativeTextureWrap::Clamp => TextureWrap::Clamp,
                NativeTextureWrap::Repeat => TextureWrap::Repeat,
            },
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
            .clone()
            .expect("resource-backed texture has a content hash");
        let mut identity = format!(
            "texture/csharp-product-{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("Engine texture hash uses SHA-256")
        );
        if filter != NativeTextureFilter::Nearest || wrap != NativeTextureWrap::Clamp {
            identity.push_str(&format!("-f{}-w{}", filter as u32, wrap as u32));
        }
        descriptor.id = identity.clone();
        descriptor.validate().map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_TEXTURE",
                format!("renderer texture identity is invalid: {error:?}"),
            )
        })?;
        let resource_identity = match descriptor.payload.as_ref().map(|payload| &payload.source) {
            Some(TexturePayloadSource::Resource { resource }) => resource.clone(),
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_TEXTURE",
                    "renderer texture payload did not retain its content resource identity",
                ));
            }
        };
        Ok(Self {
            kind: CsharpRenderResourceKind::Texture,
            identity: resource_identity,
            content_hash,
            path,
            bytes,
            texture: Some(descriptor),
            animated_mesh: None,
        })
    }

    fn admit_mesh(
        path: String,
        bytes: Arc<[u8]>,
        content_hash: String,
    ) -> Result<Self, CsharpEngineServicesError> {
        let path = renderer_path(path, ".rmesh")?;
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

    fn admit_font(path: String, bytes: Vec<u8>) -> Result<Self, CsharpEngineServicesError> {
        use sha2::{Digest, Sha256};

        let path = renderer_path(path, ".woff2")?;
        if bytes.len() < 4 || bytes.get(..4) != Some(b"wOF2") {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_FONT",
                "font resource is not an admitted WOFF2 body",
            ));
        }
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let identity = format!(
            "font/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("SHA-256 prefix")
        );
        Ok(Self {
            kind: CsharpRenderResourceKind::Font,
            identity,
            content_hash,
            path,
            bytes: Arc::from(bytes),
            texture: None,
            animated_mesh: None,
        })
    }

    pub(crate) fn admit_audio(
        path: String,
        bytes: Vec<u8>,
    ) -> Result<Self, CsharpEngineServicesError> {
        use sha2::{Digest, Sha256};

        let container = render_model::AudioContainer::identify(&bytes).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_CONTAINER",
                render_model::AUDIO_CONTAINER_POLICY,
            )
        })?;
        let path = renderer_path(path, container.extension())?;
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let identity = format!(
            "audio-resource/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("SHA-256 prefix")
        );
        Ok(Self {
            kind: CsharpRenderResourceKind::Audio,
            identity,
            content_hash,
            path,
            bytes: Arc::from(bytes),
            texture: None,
            animated_mesh: None,
        })
    }

    pub(crate) fn admit_video(
        path: String,
        bytes: Vec<u8>,
    ) -> Result<Self, CsharpEngineServicesError> {
        use sha2::{Digest, Sha256};
        let path = renderer_path(path, ".webm")?;
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
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let identity = format!(
            "video-resource/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("SHA-256 prefix")
        );
        Ok(Self {
            kind: CsharpRenderResourceKind::Video,
            identity,
            content_hash,
            path,
            bytes: Arc::from(bytes),
            texture: None,
            animated_mesh: None,
        })
    }

    fn admit_animated_mesh(
        path: String,
        bytes: Vec<u8>,
    ) -> Result<Self, CsharpEngineServicesError> {
        let path = renderer_path(path, ".glb")?;
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
    paths: BTreeMap<(String, NativeTextureFilter, NativeTextureWrap), u64>,
    identities: BTreeMap<String, u64>,
    /// Each explicit resource admission gives the caller one releasable owner.
    open_counts: BTreeMap<u64, u32>,
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
            let identity = resource.asset_identity().to_owned();
            self.entries.insert(self.next, resource);
            self.next += 1;
            self.identities.insert(identity, handle);
            handle
        };
        for path in paths {
            self.paths.insert((path, filter, wrap), handle);
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
        self.paths.retain(|_, mapped| *mapped != handle);
        self.open_counts.remove(&handle);
        self.released_this_call.push(resource.clone());
        Ok(resource)
    }

    /// A live resource with no caller-owned admission that `in_use` does not
    /// claim.
    pub(crate) fn unowned(&self, in_use: impl Fn(u64) -> bool) -> Option<u64> {
        self.identities
            .values()
            .copied()
            .find(|handle| !self.open_counts.contains_key(handle) && !in_use(*handle))
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
    mesh: BTreeMap<String, CsharpRenderResource>,
    animated: BTreeMap<String, ImportedAnimatedContent>,
}

impl RenderResourceImports {
    /// Decodes a PNG texture, packed mesh or WOFF2 font by its extension.
    /// `content_admitted` says the Content service admitted the source and
    /// already knows its digest.
    pub(crate) fn file(
        &mut self,
        content: RetainedContent,
        filter: NativeTextureFilter,
        wrap: NativeTextureWrap,
        content_admitted: bool,
    ) -> Result<CsharpRenderResource, CsharpEngineServicesError> {
        let relative_path = content.path;
        let requested_path = relative_path.clone();
        let bytes = content.bytes;
        let browser_path = format!("content/{relative_path}");
        match () {
            _ if relative_path.ends_with(".png") => {
                CsharpRenderResource::admit_texture(browser_path.clone(), bytes, filter, wrap)
            }
            _ if relative_path.ends_with(".rmesh") => {
                let resource = match self.mesh.get(&relative_path) {
                    Some(resource) if Arc::ptr_eq(&resource.bytes, &bytes) => resource.clone(),
                    _ => {
                        let hash = if content_admitted {
                            let digest = content.identity.of(&bytes);
                            format!(
                                "sha256:{:016x}{:016x}{:016x}{:016x}",
                                digest.word0, digest.word1, digest.word2, digest.word3
                            )
                        } else {
                            // Standalone source construction has no Content admission.
                            mesh_resource_content_hash(&bytes)
                        };
                        let resource =
                            CsharpRenderResource::admit_mesh(browser_path.clone(), bytes, hash)?;
                        self.mesh.insert(relative_path.clone(), resource.clone());
                        resource
                    }
                };
                Ok(resource)
            }
            _ if relative_path.ends_with(".woff2") => {
                CsharpRenderResource::admit_font(browser_path.clone(), bytes.to_vec())
            }
            _ => Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_KIND",
                format!(
                    "renderer resource `{requested_path}` must be an RGBA PNG, packed .rmesh, or WOFF2 file"
                ),
            )),
        }
    }

    /// Packs a static mesh JSON document into one mesh resource, returned with
    /// the payload and material slots that read it.
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
            let asset = serde_json::from_slice::<StaticMeshAsset>(bytes).map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_STATIC_MESH_CONTENT_JSON", error.to_string())
            })?;
            let mut packed = pack_mesh_resources(&[asset.payload], MAX_MESH_RESOURCE_BYTES)
                .map_err(|error| {
                    CsharpEngineServicesError::new("CSHARP_STATIC_MESH_PACK", format!("{error:?}"))
                })?;
            let payload = packed.payloads.pop().expect("one packed inline payload");
            let packed_resource = packed.resources.pop().expect("one packed inline resource");
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
                            .is_some_and(|current| Arc::ptr_eq(current, bytes))
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
        self.partitions
            .values()
            .any(|partition| partition.material_handles.contains(&material))
            || self
                .resources
                .values()
                .any(|mesh| mesh.material_handles.contains(&material))
    }

    pub(crate) fn remove(&mut self, handle: u64) {
        self.resources.remove(&handle);
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
                .partitions
                .remove(&partition.value);
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
        || request.indices_len < 3
        || !request.indices_len.is_multiple_of(3)
        || request.groups_len == 0
        || request.bindings_len == 0
    {
        return Err(invalid("mesh requires at least 3 vertices, matching normals/optional UV/color streams, complete triangle indices, and nonempty groups/bindings"));
    }
    let positions = borrowed_slice(request.positions, request.positions_len, "mesh positions")?;
    let normals = borrowed_slice(request.normals, request.normals_len, "mesh normals")?;
    let uvs = borrowed_slice(request.uvs, request.uvs_len, "mesh UVs")?;
    let colors = borrowed_slice(request.colors, request.colors_len, "mesh colors")?;
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
    if !colors.is_empty() {
        attributes.push(MeshAttribute {
            name: MeshAttributeName::Color,
            components: 4,
            kind: MeshAttributeKind::F32,
        });
    }
    let payload = MeshPayloadDescriptor {
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
            colors: (!colors.is_empty()).then(|| {
                colors
                    .iter()
                    .flat_map(|value| [value.r, value.g, value.b, value.a])
                    .collect()
            }),
            indices: indices.to_vec(),
        },
        provenance: MeshProvenance::Generated,
    };
    Ok((payload, slots, bindings))
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

struct PackedAnimatedSource {
    bytes: Vec<u8>,
    dependencies: Vec<(String, Arc<[u8]>)>,
}

fn pack_animated_glb_closure(
    root_path: &str,
    root_bytes: &[u8],
    content_resources: &BTreeMap<String, Arc<[u8]>>,
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
            let bytes = content_resources.get(&content_path).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GLB_CLOSURE",
                    format!("animated GLB dependency `{content_path}` is missing"),
                )
            })?;
            dependencies.push((content_path, Arc::clone(bytes)));
            Ok(GltfResource {
                uri,
                bytes: bytes.to_vec(),
            })
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

fn normalize_bundle_path(value: &str) -> Result<String, CsharpEngineServicesError> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'.' | b'-' | b'_' | b'/')
        })
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_RENDER_RESOURCE_PATH",
            "renderer resource path must be a bounded normalized relative ASCII path",
        ));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::appearance::tests::RGBA_PNG;
    use csharp_engine_abi::{NativeColor, NativeMaterialHandle, NativeMeshGroup};

    const CHARACTER_GLB: &[u8] = include_bytes!(
        "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
    );

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
                "pub(crate) fn test_mesh",
                "pub(crate) struct RenderResourceRegistry",
                "pub(crate) fn begin_call",
                "pub(crate) fn recently_released",
                "pub(crate) fn iter",
                "pub(crate) fn len",
                "pub(crate) fn is_empty",
                "pub(crate) fn first",
                "pub(crate) fn get",
                "pub(crate) fn resource",
                "pub(crate) fn animated_mesh_mut",
                "pub(crate) fn stage",
                "pub(crate) fn admit",
                "pub(crate) fn release_owner",
                "pub(crate) fn remove",
                "pub(crate) fn unowned",
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
    fn a_released_body_stays_servable_through_the_next_call() {
        // #8771: output of the releasing call can still name the body; the
        // renderer reads it after the call, so it stays until the call after next.
        let mut registry = RenderResourceRegistry::default();
        registry.begin_call();
        let texture = CsharpRenderResource::admit_texture(
            "content/atlas.png".to_owned(),
            Arc::from(RGBA_PNG),
            NativeTextureFilter::Nearest,
            NativeTextureWrap::Clamp,
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
    fn animated_embedded_image_is_packed_without_external_dependencies() {
        let embedded = external_image_glb(CHARACTER_GLB,
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg==");
        let packed = pack_animated_glb_closure("character.glb", &embedded, &BTreeMap::new())
            .expect("embedded image is packed without companion files");
        assert!(packed.dependencies.is_empty());
        assert_ne!(packed.bytes, embedded);
        let imported = asset_import::import_animated_glb_asset(
            &asset_import::SourceUri::RelativePath("character.glb".to_owned()),
            &packed.bytes,
            &asset_import::ImportContext::default(),
        );
        assert!(!imported.has_errors(), "{:?}", imported.diagnostics);
        let unchanged =
            pack_animated_glb_closure("character.glb", CHARACTER_GLB, &BTreeMap::new()).unwrap();
        assert_eq!(unchanged.bytes, CHARACTER_GLB);
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
            files: Arc::new(files.clone()),
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
