//! Renderer resource admission and the resource registry. Graphics, audio,
//! video and offline output read the same admitted bodies; the decoders and
//! slot bookkeeping stay private here.

use crate::composition::CsharpEngineServicesError;
use crate::content::RetainedContent;
use asset_import::{
    admit_glb_source, glb_relative_resource_uris, import_animated_glb_asset, GlbSourceClosure,
    GltfResource, ImportContext, SourceUri,
};
use csharp_engine_abi::{
    NativeRenderResourceHandle, NativeRenderResourceInfo, NativeRenderResourceKind,
    NativeTextureFilter, NativeTextureWrap,
};
use render_model::{
    mesh_resource_content_hash, pack_mesh_resources, validate_mesh_resource_header,
    AnimatedMeshAsset, MeshMaterialSlot, MeshPayloadDescriptor, PackedMeshResource,
    StaticMeshAsset, TextureDescriptor, TextureFilter, TexturePayloadSource, TextureWrap,
    MAX_MESH_RESOURCE_BYTES,
};
use std::{collections::BTreeMap, sync::Arc};

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
}
