use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use render_host_contracts::{
    RendererCameraBasis, RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
    RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
};
use render_model::{RenderDiff, RenderFrameDiff, SkyBackgroundDescriptor};

use crate::{appearance::RuntimeAppearanceCall, composition::ABI_OK, CsharpEngineServicesError};

#[derive(Clone)]
struct CameraState {
    cameras: BTreeMap<u64, NativeCameraDescriptor>,
    active: Option<u64>,
    sky_texture: Option<u64>,
    next_camera: u64,
}

pub(crate) struct RuntimeCameraViewCall {
    state: CameraState,
    pub(crate) composition: Option<RendererViewComposition>,
    pub(crate) sky_texture: Option<Option<u64>>,
}

/// Engine-owned typed camera/view projection. Product facts are copied at the
/// ABI edge; this owner derives private renderer identifiers and publishes the
/// complete active view against the current host surface.
pub(crate) struct RuntimeCameraViewBridge {
    state: CameraState,
    staged: Option<RuntimeCameraViewCall>,
    callback_error: Option<CsharpEngineServicesError>,
}

impl RuntimeCameraViewBridge {
    pub(crate) fn new() -> Self {
        Self {
            state: CameraState {
                cameras: BTreeMap::new(),
                active: None,
                sky_texture: None,
                next_camera: 1,
            },
            staged: None,
            callback_error: None,
        }
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeCameraViewCall {
            state: self.state.clone(),
            composition: None,
            sky_texture: None,
        });
        self.callback_error = None;
    }

    pub(crate) fn begin_attach_call(&mut self) -> Result<(), CsharpEngineServicesError> {
        self.begin_call();
        let staged = self
            .staged
            .as_mut()
            .expect("attach begins a camera/view stage");
        staged.sky_texture = Some(staged.state.sky_texture);
        stage_composition(staged)
    }

    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeCameraViewCall, CsharpEngineServicesError> {
        if let Some(error) = self.callback_error.take() {
            self.staged = None;
            return Err(error);
        }
        self.staged.take().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_CAMERA_VIEW_CALL",
                "camera/view service was called outside a product call",
            )
        })
    }

    pub(crate) fn commit(&mut self, staged: RuntimeCameraViewCall) {
        self.state = staged.state;
    }

    /// Rebuilds the current retained camera composition without entering a
    /// product callback or changing camera handles. Sky is retained by the
    /// presentation world alongside graphics resources.
    pub(crate) fn snapshot_composition(
        &self,
    ) -> Result<RendererViewComposition, CsharpEngineServicesError> {
        let mut snapshot = RuntimeCameraViewCall {
            state: self.state.clone(),
            composition: None,
            sky_texture: None,
        };
        stage_composition(&mut snapshot)?;
        Ok(snapshot
            .composition
            .expect("camera composition staging always produces a composition"))
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeCameraViewCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_CAMERA_VIEW_CALL",
                "camera/view service was called outside a product call",
            )
        })
    }

    fn create(
        &mut self,
        descriptor: NativeCameraDescriptor,
    ) -> Result<NativeCameraHandle, CsharpEngineServicesError> {
        validate_descriptor(descriptor)?;
        let staged = self.staged_mut()?;
        let handle = staged.state.next_camera;
        staged.state.next_camera = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle overflow")
        })?;
        staged.state.cameras.insert(handle, descriptor);
        stage_composition(staged)?;
        Ok(NativeCameraHandle { value: handle })
    }

    fn update(
        &mut self,
        request: NativeCameraUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        validate_descriptor(request.descriptor)?;
        let staged = self.staged_mut()?;
        let camera = staged
            .state
            .cameras
            .get_mut(&request.camera.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle is not live")
            })?;
        *camera = request.descriptor;
        stage_composition(staged)
    }

    fn replace(
        &mut self,
        request: NativeCameraReplaceRequest,
    ) -> Result<NativeCameraHandle, CsharpEngineServicesError> {
        validate_descriptor(request.replacement)?;
        let staged = self.staged_mut()?;
        if staged.state.cameras.remove(&request.camera.value).is_none() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CAMERA_HANDLE",
                "camera handle is not live",
            ));
        }
        let replacement = staged.state.next_camera;
        staged.state.next_camera = replacement.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle overflow")
        })?;
        staged
            .state
            .cameras
            .insert(replacement, request.replacement);
        if staged.state.active == Some(request.camera.value) {
            staged.state.active = Some(replacement);
        }
        stage_composition(staged)?;
        Ok(NativeCameraHandle { value: replacement })
    }

    fn destroy(&mut self, camera: NativeCameraHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        // Replacement turns the prior owner into a tombstone. Its generated
        // IDisposable must remain safe to release in normal owner-first or
        // replacement-first teardown order.
        staged.state.cameras.remove(&camera.value);
        if staged.state.active == Some(camera.value) {
            staged.state.active = None;
        }
        stage_composition(staged)
    }

    fn set_active(&mut self, camera: NativeCameraHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if !staged.state.cameras.contains_key(&camera.value) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CAMERA_HANDLE",
                "camera handle is not live",
            ));
        }
        staged.state.active = Some(camera.value);
        stage_composition(staged)
    }

    fn clear_active(&mut self) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        staged.state.active = None;
        stage_composition(staged)
    }

    fn set_sky(
        &mut self,
        texture: NativeRenderResourceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if texture.value == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SKY_TEXTURE",
                "sky texture handle must be non-zero",
            ));
        }
        let staged = self.staged_mut()?;
        staged.state.sky_texture = Some(texture.value);
        staged.sky_texture = Some(Some(texture.value));
        Ok(())
    }

    fn clear_sky(&mut self) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        staged.state.sky_texture = None;
        staged.sky_texture = Some(None);
        Ok(())
    }
}

fn stage_composition(staged: &mut RuntimeCameraViewCall) -> Result<(), CsharpEngineServicesError> {
    let (cameras, views) = match staged.state.active {
        None => (Vec::new(), Vec::new()),
        Some(handle) => {
            let descriptor = staged.state.cameras.get(&handle).copied().ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_CAMERA_HANDLE",
                    "active camera handle is not live",
                )
            })?;
            let id = format!("csharp-camera-{handle}");
            (
                vec![composition_camera(id.clone(), descriptor)?],
                vec![RendererCompositionView {
                    id: "csharp-active-view".to_owned(),
                    camera_id: id,
                    target: RendererViewTarget::Primary,
                    viewport: viewport(descriptor.viewport),
                    order: 0,
                }],
            )
        }
    };
    let composition = RendererViewComposition {
        schema_version: RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
        cameras,
        targets: Vec::new(),
        views,
        presentations: Vec::new(),
    };
    composition.validate().map_err(|error| {
        CsharpEngineServicesError::new(
            "CSHARP_CAMERA_COMPOSITION",
            format!("camera composition is invalid: {error:?}"),
        )
    })?;
    staged.composition = Some(composition);
    Ok(())
}

fn composition_camera(
    id: String,
    descriptor: NativeCameraDescriptor,
) -> Result<RendererCompositionCamera, CsharpEngineServicesError> {
    let projection = match descriptor.projection.kind {
        NativeCameraProjectionKind::Perspective => RendererCameraProjection::Perspective {
            fov_y_degrees: descriptor.projection.fov_y_degrees,
            near: descriptor.projection.near,
            far: descriptor.projection.far,
        },
        NativeCameraProjectionKind::Orthographic => RendererCameraProjection::Orthographic {
            vertical_size: descriptor.projection.vertical_size,
            near: descriptor.projection.near,
            far: descriptor.projection.far,
        },
    };
    Ok(RendererCompositionCamera {
        id,
        pose: RendererCameraPose {
            position: native_vec3(descriptor.pose.position),
            pitch_degrees: descriptor.pose.pitch_degrees,
            yaw_degrees: descriptor.pose.yaw_degrees,
        },
        basis: match descriptor.basis_mode {
            NativeCameraBasisMode::Derived => None,
            NativeCameraBasisMode::Explicit => Some(RendererCameraBasis {
                forward: native_vec3(descriptor.basis.forward),
                right: native_vec3(descriptor.basis.right),
                up: native_vec3(descriptor.basis.up),
            }),
        },
        projection,
    })
}

fn native_vec3(value: NativeVec3) -> [f64; 3] {
    [f64::from(value.x), f64::from(value.y), f64::from(value.z)]
}

fn viewport(value: NativeCameraViewport) -> RendererViewport {
    RendererViewport {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}

fn validate_descriptor(
    descriptor: NativeCameraDescriptor,
) -> Result<(), CsharpEngineServicesError> {
    let composition = RendererViewComposition {
        schema_version: RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
        cameras: vec![composition_camera("validate".to_owned(), descriptor)?],
        targets: Vec::new(),
        views: vec![RendererCompositionView {
            id: "validate-view".to_owned(),
            camera_id: "validate".to_owned(),
            target: RendererViewTarget::Primary,
            viewport: viewport(descriptor.viewport),
            order: 0,
        }],
        presentations: Vec::new(),
    };
    composition.validate().map_err(|error| {
        CsharpEngineServicesError::new(
            "CSHARP_CAMERA_DESCRIPTOR",
            format!("camera descriptor is invalid: {error:?}"),
        )
    })
}

pub(crate) fn sky_frame(
    change: Option<Option<u64>>,
    appearance: Option<&RuntimeAppearanceCall>,
) -> Result<Option<RenderFrameDiff>, CsharpEngineServicesError> {
    let Some(change) = change else {
        return Ok(None);
    };
    let mut operations = Vec::with_capacity(if change.is_some() { 2 } else { 1 });
    let background = if let Some(handle) = change {
        let texture = appearance
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SKY_TEXTURE",
                    "sky background needs an appearance call that selected its texture",
                )
            })?
            .texture_descriptor(handle)?;
        let identity = texture.id.clone();
        operations.push(RenderDiff::DefineTexture { texture });
        Some(SkyBackgroundDescriptor { texture: identity })
    } else {
        None
    };
    operations.push(RenderDiff::SetSkyBackground { background });
    RenderFrameDiff::try_from_ops(operations)
        .map(Some)
        .map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_SKY_BACKGROUND",
                format!("sky frame is invalid: {error:?}"),
            )
        })
}

pub(crate) unsafe extern "C" fn create_camera(
    context: *mut c_void,
    request: *const NativeCameraDescriptor,
    result: *mut NativeCameraHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.create(unsafe { *request }) {
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

pub(crate) unsafe extern "C" fn update_camera(
    context: *mut c_void,
    request: *const NativeCameraUpdateRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.update(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn replace_camera(
    context: *mut c_void,
    request: *const NativeCameraReplaceRequest,
    result: *mut NativeCameraHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.replace(unsafe { *request }) {
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

pub(crate) unsafe extern "C" fn destroy_camera(
    context: *mut c_void,
    camera: NativeCameraHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.destroy(camera) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_active_camera(
    context: *mut c_void,
    camera: NativeCameraHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_active(camera) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn clear_active_camera(
    context: *mut c_void,
    request: *const NativeClearActiveCameraRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.clear_active() {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_sky_background(
    context: *mut c_void,
    texture: NativeRenderResourceHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_sky(texture) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn clear_sky_background(
    context: *mut c_void,
    request: *const NativeClearSkyBackgroundRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.clear_sky() {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}
