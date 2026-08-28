//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.
//!
//! This crate owns the callback contexts and their staged Engine state. The
//! runtime crate only composes this service family with a loaded product.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use core_math::Vec3;
use csharp_engine_abi::*;
use entity_state::Quat;

use crate::{
    audio::{RuntimeAudioBridge, RuntimeAudioCall},
    camera_view::RuntimeCameraViewBridge,
    content::RuntimeContentBridge,
    dynamics::RuntimeDynamicsBridge,
    mechanics::RuntimeMechanicsBridge,
    persistence::RuntimePersistenceBridge,
    resolution::RuntimeResolutionBridge,
    rng::RuntimeRngBridge,
    rules::RuntimeRulesBridge,
    spatial::RuntimeSpatialBridge,
    standard_continuous::RuntimeStandardContinuousBridge,
    standard_exact::RuntimeStandardExactBridge,
    state_machine::RuntimeStateMachineBridge,
    ui::{RuntimeUiBridge, RuntimeUiCall},
    voxel_content::RuntimeVoxelContentBridge,
};
use render_projection::RuntimeAppearanceCatalog;
use runtime_ui::RuntimeUiProjectionEnvelope;

pub(crate) const ABI_OK: i32 = 1;
use crate::appearance::{
    create_light, create_material, create_primitive_appearance, create_sprite_appearance,
    create_static_mesh_appearance, create_static_mesh_from_content_appearance, destroy_appearance,
    destroy_light, destroy_material, open_render_resource, publish_appearance_snapshot, read_light,
    read_presentation, replace_light, replace_material, replace_primitive_appearance,
    replace_sprite_appearance, replace_static_mesh_appearance,
    replace_static_mesh_from_content_appearance, update_light, update_material,
    update_static_mesh_materials, CsharpRenderResource, RuntimeAppearanceBridge,
    RuntimeAppearanceCall,
};

fn engine_api(
    appearance_bridge: &mut RuntimeAppearanceBridge,
    content_bridge: &mut RuntimeContentBridge,
    content_store_bridge: &mut crate::content_store::RuntimeContentStoreBridge,
    audio_bridge: &mut RuntimeAudioBridge,
    camera_view_bridge: &mut RuntimeCameraViewBridge,
    dynamics_bridge: &mut RuntimeDynamicsBridge,
    spatial_bridge: &mut RuntimeSpatialBridge,
    voxel_content_bridge: &mut RuntimeVoxelContentBridge,
    rng_bridge: &mut RuntimeRngBridge,
    mechanics_bridge: &mut RuntimeMechanicsBridge,
    persistence_bridge: &mut RuntimePersistenceBridge,
    rules_bridge: &mut RuntimeRulesBridge,
    resolution_bridge: &mut RuntimeResolutionBridge,
    state_machine_bridge: &mut RuntimeStateMachineBridge,
    standard_exact_bridge: &mut RuntimeStandardExactBridge,
    standard_continuous_bridge: &mut RuntimeStandardContinuousBridge,
    ui_bridge: &mut RuntimeUiBridge,
) -> NativeEngineApi {
    NativeEngineApi {
        look: crate::look::api(),
        dynamics: crate::dynamics::api(dynamics_bridge),
        spatial: crate::spatial::api(spatial_bridge),
        voxel: crate::voxel::api(spatial_bridge),
        voxel_content: crate::voxel_content::api(voxel_content_bridge, appearance_bridge),
        content: crate::content::api(content_bridge),
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
            destroy_appearance,
            publish_snapshot: publish_appearance_snapshot,
            create_light,
            update_light,
            replace_light,
            destroy_light,
            read_light,
            read_presentation,
        },
        presentation: NativePresentationApi {
            context: (appearance_bridge as *mut RuntimeAppearanceBridge).cast(),
            create_billboard: crate::presentation::create_billboard,
            update_billboard: crate::presentation::update_billboard,
            destroy_billboard: crate::presentation::destroy_billboard,
            emit_particles: crate::presentation::emit_particles,
            create_emitter: crate::presentation::create_emitter,
            update_emitter: crate::presentation::update_emitter,
            destroy_emitter: crate::presentation::destroy_emitter,
            read: crate::presentation::read,
            read_diagnostic_at: crate::presentation::read_diagnostic_at,
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
        mechanics: crate::mechanics::api(mechanics_bridge),
        continuous_mechanics: crate::mechanics::continuous::api(mechanics_bridge),
        persistence: crate::persistence::api(persistence_bridge),
        rules: crate::rules::api(rules_bridge),
        resolution: crate::resolution::api(resolution_bridge),
        state_machine: crate::state_machine::api(state_machine_bridge),
        standard_exact: crate::standard_exact::api(standard_exact_bridge),
        standard_continuous: crate::standard_continuous::api(standard_continuous_bridge),
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
    appearance: RuntimeAppearanceBridge,
    content: RuntimeContentBridge,
    content_store: crate::content_store::RuntimeContentStoreBridge,
    audio: RuntimeAudioBridge,
    camera_view: RuntimeCameraViewBridge,
    dynamics: RuntimeDynamicsBridge,
    spatial: RuntimeSpatialBridge,
    voxel_content: RuntimeVoxelContentBridge,
    rng: RuntimeRngBridge,
    mechanics: RuntimeMechanicsBridge,
    persistence: RuntimePersistenceBridge,
    rules: RuntimeRulesBridge,
    resolution: RuntimeResolutionBridge,
    state_machine: RuntimeStateMachineBridge,
    standard_exact: RuntimeStandardExactBridge,
    standard_continuous: RuntimeStandardContinuousBridge,
    ui: RuntimeUiBridge,
}

pub struct CsharpEngineCall {
    appearance: Option<RuntimeAppearanceCall>,
    audio: RuntimeAudioCall,
    camera_view: crate::camera_view::RuntimeCameraViewCall,
    sky_frame: Option<render_model::RenderFrameDiff>,
    ui: RuntimeUiCall,
    voxel_content: crate::voxel_content::RuntimeVoxelContentCall,
}

/// Staged Engine observations from one successful product call.
pub struct CsharpEngineCallOutput {
    pub frames: Vec<render_model::RenderFrameDiff>,
    pub view_composition: Option<render_host_contracts::RendererViewComposition>,
    pub ui: Vec<RuntimeUiProjectionEnvelope>,
    pub presentation: Vec<render_presentation::PresentationFrameDiff>,
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
    ) -> Result<Self, CsharpEngineServicesError> {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let dynamics = crate::dynamics::RuntimeDynamicsBridge::new(spatial.collision_source());
        Ok(Self {
            appearance: crate::appearance::create(catalog.0, content_resources.clone()),
            content: RuntimeContentBridge::new(content_resources.clone()),
            content_store: crate::content_store::RuntimeContentStoreBridge::new(
                content_store_root,
            )?,
            audio: RuntimeAudioBridge::new(content_resources),
            camera_view: RuntimeCameraViewBridge::new(),
            dynamics,
            spatial,
            voxel_content: RuntimeVoxelContentBridge::new(),
            rng: crate::rng::RuntimeRngBridge::new(),
            mechanics: crate::mechanics::RuntimeMechanicsBridge::new(),
            persistence: crate::persistence::RuntimePersistenceBridge::new(persistence_root),
            rules: crate::rules::RuntimeRulesBridge::new(),
            resolution: crate::resolution::RuntimeResolutionBridge::new(),
            state_machine: crate::state_machine::RuntimeStateMachineBridge::new(),
            standard_exact: crate::standard_exact::RuntimeStandardExactBridge::new(),
            standard_continuous: crate::standard_continuous::RuntimeStandardContinuousBridge::new(),
            ui: crate::ui::RuntimeUiBridge::new(),
        })
    }

    pub fn api(&mut self) -> NativeEngineApi {
        engine_api(
            &mut self.appearance,
            &mut self.content,
            &mut self.content_store,
            &mut self.audio,
            &mut self.camera_view,
            &mut self.dynamics,
            &mut self.spatial,
            &mut self.voxel_content,
            &mut self.rng,
            &mut self.mechanics,
            &mut self.persistence,
            &mut self.rules,
            &mut self.resolution,
            &mut self.state_machine,
            &mut self.standard_exact,
            &mut self.standard_continuous,
            &mut self.ui,
        )
    }

    pub fn begin_call(&mut self) {
        self.appearance.begin_call();
        self.audio.begin_call();
        self.camera_view.begin_call();
        self.ui.begin_call();
        self.voxel_content.begin_call();
    }

    pub fn discard_call(&mut self) {
        self.appearance.discard_call();
        self.audio.discard_call();
        self.camera_view.discard_call();
        self.ui.discard_call();
        self.voxel_content.discard_call();
    }

    pub fn take_call(&mut self) -> Result<CsharpEngineCall, CsharpEngineServicesError> {
        let appearance = self.appearance.take_staged_call()?;
        let audio = self.audio.take_staged_call()?;
        let camera_view = self.camera_view.take_staged_call()?;
        // Sky resources are owned and admitted by Appearance. Resolve the
        // cross-family handle while both staged states are available, before
        // either state can be committed or turned into host output.
        let sky_frame =
            crate::camera_view::sky_frame(camera_view.sky_texture, appearance.as_ref())?;
        let ui = self.ui.take_staged_call()?;
        let voxel_content = self.voxel_content.take_staged_call()?;
        Ok(CsharpEngineCall {
            appearance,
            audio,
            camera_view,
            sky_frame,
            ui,
            voxel_content,
        })
    }

    pub fn commit_call(&mut self, call: CsharpEngineCall) {
        self.appearance.commit(call.appearance);
        self.audio.commit(call.audio);
        self.camera_view.commit(call.camera_view);
        self.ui.commit(call.ui);
        self.voxel_content.commit_call(call.voxel_content);
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
        let mut frames = call
            .appearance
            .as_ref()
            .map(|call| {
                call.frame
                    .clone()
                    .into_iter()
                    .chain(call.extra_frames.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(frame) = call.sky_frame.clone() {
            frames.push(frame);
        }
        frames.extend(call.voxel_content.frames.clone());
        CsharpEngineCallOutput {
            frames,
            view_composition: call.camera_view.composition.clone(),
            ui: call.ui.projections.clone(),
            presentation: call
                .appearance
                .as_ref()
                .map(|call| call.presentation.clone())
                .unwrap_or_default()
                .into_iter()
                .chain(call.audio.frame.clone())
                .collect(),
        }
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
