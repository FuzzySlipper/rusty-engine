//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.
//!
//! This crate owns the callback contexts and their Engine state. The runtime
//! crate only composes this service family with a loaded product.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use core_math::Vec3;
use csharp_engine_abi::*;
use entity_state::Quat;
use runtime_diagnostics::{RuntimeDiagnosticsSink, RuntimeUpdateAttribution};

use crate::{
    audio::{AudioRealizationFact, RuntimeAudioBridge, RuntimeAudioCall},
    authored_content::RuntimeAuthoredContentBridge,
    camera_view::RuntimeCameraViewBridge,
    content::RuntimeContentBridge,
    dynamics::RuntimeDynamicsBridge,
    persistence::RuntimePersistenceBridge,
    rng::RuntimeRngBridge,
    spatial::RuntimeSpatialBridge,
    ui::RuntimeUiBridge,
    video::{RuntimeVideoBridge, RuntimeVideoCall},
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
    destroy_material, destroy_sprite_atlas, destroy_sprite_playback, open_render_resource,
    publish_appearance_snapshot, read_light, read_presentation, read_sprite, read_sprite_playback,
    replace_light, replace_material, replace_primitive_appearance, replace_sprite_appearance,
    replace_sprite_from_atlas, replace_static_mesh_appearance,
    replace_static_mesh_from_content_appearance, sample_sprite_playback,
    select_sprite_playback_frame, set_sprite_frame, set_sprite_viewport, update_light,
    update_material, update_static_mesh_materials, RuntimeAppearanceBridge, RuntimeAppearanceCall,
};
use crate::render_resources::{
    create_mesh_resource, destroy_mesh_partition, partition_mesh, read_mesh_partition,
    take_mesh_partition_part, CsharpRenderResource,
};

#[allow(
    clippy::too_many_arguments,
    reason = "the generated ABI table needs independent mutable borrows of each named Engine bridge"
)]
fn engine_api(
    input_bridge: &mut crate::input::RuntimeInputBridge,
    gameplay_time_bridge: &mut crate::gameplay_time::RuntimeGameplayTimeBridge,
    diagnostics_bridge: &mut crate::diagnostics::RuntimeDiagnosticsBridge,
    appearance_bridge: &mut RuntimeAppearanceBridge,
    content_bridge: &mut RuntimeContentBridge,
    authored_content_bridge: &mut RuntimeAuthoredContentBridge,
    audio_bridge: &mut RuntimeAudioBridge,
    video_bridge: &mut RuntimeVideoBridge,
    render_output_bridge: &mut crate::render_output::RuntimeRenderOutputBridge,
    camera_view_bridge: &mut RuntimeCameraViewBridge,
    renderer_settings_bridge: &mut crate::renderer_settings::RuntimeRendererSettingsBridge,
    dynamics_bridge: &mut RuntimeDynamicsBridge,
    spatial_bridge: &mut RuntimeSpatialBridge,
    perception_bridge: &mut crate::perception::RuntimePerceptionBridge,
    voxel_content_bridge: &mut RuntimeVoxelContentBridge,
    voxel_scene_presentation_bridge: &mut RuntimeVoxelScenePresentationBridge,
    implicit_bridge: &mut crate::implicit_surfaces::RuntimeImplicitBridge,
    rng_bridge: &mut RuntimeRngBridge,
    persistence_bridge: &mut RuntimePersistenceBridge,
    http_bridge: &mut crate::http::RuntimeHttpBridge,
    session_bridge: &mut crate::session::RuntimeSessionBridge,
    ui_bridge: &mut RuntimeUiBridge,
) -> NativeEngineApi {
    appearance_bridge.bind_authored_content(authored_content_bridge);
    spatial_bridge.bind_appearance(appearance_bridge);
    NativeEngineApi {
        input: crate::input::api(input_bridge),
        gameplay_time: crate::gameplay_time::api(gameplay_time_bridge),
        implicit_surfaces: crate::implicit_surfaces::api(implicit_bridge, appearance_bridge),
        diagnostics: crate::diagnostics::api(diagnostics_bridge),
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
        graphics: NativeGraphicsApi {
            publish_changes: crate::appearance::publish_appearance_changes,
            context: (appearance_bridge as *mut RuntimeAppearanceBridge).cast(),
            open_resource: open_render_resource,
            read_texture_info: crate::appearance::read_texture_info,
            destroy_resource: crate::appearance::destroy_render_resource,
            open_resource_from_content: crate::appearance::open_render_resource_from_content,
            create_static_mesh_from_content_reference:
                crate::appearance::create_static_mesh_from_content_reference,
            create_material,
            update_material,
            replace_material,
            destroy_material,
            create_primitive: create_primitive_appearance,
            replace_primitive: replace_primitive_appearance,
            create_mesh_resource,
            destroy_mesh_resource: crate::appearance::destroy_mesh_resource,
            create_mesh_appearance: crate::appearance::create_mesh_appearance,
            partition_mesh,
            read_mesh_partition,
            take_mesh_partition_part,
            destroy_mesh_partition,
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
            set_sprite_viewport,
            read_sprite,
            create_sprite_playback,
            destroy_sprite_playback,
            control_sprite_playback,
            select_sprite_playback_frame,
            advance_sprite_playback,
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
            create_terrain_layer_material: crate::appearance::create_terrain_layer_material,
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
            create_ghost_plate: crate::presentation::create_ghost_plate,
            update_ghost_plate: crate::presentation::update_ghost_plate,
            recapture_ghost_plate: crate::presentation::recapture_ghost_plate,
            read_ghost_plate: crate::presentation::read_ghost_plate,
            destroy_ghost_plate: crate::presentation::destroy_ghost_plate,
        },
        animation: crate::appearance::animation_api(appearance_bridge),
        audio: crate::audio::api(audio_bridge),
        video: crate::video::api(video_bridge),
        render_output: crate::render_output::api(render_output_bridge),
        renderer_settings: crate::renderer_settings::api(renderer_settings_bridge),
        camera_view: NativeCameraViewApi {
            context: (camera_view_bridge as *mut RuntimeCameraViewBridge).cast(),
            create_camera: crate::camera_view::create_camera,
            update_camera: crate::camera_view::update_camera,
            update_camera_sample: crate::camera_view::update_camera_sample,
            replace_camera: crate::camera_view::replace_camera,
            destroy_camera: crate::camera_view::destroy_camera,
            create_camera_target: crate::camera_view::create_camera_target,
            update_camera_target: crate::camera_view::update_camera_target,
            replace_camera_target: crate::camera_view::replace_camera_target,
            destroy_camera_target: crate::camera_view::destroy_camera_target,
            set_camera_composition: crate::camera_view::set_camera_composition,
            set_active_camera: crate::camera_view::set_active_camera,
            clear_active_camera: crate::camera_view::clear_active_camera,
            set_sky_background: crate::camera_view::set_sky_background,
            set_sky_background_blend: crate::camera_view::set_sky_background_blend,
            set_fog: crate::camera_view::set_fog,
            set_tone_mapping: crate::camera_view::set_tone_mapping,
            set_bloom: crate::camera_view::set_bloom,
            set_auto_exposure: crate::camera_view::set_auto_exposure,
            set_color_grading: crate::camera_view::set_color_grading,
            set_viewport_anchor: crate::camera_view::set_viewport_anchor,
            read_surface: crate::camera_view::read_surface,
            read_viewport_anchor: crate::camera_view::read_viewport_anchor,
            clear_sky_background: crate::camera_view::clear_sky_background,
            set_background_color: crate::camera_view::set_background_color,
        },
        rng: crate::rng::api(rng_bridge),
        persistence: crate::persistence::api(persistence_bridge),
        http: crate::http::api(http_bridge),
        session: crate::session::api(session_bridge),
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
/// The runtime drives call boundaries. Services change their state directly
/// during a call; there is no rollback. A call only collects the renderer work
/// the product produced.
pub struct EngineServiceSet {
    in_call: bool,
    call_elapsed_seconds: f64,
    /// The spatial sessions' distance fields changed between calls: the
    /// presentations republish them at the next settle.
    fields_changed: bool,
    presentation_world: render_presentation::PresentationWorld,
    input: crate::input::RuntimeInputBridge,
    gameplay_time: crate::gameplay_time::RuntimeGameplayTimeBridge,
    diagnostics: crate::diagnostics::RuntimeDiagnosticsBridge,
    appearance: RuntimeAppearanceBridge,
    content: Box<RuntimeContentBridge>,
    authored_content: RuntimeAuthoredContentBridge,
    audio: RuntimeAudioBridge,
    video: RuntimeVideoBridge,
    render_output: crate::render_output::RuntimeRenderOutputBridge,
    camera_view: Box<RuntimeCameraViewBridge>,
    renderer_settings: crate::renderer_settings::RuntimeRendererSettingsBridge,
    dynamics: RuntimeDynamicsBridge,
    spatial: RuntimeSpatialBridge,
    perception: crate::perception::RuntimePerceptionBridge,
    voxel_content: RuntimeVoxelContentBridge,
    voxel_scene_presentation: RuntimeVoxelScenePresentationBridge,
    implicit: crate::implicit_surfaces::RuntimeImplicitBridge,
    rng: RuntimeRngBridge,
    persistence: RuntimePersistenceBridge,
    http: crate::http::RuntimeHttpBridge,
    session: crate::session::RuntimeSessionBridge,
    ui: RuntimeUiBridge,
}

/// A finished product call: its renderer work and any input mapping
/// replacement it selected. Every service already holds the call's changes.
pub struct CsharpEngineCall {
    input_mapping_replacement: Option<runtime_input::CompiledInputMappings>,
    output: CsharpEngineCallOutput,
}

/// Each service's state for the duration of one call. Services own their
/// state again when the call finishes, whether or not the call succeeded.
struct ServiceCalls {
    implicit: crate::implicit_surfaces::RuntimeImplicitCall,
    presentation_world: render_presentation::PresentationWorld,
    appearance: RuntimeAppearanceCall,
    audio: RuntimeAudioCall,
    video: RuntimeVideoCall,
    render_output: crate::render_output::RuntimeRenderOutputCall,
    camera_view: crate::camera_view::RuntimeCameraViewCall,
    renderer_settings: crate::renderer_settings::RuntimeRendererSettingsCall,
    ui: Vec<RuntimeUiProjectionEnvelope>,
    voxel_content: crate::voxel_content::RuntimeVoxelContentCall,
    voxel_scene_presentation: crate::voxel_scene_presentation::RuntimeVoxelScenePresentationCall,
}

/// Engine observations from one product call.
#[derive(Default)]
pub struct CsharpEngineCallOutput {
    /// The call's newly settled output jobs, for the runtime's executor.
    pub render_output: Vec<crate::render_output::RenderOutputWork>,
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
}

/// Parsed optional appearance catalog retained with admitted product content.
/// The catalog remains an Engine presentation detail rather than a runtime-host
/// dependency.
pub struct CsharpAppearanceCatalog(RuntimeAppearanceCatalog);

impl EngineServiceSet {
    /// Bind metadata-only build bundles before product creation, or rebind a
    /// re-admitted inventory after a development content edit. Only later
    /// opens see the new inventory.
    pub fn bind_content_bundles(&mut self, bundles: crate::ProductContentBundles) {
        self.content.bind_bundles(bundles);
    }

    pub fn new(
        catalog: CsharpAppearanceCatalog,
        content_resources: BTreeMap<String, Arc<[u8]>>,
        persistence_root: Option<PathBuf>,
        diagnostics_sink: RuntimeDiagnosticsSink,
    ) -> Result<Self, CsharpEngineServicesError> {
        Self::with_direct_intents(
            catalog,
            content_resources,
            persistence_root,
            diagnostics_sink,
            Vec::new(),
        )
    }

    pub fn with_direct_intents(
        catalog: CsharpAppearanceCatalog,
        content_resources: BTreeMap<String, Arc<[u8]>>,
        persistence_root: Option<PathBuf>,
        diagnostics_sink: RuntimeDiagnosticsSink,
        direct_intents: Vec<runtime_input::DirectInputIntentDescriptor>,
    ) -> Result<Self, CsharpEngineServicesError> {
        let mut spatial = crate::spatial::RuntimeSpatialBridge::new();
        let perception = crate::perception::RuntimePerceptionBridge::new(&spatial);
        let dynamics = crate::dynamics::RuntimeDynamicsBridge::new(spatial.collision_source());
        let content = Box::new(RuntimeContentBridge::new(content_resources.clone()));
        spatial.bind_content(&content);
        let mut authored_content = RuntimeAuthoredContentBridge::new();
        authored_content.bind_content(&content);
        let voxel_scene_presentation =
            RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let camera_view = Box::new(RuntimeCameraViewBridge::new());
        let mut appearance = crate::appearance::create(catalog.0, content_resources.clone());
        appearance.bind_camera_view(&camera_view);
        appearance.bind_diagnostics_sink(diagnostics_sink.clone());
        appearance.bind_content(&content);
        let mut audio = RuntimeAudioBridge::new(content_resources.clone());
        audio.bind_diagnostics_sink(diagnostics_sink.clone());
        audio.bind_content(&content);
        let mut video = RuntimeVideoBridge::new(content_resources.clone());
        video.bind_content(&content);
        let mut voxel_content = RuntimeVoxelContentBridge::new();
        voxel_content.bind_content(&content);
        let mut ui = crate::ui::RuntimeUiBridge::new();
        ui.bind_content(&content);
        Ok(Self {
            in_call: false,
            call_elapsed_seconds: 0.0,
            fields_changed: false,
            presentation_world: render_presentation::PresentationWorld::default(),
            input: crate::input::RuntimeInputBridge::new(direct_intents),
            gameplay_time: crate::gameplay_time::RuntimeGameplayTimeBridge::new(),
            diagnostics: crate::diagnostics::RuntimeDiagnosticsBridge::new(diagnostics_sink),
            appearance,
            content,
            authored_content,
            audio,
            video,
            render_output: crate::render_output::RuntimeRenderOutputBridge::new(),
            camera_view,
            renderer_settings: crate::renderer_settings::RuntimeRendererSettingsBridge::new(),
            dynamics,
            spatial,
            perception,
            voxel_content,
            voxel_scene_presentation,
            implicit: crate::implicit_surfaces::RuntimeImplicitBridge::new(),
            rng: crate::rng::RuntimeRngBridge::new(),
            session: crate::session::RuntimeSessionBridge::new(persistence_root.clone()),
            persistence: crate::persistence::RuntimePersistenceBridge::new(persistence_root),
            http: crate::http::RuntimeHttpBridge::new(),
            ui,
        })
    }

    /// The images and fonts the product granted its UI, which the product host serves.
    pub fn ui_files(&self) -> Arc<crate::UiFiles> {
        self.ui.files()
    }

    pub fn api(&mut self) -> NativeEngineApi {
        engine_api(
            &mut self.input,
            &mut self.gameplay_time,
            &mut self.diagnostics,
            &mut self.appearance,
            &mut self.content,
            &mut self.authored_content,
            &mut self.audio,
            &mut self.video,
            &mut self.render_output,
            &mut self.camera_view,
            &mut self.renderer_settings,
            &mut self.dynamics,
            &mut self.spatial,
            &mut self.perception,
            &mut self.voxel_content,
            &mut self.voxel_scene_presentation,
            &mut self.implicit,
            &mut self.rng,
            &mut self.persistence,
            &mut self.http,
            &mut self.session,
            &mut self.ui,
        )
    }

    /// The presentation surface and anchored UI rects the page last
    /// reported, which `CameraView.ReadSurface` and `ReadViewportAnchor` read
    /// during the next product call.
    pub fn ingest_camera_surface(
        &mut self,
        surface: NativeCameraSurfaceReadout,
        viewport_anchors: render_host_contracts::RendererViewportAnchors,
    ) {
        self.camera_view.set_surface(surface, viewport_anchors);
    }

    /// The renderer's settings in effect and what the device refused, as
    /// `RendererSettings.Read` returns them during the next product call.
    pub fn ingest_renderer_settings(
        &mut self,
        readout: csharp_engine_abi::NativeRendererSettingsReadout,
    ) {
        // Chunk distance fields are built only while the renderer traces
        // them: what draws, not what was asked for, on a device that refuses.
        let traced = readout.effective.ambient_occlusion
            == csharp_engine_abi::NativeAmbientOcclusionMode::DistanceField;
        self.fields_changed |= self.spatial.set_distance_fields(traced);
        self.renderer_settings.ingest(readout);
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

    /// The lifecycle's gameplay time selection, which `GameplayTime.Read`
    /// answers and requests build on. `fixed_step_hz` is `None` for a runtime
    /// without a realtime rate.
    pub fn set_gameplay_time(
        &mut self,
        fixed_step_hz: Option<u32>,
        effective: runtime_lifecycle::GameplayTime,
    ) {
        self.gameplay_time.set(fixed_step_hz, effective);
    }

    /// The gameplay time the last call selected, for the runtime to settle
    /// after `finish_call` whether or not finishing failed: a request the
    /// product saw succeed is kept, as every other call change is.
    pub fn take_gameplay_time_request(&mut self) -> Option<crate::GameplayTimeRequest> {
        self.gameplay_time.take_call()
    }

    pub fn begin_call(&mut self, ui_binding: RuntimeUiRuntimeBinding) {
        self.begin_services(ui_binding, None, false);
    }

    /// Product construction is the only non-update callback that can select a
    /// complete initial physical map before the runtime input lane exists.
    pub fn begin_create_call(&mut self, ui_binding: RuntimeUiRuntimeBinding) {
        self.begin_services(ui_binding, None, true);
    }

    pub fn begin_lifecycle_call(&mut self, ui_binding: RuntimeUiRuntimeBinding) {
        self.begin_services(ui_binding, None, true);
    }

    pub fn begin_update_call(
        &mut self,
        ui_binding: RuntimeUiRuntimeBinding,
        facts: NativeProductUpdateFacts,
    ) {
        self.spatial.reset_update_attribution();
        self.voxel_scene_presentation.reset_update_attribution();
        self.begin_services(ui_binding, Some(facts), true);
    }

    /// Returns every committed renderer stream continuation point. Detached
    /// attachment projectors deliberately do not participate: their revision
    /// only describes the fresh baseline construction, not the active product
    /// publication frontier that the replacement renderer must continue.
    pub fn renderer_publication_frontiers(&self) -> Vec<(String, u64)> {
        vec![(
            render_presentation::PRESENTATION_WORLD_STREAM.to_owned(),
            self.presentation_world.revision(),
        )]
    }

    /// Returns one completed update sample after the C# callback has returned.
    /// Service durations are nested within `callback_duration_us`, not additive
    /// frame stages.
    pub fn complete_update_attribution(
        &self,
        callback_duration_us: u64,
    ) -> RuntimeUpdateAttribution {
        let spatial = self.spatial.update_attribution();
        let presentation = self.voxel_scene_presentation.update_attribution();
        RuntimeUpdateAttribution {
            callback_duration_us,
            post_callback_duration_us: 0,
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

    fn begin_services(
        &mut self,
        ui_binding: RuntimeUiRuntimeBinding,
        update: Option<NativeProductUpdateFacts>,
        accepts_input_replacement: bool,
    ) {
        if self.in_call {
            // A call that was never finished keeps its changes like any other.
            let _ = self.finish_call();
        }
        self.in_call = true;
        self.call_elapsed_seconds = update.map_or(0.0, |facts| {
            facts.fixed_delta_seconds * f64::from(facts.admitted_step_count)
        });
        match update {
            Some(facts) => self.appearance.begin_update_call(facts),
            None => self.appearance.begin_call(),
        }
        self.input.begin_call(accepts_input_replacement);
        self.gameplay_time.begin_call();
        self.audio.begin_call();
        if update.is_some() {
            self.audio.advance_elapsed(self.call_elapsed_seconds);
        }
        self.video.begin_call();
        self.render_output.begin_call();
        self.http.begin_call();
        self.session.begin_call();
        self.camera_view.begin_call();
        self.renderer_settings.begin_call();
        self.ui.begin_call(ui_binding);
        self.voxel_content.begin_call();
        self.voxel_scene_presentation.begin_call();
        self.implicit.begin_call();
    }

    /// Copies audio output realization facts while C# is not executing. The
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

    pub fn ingest_video_realization_feedback(
        &mut self,
        replace_owner: bool,
        evicted_fact_count: u64,
        facts: impl IntoIterator<Item = crate::video::VideoRealizationFact>,
    ) -> Result<(), CsharpEngineServicesError> {
        self.video
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

    /// Replaces the renderer's latest ghost-plate realization
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

    pub fn reset_video_realization_owner(&mut self) {
        self.video.reset_realized_feedback();
    }

    pub fn reset_animation_realization_owner(&mut self) {
        self.appearance
            .ingest_animation_realization_feedback(true, 0, []);
    }

    pub fn reset_ghost_plate_realization_owner(&mut self) {
        self.appearance.ingest_ghost_plate_realization(true, []);
    }

    /// Ends a product call, successful or not. Every service keeps what the
    /// call did: there is no rollback. Returns the call's renderer work,
    /// applied to the presentation world. On an error the services still own
    /// the call's changes; only the remaining renderer work is lost, and the
    /// caller rebaselines renderers.
    pub fn finish_call(&mut self) -> Result<CsharpEngineCall, CsharpEngineServicesError> {
        if !std::mem::take(&mut self.in_call) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRODUCT_CALL",
                "no product call is open",
            ));
        }
        let input_mapping_replacement = self.input.take_call();
        let mut calls = ServiceCalls {
            implicit: self.implicit.take_call()?,
            presentation_world: std::mem::take(&mut self.presentation_world),
            appearance: self.appearance.take_staged_call(),
            audio: self.audio.take_staged_call()?,
            video: self.video.take_staged_call()?,
            render_output: self.render_output.take_call()?,
            camera_view: self.camera_view.take_staged_call()?,
            renderer_settings: self.renderer_settings.take_staged_call()?,
            ui: self.ui.finish_call(),
            voxel_content: self.voxel_content.take_staged_call()?,
            voxel_scene_presentation: self.voxel_scene_presentation.take_staged_call()?,
        };
        let output = self.settle_call(&mut calls);
        self.presentation_world = calls.presentation_world;
        self.appearance.commit(calls.appearance);
        self.implicit.commit_call(calls.implicit);
        self.audio.commit(calls.audio);
        self.video.commit(calls.video);
        self.render_output.commit(calls.render_output);
        self.camera_view.commit(calls.camera_view);
        self.voxel_content.commit_call(calls.voxel_content);
        self.voxel_scene_presentation
            .commit_call(calls.voxel_scene_presentation);
        Ok(CsharpEngineCall {
            input_mapping_replacement,
            output: output?,
        })
    }

    fn settle_call(
        &mut self,
        calls: &mut ServiceCalls,
    ) -> Result<CsharpEngineCallOutput, CsharpEngineServicesError> {
        if let Some(error) = calls.appearance.release_error.take() {
            return Err(error);
        }
        // Sky resources are owned and admitted by Appearance.
        let sky_frame = crate::camera_view::environment_frame(
            &calls.camera_view,
            &calls.appearance,
            calls.renderer_settings.settings,
        )?;
        // A selection this call builds or drops the sessions' fields now; the
        // device's answer arrives with the next report.
        if let Some(settings) = calls.renderer_settings.settings {
            let traced = settings.ambient_occlusion.mode
                == render_model::AmbientOcclusionMode::DistanceField;
            self.fields_changed |= self.spatial.set_distance_fields(traced);
        }
        if std::mem::take(&mut self.fields_changed) {
            self.voxel_scene_presentation
                .refresh_all(&mut calls.voxel_scene_presentation)?;
        }
        self.voxel_scene_presentation.settle_level_of_detail(
            &mut calls.voxel_scene_presentation,
            calls.camera_view.primary_camera_position(),
        )?;
        calls
            .presentation_world
            .advance_elapsed(self.call_elapsed_seconds);
        let mut output = Self::take_raw_outputs(calls, sky_frame);
        for item in &mut output.appearance {
            match item {
                CsharpAppearanceCallOutput::Frame(frame) => {
                    *frame = calls
                        .presentation_world
                        .apply(std::mem::take(frame))
                        .map_err(presentation_world_error)?;
                }
                CsharpAppearanceCallOutput::Presentation(frame) => {
                    *frame = calls
                        .presentation_world
                        .apply_presentation(std::mem::take(frame))
                        .map_err(presentation_world_error)?;
                }
            }
        }
        for frame in &mut output.frames {
            *frame = calls
                .presentation_world
                .apply(std::mem::take(frame))
                .map_err(presentation_world_error)?;
        }
        for frame in &mut output.presentation {
            *frame = calls
                .presentation_world
                .apply_presentation(std::mem::take(frame))
                .map_err(presentation_world_error)?;
        }
        output.render_output = self.render_output.settle(
            &mut calls.render_output,
            &calls.presentation_world,
            &calls.camera_view,
            &calls.appearance.state,
        );
        Ok(output)
    }

    pub fn seal_resource_selection(&mut self) {
        self.appearance.seal_resource_selection();
        self.audio.seal_resource_selection();
    }

    /// Committed retained-audio baseline (voices at their Engine cursors and
    /// bus state) for an in-process audio realization.
    pub fn audio_snapshot_frame(
        &self,
    ) -> Result<render_presentation::PresentationFrameDiff, CsharpEngineServicesError> {
        self.audio.snapshot_frame()
    }

    /// The committed video playback (its play op, if a clip is playing),
    /// for an in-process video realization.
    pub fn video_snapshot_frame(&self) -> render_presentation::PresentationFrameDiff {
        self.video.snapshot_frame()
    }

    /// The Engine presentation timeline the committed world has reached.
    pub fn presentation_elapsed_seconds(&self) -> f64 {
        self.presentation_world.elapsed_seconds()
    }

    /// World-space origin of the committed graphics node published for an
    /// entity, for entity-attached audio emitters.
    pub fn entity_world_position(&self, entity: u64) -> Option<[f32; 3]> {
        self.presentation_world.entity_world_position(entity)
    }

    /// The committed camera/view composition, for a realization that needs
    /// the current listener without waiting for the next camera change.
    pub fn view_composition(
        &self,
    ) -> Result<render_host_contracts::RendererViewComposition, CsharpEngineServicesError> {
        self.camera_view.snapshot_composition()
    }

    /// Encoded bytes of an admitted audio clip, shared without copying.
    pub fn audio_clip_bytes(&self, content_hash: &str) -> Option<Arc<[u8]>> {
        self.audio
            .render_resources()
            .find(|resource| resource.content_hash() == content_hash)
            .map(CsharpRenderResource::shared_bytes)
    }

    /// A retained renderer resource by identity, borrowed for an in-process
    /// renderer that reads the bytes while it holds the services.
    pub fn borrowed_renderer_resource(&self, identity: &str) -> Option<&CsharpRenderResource> {
        self.renderer_resources()
            .find(|resource| resource.identity() == identity)
    }

    /// Every resource a renderer of the committed scene may ask for.
    pub fn renderer_resources(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        self.appearance
            .state
            .render_resources
            .iter()
            .chain(self.appearance.state.render_resources.recently_released())
            .chain(self.audio.render_resources())
            .chain(self.video.render_resources())
    }

    /// Latest projection of every UI stream, bound to `binding`. A browser
    /// clears UI projections when its runtime binding changes, so a control
    /// fence republishes these without rebuilding the rest of the world.
    pub fn snapshot_ui_projections(
        &self,
        binding: RuntimeUiRuntimeBinding,
    ) -> Vec<runtime_ui::RuntimeUiProjectionEnvelope> {
        self.ui.snapshot_projections(binding)
    }

    /// Complete committed state for a fresh renderer. No product callback,
    /// projector reset, resource admission, or active publication occurs.
    pub fn snapshot_outputs(
        &self,
        binding: RuntimeUiRuntimeBinding,
    ) -> Result<CsharpEngineCallOutput, CsharpEngineServicesError> {
        let snapshot = self.presentation_world.snapshot();
        // Retained effect and media baselines are built from committed state
        // only here, for the fresh renderer that needs them.
        let mut presentation = self
            .presentation_world
            .with_retained_captures(self.appearance.snapshot_presentation()?);
        let audio = self.audio.snapshot_frame()?;
        if !audio.ops.is_empty() {
            presentation.push(audio);
        }
        let video = self.video.snapshot_frame();
        if !video.ops.is_empty() {
            presentation.push(video);
        }
        Ok(CsharpEngineCallOutput {
            render_output: Vec::new(),
            appearance: vec![CsharpAppearanceCallOutput::Frame(snapshot.frame)],
            frames: Vec::new(),
            view_composition: Some(self.camera_view.snapshot_composition()?),
            ui: self.ui.snapshot_projections(binding),
            presentation,
        })
    }

    fn take_raw_outputs(
        call: &mut ServiceCalls,
        sky_frame: Option<render_model::RenderFrameDiff>,
    ) -> CsharpEngineCallOutput {
        let appearance = std::mem::take(&mut call.appearance.outputs)
            .into_iter()
            .map(|output| match output {
                crate::appearance::RuntimeAppearanceCallOutput::Frame(frame) => {
                    CsharpAppearanceCallOutput::Frame(frame)
                }
                crate::appearance::RuntimeAppearanceCallOutput::Presentation(frame) => {
                    CsharpAppearanceCallOutput::Presentation(frame)
                }
            })
            .collect::<Vec<_>>();
        let mut frames = Vec::new();
        frames.extend(sky_frame);
        frames.append(&mut call.voxel_content.frames);
        frames.append(&mut call.voxel_scene_presentation.frames);
        CsharpEngineCallOutput {
            render_output: Vec::new(),
            appearance,
            frames,
            view_composition: call.camera_view.composition.take(),
            ui: std::mem::take(&mut call.ui),
            presentation: call
                .audio
                .frame
                .take()
                .into_iter()
                .chain(call.video.frame.take())
                .collect(),
        }
    }
}

impl CsharpEngineCall {
    pub fn take_output(&mut self) -> CsharpEngineCallOutput {
        std::mem::take(&mut self.output)
    }

    pub fn take_input_mapping_replacement(
        &mut self,
    ) -> Option<runtime_input::CompiledInputMappings> {
        self.input_mapping_replacement.take()
    }
}

fn presentation_world_error(
    error: render_presentation::PresentationWorldError,
) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_PRESENTATION_WORLD", error.to_string())
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

#[derive(Clone, Debug)]
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

    #[test]
    fn settlement_builds_no_effect_or_media_baseline_until_a_snapshot_reads_it() {
        use crate::appearance::{
            GRAPHICS_SNAPSHOT_READS, MEDIA_SNAPSHOT_READS, RESOURCE_INVENTORY_READS,
        };
        let billboards = |services: &EngineServiceSet| {
            services
                .snapshot_outputs(binding())
                .unwrap()
                .presentation
                .iter()
                .flat_map(|frame| &frame.ops)
                .filter(|op| matches!(op, render_presentation::PresentationOp::Billboard { .. }))
                .count()
        };
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).unwrap(),
            BTreeMap::from([(
                "sky.png".to_owned(),
                Arc::<[u8]>::from(crate::appearance::tests::RGBA_PNG),
            )]),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .unwrap();
        GRAPHICS_SNAPSHOT_READS.with(|c| c.set(0));
        MEDIA_SNAPSHOT_READS.with(|c| c.set(0));
        services.begin_call(binding());
        let api = services.api();
        let mut texture = NativeRenderResourceInfo::default();
        assert_eq!(
            unsafe {
                (api.graphics.open_resource)(
                    api.graphics.context,
                    &crate::appearance::tests::resource_request("sky.png"),
                    &mut texture,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
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
        let billboard = services
            .appearance
            .presentation_create_billboard(&NativePresentationBillboardDescriptor {
                logical_id: 7,
                anchor,
                content_kind: NativeBillboardContentKind::Text,
                localization_key: slice(key),
                fallback_text: slice(text),
                value: empty,
                unit_key: empty,
                fallback_unit: empty,
                texture: NativeRenderResourceReference::default(),
                font_kind: NativePresentationFontKind::System,
                font_asset: NativeRenderResourceReference::default(),
                font_family: slice(font),
                height_pixels: 16.0,
                color,
                background: NativeColor::default(),
                max_distance: 100.0,
                layer: NativePresentationBillboardLayer::AlwaysOnTop,
                visible: true,
            })
            .expect("text billboard");
        services.finish_call().unwrap();
        // Creating a retained billboard changes appearance state, but
        // settlement no longer rebuilds the effect or media baselines.
        GRAPHICS_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 0));
        MEDIA_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 0));
        RESOURCE_INVENTORY_READS.with(|c| c.set(0));
        for _ in 0..3 {
            services.begin_update_call(
                binding(),
                NativeProductUpdateFacts {
                    mode: NativeProductUpdateMode::Realtime,
                    lifecycle_state: NativeProductLifecycleState::Running,
                    generation: 1,
                    control_revision: 1,
                    observed_host_time_nanoseconds: 0,
                    simulation_step: 1,
                    fixed_step_hz: 60,
                    admitted_step_count: 1,
                    dropped_step_count: 0,
                    fixed_delta_seconds: 1.0 / 60.0,
                    gameplay_time_selected: false,
                    gameplay_rate: 1.0,
                    gameplay_advance_remaining_steps: 0,
                    host_elapsed_seconds: 0.0,
                },
            );
            services.finish_call().unwrap();
        }
        GRAPHICS_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 0));
        MEDIA_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 0));
        RESOURCE_INVENTORY_READS.with(|c| assert_eq!(c.get(), 0));
        // A fresh attachment builds them once, from committed state.
        assert_eq!(billboards(&services), 1);
        GRAPHICS_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 1));
        MEDIA_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 2));
        services.begin_call(binding());
        services
            .appearance
            .presentation_destroy_billboard(billboard)
            .unwrap();
        services.finish_call().unwrap();
        GRAPHICS_SNAPSHOT_READS.with(|c| assert_eq!(c.get(), 1));
        assert_eq!(billboards(&services), 0);
    }

    #[test]
    fn a_gameplay_time_request_survives_a_failed_call_settlement() {
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).unwrap(),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .unwrap();
        services.set_gameplay_time(Some(60), runtime_lifecycle::GameplayTime::default());
        services.begin_call(binding());
        let api = services.api();
        let mut readout = NativeGameplayTimeReadout::default();
        let mut error = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            (api.gameplay_time.select_rate)(
                api.gameplay_time.context,
                &NativeGameplayTimeRateRequest { rate: 0.0 },
                &mut readout,
                &mut error,
            )
        };
        assert_eq!(status, ABI_OK, "the product sees its hold succeed");
        services.appearance.fail_settlement_for_test();
        assert!(services.finish_call().is_err());
        assert_eq!(
            services.take_gameplay_time_request(),
            Some(crate::GameplayTimeRequest::Rate(
                runtime_lifecycle::GameplayRate::HOLD
            ))
        );
        // Taken once; the next call starts with nothing staged.
        assert_eq!(services.take_gameplay_time_request(), None);
    }

    #[test]
    fn world_presentation_time_advances_only_with_admitted_steps() {
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).unwrap(),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .unwrap();
        let update = |services: &mut EngineServiceSet, step: u64, steps: u32| {
            services.begin_update_call(
                binding(),
                NativeProductUpdateFacts {
                    mode: NativeProductUpdateMode::Realtime,
                    lifecycle_state: NativeProductLifecycleState::Running,
                    generation: 1,
                    control_revision: 1,
                    observed_host_time_nanoseconds: 0,
                    simulation_step: step,
                    fixed_step_hz: 60,
                    admitted_step_count: steps,
                    dropped_step_count: 0,
                    fixed_delta_seconds: 1.0 / 60.0,
                    gameplay_time_selected: true,
                    gameplay_rate: 0.0,
                    gameplay_advance_remaining_steps: 0,
                    host_elapsed_seconds: 1.0 / 60.0,
                },
            );
            services.finish_call().unwrap();
        };
        // Held: updates without steps, however much host time passes, add
        // no world time (animation, particles, shader time, audio cursors).
        for _ in 0..600 {
            update(&mut services, 0, 0);
        }
        assert_eq!(services.presentation_elapsed_seconds(), 0.0);
        // A two-second advance, step by step and in catch-up batches,
        // counts every admitted step exactly once.
        for step in 0..60 {
            update(&mut services, step, 1);
        }
        for batch in 0..15 {
            update(&mut services, 60 + batch * 4, 4);
        }
        assert!((services.presentation_elapsed_seconds() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn refused_audio_and_graphics_calls_retain_their_own_reason_until_exact_release() {
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).unwrap(),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .unwrap();
        services.begin_call(binding());
        let api = services.api();
        let mut audio_error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        let mut graphics_error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        let mut voice = NativeAudioVoiceHandle::default();
        let request = NativeAudioSourceDescriptor {
            clip: NativeAudioClipHandle { value: 99 },
            bus: NativeAudioBus::Sfx,
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            spatial_blend: 0.0,
            max_distance: 1.0,
            rolloff: NativeAudioRolloff::Linear,
            pan: 0.0,
            emitter_kind: NativeAudioEmitterKind::Global2d,
            position: NativeVec3::default(),
            entity: 0,
            offset: NativeVec3::default(),
        };
        assert_eq!(
            unsafe {
                (api.audio.create_voice)(api.audio.context, &request, &mut voice, &mut audio_error)
            },
            0
        );
        assert_eq!(
            unsafe {
                (api.graphics.destroy_sprite_atlas)(
                    api.graphics.context,
                    NativeSpriteAtlasHandle { value: 99 },
                    &mut graphics_error,
                )
            },
            0
        );
        for (receipt, code, detail) in [
            (
                &audio_error,
                "CSHARP_AUDIO_CLIP_HANDLE",
                "audio clip handle is not admitted",
            ),
            (
                &graphics_error,
                "CSHARP_SPRITE_ATLAS_HANDLE",
                "sprite atlas is not live",
            ),
        ] {
            assert_eq!(receipt.diagnostics_len, 1);
            let diagnostic = unsafe { &*receipt.diagnostics };
            assert_eq!(
                unsafe { borrowed_utf8(diagnostic.code.bytes, diagnostic.code.len, "code") }
                    .unwrap(),
                code
            );
            assert_eq!(
                unsafe {
                    borrowed_utf8(diagnostic.message.bytes, diagnostic.message.len, "message")
                }
                .unwrap(),
                detail
            );
        }
    }
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
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .expect("service set");

        services.begin_call(binding());
        let api = services.api();
        let mut texture = NativeRenderResourceInfo::default();
        assert_eq!(
            unsafe {
                (api.graphics.open_resource)(
                    api.graphics.context,
                    &crate::appearance::tests::resource_request("sky.png"),
                    &mut texture,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.camera_view.set_sky_background)(
                    api.camera_view.context,
                    texture.handle,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut staged = services.finish_call().expect("sky call");
        let output = staged.take_output();
        assert!(matches!(
            output.frames[0].ops.as_slice(),
            [
                render_model::RenderDiff::DefineTexture { texture: defined },
                render_model::RenderDiff::SetSkyBackground { background: Some(background) },
            ] if defined.id == background.texture
                && defined.payload.is_some()
                && defined.content_hash.is_some()
        ));

        let attachment_output = services
            .snapshot_outputs(binding())
            .expect("fresh attachment");
        let CsharpAppearanceCallOutput::Frame(frame) = &attachment_output.appearance[0] else {
            panic!("baseline graphics frame");
        };
        assert!(matches!(
            frame.ops.as_slice(),
            [
                render_model::RenderDiff::DefineTexture { texture: defined },
                render_model::RenderDiff::SetSkyBackground { background: Some(background) },
            ] if defined.id == background.texture
        ));

        // Re-selecting the same retained texture cannot emit a stale version
        // update or advance the canonical graphics frontier.
        let frontier = services.renderer_publication_frontiers();
        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.set_sky_background)(
                    api.camera_view.context,
                    texture.handle,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut call = services.finish_call().unwrap();
        assert!(call
            .take_output()
            .frames
            .iter()
            .all(|frame| frame.ops.is_empty()));
        assert_eq!(services.renderer_publication_frontiers(), frontier);

        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.set_sky_background)(
                    api.camera_view.context,
                    NativeRenderResourceHandle { value: u64::MAX },
                    std::ptr::null_mut(),
                )
            },
            ABI_OK,
            "camera staging cannot inspect Appearance-owned handles"
        );
        let error = match services.finish_call() {
            Ok(_) => panic!("unknown texture must fail atomically"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "CSHARP_RENDER_RESOURCE_HANDLE");
        let _ = services.finish_call();

        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.set_background_color)(
                    api.camera_view.context,
                    &NativeSetBackgroundColorRequest {
                        color: NativeColor {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        },
                    },
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut colored = services.finish_call().expect("background color");
        let colored_output = colored.take_output();
        assert!(matches!(
            colored_output.frames[0].ops.as_slice(),
            [render_model::RenderDiff::SetBackgroundColor { color }]
                if *color == [0.0, 0.0, 0.0, 1.0]
        ));

        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.clear_sky_background)(
                    api.camera_view.context,
                    &NativeClearSkyBackgroundRequest::default(),
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut cleared = services.finish_call().expect("clear sky");
        let cleared_output = cleared.take_output();
        assert!(matches!(
            cleared_output.frames[0].ops.as_slice(),
            [render_model::RenderDiff::SetSkyBackground { background: None }]
        ));
    }

    #[test]
    fn renderer_settings_publish_as_a_retained_op_and_read_returns_the_renderer_report() {
        use crate::operation_diagnostics::{empty_receipt, receipt_codes};
        use csharp_engine_abi::{
            NativeAmbientOcclusionMode, NativeAntialiasing, NativeRendererSettingRefusal,
            NativeRendererSettingsReadout, NativeRendererSettingsRequest,
        };
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .expect("service set");
        let request = NativeRendererSettingsRequest {
            shadows: true,
            shadow_budget: 3,
            ambient_occlusion: NativeAmbientOcclusionMode::DistanceField,
            ambient_occlusion_strength: 0.8,
            ambient_occlusion_radius: 1.5,
            antialiasing: NativeAntialiasing::Msaa2,
            render_scale: 0.75,
            vsync: false,
            clustered_lighting: true,
            gpu_culling: true,
        };
        services.begin_call(binding());
        let api = services.api();
        let set = |request: NativeRendererSettingsRequest, refusal| unsafe {
            (api.renderer_settings.set)(api.renderer_settings.context, &request, refusal)
        };
        assert_eq!(set(request, std::ptr::null_mut()), ABI_OK);
        let mut call = services.finish_call().expect("settings call");
        let selected = render_model::RenderDiff::SetRendererSettings {
            settings: render_model::RendererSettingsDescriptor {
                shadows: true,
                shadow_budget: Some(3),
                ambient_occlusion: render_model::AmbientOcclusionSettings {
                    mode: render_model::AmbientOcclusionMode::DistanceField,
                    strength: 0.8,
                    radius: 1.5,
                },
                antialiasing: 2,
                render_scale: 0.75,
                vsync: false,
                clustered_lighting: true,
                gpu_culling: true,
            },
        };
        assert_eq!(call.take_output().frames[0].ops, vec![selected.clone()]);
        let attachment = services
            .snapshot_outputs(binding())
            .expect("fresh attachment");
        let CsharpAppearanceCallOutput::Frame(frame) = &attachment.appearance[0] else {
            panic!("baseline graphics frame");
        };
        assert!(
            frame.ops.contains(&selected),
            "a rebaseline keeps the selection"
        );

        // The runtime's report is what Read returns; an invalid request is
        // refused with a code and changes nothing.
        let mut readout = NativeRendererSettingsReadout {
            requested: request,
            effective: request,
            ambient_occlusion_refusal: NativeRendererSettingRefusal::NoComputeShaders,
            antialiasing_refusal: NativeRendererSettingRefusal::None,
            vsync_refusal: NativeRendererSettingRefusal::NoDisplay,
            clustered_lighting_refusal: NativeRendererSettingRefusal::None,
            gpu_culling_refusal: NativeRendererSettingRefusal::None,
        };
        readout.effective.ambient_occlusion = NativeAmbientOcclusionMode::ScreenSpace;
        services.ingest_renderer_settings(readout);
        services.begin_call(binding());
        let api = services.api();
        let mut read = NativeRendererSettingsReadout {
            ambient_occlusion_refusal: NativeRendererSettingRefusal::None,
            ..readout
        };
        assert_eq!(
            unsafe {
                (api.renderer_settings.read)(
                    api.renderer_settings.context,
                    &mut read,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            read.ambient_occlusion_refusal,
            NativeRendererSettingRefusal::NoComputeShaders
        );
        assert_eq!(
            read.effective.ambient_occlusion,
            NativeAmbientOcclusionMode::ScreenSpace
        );
        assert_eq!(read.vsync_refusal, NativeRendererSettingRefusal::NoDisplay);
        let mut refusal = empty_receipt();
        assert_eq!(
            unsafe {
                (api.renderer_settings.set)(
                    api.renderer_settings.context,
                    &NativeRendererSettingsRequest {
                        ambient_occlusion_strength: -1.0,
                        ..request
                    },
                    &mut refusal,
                )
            },
            0
        );
        assert_eq!(receipt_codes(&refusal), ["CSHARP_RENDERER_SETTINGS"]);
        let mut call = services.finish_call().expect("refused call");
        assert!(call.take_output().frames.is_empty());
    }

    #[test]
    fn bloom_and_auto_exposure_publish_as_retained_environment_and_refuse_invalid_values() {
        use crate::operation_diagnostics::{empty_receipt, receipt_codes};
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .expect("service set");
        let auto = |enabled, speed, min_exposure, max_exposure| NativeAutoExposureRequest {
            enabled,
            speed,
            min_exposure,
            max_exposure,
        };
        services.begin_call(binding());
        let api = services.api();
        let bloom = |request: NativeBloomRequest, refusal| unsafe {
            (api.camera_view.set_bloom)(api.camera_view.context, &request, refusal)
        };
        let exposure = |request: NativeAutoExposureRequest, refusal| unsafe {
            (api.camera_view.set_auto_exposure)(api.camera_view.context, &request, refusal)
        };
        let bright = NativeBloomRequest {
            threshold: 1.2,
            intensity: 0.6,
        };
        assert_eq!(bloom(bright, std::ptr::null_mut()), ABI_OK);
        assert_eq!(
            exposure(auto(true, 1.5, 0.25, 4.0), std::ptr::null_mut()),
            ABI_OK
        );
        let mut call = services.finish_call().expect("environment call");
        let selected = [
            render_model::RenderDiff::SetBloom {
                bloom: Some(render_model::BloomDescriptor {
                    threshold: 1.2,
                    intensity: 0.6,
                }),
            },
            render_model::RenderDiff::SetAutoExposure {
                auto_exposure: Some(render_model::AutoExposureDescriptor {
                    speed: 1.5,
                    min_exposure: 0.25,
                    max_exposure: 4.0,
                }),
            },
        ];
        assert_eq!(call.take_output().frames[0].ops, selected);
        let attachment = services
            .snapshot_outputs(binding())
            .expect("fresh attachment");
        let CsharpAppearanceCallOutput::Frame(frame) = &attachment.appearance[0] else {
            panic!("baseline graphics frame");
        };
        assert!(selected.iter().all(|op| frame.ops.contains(op)));

        // Invalid values are refused; zero intensity and disabled turn them
        // off.
        services.begin_call(binding());
        let api = services.api();
        let bloom = |request: NativeBloomRequest, refusal| unsafe {
            (api.camera_view.set_bloom)(api.camera_view.context, &request, refusal)
        };
        let exposure = |request: NativeAutoExposureRequest, refusal| unsafe {
            (api.camera_view.set_auto_exposure)(api.camera_view.context, &request, refusal)
        };
        let mut refusal = empty_receipt();
        assert_eq!(
            bloom(
                NativeBloomRequest {
                    threshold: -1.0,
                    intensity: 1.0,
                },
                &mut refusal
            ),
            0
        );
        assert_eq!(receipt_codes(&refusal), ["CSHARP_BLOOM"]);
        let mut refusal = empty_receipt();
        assert_eq!(exposure(auto(true, 1.0, 2.0, 1.0), &mut refusal), 0);
        assert_eq!(receipt_codes(&refusal), ["CSHARP_AUTO_EXPOSURE"]);
        assert_eq!(
            bloom(
                NativeBloomRequest {
                    threshold: 1.0,
                    intensity: 0.0,
                },
                std::ptr::null_mut()
            ),
            ABI_OK
        );
        assert_eq!(
            exposure(auto(false, 0.0, 0.0, 0.0), std::ptr::null_mut()),
            ABI_OK
        );
        let mut call = services.finish_call().expect("off");
        assert_eq!(
            call.take_output().frames[0].ops,
            [
                render_model::RenderDiff::SetBloom { bloom: None },
                render_model::RenderDiff::SetAutoExposure {
                    auto_exposure: None
                },
            ]
        );
    }

    #[test]
    fn colour_grading_publishes_as_retained_environment_and_neutral_turns_it_off() {
        use crate::operation_diagnostics::{empty_receipt, receipt_codes};
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .expect("service set");
        let request = |temperature, tint, contrast, saturation| NativeColorGradingRequest {
            temperature,
            tint,
            contrast,
            saturation,
        };
        services.begin_call(binding());
        let api = services.api();
        let grade = |request: NativeColorGradingRequest, refusal| unsafe {
            (api.camera_view.set_color_grading)(api.camera_view.context, &request, refusal)
        };
        assert_eq!(
            grade(request(-0.2, 0.05, 0.1, 0.15), std::ptr::null_mut()),
            ABI_OK
        );
        let mut call = services.finish_call().expect("grading call");
        let selected = render_model::RenderDiff::SetColorGrading {
            color_grading: Some(render_model::ColorGradingDescriptor {
                temperature: -0.2,
                tint: 0.05,
                contrast: 0.1,
                saturation: 0.15,
            }),
        };
        assert_eq!(
            call.take_output().frames[0].ops,
            std::slice::from_ref(&selected)
        );
        let attachment = services
            .snapshot_outputs(binding())
            .expect("fresh attachment");
        let CsharpAppearanceCallOutput::Frame(frame) = &attachment.appearance[0] else {
            panic!("baseline graphics frame");
        };
        assert!(frame.ops.contains(&selected));

        services.begin_call(binding());
        let api = services.api();
        let grade = |request: NativeColorGradingRequest, refusal| unsafe {
            (api.camera_view.set_color_grading)(api.camera_view.context, &request, refusal)
        };
        let mut refusal = empty_receipt();
        assert_eq!(grade(request(0.0, 0.0, 1.5, 0.0), &mut refusal), 0);
        assert_eq!(receipt_codes(&refusal), ["CSHARP_COLOR_GRADING"]);
        let mut refusal = empty_receipt();
        assert_eq!(grade(request(f32::NAN, 0.0, 0.0, 0.0), &mut refusal), 0);
        assert_eq!(receipt_codes(&refusal), ["CSHARP_COLOR_GRADING"]);
        assert_eq!(
            grade(request(0.0, 0.0, 0.0, 0.0), std::ptr::null_mut()),
            ABI_OK
        );
        let mut call = services.finish_call().expect("neutral");
        assert_eq!(
            call.take_output().frames[0].ops,
            [render_model::RenderDiff::SetColorGrading {
                color_grading: None
            }]
        );
    }

    #[test]
    fn fog_and_tone_mapping_publish_as_retained_environment_and_refuse_invalid_values() {
        use crate::operation_diagnostics::{empty_receipt, receipt_codes};
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
        )
        .expect("service set");
        let fog = |mode, start, end, density| NativeFogRequest {
            mode,
            color: NativeColor {
                r: 0.3,
                g: 0.35,
                b: 0.4,
                a: 1.0,
            },
            start,
            end,
            density,
        };

        services.begin_call(binding());
        let api = services.api();
        assert_eq!(
            unsafe {
                (api.camera_view.set_fog)(
                    api.camera_view.context,
                    &fog(NativeFogMode::Linear, 3.0, 30.0, 0.0),
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.camera_view.set_tone_mapping)(
                    api.camera_view.context,
                    &NativeToneMappingRequest {
                        operator: NativeToneMappingOperator::AcesFilmic,
                        exposure: 0.8,
                    },
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut call = services.finish_call().expect("environment call");
        let selected = [
            render_model::RenderDiff::SetFog {
                fog: Some(render_model::FogDescriptor::Linear {
                    color: [0.3, 0.35, 0.4],
                    start: 3.0,
                    end: 30.0,
                }),
            },
            render_model::RenderDiff::SetToneMapping {
                tone_mapping: render_model::ToneMappingDescriptor {
                    operator: render_model::ToneMappingOperator::AcesFilmic,
                    exposure: 0.8,
                },
            },
        ];
        assert_eq!(call.take_output().frames[0].ops, selected);

        // A fresh attachment's baseline carries the retained selections.
        let attachment = services
            .snapshot_outputs(binding())
            .expect("fresh attachment");
        let CsharpAppearanceCallOutput::Frame(frame) = &attachment.appearance[0] else {
            panic!("baseline graphics frame");
        };
        assert!(selected.iter().all(|op| frame.ops.contains(op)));

        // Invalid values are refused at the call, and nothing is staged.
        services.begin_call(binding());
        let api = services.api();
        for (request, code) in [
            (fog(NativeFogMode::Linear, 10.0, 5.0, 0.0), "CSHARP_FOG"),
            (fog(NativeFogMode::Exponential, 0.0, 0.0, 0.0), "CSHARP_FOG"),
        ] {
            let mut refusal = empty_receipt();
            assert_eq!(
                unsafe {
                    (api.camera_view.set_fog)(api.camera_view.context, &request, &mut refusal)
                },
                0
            );
            assert_eq!(receipt_codes(&refusal), [code]);
        }
        let mut refusal = empty_receipt();
        assert_eq!(
            unsafe {
                (api.camera_view.set_tone_mapping)(
                    api.camera_view.context,
                    &NativeToneMappingRequest {
                        operator: NativeToneMappingOperator::Neutral,
                        exposure: f32::NAN,
                    },
                    &mut refusal,
                )
            },
            0
        );
        assert_eq!(receipt_codes(&refusal), ["CSHARP_TONE_MAPPING"]);
        assert_eq!(
            unsafe {
                (api.camera_view.set_fog)(
                    api.camera_view.context,
                    &fog(NativeFogMode::Off, 0.0, 0.0, 0.0),
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut call = services.finish_call().expect("fog off");
        assert_eq!(
            call.take_output().frames[0].ops,
            [render_model::RenderDiff::SetFog { fog: None }]
        );
    }

    #[test]
    fn spatial_mutation_survives_later_call_failure_and_outer_discard() {
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            BTreeMap::new(),
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
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
                    std::ptr::null_mut(),
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
                    std::ptr::null_mut(),
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
                    std::ptr::null_mut(),
                )
            },
            0,
            "the later generated call would throw and fail the product callback"
        );
        let _ = services.finish_call();

        let api = services.api();
        let mut after_discard = NativeNavigationProjectionReadout::default();
        assert_eq!(
            unsafe {
                (api.spatial.read_navigation_projection)(
                    api.spatial.context,
                    NativeNavigationProjectionReadRequest { session },
                    &mut after_discard,
                    std::ptr::null_mut(),
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
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(retry.navigation_revision, 2);
        services.finish_call().expect("unrelated staged families");
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
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
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
                    std::ptr::null_mut(),
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
                (api.content.open_reference)(
                    api.content.context,
                    &valid_open,
                    &mut valid_reference,
                    std::ptr::null_mut(),
                )
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
        let mut error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.spatial.replace_content_artifact)(
                    api.spatial.context,
                    &request,
                    &mut receipt,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.content_reference_value, valid_reference.value);
        assert_eq!(receipt.collision_vertex_count, 3);
        assert_eq!(receipt.collision_triangle_count, 1);
        assert_eq!(receipt.navigation_cell_count, 3);

        let mut step = NativeNavigationStepResult::default();
        assert_eq!(
            unsafe {
                (api.spatial.evaluate_navigation_step)(
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
                    std::ptr::null_mut(),
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
                    std::ptr::null_mut(),
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
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let scene_before = Arc::clone(&services.spatial.sessions[&session.value].scene);
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
                    &mut error,
                )
            },
            0
        );
        assert_spatial_admission_diagnostic(error, "CSHARP_SPATIAL_CONTENT_COLLISION");
        let mut after_rejection = NativeSpatialContentArtifactReadout::default();
        assert_eq!(
            unsafe {
                (api.spatial.read_content_artifact)(
                    api.spatial.context,
                    NativeSpatialContentArtifactReadRequest { session },
                    &mut after_rejection,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(after_rejection, readout);
        assert!(
            Arc::ptr_eq(
                &scene_before,
                &services.spatial.sessions[&session.value].scene
            ),
            "refusal must preserve the exact scene, including collision and residency"
        );

        assert_eq!(
            unsafe { (api.content.destroy_reference)(api.content.context, valid_reference) },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.spatial.replace_content_artifact)(
                    api.spatial.context,
                    &request,
                    &mut rejected,
                    &mut error,
                )
            },
            0,
            "a stale Content reference was accepted"
        );
        assert_spatial_admission_diagnostic(error, "CSHARP_SPATIAL_CONTENT_REFERENCE");
        let _ = services.finish_call();
    }
    /// An admitted artifact's navigation, like its collision, follows a
    /// world-origin commit by a non-whole number of its 0.8 m cells, and a
    /// replacement admitted afterwards keeps the committed origin.
    #[test]
    fn content_artifact_navigation_follows_world_origin_commits() {
        let path = "spatial/example/collision-navigation.json";
        let artifact = br#"{
            "schemaVersion":1,
            "staticMeshArtifactId":"mesh/example",
            "bounds":{"min":[0.0,0.0,0.0],"max":[3.0,20.0,2.0]},
            "collision":{"positions":[[0.0,0.0,0.0],[2.0,0.0,0.0],[0.0,0.0,2.0]],"triangles":[[0,1,2]]},
            "navigation":{"id":"navigation/example","config":{"schemaVersion":1,"cellSize":0.8,"levelQuantum":0.25,"maximumSlopeDegrees":45.0,"requiredHeadroom":1.0,"supportProbeDrop":0.1},"cells":[{"column":0,"row":0,"level":51,"supportHeight":12.8,"walkable":true},{"column":1,"row":0,"level":51,"supportHeight":12.8,"walkable":true},{"column":2,"row":0,"level":51,"supportHeight":12.8,"walkable":true}]}
        }"#;
        let mut content = BTreeMap::new();
        content.insert(path.to_owned(), Arc::<[u8]>::from(artifact.as_slice()));
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            content,
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
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
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut reference = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                (api.content.open_reference)(
                    api.content.context,
                    &NativeContentOpenRequest {
                        path: NativeUtf8Slice {
                            bytes: path.as_ptr(),
                            len: path.len(),
                        },
                    },
                    &mut reference,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let replace = || {
            let mut receipt = NativeSpatialContentArtifactReplaceReceipt::default();
            let mut error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            assert_eq!(
                unsafe {
                    (api.spatial.replace_content_artifact)(
                        api.spatial.context,
                        &NativeSpatialContentArtifactReplaceRequest {
                            session,
                            content: reference,
                            navigation_grid_id: 7,
                            navigation_chunk_size: 8,
                            navigation_max_step_cells: 1,
                        },
                        &mut receipt,
                        &mut error,
                    )
                },
                ABI_OK
            );
        };
        let step = |from: [f32; 3], target: [f32; 3]| {
            let vec3 = |[x, y, z]: [f32; 3]| NativeVec3 { x, y, z };
            let mut step = NativeNavigationStepResult::default();
            assert_eq!(
                unsafe {
                    (api.spatial.evaluate_navigation_step)(
                        api.spatial.context,
                        NativeNavigationStepRequest {
                            session,
                            from: vec3(from),
                            target: vec3(target),
                            max_step_units: 0.5,
                            max_visited: 32,
                        },
                        &mut step,
                        std::ptr::null_mut(),
                    )
                },
                ABI_OK
            );
            step
        };
        let ray_distance = |services: &EngineServiceSet, from: [f32; 3]| {
            let hit = services.spatial.sessions[&session.value]
                .scene
                .raycast_world(from.map(f64::from), [0.0, -1.0, 0.0], 50.0);
            match hit {
                Some(engine_spatial::SpatialCollisionHit::StaticMesh(hit)) => Some(hit.distance),
                _ => None,
            }
        };
        let moved =
            |point: [f32; 3], delta: [f32; 3]| [0, 1, 2].map(|axis| point[axis] + delta[axis]);
        let (from, target, above) = ([0.4, 12.8, 0.4], [2.0, 12.8, 0.4], [0.5, 5.0, 0.5]);

        replace();
        let before = step(from, target);
        assert_eq!(before.outcome, NativeNavigationPathOutcome::Reached);
        let distance = ray_distance(&services, above).expect("the artifact collision is resident");

        let mut prepared = NativeWorldOriginPreparedHandle::default();
        assert_eq!(
            unsafe {
                (api.world_origin.prepare)(
                    api.world_origin.context,
                    &NativeWorldOriginPrepareRequest {
                        session,
                        target_cell_x: 1_000,
                        target_cell_y: 3,
                        target_cell_z: 5,
                        entities: std::ptr::null(),
                        entities_len: 0,
                    },
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut commit = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (api.world_origin.commit)(
                    api.world_origin.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut commit,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let delta = [-1_000.0, -3.0, -5.0];

        let after = step(moved(from, delta), moved(target, delta));
        assert_eq!(after.outcome, NativeNavigationPathOutcome::Reached);
        assert_eq!(
            (
                after.next_path_cell.x,
                after.next_path_cell.y,
                after.next_path_cell.z
            ),
            (
                before.next_path_cell.x,
                before.next_path_cell.y,
                before.next_path_cell.z
            )
        );
        let expected = moved(
            [
                before.next_waypoint.x,
                before.next_waypoint.y,
                before.next_waypoint.z,
            ],
            delta,
        );
        assert!(
            (after.next_waypoint.x - expected[0]).abs() < 1.0e-3
                && (after.next_waypoint.y - expected[1]).abs() < 1.0e-3
                && (after.next_waypoint.z - expected[2]).abs() < 1.0e-3,
            "waypoint {:?} is not {expected:?}",
            after.next_waypoint
        );
        assert_eq!(
            step(from, target).outcome,
            NativeNavigationPathOutcome::StartNotWalkable,
            "the pre-commit local position still navigated"
        );
        assert_eq!(ray_distance(&services, moved(above, delta)), Some(distance));

        // A whole-content replacement keeps the committed origin and admits
        // the artifact's collision and navigation together in the current frame.
        replace();
        let mut origin = NativeWorldOriginReadout::default();
        assert_eq!(
            unsafe {
                (api.world_origin.read)(
                    api.world_origin.context,
                    NativeWorldOriginReadRequest { session },
                    &mut origin,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!((origin.cell_x, origin.cell_y, origin.cell_z), (1_000, 3, 5));
        assert_eq!(
            step(from, target).outcome,
            NativeNavigationPathOutcome::Reached
        );
        assert_eq!(ray_distance(&services, above), Some(distance));
        let _ = services.finish_call();
    }

    /// Placed spatial artifacts compose with each other, the session's own
    /// static collision and world-origin commits, and each comes and goes
    /// under its identity without touching the rest.
    #[test]
    fn placed_content_artifacts_compose_and_unload_independently() {
        // A strip of `columns` walkable 1 m cells at height 0, floored by two
        // collision triangles.
        let strip = |name: &str, columns: usize| {
            let cells: Vec<String> = (0..columns)
                .map(|column| {
                    format!(r#"{{"column":{column},"row":0,"level":0,"supportHeight":0.0,"walkable":true}}"#)
                })
                .collect();
            format!(
                r#"{{"schemaVersion":1,"staticMeshArtifactId":"mesh/{name}","bounds":{{"min":[0.0,-1.0,0.0],"max":[{columns}.0,1.0,1.0]}},"collision":{{"positions":[[0.0,0.0,0.0],[{columns}.0,0.0,0.0],[0.0,0.0,1.0],[{columns}.0,0.0,1.0]],"triangles":[[0,1,2],[1,3,2]]}},"navigation":{{"id":"navigation/{name}","config":{{"schemaVersion":1,"cellSize":1.0,"levelQuantum":0.25,"maximumSlopeDegrees":45.0,"requiredHeadroom":1.0,"supportProbeDrop":0.1}},"cells":[{}]}}}}"#,
                cells.join(",")
            )
        };
        let mut content = BTreeMap::new();
        for (path, bytes) in [
            ("spatial/west.json", strip("west", 3)),
            ("spatial/east.json", strip("east", 2)),
            ("spatial/broken.json", "{\"schemaVersion\":1}".to_owned()),
        ] {
            content.insert(path.to_owned(), Arc::<[u8]>::from(bytes.as_bytes()));
        }
        let mut services = EngineServiceSet::new(
            parse_runtime_appearance_catalog(None).expect("default catalog"),
            content,
            None,
            RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
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
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let open = |path: &str| {
            let mut reference = NativeContentReferenceHandle::default();
            assert_eq!(
                unsafe {
                    (api.content.open_reference)(
                        api.content.context,
                        &NativeContentOpenRequest {
                            path: NativeUtf8Slice {
                                bytes: path.as_ptr(),
                                len: path.len(),
                            },
                        },
                        &mut reference,
                        std::ptr::null_mut(),
                    )
                },
                ABI_OK
            );
            reference
        };
        let (west, east, broken) = (
            open("spatial/west.json"),
            open("spatial/east.json"),
            open("spatial/broken.json"),
        );
        let place = |id, content, column_offset| NativeSpatialContentArtifactInstance {
            id,
            content,
            column_offset,
            level_offset: 0,
            row_offset: 0,
            quarter_turns: 0,
            translation: NativeVec3::default(),
        };
        let translated =
            |id, content, column_offset, translation| NativeSpatialContentArtifactInstance {
                translation,
                ..place(id, content, column_offset)
            };
        let residency = |admitted: &[NativeSpatialContentArtifactInstance], removed: &[u64]| {
            let mut receipt = NativeSpatialContentArtifactResidencyReceipt::default();
            let mut error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            let status = unsafe {
                (api.spatial.apply_content_artifact_residency)(
                    api.spatial.context,
                    &NativeSpatialContentArtifactResidencyRequest {
                        session,
                        admitted: admitted.as_ptr(),
                        admitted_len: admitted.len(),
                        removed: removed.as_ptr(),
                        removed_len: removed.len(),
                        navigation_grid_id: 7,
                        navigation_chunk_size: 8,
                        navigation_max_step_cells: 1,
                    },
                    &mut receipt,
                    &mut error,
                )
            };
            if status == ABI_OK {
                Ok(receipt)
            } else {
                assert_eq!(error.diagnostics_len, 1);
                let diagnostic = unsafe { *error.diagnostics };
                Err(unsafe {
                    std::str::from_utf8(std::slice::from_raw_parts(
                        diagnostic.code.bytes,
                        diagnostic.code.len,
                    ))
                    .unwrap()
                    .to_owned()
                })
            }
        };
        let step = |from: [f32; 3], target: [f32; 3]| {
            let vec3 = |[x, y, z]: [f32; 3]| NativeVec3 { x, y, z };
            let mut step = NativeNavigationStepResult::default();
            assert_eq!(
                unsafe {
                    (api.spatial.evaluate_navigation_step)(
                        api.spatial.context,
                        NativeNavigationStepRequest {
                            session,
                            from: vec3(from),
                            target: vec3(target),
                            max_step_units: 0.5,
                            max_visited: 64,
                        },
                        &mut step,
                        std::ptr::null_mut(),
                    )
                },
                ABI_OK
            );
            step
        };
        let floor_height = |services: &EngineServiceSet, [x, z]: [f32; 2]| match services
            .spatial
            .sessions[&session.value]
            .scene
            .raycast_world([f64::from(x), 5.0, f64::from(z)], [0.0, -1.0, 0.0], 50.0)
        {
            Some(engine_spatial::SpatialCollisionHit::StaticMesh(hit)) => Some(hit.point.y),
            _ => None,
        };
        let floor_below =
            |services: &EngineServiceSet, point| floor_height(services, point).is_some();
        let has_instance = |services: &EngineServiceSet, id: u64| {
            services.spatial.sessions[&session.value]
                .scene
                .static_mesh_instance(engine_spatial::StaticMeshInstanceId(id))
                .is_some()
        };

        // The session's own static collision: a terrain tile far away.
        let terrain_vertices = [
            NativeVec3 {
                x: 20.0,
                y: 0.0,
                z: 0.0,
            },
            NativeVec3 {
                x: 22.0,
                y: 0.0,
                z: 0.0,
            },
            NativeVec3 {
                x: 20.0,
                y: 0.0,
                z: 2.0,
            },
        ];
        let terrain_triangle = [NativeTriangle { a: 0, b: 1, c: 2 }];
        let terrain_asset = [NativeStaticMeshAsset {
            id: 500,
            mesh_resource: NativeMeshResourceReference { value: 0 },
            first_vertex: 0,
            vertex_count: 3,
            first_triangle: 0,
            triangle_count: 1,
        }];
        let identity = NativeTransform {
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
        };
        let terrain_instance = [NativeStaticMeshInstance {
            id: 500,
            asset: 500,
            transform: identity,
        }];
        let collision_residency = |removed_instances: &[u64]| {
            let mut receipt = NativeCollisionReplaceReceipt::default();
            let mut error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            let adding = removed_instances.is_empty();
            unsafe {
                (api.spatial.apply_collision_residency)(
                    api.spatial.context,
                    &NativeCollisionResidencyRequest {
                        session,
                        assets: terrain_asset.as_ptr(),
                        assets_len: usize::from(adding),
                        vertices: terrain_vertices.as_ptr(),
                        vertices_len: terrain_vertices.len(),
                        triangles: terrain_triangle.as_ptr(),
                        triangles_len: terrain_triangle.len(),
                        instances: terrain_instance.as_ptr(),
                        instances_len: usize::from(adding),
                        removed_assets: std::ptr::null(),
                        removed_assets_len: 0,
                        removed_instances: removed_instances.as_ptr(),
                        removed_instances_len: removed_instances.len(),
                    },
                    &mut receipt,
                    &mut error,
                )
            }
        };
        assert_eq!(collision_residency(&[]), ABI_OK);

        // Two closures side by side: one navigation across their boundary.
        let translated_west = NativeVec3 {
            x: 0.25,
            y: 0.375,
            z: 0.25,
        };
        let translated_east = NativeVec3 {
            x: 0.1,
            y: 0.125,
            z: 0.2,
        };
        let receipt = residency(
            &[
                translated(1, west, 0, translated_west),
                translated(2, east, 3, translated_east),
            ],
            &[],
        )
        .unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (2, 5)
        );
        let across = step([0.5, 0.0, 0.5], [4.5, 0.0, 0.5]);
        assert_eq!(across.outcome, NativeNavigationPathOutcome::Reached);
        let expected_waypoint_y =
            0.375 * 0.5 / (1.25f32.powi(2) + 0.375f32.powi(2) + 0.25f32.powi(2)).sqrt();
        assert!((across.next_waypoint.y - expected_waypoint_y).abs() < 1.0e-3);
        assert!((floor_height(&services, [1.5, 0.5]).unwrap() - 0.375).abs() < 1.0e-9);
        assert!((floor_height(&services, [4.5, 0.5]).unwrap() - 0.125).abs() < 1.0e-9);
        assert!(floor_below(&services, [1.5, 0.5]) && floor_below(&services, [4.5, 0.5]));
        assert!(
            has_instance(&services, 500),
            "the session's own collision stays"
        );
        // The translated west artifact begins at x=.25. A position in its
        // old integer cell but outside that translated footprint must fail
        // both collision and navigation instead of falling back to cell 0.
        assert!(floor_height(&services, [0.05, 0.5]).is_none());
        assert_eq!(
            step([0.05, 0.0, 0.5], [1.5, 0.0, 0.5]).outcome,
            NativeNavigationPathOutcome::StartNotWalkable
        );

        // Conflicts and malformed artifacts refuse and change nothing.
        let revision = across.navigation_revision;
        assert_eq!(
            residency(&[place(1, west, 6)], &[]).unwrap_err(),
            "CSHARP_SPATIAL_CONTENT_IDENTITY"
        );
        assert_eq!(
            residency(&[place(500, west, 6)], &[]).unwrap_err(),
            "CSHARP_SPATIAL_CONTENT_IDENTITY"
        );
        assert_eq!(
            residency(&[place(3, west, 6), place(4, broken, 9)], &[]).unwrap_err(),
            "CSHARP_SPATIAL_CONTENT_SCHEMA"
        );
        let malformed_translation = translated(
            3,
            west,
            6,
            NativeVec3 {
                x: f32::NAN,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_eq!(
            residency(&[malformed_translation], &[]).unwrap_err(),
            "CSHARP_SPATIAL_CONTENT_TRANSFORM"
        );
        assert_ne!(
            collision_residency(&[1]),
            ABI_OK,
            "a placed artifact's collision is its own"
        );
        let unchanged = step([0.5, 0.0, 0.5], [4.5, 0.0, 0.5]);
        assert_eq!(unchanged.navigation_revision, revision);
        assert!(!has_instance(&services, 3) && has_instance(&services, 1));

        // Unload one closure; the other and the terrain stay.
        let receipt = residency(&[], &[1]).unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (1, 2)
        );
        assert!(
            !has_instance(&services, 1)
                && has_instance(&services, 2)
                && has_instance(&services, 500)
        );
        assert!(!floor_below(&services, [1.5, 0.5]) && floor_below(&services, [4.5, 0.5]));
        assert_eq!(
            step([0.5, 0.0, 0.5], [4.5, 0.0, 0.5]).outcome,
            NativeNavigationPathOutcome::StartNotWalkable
        );
        // Reload it under the same identity.
        let receipt = residency(&[translated(1, west, 0, translated_west)], &[]).unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (2, 5)
        );
        assert!(floor_below(&services, [1.5, 0.5]));

        // A quarter turn takes the east strip from +X to -Z: its cells run
        // north from the west strip's first one, and so does its floor.
        let turned = NativeSpatialContentArtifactInstance {
            quarter_turns: 1,
            ..place(9, east, 0)
        };
        let receipt = residency(&[turned], &[]).unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (3, 7)
        );
        assert_eq!(
            step([0.5, 0.0, 0.5], [0.5, 0.0, -1.5]).outcome,
            NativeNavigationPathOutcome::Reached
        );
        assert!(floor_below(&services, [0.5, -1.5]) && !floor_below(&services, [1.5, -0.5]));
        let receipt = residency(&[], &[9]).unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (2, 5)
        );

        // A world-origin commit moves every closure; a later placement lands
        // on the same grid.
        let mut prepared = NativeWorldOriginPreparedHandle::default();
        assert_eq!(
            unsafe {
                (api.world_origin.prepare)(
                    api.world_origin.context,
                    &NativeWorldOriginPrepareRequest {
                        session,
                        target_cell_x: 10,
                        target_cell_y: 0,
                        target_cell_z: 0,
                        entities: std::ptr::null(),
                        entities_len: 0,
                    },
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut commit = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (api.world_origin.commit)(
                    api.world_origin.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut commit,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            step([-9.5, 0.0, 0.5], [-5.5, 0.0, 0.5]).outcome,
            NativeNavigationPathOutcome::Reached
        );
        assert!(floor_below(&services, [-5.5, 0.5]));
        assert!((floor_height(&services, [-5.5, 0.5]).unwrap() - 0.125).abs() < 1.0e-9);
        let receipt = residency(&[place(4, east, 5)], &[]).unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (3, 7)
        );
        assert_eq!(
            step([-9.5, 0.0, 0.5], [-3.5, 0.0, 0.5]).outcome,
            NativeNavigationPathOutcome::Reached
        );
        assert!(floor_below(&services, [-3.5, 0.5]));

        let receipt = residency(&[], &[1, 2, 4]).unwrap();
        assert_eq!(
            (receipt.instance_count, receipt.navigation_cell_count),
            (0, 0)
        );
        assert!(has_instance(&services, 500));
        // Unloading every closure keeps the rebased grid for the next one.
        residency(&[translated(1, west, 0, translated_west)], &[]).unwrap();
        assert!(floor_below(&services, [-8.5, 0.5]) && !floor_below(&services, [1.5, 0.5]));
        assert!((floor_height(&services, [-8.5, 0.5]).unwrap() - 0.375).abs() < 1.0e-9);
        assert_eq!(
            step([-9.5, 0.0, 0.5], [-8.5, 0.0, 0.5]).outcome,
            NativeNavigationPathOutcome::Reached
        );
        let _ = services.finish_call();
    }

    fn assert_spatial_admission_diagnostic(error: NativeOperationErrorReceipt, expected: &str) {
        assert_eq!(error.diagnostics_len, 1);
        let diagnostic = unsafe { *error.diagnostics };
        let text = |value: NativeUtf8Slice| unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(value.bytes, value.len)).unwrap()
        };
        assert_eq!(text(diagnostic.code), expected);
        assert!(!text(diagnostic.message).is_empty());
    }
}
