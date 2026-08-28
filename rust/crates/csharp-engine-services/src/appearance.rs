use crate::composition::{borrowed_slice, borrowed_utf8, CsharpEngineServicesError, ABI_OK};
use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use csharp_engine_abi::*;
use render_model::*;
use render_presentation::{
    validate_animation_catalog, AnimationCatalog, AnimationClipAsset, AnimationCondition,
    AnimationControllerService, AnimationGraphDefinition, AnimationMotionDefinition,
    AnimationParameterDefinition, AnimationParameterKind, AnimationParameterValue,
    AnimationProjectionTarget, AnimationProjector, AnimationStateDefinition,
    AnimationTransitionDefinition, AnimationTransitionFactMoment, BillboardAnchor,
    BillboardContent, BillboardDescriptor, BillboardFontRef, BillboardHandle, BillboardLayer,
    BillboardPatch, BillboardProjectionDiagnosticCode, BillboardProjectionOp, BillboardProjector,
    BillboardTextureRef, ParticleAnchor, ParticleEmitterDescriptor, ParticleEmitterHandle,
    ParticleEmitterPatch, ParticleProjectionDiagnosticCode, ParticleProjectionOp,
    ParticleProjector, ParticleSpriteRef, ParticleVisual, PresentationFrameDiff,
    PresentationOpMeta,
};
use render_projection::{
    Appearance, RuntimeAppearanceCatalog, RuntimeAppearanceFact, RuntimeAppearanceProjector,
    RuntimeLightFact,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    sync::Arc,
};

// Renderer resources cross into the product-development host after C# has
// selected them. Keep the pre-split per-resource ceiling at the selection
// boundary so a successful product call is already host-admissible.
const MAX_RENDER_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_INLINE_MESH_RESOURCE_BYTES: u32 = MAX_RENDER_RESOURCE_BYTES as u32;

/// Immutable renderer content selected through the Engine appearance API.
/// Host bundle realization remains the runtime's responsibility.
#[derive(Debug, Clone, PartialEq)]
pub struct CsharpRenderResource {
    kind: CsharpRenderResourceKind,
    identity: String,
    content_hash: String,
    path: String,
    bytes: Vec<u8>,
    animated_mesh: Option<AnimatedMeshAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsharpRenderResourceKind {
    Texture,
    Mesh,
    Font,
    Audio,
    AnimatedMesh,
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
            animated_mesh: None,
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
            animated_mesh: None,
        })
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
        admit_bundle_resource(&path, &bytes)?;
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
            bytes,
            animated_mesh: None,
        })
    }

    pub(crate) fn admit_audio(
        path: String,
        bytes: Vec<u8>,
    ) -> Result<Self, CsharpEngineServicesError> {
        use sha2::{Digest, Sha256};

        let path = renderer_path(path, ".wav")?;
        if bytes.len() < 44 || bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE")
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_WAV",
                "audio resource must be an admitted RIFF/WAVE body",
            ));
        }
        admit_bundle_resource(&path, &bytes)?;
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
            bytes,
            animated_mesh: None,
        })
    }

    pub(crate) fn admit_animated_mesh(
        path: String,
        bytes: Vec<u8>,
    ) -> Result<Self, CsharpEngineServicesError> {
        use sha2::{Digest, Sha256};

        let path = renderer_path(path, ".glb")?;
        admit_bundle_resource(&path, &bytes)?;
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
        if imported.runtime_resource_bytes != bytes {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GLB_ADMISSION",
                "animated GLB admission unexpectedly changed immutable source bytes",
            ));
        }
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        if imported.animated_mesh.content_hash.as_deref() != Some(content_hash.as_str()) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GLB_ADMISSION",
                "animated GLB descriptor content hash did not match admitted source bytes",
            ));
        }
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
            bytes,
            animated_mesh: Some(imported.animated_mesh),
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
    pub(crate) fn animated_mesh(&self) -> Option<&AnimatedMeshAsset> {
        self.animated_mesh.as_ref()
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
    lights: BTreeMap<u64, RuntimeLightFact>,
    next_light: u64,
    materials: BTreeMap<u64, String>,
    appearance_materials: BTreeMap<u64, BTreeSet<u64>>,
    retained_appearances: BTreeMap<u64, u64>,
    next_material: u64,
    retained_object_count: u32,
    retained_light_count: u32,
    pub(crate) render_resources: Vec<CsharpRenderResource>,
    resource_paths: BTreeMap<String, u64>,
    resource_identities: BTreeMap<String, u64>,
    animated_appearances: BTreeMap<u64, u64>,
    animation_instances: BTreeMap<u64, AnimationInstance>,
    animation_graphs: BTreeMap<u64, AnimationGraphBuilder>,
    animation_transitions: BTreeMap<u64, AnimationTransitionRef>,
    animation_controllers: BTreeMap<u64, AnimationController>,
    next_animation_instance: u64,
    next_animation_graph: u64,
    next_animation_transition: u64,
    next_animation_controller: u64,
    billboard_projector: BillboardProjector,
    particle_projector: ParticleProjector,
    billboards: BTreeMap<u64, BillboardHandle>,
    emitters: BTreeMap<u64, ParticleEmitterHandle>,
}

pub(crate) struct RuntimeAppearanceCall {
    pub(crate) state: RuntimeAppearanceState,
    pub(crate) frame: Option<render_model::RenderFrameDiff>,
    pub(crate) extra_frames: Vec<render_model::RenderFrameDiff>,
    pub(crate) presentation: Vec<PresentationFrameDiff>,
    animation_teardown_staged: bool,
}

const MAX_PRESENTATION_DIAGNOSTICS: usize = 128;

#[derive(Clone, Copy)]
struct StoredPresentationDiagnostic {
    domain: NativePresentationDiagnosticDomain,
    receipt: NativePresentationDiagnosticAtReceipt,
}

#[derive(Clone)]
struct AnimationInstance {
    appearance: u64,
    object_id: u64,
    asset: String,
    content_hash: String,
    direct_playback: Option<AnimatedMeshPlaybackCommand>,
    pending_playback: bool,
    last_playback_target: Option<RenderHandle>,
    controller: Option<u64>,
}

#[derive(Clone)]
struct AnimationGraphBuilder {
    resource: u64,
    definition: AnimationGraphDefinition,
    state_order: Vec<String>,
}

#[derive(Clone, Copy)]
struct AnimationTransitionRef {
    graph: u64,
    index: usize,
}

#[derive(Clone)]
struct AnimationController {
    graph: u64,
    instance: u64,
    tick_duration_millis: u32,
    service: AnimationControllerService,
    projector: AnimationProjector,
    projected: bool,
    last_target: Option<RenderHandle>,
    last_revision: Option<u64>,
}

impl RuntimeAppearanceCall {
    pub(crate) fn texture_identity(
        &self,
        handle: u64,
    ) -> Result<String, CsharpEngineServicesError> {
        let index = usize::try_from(handle.saturating_sub(1)).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "invalid resource handle",
            )
        })?;
        let resource = self.state.render_resources.get(index).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "unknown resource handle",
            )
        })?;
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SKY_TEXTURE",
                "sky background requires a selected texture resource",
            ));
        }
        Ok(resource.identity().to_owned())
    }
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
    presentation_diagnostics: Vec<StoredPresentationDiagnostic>,
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
                lights: BTreeMap::new(),
                next_light: 1,
                materials: BTreeMap::new(),
                appearance_materials: BTreeMap::new(),
                retained_appearances: BTreeMap::new(),
                next_material: 1,
                retained_object_count: 0,
                retained_light_count: 0,
                render_resources: Vec::new(),
                resource_paths: BTreeMap::new(),
                resource_identities: BTreeMap::new(),
                animated_appearances: BTreeMap::new(),
                animation_instances: BTreeMap::new(),
                animation_graphs: BTreeMap::new(),
                animation_transitions: BTreeMap::new(),
                animation_controllers: BTreeMap::new(),
                next_animation_instance: 1,
                next_animation_graph: 1,
                next_animation_transition: 1,
                next_animation_controller: 1,
                billboard_projector: BillboardProjector::default(),
                particle_projector: ParticleProjector::default(),
                billboards: BTreeMap::new(),
                emitters: BTreeMap::new(),
            },
            content_resources,
            selection_sealed: false,
            staged: None,
            callback_error: None,
            presentation_diagnostics: Vec::new(),
        }
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeAppearanceCall {
            state: self.state.clone(),
            frame: None,
            extra_frames: Vec::new(),
            presentation: Vec::new(),
            animation_teardown_staged: false,
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

    pub(crate) fn presentation_create_billboard(
        &mut self,
        request: &NativePresentationBillboardDescriptor,
    ) -> Result<NativePresentationBillboardHandle, CsharpEngineServicesError> {
        if request.logical_id == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_BILLBOARD",
                "billboard logical id must be nonzero",
            ));
        }
        let descriptor = self.presentation_billboard_descriptor(request)?;
        let handle = BillboardHandle::new(request.logical_id);
        self.stage_billboard(BillboardProjectionOp::Create { handle, descriptor })?;
        self.staged_mut()?
            .state
            .billboards
            .insert(handle.raw(), handle);
        Ok(NativePresentationBillboardHandle {
            value: request.logical_id,
        })
    }

    pub(crate) fn presentation_update_billboard(
        &mut self,
        owner: NativePresentationBillboardHandle,
        request: &NativePresentationBillboardDescriptor,
    ) -> Result<(), CsharpEngineServicesError> {
        let descriptor = self.presentation_billboard_descriptor(request)?;
        let handle = self
            .staged_mut()?
            .state
            .billboards
            .get(&owner.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_BILLBOARD",
                    "billboard owner is not live",
                )
            })?;
        if request.logical_id != owner.value {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_BILLBOARD",
                "full billboard update must retain its logical id",
            ));
        }
        let patch = BillboardPatch {
            anchor: Some(descriptor.anchor),
            content: Some(descriptor.content),
            font: Some(descriptor.font),
            height_pixels: Some(descriptor.height_pixels),
            color: Some(descriptor.color),
            background: Some(descriptor.background),
            max_distance: Some(descriptor.max_distance),
            layer: Some(descriptor.layer),
            visible: Some(descriptor.visible),
            layout: descriptor.layout,
        };
        self.stage_billboard(BillboardProjectionOp::Update { handle, patch })
    }

    pub(crate) fn presentation_destroy_billboard(
        &mut self,
        owner: NativePresentationBillboardHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = self
            .staged_mut()?
            .state
            .billboards
            .get(&owner.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_BILLBOARD",
                    "billboard owner is not live",
                )
            })?;
        self.stage_billboard(BillboardProjectionOp::Destroy { handle })?;
        self.staged_mut()?.state.billboards.remove(&owner.value);
        Ok(())
    }

    pub(crate) fn presentation_emit_particles(
        &mut self,
        signal_id: NativeUtf8Slice,
        request: &NativePresentationParticleDescriptor,
    ) -> Result<(), CsharpEngineServicesError> {
        let signal_id =
            unsafe { borrowed_utf8(signal_id.bytes, signal_id.len, "particle signal id")? }
                .to_owned();
        let descriptor = self.presentation_particle_descriptor(request)?;
        self.stage_particle(ParticleProjectionOp::Emit {
            signal_id,
            descriptor,
        })
    }

    pub(crate) fn presentation_create_emitter(
        &mut self,
        request: &NativePresentationParticleDescriptor,
    ) -> Result<NativePresentationEmitterHandle, CsharpEngineServicesError> {
        if request.logical_id == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_PARTICLE",
                "particle emitter logical id must be nonzero",
            ));
        }
        let descriptor = self.presentation_particle_descriptor(request)?;
        let handle = ParticleEmitterHandle::new(request.logical_id);
        self.stage_particle(ParticleProjectionOp::Create { handle, descriptor })?;
        self.staged_mut()?
            .state
            .emitters
            .insert(handle.raw(), handle);
        Ok(NativePresentationEmitterHandle {
            value: request.logical_id,
        })
    }

    pub(crate) fn presentation_update_emitter(
        &mut self,
        owner: NativePresentationEmitterHandle,
        request: &NativePresentationParticleDescriptor,
    ) -> Result<(), CsharpEngineServicesError> {
        let descriptor = self.presentation_particle_descriptor(request)?;
        let handle = self
            .staged_mut()?
            .state
            .emitters
            .get(&owner.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_PARTICLE",
                    "emitter owner is not live",
                )
            })?;
        if request.logical_id != owner.value {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_PARTICLE",
                "full particle update must retain its logical id",
            ));
        }
        let patch = ParticleEmitterPatch {
            anchor: Some(descriptor.anchor),
            visual: Some(descriptor.visual),
            sprite: None,
            rate_per_second: Some(descriptor.rate_per_second),
            burst_count: Some(descriptor.burst_count),
            lifetime_seconds: Some(descriptor.lifetime_seconds),
            velocity_min: Some(descriptor.velocity_min),
            velocity_max: Some(descriptor.velocity_max),
            acceleration: Some(descriptor.acceleration),
            size_curve: Some(descriptor.size_curve),
            color_curve: Some(descriptor.color_curve),
            flipbook_frames_per_second: Some(descriptor.flipbook_frames_per_second),
            max_particles: Some(descriptor.max_particles),
            visible: Some(descriptor.visible),
            collision: Some(descriptor.collision),
        };
        self.stage_particle(ParticleProjectionOp::Update { handle, patch })
    }

    pub(crate) fn presentation_destroy_emitter(
        &mut self,
        owner: NativePresentationEmitterHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = self
            .staged_mut()?
            .state
            .emitters
            .get(&owner.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_PARTICLE",
                    "emitter owner is not live",
                )
            })?;
        self.stage_particle(ParticleProjectionOp::Destroy { handle })?;
        self.staged_mut()?.state.emitters.remove(&owner.value);
        Ok(())
    }

    pub(crate) fn presentation_readout(&self) -> NativePresentationFactsReadout {
        let state = self
            .staged
            .as_ref()
            .map(|call| &call.state)
            .unwrap_or(&self.state);
        let billboards = state.billboard_projector.readout();
        let particles = state.particle_projector.readout();
        NativePresentationFactsReadout {
            active_billboards: billboards.active_billboards,
            active_emitters: particles.active_emitters,
            reserved_particles: particles.reserved_particles,
            emitted_bursts: particles.emitted_bursts,
            billboard_diagnostic_count: self
                .presentation_diagnostic_count(NativePresentationDiagnosticDomain::Billboard),
            particle_diagnostic_count: self
                .presentation_diagnostic_count(NativePresentationDiagnosticDomain::Particle),
        }
    }

    pub(crate) fn presentation_diagnostic(
        &self,
        request: NativePresentationDiagnosticAtRequest,
    ) -> NativePresentationDiagnosticAtReceipt {
        self.presentation_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.domain == request.domain)
            .nth(request.index as usize)
            .map(|diagnostic| diagnostic.receipt)
            .unwrap_or_default()
    }

    pub(crate) fn record_callback_error(&mut self, error: CsharpEngineServicesError) {
        self.callback_error = Some(error);
    }

    fn presentation_diagnostic_count(&self, domain: NativePresentationDiagnosticDomain) -> u32 {
        self.presentation_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.domain == domain)
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }

    fn record_presentation_diagnostic(
        &mut self,
        domain: NativePresentationDiagnosticDomain,
        code: NativePresentationDiagnosticCode,
        sequence: u32,
        logical_id: u64,
    ) {
        if self.presentation_diagnostics.len() == MAX_PRESENTATION_DIAGNOSTICS {
            self.presentation_diagnostics.remove(0);
        }
        self.presentation_diagnostics
            .push(StoredPresentationDiagnostic {
                domain,
                receipt: NativePresentationDiagnosticAtReceipt {
                    present: true,
                    code,
                    sequence,
                    logical_id,
                },
            });
    }

    /// Resolves one live C# material into an Engine-owned descriptor for a
    /// separate retained presentation family. The caller copies the returned
    /// value; it never retains this Appearance handle or any product pointer.
    pub(crate) fn voxel_material_descriptor(
        &mut self,
        material: NativeMaterialHandle,
    ) -> Result<RenderMaterialDescriptor, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let id = staged.state.materials.get(&material.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_MATERIAL",
                "voxel-object material handle is not live",
            )
        })?;
        staged
            .state
            .projector
            .resources_mut()
            .materials
            .iter()
            .find(|candidate| candidate.id == *id)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_PRESENTATION_MATERIAL",
                    "voxel-object material descriptor is not retained",
                )
            })
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeAppearanceCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_CALL",
                "appearance service was called outside a product call",
            )
        })
    }

    fn presentation_billboard_descriptor(
        &self,
        request: &NativePresentationBillboardDescriptor,
    ) -> Result<BillboardDescriptor, CsharpEngineServicesError> {
        let key = native_presentation_text(request.localization_key, "billboard localization key")?;
        let fallback = native_presentation_text(request.fallback_text, "billboard fallback text")?;
        let value = native_presentation_text(request.value, "billboard value")?;
        let unit_key = native_presentation_optional_text(request.unit_key, "billboard unit key")?;
        let fallback_unit =
            native_presentation_optional_text(request.fallback_unit, "billboard fallback unit")?;
        let content = match request.content_kind {
            NativeBillboardContentKind::Text => BillboardContent::Text {
                localization_key: key,
                fallback_text: fallback,
                arguments: Vec::new(),
            },
            NativeBillboardContentKind::Value => BillboardContent::Value {
                label_key: key,
                fallback_label: fallback,
                value,
                unit_key,
                fallback_unit,
            },
            NativeBillboardContentKind::Icon => BillboardContent::Icon {
                texture: self.presentation_texture_ref(request.texture)?,
                alt_key: key,
                fallback_alt: fallback,
            },
        };
        Ok(BillboardDescriptor {
            anchor: native_presentation_billboard_anchor(request.anchor),
            content,
            font: self.presentation_font_ref(
                request.font_kind,
                request.font_asset,
                request.font_family,
            )?,
            height_pixels: request.height_pixels,
            color: native_color(request.color),
            background: native_color(request.background),
            max_distance: request.max_distance,
            layer: match request.layer {
                NativePresentationBillboardLayer::AlwaysOnTop => BillboardLayer::AlwaysOnTop,
                NativePresentationBillboardLayer::DepthTested => BillboardLayer::DepthTested,
                NativePresentationBillboardLayer::Occluded => BillboardLayer::Occluded,
            },
            visible: request.visible,
            layout: None,
        })
    }

    fn presentation_particle_descriptor(
        &self,
        request: &NativePresentationParticleDescriptor,
    ) -> Result<ParticleEmitterDescriptor, CsharpEngineServicesError> {
        let visual = match request.visual {
            NativePresentationParticleVisual::Billboard => ParticleVisual::Billboard {
                sprite: ParticleSpriteRef {
                    asset: self.presentation_texture_ref(request.sprite)?.asset,
                    content_hash: self.presentation_texture_ref(request.sprite)?.content_hash,
                    frame_count: request.sprite_frame_count,
                },
            },
            NativePresentationParticleVisual::Cube => ParticleVisual::Cube,
        };
        let size_curve = unsafe {
            borrowed_slice(
                request.size_curve,
                request.size_curve_len,
                "particle size curve",
            )?
        }
        .iter()
        .map(|key| render_presentation::ParticleScalarKey {
            age: key.age,
            value: key.value,
        })
        .collect();
        let color_curve = unsafe {
            borrowed_slice(
                request.color_curve,
                request.color_curve_len,
                "particle color curve",
            )?
        }
        .iter()
        .map(|key| render_presentation::ParticleColorKey {
            age: key.age,
            color: native_color(key.color),
        })
        .collect();
        Ok(ParticleEmitterDescriptor {
            anchor: native_presentation_particle_anchor(request.anchor),
            visual,
            rate_per_second: request.rate_per_second,
            burst_count: request.burst_count,
            lifetime_seconds: [request.lifetime_min_seconds, request.lifetime_max_seconds],
            velocity_min: native_vec3_array(request.velocity_min),
            velocity_max: native_vec3_array(request.velocity_max),
            acceleration: native_vec3_array(request.acceleration),
            size_curve,
            color_curve,
            flipbook_frames_per_second: request.flipbook_frames_per_second,
            seed: request.seed,
            max_particles: request.max_particles,
            visible: request.visible,
            collision: None,
        })
    }

    fn presentation_texture_ref(
        &self,
        resource: NativeRenderResourceHandle,
    ) -> Result<BillboardTextureRef, CsharpEngineServicesError> {
        let resource = self.resource(resource.value)?;
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_TEXTURE",
                "billboard and particle sprites require an admitted texture resource",
            ));
        }
        Ok(BillboardTextureRef {
            asset: resource.identity().to_owned(),
            content_hash: resource.content_hash().to_owned(),
        })
    }

    fn presentation_font_ref(
        &self,
        kind: NativePresentationFontKind,
        asset: NativeRenderResourceHandle,
        family: NativeUtf8Slice,
    ) -> Result<BillboardFontRef, CsharpEngineServicesError> {
        let family = native_presentation_text(family, "billboard font family")?;
        match kind {
            NativePresentationFontKind::System => Ok(BillboardFontRef::System { family }),
            NativePresentationFontKind::Asset => {
                let resource = self.resource(asset.value)?;
                if resource.kind() != CsharpRenderResourceKind::Font {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_PRESENTATION_FONT",
                        "asset billboard font requires an admitted WOFF2 font resource",
                    ));
                }
                Ok(BillboardFontRef::Asset {
                    asset: resource.identity().to_owned(),
                    content_hash: resource.content_hash().to_owned(),
                    family,
                })
            }
        }
    }

    fn stage_billboard(
        &mut self,
        op: BillboardProjectionOp,
    ) -> Result<(), CsharpEngineServicesError> {
        let logical_id = billboard_operation_handle(&op).map_or(0, BillboardHandle::raw);
        let result = {
            let staged = self.staged_mut()?;
            let assets = presentation_assets(&staged.state.render_resources);
            staged
                .state
                .billboard_projector
                .project(&assets, PresentationOpMeta::new(0), op)
        };
        match result {
            Ok(projected) => {
                let mut frame = PresentationFrameDiff::new();
                frame.ops.push(projected);
                self.staged_mut()?.presentation.push(frame);
                Ok(())
            }
            Err(diagnostic) => {
                self.record_presentation_diagnostic(
                    NativePresentationDiagnosticDomain::Billboard,
                    native_billboard_diagnostic_code(diagnostic.code),
                    diagnostic.sequence,
                    diagnostic.handle.map_or(logical_id, BillboardHandle::raw),
                );
                Err(CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_BILLBOARD",
                    diagnostic.message,
                ))
            }
        }
    }

    fn stage_particle(
        &mut self,
        op: ParticleProjectionOp,
    ) -> Result<(), CsharpEngineServicesError> {
        let logical_id = particle_operation_handle(&op).map_or(0, ParticleEmitterHandle::raw);
        let result = {
            let staged = self.staged_mut()?;
            let assets = presentation_assets(&staged.state.render_resources);
            staged
                .state
                .particle_projector
                .project(&assets, PresentationOpMeta::new(0), op)
        };
        match result {
            Ok(projected) => {
                let mut frame = PresentationFrameDiff::new();
                frame.ops.push(projected);
                self.staged_mut()?.presentation.push(frame);
                Ok(())
            }
            Err(diagnostic) => {
                self.record_presentation_diagnostic(
                    NativePresentationDiagnosticDomain::Particle,
                    native_particle_diagnostic_code(diagnostic.code),
                    diagnostic.sequence,
                    diagnostic
                        .handle
                        .map_or(logical_id, ParticleEmitterHandle::raw),
                );
                Err(CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_PARTICLE",
                    diagnostic.message,
                ))
            }
        }
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
            _ if relative_path.ends_with(".woff2") => {
                CsharpRenderResource::admit_font(browser_path.clone(), bytes.to_vec())
            }
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_KIND",
                    format!(
                        "renderer resource `{requested_path}` must be an RGBA PNG, packed .rmesh, or WOFF2 file"
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
                CsharpRenderResourceKind::Texture => NativeRenderResourceKind::Texture,
                CsharpRenderResourceKind::Mesh => NativeRenderResourceKind::StaticMesh,
                CsharpRenderResourceKind::Font => NativeRenderResourceKind::Font,
                CsharpRenderResourceKind::Audio => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_KIND",
                        "audio resources are exposed by the Audio service, not Appearance",
                    ))
                }
                CsharpRenderResourceKind::AnimatedMesh => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_RENDER_RESOURCE_KIND",
                        "animated GLB resources are exposed by the Animation service, not Appearance",
                    ))
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

    fn create_light(
        &mut self,
        request: NativeLightRequest,
    ) -> Result<NativeLightHandle, CsharpEngineServicesError> {
        let fact = runtime_light_fact(request)?;
        let staged = self.staged_mut()?;
        if staged
            .state
            .lights
            .values()
            .any(|candidate| candidate.light_id == fact.light_id)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_LIGHT_LOGICAL_ID",
                "logical light id is already owned by a live light",
            ));
        }
        let handle = staged.state.next_light;
        staged.state.next_light = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_LIGHT_HANDLE", "light handle overflow")
        })?;
        staged.state.lights.insert(handle, fact);
        project_staged_lights(staged)?;
        Ok(NativeLightHandle { value: handle })
    }

    fn update_light(
        &mut self,
        request: NativeLightUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let replacement = runtime_light_fact(request.replacement)?;
        let staged = self.staged_mut()?;
        if !staged.state.lights.contains_key(&request.light.value) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_LIGHT_HANDLE",
                "light handle is not live",
            ));
        }
        if staged.state.lights.iter().any(|(handle, candidate)| {
            *handle != request.light.value && candidate.light_id == replacement.light_id
        }) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_LIGHT_LOGICAL_ID",
                "logical light id is already owned by a different live light",
            ));
        }
        staged.state.lights.insert(request.light.value, replacement);
        project_staged_lights(staged)
    }

    fn replace_light(
        &mut self,
        request: NativeLightUpdateRequest,
    ) -> Result<NativeLightHandle, CsharpEngineServicesError> {
        self.destroy_light(request.light)?;
        self.create_light(request.replacement)
    }

    fn destroy_light(&mut self, light: NativeLightHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged.state.lights.remove(&light.value).is_some() {
            project_staged_lights(staged)?;
        }
        // A successful replacement turns the prior generated owner into a
        // tombstone, so a later IDisposable release is ordinary teardown.
        Ok(())
    }

    fn read_light(
        &mut self,
        light: NativeLightHandle,
    ) -> Result<NativeLightReadout, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_LIGHT_CALL",
                "light service was called outside a product call",
            )
        })?;
        let fact = staged.state.lights.get(&light.value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_LIGHT_HANDLE", "light handle is not live")
        })?;
        Ok(native_light_readout(fact))
    }

    fn create_material(
        &mut self,
        request: NativeMaterialRequest,
    ) -> Result<NativeMaterialHandle, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let handle = staged.state.next_material;
        staged.state.next_material = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_HANDLE", "material handle overflow")
        })?;
        let id = format!("material/csharp-{handle}");
        let descriptor = material_descriptor(id.clone(), request, &staged.state.render_resources)?;
        staged
            .state
            .projector
            .resources_mut()
            .materials
            .push(descriptor);
        staged.state.materials.insert(handle, id);
        Ok(NativeMaterialHandle { value: handle })
    }

    fn update_material(
        &mut self,
        request: NativeMaterialUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let id = staged
            .state
            .materials
            .get(&request.material.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_MATERIAL_HANDLE",
                    "material handle is not live",
                )
            })?;
        let descriptor = material_descriptor(
            id.clone(),
            request.replacement,
            &staged.state.render_resources,
        )?;
        let material = staged
            .state
            .projector
            .resources_mut()
            .materials
            .iter_mut()
            .find(|material| material.id == id)
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_MATERIAL", "material catalog drifted")
            })?;
        *material = descriptor;
        Ok(())
    }

    fn replace_material(
        &mut self,
        request: NativeMaterialUpdateRequest,
    ) -> Result<NativeMaterialHandle, CsharpEngineServicesError> {
        self.destroy_material(request.material)?;
        self.create_material(request.replacement)
    }

    fn destroy_material(
        &mut self,
        material: NativeMaterialHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(id) = staged.state.materials.remove(&material.value) else {
            // A successful replacement turns the prior generated owner into a
            // tombstone. Its later IDisposable release is normal teardown.
            return Ok(());
        };
        if staged
            .state
            .appearance_materials
            .values()
            .any(|bindings| bindings.contains(&material.value))
        {
            staged.state.materials.insert(material.value, id);
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_IN_USE",
                "dispose appearances using this material before disposing the material",
            ));
        }
        let resources = staged.state.projector.resources_mut();
        resources.materials.retain(|candidate| candidate.id != id);
        Ok(())
    }

    fn destroy_appearance(
        &mut self,
        appearance: NativeAppearanceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged
            .state
            .retained_appearances
            .values()
            .any(|retained| *retained == appearance.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_IN_USE",
                "publish a snapshot without this appearance before disposing or replacing it",
            ));
        }
        if staged
            .state
            .animation_instances
            .values()
            .any(|instance| instance.appearance == appearance.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_APPEARANCE_IN_USE",
                "dispose animation instances using this appearance before disposing or replacing it",
            ));
        }
        let Some(identity) = staged.state.appearances.remove(&appearance.value) else {
            // Match the other generated retained owners: replacement-first
            // then owner disposal is safe and has no renderer side channel.
            return Ok(());
        };
        staged.state.appearance_materials.remove(&appearance.value);
        staged.state.animated_appearances.remove(&appearance.value);
        staged.state.projector.remove_appearance(&identity);
        Ok(())
    }

    fn replace_primitive(
        &mut self,
        request: NativePrimitiveAppearanceReplaceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        self.destroy_appearance(request.appearance)?;
        self.create_primitive(request.replacement)
    }

    unsafe fn update_static_mesh_materials(
        &mut self,
        request: &NativeStaticMeshMaterialUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let bindings = borrowed_slice(
            request.bindings,
            request.bindings_len,
            "static mesh material bindings",
        )?;
        let staged = self.staged_mut()?;
        let identity = staged
            .state
            .appearances
            .get(&request.appearance.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_APPEARANCE_HANDLE",
                    "appearance handle is not live",
                )
            })?;
        let mut slots = BTreeSet::new();
        let mut material_overrides = Vec::with_capacity(bindings.len());
        let mut material_handles = BTreeSet::new();
        for binding in bindings {
            let slot = u16::try_from(binding.material_slot).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_SLOT",
                    "mesh material slot exceeded u16",
                )
            })?;
            if !slots.insert(slot) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_SLOT",
                    "mesh material bindings must not repeat a slot",
                ));
            }
            let material = staged
                .state
                .materials
                .get(&binding.material.value)
                .cloned()
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_MATERIAL_HANDLE",
                        "material handle is not live",
                    )
                })?;
            material_overrides.push(MeshMaterialSlot { slot, material });
            material_handles.insert(binding.material.value);
        }
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::StaticMesh {
                material_overrides: current,
                ..
            }) => {
                *current = material_overrides;
            }
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_APPEARANCE",
                    "material bindings require a live static mesh appearance",
                ))
            }
        }
        staged
            .state
            .appearance_materials
            .insert(request.appearance.value, material_handles);
        Ok(())
    }

    fn create_primitive(
        &mut self,
        request: NativePrimitiveAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let geometry = match request.geometry {
            NativePrimitiveGeometry::Cube => Geometry::Cube,
            NativePrimitiveGeometry::Sphere => Geometry::Sphere,
            NativePrimitiveGeometry::Quad => Geometry::Quad,
            NativePrimitiveGeometry::Point => Geometry::Point,
        };
        self.allocate_appearance(Appearance::Primitive {
            geometry,
            material: Material {
                color: native_color(request.color),
                wireframe: request.wireframe,
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
            NativeBillboardMode::None => BillboardMode::None,
            NativeBillboardMode::Spherical => BillboardMode::Spherical,
            NativeBillboardMode::Cylindrical => BillboardMode::Cylindrical,
        };
        let size_mode = match request.size_mode {
            NativeSpriteSizeMode::World => SpriteSizeMode::World,
            NativeSpriteSizeMode::Pixel => SpriteSizeMode::Pixel,
        };
        let depth = match request.depth {
            NativeSpriteDepthPolicy::Default => SpriteDepthPolicy::Default,
            NativeSpriteDepthPolicy::DepthTestOff => SpriteDepthPolicy::DepthTestOff,
            NativeSpriteDepthPolicy::DepthWriteOff => SpriteDepthPolicy::DepthWriteOff,
        };
        self.allocate_appearance(Appearance::Sprite {
            sprite: SpriteInstanceDescriptor {
                asset: atlas_id,
                frame: 0,
                pivot: native_vec2(request.pivot),
                size: native_vec2(request.size),
                size_mode,
                billboard,
                tint: native_color(request.tint),
                render_order: request.render_order,
                depth,
                shading: SpriteShading::Unlit,
                material: SpriteMaterialDescriptor::default(),
                visible: true,
                transform: Transform::IDENTITY,
                attachment: SpriteAttachment::default(),
                metadata: RenderMetadata::default(),
            },
        })
    }

    fn open_animated_mesh(
        &mut self,
        request: &NativeAnimatedMeshResourceRequest,
    ) -> Result<NativeRenderResourceHandle, CsharpEngineServicesError> {
        let requested_path = unsafe {
            borrowed_utf8(
                request.path.bytes,
                request.path.len,
                "animated mesh resource path",
            )?
            .to_owned()
        };
        if let Some(handle) = self
            .staged
            .as_ref()
            .and_then(|staged| staged.state.resource_paths.get(&requested_path))
            .copied()
        {
            if self.resource(handle)?.kind() == CsharpRenderResourceKind::AnimatedMesh {
                return Ok(NativeRenderResourceHandle { value: handle });
            }
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE_KIND",
                "this content path is already admitted as a different renderer resource kind",
            ));
        }
        if self.selection_sealed {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE_SELECTION_CLOSED",
                "animated GLB resources must be selected during product Create",
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
                    "CSHARP_ANIMATION_RESOURCE_UNKNOWN",
                    format!("product content has no animated GLB `{requested_path}`"),
                )
            })?;
        let browser_path = format!("content/{relative_path}");
        let resource =
            CsharpRenderResource::admit_animated_mesh(browser_path.clone(), bytes.to_vec())?;
        let handle =
            self.stage_resource(resource, [browser_path, relative_path, requested_path])?;
        Ok(NativeRenderResourceHandle { value: handle })
    }

    fn create_animated_mesh_appearance(
        &mut self,
        request: NativeAnimatedMeshAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let resource = self.resource(request.resource.value)?.clone();
        let asset = resource.animated_mesh().cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE_KIND",
                "animated mesh appearance requires an admitted animated GLB resource",
            )
        })?;
        {
            let staged = self.staged_mut()?;
            if !staged
                .state
                .projector
                .resources_mut()
                .animated_meshes
                .iter()
                .any(|candidate| candidate.asset == asset.asset)
            {
                staged
                    .state
                    .projector
                    .resources_mut()
                    .animated_meshes
                    .push(asset.clone());
            }
        }
        let appearance = self.allocate_appearance(Appearance::AnimatedMesh {
            asset: asset.asset,
            material_overrides: Vec::new(),
            playback: None,
        })?;
        self.staged_mut()?
            .state
            .animated_appearances
            .insert(appearance.value, request.resource.value);
        Ok(appearance)
    }

    fn replace_animated_mesh_appearance(
        &mut self,
        appearance: NativeAppearanceHandle,
        request: NativeAnimatedMeshAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        self.destroy_appearance(appearance)?;
        self.create_animated_mesh_appearance(request)
    }

    fn create_animation_instance(
        &mut self,
        request: NativeAnimationInstanceRequest,
    ) -> Result<NativeAnimationInstanceHandle, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let resource = staged
            .state
            .animated_appearances
            .get(&request.appearance.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_APPEARANCE",
                    "animation instances require a live animated-mesh appearance",
                )
            })?;
        if staged
            .state
            .animation_instances
            .values()
            .any(|instance| instance.object_id == request.object_id)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE_OBJECT",
                "a product object may have only one retained animation instance",
            ));
        }
        let mesh = staged
            .state
            .render_resources
            .get(usize::try_from(resource.saturating_sub(1)).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_RESOURCE",
                    "animated resource handle overflow",
                )
            })?)
            .and_then(CsharpRenderResource::animated_mesh)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_RESOURCE",
                    "animated appearance resource is unavailable",
                )
            })?;
        let handle = staged.state.next_animation_instance;
        staged.state.next_animation_instance = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE",
                "animation instance handles exhausted",
            )
        })?;
        staged.state.animation_instances.insert(
            handle,
            AnimationInstance {
                appearance: request.appearance.value,
                object_id: request.object_id,
                asset: mesh.asset.clone(),
                content_hash: mesh.content_hash.clone().unwrap_or_default(),
                direct_playback: None,
                pending_playback: false,
                last_playback_target: None,
                controller: None,
            },
        );
        Ok(NativeAnimationInstanceHandle { value: handle })
    }

    fn destroy_animation_instance(
        &mut self,
        handle: NativeAnimationInstanceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_instances
            .get(&handle.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        if instance.controller.is_some() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE_IN_USE",
                "dispose the animation controller before disposing its instance",
            ));
        }
        if staged.frame.is_some() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                "dispose an animation instance in a product call separate from PublishSnapshot",
            ));
        }
        if let Some(target) = instance.last_playback_target {
            let frame = render_model::RenderFrameDiff::try_from_ops(vec![
                render_model::RenderDiff::SetAnimatedMeshPlayback {
                    handle: target,
                    playback: AnimatedMeshPlaybackCommand::Stop { fade_seconds: None },
                },
            ])
            .map_err(|error| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_FRAME",
                    format!("animation teardown frame is invalid: {error:?}"),
                )
            })?;
            staged.extra_frames.push(frame);
        }
        staged.animation_teardown_staged = true;
        staged.state.animation_instances.remove(&handle.value);
        Ok(())
    }

    fn replace_animation_instance(
        &mut self,
        prior: NativeAnimationInstanceHandle,
        request: NativeAnimationInstanceRequest,
    ) -> Result<NativeAnimationInstanceHandle, CsharpEngineServicesError> {
        self.destroy_animation_instance(prior)?;
        self.create_animation_instance(request)
    }

    fn create_animation_graph(
        &mut self,
        request: &NativeAnimationGraphCreateRequest,
    ) -> Result<NativeAnimationGraphHandle, CsharpEngineServicesError> {
        let graph_id = unsafe {
            borrowed_utf8(
                request.graph_id.bytes,
                request.graph_id.len,
                "animation graph id",
            )?
        }
        .to_owned();
        let initial_state_id = unsafe {
            borrowed_utf8(
                request.initial_state_id.bytes,
                request.initial_state_id.len,
                "animation initial state id",
            )?
        }
        .to_owned();
        if request.version == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GRAPH",
                "animation graph version must be non-zero",
            ));
        }
        let asset_id = self
            .resource(request.resource.value)?
            .animated_mesh()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GRAPH_RESOURCE",
                    "animation graph requires an admitted animated GLB",
                )
            })?
            .asset
            .clone();
        let staged = self.staged_mut()?;
        let handle = staged.state.next_animation_graph;
        staged.state.next_animation_graph = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GRAPH",
                "animation graph handles exhausted",
            )
        })?;
        staged.state.animation_graphs.insert(
            handle,
            AnimationGraphBuilder {
                resource: request.resource.value,
                definition: AnimationGraphDefinition {
                    graph_id,
                    version: request.version,
                    asset_id,
                    initial_state_id,
                    parameters: Vec::new(),
                    states: Vec::new(),
                    transitions: Vec::new(),
                },
                state_order: Vec::new(),
            },
        );
        Ok(NativeAnimationGraphHandle { value: handle })
    }

    fn destroy_animation_graph(
        &mut self,
        graph: NativeAnimationGraphHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged
            .state
            .animation_controllers
            .values()
            .any(|controller| controller.graph == graph.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GRAPH_IN_USE",
                "dispose controllers using this graph before disposing it",
            ));
        }
        if staged.state.animation_graphs.remove(&graph.value).is_none() {
            return Ok(());
        }
        staged
            .state
            .animation_transitions
            .retain(|_, transition| transition.graph != graph.value);
        Ok(())
    }

    fn define_animation_parameter(
        &mut self,
        request: &NativeAnimationParameterDefinitionRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let parameter_id = unsafe {
            borrowed_utf8(
                request.parameter_id.bytes,
                request.parameter_id.len,
                "animation parameter id",
            )?
        }
        .to_owned();
        let kind = match request.kind {
            NativeAnimationParameterKind::Float => AnimationParameterKind::Float,
            NativeAnimationParameterKind::Bool => AnimationParameterKind::Bool,
            NativeAnimationParameterKind::Trigger => AnimationParameterKind::Trigger,
        };
        let default_value = match kind {
            AnimationParameterKind::Float => {
                AnimationParameterValue::Float(request.float_default_milli)
            }
            AnimationParameterKind::Bool => AnimationParameterValue::Bool(request.bool_default),
            AnimationParameterKind::Trigger => {
                AnimationParameterValue::Trigger(request.bool_default)
            }
        };
        let graph = self
            .staged_mut()?
            .state
            .animation_graphs
            .get_mut(&request.graph.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GRAPH",
                    "animation graph is not live",
                )
            })?;
        graph
            .definition
            .parameters
            .push(AnimationParameterDefinition {
                parameter_id,
                kind,
                default_value,
            });
        Ok(())
    }

    fn define_animation_state(
        &mut self,
        request: &NativeAnimationStateDefinitionRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let state_id = unsafe {
            borrowed_utf8(
                request.state_id.bytes,
                request.state_id.len,
                "animation state id",
            )?
        }
        .to_owned();
        let clip_a =
            unsafe { borrowed_utf8(request.clip_a.bytes, request.clip_a.len, "animation clip a")? }
                .to_owned();
        let clip_b =
            unsafe { borrowed_utf8(request.clip_b.bytes, request.clip_b.len, "animation clip b")? }
                .to_owned();
        let parameter_id = unsafe {
            borrowed_utf8(
                request.parameter_id.bytes,
                request.parameter_id.len,
                "animation blend parameter",
            )?
        }
        .to_owned();
        let motion = match request.motion_kind {
            NativeAnimationMotionKind::Clip => AnimationMotionDefinition::Clip {
                clip_id: clip_a,
                speed_milli: request.speed_milli,
            },
            NativeAnimationMotionKind::LinearBlend => AnimationMotionDefinition::LinearBlend {
                parameter_id,
                low_clip_id: clip_a,
                high_clip_id: clip_b,
                minimum_milli: request.minimum_milli,
                maximum_milli: request.maximum_milli,
                speed_milli: request.speed_milli,
            },
        };
        let graph = self
            .staged_mut()?
            .state
            .animation_graphs
            .get_mut(&request.graph.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GRAPH",
                    "animation graph is not live",
                )
            })?;
        graph.state_order.push(state_id.clone());
        graph
            .definition
            .states
            .push(AnimationStateDefinition { state_id, motion });
        Ok(())
    }

    fn define_animation_transition(
        &mut self,
        request: &NativeAnimationTransitionDefinitionRequest,
    ) -> Result<NativeAnimationTransitionHandle, CsharpEngineServicesError> {
        let transition_id = unsafe {
            borrowed_utf8(
                request.transition_id.bytes,
                request.transition_id.len,
                "animation transition id",
            )?
        }
        .to_owned();
        let from_state_id = unsafe {
            borrowed_utf8(
                request.from_state_id.bytes,
                request.from_state_id.len,
                "animation source state",
            )?
        }
        .to_owned();
        let to_state_id = unsafe {
            borrowed_utf8(
                request.to_state_id.bytes,
                request.to_state_id.len,
                "animation target state",
            )?
        }
        .to_owned();
        let staged = self.staged_mut()?;
        let graph = staged
            .state
            .animation_graphs
            .get_mut(&request.graph.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GRAPH",
                    "animation graph is not live",
                )
            })?;
        let priority = u16::try_from(request.priority).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_TRANSITION",
                "animation transition priority exceeded u16",
            )
        })?;
        graph
            .definition
            .transitions
            .push(AnimationTransitionDefinition {
                transition_id,
                from_state_id,
                to_state_id,
                priority,
                duration_ticks: request.duration_ticks,
                conditions: Vec::new(),
            });
        let index = graph.definition.transitions.len() - 1;
        let handle = staged.state.next_animation_transition;
        staged.state.next_animation_transition = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_TRANSITION",
                "animation transition handles exhausted",
            )
        })?;
        staged.state.animation_transitions.insert(
            handle,
            AnimationTransitionRef {
                graph: request.graph.value,
                index,
            },
        );
        Ok(NativeAnimationTransitionHandle { value: handle })
    }

    fn define_animation_condition(
        &mut self,
        request: &NativeAnimationConditionDefinitionRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let parameter_id = unsafe {
            borrowed_utf8(
                request.parameter_id.bytes,
                request.parameter_id.len,
                "animation condition parameter",
            )?
        }
        .to_owned();
        let condition = match request.kind {
            NativeAnimationConditionKind::FloatGreaterThan => {
                AnimationCondition::FloatGreaterThan {
                    parameter_id,
                    threshold_milli: request.threshold_milli,
                }
            }
            NativeAnimationConditionKind::FloatLessThanOrEqual => {
                AnimationCondition::FloatLessThanOrEqual {
                    parameter_id,
                    threshold_milli: request.threshold_milli,
                }
            }
            NativeAnimationConditionKind::BoolEquals => AnimationCondition::BoolEquals {
                parameter_id,
                value: request.bool_value,
            },
            NativeAnimationConditionKind::TriggerSet => {
                AnimationCondition::TriggerSet { parameter_id }
            }
        };
        let staged = self.staged_mut()?;
        let reference = staged
            .state
            .animation_transitions
            .get(&request.transition.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_TRANSITION",
                    "animation transition is not live",
                )
            })?;
        let transition = staged
            .state
            .animation_graphs
            .get_mut(&reference.graph)
            .and_then(|graph| graph.definition.transitions.get_mut(reference.index))
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_TRANSITION",
                    "animation transition drifted from its graph",
                )
            })?;
        transition.conditions.push(condition);
        Ok(())
    }

    fn create_animation_controller(
        &mut self,
        request: NativeAnimationControllerCreateRequest,
    ) -> Result<NativeAnimationControllerHandle, CsharpEngineServicesError> {
        if request.tick_duration_millis == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_TICK_DURATION",
                "animation controller tick duration must be non-zero",
            ));
        }
        let staged = self.staged_mut()?;
        let graph = staged
            .state
            .animation_graphs
            .get(&request.graph.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GRAPH",
                    "animation graph is not live",
                )
            })?;
        let instance = staged
            .state
            .animation_instances
            .get(&request.instance.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        if instance.controller.is_some() || instance.direct_playback.is_some() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE_MODE",
                "an animation instance cannot mix direct playback with a controller",
            ));
        }
        let resource = staged
            .state
            .render_resources
            .get(
                usize::try_from(graph.resource.saturating_sub(1)).map_err(|_| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_RESOURCE",
                        "animation resource handle overflow",
                    )
                })?,
            )
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_RESOURCE",
                    "animation graph resource is unavailable",
                )
            })?;
        let mesh = resource.animated_mesh().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE",
                "animation graph resource is not an animated GLB",
            )
        })?;
        if graph.definition.asset_id != instance.asset || mesh.asset != instance.asset {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GRAPH_ASSET",
                "animation graph and instance must reference the same animated GLB",
            ));
        }
        let assets = BTreeMap::from([(
            mesh.asset.clone(),
            ResolvedRenderAsset {
                id: mesh.asset.clone(),
                kind: RenderAssetKind::AnimatedMesh,
                content_hash: mesh.content_hash.clone(),
                version: 0,
            },
        )]);
        let catalog = validate_animation_catalog(
            AnimationCatalog {
                schema_version: 1,
                catalog_id: format!("csharp/{}", graph.definition.graph_id),
                assets: vec![AnimationClipAsset {
                    asset_id: mesh.asset.clone(),
                    content_hash: mesh.content_hash.clone().unwrap_or_default(),
                    clips: mesh.clips.iter().map(|clip| clip.id.clone()).collect(),
                }],
                graphs: vec![graph.definition.clone()],
            },
            &assets,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_ANIMATION_GRAPH", error.to_string())
        })?;
        let mut service = AnimationControllerService::new(catalog);
        service
            .attach(instance.object_id, graph.definition.graph_id.clone())
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        let handle = staged.state.next_animation_controller;
        staged.state.next_animation_controller = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CONTROLLER",
                "animation controller handles exhausted",
            )
        })?;
        staged
            .state
            .animation_instances
            .get_mut(&request.instance.value)
            .expect("validated instance remains live")
            .controller = Some(handle);
        staged.state.animation_controllers.insert(
            handle,
            AnimationController {
                graph: request.graph.value,
                instance: request.instance.value,
                tick_duration_millis: request.tick_duration_millis,
                service,
                projector: AnimationProjector::new(),
                projected: false,
                last_target: None,
                last_revision: None,
            },
        );
        Ok(NativeAnimationControllerHandle { value: handle })
    }

    fn destroy_animation_controller(
        &mut self,
        handle: NativeAnimationControllerHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged.frame.is_some() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                "dispose an animation controller in a product call separate from PublishSnapshot",
            ));
        }
        let Some(controller) = staged.state.animation_controllers.remove(&handle.value) else {
            return Ok(());
        };
        if controller.projected {
            let sequence = u32::try_from(staged.presentation.len()).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_PRESENTATION",
                    "too many animation presentation frames in one product call",
                )
            })?;
            let instance = staged
                .state
                .animation_instances
                .get(&controller.instance)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_INSTANCE",
                        "animation controller instance is not live",
                    )
                })?;
            let mut projector = controller.projector;
            let op = projector
                .destroy_entity(instance.object_id, PresentationOpMeta::new(sequence))
                .map_err(|diagnostic| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_PROJECTION",
                        diagnostic.message,
                    )
                })?;
            let mut frame = PresentationFrameDiff::new();
            frame.ops.push(op);
            staged.presentation.push(frame);
        }
        if let Some(instance) = staged
            .state
            .animation_instances
            .get_mut(&controller.instance)
        {
            instance.controller = None;
        }
        staged.animation_teardown_staged = true;
        Ok(())
    }

    fn set_animation_float(
        &mut self,
        request: &NativeAnimationSetFloatRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let parameter_id = unsafe {
            borrowed_utf8(
                request.parameter_id.bytes,
                request.parameter_id.len,
                "animation float parameter",
            )?
        }
        .to_owned();
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_controllers
            .get(&request.controller.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CONTROLLER",
                    "animation controller is not live",
                )
            })?
            .instance;
        let entity = staged
            .state
            .animation_instances
            .get(&instance)
            .expect("controller instance remains live")
            .object_id;
        let controller = staged
            .state
            .animation_controllers
            .get_mut(&request.controller.value)
            .expect("controller remains live");
        controller
            .service
            .set_float(entity, parameter_id, request.value_milli)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        self.flush_animation_controller(request.controller.value)
    }

    fn set_animation_bool(
        &mut self,
        request: &NativeAnimationSetBoolRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let parameter_id = unsafe {
            borrowed_utf8(
                request.parameter_id.bytes,
                request.parameter_id.len,
                "animation bool parameter",
            )?
        }
        .to_owned();
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_controllers
            .get(&request.controller.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CONTROLLER",
                    "animation controller is not live",
                )
            })?
            .instance;
        let entity = staged
            .state
            .animation_instances
            .get(&instance)
            .expect("controller instance remains live")
            .object_id;
        let controller = staged
            .state
            .animation_controllers
            .get_mut(&request.controller.value)
            .expect("controller remains live");
        controller
            .service
            .set_bool(entity, parameter_id, request.value)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        self.flush_animation_controller(request.controller.value)
    }

    fn fire_animation_trigger(
        &mut self,
        request: &NativeAnimationFireTriggerRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let parameter_id = unsafe {
            borrowed_utf8(
                request.parameter_id.bytes,
                request.parameter_id.len,
                "animation trigger parameter",
            )?
        }
        .to_owned();
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_controllers
            .get(&request.controller.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CONTROLLER",
                    "animation controller is not live",
                )
            })?
            .instance;
        let entity = staged
            .state
            .animation_instances
            .get(&instance)
            .expect("controller instance remains live")
            .object_id;
        let controller = staged
            .state
            .animation_controllers
            .get_mut(&request.controller.value)
            .expect("controller remains live");
        controller
            .service
            .fire_trigger(entity, parameter_id)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        self.flush_animation_controller(request.controller.value)
    }

    fn tick_animation(
        &mut self,
        request: NativeAnimationTickRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_controllers
            .get(&request.controller.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CONTROLLER",
                    "animation controller is not live",
                )
            })?
            .instance;
        let entity = staged
            .state
            .animation_instances
            .get(&instance)
            .expect("controller instance remains live")
            .object_id;
        let controller = staged
            .state
            .animation_controllers
            .get_mut(&request.controller.value)
            .expect("controller remains live");
        controller
            .service
            .tick(entity, request.tick)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        self.flush_animation_controller(request.controller.value)
    }

    fn set_animation_playback(
        &mut self,
        request: &NativeAnimationPlaybackRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let clip = unsafe {
            borrowed_utf8(
                request.clip.bytes,
                request.clip.len,
                "animation playback clip",
            )?
        }
        .to_owned();
        let command = match request.kind {
            NativeAnimationPlaybackKind::Play => AnimatedMeshPlaybackCommand::Play {
                clip,
                r#loop: match request.loop_mode {
                    NativeAnimationLoopMode::Once => AnimationLoopMode::Once,
                    NativeAnimationLoopMode::Repeat => AnimationLoopMode::Repeat,
                    NativeAnimationLoopMode::PingPong => AnimationLoopMode::PingPong,
                },
                speed: request.speed,
                weight: request.weight,
                restart: request.restart,
                fade_seconds: request.has_fade.then_some(request.fade_seconds),
            },
            NativeAnimationPlaybackKind::Stop => AnimatedMeshPlaybackCommand::Stop {
                fade_seconds: request.has_fade.then_some(request.fade_seconds),
            },
            NativeAnimationPlaybackKind::Sample => AnimatedMeshPlaybackCommand::Sample {
                clip,
                normalized_time: request.normalized_time,
            },
            NativeAnimationPlaybackKind::Pause => AnimatedMeshPlaybackCommand::Pause,
            NativeAnimationPlaybackKind::Resume => AnimatedMeshPlaybackCommand::Resume,
        };
        command.validate().map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_PLAYBACK",
                format!("invalid playback command: {error:?}"),
            )
        })?;
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_instances
            .get_mut(&request.instance.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        if instance.controller.is_some() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE_MODE",
                "direct playback cannot be mixed with a controller on one instance",
            ));
        }
        if matches!(
            command,
            AnimatedMeshPlaybackCommand::Play { .. } | AnimatedMeshPlaybackCommand::Sample { .. }
        ) && !animation_asset_has_clip(
            &staged.state.render_resources,
            &instance.asset,
            command_clip(&command).unwrap_or_default(),
        ) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP",
                "playback references a clip absent from the admitted animated GLB",
            ));
        }
        instance.direct_playback = Some(command);
        instance.pending_playback = true;
        self.flush_direct_playback(request.instance.value)
    }

    fn flush_direct_playback(
        &mut self,
        instance_handle: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let instance = staged
            .state
            .animation_instances
            .get(&instance_handle)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        let Some(playback) = instance.direct_playback else {
            return Ok(());
        };
        let Some(handle) = staged.state.projector.object_handle(instance.object_id) else {
            if let Some(instance) = staged.state.animation_instances.get_mut(&instance_handle) {
                instance.last_playback_target = None;
            }
            return Ok(());
        };
        if !instance.pending_playback && instance.last_playback_target == Some(handle) {
            return Ok(());
        }
        let frame = render_model::RenderFrameDiff::try_from_ops(vec![
            render_model::RenderDiff::SetAnimatedMeshPlayback { handle, playback },
        ])
        .map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_FRAME",
                format!("animation playback frame is invalid: {error:?}"),
            )
        })?;
        staged.extra_frames.push(frame);
        let instance = staged
            .state
            .animation_instances
            .get_mut(&instance_handle)
            .expect("instance remains live while staged");
        instance.pending_playback = false;
        instance.last_playback_target = Some(handle);
        Ok(())
    }

    fn flush_animation_controller(
        &mut self,
        controller_handle: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let (instance, assets, sequence) = {
            let controller = staged
                .state
                .animation_controllers
                .get(&controller_handle)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_CONTROLLER",
                        "animation controller is not live",
                    )
                })?;
            let instance = staged
                .state
                .animation_instances
                .get(&controller.instance)
                .cloned()
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_INSTANCE",
                        "animation controller instance is not live",
                    )
                })?;
            let assets = animation_assets(&staged.state.render_resources);
            let sequence = u32::try_from(staged.presentation.len()).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_PRESENTATION",
                    "too many animation presentation frames in one product call",
                )
            })?;
            (instance, assets, sequence)
        };
        let Some(target) = staged.state.projector.object_handle(instance.object_id) else {
            let controller = staged
                .state
                .animation_controllers
                .get_mut(&controller_handle)
                .expect("controller was checked above");
            if controller.projected {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                    "remove a projected controller before publishing a snapshot that removes or replaces its animated target",
                ));
            }
            controller.projected = false;
            controller.last_target = None;
            controller.last_revision = None;
            controller.projector.reset();
            return Ok(());
        };
        let controller = staged
            .state
            .animation_controllers
            .get_mut(&controller_handle)
            .expect("controller was checked above");
        let state = controller
            .service
            .state(instance.object_id)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        if controller.projected
            && controller.last_target == Some(target)
            && controller.last_revision == Some(state.revision)
        {
            return Ok(());
        }
        let targets = BTreeSet::from([target]);
        let meta = PresentationOpMeta::new(sequence);
        let op = if controller.projected {
            controller
                .projector
                .update_for_state(&assets, &targets, &state, meta)
        } else {
            controller.projector.create_for_state(
                &assets,
                &targets,
                AnimationProjectionTarget {
                    target,
                    content_hash: instance.content_hash,
                    tick_duration_millis: controller.tick_duration_millis,
                },
                &state,
                meta,
            )
        }
        .map_err(|diagnostic| {
            CsharpEngineServicesError::new("CSHARP_ANIMATION_PROJECTION", diagnostic.message)
        })?;
        controller.projected = true;
        controller.last_target = Some(target);
        controller.last_revision = Some(state.revision);
        let mut frame = PresentationFrameDiff::new();
        frame.ops.push(op);
        staged.presentation.push(frame);
        Ok(())
    }

    fn flush_all_animations(&mut self) -> Result<(), CsharpEngineServicesError> {
        let (direct, controllers) = {
            let staged = self.staged.as_ref().ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CALL",
                    "animation service was called outside a product call",
                )
            })?;
            (
                staged
                    .state
                    .animation_instances
                    .iter()
                    .filter_map(|(handle, instance)| {
                        instance.direct_playback.is_some().then_some(*handle)
                    })
                    .collect::<Vec<_>>(),
                staged
                    .state
                    .animation_controllers
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
            )
        };
        for instance in direct {
            self.flush_direct_playback(instance)?;
        }
        for controller in controllers {
            self.flush_animation_controller(controller)?;
        }
        Ok(())
    }

    fn read_animation_controller(
        &mut self,
        handle: NativeAnimationControllerHandle,
    ) -> Result<NativeAnimationControllerReadout, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CALL",
                "animation service was called outside a product call",
            )
        })?;
        let controller = staged
            .state
            .animation_controllers
            .get(&handle.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_CONTROLLER",
                    "animation controller is not live",
                )
            })?;
        let instance = staged
            .state
            .animation_instances
            .get(&controller.instance)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation controller instance is not live",
                )
            })?;
        let graph = staged
            .state
            .animation_graphs
            .get(&controller.graph)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_GRAPH",
                    "animation controller graph is not live",
                )
            })?;
        let state = controller
            .service
            .state(instance.object_id)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_CONTROLLER", error.to_string())
            })?;
        let clips = animation_asset_clips(&staged.state.render_resources, &instance.asset);
        let index = |values: &[String], value: &str| {
            values
                .iter()
                .position(|candidate| candidate == value)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(u32::MAX)
        };
        let (from, to, elapsed, duration) = state
            .transition
            .as_ref()
            .map(|transition| {
                (
                    index(&graph.state_order, &transition.from_state_id),
                    index(&graph.state_order, &transition.to_state_id),
                    transition.elapsed_ticks,
                    transition.duration_ticks,
                )
            })
            .unwrap_or((u32::MAX, u32::MAX, 0, 0));
        let moment = match state.transition_fact.as_ref().map(|fact| fact.moment) {
            Some(AnimationTransitionFactMoment::Started) => {
                NativeAnimationTransitionMoment::Started
            }
            Some(AnimationTransitionFactMoment::Completed) => {
                NativeAnimationTransitionMoment::Completed
            }
            None => NativeAnimationTransitionMoment::None,
        };
        Ok(NativeAnimationControllerReadout {
            state_index: index(&graph.state_order, &state.current_state_id),
            clip_a_index: index(&clips, &state.motion.clip_a),
            clip_b_index: state
                .motion
                .clip_b
                .as_deref()
                .map(|clip| index(&clips, clip))
                .unwrap_or(u32::MAX),
            blend_weight_milli: state.motion.blend_weight_milli,
            speed_milli: state.motion.speed_milli,
            revision: state.revision,
            controller_tick: state.controller_tick,
            transition_from_state_index: from,
            transition_to_state_index: to,
            transition_elapsed_ticks: elapsed,
            transition_duration_ticks: duration,
            transition_moment: moment,
        })
    }

    fn read_animation(&mut self) -> Result<NativeAnimationReadout, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CALL",
                "animation service was called outside a product call",
            )
        })?;
        Ok(NativeAnimationReadout {
            admitted_meshes: u32::try_from(
                staged
                    .state
                    .render_resources
                    .iter()
                    .filter(|resource| resource.kind() == CsharpRenderResourceKind::AnimatedMesh)
                    .count(),
            )
            .unwrap_or(u32::MAX),
            retained_instances: u32::try_from(staged.state.animation_instances.len())
                .unwrap_or(u32::MAX),
            retained_graphs: u32::try_from(staged.state.animation_graphs.len()).unwrap_or(u32::MAX),
            retained_controllers: u32::try_from(staged.state.animation_controllers.len())
                .unwrap_or(u32::MAX),
            pending_playback_commands: u32::try_from(
                staged
                    .state
                    .animation_instances
                    .values()
                    .filter(|instance| instance.pending_playback)
                    .count(),
            )
            .unwrap_or(u32::MAX),
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
        let mut retained_appearances = BTreeMap::new();
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
                visible: fact.visible,
                layer: native_render_layer(fact.layer)?,
            });
            retained_appearances.insert(fact.object_id, fact.appearance.value);
        }
        let staged = self.staged_mut()?;
        if staged.animation_teardown_staged {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                "PublishSnapshot cannot share a product call with animation instance or controller disposal",
            ));
        }
        for controller in staged.state.animation_controllers.values() {
            if !controller.projected {
                continue;
            }
            let instance = staged
                .state
                .animation_instances
                .get(&controller.instance)
                .expect("live controller retains its instance");
            if retained_appearances.get(&instance.object_id) != Some(&instance.appearance) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                    "remove a projected controller before publishing a snapshot that removes or replaces its animated target",
                ));
            }
        }
        let projection = staged.state.projector.project(&owned).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_VISUAL_SNAPSHOT", format!("{error:?}"))
        })?;
        staged.state.retained_object_count = narrow_retained_count(
            projection.retained_objects,
            "retained object count exceeded u32",
        )?;
        staged.state.retained_light_count = narrow_retained_count(
            projection.retained_lights,
            "retained light count exceeded u32",
        )?;
        staged.state.retained_appearances = retained_appearances;
        append_projection_frame(staged, projection.frame)?;
        self.flush_all_animations()?;
        Ok(())
    }
}

fn animation_assets(resources: &[CsharpRenderResource]) -> BTreeMap<String, ResolvedRenderAsset> {
    resources
        .iter()
        .filter_map(|resource| resource.animated_mesh())
        .map(|mesh| {
            (
                mesh.asset.clone(),
                ResolvedRenderAsset {
                    id: mesh.asset.clone(),
                    kind: RenderAssetKind::AnimatedMesh,
                    content_hash: mesh.content_hash.clone(),
                    version: 0,
                },
            )
        })
        .collect()
}

fn presentation_assets(
    resources: &[CsharpRenderResource],
) -> BTreeMap<String, ResolvedRenderAsset> {
    resources
        .iter()
        .filter(|resource| {
            matches!(
                resource.kind(),
                CsharpRenderResourceKind::Texture | CsharpRenderResourceKind::Font
            )
        })
        .map(|resource| {
            (
                resource.identity().to_owned(),
                ResolvedRenderAsset {
                    id: resource.identity().to_owned(),
                    kind: match resource.kind() {
                        CsharpRenderResourceKind::Texture => RenderAssetKind::Texture,
                        CsharpRenderResourceKind::Font => RenderAssetKind::Font,
                        _ => unreachable!("presentation assets only include textures and fonts"),
                    },
                    content_hash: Some(resource.content_hash().to_owned()),
                    version: 0,
                },
            )
        })
        .collect()
}

fn native_presentation_billboard_anchor(value: NativePresentationAnchor) -> BillboardAnchor {
    match value.kind {
        NativePresentationAnchorKind::World => BillboardAnchor::World {
            position: native_vec3_array(value.position),
        },
        NativePresentationAnchorKind::EntityAttached => BillboardAnchor::EntityAttached {
            entity: value.entity,
            offset: native_vec3_array(value.offset),
        },
    }
}

fn native_presentation_particle_anchor(value: NativePresentationAnchor) -> ParticleAnchor {
    match value.kind {
        NativePresentationAnchorKind::World => ParticleAnchor::World {
            position: native_vec3_array(value.position),
        },
        NativePresentationAnchorKind::EntityAttached => ParticleAnchor::EntityAttached {
            entity: value.entity,
            offset: native_vec3_array(value.offset),
        },
    }
}

fn billboard_operation_handle(operation: &BillboardProjectionOp) -> Option<BillboardHandle> {
    match operation {
        BillboardProjectionOp::Create { handle, .. }
        | BillboardProjectionOp::Update { handle, .. }
        | BillboardProjectionOp::Destroy { handle } => Some(*handle),
    }
}

fn particle_operation_handle(operation: &ParticleProjectionOp) -> Option<ParticleEmitterHandle> {
    match operation {
        ParticleProjectionOp::Emit { .. } => None,
        ParticleProjectionOp::Create { handle, .. }
        | ParticleProjectionOp::Update { handle, .. }
        | ParticleProjectionOp::Destroy { handle } => Some(*handle),
    }
}

fn native_presentation_text(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<String, CsharpEngineServicesError> {
    Ok(unsafe { borrowed_utf8(value.bytes, value.len, field)? }.to_owned())
}

fn native_presentation_optional_text(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<Option<String>, CsharpEngineServicesError> {
    if value.len == 0 {
        return Ok(None);
    }
    native_presentation_text(value, field).map(Some)
}

fn native_billboard_diagnostic_code(
    value: BillboardProjectionDiagnosticCode,
) -> NativePresentationDiagnosticCode {
    match value {
        BillboardProjectionDiagnosticCode::InvalidDescriptor => {
            NativePresentationDiagnosticCode::InvalidDescriptor
        }
        BillboardProjectionDiagnosticCode::AssetMissing => {
            NativePresentationDiagnosticCode::AssetMissing
        }
        BillboardProjectionDiagnosticCode::AssetKindMismatch => {
            NativePresentationDiagnosticCode::AssetKindMismatch
        }
        BillboardProjectionDiagnosticCode::ContentHashMismatch => {
            NativePresentationDiagnosticCode::ContentHashMismatch
        }
        BillboardProjectionDiagnosticCode::DuplicateHandle => {
            NativePresentationDiagnosticCode::DuplicateHandle
        }
        BillboardProjectionDiagnosticCode::UnknownHandle => {
            NativePresentationDiagnosticCode::UnknownHandle
        }
        BillboardProjectionDiagnosticCode::AnchorMissing => {
            NativePresentationDiagnosticCode::AnchorMissing
        }
        BillboardProjectionDiagnosticCode::UnavailableHost => {
            NativePresentationDiagnosticCode::UnavailableHost
        }
        BillboardProjectionDiagnosticCode::FontLoadFailed => {
            NativePresentationDiagnosticCode::FontLoadFailed
        }
        BillboardProjectionDiagnosticCode::IconLoadFailed => {
            NativePresentationDiagnosticCode::IconOrSpriteLoadFailed
        }
        BillboardProjectionDiagnosticCode::HostFailure => {
            NativePresentationDiagnosticCode::HostFailure
        }
    }
}

fn native_particle_diagnostic_code(
    value: ParticleProjectionDiagnosticCode,
) -> NativePresentationDiagnosticCode {
    match value {
        ParticleProjectionDiagnosticCode::InvalidDescriptor => {
            NativePresentationDiagnosticCode::InvalidDescriptor
        }
        ParticleProjectionDiagnosticCode::AssetMissing => {
            NativePresentationDiagnosticCode::AssetMissing
        }
        ParticleProjectionDiagnosticCode::AssetKindMismatch => {
            NativePresentationDiagnosticCode::AssetKindMismatch
        }
        ParticleProjectionDiagnosticCode::ContentHashMismatch => {
            NativePresentationDiagnosticCode::ContentHashMismatch
        }
        ParticleProjectionDiagnosticCode::DuplicateSignal => {
            NativePresentationDiagnosticCode::DuplicateSignal
        }
        ParticleProjectionDiagnosticCode::DuplicateHandle => {
            NativePresentationDiagnosticCode::DuplicateHandle
        }
        ParticleProjectionDiagnosticCode::UnknownHandle => {
            NativePresentationDiagnosticCode::UnknownHandle
        }
        ParticleProjectionDiagnosticCode::AnchorMissing => {
            NativePresentationDiagnosticCode::AnchorMissing
        }
        ParticleProjectionDiagnosticCode::BudgetExceeded => {
            NativePresentationDiagnosticCode::BudgetExceeded
        }
        ParticleProjectionDiagnosticCode::UnavailableHost => {
            NativePresentationDiagnosticCode::UnavailableHost
        }
        ParticleProjectionDiagnosticCode::SpriteLoadFailed => {
            NativePresentationDiagnosticCode::IconOrSpriteLoadFailed
        }
        ParticleProjectionDiagnosticCode::HostFailure => {
            NativePresentationDiagnosticCode::HostFailure
        }
    }
}

fn animation_asset_clips(resources: &[CsharpRenderResource], asset: &str) -> Vec<String> {
    resources
        .iter()
        .filter_map(CsharpRenderResource::animated_mesh)
        .find(|mesh| mesh.asset == asset)
        .map(|mesh| {
            mesh.clips
                .iter()
                .map(|clip| clip.id.clone())
                .chain(
                    mesh.clip_packs
                        .iter()
                        .flat_map(|pack| pack.clips.iter().map(|clip| clip.id.clone())),
                )
                .collect()
        })
        .unwrap_or_default()
}

fn animation_asset_has_clip(resources: &[CsharpRenderResource], asset: &str, clip: &str) -> bool {
    animation_asset_clips(resources, asset)
        .iter()
        .any(|candidate| candidate == clip)
}

fn command_clip(command: &AnimatedMeshPlaybackCommand) -> Option<&str> {
    match command {
        AnimatedMeshPlaybackCommand::Play { clip, .. }
        | AnimatedMeshPlaybackCommand::Sample { clip, .. } => Some(clip),
        AnimatedMeshPlaybackCommand::Stop { .. }
        | AnimatedMeshPlaybackCommand::Pause
        | AnimatedMeshPlaybackCommand::Resume => None,
    }
}

fn native_render_layer(value: NativeRenderLayer) -> Result<RenderLayer, CsharpEngineServicesError> {
    match value {
        NativeRenderLayer::Scene => Ok(RenderLayer::Scene),
        NativeRenderLayer::Debug => Ok(RenderLayer::Debug),
        NativeRenderLayer::Ui => Ok(RenderLayer::Ui),
        NativeRenderLayer::Viewmodel => Ok(RenderLayer::Viewmodel),
    }
}

fn project_staged_lights(
    staged: &mut RuntimeAppearanceCall,
) -> Result<(), CsharpEngineServicesError> {
    let facts: Vec<RuntimeLightFact> = staged.state.lights.values().cloned().collect();
    let projection = staged
        .state
        .projector
        .project_lights(&facts)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_LIGHT_PROJECTION", format!("{error:?}"))
        })?;
    staged.state.retained_object_count = narrow_retained_count(
        projection.retained_objects,
        "retained object count exceeded u32",
    )?;
    staged.state.retained_light_count = narrow_retained_count(
        projection.retained_lights,
        "retained light count exceeded u32",
    )?;
    append_projection_frame(staged, projection.frame)
}

fn append_projection_frame(
    staged: &mut RuntimeAppearanceCall,
    next: render_model::RenderFrameDiff,
) -> Result<(), CsharpEngineServicesError> {
    let Some(previous) = staged.frame.take() else {
        staged.frame = Some(next);
        return Ok(());
    };
    let mut operations = previous.ops;
    operations.extend(next.ops);
    staged.frame = Some(
        render_model::RenderFrameDiff::try_from_ops(operations).map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_FRAME",
                format!("combined retained appearance/light frame is invalid: {error:?}"),
            )
        })?,
    );
    Ok(())
}

fn narrow_retained_count(
    value: usize,
    message: &'static str,
) -> Result<u32, CsharpEngineServicesError> {
    u32::try_from(value)
        .map_err(|_| CsharpEngineServicesError::new("CSHARP_PRESENTATION_READOUT", message))
}

fn runtime_light_fact(
    request: NativeLightRequest,
) -> Result<RuntimeLightFact, CsharpEngineServicesError> {
    let shadow_intent = match request.descriptor.shadow_intent {
        NativeLightShadowIntent::Disabled => LightShadowIntent::Disabled,
        NativeLightShadowIntent::Requested => LightShadowIntent::Requested,
    };
    let color = native_vec3_array(request.descriptor.color);
    let position = native_vec3_array(request.descriptor.position);
    let direction = native_vec3_array(request.descriptor.direction);
    let range = request
        .descriptor
        .has_range
        .then_some(request.descriptor.range);
    let light = match request.descriptor.kind {
        NativeLightKind::Ambient => LightDescriptor::Ambient {
            color,
            intensity: request.descriptor.intensity,
            enabled: request.descriptor.enabled,
            shadow_intent,
        },
        NativeLightKind::Directional => LightDescriptor::Directional {
            color,
            intensity: request.descriptor.intensity,
            enabled: request.descriptor.enabled,
            direction,
            shadow_intent,
        },
        NativeLightKind::Point => LightDescriptor::Point {
            color,
            intensity: request.descriptor.intensity,
            enabled: request.descriptor.enabled,
            position,
            range,
            decay: request.descriptor.decay,
            shadow_intent,
        },
        NativeLightKind::Spot => LightDescriptor::Spot {
            color,
            intensity: request.descriptor.intensity,
            enabled: request.descriptor.enabled,
            position,
            direction,
            range,
            decay: request.descriptor.decay,
            outer_angle_radians: request.descriptor.outer_angle_radians,
            penumbra: request.descriptor.penumbra,
            shadow_intent,
        },
    };
    light.validate().map_err(|error| {
        CsharpEngineServicesError::new(
            "CSHARP_LIGHT_DESCRIPTOR",
            format!("invalid light descriptor: {error:?}"),
        )
    })?;
    Ok(RuntimeLightFact {
        light_id: request.logical_id,
        parent_object_id: request
            .has_parent_object
            .then_some(request.parent_object_id),
        light,
    })
}

fn native_light_readout(fact: &RuntimeLightFact) -> NativeLightReadout {
    let (
        kind,
        color,
        intensity,
        enabled,
        position,
        direction,
        range,
        decay,
        outer_angle_radians,
        penumbra,
        shadow_intent,
    ) = match &fact.light {
        LightDescriptor::Ambient {
            color,
            intensity,
            enabled,
            shadow_intent,
        } => (
            NativeLightKind::Ambient,
            *color,
            *intensity,
            *enabled,
            [0.0; 3],
            [0.0; 3],
            None,
            0.0,
            0.0,
            0.0,
            *shadow_intent,
        ),
        LightDescriptor::Directional {
            color,
            intensity,
            enabled,
            direction,
            shadow_intent,
        } => (
            NativeLightKind::Directional,
            *color,
            *intensity,
            *enabled,
            [0.0; 3],
            *direction,
            None,
            0.0,
            0.0,
            0.0,
            *shadow_intent,
        ),
        LightDescriptor::Point {
            color,
            intensity,
            enabled,
            position,
            range,
            decay,
            shadow_intent,
        } => (
            NativeLightKind::Point,
            *color,
            *intensity,
            *enabled,
            *position,
            [0.0; 3],
            *range,
            *decay,
            0.0,
            0.0,
            *shadow_intent,
        ),
        LightDescriptor::Spot {
            color,
            intensity,
            enabled,
            position,
            direction,
            range,
            decay,
            outer_angle_radians,
            penumbra,
            shadow_intent,
        } => (
            NativeLightKind::Spot,
            *color,
            *intensity,
            *enabled,
            *position,
            *direction,
            *range,
            *decay,
            *outer_angle_radians,
            *penumbra,
            *shadow_intent,
        ),
    };
    NativeLightReadout {
        logical_id: fact.light_id,
        has_parent_object: fact.parent_object_id.is_some(),
        parent_object_id: fact.parent_object_id.unwrap_or_default(),
        descriptor: NativeLightDescriptor {
            kind,
            color: NativeVec3 {
                x: color[0],
                y: color[1],
                z: color[2],
            },
            intensity,
            enabled,
            position: NativeVec3 {
                x: position[0],
                y: position[1],
                z: position[2],
            },
            direction: NativeVec3 {
                x: direction[0],
                y: direction[1],
                z: direction[2],
            },
            has_range: range.is_some(),
            range: range.unwrap_or_default(),
            decay,
            outer_angle_radians,
            penumbra,
            shadow_intent: match shadow_intent {
                LightShadowIntent::Disabled => NativeLightShadowIntent::Disabled,
                LightShadowIntent::Requested => NativeLightShadowIntent::Requested,
            },
        },
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

pub(crate) unsafe extern "C" fn replace_primitive_appearance(
    context: *mut c_void,
    request: NativePrimitiveAppearanceReplaceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    appearance_result(context, result, |bridge| bridge.replace_primitive(request))
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

pub(crate) unsafe extern "C" fn replace_static_mesh_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    request: *const NativeStaticMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    appearance_result(context, result, |bridge| unsafe {
        let request = &*request;
        bridge.destroy_appearance(appearance)?;
        bridge.create_static_mesh(request)
    })
}

pub(crate) unsafe extern "C" fn replace_static_mesh_from_content_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    request: *const NativeStaticMeshContentAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    appearance_result(context, result, |bridge| {
        let request = unsafe { &*request };
        bridge.destroy_appearance(appearance)?;
        bridge.create_static_mesh_from_content(request)
    })
}

pub(crate) unsafe extern "C" fn update_static_mesh_materials(
    context: *mut c_void,
    request: *const NativeStaticMeshMaterialUpdateRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    appearance_void(context, |bridge| unsafe {
        bridge.update_static_mesh_materials(&*request)
    })
}

pub(crate) unsafe extern "C" fn create_sprite_appearance(
    context: *mut c_void,
    request: NativeSpriteAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    appearance_result(context, result, |bridge| bridge.create_sprite(request))
}

pub(crate) unsafe extern "C" fn replace_sprite_appearance(
    context: *mut c_void,
    request: NativeSpriteAppearanceReplaceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    appearance_result(context, result, |bridge| {
        bridge.destroy_appearance(request.appearance)?;
        bridge.create_sprite(request.replacement)
    })
}

pub(crate) unsafe extern "C" fn destroy_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
) -> i32 {
    appearance_void(context, |bridge| bridge.destroy_appearance(appearance))
}

pub(crate) unsafe extern "C" fn create_light(
    context: *mut c_void,
    request: NativeLightRequest,
    result: *mut NativeLightHandle,
) -> i32 {
    light_result(context, result, |bridge| bridge.create_light(request))
}

pub(crate) unsafe extern "C" fn update_light(
    context: *mut c_void,
    request: NativeLightUpdateRequest,
) -> i32 {
    appearance_void(context, |bridge| bridge.update_light(request))
}

pub(crate) unsafe extern "C" fn replace_light(
    context: *mut c_void,
    request: NativeLightUpdateRequest,
    result: *mut NativeLightHandle,
) -> i32 {
    light_result(context, result, |bridge| bridge.replace_light(request))
}

pub(crate) unsafe extern "C" fn destroy_light(
    context: *mut c_void,
    light: NativeLightHandle,
) -> i32 {
    appearance_void(context, |bridge| bridge.destroy_light(light))
}

pub(crate) unsafe extern "C" fn read_light(
    context: *mut c_void,
    light: NativeLightHandle,
    result: *mut NativeLightReadout,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    match bridge.read_light(light) {
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

pub(crate) unsafe extern "C" fn create_material(
    context: *mut c_void,
    request: NativeMaterialRequest,
    result: *mut NativeMaterialHandle,
) -> i32 {
    material_result(context, result, |bridge| bridge.create_material(request))
}

pub(crate) unsafe extern "C" fn update_material(
    context: *mut c_void,
    request: NativeMaterialUpdateRequest,
) -> i32 {
    appearance_void(context, |bridge| bridge.update_material(request))
}

pub(crate) unsafe extern "C" fn replace_material(
    context: *mut c_void,
    request: NativeMaterialUpdateRequest,
    result: *mut NativeMaterialHandle,
) -> i32 {
    material_result(context, result, |bridge| bridge.replace_material(request))
}

pub(crate) unsafe extern "C" fn destroy_material(
    context: *mut c_void,
    material: NativeMaterialHandle,
) -> i32 {
    appearance_void(context, |bridge| bridge.destroy_material(material))
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

fn light_result(
    context: *mut c_void,
    result: *mut NativeLightHandle,
    action: impl FnOnce(
        &mut RuntimeAppearanceBridge,
    ) -> Result<NativeLightHandle, CsharpEngineServicesError>,
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

fn material_result(
    context: *mut c_void,
    result: *mut NativeMaterialHandle,
    action: impl FnOnce(
        &mut RuntimeAppearanceBridge,
    ) -> Result<NativeMaterialHandle, CsharpEngineServicesError>,
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

fn appearance_void(
    context: *mut c_void,
    action: impl FnOnce(&mut RuntimeAppearanceBridge) -> Result<(), CsharpEngineServicesError>,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    match action(bridge) {
        Ok(()) => ABI_OK,
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

pub(crate) unsafe extern "C" fn read_presentation(
    context: *mut c_void,
    result: *mut NativePresentationReadout,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    let state = bridge
        .staged
        .as_ref()
        .map(|call| &call.state)
        .unwrap_or(&bridge.state);
    let resource_count = match u32::try_from(state.render_resources.len()) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let appearance_count = match u32::try_from(state.appearances.len()) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let material_count = match u32::try_from(state.materials.len()) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    unsafe {
        *result = NativePresentationReadout {
            retained_object_count: state.retained_object_count,
            appearance_count,
            material_count,
            resource_count,
        };
    }
    ABI_OK
}

fn animation_result<T: Copy>(
    context: *mut c_void,
    result: *mut T,
    action: impl FnOnce(&mut RuntimeAppearanceBridge) -> Result<T, CsharpEngineServicesError>,
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

fn animation_void(
    context: *mut c_void,
    action: impl FnOnce(&mut RuntimeAppearanceBridge) -> Result<(), CsharpEngineServicesError>,
) -> i32 {
    appearance_void(context, action)
}

pub(crate) unsafe extern "C" fn open_animated_mesh(
    context: *mut c_void,
    request: *const NativeAnimatedMeshResourceRequest,
    result: *mut NativeRenderResourceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.open_animated_mesh(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn create_animated_mesh_appearance(
    context: *mut c_void,
    request: *const NativeAnimatedMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.create_animated_mesh_appearance(unsafe { *request })
    })
}
pub(crate) unsafe extern "C" fn replace_animated_mesh_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    request: *const NativeAnimatedMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.replace_animated_mesh_appearance(appearance, unsafe { *request })
    })
}
pub(crate) unsafe extern "C" fn destroy_animated_mesh_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
) -> i32 {
    animation_void(context, |bridge| bridge.destroy_appearance(appearance))
}
pub(crate) unsafe extern "C" fn create_animation_instance(
    context: *mut c_void,
    request: *const NativeAnimationInstanceRequest,
    result: *mut NativeAnimationInstanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.create_animation_instance(unsafe { *request })
    })
}
pub(crate) unsafe extern "C" fn destroy_animation_instance(
    context: *mut c_void,
    value: NativeAnimationInstanceHandle,
) -> i32 {
    animation_void(context, |bridge| bridge.destroy_animation_instance(value))
}
pub(crate) unsafe extern "C" fn replace_animation_instance(
    context: *mut c_void,
    prior: NativeAnimationInstanceHandle,
    request: *const NativeAnimationInstanceRequest,
    result: *mut NativeAnimationInstanceHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.replace_animation_instance(prior, unsafe { *request })
    })
}
pub(crate) unsafe extern "C" fn set_animation_playback(
    context: *mut c_void,
    request: *const NativeAnimationPlaybackRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.set_animation_playback(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn create_animation_graph(
    context: *mut c_void,
    request: *const NativeAnimationGraphCreateRequest,
    result: *mut NativeAnimationGraphHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.create_animation_graph(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn destroy_animation_graph(
    context: *mut c_void,
    value: NativeAnimationGraphHandle,
) -> i32 {
    animation_void(context, |bridge| bridge.destroy_animation_graph(value))
}
pub(crate) unsafe extern "C" fn define_animation_parameter(
    context: *mut c_void,
    request: *const NativeAnimationParameterDefinitionRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.define_animation_parameter(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn define_animation_state(
    context: *mut c_void,
    request: *const NativeAnimationStateDefinitionRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.define_animation_state(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn define_animation_transition(
    context: *mut c_void,
    request: *const NativeAnimationTransitionDefinitionRequest,
    result: *mut NativeAnimationTransitionHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.define_animation_transition(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn define_animation_condition(
    context: *mut c_void,
    request: *const NativeAnimationConditionDefinitionRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.define_animation_condition(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn create_animation_controller(
    context: *mut c_void,
    request: *const NativeAnimationControllerCreateRequest,
    result: *mut NativeAnimationControllerHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_result(context, result, |bridge| {
        bridge.create_animation_controller(unsafe { *request })
    })
}
pub(crate) unsafe extern "C" fn destroy_animation_controller(
    context: *mut c_void,
    value: NativeAnimationControllerHandle,
) -> i32 {
    animation_void(context, |bridge| bridge.destroy_animation_controller(value))
}
pub(crate) unsafe extern "C" fn set_animation_float(
    context: *mut c_void,
    request: *const NativeAnimationSetFloatRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.set_animation_float(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn set_animation_bool(
    context: *mut c_void,
    request: *const NativeAnimationSetBoolRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.set_animation_bool(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn fire_animation_trigger(
    context: *mut c_void,
    request: *const NativeAnimationFireTriggerRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| {
        bridge.fire_animation_trigger(unsafe { &*request })
    })
}
pub(crate) unsafe extern "C" fn tick_animation(
    context: *mut c_void,
    request: *const NativeAnimationTickRequest,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    animation_void(context, |bridge| bridge.tick_animation(unsafe { *request }))
}
pub(crate) unsafe extern "C" fn read_animation_controller(
    context: *mut c_void,
    value: NativeAnimationControllerHandle,
    result: *mut NativeAnimationControllerReadout,
) -> i32 {
    animation_result(context, result, |bridge| {
        bridge.read_animation_controller(value)
    })
}
pub(crate) unsafe extern "C" fn read_animation(
    context: *mut c_void,
    result: *mut NativeAnimationReadout,
) -> i32 {
    animation_result(context, result, RuntimeAppearanceBridge::read_animation)
}

pub(crate) fn animation_api(bridge: &mut RuntimeAppearanceBridge) -> NativeAnimationApi {
    NativeAnimationApi {
        context: (bridge as *mut RuntimeAppearanceBridge).cast(),
        open_animated_mesh,
        create_animated_mesh_appearance,
        replace_animated_mesh_appearance,
        destroy_appearance: destroy_animated_mesh_appearance,
        create_instance: create_animation_instance,
        destroy_instance: destroy_animation_instance,
        replace_instance: replace_animation_instance,
        set_playback: set_animation_playback,
        create_graph: create_animation_graph,
        destroy_graph: destroy_animation_graph,
        define_parameter: define_animation_parameter,
        define_state: define_animation_state,
        define_transition: define_animation_transition,
        define_condition: define_animation_condition,
        create_controller: create_animation_controller,
        destroy_controller: destroy_animation_controller,
        set_float: set_animation_float,
        set_bool: set_animation_bool,
        fire_trigger: fire_animation_trigger,
        tick: tick_animation,
        read_controller: read_animation_controller,
        read: read_animation,
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

fn material_descriptor(
    id: String,
    request: NativeMaterialRequest,
    resources: &[CsharpRenderResource],
) -> Result<RenderMaterialDescriptor, CsharpEngineServicesError> {
    let texture = if request.texture.value == 0 {
        None
    } else {
        let index = usize::try_from(request.texture.value - 1).map_err(|_| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_TEXTURE", "invalid texture handle")
        })?;
        let resource = resources.get(index).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_TEXTURE", "unknown texture handle")
        })?;
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_TEXTURE",
                "material texture must be an admitted texture resource",
            ));
        }
        Some(resource.identity().to_owned())
    };
    let descriptor = RenderMaterialDescriptor {
        schema_version: 1,
        id,
        color: native_color(request.color),
        texture,
        roughness: request.roughness,
        texture_tint: native_color(request.texture_tint),
        emission_color: native_vec3_array(request.emission_color),
        emission_intensity: request.emission_intensity,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: MaterialAlphaModeDescriptor::Opaque,
        double_sided: request.double_sided,
        voxel_surface: None,
    };
    descriptor.validate().map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_MATERIAL", format!("material is invalid: {error:?}"))
    })?;
    Ok(descriptor)
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

    fn primitive_request() -> NativePrimitiveAppearanceRequest {
        NativePrimitiveAppearanceRequest {
            geometry: NativePrimitiveGeometry::Cube,
            wireframe: false,
            color: NativeColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        }
    }

    fn appearance_fact(appearance: NativeAppearanceHandle) -> NativeAppearanceFact {
        NativeAppearanceFact {
            object_id: 7,
            transform: NativeTransform {
                translation: NativeVec3::default(),
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
            },
            appearance,
            visible: true,
            layer: NativeRenderLayer::Scene,
        }
    }

    fn point_light_request(logical_id: u64, parent_object_id: Option<u64>) -> NativeLightRequest {
        NativeLightRequest {
            logical_id,
            has_parent_object: parent_object_id.is_some(),
            parent_object_id: parent_object_id.unwrap_or_default(),
            descriptor: NativeLightDescriptor {
                kind: NativeLightKind::Point,
                color: NativeVec3 {
                    x: 0.4,
                    y: 0.5,
                    z: 0.6,
                },
                intensity: 2.0,
                enabled: true,
                position: NativeVec3 {
                    x: 2.0,
                    y: 3.0,
                    z: 4.0,
                },
                direction: NativeVec3::default(),
                has_range: true,
                range: 12.0,
                decay: 2.0,
                outer_angle_radians: 0.0,
                penumbra: 0.0,
                shadow_intent: NativeLightShadowIntent::Requested,
            },
        }
    }

    #[test]
    fn retained_appearance_must_leave_the_complete_snapshot_before_disposal_or_replacement() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let appearance = bridge.create_primitive(primitive_request()).unwrap();
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let error = bridge.destroy_appearance(appearance).unwrap_err();
        assert_eq!(error.code(), "CSHARP_APPEARANCE_IN_USE");

        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        let replacement = bridge
            .replace_primitive(NativePrimitiveAppearanceReplaceRequest {
                appearance,
                replacement: primitive_request(),
            })
            .unwrap();
        assert_ne!(replacement.value, appearance.value);
    }

    #[test]
    fn lights_are_owned_readable_and_compose_with_the_retained_appearance_frame() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let appearance = bridge.create_primitive(primitive_request()).unwrap();
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let light = bridge
            .create_light(point_light_request(91, Some(7)))
            .unwrap();
        let readout = bridge.read_light(light).unwrap();
        assert_eq!(readout.logical_id, 91);
        assert!(readout.has_parent_object);
        assert_eq!(readout.parent_object_id, 7);
        assert_eq!(readout.descriptor.kind, NativeLightKind::Point);
        let staged = bridge.take_staged_call().unwrap().unwrap();
        assert_eq!(staged.state.retained_object_count, 1);
        assert_eq!(staged.state.retained_light_count, 1);
        assert!(matches!(
            staged.frame.as_ref().unwrap().ops.as_slice(),
            [
                render_model::RenderDiff::Create { .. },
                render_model::RenderDiff::CreateLight { .. }
            ]
        ));
        bridge.commit(Some(staged));

        bridge.begin_call();
        let mut replacement = point_light_request(91, Some(7));
        replacement.descriptor.intensity = 3.0;
        bridge
            .update_light(NativeLightUpdateRequest { light, replacement })
            .unwrap();
        let staged = bridge.take_staged_call().unwrap().unwrap();
        assert!(matches!(
            staged.frame.as_ref().unwrap().ops.as_slice(),
            [render_model::RenderDiff::UpdateLight { .. }]
        ));
        bridge.commit(Some(staged));
    }

    #[test]
    fn invalid_light_replacement_preserves_the_committed_owner_and_requested_facts() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let light = bridge.create_light(point_light_request(91, None)).unwrap();
        let staged = bridge.take_staged_call().unwrap();
        bridge.commit(staged);

        bridge.begin_call();
        let mut invalid = point_light_request(92, None);
        invalid.descriptor.kind = NativeLightKind::Directional;
        invalid.descriptor.direction = NativeVec3::default();
        let error = bridge
            .replace_light(NativeLightUpdateRequest {
                light,
                replacement: invalid,
            })
            .unwrap_err();
        assert_eq!(error.code(), "CSHARP_LIGHT_DESCRIPTOR");
        bridge.discard_call();

        bridge.begin_call();
        let retained = bridge.read_light(light).unwrap();
        assert_eq!(retained.logical_id, 91);
        assert_eq!(retained.descriptor.kind, NativeLightKind::Point);
        bridge.destroy_light(light).unwrap();
        let staged = bridge.take_staged_call().unwrap().unwrap();
        assert!(matches!(
            staged.frame.as_ref().unwrap().ops.as_slice(),
            [render_model::RenderDiff::Destroy { .. }]
        ));
        bridge.commit(Some(staged));

        bridge.begin_call();
        bridge.destroy_light(light).unwrap();
        assert!(bridge.take_staged_call().unwrap().unwrap().frame.is_none());
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

    #[test]
    fn animated_direct_playback_is_emitted_once_per_command() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut content_resources = BTreeMap::new();
        content_resources.insert("character.glb".to_owned(), Arc::from(CHARACTER_GLB));
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let resource_path = b"character.glb";

        bridge.begin_call();
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: resource_path.as_ptr(),
                    len: resource_path.len(),
                },
            })
            .expect("admitted animated GLB");
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .expect("animated appearance");
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 7,
            })
            .expect("retained animation instance");
        bridge
            .set_animation_playback(&NativeAnimationPlaybackRequest {
                instance,
                kind: NativeAnimationPlaybackKind::Stop,
                clip: NativeUtf8Slice {
                    bytes: std::ptr::null(),
                    len: 0,
                },
                loop_mode: NativeAnimationLoopMode::Once,
                speed: 0.0,
                weight: 0.0,
                restart: false,
                fade_seconds: 0.0,
                has_fade: false,
                normalized_time: 0.0,
            })
            .expect("one-shot stop command");
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("appearance snapshot");
        assert_eq!(
            bridge
                .staged
                .as_ref()
                .expect("staged frame")
                .extra_frames
                .len(),
            1
        );
        let first_call = bridge.take_staged_call().expect("first animation call");
        bridge.commit(first_call);

        bridge.begin_call();
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("unchanged appearance snapshot");
        assert!(bridge
            .staged
            .as_ref()
            .expect("second staged frame")
            .extra_frames
            .is_empty());
        assert_eq!(
            bridge
                .destroy_animation_instance(instance)
                .expect_err("snapshot and disposal must be ordered across product calls")
                .code(),
            "CSHARP_ANIMATION_SNAPSHOT_ORDER"
        );
        bridge.discard_call();

        bridge.begin_call();
        bridge
            .destroy_animation_instance(instance)
            .expect("direct instance teardown");
        assert_eq!(
            bridge
                .staged
                .as_ref()
                .expect("teardown call")
                .extra_frames
                .len(),
            1
        );
    }

    #[test]
    fn animated_controller_disposal_emits_owner_destroy_before_target_removal() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut content_resources = BTreeMap::new();
        content_resources.insert("character.glb".to_owned(), Arc::from(CHARACTER_GLB));
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let path = b"character.glb";
        let graph_id = b"controller";
        let idle = b"idle";

        bridge.begin_call();
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("admitted animated GLB");
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .expect("animated appearance");
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 7,
            })
            .expect("retained animation instance");
        let graph = bridge
            .create_animation_graph(&NativeAnimationGraphCreateRequest {
                resource,
                graph_id: NativeUtf8Slice {
                    bytes: graph_id.as_ptr(),
                    len: graph_id.len(),
                },
                version: 1,
                initial_state_id: NativeUtf8Slice {
                    bytes: idle.as_ptr(),
                    len: idle.len(),
                },
            })
            .expect("animation graph");
        bridge
            .define_animation_state(&NativeAnimationStateDefinitionRequest {
                graph,
                state_id: NativeUtf8Slice {
                    bytes: idle.as_ptr(),
                    len: idle.len(),
                },
                motion_kind: NativeAnimationMotionKind::Clip,
                clip_a: NativeUtf8Slice {
                    bytes: idle.as_ptr(),
                    len: idle.len(),
                },
                clip_b: NativeUtf8Slice {
                    bytes: std::ptr::null(),
                    len: 0,
                },
                parameter_id: NativeUtf8Slice {
                    bytes: std::ptr::null(),
                    len: 0,
                },
                minimum_milli: 0,
                maximum_milli: 0,
                speed_milli: 1000,
            })
            .expect("idle graph state");
        let controller = bridge
            .create_animation_controller(NativeAnimationControllerCreateRequest {
                graph,
                instance,
                tick_duration_millis: 16,
            })
            .expect("animation controller");
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("controller target snapshot");
        assert_eq!(
            bridge
                .staged
                .as_ref()
                .expect("controller setup")
                .presentation
                .len(),
            1
        );
        let setup = bridge.take_staged_call().expect("controller setup call");
        bridge.commit(setup);

        bridge.begin_call();
        bridge
            .destroy_animation_controller(controller)
            .expect("controller teardown");
        assert_eq!(
            bridge
                .staged
                .as_ref()
                .expect("controller teardown call")
                .presentation
                .len(),
            1
        );
        assert_eq!(
            unsafe { bridge.stage_snapshot(&fact, 1) }
                .expect_err("controller teardown and target snapshot must be ordered")
                .code(),
            "CSHARP_ANIMATION_SNAPSHOT_ORDER"
        );
    }

    #[test]
    fn presentation_facts_stage_projected_billboard_and_particle_frames() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let key = b"status";
        let text = b"Ready";
        let font = b"sans-serif";
        let empty = NativeUtf8Slice {
            bytes: std::ptr::null(),
            len: 0,
        };
        let slice = |value: &[u8]| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        let anchor = NativePresentationAnchor {
            kind: NativePresentationAnchorKind::World,
            position: NativeVec3::default(),
            entity: 0,
            offset: NativeVec3::default(),
        };
        let color = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        bridge.begin_call();
        let billboard = bridge
            .presentation_create_billboard(&NativePresentationBillboardDescriptor {
                logical_id: 7,
                anchor,
                content_kind: NativeBillboardContentKind::Text,
                localization_key: slice(key),
                fallback_text: slice(text),
                value: empty,
                unit_key: empty,
                fallback_unit: empty,
                texture: NativeRenderResourceHandle::default(),
                font_kind: NativePresentationFontKind::System,
                font_asset: NativeRenderResourceHandle::default(),
                font_family: slice(font),
                height_pixels: 16.0,
                color,
                background: NativeColor::default(),
                max_distance: 100.0,
                layer: NativePresentationBillboardLayer::AlwaysOnTop,
                visible: true,
            })
            .expect("text billboard");
        let size_curve = [
            NativePresentationParticleScalarKey {
                age: 0.0,
                value: 1.0,
            },
            NativePresentationParticleScalarKey {
                age: 1.0,
                value: 0.0,
            },
        ];
        let color_curve = [
            NativePresentationParticleColorKey { age: 0.0, color },
            NativePresentationParticleColorKey { age: 1.0, color },
        ];
        let emitter = bridge
            .presentation_create_emitter(&NativePresentationParticleDescriptor {
                logical_id: 8,
                signal_id: empty,
                anchor,
                visual: NativePresentationParticleVisual::Cube,
                sprite: NativeRenderResourceHandle::default(),
                sprite_frame_count: 0,
                rate_per_second: 1.0,
                burst_count: 1,
                lifetime_min_seconds: 0.1,
                lifetime_max_seconds: 1.0,
                velocity_min: NativeVec3::default(),
                velocity_max: NativeVec3::default(),
                acceleration: NativeVec3::default(),
                size_curve: size_curve.as_ptr(),
                size_curve_len: size_curve.len(),
                color_curve: color_curve.as_ptr(),
                color_curve_len: color_curve.len(),
                flipbook_frames_per_second: 0.0,
                seed: 3,
                max_particles: 4,
                visible: true,
            })
            .expect("cube emitter");
        bridge
            .presentation_update_billboard(
                billboard,
                &NativePresentationBillboardDescriptor {
                    logical_id: 7,
                    anchor,
                    content_kind: NativeBillboardContentKind::Text,
                    localization_key: slice(key),
                    fallback_text: slice(text),
                    value: empty,
                    unit_key: empty,
                    fallback_unit: empty,
                    texture: NativeRenderResourceHandle::default(),
                    font_kind: NativePresentationFontKind::System,
                    font_asset: NativeRenderResourceHandle::default(),
                    font_family: slice(font),
                    height_pixels: 18.0,
                    color,
                    background: NativeColor::default(),
                    max_distance: 100.0,
                    layer: NativePresentationBillboardLayer::AlwaysOnTop,
                    visible: true,
                },
            )
            .expect("full billboard update");
        bridge
            .presentation_emit_particles(
                slice(b"burst-1"),
                &NativePresentationParticleDescriptor {
                    logical_id: 9,
                    signal_id: slice(b"burst-1"),
                    anchor,
                    visual: NativePresentationParticleVisual::Cube,
                    sprite: NativeRenderResourceHandle::default(),
                    sprite_frame_count: 0,
                    rate_per_second: 0.0,
                    burst_count: 1,
                    lifetime_min_seconds: 0.1,
                    lifetime_max_seconds: 1.0,
                    velocity_min: NativeVec3::default(),
                    velocity_max: NativeVec3::default(),
                    acceleration: NativeVec3::default(),
                    size_curve: size_curve.as_ptr(),
                    size_curve_len: size_curve.len(),
                    color_curve: color_curve.as_ptr(),
                    color_curve_len: color_curve.len(),
                    flipbook_frames_per_second: 0.0,
                    seed: 4,
                    max_particles: 4,
                    visible: true,
                },
            )
            .expect("direct burst");
        assert_eq!(bridge.presentation_readout().active_billboards, 1);
        assert_eq!(bridge.presentation_readout().active_emitters, 1);
        assert_eq!(bridge.presentation_readout().emitted_bursts, 1);
        let call = bridge
            .take_staged_call()
            .expect("staged presentation call")
            .expect("appearance call");
        assert_eq!(call.presentation.len(), 4);
        assert!(call
            .presentation
            .iter()
            .all(|frame| frame.validate().is_ok()));
        let _ = emitter;
    }

    #[test]
    fn rejected_presentation_fact_keeps_bounded_diagnostic_after_call_discard() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let key = b"status";
        let text = b"Ready";
        let font = b"sans-serif";
        let empty = NativeUtf8Slice {
            bytes: std::ptr::null(),
            len: 0,
        };
        let slice = |value: &[u8]| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        let descriptor = NativePresentationBillboardDescriptor {
            logical_id: 7,
            anchor: NativePresentationAnchor {
                kind: NativePresentationAnchorKind::World,
                position: NativeVec3::default(),
                entity: 0,
                offset: NativeVec3::default(),
            },
            content_kind: NativeBillboardContentKind::Text,
            localization_key: slice(key),
            fallback_text: slice(text),
            value: empty,
            unit_key: empty,
            fallback_unit: empty,
            texture: NativeRenderResourceHandle::default(),
            font_kind: NativePresentationFontKind::System,
            font_asset: NativeRenderResourceHandle::default(),
            font_family: slice(font),
            height_pixels: 16.0,
            color: NativeColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            background: NativeColor::default(),
            max_distance: 100.0,
            layer: NativePresentationBillboardLayer::AlwaysOnTop,
            visible: true,
        };
        bridge.begin_call();
        bridge
            .presentation_create_billboard(&descriptor)
            .expect("initial billboard");
        let initial_call = bridge.take_staged_call().expect("initial call");
        bridge.commit(initial_call);
        bridge.begin_call();
        let error = bridge
            .presentation_create_billboard(&descriptor)
            .expect_err("duplicate billboard");
        bridge.record_callback_error(error);
        assert_eq!(bridge.presentation_readout().billboard_diagnostic_count, 1);
        assert_eq!(
            bridge
                .presentation_diagnostic(NativePresentationDiagnosticAtRequest {
                    domain: NativePresentationDiagnosticDomain::Billboard,
                    index: 0
                })
                .logical_id,
            7
        );
        assert!(bridge.take_staged_call().is_err());
        assert_eq!(bridge.presentation_readout().billboard_diagnostic_count, 1);
    }

    #[test]
    fn admitted_woff2_font_is_resolved_by_asset_billboard_without_raw_asset_strings() {
        let mut content_resources = BTreeMap::new();
        content_resources.insert("ui.woff2".to_owned(), Arc::from(&b"wOF2font-body"[..]));
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let key = b"status";
        let text = b"Ready";
        let family = b"Ui Font";
        let empty = NativeUtf8Slice {
            bytes: std::ptr::null(),
            len: 0,
        };
        let slice = |value: &[u8]| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        bridge.begin_call();
        let font = bridge
            .open_resource(&resource_request("ui.woff2"))
            .expect("admitted font");
        assert_eq!(font.kind, NativeRenderResourceKind::Font);
        bridge
            .presentation_create_billboard(&NativePresentationBillboardDescriptor {
                logical_id: 11,
                anchor: NativePresentationAnchor {
                    kind: NativePresentationAnchorKind::World,
                    position: NativeVec3::default(),
                    entity: 0,
                    offset: NativeVec3::default(),
                },
                content_kind: NativeBillboardContentKind::Text,
                localization_key: slice(key),
                fallback_text: slice(text),
                value: empty,
                unit_key: empty,
                fallback_unit: empty,
                texture: NativeRenderResourceHandle::default(),
                font_kind: NativePresentationFontKind::Asset,
                font_asset: font.handle,
                font_family: slice(family),
                height_pixels: 16.0,
                color: NativeColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                background: NativeColor::default(),
                max_distance: 100.0,
                layer: NativePresentationBillboardLayer::AlwaysOnTop,
                visible: true,
            })
            .expect("asset-font billboard");
        let call = bridge
            .take_staged_call()
            .expect("font call")
            .expect("appearance call");
        assert!(matches!(
            &call.presentation[0].ops[0],
            render_presentation::PresentationOp::Billboard { op: BillboardProjectionOp::Create { descriptor: BillboardDescriptor { font: BillboardFontRef::Asset { family: resolved_family, .. }, .. }, .. }, .. }
                if resolved_family == "Ui Font"
        ));
    }
}
