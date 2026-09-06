use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use render_host_contracts::{
    RendererCameraBasis, RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
    RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
};
use render_model::{RenderDiff, RenderFrameDiff, SkyBackgroundDescriptor};

use crate::{
    appearance::RuntimeAppearanceCall,
    composition::{borrowed_slice, ABI_OK},
    CsharpEngineServicesError,
};

#[derive(Clone)]
struct CameraState {
    cameras: BTreeMap<u64, NativeCameraDescriptor>,
    targets: BTreeMap<u64, CameraTargetState>,
    views: Vec<NativeCameraCompositionView>,
    presentations: Vec<NativeCameraCompositionPresentation>,
    /// `SetActiveCamera` keeps this convenience selection live. Explicit
    /// compositions clear it and own their copied viewports independently.
    active_camera: Option<u64>,
    sky_texture: Option<u64>,
    next_camera: u64,
    next_target: u64,
}

#[derive(Clone, Copy)]
struct CameraTargetState {
    descriptor: NativeCameraTargetDescriptor,
    revision: u64,
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
                targets: BTreeMap::new(),
                views: Vec::new(),
                presentations: Vec::new(),
                active_camera: None,
                sky_texture: None,
                next_camera: 1,
                next_target: 1,
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
        if staged.state.active_camera == Some(request.camera.value) {
            if let Some(view) = staged.state.views.first_mut() {
                view.viewport = request.descriptor.viewport;
            }
        }
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
        for view in &mut staged.state.views {
            if view.camera == request.camera {
                view.camera = NativeCameraHandle { value: replacement };
            }
        }
        if staged.state.active_camera == Some(request.camera.value) {
            staged.state.active_camera = Some(replacement);
            if let Some(view) = staged.state.views.first_mut() {
                view.viewport = request.replacement.viewport;
            }
        }
        stage_composition(staged)?;
        Ok(NativeCameraHandle { value: replacement })
    }

    fn create_target(
        &mut self,
        descriptor: NativeCameraTargetDescriptor,
    ) -> Result<NativeCameraTargetHandle, CsharpEngineServicesError> {
        validate_target_descriptor(descriptor)?;
        let staged = self.staged_mut()?;
        let handle = staged.state.next_target;
        staged.state.next_target = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_TARGET_HANDLE", "target handle overflow")
        })?;
        staged.state.targets.insert(
            handle,
            CameraTargetState {
                descriptor,
                revision: 1,
            },
        );
        stage_composition(staged)?;
        Ok(NativeCameraTargetHandle { value: handle })
    }

    fn update_target(
        &mut self,
        request: NativeCameraTargetUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        validate_target_descriptor(request.descriptor)?;
        let staged = self.staged_mut()?;
        let target = staged
            .state
            .targets
            .get_mut(&request.target.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_CAMERA_TARGET_HANDLE",
                    "target handle is not live",
                )
            })?;
        target.descriptor = request.descriptor;
        target.revision = target.revision.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_CAMERA_TARGET_REVISION",
                "target revision overflow",
            )
        })?;
        stage_composition(staged)
    }

    fn replace_target(
        &mut self,
        request: NativeCameraTargetReplaceRequest,
    ) -> Result<NativeCameraTargetHandle, CsharpEngineServicesError> {
        validate_target_descriptor(request.replacement)?;
        let staged = self.staged_mut()?;
        if staged.state.targets.remove(&request.target.value).is_none() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CAMERA_TARGET_HANDLE",
                "target handle is not live",
            ));
        }
        let handle = staged.state.next_target;
        staged.state.next_target = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_TARGET_HANDLE", "target handle overflow")
        })?;
        staged.state.targets.insert(
            handle,
            CameraTargetState {
                descriptor: request.replacement,
                revision: 1,
            },
        );
        let replacement = NativeCameraTargetHandle { value: handle };
        for view in &mut staged.state.views {
            if view.target.value == request.target.value {
                view.target = NativeCameraTargetReference { value: handle };
            }
        }
        for presentation in &mut staged.state.presentations {
            if presentation.source_target == request.target {
                presentation.source_target = replacement;
            }
        }
        stage_composition(staged)?;
        Ok(replacement)
    }

    fn destroy_target(
        &mut self,
        target: NativeCameraTargetHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if target.value == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CAMERA_TARGET_HANDLE",
                "the primary surface is not an owned camera target",
            ));
        }
        let staged = self.staged_mut()?;
        staged.state.targets.remove(&target.value);
        staged
            .state
            .views
            .retain(|view| view.target.value != target.value);
        staged
            .state
            .presentations
            .retain(|presentation| presentation.source_target != target);
        stage_composition(staged)
    }

    unsafe fn set_composition(
        &mut self,
        request: &NativeCameraCompositionRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let views = borrowed_slice(request.views, request.views_len, "camera composition views")?;
        let presentations = borrowed_slice(
            request.presentations,
            request.presentations_len,
            "camera composition presentations",
        )?;
        let staged = self.staged_mut()?;
        let mut candidate = RuntimeCameraViewCall {
            state: staged.state.clone(),
            composition: None,
            sky_texture: None,
        };
        candidate.state.views = views.to_vec();
        candidate.state.presentations = presentations.to_vec();
        candidate.state.active_camera = None;
        stage_composition(&mut candidate)?;
        staged.state = candidate.state;
        staged.composition = candidate.composition;
        Ok(())
    }

    fn destroy(&mut self, camera: NativeCameraHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        // Replacement turns the prior owner into a tombstone. Its generated
        // IDisposable must remain safe to release in normal owner-first or
        // replacement-first teardown order.
        staged.state.cameras.remove(&camera.value);
        staged.state.views.retain(|view| view.camera != camera);
        if staged.state.active_camera == Some(camera.value) {
            staged.state.active_camera = None;
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
        staged.state.views = vec![NativeCameraCompositionView {
            camera,
            target: NativeCameraTargetReference::default(),
            viewport: staged
                .state
                .cameras
                .get(&camera.value)
                .expect("live camera was checked")
                .viewport,
            order: 0,
        }];
        staged.state.presentations.clear();
        staged.state.active_camera = Some(camera.value);
        stage_composition(staged)
    }

    fn clear_active(&mut self) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        staged.state.views.clear();
        staged.state.presentations.clear();
        staged.state.active_camera = None;
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
    let mut cameras = BTreeMap::new();
    let mut targets = BTreeMap::new();
    let mut views = Vec::with_capacity(staged.state.views.len());
    for (index, view) in staged.state.views.iter().enumerate() {
        let descriptor = staged
            .state
            .cameras
            .get(&view.camera.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_CAMERA_COMPOSITION_CAMERA",
                    "composition view names a camera that is not live",
                )
            })?;
        let camera_id = format!("csharp-camera-{}", view.camera.value);
        cameras
            .entry(view.camera.value)
            .or_insert(composition_camera(camera_id.clone(), descriptor)?);
        let target = if view.target.value == 0 {
            RendererViewTarget::Primary
        } else {
            let target_state = staged
                .state
                .targets
                .get(&view.target.value)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_CAMERA_COMPOSITION_TARGET",
                        "composition view names a target that is not live",
                    )
                })?;
            let target_id = format!("csharp-target-{}", view.target.value);
            targets
                .entry(view.target.value)
                .or_insert_with(|| composition_target(target_id.clone(), *target_state));
            RendererViewTarget::Offscreen {
                target_id,
                target_revision: target_state.revision,
            }
        };
        views.push(RendererCompositionView {
            id: format!("csharp-view-{index}"),
            camera_id,
            target,
            viewport: viewport(view.viewport),
            order: view.order,
        });
    }
    let mut presentations = Vec::with_capacity(staged.state.presentations.len());
    for (index, presentation) in staged.state.presentations.iter().enumerate() {
        let target_state = staged
            .state
            .targets
            .get(&presentation.source_target.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_CAMERA_COMPOSITION_TARGET",
                    "composition presentation names a target that is not live",
                )
            })?;
        let target_id = format!("csharp-target-{}", presentation.source_target.value);
        targets
            .entry(presentation.source_target.value)
            .or_insert_with(|| composition_target(target_id.clone(), *target_state));
        presentations.push(render_host_contracts::RendererCompositionPresentation {
            id: format!("csharp-presentation-{index}"),
            source_target_id: target_id,
            source_target_revision: target_state.revision,
            destination: render_host_contracts::RendererPrimaryDestination {
                kind: render_host_contracts::RendererPrimaryDestinationKind::Primary,
                viewport: viewport(presentation.destination),
            },
            order: presentation.order,
        });
    }
    let composition = RendererViewComposition {
        schema_version: RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
        cameras: cameras.into_values().collect(),
        targets: targets.into_values().collect(),
        views,
        presentations,
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

fn composition_target(
    id: String,
    state: CameraTargetState,
) -> render_host_contracts::RendererCompositionTarget {
    render_host_contracts::RendererCompositionTarget {
        id,
        revision: state.revision,
        width: state.descriptor.width,
        height: state.descriptor.height,
        color: match state.descriptor.color {
            NativeCameraTargetColor::Rgba8Srgb => {
                render_host_contracts::RendererTargetColor::Rgba8Srgb
            }
        },
        depth: match state.descriptor.depth {
            NativeCameraTargetDepth::Depth24 => render_host_contracts::RendererTargetDepth::Depth24,
            NativeCameraTargetDepth::None => render_host_contracts::RendererTargetDepth::None,
        },
        sampling: match state.descriptor.sampling {
            NativeCameraTargetSampling::Linear => {
                render_host_contracts::RendererTargetSampling::Linear
            }
            NativeCameraTargetSampling::Nearest => {
                render_host_contracts::RendererTargetSampling::Nearest
            }
        },
    }
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

fn validate_target_descriptor(
    descriptor: NativeCameraTargetDescriptor,
) -> Result<(), CsharpEngineServicesError> {
    let composition = RendererViewComposition {
        schema_version: RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
        cameras: Vec::new(),
        targets: vec![composition_target(
            "validate-target".to_owned(),
            CameraTargetState {
                descriptor,
                revision: 1,
            },
        )],
        views: Vec::new(),
        presentations: Vec::new(),
    };
    composition.validate().map_err(|error| {
        CsharpEngineServicesError::new(
            "CSHARP_CAMERA_TARGET_DESCRIPTOR",
            format!("camera target descriptor is invalid: {error:?}"),
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

pub(crate) unsafe extern "C" fn create_camera_target(
    context: *mut c_void,
    request: *const NativeCameraTargetDescriptor,
    result: *mut NativeCameraTargetHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.create_target(unsafe { *request }) {
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

pub(crate) unsafe extern "C" fn update_camera_target(
    context: *mut c_void,
    request: *const NativeCameraTargetUpdateRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.update_target(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn replace_camera_target(
    context: *mut c_void,
    request: *const NativeCameraTargetReplaceRequest,
    result: *mut NativeCameraTargetHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.replace_target(unsafe { *request }) {
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

pub(crate) unsafe extern "C" fn destroy_camera_target(
    context: *mut c_void,
    target: NativeCameraTargetHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.destroy_target(target) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_camera_composition(
    context: *mut c_void,
    request: *const NativeCameraCompositionRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match unsafe { bridge.set_composition(&*request) } {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn camera_descriptor(viewport: NativeCameraViewport) -> NativeCameraDescriptor {
        NativeCameraDescriptor {
            pose: NativeCameraPose {
                position: NativeVec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                pitch_degrees: 0.0,
                yaw_degrees: 0.0,
            },
            basis_mode: NativeCameraBasisMode::Derived,
            basis: NativeCameraBasis::default(),
            projection: NativeCameraProjection {
                kind: NativeCameraProjectionKind::Perspective,
                fov_y_degrees: 70.0,
                vertical_size: 1.0,
                near: 0.1,
                far: 1_000.0,
            },
            viewport,
        }
    }

    fn target_descriptor() -> NativeCameraTargetDescriptor {
        NativeCameraTargetDescriptor {
            width: 320,
            height: 180,
            color: NativeCameraTargetColor::Rgba8Srgb,
            depth: NativeCameraTargetDepth::Depth24,
            sampling: NativeCameraTargetSampling::Linear,
        }
    }

    #[test]
    fn retained_multi_view_target_composition_reconstructs_and_reconciles_owners() {
        let mut bridge = RuntimeCameraViewBridge::new();
        bridge.begin_call();
        let front = bridge
            .create(camera_descriptor(NativeCameraViewport {
                x: 0.0,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            }))
            .expect("front camera");
        let rear = bridge
            .create(camera_descriptor(NativeCameraViewport {
                x: 0.5,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            }))
            .expect("rear camera");
        let target = bridge
            .create_target(target_descriptor())
            .expect("offscreen target");
        let views = [
            NativeCameraCompositionView {
                camera: front,
                target: NativeCameraTargetReference::default(),
                viewport: NativeCameraViewport {
                    x: 0.0,
                    y: 0.0,
                    width: 0.5,
                    height: 1.0,
                },
                order: 0,
            },
            NativeCameraCompositionView {
                camera: rear,
                target: NativeCameraTargetReference {
                    value: target.value,
                },
                viewport: NativeCameraViewport {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                order: 1,
            },
        ];
        let presentations = [NativeCameraCompositionPresentation {
            source_target: target,
            destination: NativeCameraViewport {
                x: 0.7,
                y: 0.7,
                width: 0.25,
                height: 0.25,
            },
            order: 2,
        }];
        unsafe {
            bridge
                .set_composition(&NativeCameraCompositionRequest {
                    views: views.as_ptr(),
                    views_len: views.len(),
                    presentations: presentations.as_ptr(),
                    presentations_len: presentations.len(),
                })
                .expect("split and inset composition");
        }
        let staged = bridge.take_staged_call().expect("staged composition");
        let composition = staged.composition.clone().expect("composition output");
        assert_eq!(composition.cameras.len(), 2);
        assert_eq!(composition.targets.len(), 1);
        assert_eq!(composition.views.len(), 2);
        assert_eq!(composition.presentations.len(), 1);
        assert_eq!(composition.targets[0].revision, 1);
        assert!(serde_json::to_string(&composition)
            .expect("serializable composition")
            .contains("csharp-target-1"));
        bridge.commit(staged);

        bridge.begin_attach_call().expect("fresh baseline");
        let attach = bridge.take_staged_call().expect("attach output");
        assert_eq!(attach.composition.as_ref(), Some(&composition));
        bridge.commit(attach);

        bridge.begin_call();
        let mut updated = target_descriptor();
        updated.sampling = NativeCameraTargetSampling::Nearest;
        bridge
            .update_target(NativeCameraTargetUpdateRequest {
                target,
                descriptor: updated,
            })
            .expect("target update");
        let update = bridge.take_staged_call().expect("updated composition");
        assert_eq!(update.composition.as_ref().unwrap().targets[0].revision, 2);
        bridge.commit(update);

        bridge.begin_call();
        let replacement_camera = bridge
            .replace(NativeCameraReplaceRequest {
                camera: rear,
                replacement: camera_descriptor(NativeCameraViewport {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }),
            })
            .expect("camera replacement");
        let replacement_target = bridge
            .replace_target(NativeCameraTargetReplaceRequest {
                target,
                replacement: target_descriptor(),
            })
            .expect("target replacement");
        let replacement = bridge.take_staged_call().expect("replacement composition");
        let composition = replacement.composition.as_ref().unwrap();
        assert!(composition
            .views
            .iter()
            .any(|view| view.camera_id == format!("csharp-camera-{}", replacement_camera.value)));
        assert_eq!(
            composition.targets[0].id,
            format!("csharp-target-{}", replacement_target.value)
        );
        bridge.commit(replacement);

        bridge.begin_call();
        bridge
            .destroy_target(replacement_target)
            .expect("target destroys dependent composition facts");
        let removal = bridge.take_staged_call().expect("target removal");
        let composition = removal.composition.as_ref().unwrap();
        assert_eq!(composition.targets.len(), 0);
        assert_eq!(composition.presentations.len(), 0);
        assert_eq!(composition.views.len(), 1);
        assert_eq!(
            composition.views[0].camera_id,
            format!("csharp-camera-{}", front.value)
        );
    }

    #[test]
    fn invalid_composition_is_fail_atomic_and_active_camera_uses_it_as_convenience() {
        let mut bridge = RuntimeCameraViewBridge::new();
        bridge.begin_call();
        let camera = bridge
            .create(camera_descriptor(NativeCameraViewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }))
            .expect("camera");
        bridge.set_active(camera).expect("active convenience");
        assert_eq!(bridge.staged.as_ref().unwrap().state.views.len(), 1);
        let mut updated_camera = camera_descriptor(NativeCameraViewport {
            x: 0.0,
            y: 0.0,
            width: 0.75,
            height: 1.0,
        });
        updated_camera.viewport.x = 0.25;
        bridge
            .update(NativeCameraUpdateRequest {
                camera,
                descriptor: updated_camera,
            })
            .expect("active camera update");
        assert_eq!(
            bridge.staged.as_ref().unwrap().state.views[0].viewport.x,
            0.25
        );
        let explicit = [NativeCameraCompositionView {
            camera,
            target: NativeCameraTargetReference::default(),
            viewport: NativeCameraViewport {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.4,
            },
            order: 0,
        }];
        unsafe {
            bridge
                .set_composition(&NativeCameraCompositionRequest {
                    views: explicit.as_ptr(),
                    views_len: explicit.len(),
                    presentations: std::ptr::null(),
                    presentations_len: 0,
                })
                .expect("explicit composition");
        }
        updated_camera.viewport = NativeCameraViewport {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        bridge
            .update(NativeCameraUpdateRequest {
                camera,
                descriptor: updated_camera,
            })
            .expect("explicit camera update");
        assert_eq!(
            bridge.staged.as_ref().unwrap().state.views[0].viewport.x,
            0.1
        );
        assert_eq!(
            bridge
                .destroy_target(NativeCameraTargetHandle::default())
                .expect_err("primary is not an owned target")
                .code(),
            "CSHARP_CAMERA_TARGET_HANDLE"
        );
        let invalid = [NativeCameraCompositionView {
            camera: NativeCameraHandle { value: 99 },
            target: NativeCameraTargetReference::default(),
            viewport: NativeCameraViewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            order: 0,
        }];
        assert_eq!(
            unsafe {
                bridge.set_composition(&NativeCameraCompositionRequest {
                    views: invalid.as_ptr(),
                    views_len: invalid.len(),
                    presentations: std::ptr::null(),
                    presentations_len: 0,
                })
            }
            .expect_err("missing camera")
            .code(),
            "CSHARP_CAMERA_COMPOSITION_CAMERA"
        );
        // The failed staging attempt must not overwrite the active-camera view.
        assert_eq!(
            bridge.staged.as_ref().unwrap().state.views[0].camera,
            camera
        );
    }
}
