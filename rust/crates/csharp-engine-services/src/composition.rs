//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.
//!
//! This crate owns the callback contexts and their staged Engine state. The
//! runtime crate only composes this service family with a loaded product.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use core_math::Vec3;
use csharp_engine_abi::*;
use entity_state::Quat;
use product_dev_host::{CanonicalU64, ProductDevUpdateAttribution};

use crate::{
    audio::{AudioRealizationFact, RuntimeAudioBridge, RuntimeAudioCall},
    authored_content::RuntimeAuthoredContentBridge,
    camera_view::RuntimeCameraViewBridge,
    content::RuntimeContentBridge,
    dynamics::RuntimeDynamicsBridge,
    persistence::RuntimePersistenceBridge,
    rng::RuntimeRngBridge,
    spatial::RuntimeSpatialBridge,
    ui::{RuntimeUiBridge, RuntimeUiCall},
    voxel_content::RuntimeVoxelContentBridge,
    voxel_scene_presentation::RuntimeVoxelScenePresentationBridge,
};
use render_projection::RuntimeAppearanceCatalog;
use runtime_ui::{RuntimeUiProjectionEnvelope, RuntimeUiRuntimeBinding};

pub(crate) const ABI_OK: i32 = 1;
use crate::appearance::{
    advance_sprite_playback, control_sprite_playback, create_light, create_material,
    create_primitive_appearance, create_sprite_appearance, create_sprite_atlas,
    create_sprite_from_atlas, create_sprite_playback, create_static_mesh_appearance,
    create_static_mesh_from_content_appearance, destroy_appearance, destroy_light,
    destroy_material, destroy_sprite_atlas, destroy_sprite_playback,
    destroy_sprite_playback_advance_lease, open_render_resource, publish_appearance_snapshot,
    read_light, read_presentation, read_sprite, read_sprite_playback, replace_light,
    replace_material, replace_primitive_appearance, replace_sprite_appearance,
    replace_sprite_from_atlas, replace_static_mesh_appearance,
    replace_static_mesh_from_content_appearance, sample_sprite_playback,
    select_sprite_playback_frame, set_sprite_frame, update_light, update_material,
    update_static_mesh_materials, AnimationCueDefinition, CsharpRenderResource,
    RuntimeAppearanceBridge, RuntimeAppearanceCall,
};

fn engine_api(
    diagnostics_bridge: &mut crate::diagnostics::RuntimeDiagnosticsBridge,
    appearance_bridge: &mut RuntimeAppearanceBridge,
    content_bridge: &mut RuntimeContentBridge,
    authored_content_bridge: &mut RuntimeAuthoredContentBridge,
    content_store_bridge: &mut crate::content_store::RuntimeContentStoreBridge,
    audio_bridge: &mut RuntimeAudioBridge,
    camera_view_bridge: &mut RuntimeCameraViewBridge,
    dynamics_bridge: &mut RuntimeDynamicsBridge,
    spatial_bridge: &mut RuntimeSpatialBridge,
    perception_bridge: &mut crate::perception::RuntimePerceptionBridge,
    voxel_content_bridge: &mut RuntimeVoxelContentBridge,
    voxel_scene_presentation_bridge: &mut RuntimeVoxelScenePresentationBridge,
    rng_bridge: &mut RuntimeRngBridge,
    persistence_bridge: &mut RuntimePersistenceBridge,
    ui_bridge: &mut RuntimeUiBridge,
) -> NativeEngineApi {
    appearance_bridge.bind_authored_content(authored_content_bridge);
    NativeEngineApi {
        diagnostics: crate::diagnostics::api(diagnostics_bridge),
        look: crate::look::api(),
        dynamics: crate::dynamics::api(dynamics_bridge),
        motion: crate::motion::api(),
        kinematic: crate::kinematic::api(spatial_bridge),
        spatial: crate::spatial::api(spatial_bridge),
        perception: crate::perception::api(perception_bridge),
        world_origin: crate::world_origin::api(spatial_bridge),
        voxel: crate::voxel::api(spatial_bridge),
        voxel_content: crate::voxel_content::api_with_spatial(
            voxel_content_bridge,
            appearance_bridge,
            spatial_bridge,
        ),
        voxel_scene_presentation: crate::voxel_scene_presentation::api(
            voxel_scene_presentation_bridge,
            appearance_bridge,
        ),
        content: crate::content::api(content_bridge),
        authored_content: crate::authored_content::api(authored_content_bridge),
        content_store: crate::content_store::api(content_store_bridge),
        appearance: NativeAppearanceApi {
            context: (appearance_bridge as *mut RuntimeAppearanceBridge).cast(),
            open_resource: open_render_resource,
            create_material,
            update_material,
            replace_material,
            destroy_material,
            create_primitive: create_primitive_appearance,
            replace_primitive: replace_primitive_appearance,
            create_static_mesh: create_static_mesh_appearance,
            create_static_mesh_from_content: create_static_mesh_from_content_appearance,
            replace_static_mesh: replace_static_mesh_appearance,
            replace_static_mesh_from_content: replace_static_mesh_from_content_appearance,
            update_static_mesh_materials,
            create_sprite: create_sprite_appearance,
            replace_sprite: replace_sprite_appearance,
            create_sprite_atlas,
            destroy_sprite_atlas,
            create_sprite_from_atlas,
            replace_sprite_from_atlas,
            set_sprite_frame,
            read_sprite,
            create_sprite_playback,
            destroy_sprite_playback,
            control_sprite_playback,
            select_sprite_playback_frame,
            advance_sprite_playback,
            destroy_sprite_playback_advance_lease,
            sample_sprite_playback,
            read_sprite_playback,
            destroy_appearance,
            publish_snapshot: publish_appearance_snapshot,
            create_light,
            update_light,
            replace_light,
            destroy_light,
            read_light,
            read_presentation,
            create_authored_material: crate::appearance::create_authored_material,
        },
        presentation: NativePresentationApi {
            context: (appearance_bridge as *mut RuntimeAppearanceBridge).cast(),
            create_billboard: crate::presentation::create_billboard,
            update_billboard: crate::presentation::update_billboard,
            create_structured_billboard: crate::presentation::create_structured_billboard,
            update_structured_billboard: crate::presentation::update_structured_billboard,
            destroy_billboard: crate::presentation::destroy_billboard,
            emit_particles: crate::presentation::emit_particles,
            create_emitter: crate::presentation::create_emitter,
            update_emitter: crate::presentation::update_emitter,
            destroy_emitter: crate::presentation::destroy_emitter,
            read: crate::presentation::read,
            read_diagnostic_at: crate::presentation::read_diagnostic_at,
            create_ghost_plate: crate::presentation::create_ghost_plate,
            update_ghost_plate: crate::presentation::update_ghost_plate,
            recapture_ghost_plate: crate::presentation::recapture_ghost_plate,
            read_ghost_plate: crate::presentation::read_ghost_plate,
            destroy_ghost_plate: crate::presentation::destroy_ghost_plate,
        },
        animation: crate::appearance::animation_api(appearance_bridge),
        audio: crate::audio::api(audio_bridge),
        camera_view: NativeCameraViewApi {
            context: (camera_view_bridge as *mut RuntimeCameraViewBridge).cast(),
            create_camera: crate::camera_view::create_camera,
            update_camera: crate::camera_view::update_camera,
            replace_camera: crate::camera_view::replace_camera,
            destroy_camera: crate::camera_view::destroy_camera,
            set_active_camera: crate::camera_view::set_active_camera,
            clear_active_camera: crate::camera_view::clear_active_camera,
            set_sky_background: crate::camera_view::set_sky_background,
            clear_sky_background: crate::camera_view::clear_sky_background,
        },
        rng: crate::rng::api(rng_bridge),
        persistence: crate::persistence::api(persistence_bridge),
        ui: crate::ui::api(ui_bridge),
    }
}

pub(crate) fn native_vec3(value: Vec3) -> NativeVec3 {
    NativeVec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

pub(crate) fn native_quat(value: Quat) -> NativeQuat {
    NativeQuat {
        x: value.x,
        y: value.y,
        z: value.z,
        w: value.w,
    }
}

pub(crate) fn native_quat_value(value: NativeQuat) -> Quat {
    Quat::new(value.x, value.y, value.z, value.w)
}

pub(crate) fn native_vec3_value(value: NativeVec3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

pub(crate) unsafe fn borrowed_slice<'a, T>(
    pointer: *const T,
    len: usize,
    field: &'static str,
) -> Result<&'a [T], CsharpEngineServicesError> {
    if len > 0 && pointer.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_SPATIAL_POINTER",
            format!("C# {field} had length without a pointer"),
        ));
    }
    if len == 0 {
        Ok(&[])
    } else {
        // SAFETY: direct-call borrowing retains this span until callback return.
        Ok(unsafe { std::slice::from_raw_parts(pointer, len) })
    }
}

pub(crate) unsafe fn borrowed_utf8<'a>(
    pointer: *const u8,
    len: usize,
    field: &'static str,
) -> Result<&'a str, CsharpEngineServicesError> {
    if len > 0 && pointer.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_UTF8_POINTER",
            format!("C# {field} had length without bytes"),
        ));
    }
    let bytes = if len == 0 {
        &[]
    } else {
        // SAFETY: a non-empty borrowed range was checked above and is only used during this callback.
        unsafe { std::slice::from_raw_parts(pointer, len) }
    };
    std::str::from_utf8(bytes).map_err(|_| {
        CsharpEngineServicesError::new("CSHARP_UTF8", format!("C# {field} was not UTF-8"))
    })
}

/// The concrete callback family retained while a trusted C# product is live.
///
/// The runtime drives call boundaries; this owner stages and commits only
/// Engine-facing effects created through the generated function tables.
pub struct EngineServiceSet {
    diagnostics: crate::diagnostics::RuntimeDiagnosticsBridge,
    appearance: RuntimeAppearanceBridge,
    content: Box<RuntimeContentBridge>,
    authored_content: RuntimeAuthoredContentBridge,
    content_store: Box<crate::content_store::RuntimeContentStoreBridge>,
    audio: RuntimeAudioBridge,
    camera_view: RuntimeCameraViewBridge,
    dynamics: RuntimeDynamicsBridge,
    spatial: RuntimeSpatialBridge,
    perception: crate::perception::RuntimePerceptionBridge,
    voxel_content: RuntimeVoxelContentBridge,
    voxel_scene_presentation: RuntimeVoxelScenePresentationBridge,
    rng: RuntimeRngBridge,
    persistence: RuntimePersistenceBridge,
    ui: RuntimeUiBridge,
}

pub struct CsharpEngineCall {
    appearance: Option<RuntimeAppearanceCall>,
    audio: RuntimeAudioCall,
    camera_view: crate::camera_view::RuntimeCameraViewCall,
    sky_frame: Option<render_model::RenderFrameDiff>,
    ui: RuntimeUiCall,
    voxel_content: crate::voxel_content::RuntimeVoxelContentCall,
    voxel_scene_presentation: crate::voxel_scene_presentation::RuntimeVoxelScenePresentationCall,
}

/// Staged Engine observations from one successful product call.
pub struct CsharpEngineCallOutput {
    pub appearance: Vec<CsharpAppearanceCallOutput>,
    pub frames: Vec<render_model::RenderFrameDiff>,
    pub view_composition: Option<render_host_contracts::RendererViewComposition>,
    pub ui: Vec<RuntimeUiProjectionEnvelope>,
    pub presentation: Vec<render_presentation::PresentationFrameDiff>,
}

/// Ordered renderer realization work emitted by the existing Appearance API
/// during one product callback.
pub enum CsharpAppearanceCallOutput {
    Frame(render_model::RenderFrameDiff),
    Presentation(render_presentation::PresentationFrameDiff),
    AnimationCueDefinitions(Vec<AnimationCueDefinition>),
}

/// Parsed optional appearance catalog retained with admitted product content.
/// The catalog remains an Engine presentation detail rather than a runtime-host
/// dependency.
pub struct CsharpAppearanceCatalog(RuntimeAppearanceCatalog);

impl EngineServiceSet {
    pub fn new(
        catalog: CsharpAppearanceCatalog,
        content_resources: BTreeMap<String, Arc<[u8]>>,
        persistence_root: Option<PathBuf>,
        content_store_root: Option<PathBuf>,
        diagnostics_sink: product_dev_host::ProductDevLog,
    ) -> Result<Self, CsharpEngineServicesError> {
        let mut spatial = crate::spatial::RuntimeSpatialBridge::new();
        let perception = crate::perception::RuntimePerceptionBridge::new(&spatial);
        let dynamics = crate::dynamics::RuntimeDynamicsBridge::new(spatial.collision_source());
        let content = Box::new(RuntimeContentBridge::new(content_resources.clone()));
        spatial.bind_content(&content);
        let mut authored_content = RuntimeAuthoredContentBridge::new();
        authored_content.bind_content(&content);
        let mut content_store = Box::new(crate::content_store::RuntimeContentStoreBridge::new(
            content_store_root,
        )?);
        authored_content.bind_content_store(&mut content_store);
        let voxel_scene_presentation =
            RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance = crate::appearance::create(catalog.0, content_resources.clone());
        appearance.bind_diagnostics_sink(diagnostics_sink.clone());
        let mut audio = RuntimeAudioBridge::new(content_resources);
        audio.bind_diagnostics_sink(diagnostics_sink.clone());
        Ok(Self {
            diagnostics: crate::diagnostics::RuntimeDiagnosticsBridge::new(diagnostics_sink),
            appearance,
            content,
            authored_content,
            content_store,
            audio,
            camera_view: RuntimeCameraViewBridge::new(),
            dynamics,
            spatial,
            perception,
            voxel_content: RuntimeVoxelContentBridge::new(),
            voxel_scene_presentation,
            rng: crate::rng::RuntimeRngBridge::new(),
            persistence: crate::persistence::RuntimePersistenceBridge::new(persistence_root),
            ui: crate::ui::RuntimeUiBridge::new(),
        })
    }

    pub fn api(&mut self) -> NativeEngineApi {
        engine_api(
            &mut self.diagnostics,
            &mut self.appearance,
            &mut self.content,
            &mut self.authored_content,
            &mut self.content_store,
            &mut self.audio,
            &mut self.camera_view,
            &mut self.dynamics,
            &mut self.spatial,
            &mut self.perception,
            &mut self.voxel_content,
            &mut self.voxel_scene_presentation,
            &mut self.rng,
            &mut self.persistence,
            &mut self.ui,
        )
    }

    pub fn ingest_renderer_diagnostics(
        &mut self,
        snapshot: &serde_json::Value,
    ) -> Result<(), CsharpEngineServicesError> {
        self.diagnostics.ingest_renderer(snapshot).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDERER_DIAGNOSTICS_ENCODE",
                "renderer diagnostics snapshot could not be encoded",
            )
        })
    }

    pub fn renderer_diagnostics_json(&self) -> Option<&str> {
        self.diagnostics.renderer_json()
    }

    pub fn begin_call(&mut self, ui_binding: RuntimeUiRuntimeBinding) {
        self.appearance.begin_call();
        self.begin_other_services(ui_binding);
    }

    /// Begins a detached browser-attachment projection. Renderer-facing
    /// projectors rebase for a fresh consumer, while commit is deliberately
    /// left to the caller so active runtime service state is not replaced.
    pub fn begin_attach_call(
        &mut self,
        ui_binding: RuntimeUiRuntimeBinding,
    ) -> Result<(), CsharpEngineServicesError> {
        self.appearance.begin_attach_call();
        self.audio.begin_call();
        self.camera_view.begin_attach_call()?;
        self.dynamics.begin_call();
        self.ui.begin_call(ui_binding);
        self.voxel_content.begin_attach_call();
        self.voxel_scene_presentation.begin_attach_call();
        Ok(())
    }

    pub fn begin_update_call(
        &mut self,
        ui_binding: RuntimeUiRuntimeBinding,
        facts: NativeProductUpdateFacts,
    ) {
        self.spatial.reset_update_attribution();
        self.voxel_scene_presentation.reset_update_attribution();
        self.appearance.begin_update_call(facts);
        self.begin_other_services(ui_binding);
    }

    /// Returns one completed update sample after the C# callback has returned.
    /// Service durations are nested within `callback_duration_us`, not additive
    /// frame stages.
    pub fn complete_update_attribution(
        &self,
        callback_duration_us: u64,
    ) -> ProductDevUpdateAttribution {
        let spatial = self.spatial.update_attribution();
        let presentation = self.voxel_scene_presentation.update_attribution();
        ProductDevUpdateAttribution {
            callback_duration_us: CanonicalU64::new(callback_duration_us),
            character_step_calls: spatial.character_step_calls,
            character_step_duration_us: spatial.character_step_duration_us,
            character_step_cast_count: spatial.character_step_cast_count,
            character_step_candidate_count: spatial.character_step_candidate_count,
            character_step_narrow_phase_count: spatial.character_step_narrow_phase_count,
            voxel_residency_calls: spatial.voxel_residency_calls,
            voxel_residency_duration_us: spatial.voxel_residency_duration_us,
            voxel_scene_presentation_calls: presentation.voxel_scene_presentation_calls,
            voxel_scene_presentation_duration_us: presentation.voxel_scene_presentation_duration_us,
        }
    }

    fn begin_other_services(&mut self, ui_binding: RuntimeUiRuntimeBinding) {
        self.audio.begin_call();
        self.camera_view.begin_call();
        self.dynamics.begin_call();
        self.ui.begin_call(ui_binding);
        self.voxel_content.begin_call();
        self.voxel_scene_presentation.begin_call();
    }

    /// Copies browser-host realization facts while C# is not executing. The
    /// next normal product call snapshots this store for generated reads.
    pub fn ingest_audio_realization_feedback(
        &mut self,
        replace_owner: bool,
        evicted_fact_count: u64,
        facts: impl IntoIterator<Item = AudioRealizationFact>,
    ) -> Result<(), CsharpEngineServicesError> {
        self.audio
            .ingest_realized_feedback(replace_owner, evicted_fact_count, facts)
    }

    pub fn ingest_animation_realization_feedback(
        &mut self,
        replace_owner: bool,
        evicted_fact_count: u64,
        facts: impl IntoIterator<Item = crate::appearance::AnimationRealizationFact>,
    ) {
        self.appearance.ingest_animation_realization_feedback(
            replace_owner,
            evicted_fact_count,
            facts,
        );
    }

    /// Replaces the bound browser host's latest ghost-plate realization
    /// snapshot. Generated C# reads it during the next ordinary product call.
    pub fn ingest_ghost_plate_realization_feedback(
        &mut self,
        replace_owner: bool,
        facts: impl IntoIterator<Item = crate::appearance::GhostPlateRealizationFact>,
    ) {
        self.appearance
            .ingest_ghost_plate_realization(replace_owner, facts);
    }

    /// Clears realization observations when the exact runtime binding changes.
    pub fn reset_audio_realization_owner(&mut self) {
        self.audio.reset_realized_feedback();
    }

    pub fn reset_animation_realization_owner(&mut self) {
        self.appearance
            .ingest_animation_realization_feedback(true, 0, []);
    }

    pub fn reset_ghost_plate_realization_owner(&mut self) {
        self.appearance.ingest_ghost_plate_realization(true, []);
    }

    pub fn discard_call(&mut self) {
        self.appearance.discard_call();
        self.audio.discard_call();
        self.camera_view.discard_call();
        self.dynamics.discard_call();
        self.ui.discard_call();
        self.voxel_content.discard_call();
        self.voxel_scene_presentation.discard_call();
    }

    pub fn take_call(&mut self) -> Result<CsharpEngineCall, CsharpEngineServicesError> {
        let appearance = self.appearance.take_staged_call()?;
        let audio = self.audio.take_staged_call()?;
        let camera_view = self.camera_view.take_staged_call()?;
        self.dynamics.take_staged_call()?;
        // Sky resources are owned and admitted by Appearance. Resolve the
        // cross-family handle while both staged states are available, before
        // either state can be committed or turned into host output.
        let sky_frame =
            crate::camera_view::sky_frame(camera_view.sky_texture, appearance.as_ref())?;
        let ui = self.ui.take_staged_call()?;
        let voxel_content = self.voxel_content.take_staged_call()?;
        let voxel_scene_presentation = self.voxel_scene_presentation.take_staged_call()?;
        Ok(CsharpEngineCall {
            appearance,
            audio,
            camera_view,
            sky_frame,
            ui,
            voxel_content,
            voxel_scene_presentation,
        })
    }

    pub fn commit_call(&mut self, call: CsharpEngineCall) {
        self.appearance.commit(call.appearance);
        self.audio.commit(call.audio);
        self.camera_view.commit(call.camera_view);
        self.dynamics.commit_call();
        self.ui.commit(call.ui);
        self.voxel_content.commit_call(call.voxel_content);
        self.voxel_scene_presentation
            .commit_call(call.voxel_scene_presentation);
    }

    pub fn seal_resource_selection(&mut self) {
        self.appearance.seal_resource_selection();
        self.audio.seal_resource_selection();
    }

    pub fn render_resources(&self) -> Vec<CsharpRenderResource> {
        self.appearance
            .state
            .render_resources
            .iter()
            .cloned()
            .chain(self.audio.render_resources().cloned())
            .collect()
    }

    pub fn outputs(&self, call: &CsharpEngineCall) -> CsharpEngineCallOutput {
        let appearance = call
            .appearance
            .as_ref()
            .map(|call| {
                call.outputs
                    .iter()
                    .cloned()
                    .map(|output| match output {
                        crate::appearance::RuntimeAppearanceCallOutput::Frame(frame) => {
                            CsharpAppearanceCallOutput::Frame(frame)
                        }
                        crate::appearance::RuntimeAppearanceCallOutput::Presentation(frame) => {
                            CsharpAppearanceCallOutput::Presentation(frame)
                        }
                        crate::appearance::RuntimeAppearanceCallOutput::AnimationCueDefinitions(
                            definitions,
                        ) => CsharpAppearanceCallOutput::AnimationCueDefinitions(definitions),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut frames = Vec::new();
        if let Some(frame) = call.sky_frame.clone() {
            frames.push(frame);
        }
        frames.extend(call.voxel_content.frames.clone());
        frames.extend(call.voxel_scene_presentation.frames.clone());
        CsharpEngineCallOutput {
            appearance,
            frames,
            view_composition: call.camera_view.composition.clone(),
            ui: call.ui.projections.clone(),
            presentation: call.audio.frame.clone().into_iter().collect(),
        }
    }

    /// Extracts only the renderer work needed to reconcile retained voxel
    /// presentation after a failed callback. This deliberately has no UI,
    /// lifecycle, or complete-baseline marker: the product incarnation remains
    /// tainted and normal interaction still follows the existing replacement
    /// policy.
    pub fn recover_voxel_presentation_outputs(
        &self,
    ) -> Result<CsharpEngineCallOutput, CsharpEngineServicesError> {
        Ok(CsharpEngineCallOutput {
            appearance: Vec::new(),
            frames: self.voxel_scene_presentation.recover_from_canonical()?,
            view_composition: None,
            ui: Vec::new(),
            presentation: Vec::new(),
        })
    }
}

impl CsharpEngineCall {
    /// Retags only staged UI envelopes after their owning lifecycle action has
    /// actually succeeded. Other Engine service state stays staged unchanged.
    pub fn rebind_ui_runtime(&mut self, binding: RuntimeUiRuntimeBinding) {
        self.ui.rebind_runtime(binding);
    }
}

/// Admits the optional retained-appearance catalog carried by product content.
pub fn parse_runtime_appearance_catalog(
    bytes: Option<&[u8]>,
) -> Result<CsharpAppearanceCatalog, CsharpEngineServicesError> {
    match bytes {
        Some(bytes) => serde_json::from_slice(bytes)
            .map(CsharpAppearanceCatalog)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_RUNTIME_APPEARANCES", error.to_string())
            }),
        None => Ok(CsharpAppearanceCatalog(RuntimeAppearanceCatalog::default())),
    }
}

#[derive(Debug)]
pub struct CsharpEngineServicesError {
    code: &'static str,
    detail: String,
}

impl CsharpEngineServicesError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
    pub const fn code(&self) -> &'static str {
        self.code
    }
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for CsharpEngineServicesError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for CsharpEngineServicesError {}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

    fn binding() -> RuntimeUiRuntimeBinding {
        RuntimeUiRuntimeBinding::new(
            RuntimeInstanceId::new(1),
            RuntimeGeneration::new(1),
            RuntimeControlRevision::new(1),
        )
    }

    fn navigation_request(
        session: NativeSpatialSessionHandle,
        cells: &[NativePlanarNavCell],
        chunk_size: u32,
    ) -> NativeNavigationReplaceRequest {
        NativeNavigationReplaceRequest {
            session,
            config: NativePlanarNavConfig {
                grid_id: 1,
                cell_size: 1.0,
                chunk_size,
                max_step_cells: 1,
            },
            cells: cells.as_ptr(),
            cells_len: cells.len(),
        }
    }

    #[test]
    fn sky_background_publishes_its_texture_before_selection_and_rebuilds_on_attach() {
        let mut content = BTreeMap::new();
        content.insert(
            "sky.png".to_owned(),
            Arc::<[u8]>::from(crate::appearance::tests::RGBA_PNG),
        );
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            content,
            None,
            None,
            product_dev_host::ProductDevLog::new(Default::default()).unwrap(),
        )
        .expect("service set");

        services.begin_call(binding());
        let api = services.api();
        let mut texture = NativeRenderResourceInfo::default();
        assert_eq!(
            unsafe {
                (api.appearance.open_resource)(
                    api.appearance.context,
                    &crate::appearance::tests::resource_request("sky.png"),
                    &mut texture,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.camera_view.set_sky_background)(api.camera_view.context, texture.handle)
            },
            ABI_OK
        );
        let staged = services.take_call().expect("sky call");
        let output = services.outputs(&staged);
        assert!(matches!(
            output.frames[0].ops.as_slice(),
            [
                render_model::RenderDiff::DefineTexture { texture: defined },
                render_model::RenderDiff::SetSkyBackground { background: Some(background) },
            ] if defined.id == background.texture
                && defined.payload.is_some()
                && defined.content_hash.is_some()
        ));
        services.commit_call(staged);

        services
            .begin_attach_call(binding())
            .expect("fresh attachment stage");
        let attachment = services.take_call().expect("fresh attachment");
        let attachment_output = services.outputs(&attachment);
        assert!(matches!(
            attachment_output.frames[0].ops.as_slice(),
            [
                render_model::RenderDiff::DefineTexture { texture: defined },
                render_model::RenderDiff::SetSkyBackground { background: Some(background) },
            ] if defined.id == background.texture
        ));
        services.discard_call();

        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.set_sky_background)(
                    api.camera_view.context,
                    NativeRenderResourceHandle { value: u64::MAX },
                )
            },
            ABI_OK,
            "camera staging cannot inspect Appearance-owned handles"
        );
        let error = match services.take_call() {
            Ok(_) => panic!("unknown texture must fail atomically"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "CSHARP_RENDER_RESOURCE_HANDLE");
        services.discard_call();

        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.clear_sky_background)(
                    api.camera_view.context,
                    &NativeClearSkyBackgroundRequest::default(),
                )
            },
            ABI_OK
        );
        let cleared = services.take_call().expect("clear sky");
        let cleared_output = services.outputs(&cleared);
        assert!(matches!(
            cleared_output.frames[0].ops.as_slice(),
            [render_model::RenderDiff::SetSkyBackground { background: None }]
        ));
    }

    #[test]
    fn spatial_mutation_survives_later_call_failure_and_outer_discard() {
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            BTreeMap::new(),
            None,
            None,
            product_dev_host::ProductDevLog::new(Default::default()).unwrap(),
        )
        .expect("service set");
        services.begin_call(binding());
        let api = services.api();
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (api.spatial.create_session)(
                    api.spatial.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        let cells = [NativePlanarNavCell::default()];
        let mut first = NativeNavigationReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (api.spatial.replace_navigation)(
                    api.spatial.context,
                    &navigation_request(session, &cells, 8),
                    &mut first,
                )
            },
            ABI_OK
        );
        assert_eq!(first.navigation_revision, 1);

        let mut rejected = NativeNavigationReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (api.spatial.replace_navigation)(
                    api.spatial.context,
                    &navigation_request(session, &cells, 0),
                    &mut rejected,
                )
            },
            0,
            "the later generated call would throw and fail the product callback"
        );
        services.discard_call();

        let api = services.api();
        let mut after_discard = NativeNavigationProjectionReadout::default();
        assert_eq!(
            unsafe {
                (api.spatial.read_navigation_projection)(
                    api.spatial.context,
                    NativeNavigationProjectionReadRequest { session },
                    &mut after_discard,
                )
            },
            ABI_OK
        );
        assert!(after_discard.present);
        assert_eq!(after_discard.navigation_revision, first.navigation_revision);
        assert_eq!(after_discard.projection_hash, first.projection_hash);

        services.begin_call(binding());
        let api = services.api();
        let mut retry = NativeNavigationReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (api.spatial.replace_navigation)(
                    api.spatial.context,
                    &navigation_request(session, &cells, 8),
                    &mut retry,
                )
            },
            ABI_OK
        );
        assert_eq!(retry.navigation_revision, 2);
        let staged = services.take_call().expect("unrelated staged families");
        services.commit_call(staged);
    }

    #[test]
    fn content_backed_spatial_replacement_is_identified_and_fail_atomic() {
        let valid_path = "spatial/example/collision-navigation.json";
        let invalid_path = "spatial/example/invalid-collision-navigation.json";
        let valid = br#"{
            "schemaVersion":1,
            "staticMeshArtifactId":"mesh/example",
            "bounds":{"min":[0.0,0.0,0.0],"max":[3.0,20.0,2.0]},
            "collision":{"positions":[[0.0,0.0,0.0],[2.0,0.0,0.0],[0.0,0.0,2.0]],"triangles":[[0,1,2]]},
            "navigation":{"id":"navigation/example","config":{"schemaVersion":1,"cellSize":0.8,"levelQuantum":0.25,"maximumSlopeDegrees":45.0,"requiredHeadroom":1.0,"supportProbeDrop":0.1},"cells":[{"column":0,"row":0,"level":51,"supportHeight":12.8,"walkable":true},{"column":1,"row":0,"level":51,"supportHeight":12.8,"walkable":true},{"column":2,"row":0,"level":51,"supportHeight":12.8,"walkable":true}]}
        }"#;
        let invalid = br#"{
            "schemaVersion":1,
            "staticMeshArtifactId":"mesh/invalid",
            "bounds":{"min":[0.0,0.0,0.0],"max":[2.0,1.0,2.0]},
            "collision":{"positions":[[0.0,0.0,0.0],[2.0,0.0,0.0],[0.0,0.0,2.0]],"triangles":[[0,1,9]]},
            "navigation":{"id":"navigation/invalid","config":{"schemaVersion":1,"cellSize":1.0,"levelQuantum":0.25,"maximumSlopeDegrees":45.0,"requiredHeadroom":1.0,"supportProbeDrop":0.1},"cells":[]}
        }"#;
        let mut content = BTreeMap::new();
        content.insert(valid_path.to_owned(), Arc::<[u8]>::from(valid.as_slice()));
        content.insert(
            invalid_path.to_owned(),
            Arc::<[u8]>::from(invalid.as_slice()),
        );
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            content,
            None,
            None,
            product_dev_host::ProductDevLog::new(Default::default()).unwrap(),
        )
        .expect("service set");
        services.begin_call(binding());
        let api = services.api();
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (api.spatial.create_session)(
                    api.spatial.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        let mut valid_reference = NativeContentReferenceHandle::default();
        let valid_open = NativeContentOpenRequest {
            path: NativeUtf8Slice {
                bytes: valid_path.as_ptr(),
                len: valid_path.len(),
            },
        };
        assert_eq!(
            unsafe {
                (api.content.open_reference)(api.content.context, &valid_open, &mut valid_reference)
            },
            ABI_OK
        );
        let request = NativeSpatialContentArtifactReplaceRequest {
            session,
            content: valid_reference,
            navigation_grid_id: 7,
            navigation_chunk_size: 8,
            navigation_max_step_cells: 1,
        };
        let mut receipt = NativeSpatialContentArtifactReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (api.spatial.replace_content_artifact)(api.spatial.context, &request, &mut receipt)
            },
            ABI_OK
        );
        assert_eq!(receipt.content_reference_value, valid_reference.value);
        assert_eq!(receipt.collision_vertex_count, 3);
        assert_eq!(receipt.collision_triangle_count, 1);
        assert_eq!(receipt.navigation_cell_count, 3);

        let mut step = NativeNavigationStepReceipt::default();
        assert_eq!(
            unsafe {
                (api.spatial.propose_navigation_step)(
                    api.spatial.context,
                    NativeNavigationStepRequest {
                        session,
                        from: NativeVec3 {
                            x: 0.4,
                            y: 12.8,
                            z: 0.4,
                        },
                        target: NativeVec3 {
                            x: 2.0,
                            y: 12.8,
                            z: 0.4,
                        },
                        max_step_units: 0.5,
                        max_visited: 32,
                    },
                    &mut step,
                )
            },
            ABI_OK
        );
        assert_eq!(step.outcome, NativeNavigationPathOutcome::Reached);
        assert_eq!(step.next_path_cell.y, 51);
        assert!((step.next_waypoint.y - 12.8).abs() < f32::EPSILON);

        let mut readout = NativeSpatialContentArtifactReadout::default();
        assert_eq!(
            unsafe {
                (api.spatial.read_content_artifact)(
                    api.spatial.context,
                    NativeSpatialContentArtifactReadRequest { session },
                    &mut readout,
                )
            },
            ABI_OK
        );
        assert!(readout.present);
        assert_eq!(readout.content_sha256, receipt.content_sha256);
        assert_eq!(
            readout.collision_projection_hash,
            receipt.collision_projection_hash
        );
        assert_eq!(
            readout.navigation_projection_hash,
            receipt.navigation_projection_hash
        );

        let mut invalid_reference = NativeContentReferenceHandle::default();
        let invalid_open = NativeContentOpenRequest {
            path: NativeUtf8Slice {
                bytes: invalid_path.as_ptr(),
                len: invalid_path.len(),
            },
        };
        assert_eq!(
            unsafe {
                (api.content.open_reference)(
                    api.content.context,
                    &invalid_open,
                    &mut invalid_reference,
                )
            },
            ABI_OK
        );
        let invalid_request = NativeSpatialContentArtifactReplaceRequest {
            content: invalid_reference,
            ..request
        };
        let mut rejected = NativeSpatialContentArtifactReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (api.spatial.replace_content_artifact)(
                    api.spatial.context,
                    &invalid_request,
                    &mut rejected,
                )
            },
            0
        );
        let mut after_rejection = NativeSpatialContentArtifactReadout::default();
        assert_eq!(
            unsafe {
                (api.spatial.read_content_artifact)(
                    api.spatial.context,
                    NativeSpatialContentArtifactReadRequest { session },
                    &mut after_rejection,
                )
            },
            ABI_OK
        );
        assert_eq!(after_rejection, readout);

        assert_eq!(
            unsafe { (api.content.destroy_reference)(api.content.context, valid_reference) },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.spatial.replace_content_artifact)(api.spatial.context, &request, &mut rejected)
            },
            0,
            "a stale Content reference was accepted"
        );
        services.discard_call();
    }
}
