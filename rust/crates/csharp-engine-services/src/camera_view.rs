use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use render_host_contracts::{
    RendererCameraBasis, RendererCameraInterpolation, RendererCameraMotion, RendererCameraPose,
    RendererCameraProjection, RendererCompositionCamera, RendererCompositionPresentation,
    RendererCompositionTarget, RendererCompositionView, RendererViewComposition,
    RendererViewTarget, RendererViewport,
};
use render_model::{
    AtmosphereDescriptor, AutoExposureDescriptor, BloomDescriptor, CloudsDescriptor,
    ColorGradingDescriptor, FogDescriptor, IndirectAmbient, IndirectLightDescriptor,
    PrecipitationDescriptor, PrecipitationShape, RenderDiff, RenderFrameDiff,
    SkyBackgroundDescriptor, SkyLightDescriptor, SnowDescriptor, SunShaftsDescriptor,
    ToneMappingDescriptor, ToneMappingOperator, WetnessDescriptor, WindDescriptor,
};

use crate::{
    appearance::RuntimeAppearanceCall,
    composition::{borrowed_slice, ABI_OK},
    CsharpEngineServicesError,
};

/// The snow's colour when a request gives none: a slightly blue white,
/// linear.
pub(crate) const SNOW_WHITE: [f32; 3] = [0.82, 0.86, 0.92];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum CameraBackground {
    #[default]
    Default,
    Sky(u64),
    Blend {
        first: u64,
        second: u64,
        amount: f32,
    },
    Color([f32; 4]),
}

#[derive(Default)]
struct CameraState {
    cameras: BTreeMap<u64, CameraEntry>,
    targets: BTreeMap<u64, CameraTargetState>,
    views: Vec<NativeCameraCompositionView>,
    presentations: Vec<NativeCameraCompositionPresentation>,
    /// `SetActiveCamera` keeps this convenience selection live. Explicit
    /// compositions clear it and own their copied viewports independently.
    active_camera: Option<u64>,
    background: CameraBackground,
    /// The star field's catalogue and band, which may not be released while
    /// it draws them.
    stars: Option<NativeStarsRequest>,
    /// The snow in force (`ReadSnow`), as the last `SetSnow` left it.
    snow: Option<SnowDescriptor>,
    /// The decals placed, by id, with the presentation seconds each is gone
    /// at (0: it stays); a decal placed this call has 0 until it is
    /// published (`decal_operations`).
    decals: BTreeMap<u64, f64>,
    /// The decal atlas's textures, which may not be released while decals
    /// sample them.
    decal_atlas: Option<NativeDecalAtlasRequest>,
    next_camera: u64,
    next_target: u64,
    /// The renderer composition `views` and `presentations` describe.
    composition: RetainedComposition,
}

/// A renderer composition kept current by each camera change: a camera's
/// or target's value change patches its own entries, and a change to which
/// cameras and targets the views name builds it again. Every view and
/// presentation names a live camera and target: `set_composition` admits
/// only live ones, and removal drops or remaps the views that named them.
#[derive(Default)]
struct RetainedComposition {
    /// The cameras and targets the views and presentations name, by handle.
    cameras: BTreeMap<u64, RendererCompositionCamera>,
    targets: BTreeMap<u64, RendererCompositionTarget>,
    /// In the order of `CameraState::views` and `::presentations`.
    views: Vec<RendererCompositionView>,
    presentations: Vec<RendererCompositionPresentation>,
}

struct CameraEntry {
    descriptor: NativeCameraDescriptor,
    motion: Option<RendererCameraMotion>,
    /// Starts at one because zero is never a renderer sample identity. This
    /// identity is retained per live camera, not supplied by the product.
    next_sample_id: u64,
    /// The product UI element its primary views follow, if any.
    viewport_anchor: Option<String>,
}

#[derive(Clone, Copy)]
struct CameraTargetState {
    descriptor: NativeCameraTargetDescriptor,
    revision: u64,
}

pub(crate) struct RuntimeCameraViewCall {
    state: CameraState,
    /// Whether a camera change this call publishes the composition.
    composition_changed: bool,
    pub(crate) background: Option<CameraBackground>,
    /// Fog selected during the call (`Some(None)` turns it off). The
    /// presentation world retains the selection.
    pub(crate) fog: Option<Option<FogDescriptor>>,
    pub(crate) tone_mapping: Option<ToneMappingDescriptor>,
    /// Bloom, auto exposure, colour grading, the atmosphere, sun shafts and
    /// the sky's light selected during the call, as fog is.
    pub(crate) bloom: Option<Option<BloomDescriptor>>,
    pub(crate) auto_exposure: Option<Option<AutoExposureDescriptor>>,
    pub(crate) color_grading: Option<Option<ColorGradingDescriptor>>,
    pub(crate) atmosphere: Option<Option<AtmosphereDescriptor>>,
    pub(crate) sun_shafts: Option<Option<SunShaftsDescriptor>>,
    pub(crate) moon: Option<Option<render_model::MoonDescriptor>>,
    /// The star field as the product asked for it; its resources are
    /// resolved when the frame is built.
    pub(crate) stars: Option<Option<NativeStarsRequest>>,
    pub(crate) wind: Option<Option<WindDescriptor>>,
    pub(crate) clouds: Option<Option<CloudsDescriptor>>,
    pub(crate) wetness: Option<Option<WetnessDescriptor>>,
    pub(crate) snow: Option<Option<SnowDescriptor>>,
    /// Backdrop links set (Some) or cleared (None) this call, by camera
    /// handle (0: every view's).
    pub(crate) backdrops: Vec<(u64, Option<BackdropLink>)>,
    /// The volumetric fog medium, and fog volumes placed (`Some`) or removed
    /// (`None`) in call order.
    pub(crate) volumetric_fog: Option<render_model::VolumetricFogDescriptor>,
    /// Cloud regions placed (`Some`) or removed (`None`) in call order.
    pub(crate) cloud_regions: Vec<(u32, Option<render_model::CloudRegionDescriptor>)>,
    pub(crate) fog_volumes: Vec<(u32, Option<render_model::FogVolumeDescriptor>)>,
    pub(crate) precipitation: Option<Option<PrecipitationDescriptor>>,
    /// The image effect as the product asked for it; its resources are
    /// resolved when the frame is built.
    pub(crate) image_effect: Option<Option<NativeImageEffectRequest>>,
    /// Decals placed (`Some`) or removed (`None`) in call order; their fade
    /// is stamped when the call is published.
    pub(crate) decals: Vec<(u64, Option<PendingDecal>)>,
    pub(crate) decal_atlas: Option<Option<NativeDecalAtlasRequest>>,
    pub(crate) indirect_light: Option<Option<IndirectLightDescriptor>>,
    pub(crate) sky_light: Option<Option<SkyLightDescriptor>>,
}

impl RuntimeCameraViewCall {
    /// The composition the call leaves, when a camera change published it.
    pub(crate) fn take_composition(&mut self) -> Option<RendererViewComposition> {
        std::mem::take(&mut self.composition_changed).then(|| self.state.composition.renderer())
    }

    /// Where a live camera stands, as the call leaves it.
    pub(crate) fn camera_position(&self, camera: u64) -> Option<[f64; 3]> {
        let position = self.state.cameras.get(&camera)?.descriptor.pose.position;
        Some([position.x, position.y, position.z].map(f64::from))
    }

    /// The camera of the lowest-ordered primary view and where it stands,
    /// as the call leaves it.
    pub(crate) fn primary_camera(&self) -> Option<(u64, [f64; 3])> {
        let (_, view) = self
            .state
            .views
            .iter()
            .enumerate()
            .filter(|(_, view)| view.target.value == 0)
            .min_by_key(|(index, view)| (view.order, *index))?;
        let position = self
            .state
            .cameras
            .get(&view.camera.value)?
            .descriptor
            .pose
            .position;
        Some((
            view.camera.value,
            [position.x, position.y, position.z].map(f64::from),
        ))
    }
}

/// Engine-owned typed camera/view projection. Product facts are copied at the
/// ABI edge; this owner derives private renderer identifiers and publishes the
/// complete active view against the current host surface.
/// A backdrop link as the product gave it: an exact world anchor, resolved
/// into the local frame of `session` (0: none, the anchor is local) each
/// time that session's origin moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BackdropLink {
    session: u64,
    anchor: core_space::GlobalPosition,
    origin: [f64; 3],
    scale: f64,
}

pub(crate) struct RuntimeCameraViewBridge {
    state: CameraState,
    /// Every live backdrop link by camera handle (0: every view's), and the
    /// renderer descriptors last emitted for them.
    backdrop_links: BTreeMap<u64, BackdropLink>,
    backdrop_emitted: BTreeMap<u64, render_model::BackdropDescriptor>,
    /// The spatial sessions' world origins, which world anchors resolve in.
    spatial: Option<crate::spatial::SpatialCollisionSource>,
    surface: NativeCameraSurfaceReadout,
    /// The anchored UI rects of the same report as `surface`.
    viewport_anchors: render_host_contracts::RendererViewportAnchors,
    staged: Option<RuntimeCameraViewCall>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
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
                background: CameraBackground::Default,
                stars: None,
                snow: None,
                decals: BTreeMap::new(),
                decal_atlas: None,
                next_camera: 1,
                next_target: 1,
                composition: RetainedComposition::default(),
            },
            // Until the runtime reports otherwise, assume someone watches.
            surface: NativeCameraSurfaceReadout {
                watching: true,
                ..Default::default()
            },
            viewport_anchors: BTreeMap::new(),
            staged: None,
            operation_diagnostics: Default::default(),
            backdrop_links: BTreeMap::new(),
            backdrop_emitted: BTreeMap::new(),
            spatial: None,
        }
    }

    pub(crate) fn bind_spatial(&mut self, spatial: crate::spatial::SpatialCollisionSource) {
        self.spatial = Some(spatial);
    }

    /// The backdrop links this call set or cleared, applied, then every
    /// link resolved into its session's current frame: a `SetBackdrop` for
    /// each whose descriptor changed (a new link, or its session's origin
    /// moved) and a removal for each that went. A link whose session ended
    /// is dropped.
    pub(crate) fn backdrop_operations(&mut self, call: &RuntimeCameraViewCall) -> Vec<RenderDiff> {
        for (camera, link) in &call.backdrops {
            match link {
                Some(link) => {
                    self.backdrop_links.insert(*camera, *link);
                }
                None => {
                    self.backdrop_links.remove(camera);
                }
            }
        }
        let spatial = self.spatial.as_ref();
        let desired: BTreeMap<u64, render_model::BackdropDescriptor> = self
            .backdrop_links
            .iter()
            .filter_map(|(camera, link)| {
                let origin = if link.session == 0 {
                    core_space::WorldOrigin::ZERO
                } else {
                    spatial?.world_origin(NativeSpatialSessionHandle {
                        value: link.session,
                    })?
                };
                let cell = origin.cell();
                let anchor: [f64; 3] = std::array::from_fn(|axis| {
                    (i128::from(link.anchor.cell()[axis]) - i128::from(cell[axis])) as f64
                        + link.anchor.offset()[axis]
                });
                Some((
                    *camera,
                    render_model::BackdropDescriptor {
                        anchor,
                        origin: link.origin,
                        scale: link.scale,
                    },
                ))
            })
            .collect();
        let camera_id = |camera: u64| (camera != 0).then(|| format!("csharp-camera-{camera}"));
        let mut operations = Vec::new();
        for (camera, backdrop) in &desired {
            if self.backdrop_emitted.get(camera) != Some(backdrop) {
                operations.push(RenderDiff::SetBackdrop {
                    camera: camera_id(*camera),
                    backdrop: Some(*backdrop),
                });
            }
        }
        for camera in self.backdrop_emitted.keys() {
            if !desired.contains_key(camera) {
                operations.push(RenderDiff::SetBackdrop {
                    camera: camera_id(*camera),
                    backdrop: None,
                });
            }
        }
        self.backdrop_emitted = desired;
        operations
    }

    /// Where a camera at world `position` stands in the backdrop under
    /// the link its views take (its own, else every view's) as last
    /// resolved by `backdrop_operations`; None without one.
    pub(crate) fn backdrop_eye(&self, camera: u64, position: [f64; 3]) -> Option<[f64; 3]> {
        self.backdrop_emitted
            .get(&camera)
            .or_else(|| self.backdrop_emitted.get(&0))
            .map(|backdrop| backdrop.eye(position))
    }

    pub(crate) fn begin_call(&mut self) {
        // The call owns the state until it finishes; nothing is copied.
        self.staged = Some(RuntimeCameraViewCall {
            state: std::mem::take(&mut self.state),
            composition_changed: false,
            background: None,
            fog: None,
            tone_mapping: None,
            bloom: None,
            auto_exposure: None,
            color_grading: None,
            atmosphere: None,
            sun_shafts: None,
            moon: None,
            stars: None,
            wind: None,
            clouds: None,
            wetness: None,
            snow: None,
            backdrops: Vec::new(),
            volumetric_fog: None,
            cloud_regions: Vec::new(),
            fog_volumes: Vec::new(),
            precipitation: None,
            image_effect: None,
            decals: Vec::new(),
            decal_atlas: None,
            indirect_light: None,
            sky_light: None,
        });
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeCameraViewCall, CsharpEngineServicesError> {
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

    /// Appearance resource release must observe a sky selected earlier in the
    /// same product callback as well as the committed retained selection.
    pub(crate) fn uses_sky_texture(&self, resource: u64) -> bool {
        let state = self
            .staged
            .as_ref()
            .map(|staged| &staged.state)
            .unwrap_or(&self.state);
        let stars = state
            .stars
            .is_some_and(|stars| stars.catalogue.value == resource || stars.band.value == resource);
        let decals = state
            .decal_atlas
            .is_some_and(|atlas| atlas.color.value == resource || atlas.normals.value == resource);
        stars
            || decals
            || match state.background {
                CameraBackground::Sky(id) => id == resource,
                CameraBackground::Blend { first, second, .. } => {
                    first == resource || second == resource
                }
                _ => false,
            }
    }

    /// The committed camera composition, without entering a product
    /// callback. Sky is retained by the presentation world alongside
    /// graphics resources.
    pub(crate) fn snapshot_composition(&self) -> RendererViewComposition {
        self.state.composition.renderer()
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeCameraViewCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_CAMERA_VIEW_CALL",
                "camera/view service was called outside a product call",
            )
        })
    }

    // Each change below checks everything that can refuse it before it
    // changes anything, so a refused change leaves the call's cameras,
    // targets and composition as they were.

    fn create(
        &mut self,
        descriptor: NativeCameraDescriptor,
    ) -> Result<NativeCameraHandle, CsharpEngineServicesError> {
        validate_descriptor(&descriptor)?;
        let staged = self.staged_mut()?;
        let handle = staged.state.next_camera;
        staged.state.next_camera = handle + 1;
        staged.state.cameras.insert(
            handle,
            CameraEntry {
                descriptor,
                motion: None,
                next_sample_id: 1,
                viewport_anchor: None,
            },
        );
        // No view names the new camera yet, so the composition is unchanged.
        Ok(NativeCameraHandle { value: handle })
    }

    fn update(
        &mut self,
        request: NativeCameraUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        validate_descriptor(&request.descriptor)?;
        let staged = self.staged_mut()?;
        let camera = staged
            .state
            .cameras
            .get_mut(&request.camera.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle is not live")
            })?;
        camera.descriptor = request.descriptor;
        // The established immediate update path explicitly drops retained
        // sampling state, so it cannot accidentally keep smoothing a new pose.
        camera.motion = None;
        staged.state.patch_camera(request.camera.value);
        staged.composition_changed = true;
        Ok(())
    }

    fn update_sample(
        &mut self,
        request: NativeCameraSampleRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        validate_descriptor(&request.descriptor)?;
        let motion = sample_motion(request)?;
        let staged = self.staged_mut()?;
        let camera = staged
            .state
            .cameras
            .get_mut(&request.camera.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle is not live")
            })?;
        if let Some(mut motion) = motion {
            let next_sample_id = camera.next_sample_id + 1;
            motion.sample_id = camera.next_sample_id.to_string();
            camera.next_sample_id = next_sample_id;
            camera.motion = Some(motion);
        } else {
            camera.motion = None;
        }
        camera.descriptor = request.descriptor;
        staged.state.patch_camera(request.camera.value);
        staged.composition_changed = true;
        Ok(())
    }

    fn replace(
        &mut self,
        request: NativeCameraReplaceRequest,
    ) -> Result<NativeCameraHandle, CsharpEngineServicesError> {
        validate_descriptor(&request.replacement)?;
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        if !state.cameras.contains_key(&request.camera.value) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CAMERA_HANDLE",
                "camera handle is not live",
            ));
        }
        let replacement = state.next_camera;
        state.next_camera = replacement + 1;
        state.cameras.remove(&request.camera.value);
        state.cameras.insert(
            replacement,
            CameraEntry {
                descriptor: request.replacement,
                motion: None,
                next_sample_id: 1,
                viewport_anchor: None,
            },
        );
        for view in &mut state.views {
            if view.camera == request.camera {
                view.camera = NativeCameraHandle { value: replacement };
            }
        }
        if state.active_camera == Some(request.camera.value) {
            state.active_camera = Some(replacement);
            if let Some(view) = state.views.first_mut() {
                view.viewport = request.replacement.viewport;
            }
        }
        state.rebuild_composition();
        staged.composition_changed = true;
        Ok(NativeCameraHandle { value: replacement })
    }

    fn create_target(
        &mut self,
        descriptor: NativeCameraTargetDescriptor,
    ) -> Result<NativeCameraTargetHandle, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let handle = staged.state.next_target;
        let next_target = handle + 1;
        let target = CameraTargetState {
            descriptor,
            revision: 1,
        };
        // No view names the new target yet, so it takes none of the budget.
        composition_target(handle, target)
            .validate()
            .map_err(target_descriptor_error)?;
        staged.state.next_target = next_target;
        // Nor does any view name the new target.
        staged.state.targets.insert(handle, target);
        Ok(NativeCameraTargetHandle { value: handle })
    }

    fn update_target(
        &mut self,
        request: NativeCameraTargetUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        let handle = request.target.value;
        let current = state.targets.get(&handle).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_CAMERA_TARGET_HANDLE",
                "target handle is not live",
            )
        })?;
        let target = CameraTargetState {
            descriptor: request.descriptor,
            revision: current.revision + 1,
        };
        let rendered = composition_target(handle, target);
        rendered.validate().map_err(target_descriptor_error)?;
        state.composition.check_target_budget(handle, &rendered)?;
        state.targets.insert(handle, target);
        state.patch_target(handle, rendered);
        staged.composition_changed = true;
        Ok(())
    }

    fn replace_target(
        &mut self,
        request: NativeCameraTargetReplaceRequest,
    ) -> Result<NativeCameraTargetHandle, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        if !state.targets.contains_key(&request.target.value) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CAMERA_TARGET_HANDLE",
                "target handle is not live",
            ));
        }
        let handle = state.next_target;
        let next_target = handle + 1;
        let target = CameraTargetState {
            descriptor: request.replacement,
            revision: 1,
        };
        let rendered = composition_target(handle, target);
        rendered.validate().map_err(target_descriptor_error)?;
        // The replacement takes the old target's place in every view.
        state
            .composition
            .check_target_budget(request.target.value, &rendered)?;
        state.next_target = next_target;
        state.targets.remove(&request.target.value);
        state.targets.insert(handle, target);
        let replacement = NativeCameraTargetHandle { value: handle };
        for view in &mut state.views {
            if view.target.value == request.target.value {
                view.target = NativeCameraTargetReference { value: handle };
            }
        }
        for presentation in &mut state.presentations {
            if presentation.source_target == request.target {
                presentation.source_target = replacement;
            }
        }
        state.rebuild_composition();
        staged.composition_changed = true;
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
        let state = &mut staged.state;
        state.targets.remove(&target.value);
        state.views.retain(|view| view.target.value != target.value);
        state
            .presentations
            .retain(|presentation| presentation.source_target != target);
        state.rebuild_composition();
        staged.composition_changed = true;
        Ok(())
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
        let state = &mut staged.state;
        let target_refused = |detail: &str| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_COMPOSITION_TARGET", detail)
        };
        for view in views {
            if !state.cameras.contains_key(&view.camera.value) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_CAMERA_COMPOSITION_CAMERA",
                    "composition view names a camera that is not live",
                ));
            }
            if view.target.value != 0 && !state.targets.contains_key(&view.target.value) {
                return Err(target_refused(
                    "composition view names a target that is not live",
                ));
            }
        }
        for presentation in presentations {
            if !state
                .targets
                .contains_key(&presentation.source_target.value)
            {
                return Err(target_refused(
                    "composition presentation names a target that is not live",
                ));
            }
        }
        // The cameras and targets were checked when they entered; the
        // product's viewports and orders, and the targets' budget
        // together, are checked here.
        let composition =
            RetainedComposition::build(&state.cameras, &state.targets, views, presentations);
        composition
            .views
            .iter()
            .try_for_each(RendererCompositionView::validate)
            .and_then(|()| {
                composition
                    .presentations
                    .iter()
                    .try_for_each(RendererCompositionPresentation::validate)
            })
            .and_then(|()| {
                render_host_contracts::validate_target_pixels(composition.targets.values())
            })
            .map_err(composition_error)?;
        state.views = views.to_vec();
        state.presentations = presentations.to_vec();
        state.active_camera = None;
        state.composition = composition;
        staged.composition_changed = true;
        Ok(())
    }

    fn destroy(&mut self, camera: NativeCameraHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        // Replacement turns the prior owner into a tombstone. Its generated
        // IDisposable must remain safe to release in normal owner-first or
        // replacement-first teardown order.
        state.cameras.remove(&camera.value);
        state.views.retain(|view| view.camera != camera);
        if state.active_camera == Some(camera.value) {
            state.active_camera = None;
        }
        state.rebuild_composition();
        staged.composition_changed = true;
        Ok(())
    }

    fn set_active(&mut self, camera: NativeCameraHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        let entry = state.cameras.get(&camera.value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle is not live")
        })?;
        state.views = vec![NativeCameraCompositionView {
            camera,
            target: NativeCameraTargetReference::default(),
            viewport: entry.descriptor.viewport,
            order: 0,
        }];
        state.presentations.clear();
        state.active_camera = Some(camera.value);
        state.rebuild_composition();
        staged.composition_changed = true;
        Ok(())
    }

    fn clear_active(&mut self) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        state.views.clear();
        state.presentations.clear();
        state.active_camera = None;
        state.composition = RetainedComposition::default();
        staged.composition_changed = true;
        Ok(())
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
        staged.state.background = CameraBackground::Sky(texture.value);
        staged.background = Some(staged.state.background);
        Ok(())
    }

    fn clear_sky(&mut self) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        staged.state.background = CameraBackground::Default;
        staged.background = Some(staged.state.background);
        Ok(())
    }

    fn set_background_color(
        &mut self,
        request: NativeSetBackgroundColorRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let color = [
            request.color.r,
            request.color.g,
            request.color.b,
            request.color.a,
        ];
        if !color
            .iter()
            .all(|component| component.is_finite() && *component >= 0.0)
            || request.color.a != 1.0
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_BACKGROUND_COLOR",
                "background color must be finite, non-negative and opaque",
            ));
        }
        let staged = self.staged_mut()?;
        staged.state.background = CameraBackground::Color(color);
        staged.background = Some(staged.state.background);
        Ok(())
    }

    fn set_fog(&mut self, request: NativeFogRequest) -> Result<(), CsharpEngineServicesError> {
        let color = [request.color.r, request.color.g, request.color.b];
        let fog = match request.mode {
            NativeFogMode::Off => None,
            NativeFogMode::Linear => Some(FogDescriptor::Linear {
                color,
                start: request.start,
                end: request.end,
            }),
            NativeFogMode::Exponential => Some(FogDescriptor::Exponential {
                color,
                density: request.density,
            }),
            NativeFogMode::ExponentialSquared => Some(FogDescriptor::ExponentialSquared {
                color,
                density: request.density,
            }),
        };
        if fog.is_some_and(|fog| !fog.valid()) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_FOG",
                "fog needs a finite non-negative colour; linear fog 0 <= start < end; \
                 exponential fog density > 0",
            ));
        }
        self.staged_mut()?.fog = Some(fog);
        Ok(())
    }

    pub(crate) fn set_surface(
        &mut self,
        surface: NativeCameraSurfaceReadout,
        viewport_anchors: render_host_contracts::RendererViewportAnchors,
    ) {
        self.surface = surface;
        self.viewport_anchors = viewport_anchors;
    }

    fn viewport_anchor(&self, anchor: &str) -> NativeCameraViewportAnchorReadout {
        self.viewport_anchors.get(anchor).map_or_else(
            NativeCameraViewportAnchorReadout::default,
            |rect| NativeCameraViewportAnchorReadout {
                reported: true,
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                revision: self.surface.revision,
            },
        )
    }

    /// Makes `camera`'s primary views follow the product UI element anchored
    /// under `anchor`, or their own viewports again with an empty name.
    fn set_viewport_anchor(
        &mut self,
        camera: NativeCameraHandle,
        anchor: &str,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let entry = staged.state.cameras.get_mut(&camera.value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_CAMERA_HANDLE", "camera handle is not live")
        })?;
        entry.viewport_anchor = (!anchor.is_empty()).then(|| anchor.to_owned());
        staged.state.rebuild_composition();
        staged.composition_changed = true;
        Ok(())
    }

    fn set_tone_mapping(
        &mut self,
        request: NativeToneMappingRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        if !request.exposure.is_finite() || request.exposure < 0.0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_TONE_MAPPING",
                "tone mapping exposure must be finite and non-negative",
            ));
        }
        self.staged_mut()?.tone_mapping = Some(ToneMappingDescriptor {
            operator: match request.operator {
                NativeToneMappingOperator::None => ToneMappingOperator::None,
                NativeToneMappingOperator::Neutral => ToneMappingOperator::Neutral,
                NativeToneMappingOperator::AcesFilmic => ToneMappingOperator::AcesFilmic,
            },
            exposure: request.exposure,
        });
        Ok(())
    }
}

impl RuntimeCameraViewBridge {
    fn set_bloom(&mut self, request: NativeBloomRequest) -> Result<(), CsharpEngineServicesError> {
        let bloom = BloomDescriptor {
            threshold: request.threshold,
            intensity: request.intensity,
        };
        if !bloom.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_BLOOM",
                "bloom threshold and intensity must be finite and non-negative",
            ));
        }
        self.staged_mut()?.bloom = Some((request.intensity > 0.0).then_some(bloom));
        Ok(())
    }

    fn set_auto_exposure(
        &mut self,
        request: NativeAutoExposureRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let auto_exposure = AutoExposureDescriptor {
            speed: request.speed,
            min_exposure: request.min_exposure,
            max_exposure: request.max_exposure,
        };
        if request.enabled && !auto_exposure.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUTO_EXPOSURE",
                "auto exposure speed must be positive and its exposure range positive and ordered",
            ));
        }
        self.staged_mut()?.auto_exposure = Some(request.enabled.then_some(auto_exposure));
        Ok(())
    }

    fn set_color_grading(
        &mut self,
        request: NativeColorGradingRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let grading = ColorGradingDescriptor {
            temperature: request.temperature,
            tint: request.tint,
            contrast: request.contrast,
            saturation: request.saturation,
        };
        if !grading.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_COLOR_GRADING",
                "colour grading temperature, tint, contrast and saturation must each be within -1 to 1",
            ));
        }
        // All zero grades nothing: the finish skips grading.
        let graded = grading != ColorGradingDescriptor::default();
        self.staged_mut()?.color_grading = Some(graded.then_some(grading));
        Ok(())
    }

    fn set_atmosphere(
        &mut self,
        request: NativeAtmosphereRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let atmosphere = AtmosphereDescriptor {
            fog_base_height: request.fog_base_height,
            fog_falloff_height: request.fog_falloff_height,
            haze_color: [
                request.haze_color.r,
                request.haze_color.g,
                request.haze_color.b,
            ],
            haze_exponent: request.haze_exponent,
            sun_radius_degrees: request.sun_radius_degrees,
            sun_halo: request.sun_halo,
            sun_radiance: request.sun_radiance,
            sky_sun_direction: [
                request.sky_sun_direction.x,
                request.sky_sun_direction.y,
                request.sky_sun_direction.z,
            ],
            sky_sun_color: [
                request.sky_sun_color.r,
                request.sky_sun_color.g,
                request.sky_sun_color.b,
            ],
            sky_sun_intensity: request.sky_sun_intensity,
        };
        if !atmosphere.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ATMOSPHERE",
                "atmosphere values must be finite, and all but the fog base height and sky sun direction non-negative",
            ));
        }
        // All zero is no atmosphere.
        let set = atmosphere != AtmosphereDescriptor::default();
        self.staged_mut()?.atmosphere = Some(set.then_some(atmosphere));
        Ok(())
    }

    fn set_sun_shafts(
        &mut self,
        request: NativeSunShaftsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let shafts = SunShaftsDescriptor {
            intensity: request.intensity,
            length: request.length.clamp(0.0, 1.0),
        };
        if !shafts.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SUN_SHAFTS",
                "sun shafts intensity must be finite and non-negative, and length finite",
            ));
        }
        self.staged_mut()?.sun_shafts = Some((request.intensity > 0.0).then_some(shafts));
        Ok(())
    }

    fn set_moon(&mut self, request: NativeMoonRequest) -> Result<(), CsharpEngineServicesError> {
        let moon = render_model::MoonDescriptor {
            direction: [
                request.direction.x,
                request.direction.y,
                request.direction.z,
            ],
            radius_degrees: request.radius_degrees,
            color: [request.color.x, request.color.y, request.color.z],
            radiance: request.radiance,
            phase: request.phase.clamp(0.0, 1.0),
            earthshine: request.earthshine.clamp(0.0, 1.0),
            halo: request.halo,
            light_intensity: request.light_intensity,
        };
        // Radiance 0 (the default request) takes the moon away, whatever else.
        if request.radiance == 0.0 {
            self.staged_mut()?.moon = Some(None);
            return Ok(());
        }
        if !moon.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MOON",
                "the moon needs a finite direction with some length, and finite values otherwise, all but the phase and earthshine non-negative",
            ));
        }
        self.staged_mut()?.moon = Some(Some(moon));
        Ok(())
    }

    fn set_stars(&mut self, request: NativeStarsRequest) -> Result<(), CsharpEngineServicesError> {
        // Catalogue 0 (the default request) takes the field away.
        if request.catalogue.value == 0 {
            let staged = self.staged_mut()?;
            staged.stars = Some(None);
            staged.state.stars = None;
            return Ok(());
        }
        let [x, y, z, w] = [
            request.orientation.x,
            request.orientation.y,
            request.orientation.z,
            request.orientation.w,
        ];
        let unit = 0.0..=1.0;
        let valid = [x, y, z, w].iter().all(|value| value.is_finite())
            && x * x + y * y + z * z + w * w > 0.0
            && request.limiting_magnitude.is_finite()
            && request.limiting_magnitude.abs() <= render_model::MAX_STAR_MAGNITUDE
            && (0.0..=render_model::StarsDescriptor::MAX_BRIGHTNESS).contains(&request.brightness)
            && unit.contains(&request.band_strength)
            && unit.contains(&request.twinkle);
        if !valid {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_STARS",
                "a star field needs a finite orientation with some length, a limiting magnitude within -30 to 30, a brightness within 0 to 1000, and a band strength and twinkle within 0 to 1",
            ));
        }
        let staged = self.staged_mut()?;
        staged.stars = Some(Some(request));
        staged.state.stars = Some(request);
        Ok(())
    }

    fn set_wind(&mut self, request: NativeWindRequest) -> Result<(), CsharpEngineServicesError> {
        let wind = WindDescriptor {
            direction: [request.direction.x, request.direction.y],
            strength: request.strength,
            gust: request.gust.clamp(0.0, 1.0),
        };
        if !wind.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_WIND",
                "wind direction must be finite with some length, strength finite and non-negative and gust finite",
            ));
        }
        self.staged_mut()?.wind = Some((request.strength > 0.0).then_some(wind));
        Ok(())
    }

    fn set_clouds(
        &mut self,
        request: NativeCloudsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let clouds = CloudsDescriptor {
            coverage: request.coverage.clamp(0.0, 1.0),
            drift: [request.drift.x, request.drift.y],
            altitude: request.altitude,
            scale: request.scale,
            color: [request.color.x, request.color.y, request.color.z],
            thickness: request.thickness,
            kind: cloud_kind(request.kind),
            twilight_degrees: request.twilight_degrees,
        };
        if !clouds.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CLOUDS",
                "clouds need a finite coverage and drift, an altitude and scale above 0, and a non-negative thickness, colour and twilight, all finite",
            ));
        }
        // Coverage 0 draws no layer, but its altitude, scale, drift, kind and
        // twilight still shape the cloud regions under it.
        self.staged_mut()?.clouds = Some(Some(clouds));
        Ok(())
    }

    fn set_wetness(
        &mut self,
        request: NativeWetnessRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let wetness = WetnessDescriptor {
            wetness: request.wetness.clamp(0.0, 1.0),
            puddles: request.puddles.clamp(0.0, 1.0),
        };
        if !wetness.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_WETNESS",
                "wetness and puddles must each be finite",
            ));
        }
        self.staged_mut()?.wetness = Some((wetness.wetness > 0.0).then_some(wetness));
        Ok(())
    }

    fn set_snow(&mut self, request: NativeSnowRequest) -> Result<(), CsharpEngineServicesError> {
        let color = [request.color.x, request.color.y, request.color.z];
        let snow = SnowDescriptor {
            amount: request.amount.clamp(0.0, 1.0),
            color: if color == [0.0; 3] { SNOW_WHITE } else { color },
            texels_per_metre: request.texels_per_metre,
        };
        if !snow.amount.is_finite()
            || !snow
                .color
                .iter()
                .chain([&snow.texels_per_metre])
                .all(|value| value.is_finite() && *value >= 0.0)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SNOW",
                "the snow's amount must be finite, and its colour and texels per metre finite and not negative",
            ));
        }
        let snow = (snow.amount > 0.0).then_some(snow);
        let staged = self.staged_mut()?;
        staged.state.snow = snow;
        staged.snow = Some(snow);
        Ok(())
    }

    /// The snow in force as this call leaves it.
    fn read_snow(&self) -> NativeSnowReadout {
        let state = self
            .staged
            .as_ref()
            .map(|staged| &staged.state)
            .unwrap_or(&self.state);
        state
            .snow
            .map_or_else(Default::default, |snow| NativeSnowReadout {
                amount: snow.amount,
                color: NativeVec3 {
                    x: snow.color[0],
                    y: snow.color[1],
                    z: snow.color[2],
                },
                texels_per_metre: snow.texels_per_metre,
            })
    }

    fn set_backdrop(
        &mut self,
        request: NativeBackdropRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let refused = |message: &str| CsharpEngineServicesError::new("CSHARP_BACKDROP", message);
        let anchor = core_space::GlobalPosition::new(
            [
                request.anchor.cell_x,
                request.anchor.cell_y,
                request.anchor.cell_z,
            ],
            [
                request.anchor.offset_x,
                request.anchor.offset_y,
                request.anchor.offset_z,
            ],
        )
        .map_err(|_| refused("the backdrop's anchor must be a finite world position"))?;
        let origin = [request.origin.x, request.origin.y, request.origin.z].map(f64::from);
        let scale = f64::from(request.scale);
        if !origin.iter().all(|value| value.is_finite()) || !scale.is_finite() || scale <= 0.0 {
            return Err(refused(
                "the backdrop's origin must be finite and its scale above 0",
            ));
        }
        let session = request.session.session;
        if session.value != 0
            && self
                .spatial
                .as_ref()
                .and_then(|spatial| spatial.world_origin(session))
                .is_none()
        {
            return Err(refused(
                "the backdrop's anchor names a spatial session that does not live",
            ));
        }
        let camera = request.camera.camera.value;
        let staged = self.staged_mut()?;
        if camera != 0 && !staged.state.cameras.contains_key(&camera) {
            return Err(refused("the backdrop names a camera that does not live"));
        }
        staged.backdrops.push((
            camera,
            Some(BackdropLink {
                session: session.value,
                anchor,
                origin,
                scale,
            }),
        ));
        Ok(())
    }

    fn clear_backdrop(
        &mut self,
        request: NativeClearBackdropRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        self.staged_mut()?
            .backdrops
            .push((request.camera.camera.value, None));
        Ok(())
    }

    fn set_cloud_region(
        &mut self,
        request: NativeCloudRegionRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let region = render_model::CloudRegionDescriptor {
            center: [request.center.x, request.center.y],
            radius: request.radius,
            coverage: request.coverage.clamp(0.0, 1.0),
            darkness: request.darkness.clamp(0.0, 1.0),
            drift: [request.drift.x, request.drift.y],
            kind: cloud_kind(request.kind),
            thickness: request.thickness,
        };
        if !region.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CLOUD_REGION",
                "a cloud region needs a finite centre, drift, coverage and darkness, a finite radius above 0 and a finite non-negative thickness",
            ));
        }
        self.staged_mut()?
            .cloud_regions
            .push((request.id, Some(region)));
        Ok(())
    }

    fn remove_cloud_region(
        &mut self,
        request: NativeCloudRegionRemoval,
    ) -> Result<(), CsharpEngineServicesError> {
        self.staged_mut()?.cloud_regions.push((request.id, None));
        Ok(())
    }

    fn set_volumetric_fog(
        &mut self,
        request: NativeVolumetricFogRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let fog = render_model::VolumetricFogDescriptor {
            density: request.density,
            albedo: [request.albedo.x, request.albedo.y, request.albedo.z],
            anisotropy: request.anisotropy,
            base_height: request.base_height,
            falloff_height: request.falloff_height,
            distance: request.distance,
            ambient: request.ambient,
        };
        if !fog.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOLUMETRIC_FOG",
                "volumetric fog needs finite values: non-negative density, albedo, falloff height and ambient, anisotropy within -0.9 to 0.9 and a distance above 0",
            ));
        }
        self.staged_mut()?.volumetric_fog = Some(fog);
        Ok(())
    }

    fn set_fog_volume(
        &mut self,
        request: NativeFogVolumeRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let vec3 = |value: NativeVec3| [value.x, value.y, value.z];
        let volume = render_model::FogVolumeDescriptor {
            shape: match request.shape {
                NativeFogVolumeShape::Box => render_model::FogVolumeShape::Box,
                NativeFogVolumeShape::Ellipsoid => render_model::FogVolumeShape::Ellipsoid,
            },
            center: vec3(request.center),
            half_extents: vec3(request.half_extents),
            yaw_degrees: request.yaw_degrees,
            density: request.density,
            albedo: vec3(request.albedo),
            emission: vec3(request.emission),
            edge: request.edge.clamp(0.0, 1.0),
            noise_scale: request.noise_scale,
            noise_strength: request.noise_strength.clamp(0.0, 1.0),
            noise_velocity: vec3(request.noise_velocity),
        };
        if !volume.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_FOG_VOLUME",
                "a fog volume needs finite values: half extents above 0, and a non-negative density, albedo, emission and noise scale",
            ));
        }
        self.staged_mut()?
            .fog_volumes
            .push((request.id, Some(volume)));
        Ok(())
    }

    fn set_decal(&mut self, request: NativeDecalRequest) -> Result<(), CsharpEngineServicesError> {
        let refused =
            |message: &'static str| CsharpEngineServicesError::new("CSHARP_DECAL", message);
        let vec3 = |value: NativeVec3| [value.x, value.y, value.z];
        let mut rotation = [
            request.rotation.x,
            request.rotation.y,
            request.rotation.z,
            request.rotation.w,
        ];
        if rotation == [0.0; 4] {
            rotation = [0.0, 0.0, 0.0, 1.0];
        }
        let mut uv_rect = [
            request.region.x,
            request.region.y,
            request.region.z,
            request.region.w,
        ];
        if uv_rect == [0.0; 4] {
            uv_rect = [0.0, 0.0, 1.0, 1.0];
        }
        let color = [
            request.color.r,
            request.color.g,
            request.color.b,
            request.color.a.clamp(0.0, 1.0),
        ];
        let center = vec3(request.center);
        let size = vec3(request.size);
        let finite = center
            .iter()
            .chain(&rotation)
            .chain(&uv_rect)
            .chain(&color)
            .chain([&request.roughness, &request.normal_strength])
            .all(|value| value.is_finite());
        let times = [request.life_seconds, request.fade_seconds];
        if !finite
            || !size.iter().all(|value| value.is_finite() && *value > 0.0)
            || !times.iter().all(|value| value.is_finite() && *value >= 0.0)
        {
            return Err(refused(
                "a decal needs finite values: a size above 0 along each axis, and a life and fade of 0 or more seconds",
            ));
        }
        let staged = self.staged_mut()?;
        if !staged.state.decals.contains_key(&request.id)
            && staged.state.decals.len() >= render_model::DecalDescriptor::MAX_DECALS
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DECAL_BUDGET",
                "a scene holds at most 4096 decals; remove one before placing another",
            ));
        }
        staged.state.decals.insert(request.id, 0.0);
        staged.decals.push((
            request.id,
            Some(PendingDecal {
                decal: render_model::DecalDescriptor {
                    center,
                    rotation,
                    size,
                    uv_rect,
                    color,
                    roughness: request.roughness.clamp(0.0, 1.0),
                    normal_strength: request.normal_strength.max(0.0),
                    angle_falloff: request.angle_falloff,
                    on_vegetation: request.on_vegetation,
                    on_animated: request.on_animated_meshes,
                    fade_start: 0.0,
                    fade_end: 0.0,
                },
                life: f64::from(request.life_seconds),
                fade: f64::from(request.fade_seconds),
            }),
        ));
        Ok(())
    }

    fn remove_decal(
        &mut self,
        request: NativeDecalRemoval,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged.state.decals.remove(&request.id).is_some() {
            staged.decals.push((request.id, None));
        }
        Ok(())
    }

    fn set_decal_atlas(
        &mut self,
        request: NativeDecalAtlasRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let atlas = (request.color.value != 0).then_some(request);
        let staged = self.staged_mut()?;
        staged.decal_atlas = Some(atlas);
        staged.state.decal_atlas = atlas;
        Ok(())
    }

    /// The decals placed as this call leaves them.
    fn read_decals(&self) -> NativeDecalReadout {
        let state = self
            .staged
            .as_ref()
            .map(|staged| &staged.state)
            .unwrap_or(&self.state);
        NativeDecalReadout {
            decals: state.decals.len() as u32,
            budget: render_model::DecalDescriptor::MAX_DECALS as u32,
        }
    }

    fn remove_fog_volume(
        &mut self,
        request: NativeFogVolumeRemoval,
    ) -> Result<(), CsharpEngineServicesError> {
        self.staged_mut()?.fog_volumes.push((request.id, None));
        Ok(())
    }

    fn set_image_effect(
        &mut self,
        request: NativeImageEffectRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let finite = [
            request.parameter_0,
            request.parameter_1,
            request.parameter_2,
            request.parameter_3,
        ]
        .iter()
        .all(|row| {
            [row.x, row.y, row.z, row.w]
                .iter()
                .all(|value| value.is_finite())
        });
        if !finite {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_IMAGE_EFFECT",
                "an image effect's parameters must be finite",
            ));
        }
        self.staged_mut()?.image_effect = Some((request.shader.value != 0).then_some(request));
        Ok(())
    }

    fn set_precipitation(
        &mut self,
        request: NativePrecipitationRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let precipitation = PrecipitationDescriptor {
            drops: request.drops,
            shape: match request.shape {
                NativePrecipitationShape::Streak => PrecipitationShape::Streak,
                NativePrecipitationShape::Flake => PrecipitationShape::Flake,
            },
            velocity: [request.velocity.x, request.velocity.y, request.velocity.z],
            size: request.size,
            streak_seconds: request.streak_seconds,
            color: [
                request.color.r,
                request.color.g,
                request.color.b,
                request.color.a,
            ],
            additive: request.additive,
            radius: request.radius,
            height: request.height,
        };
        if !precipitation.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRECIPITATION",
                "precipitation needs at most 200000 drops and finite values: a size above 0, a non-negative streak time and colour with alpha within 0 to 1, and a radius and height of at least 1 m",
            ));
        }
        self.staged_mut()?.precipitation = Some((request.drops > 0).then_some(precipitation));
        Ok(())
    }

    fn set_indirect_light(
        &mut self,
        request: NativeIndirectLightRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let descriptor = IndirectLightDescriptor {
            center: [request.center.x, request.center.y, request.center.z],
            extent: [request.extent.x, request.extent.y, request.extent.z],
            spacing: request.spacing,
            bounces: request.bounces,
            ambient: match request.ambient {
                NativeIndirectAmbient::Sky => IndirectAmbient::Sky,
                NativeIndirectAmbient::Floor => IndirectAmbient::Floor,
            },
        };
        let off = descriptor.extent.iter().all(|value| *value == 0.0);
        if !off && !descriptor.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_INDIRECT_LIGHT",
                "indirect light needs a finite centre and extent, a spacing from 0.5 to 8 m, 1 to 4 bounces and at most 262,144 probes",
            ));
        }
        self.staged_mut()?.indirect_light = Some((!off).then_some(descriptor));
        Ok(())
    }

    fn set_sky_light(
        &mut self,
        request: NativeSkyLightRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let sky_light = SkyLightDescriptor {
            intensity: request.intensity,
        };
        if !sky_light.valid() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SKY_LIGHT",
                "sky light intensity must be finite and non-negative",
            ));
        }
        self.staged_mut()?.sky_light = Some((request.intensity > 0.0).then_some(sky_light));
        Ok(())
    }
}

impl CameraState {
    /// Builds the composition again after a change to which cameras and
    /// targets the views and presentations name.
    fn rebuild_composition(&mut self) {
        self.composition = RetainedComposition::build(
            &self.cameras,
            &self.targets,
            &self.views,
            &self.presentations,
        );
    }

    /// Brings the composition up to a live camera's changed descriptor or
    /// motion: its own renderer camera, and the active camera's view.
    fn patch_camera(&mut self, handle: u64) {
        let entry = &self.cameras[&handle];
        if let Some(camera) = self.composition.cameras.get_mut(&handle) {
            *camera = composition_camera(std::mem::take(&mut camera.id), entry);
        }
        if self.active_camera == Some(handle) {
            if let (Some(view), Some(rendered)) =
                (self.views.first_mut(), self.composition.views.first_mut())
            {
                view.viewport = entry.descriptor.viewport;
                rendered.viewport = viewport(entry.descriptor.viewport);
            }
        }
    }

    /// Brings the composition up to a live target's changed descriptor:
    /// the target and the revision every view and presentation of it names.
    fn patch_target(&mut self, handle: u64, rendered: RendererCompositionTarget) {
        let Some(target) = self.composition.targets.get_mut(&handle) else {
            return;
        };
        let revision = rendered.revision;
        *target = rendered;
        for (view, rendered) in self.views.iter().zip(&mut self.composition.views) {
            if view.target.value != handle {
                continue;
            }
            if let RendererViewTarget::Offscreen {
                target_revision, ..
            } = &mut rendered.target
            {
                *target_revision = revision;
            }
        }
        for (presentation, rendered) in self
            .presentations
            .iter()
            .zip(&mut self.composition.presentations)
        {
            if presentation.source_target.value == handle {
                rendered.source_target_revision = revision;
            }
        }
    }
}

impl RetainedComposition {
    /// Every view and presentation must name a live camera and target.
    fn build(
        cameras: &BTreeMap<u64, CameraEntry>,
        targets: &BTreeMap<u64, CameraTargetState>,
        views: &[NativeCameraCompositionView],
        presentations: &[NativeCameraCompositionPresentation],
    ) -> Self {
        let mut composition = Self::default();
        for (index, view) in views.iter().enumerate() {
            let camera = &cameras[&view.camera.value];
            let camera_id = composition
                .cameras
                .entry(view.camera.value)
                .or_insert_with(|| {
                    composition_camera(format!("csharp-camera-{}", view.camera.value), camera)
                })
                .id
                .clone();
            let target = if view.target.value == 0 {
                RendererViewTarget::Primary
            } else {
                let target = composition.target(targets, view.target.value);
                RendererViewTarget::Offscreen {
                    target_id: target.id.clone(),
                    target_revision: target.revision,
                }
            };
            composition.views.push(RendererCompositionView {
                id: format!("csharp-view-{index}"),
                camera_id,
                target,
                viewport: viewport(view.viewport),
                order: view.order,
                viewport_anchor: (view.target.value == 0)
                    .then(|| camera.viewport_anchor.clone())
                    .flatten(),
            });
        }
        for (index, presentation) in presentations.iter().enumerate() {
            let target = composition.target(targets, presentation.source_target.value);
            let (source_target_id, source_target_revision) = (target.id.clone(), target.revision);
            composition.presentations.push(
                render_host_contracts::RendererCompositionPresentation {
                    id: format!("csharp-presentation-{index}"),
                    source_target_id,
                    source_target_revision,
                    destination: render_host_contracts::RendererPrimaryDestination {
                        kind: render_host_contracts::RendererPrimaryDestinationKind::Primary,
                        viewport: viewport(presentation.destination),
                    },
                    order: presentation.order,
                },
            );
        }
        composition
    }

    fn target(
        &mut self,
        targets: &BTreeMap<u64, CameraTargetState>,
        handle: u64,
    ) -> &RendererCompositionTarget {
        self.targets
            .entry(handle)
            .or_insert_with(|| composition_target(handle, targets[&handle]))
    }

    /// Refuses `rendered` in place of target `handle` when the targets the
    /// composition names would no longer fit the pixel budget together.
    fn check_target_budget(
        &self,
        handle: u64,
        rendered: &RendererCompositionTarget,
    ) -> Result<(), CsharpEngineServicesError> {
        if !self.targets.contains_key(&handle) {
            return Ok(());
        }
        render_host_contracts::validate_target_pixels(self.targets.iter().map(|(named, target)| {
            if *named == handle {
                rendered
            } else {
                target
            }
        }))
        .map_err(composition_error)
    }

    fn renderer(&self) -> RendererViewComposition {
        RendererViewComposition {
            cameras: self.cameras.values().cloned().collect(),
            targets: self.targets.values().cloned().collect(),
            views: self.views.clone(),
            presentations: self.presentations.clone(),
        }
    }
}

fn composition_target(
    handle: u64,
    state: CameraTargetState,
) -> render_host_contracts::RendererCompositionTarget {
    render_host_contracts::RendererCompositionTarget {
        id: format!("csharp-target-{handle}"),
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

/// A product camera's renderer facts.
fn camera_facts(
    descriptor: &NativeCameraDescriptor,
) -> (
    RendererCameraPose,
    Option<RendererCameraBasis>,
    RendererCameraProjection,
) {
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
    let pose = RendererCameraPose {
        position: native_vec3(descriptor.pose.position),
        pitch_degrees: descriptor.pose.pitch_degrees,
        yaw_degrees: descriptor.pose.yaw_degrees,
    };
    let basis = match descriptor.basis_mode {
        NativeCameraBasisMode::Derived => None,
        NativeCameraBasisMode::Explicit => Some(RendererCameraBasis {
            forward: native_vec3(descriptor.basis.forward),
            right: native_vec3(descriptor.basis.right),
            up: native_vec3(descriptor.basis.up),
        }),
    };
    (pose, basis, projection)
}

fn composition_camera(id: String, camera: &CameraEntry) -> RendererCompositionCamera {
    let (pose, basis, projection) = camera_facts(&camera.descriptor);
    RendererCompositionCamera {
        id,
        pose,
        basis,
        projection,
        motion: camera.motion.clone(),
        viewmodel_fov_y_degrees: camera.descriptor.viewmodel_fov_y_degrees,
    }
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

fn sample_motion(
    request: NativeCameraSampleRequest,
) -> Result<Option<RendererCameraMotion>, CsharpEngineServicesError> {
    if !request.sample_time_seconds.is_finite() || request.sample_time_seconds < 0.0 {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_CAMERA_SAMPLE_TIME",
            "camera sample time must be finite and non-negative",
        ));
    }
    let interpolation = match request.interpolation {
        NativeCameraInterpolation::Latest => return Ok(None),
        NativeCameraInterpolation::Position => RendererCameraInterpolation::Position,
        NativeCameraInterpolation::Pose => RendererCameraInterpolation::Pose,
    };
    if !request.delay_seconds.is_finite() || request.delay_seconds <= 0.0 {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_CAMERA_SAMPLE_DELAY",
            "interpolated camera delay must be finite and positive",
        ));
    }
    Ok(Some(RendererCameraMotion {
        // `update_sample` installs the checked, per-camera Engine identity.
        sample_id: String::new(),
        sample_time_seconds: request.sample_time_seconds,
        delay_seconds: request.delay_seconds,
        interpolation,
        cut: request.cut != 0,
    }))
}

/// Checks a product camera where it enters, by the renderer's rules for the
/// composition camera and the active view it may become.
fn validate_descriptor(
    descriptor: &NativeCameraDescriptor,
) -> Result<(), CsharpEngineServicesError> {
    let viewmodel_fov = descriptor.viewmodel_fov_y_degrees;
    if !(viewmodel_fov.is_finite() && (0.0..180.0).contains(&viewmodel_fov)) {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_CAMERA_DESCRIPTOR",
            "the viewmodel field of view must be 0 (the camera's own) or between 0 and 180 degrees",
        ));
    }
    let (pose, basis, projection) = camera_facts(descriptor);
    pose.validate()
        .and_then(|()| basis.map_or(Ok(()), |basis| basis.validate()))
        .and_then(|()| projection.validate())
        .and_then(|()| viewport(descriptor.viewport).validate())
        .map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_CAMERA_DESCRIPTOR",
                format!("camera descriptor is invalid: {error:?}"),
            )
        })
}

fn target_descriptor_error(
    error: render_host_contracts::RendererHostContractError,
) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_CAMERA_TARGET_DESCRIPTOR",
        format!("camera target descriptor is invalid: {error:?}"),
    )
}

fn composition_error(
    error: render_host_contracts::RendererHostContractError,
) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_CAMERA_COMPOSITION",
        format!("camera composition is invalid: {error:?}"),
    )
}

/// The call's background, fog, tone mapping, bloom, auto exposure, colour
/// grading, atmosphere, sun shaft and sky light selections as one frame, or
/// None when the call selected none of them.
/// A decal as the product placed it: its fade is stamped on the
/// presentation timeline when the call is published.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingDecal {
    decal: render_model::DecalDescriptor,
    life: f64,
    fade: f64,
}

/// The shortest fade, presentation seconds: a decal with a life and no fade
/// goes at once, not by a division by zero.
const DECAL_SHORTEST_FADE: f64 = 1e-3;

/// The decals this call placed and removed, stamped at presentation second
/// `now` (the call's published time): a decal with a life shows until `now`
/// plus it, then fades out. Then every decal whose fade has ended by `now`
/// is removed.
pub(crate) fn decal_operations(call: &mut RuntimeCameraViewCall, now: f64) -> Vec<RenderDiff> {
    let mut operations = Vec::new();
    for (id, change) in call.decals.drain(..) {
        match change {
            Some(PendingDecal {
                mut decal,
                life,
                fade,
            }) => {
                if life > 0.0 {
                    decal.fade_start = now + life;
                    decal.fade_end = decal.fade_start + fade.max(DECAL_SHORTEST_FADE);
                }
                // A decal placed and removed in the call is not published.
                if let Some(end) = call.state.decals.get_mut(&id) {
                    *end = decal.fade_end;
                    operations.push(RenderDiff::SetDecal { id, decal });
                }
            }
            None => operations.push(RenderDiff::RemoveDecal { id }),
        }
    }
    let gone: Vec<u64> = call
        .state
        .decals
        .iter()
        .filter(|(_, end)| **end > 0.0 && **end <= now)
        .map(|(id, _)| *id)
        .collect();
    for id in gone {
        call.state.decals.remove(&id);
        operations.push(RenderDiff::RemoveDecal { id });
    }
    operations
}

pub(crate) fn environment_frame(
    call: &RuntimeCameraViewCall,
    appearance: &RuntimeAppearanceCall,
    renderer_settings: Option<render_model::RendererSettingsDescriptor>,
) -> Result<Option<RenderFrameDiff>, CsharpEngineServicesError> {
    let mut operations = Vec::with_capacity(4);
    if let Some(settings) = renderer_settings {
        operations.push(RenderDiff::SetRendererSettings { settings });
    }
    if let Some(fog) = call.fog {
        operations.push(RenderDiff::SetFog { fog });
    }
    if let Some(tone_mapping) = call.tone_mapping {
        operations.push(RenderDiff::SetToneMapping { tone_mapping });
    }
    if let Some(bloom) = call.bloom {
        operations.push(RenderDiff::SetBloom { bloom });
    }
    if let Some(auto_exposure) = call.auto_exposure {
        operations.push(RenderDiff::SetAutoExposure { auto_exposure });
    }
    if let Some(color_grading) = call.color_grading {
        operations.push(RenderDiff::SetColorGrading { color_grading });
    }
    if let Some(atmosphere) = call.atmosphere {
        operations.push(RenderDiff::SetAtmosphere { atmosphere });
    }
    if let Some(sun_shafts) = call.sun_shafts {
        operations.push(RenderDiff::SetSunShafts { sun_shafts });
    }
    if let Some(moon) = call.moon {
        operations.push(RenderDiff::SetMoon { moon });
    }
    if let Some(request) = call.stars {
        let stars = match request {
            Some(request) => {
                let (band, stars) = appearance.star_field(request)?;
                if let Some(texture) = band {
                    operations.push(RenderDiff::DefineTexture { texture });
                }
                Some(stars)
            }
            None => None,
        };
        operations.push(RenderDiff::SetStars { stars });
    }
    if let Some(wind) = call.wind {
        operations.push(RenderDiff::SetWind { wind });
    }
    if let Some(clouds) = call.clouds {
        operations.push(RenderDiff::SetClouds { clouds });
    }
    if let Some(wetness) = call.wetness {
        operations.push(RenderDiff::SetWetness { wetness });
    }
    if let Some(snow) = call.snow {
        operations.push(RenderDiff::SetSnow { snow });
    }
    for (id, region) in &call.cloud_regions {
        operations.push(match region {
            Some(region) => RenderDiff::SetCloudRegion {
                id: *id,
                region: *region,
            },
            None => RenderDiff::RemoveCloudRegion { id: *id },
        });
    }
    if let Some(fog) = call.volumetric_fog {
        operations.push(RenderDiff::SetVolumetricFog { fog });
    }
    for (id, volume) in &call.fog_volumes {
        operations.push(match volume {
            Some(volume) => RenderDiff::SetFogVolume {
                id: *id,
                volume: *volume,
            },
            None => RenderDiff::RemoveFogVolume { id: *id },
        });
    }
    if let Some(request) = call.decal_atlas {
        let atlas = match request {
            Some(request) => {
                let (textures, atlas) = appearance.decal_atlas(request)?;
                operations.extend(
                    textures
                        .into_iter()
                        .map(|texture| RenderDiff::DefineTexture { texture }),
                );
                Some(atlas)
            }
            None => None,
        };
        operations.push(RenderDiff::SetDecalAtlas { atlas });
    }
    if let Some(precipitation) = call.precipitation {
        operations.push(RenderDiff::SetPrecipitation { precipitation });
    }
    if let Some(request) = call.image_effect {
        let effect = match request {
            Some(request) => {
                let (shader, textures, effect) = appearance.image_effect(request)?;
                operations.push(RenderDiff::DefineShader { shader });
                for texture in textures {
                    operations.push(RenderDiff::DefineTexture { texture });
                }
                Some(effect)
            }
            None => None,
        };
        operations.push(RenderDiff::SetImageEffect { effect });
    }
    if let Some(indirect_light) = call.indirect_light {
        operations.push(RenderDiff::SetIndirectLight { indirect_light });
    }
    if let Some(sky_light) = call.sky_light {
        operations.push(RenderDiff::SetSkyLight { sky_light });
    }
    if let Some(change) = call.background {
        background_operations(change, appearance, &mut operations)?;
    }
    if operations.is_empty() {
        return Ok(None);
    }
    Ok(Some(RenderFrameDiff::from_ops(operations)))
}

fn background_operations(
    change: CameraBackground,
    appearance: &RuntimeAppearanceCall,
    operations: &mut Vec<RenderDiff>,
) -> Result<(), CsharpEngineServicesError> {
    match change {
        CameraBackground::Sky(handle) => {
            let texture = appearance.texture_descriptor(handle)?;
            let identity = texture.id.clone();
            operations.push(RenderDiff::DefineTexture { texture });
            operations.push(RenderDiff::SetSkyBackground {
                background: Some(SkyBackgroundDescriptor {
                    texture: identity,
                    blend: None,
                }),
            });
        }
        CameraBackground::Blend {
            first,
            second,
            amount,
        } => {
            let first = appearance.texture_descriptor(first)?;
            let second = appearance.texture_descriptor(second)?;
            let background = SkyBackgroundDescriptor {
                texture: first.id.clone(),
                blend: Some(render_model::SkyBackgroundBlend {
                    texture: second.id.clone(),
                    amount,
                }),
            };
            operations.push(RenderDiff::DefineTexture { texture: first });
            operations.push(RenderDiff::DefineTexture { texture: second });
            operations.push(RenderDiff::SetSkyBackground {
                background: Some(background),
            });
        }
        CameraBackground::Color(color) => operations.push(RenderDiff::SetBackgroundColor { color }),
        CameraBackground::Default => {
            operations.push(RenderDiff::SetSkyBackground { background: None })
        }
    }
    Ok(())
}

pub(crate) unsafe extern "C" fn create_camera(
    context: *mut c_void,
    request: *const NativeCameraDescriptor,
    result: *mut NativeCameraHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
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
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_camera(
    context: *mut c_void,
    request: *const NativeCameraUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.update(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_camera_sample(
    context: *mut c_void,
    request: *const NativeCameraSampleRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.update_sample(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn replace_camera(
    context: *mut c_void,
    request: *const NativeCameraReplaceRequest,
    result: *mut NativeCameraHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
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
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_camera(
    context: *mut c_void,
    camera: NativeCameraHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.destroy(camera) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn create_camera_target(
    context: *mut c_void,
    request: *const NativeCameraTargetDescriptor,
    result: *mut NativeCameraTargetHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
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
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn update_camera_target(
    context: *mut c_void,
    request: *const NativeCameraTargetUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.update_target(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn replace_camera_target(
    context: *mut c_void,
    request: *const NativeCameraTargetReplaceRequest,
    result: *mut NativeCameraTargetHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
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
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_camera_target(
    context: *mut c_void,
    target: NativeCameraTargetHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.destroy_target(target) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_camera_composition(
    context: *mut c_void,
    request: *const NativeCameraCompositionRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match unsafe { bridge.set_composition(&*request) } {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_active_camera(
    context: *mut c_void,
    camera: NativeCameraHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_active(camera) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn clear_active_camera(
    context: *mut c_void,
    request: *const NativeClearActiveCameraRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.clear_active() {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_sky_background(
    context: *mut c_void,
    texture: NativeRenderResourceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_sky(texture) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn clear_sky_background(
    context: *mut c_void,
    request: *const NativeClearSkyBackgroundRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.clear_sky() {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_background_color(
    context: *mut c_void,
    request: *const NativeSetBackgroundColorRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_background_color(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_fog(
    context: *mut c_void,
    request: *const NativeFogRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_fog(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_surface(
    context: *mut c_void,
    output: *mut NativeCameraSurfaceReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeCameraViewBridge>() };
    unsafe { *output = bridge.surface };
    ABI_OK
}

pub(crate) unsafe extern "C" fn read_viewport_anchor(
    context: *mut c_void,
    request: *const NativeCameraViewportAnchorReadRequest,
    output: *mut NativeCameraViewportAnchorReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    let request = unsafe { *request };
    match unsafe {
        crate::composition::borrowed_utf8(request.anchor.bytes, request.anchor.len, "anchor")
    } {
        Ok(anchor) => {
            unsafe { *output = bridge.viewport_anchor(anchor) };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_viewport_anchor(
    context: *mut c_void,
    request: *const NativeCameraViewportAnchorRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    let request = unsafe { *request };
    let result = unsafe {
        crate::composition::borrowed_utf8(request.anchor.bytes, request.anchor.len, "anchor")
    }
    .and_then(|anchor| bridge.set_viewport_anchor(request.camera, anchor));
    match result {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_tone_mapping(
    context: *mut c_void,
    request: *const NativeToneMappingRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_tone_mapping(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_bloom(
    context: *mut c_void,
    request: *const NativeBloomRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_bloom(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_auto_exposure(
    context: *mut c_void,
    request: *const NativeAutoExposureRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_auto_exposure(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_sky_light(
    context: *mut c_void,
    request: *const NativeSkyLightRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_sky_light(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_sun_shafts(
    context: *mut c_void,
    request: *const NativeSunShaftsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_sun_shafts(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_moon(
    context: *mut c_void,
    request: *const NativeMoonRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_moon(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_stars(
    context: *mut c_void,
    request: *const NativeStarsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_stars(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_wind(
    context: *mut c_void,
    request: *const NativeWindRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_wind(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_image_effect(
    context: *mut c_void,
    request: *const NativeImageEffectRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_image_effect(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_precipitation(
    context: *mut c_void,
    request: *const NativePrecipitationRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_precipitation(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_wetness(
    context: *mut c_void,
    request: *const NativeWetnessRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_wetness(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_snow(
    context: *mut c_void,
    request: *const NativeSnowRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_snow(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_snow(
    context: *mut c_void,
    output: *mut NativeSnowReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeCameraViewBridge>() };
    unsafe { *output = bridge.read_snow() };
    ABI_OK
}

pub(crate) unsafe extern "C" fn set_backdrop(
    context: *mut c_void,
    request: *const NativeBackdropRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_backdrop(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn clear_backdrop(
    context: *mut c_void,
    request: *const NativeClearBackdropRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.clear_backdrop(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_cloud_region(
    context: *mut c_void,
    request: *const NativeCloudRegionRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_cloud_region(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn remove_cloud_region(
    context: *mut c_void,
    request: *const NativeCloudRegionRemoval,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.remove_cloud_region(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_volumetric_fog(
    context: *mut c_void,
    request: *const NativeVolumetricFogRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_volumetric_fog(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_decal(
    context: *mut c_void,
    request: *const NativeDecalRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_decal(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn remove_decal(
    context: *mut c_void,
    request: *const NativeDecalRemoval,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.remove_decal(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_decal_atlas(
    context: *mut c_void,
    request: *const NativeDecalAtlasRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_decal_atlas(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_decals(
    context: *mut c_void,
    output: *mut NativeDecalReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeCameraViewBridge>() };
    unsafe { *output = bridge.read_decals() };
    ABI_OK
}

pub(crate) unsafe extern "C" fn set_fog_volume(
    context: *mut c_void,
    request: *const NativeFogVolumeRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_fog_volume(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn remove_fog_volume(
    context: *mut c_void,
    request: *const NativeFogVolumeRemoval,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.remove_fog_volume(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

fn cloud_kind(kind: NativeCloudKind) -> render_model::CloudKind {
    match kind {
        NativeCloudKind::Cumulus => render_model::CloudKind::Cumulus,
        NativeCloudKind::Stratus => render_model::CloudKind::Stratus,
        NativeCloudKind::Cumulonimbus => render_model::CloudKind::Cumulonimbus,
    }
}

pub(crate) unsafe extern "C" fn set_clouds(
    context: *mut c_void,
    request: *const NativeCloudsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_clouds(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_indirect_light(
    context: *mut c_void,
    request: *const NativeIndirectLightRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_indirect_light(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_atmosphere(
    context: *mut c_void,
    request: *const NativeAtmosphereRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_atmosphere(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_color_grading(
    context: *mut c_void,
    request: *const NativeColorGradingRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    match bridge.set_color_grading(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

impl RuntimeCameraViewCall {
    pub(crate) fn output_camera(
        &self,
        id: u64,
    ) -> Result<RendererCompositionCamera, CsharpEngineServicesError> {
        let camera = self.state.cameras.get(&id).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_RENDER_OUTPUT_CAMERA", "unknown capture camera")
        })?;
        let mut camera = composition_camera(format!("capture-{id}"), camera);
        camera.motion = None;
        Ok(camera)
    }
}

pub(crate) unsafe extern "C" fn set_sky_background_blend(
    context: *mut c_void,
    request: *const NativeSkyBackgroundBlendRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeCameraViewBridge>() };
    let request = unsafe { *request };
    let result = (|| {
        if request.first.value == 0 || request.second.value == 0 || !request.amount.is_finite() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SKY_BLEND",
                "sky blend requires two live textures and a finite amount",
            ));
        }
        let staged = bridge.staged_mut()?;
        staged.state.background = CameraBackground::Blend {
            first: request.first.value,
            second: request.second.value,
            amount: request.amount.clamp(0.0, 1.0),
        };
        staged.background = Some(staged.state.background);
        Ok(())
    })();
    match result {
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
            viewmodel_fov_y_degrees: 0.0,
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

    fn sample_request(
        camera: NativeCameraHandle,
        descriptor: NativeCameraDescriptor,
        interpolation: NativeCameraInterpolation,
        cut: bool,
    ) -> NativeCameraSampleRequest {
        NativeCameraSampleRequest {
            camera,
            descriptor,
            sample_time_seconds: 42.0,
            delay_seconds: 1.0 / 60.0,
            interpolation,
            cut: u8::from(cut),
        }
    }

    const FULL: NativeCameraViewport = NativeCameraViewport {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    /// Four cameras: one on the primary surface, two drawing into targets
    /// shown as insets, and a fourth inset on the primary surface.
    fn split_scene(
        bridge: &mut RuntimeCameraViewBridge,
        first_target: NativeCameraTargetDescriptor,
    ) -> (Vec<NativeCameraHandle>, Vec<NativeCameraTargetHandle>) {
        bridge.begin_call();
        let cameras: Vec<_> = (0..4)
            .map(|_| bridge.create(camera_descriptor(FULL)).expect("camera"))
            .collect();
        let targets: Vec<_> = [first_target, target_descriptor(), target_descriptor()]
            .into_iter()
            .map(|descriptor| bridge.create_target(descriptor).expect("target"))
            .collect();
        let inset = NativeCameraViewport {
            x: 0.75,
            y: 0.75,
            width: 0.25,
            height: 0.25,
        };
        let view = |camera: NativeCameraHandle, target: u64, viewport, order| {
            NativeCameraCompositionView {
                camera,
                target: NativeCameraTargetReference { value: target },
                viewport,
                order,
            }
        };
        let views = [
            view(cameras[0], 0, FULL, 0),
            view(cameras[1], targets[0].value, FULL, 1),
            view(cameras[2], targets[1].value, FULL, 2),
            view(cameras[2], targets[2].value, FULL, 3),
            view(cameras[3], 0, inset, 4),
        ];
        let presentations =
            targets
                .iter()
                .enumerate()
                .map(|(index, target)| NativeCameraCompositionPresentation {
                    source_target: *target,
                    destination: inset,
                    order: 10 + index as u64,
                });
        let presentations: Vec<_> = presentations.collect();
        unsafe {
            bridge
                .set_composition(&NativeCameraCompositionRequest {
                    views: views.as_ptr(),
                    views_len: views.len(),
                    presentations: presentations.as_ptr(),
                    presentations_len: presentations.len(),
                })
                .expect("split composition");
        }
        let call = bridge.take_staged_call().expect("scene call");
        bridge.commit(call);
        (cameras, targets)
    }

    /// The composition built from nothing but the retained cameras, targets,
    /// views and presentations.
    fn rebuilt(state: &CameraState) -> RendererViewComposition {
        RetainedComposition::build(
            &state.cameras,
            &state.targets,
            &state.views,
            &state.presentations,
        )
        .renderer()
    }

    fn staged_state(bridge: &RuntimeCameraViewBridge) -> &CameraState {
        &bridge.staged.as_ref().expect("inside a call").state
    }

    /// Where the composition's identifiers live: a patch keeps them, a
    /// rebuild makes new ones.
    fn identifier_addresses(composition: &RetainedComposition) -> Vec<*const u8> {
        let cameras = composition
            .cameras
            .values()
            .map(|camera| camera.id.as_ptr());
        let targets = composition
            .targets
            .values()
            .map(|target| target.id.as_ptr());
        let views = composition.views.iter().map(|view| view.id.as_ptr());
        cameras.chain(targets).chain(views).collect()
    }

    #[test]
    fn samples_patch_their_own_camera_and_a_target_change_its_own_target() {
        let mut bridge = RuntimeCameraViewBridge::new();
        let (cameras, targets) = split_scene(&mut bridge, target_descriptor());
        let addresses = identifier_addresses(&bridge.state.composition);
        let untouched = |composition: &RendererViewComposition| {
            (
                composition.cameras[1..].to_vec(),
                composition.targets.clone(),
            )
        };
        let unchanged = untouched(&bridge.snapshot_composition());

        for frame in 0..8 {
            bridge.begin_call();
            for camera in [cameras[0], cameras[0]] {
                let mut descriptor = camera_descriptor(FULL);
                descriptor.pose.position.x = frame as f32;
                bridge
                    .update_sample(NativeCameraSampleRequest {
                        sample_time_seconds: f64::from(frame) / 60.0,
                        ..sample_request(
                            camera,
                            descriptor,
                            NativeCameraInterpolation::Position,
                            false,
                        )
                    })
                    .expect("sample");
            }
            let mut call = bridge.take_staged_call().expect("sample call");
            let published = call.take_composition().expect("a sample publishes");
            assert_eq!(published, rebuilt(&call.state));
            assert_eq!(published.cameras[0].pose.position[0], f64::from(frame));
            assert_eq!(untouched(&published), unchanged);
            bridge.commit(call);
            assert_eq!(
                identifier_addresses(&bridge.state.composition),
                addresses,
                "a sample rebuilds nothing"
            );
        }

        bridge.begin_call();
        let mut resized = target_descriptor();
        resized.width = 640;
        bridge
            .update_target(NativeCameraTargetUpdateRequest {
                target: targets[1],
                descriptor: resized,
            })
            .expect("target update");
        let mut call = bridge.take_staged_call().expect("target call");
        let published = call.take_composition().expect("a target change publishes");
        assert_eq!(published, rebuilt(&call.state));
        assert_eq!(published.targets[1].width, 640);
        assert_eq!(published.targets[1].revision, 2);
        assert_eq!(published.presentations[1].source_target_revision, 2);
        assert_eq!(published.targets[0], unchanged.1[0]);
        bridge.commit(call);
        // Only the changed target is made again.
        let mut kept = addresses.clone();
        kept[cameras.len() + 1] = bridge.state.composition.targets[&targets[1].value]
            .id
            .as_ptr();
        assert_eq!(
            identifier_addresses(&bridge.state.composition),
            kept,
            "a target change rebuilds nothing else"
        );

        bridge.begin_call();
        bridge.destroy(cameras[2]).expect("composition change");
        let mut call = bridge.take_staged_call().expect("destroy call");
        let published = call.take_composition().expect("a removal publishes");
        assert_eq!(published, rebuilt(&call.state));
        assert_eq!(published.cameras.len(), 3);
        assert_eq!(published.views.len(), 3);
        bridge.commit(call);
    }

    #[test]
    fn creating_a_camera_or_target_no_view_names_publishes_no_composition() {
        let mut bridge = RuntimeCameraViewBridge::new();
        bridge.begin_call();
        bridge.create(camera_descriptor(FULL)).expect("camera");
        bridge.create_target(target_descriptor()).expect("target");
        let mut call = bridge.take_staged_call().expect("create call");
        assert_eq!(call.take_composition(), None);
        bridge.commit(call);
        let composition = bridge.snapshot_composition();
        assert!(composition.cameras.is_empty() && composition.targets.is_empty());
    }

    #[test]
    fn a_refused_change_leaves_the_cameras_and_the_published_composition_as_they_were() {
        let mut bridge = RuntimeCameraViewBridge::new();
        // Half the composition's pixel budget, wider than it is tall.
        let largest = NativeCameraTargetDescriptor {
            width: 4_096,
            height: 1_024,
            ..target_descriptor()
        };
        // One target at the largest size leaves room for the small ones
        // but not for a second at the largest size.
        let (cameras, targets) = split_scene(&mut bridge, largest);

        bridge.begin_call();
        bridge
            .update_sample(sample_request(
                cameras[0],
                camera_descriptor(FULL),
                NativeCameraInterpolation::Position,
                false,
            ))
            .expect("accepted sample");
        let facts = |bridge: &RuntimeCameraViewBridge| {
            let state = staged_state(bridge);
            format!(
                "{:?}",
                (
                    state
                        .cameras
                        .iter()
                        .map(|(handle, camera)| (
                            handle,
                            camera.descriptor,
                            &camera.motion,
                            camera.next_sample_id,
                            &camera.viewport_anchor
                        ))
                        .collect::<Vec<_>>(),
                    state
                        .targets
                        .iter()
                        .map(|(handle, target)| (handle, target.descriptor, target.revision))
                        .collect::<Vec<_>>(),
                    &state.views,
                    &state.presentations,
                    state.active_camera,
                    state.next_camera,
                    state.next_target,
                )
            )
        };
        let before = (facts(&bridge), staged_state(&bridge).composition.renderer());
        let unchanged = |bridge: &RuntimeCameraViewBridge, refused: &str| {
            assert_eq!(
                (facts(bridge), staged_state(bridge).composition.renderer()),
                before,
                "{refused}"
            );
        };

        let mut invalid = camera_descriptor(FULL);
        invalid.pose.position.x = f32::NAN;
        let refusals: [(&str, &str, Result<(), CsharpEngineServicesError>); 3] = [
            (
                "a non-finite pose",
                "CSHARP_CAMERA_DESCRIPTOR",
                bridge.update(NativeCameraUpdateRequest {
                    camera: cameras[0],
                    descriptor: invalid,
                }),
            ),
            (
                "a viewport off the surface",
                "CSHARP_CAMERA_DESCRIPTOR",
                bridge.update_sample(sample_request(
                    cameras[1],
                    camera_descriptor(NativeCameraViewport { x: 0.5, ..FULL }),
                    NativeCameraInterpolation::Pose,
                    false,
                )),
            ),
            (
                "a second target at the largest size",
                "CSHARP_CAMERA_COMPOSITION",
                bridge.update_target(NativeCameraTargetUpdateRequest {
                    target: targets[1],
                    descriptor: largest,
                }),
            ),
        ];
        for (refused, code, result) in refusals {
            assert_eq!(result.expect_err(refused).code(), code, "{refused}");
            unchanged(&bridge, refused);
        }
        let replaced = bridge.replace_target(NativeCameraTargetReplaceRequest {
            target: targets[2],
            replacement: largest,
        });
        assert_eq!(
            replaced.expect_err("replacement over budget").code(),
            "CSHARP_CAMERA_COMPOSITION"
        );
        unchanged(&bridge, "a replacement target over the budget");
        assert_eq!(
            bridge
                .create_target(NativeCameraTargetDescriptor {
                    width: 0,
                    ..target_descriptor()
                })
                .expect_err("empty target")
                .code(),
            "CSHARP_CAMERA_TARGET_DESCRIPTOR"
        );
        unchanged(&bridge, "an empty target");
        for (refused, view) in [
            (
                "a view off the surface",
                NativeCameraCompositionView {
                    camera: cameras[0],
                    target: NativeCameraTargetReference::default(),
                    viewport: NativeCameraViewport { width: 0.0, ..FULL },
                    order: 0,
                },
            ),
            (
                "a view of a dead target",
                NativeCameraCompositionView {
                    camera: cameras[0],
                    target: NativeCameraTargetReference { value: 99 },
                    viewport: FULL,
                    order: 0,
                },
            ),
        ] {
            let views = [view];
            unsafe {
                bridge
                    .set_composition(&NativeCameraCompositionRequest {
                        views: views.as_ptr(),
                        views_len: views.len(),
                        presentations: std::ptr::null(),
                        presentations_len: 0,
                    })
                    .expect_err(refused);
            }
            unchanged(&bridge, refused);
        }
        bridge
            .set_active(NativeCameraHandle { value: 99 })
            .expect_err("a dead camera");
        unchanged(&bridge, "a dead active camera");

        let mut call = bridge.take_staged_call().expect("refusals keep the call");
        assert_eq!(
            call.take_composition(),
            Some(before.1.clone()),
            "the call publishes what its accepted sample left"
        );
        bridge.commit(call);
        assert_eq!(bridge.snapshot_composition(), before.1);

        bridge.begin_call();
        bridge
            .update(NativeCameraUpdateRequest {
                camera: cameras[0],
                descriptor: invalid,
            })
            .expect_err("refused");
        let mut call = bridge.take_staged_call().expect("refusal keeps the call");
        assert_eq!(call.take_composition(), None, "a refusal publishes nothing");
        bridge.commit(call);
    }

    #[test]
    fn refused_target_update_keeps_prior_descriptor_and_composition() {
        let mut bridge = RuntimeCameraViewBridge::new();
        bridge.begin_call();
        let full = NativeCameraViewport {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let camera = bridge.create(camera_descriptor(full)).expect("camera");
        let targets = [(); 3].map(|()| bridge.create_target(target_descriptor()).expect("target"));
        let views = targets.map(|target| NativeCameraCompositionView {
            camera,
            target: NativeCameraTargetReference {
                value: target.value,
            },
            viewport: full,
            order: 0,
        });
        unsafe {
            bridge
                .set_composition(&NativeCameraCompositionRequest {
                    views: views.as_ptr(),
                    views_len: views.len(),
                    presentations: std::ptr::null(),
                    presentations_len: 0,
                })
                .expect("three referenced targets");
        }
        let initial = bridge.take_staged_call().expect("initial composition");
        bridge.commit(initial);

        bridge.begin_call();
        // Half the composition's pixel budget, wider than it is tall.
        let largest = NativeCameraTargetDescriptor {
            width: 4_096,
            height: 1_024,
            ..target_descriptor()
        };
        bridge
            .update_target(NativeCameraTargetUpdateRequest {
                target: targets[0],
                descriptor: largest,
            })
            .expect("one large target fits the pixel budget");
        bridge
            .update_target(NativeCameraTargetUpdateRequest {
                target: targets[1],
                descriptor: largest,
            })
            .expect_err("a second large target exceeds the pixel budget");
        let mut staged = bridge
            .take_staged_call()
            .expect("refusal does not fail the call");
        let retained = &staged.state.targets[&targets[1].value];
        assert_eq!(retained.descriptor.width, target_descriptor().width);
        assert_eq!(retained.revision, 1);
        let composition = staged.take_composition().expect("accepted update output");
        assert_eq!(composition.targets.len(), 3);
        assert_eq!(
            composition.targets[1].width,
            target_descriptor().width,
            "the refused target keeps its size"
        );
        bridge.commit(staged);
        assert_eq!(bridge.snapshot_composition(), composition);
    }

    #[test]
    fn sampled_camera_metadata_survives_snapshots_and_immediate_updates_clear_it() {
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
        bridge.set_active(camera).expect("active camera");
        let initial = bridge.take_staged_call().expect("initial camera");
        bridge.commit(initial);

        bridge.begin_call();
        bridge
            .update_sample(sample_request(
                camera,
                camera_descriptor(NativeCameraViewport {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }),
                NativeCameraInterpolation::Position,
                false,
            ))
            .expect("position sample");
        let mut first = bridge.take_staged_call().expect("first sample");
        let first_motion = first.take_composition().unwrap().cameras[0]
            .motion
            .as_ref()
            .expect("position metadata")
            .clone();
        assert_eq!(first_motion.sample_id, "1");
        assert_eq!(
            first_motion.interpolation,
            RendererCameraInterpolation::Position
        );
        assert!(!first_motion.cut);
        bridge.commit(first);

        let snapshot = bridge.snapshot_composition();
        assert_eq!(snapshot.cameras[0].motion.as_ref(), Some(&first_motion));

        bridge.begin_call();
        bridge
            .update_sample(sample_request(
                camera,
                camera_descriptor(NativeCameraViewport {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }),
                NativeCameraInterpolation::Pose,
                true,
            ))
            .expect("pose sample");
        let mut second = bridge.take_staged_call().expect("second sample");
        let second_motion = second.take_composition().unwrap().cameras[0]
            .motion
            .clone()
            .expect("pose metadata");
        assert_eq!(second_motion.sample_id, "2");
        assert_eq!(
            second_motion.interpolation,
            RendererCameraInterpolation::Pose
        );
        assert!(second_motion.cut);
        bridge.commit(second);

        bridge.begin_call();
        bridge
            .update(NativeCameraUpdateRequest {
                camera,
                descriptor: camera_descriptor(NativeCameraViewport {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }),
            })
            .expect("immediate update");
        let mut immediate = bridge.take_staged_call().expect("immediate composition");
        assert!(immediate.take_composition().unwrap().cameras[0]
            .motion
            .is_none());
    }

    #[test]
    fn sampled_camera_rejects_invalid_timeline_math() {
        let mut bridge = RuntimeCameraViewBridge::new();
        bridge.begin_call();
        let descriptor = camera_descriptor(NativeCameraViewport {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        let camera = bridge.create(descriptor).expect("camera");
        let mut invalid_time = sample_request(
            camera,
            descriptor,
            NativeCameraInterpolation::Position,
            false,
        );
        invalid_time.sample_time_seconds = f64::NAN;
        assert_eq!(
            bridge
                .update_sample(invalid_time)
                .expect_err("invalid time")
                .code(),
            "CSHARP_CAMERA_SAMPLE_TIME"
        );
        let mut invalid_delay =
            sample_request(camera, descriptor, NativeCameraInterpolation::Pose, false);
        invalid_delay.delay_seconds = 0.0;
        assert_eq!(
            bridge
                .update_sample(invalid_delay)
                .expect_err("invalid delay")
                .code(),
            "CSHARP_CAMERA_SAMPLE_DELAY"
        );
    }

    #[test]
    fn a_viewport_anchor_names_the_cameras_primary_views_and_reads_back_the_surface() {
        let mut bridge = RuntimeCameraViewBridge::new();
        bridge.begin_call();
        let quarter = NativeCameraViewport {
            x: 0.0,
            y: 0.0,
            width: 0.25,
            height: 0.25,
        };
        let camera = bridge.create(camera_descriptor(quarter)).expect("camera");
        bridge.set_active(camera).expect("active");
        let anchors = |bridge: &RuntimeCameraViewBridge| {
            bridge
                .staged
                .as_ref()
                .unwrap()
                .state
                .composition
                .views
                .iter()
                .map(|view| (view.viewport_anchor.clone(), view.viewport.width))
                .collect::<Vec<_>>()
        };
        bridge.set_viewport_anchor(camera, "hero").expect("anchor");
        // The committed viewport stays; the renderer substitutes the rect.
        assert_eq!(anchors(&bridge), [(Some("hero".to_owned()), 0.25)]);
        bridge.set_viewport_anchor(camera, "").expect("no anchor");
        assert_eq!(anchors(&bridge), [(None, 0.25)]);
        assert!(bridge
            .set_viewport_anchor(NativeCameraHandle { value: 99 }, "hero")
            .is_err());

        let surface = NativeCameraSurfaceReadout {
            reported: true,
            watching: true,
            css_width: 800.0,
            css_height: 600.0,
            device_width: 1600.0,
            device_height: 1200.0,
            device_pixel_ratio: 2.0,
            ui_scale: 1.25,
            revision: 3,
        };
        let hero = render_host_contracts::RendererViewport {
            x: 0.1,
            y: 0.2,
            width: 0.5,
            height: 0.6,
        };
        bridge.set_surface(surface, BTreeMap::from([("hero".to_owned(), hero)]));
        let mut read = NativeCameraSurfaceReadout::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                read_surface(
                    (&mut bridge as *mut RuntimeCameraViewBridge).cast(),
                    &mut read,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(read, surface);

        let read_anchor = |bridge: &mut RuntimeCameraViewBridge, name: &str| {
            let mut read = NativeCameraViewportAnchorReadout::default();
            let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
            let request = NativeCameraViewportAnchorReadRequest {
                anchor: NativeUtf8Slice {
                    bytes: name.as_ptr(),
                    len: name.len(),
                },
            };
            let status = unsafe {
                read_viewport_anchor(
                    (bridge as *mut RuntimeCameraViewBridge).cast(),
                    &request,
                    &mut read,
                    &mut error,
                )
            };
            assert_eq!(status, ABI_OK);
            read
        };
        assert_eq!(
            read_anchor(&mut bridge, "hero"),
            NativeCameraViewportAnchorReadout {
                reported: true,
                x: 0.1,
                y: 0.2,
                width: 0.5,
                height: 0.6,
                revision: 3,
            }
        );
        assert_eq!(
            read_anchor(&mut bridge, "minimap"),
            NativeCameraViewportAnchorReadout::default()
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
        assert_eq!(
            staged_state(&bridge).composition.renderer(),
            rebuilt(staged_state(&bridge)),
            "the active view follows its camera's viewport"
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

    #[test]
    fn retained_multi_view_target_composition_rebuilds_and_reconciles_owners() {
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
        let mut staged = bridge.take_staged_call().expect("staged composition");
        let composition = staged.take_composition().expect("composition output");
        assert_eq!(composition.cameras.len(), 2);
        assert_eq!(composition.targets.len(), 1);
        assert_eq!(composition.views.len(), 2);
        assert_eq!(composition.presentations.len(), 1);
        assert_eq!(composition.targets[0].revision, 1);
        assert!(serde_json::to_string(&composition)
            .expect("serializable composition")
            .contains("csharp-target-1"));
        bridge.commit(staged);

        assert_eq!(bridge.snapshot_composition(), composition);

        bridge.begin_call();
        let mut updated = target_descriptor();
        updated.sampling = NativeCameraTargetSampling::Nearest;
        bridge
            .update_target(NativeCameraTargetUpdateRequest {
                target,
                descriptor: updated,
            })
            .expect("target update");
        let mut update = bridge.take_staged_call().expect("updated composition");
        let composition = update.take_composition().unwrap();
        assert_eq!(composition.targets[0].revision, 2);
        assert_eq!(
            composition.views[1].target,
            RendererViewTarget::Offscreen {
                target_id: "csharp-target-1".to_owned(),
                target_revision: 2,
            }
        );
        assert_eq!(composition.presentations[0].source_target_revision, 2);
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
        let mut replacement = bridge.take_staged_call().expect("replacement composition");
        let composition = replacement.take_composition().unwrap();
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
        let mut removal = bridge.take_staged_call().expect("target removal");
        let composition = removal.take_composition().unwrap();
        assert_eq!(composition.targets.len(), 0);
        assert_eq!(composition.presentations.len(), 0);
        assert_eq!(composition.views.len(), 1);
        assert_eq!(
            composition.views[0].camera_id,
            format!("csharp-camera-{}", front.value)
        );
    }
}

#[cfg(test)]
mod viewmodel_field_of_view {
    use super::*;

    fn entry(viewmodel_fov_y_degrees: f64) -> CameraEntry {
        CameraEntry {
            descriptor: descriptor(viewmodel_fov_y_degrees),
            motion: None,
            next_sample_id: 1,
            viewport_anchor: None,
        }
    }

    fn descriptor(viewmodel_fov_y_degrees: f64) -> NativeCameraDescriptor {
        NativeCameraDescriptor {
            pose: NativeCameraPose::default(),
            basis_mode: NativeCameraBasisMode::Derived,
            basis: NativeCameraBasis::default(),
            projection: NativeCameraProjection {
                kind: NativeCameraProjectionKind::Perspective,
                fov_y_degrees: 70.0,
                vertical_size: 1.0,
                near: 0.1,
                far: 500.0,
            },
            viewport: NativeCameraViewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            viewmodel_fov_y_degrees,
        }
    }

    #[test]
    fn the_viewmodel_field_of_view_reaches_the_composition_camera_within_its_range() {
        validate_descriptor(&descriptor(0.0)).expect("0 keeps the camera's own");
        let camera = composition_camera("eye".to_owned(), &entry(0.0));
        assert_eq!(camera.viewmodel_fov_y_degrees, 0.0);
        validate_descriptor(&descriptor(35.0)).expect("within range");
        let camera = composition_camera("eye".to_owned(), &entry(35.0));
        assert_eq!(camera.viewmodel_fov_y_degrees, 35.0);
        for refused in [180.0, -1.0, f64::NAN] {
            let error = validate_descriptor(&descriptor(refused)).unwrap_err();
            assert_eq!(error.code(), "CSHARP_CAMERA_DESCRIPTOR", "{refused}");
        }
    }

    #[test]
    fn an_indirect_light_request_is_staged_whole_and_a_zero_extent_turns_it_off() {
        let mut bridge = RuntimeCameraViewBridge::new();
        let request = NativeIndirectLightRequest {
            center: NativeVec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            extent: NativeVec3 {
                x: 8.0,
                y: 4.0,
                z: 8.0,
            },
            spacing: 1.0,
            bounces: 2,
            ambient: NativeIndirectAmbient::Floor,
        };
        bridge.begin_call();
        bridge
            .set_indirect_light(request)
            .expect("a valid volume is staged");
        let call = bridge.take_staged_call().expect("inside a call");
        assert_eq!(
            call.indirect_light,
            Some(Some(IndirectLightDescriptor {
                center: [1.0, 2.0, 3.0],
                extent: [8.0, 4.0, 8.0],
                spacing: 1.0,
                bounces: 2,
                ambient: IndirectAmbient::Floor,
            }))
        );
        bridge.commit(call);

        bridge.begin_call();
        let refused = bridge
            .set_indirect_light(NativeIndirectLightRequest {
                spacing: 0.1,
                ..request
            })
            .expect_err("a spacing under half a metre is refused");
        assert_eq!(refused.code(), "CSHARP_INDIRECT_LIGHT");
        let refused = bridge
            .set_indirect_light(NativeIndirectLightRequest {
                spacing: 0.5,
                extent: NativeVec3 {
                    x: 100.0,
                    y: 100.0,
                    z: 100.0,
                },
                ..request
            })
            .expect_err("more probes than the volume holds are refused");
        assert_eq!(refused.code(), "CSHARP_INDIRECT_LIGHT");
        bridge
            .set_indirect_light(NativeIndirectLightRequest {
                extent: NativeVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                ..request
            })
            .expect("a zero extent turns the volume off");
        let call = bridge.take_staged_call().expect("inside a call");
        assert_eq!(call.indirect_light, Some(None));
    }
}
