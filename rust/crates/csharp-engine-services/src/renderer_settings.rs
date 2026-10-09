//! The `RendererSettings` service: a product reads the renderer's settings
//! in effect and selects new ones. The selection is retained by the
//! presentation world (`RenderDiff::SetRendererSettings`), so a rebaseline
//! keeps it; the runtime reports what the renderer draws with between calls.

use std::ffi::c_void;

use csharp_engine_abi::{
    NativeAmbientOcclusionMode, NativeAntialiasing, NativeOperationErrorReceipt,
    NativeRendererSettingRefusal, NativeRendererSettingsApi, NativeRendererSettingsReadout,
    NativeRendererSettingsRequest, NativeVolumetricCloudsQuality, NativeVolumetricFogQuality,
};
use render_model::{
    AmbientOcclusionMode, AmbientOcclusionSettings, RendererSettingsDescriptor,
    VolumetricCloudsQuality, VolumetricFogQuality,
};

use crate::composition::ABI_OK;
use crate::CsharpEngineServicesError;

/// The settings a product selected during one call, if it did.
pub(crate) struct RuntimeRendererSettingsCall {
    pub(crate) settings: Option<RendererSettingsDescriptor>,
}

pub(crate) struct RuntimeRendererSettingsBridge {
    staged: Option<RuntimeRendererSettingsCall>,
    /// What the renderer last reported: the manifest's values until the
    /// product sets others, and the Engine defaults until the renderer
    /// reports.
    readout: NativeRendererSettingsReadout,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
}

impl RuntimeRendererSettingsBridge {
    pub(crate) fn new() -> Self {
        let defaults = renderer_settings_request(&RendererSettingsDescriptor::DEFAULT);
        Self {
            staged: None,
            readout: NativeRendererSettingsReadout {
                requested: defaults,
                effective: defaults,
                ambient_occlusion_refusal: NativeRendererSettingRefusal::None,
                antialiasing_refusal: NativeRendererSettingRefusal::None,
                vsync_refusal: NativeRendererSettingRefusal::None,
                clustered_lighting_refusal: NativeRendererSettingRefusal::None,
                gpu_culling_refusal: NativeRendererSettingRefusal::None,
                volumetric_fog_refusal: csharp_engine_abi::NativeRendererSettingRefusal::None,
                volumetric_clouds_refusal: csharp_engine_abi::NativeRendererSettingRefusal::None,
            },
            operation_diagnostics: Default::default(),
        }
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeRendererSettingsCall { settings: None });
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeRendererSettingsCall, CsharpEngineServicesError> {
        self.staged.take().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDERER_SETTINGS_CALL",
                "renderer settings service was called outside a product call",
            )
        })
    }

    /// The renderer's report of what it draws with, between calls.
    pub(crate) fn ingest(&mut self, readout: NativeRendererSettingsReadout) {
        self.readout = readout;
    }

    fn set(
        &mut self,
        request: NativeRendererSettingsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let settings = renderer_settings_descriptor(&request);
        if !settings.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDERER_SETTINGS",
                "ambient occlusion strength must be finite and at least 0, its radius finite and above 0, and the render scale from 0.5 to 1",
            ));
        }
        let staged = self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDERER_SETTINGS_CALL",
                "renderer settings service was called outside a product call",
            )
        })?;
        staged.settings = Some(settings);
        Ok(())
    }
}

/// A request as the retained model keeps it.
pub(crate) fn renderer_settings_descriptor(
    request: &NativeRendererSettingsRequest,
) -> RendererSettingsDescriptor {
    RendererSettingsDescriptor {
        shadows: request.shadows,
        shadow_budget: (request.shadow_budget > 0).then_some(request.shadow_budget),
        ambient_occlusion: AmbientOcclusionSettings {
            mode: match request.ambient_occlusion {
                NativeAmbientOcclusionMode::Disabled => AmbientOcclusionMode::Disabled,
                NativeAmbientOcclusionMode::ScreenSpace => AmbientOcclusionMode::ScreenSpace,
                NativeAmbientOcclusionMode::DistanceField => AmbientOcclusionMode::DistanceField,
            },
            strength: request.ambient_occlusion_strength,
            radius: request.ambient_occlusion_radius,
        },
        antialiasing: request.antialiasing as u32,
        render_scale: request.render_scale,
        vsync: request.vsync,
        clustered_lighting: request.clustered_lighting,
        gpu_culling: request.gpu_culling,
        volumetric_fog: match request.volumetric_fog {
            NativeVolumetricFogQuality::Off => VolumetricFogQuality::Off,
            NativeVolumetricFogQuality::Low => VolumetricFogQuality::Low,
            NativeVolumetricFogQuality::High => VolumetricFogQuality::High,
        },
        volumetric_clouds: match request.volumetric_clouds {
            NativeVolumetricCloudsQuality::Off => VolumetricCloudsQuality::Off,
            NativeVolumetricCloudsQuality::Low => VolumetricCloudsQuality::Low,
            NativeVolumetricCloudsQuality::High => VolumetricCloudsQuality::High,
        },
    }
}

/// Retained settings as a product reads them. A sample count the model
/// does not name reads as 4.
pub fn renderer_settings_request(
    settings: &RendererSettingsDescriptor,
) -> NativeRendererSettingsRequest {
    NativeRendererSettingsRequest {
        shadows: settings.shadows,
        shadow_budget: settings.shadow_budget.unwrap_or(0),
        ambient_occlusion: match settings.ambient_occlusion.mode {
            AmbientOcclusionMode::Disabled => NativeAmbientOcclusionMode::Disabled,
            AmbientOcclusionMode::ScreenSpace => NativeAmbientOcclusionMode::ScreenSpace,
            AmbientOcclusionMode::DistanceField => NativeAmbientOcclusionMode::DistanceField,
        },
        ambient_occlusion_strength: settings.ambient_occlusion.strength,
        ambient_occlusion_radius: settings.ambient_occlusion.radius,
        antialiasing: match settings.antialiasing {
            1 => NativeAntialiasing::Off,
            2 => NativeAntialiasing::Msaa2,
            _ => NativeAntialiasing::Msaa4,
        },
        render_scale: settings.render_scale,
        vsync: settings.vsync,
        clustered_lighting: settings.clustered_lighting,
        gpu_culling: settings.gpu_culling,
        volumetric_fog: match settings.volumetric_fog {
            VolumetricFogQuality::Off => NativeVolumetricFogQuality::Off,
            VolumetricFogQuality::Low => NativeVolumetricFogQuality::Low,
            VolumetricFogQuality::High => NativeVolumetricFogQuality::High,
        },
        volumetric_clouds: match settings.volumetric_clouds {
            VolumetricCloudsQuality::Off => NativeVolumetricCloudsQuality::Off,
            VolumetricCloudsQuality::Low => NativeVolumetricCloudsQuality::Low,
            VolumetricCloudsQuality::High => NativeVolumetricCloudsQuality::High,
        },
    }
}

pub(crate) fn api(bridge: &mut RuntimeRendererSettingsBridge) -> NativeRendererSettingsApi {
    NativeRendererSettingsApi {
        context: (bridge as *mut RuntimeRendererSettingsBridge).cast(),
        read,
        set,
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    readout: *mut NativeRendererSettingsReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || readout.is_null() {
        return 0;
    }
    // SAFETY: the function-table context is the live bridge and the caller
    // owns the output for this direct generated service call.
    let bridge = unsafe { &*context.cast::<RuntimeRendererSettingsBridge>() };
    unsafe { *readout = bridge.readout };
    ABI_OK
}

unsafe extern "C" fn set(
    context: *mut c_void,
    request: *const NativeRendererSettingsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeRendererSettingsBridge>() };
    match bridge.set(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}
