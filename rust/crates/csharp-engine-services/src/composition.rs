//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.
//!
//! This crate owns the callback contexts and their staged Engine state. The
//! runtime crate only composes this service family with a loaded product.

use std::{collections::BTreeMap, sync::Arc};

use core_math::Vec3;
use csharp_engine_abi::*;
use entity_state::Quat;

use crate::{
    rng::RuntimeRngBridge,
    spatial::RuntimeSpatialBridge,
    ui::{RuntimeUiBridge, RuntimeUiCall},
};
use render_projection::RuntimeAppearanceCatalog;
use runtime_ui::RuntimeUiProjectionEnvelope;

pub(crate) const ABI_OK: i32 = 1;
use crate::appearance::{
    create_primitive_appearance, create_sprite_appearance, create_static_mesh_appearance,
    create_static_mesh_from_content_appearance, open_render_resource, publish_appearance_snapshot,
    CsharpRenderResource, RuntimeAppearanceBridge, RuntimeAppearanceCall,
};

fn engine_api(
    appearance_bridge: &mut RuntimeAppearanceBridge,
    spatial_bridge: &mut RuntimeSpatialBridge,
    rng_bridge: &mut RuntimeRngBridge,
    ui_bridge: &mut RuntimeUiBridge,
) -> NativeEngineApi {
    NativeEngineApi {
        look: crate::look::api(),
        spatial: crate::spatial::api(spatial_bridge),
        appearance: NativeAppearanceApi {
            context: (appearance_bridge as *mut RuntimeAppearanceBridge).cast(),
            open_resource: open_render_resource,
            create_primitive: create_primitive_appearance,
            create_static_mesh: create_static_mesh_appearance,
            create_static_mesh_from_content: create_static_mesh_from_content_appearance,
            create_sprite: create_sprite_appearance,
            publish_snapshot: publish_appearance_snapshot,
        },
        rng: crate::rng::api(rng_bridge),
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
    spatial: RuntimeSpatialBridge,
    rng: RuntimeRngBridge,
    ui: RuntimeUiBridge,
}

pub struct CsharpEngineCall {
    appearance: Option<RuntimeAppearanceCall>,
    ui: RuntimeUiCall,
}

/// Staged Engine observations from one successful product call.
pub struct CsharpEngineCallOutput {
    pub frame: Option<render_model::RenderFrameDiff>,
    pub ui: Vec<RuntimeUiProjectionEnvelope>,
}

/// Parsed optional appearance catalog retained with admitted product content.
/// The catalog remains an Engine presentation detail rather than a runtime-host
/// dependency.
pub struct CsharpAppearanceCatalog(RuntimeAppearanceCatalog);

impl EngineServiceSet {
    pub fn new(
        catalog: CsharpAppearanceCatalog,
        content_resources: BTreeMap<String, Arc<[u8]>>,
    ) -> Self {
        Self {
            appearance: crate::appearance::create(catalog.0, content_resources),
            spatial: crate::spatial::RuntimeSpatialBridge::new(),
            rng: crate::rng::RuntimeRngBridge::new(),
            ui: crate::ui::RuntimeUiBridge::new(),
        }
    }

    pub fn api(&mut self) -> NativeEngineApi {
        engine_api(
            &mut self.appearance,
            &mut self.spatial,
            &mut self.rng,
            &mut self.ui,
        )
    }

    pub fn begin_call(&mut self) {
        self.appearance.begin_call();
        self.ui.begin_call();
    }

    pub fn discard_call(&mut self) {
        self.appearance.discard_call();
        self.ui.discard_call();
    }

    pub fn take_call(&mut self) -> Result<CsharpEngineCall, CsharpEngineServicesError> {
        let appearance = self.appearance.take_staged_call()?;
        let ui = self.ui.take_staged_call()?;
        Ok(CsharpEngineCall { appearance, ui })
    }

    pub fn commit_call(&mut self, call: CsharpEngineCall) {
        self.appearance.commit(call.appearance);
        self.ui.commit(call.ui);
    }

    pub fn seal_resource_selection(&mut self) {
        self.appearance.seal_resource_selection();
    }

    pub fn render_resources(&self) -> &[CsharpRenderResource] {
        &self.appearance.state.render_resources
    }

    pub fn outputs(&self, call: &CsharpEngineCall) -> CsharpEngineCallOutput {
        CsharpEngineCallOutput {
            frame: call.appearance.as_ref().and_then(|call| call.frame.clone()),
            ui: call.ui.projections.clone(),
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
