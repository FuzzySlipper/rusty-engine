//! The `RendererSettings` service: a product reads the renderer's settings
//! in effect and selects new ones. The selection is retained by the
//! presentation world (`RenderDiff::SetRendererSettings`), so a rebaseline
//! keeps it; the runtime reports what the renderer draws with between calls.

use std::ffi::c_void;

use csharp_engine_abi::{
    NativeAmbientOcclusionMode, NativeAntialiasing, NativeOperationErrorReceipt,
    NativeRendererSettingChoiceReadout, NativeRendererSettingKind,
    NativeRendererSettingOptionReadout, NativeRendererSettingRefusal, NativeRendererSettingsApi,
    NativeRendererSettingsCatalogueResult, NativeRendererSettingsReadout,
    NativeRendererSettingsRequest, NativeUtf8Slice, NativeVolumetricCloudsQuality,
    NativeVolumetricFogQuality,
};
use render_model::{
    renderer_setting_value, AmbientOcclusionMode, AmbientOcclusionSettings, RendererSettingKind,
    RendererSettingsDescriptor, VolumetricCloudsQuality, VolumetricFogQuality,
    RENDERER_SETTING_OPTIONS,
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
    /// The last catalogue described: its value texts and rows, kept until
    /// the next call so the borrowed result stays valid.
    catalogue: DescribedCatalogue,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
}

#[derive(Default)]
struct DescribedCatalogue {
    /// Owns the value texts the rows point into.
    _texts: Vec<String>,
    options: Vec<NativeRendererSettingOptionReadout>,
    choices: Vec<NativeRendererSettingChoiceReadout>,
}

fn text(value: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}

/// A setting's value in the text form the catalogue's choices use.
fn value_text(settings: &RendererSettingsDescriptor, id: &str) -> String {
    match renderer_setting_value(settings, id) {
        Some(serde_json::Value::String(value)) => value,
        Some(value) => value.to_string(),
        None => String::new(),
    }
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
            catalogue: DescribedCatalogue::default(),
            operation_diagnostics: Default::default(),
        }
    }

    /// The catalogue with this product's values (`RendererSettings.Describe`).
    fn describe(&mut self) -> NativeRendererSettingsCatalogueResult {
        let requested = renderer_settings_descriptor(&self.readout.requested);
        let effective = renderer_settings_descriptor(&self.readout.effective);
        let defaults = RendererSettingsDescriptor::DEFAULT;
        let readout = self.readout;
        let refusal = |id: &str| match id {
            "ambientOcclusion" => readout.ambient_occlusion_refusal,
            "antialiasing" => readout.antialiasing_refusal,
            "vsync" => readout.vsync_refusal,
            "clusteredLighting" => readout.clustered_lighting_refusal,
            "gpuCulling" => readout.gpu_culling_refusal,
            "volumetricFog" => readout.volumetric_fog_refusal,
            "volumetricClouds" => readout.volumetric_clouds_refusal,
            _ => NativeRendererSettingRefusal::None,
        };
        // The value texts first, then rows pointing into them: the strings'
        // buffers do not move once the vector of them is built.
        let texts: Vec<String> = RENDERER_SETTING_OPTIONS
            .iter()
            .flat_map(|option| {
                [
                    value_text(&defaults, option.id),
                    value_text(&requested, option.id),
                    value_text(&effective, option.id),
                ]
            })
            .collect();
        let mut options = Vec::with_capacity(RENDERER_SETTING_OPTIONS.len());
        let mut choices = Vec::new();
        for (index, option) in RENDERER_SETTING_OPTIONS.iter().enumerate() {
            let (kind, min, max, step, unit) =
                match option.kind {
                    RendererSettingKind::Toggle => {
                        (NativeRendererSettingKind::Toggle, 0.0, 0.0, 0.0, "")
                    }
                    RendererSettingKind::Choice { choices: named } => {
                        choices.extend(named.iter().map(|choice| {
                            NativeRendererSettingChoiceReadout {
                                option_id: text(option.id),
                                value: text(choice.value),
                                label: text(choice.label),
                            }
                        }));
                        (NativeRendererSettingKind::Choice, 0.0, 0.0, 0.0, "")
                    }
                    RendererSettingKind::Range {
                        min,
                        max,
                        step,
                        unit,
                    } => (NativeRendererSettingKind::Range, min, max, step, unit),
                };
            options.push(NativeRendererSettingOptionReadout {
                id: text(option.id),
                label: text(option.label),
                group: text(option.group),
                description: text(option.description),
                kind,
                min,
                max,
                step,
                unit: text(unit),
                engine_default: text(&texts[index * 3]),
                requested: text(&texts[index * 3 + 1]),
                value: text(&texts[index * 3 + 2]),
                refusal: refusal(option.id),
                restart: option.restart,
                cost: text(option.cost),
            });
        }
        self.catalogue = DescribedCatalogue {
            _texts: texts,
            options,
            choices,
        };
        NativeRendererSettingsCatalogueResult {
            options: self.catalogue.options.as_ptr(),
            options_len: self.catalogue.options.len(),
            choices: self.catalogue.choices.as_ptr(),
            choices_len: self.catalogue.choices.len(),
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
        describe,
    }
}

unsafe extern "C" fn describe(
    context: *mut c_void,
    catalogue: *mut NativeRendererSettingsCatalogueResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || catalogue.is_null() {
        return 0;
    }
    // SAFETY: the function-table context is the live bridge; the result
    // borrows the bridge's catalogue until the next call on it.
    let bridge = unsafe { &mut *context.cast::<RuntimeRendererSettingsBridge>() };
    unsafe { *catalogue = bridge.describe() };
    ABI_OK
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

#[cfg(test)]
mod tests {
    use super::*;

    fn read(slice: NativeUtf8Slice) -> String {
        // SAFETY: the slice borrows the bridge's catalogue, alive here.
        String::from_utf8(unsafe { std::slice::from_raw_parts(slice.bytes, slice.len) }.to_vec())
            .unwrap()
    }

    #[test]
    fn describe_lists_every_setting_with_its_values_refusal_and_cost() {
        let mut bridge = RuntimeRendererSettingsBridge::new();
        let mut readout = bridge.readout;
        readout.requested.volumetric_fog = NativeVolumetricFogQuality::High;
        readout.volumetric_fog_refusal = NativeRendererSettingRefusal::SoftwareAdapter;
        bridge.ingest(readout);
        let catalogue = bridge.describe();
        let options =
            unsafe { std::slice::from_raw_parts(catalogue.options, catalogue.options_len) };
        let choices =
            unsafe { std::slice::from_raw_parts(catalogue.choices, catalogue.choices_len) };
        assert_eq!(options.len(), RENDERER_SETTING_OPTIONS.len());
        let fog = options
            .iter()
            .find(|option| read(option.id) == "volumetricFog")
            .expect("volumetric fog");
        assert_eq!(fog.kind, NativeRendererSettingKind::Choice);
        assert_eq!(read(fog.engine_default), "off");
        assert_eq!(read(fog.requested), "high");
        assert_eq!(
            read(fog.value),
            "off",
            "nothing refused it in this readout's effective"
        );
        assert_eq!(fog.refusal, NativeRendererSettingRefusal::SoftwareAdapter);
        assert!(!fog.restart);
        assert!(read(fog.cost).contains("ms"));
        let fog_choices: Vec<String> = choices
            .iter()
            .filter(|choice| read(choice.option_id) == "volumetricFog")
            .map(|choice| read(choice.value))
            .collect();
        assert_eq!(fog_choices, ["off", "low", "high"]);
        let scale = options
            .iter()
            .find(|option| read(option.id) == "renderScale")
            .expect("render scale");
        assert_eq!(scale.kind, NativeRendererSettingKind::Range);
        assert_eq!((scale.min, scale.max), (0.5, 1.0));
        assert_eq!(read(scale.value), "1.0");
    }
}
