use crate::composition::{borrowed_slice, borrowed_utf8, CsharpEngineServicesError, ABI_OK};
use crate::render_resources::{
    CsharpRenderResource, CsharpRenderResourceKind, GeneratedMeshes, RenderResourceImports,
    RenderResourceRegistry,
};
use csharp_engine_abi::*;
use render_model::*;
use render_presentation::{
    validate_animation_catalog, AnimationCatalog, AnimationClipAsset, AnimationCondition,
    AnimationControllerService, AnimationGraphDefinition, AnimationMotionDefinition,
    AnimationParameterDefinition, AnimationParameterKind, AnimationParameterValue,
    AnimationProjectionDescriptor, AnimationProjectionTarget, AnimationProjector,
    AnimationStateDefinition, AnimationTransitionDefinition, AnimationTransitionFactMoment,
    BillboardAlignment, BillboardAnchor, BillboardContent, BillboardDescriptor,
    BillboardEdgeBehavior, BillboardFontRef, BillboardHandle, BillboardIndicator, BillboardLayer,
    BillboardLayoutPolicy, BillboardLayoutSizing, BillboardMeter, BillboardMeterFillDirection,
    BillboardOverlapBehavior, BillboardPatch, BillboardProjectionDiagnosticCode,
    BillboardProjectionOp, BillboardProjector, BillboardSafeArea, BillboardStatusCue,
    BillboardStyle, BillboardTextureRef, GhostPlateAnchorPolicy, GhostPlateCaptureLighting,
    GhostPlateCaptureLightingMode, GhostPlateCaptureSettings, GhostPlateConfig,
    GhostPlateDescriptor, GhostPlateHandle, GhostPlateMapping, GhostPlatePatch,
    GhostPlatePlacement, GhostPlateProjectionOp, GhostPlateProjector, GhostPlateShellMode,
    ParticleAnchor, ParticleBlendMode, ParticleCollisionDescriptor, ParticleCollisionLimitBehavior,
    ParticleCollisionVolume, ParticleEmissionAdmissionOutcome, ParticleEmitterDescriptor,
    ParticleEmitterHandle, ParticleEmitterPatch, ParticleProjectionDiagnosticCode,
    ParticleProjectionOp, ParticleProjector, ParticleSizeMode, ParticleSpriteRef, ParticleVisual,
    PresentationFrameDiff, PresentationOp, PresentationOpMeta,
};
use render_projection::{
    Appearance, AppearanceProjectionError, RuntimeAppearanceCatalog, RuntimeAppearanceFact,
    RuntimeAppearanceProjector, RuntimeLightFact,
};
use runtime_diagnostics::RuntimeDiagnosticsSink;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::c_void,
    sync::Arc,
};

// Admission policy for immutable bundle-backed resource files. Generated mesh
// definitions remain typed and do not use this file-resource byte ceiling.
const MAX_ANIMATION_REALIZATION_FACTS: usize = 128;
const MAX_SPRITE_PLAYBACK_TRANSITIONS_PER_ADVANCE: usize = 16_384;

/// Latest bounded renderer realization snapshot for one opaque ghost owner.
/// It is observation only: product callbacks continue to own all ghost policy.
#[derive(Clone, Copy, Debug)]
pub struct GhostPlateRealizationFact {
    pub handle: u64,
    pub source_matches: bool,
    pub current_sector: u32,
    pub local_angular_offset_degrees: Option<f32>,
    pub fallback_active: bool,
    pub fallback_reason: NativeGhostPlateFallbackReason,
    pub limitation_mask: NativeGhostPlateLimitationMask,
    pub preparation_cpu_milliseconds: Option<f64>,
    pub capture_cpu_submission_milliseconds: Option<f64>,
    pub retained_sector_count: u32,
    pub retained_mesh_count: u32,
    pub retained_material_count: u32,
    pub retained_borrowed_texture_count: u32,
}

#[derive(Clone)]
pub enum AnimationRealizationFact {
    MeshInspection {
        fact_id: u64,
        object_id: u64,
        generation: u64,
        request: u32,
        bounds_min: [f32; 3],
        bounds_max: [f32; 3],
        has_bounds: bool,
        voxel_normal_meshes: u32,
    },
    Playback {
        fact_id: u64,
        object_id: u64,
        generation: u64,
        sequence: u32,
        status: String,
        clip: Option<String>,
        sampled_millis: Option<u64>,
    },
    NaturalCompletion {
        fact_id: u64,
        object_id: u64,
        generation: u64,
        clip: String,
    },
    Diagnostic {
        fact_id: u64,
        object_id: Option<u64>,
        generation: Option<u64>,
        code: String,
        sequence: u32,
    },
    Stopped {
        fact_id: u64,
        object_id: u64,
        generation: u64,
        sequence: u32,
        reason: String,
    },
}

impl RuntimeAppearanceData {
    /// The catalog material id of a live material handle.
    pub(crate) fn material_id(&self, material: u64) -> Option<&str> {
        self.materials.get(&material).map(String::as_str)
    }

    /// Adds a generated mesh definition to the renderer catalog.
    pub(crate) fn project_static_mesh(&mut self, definition: Arc<StaticMeshAsset>) {
        self.projector
            .resources_mut()
            .static_meshes
            .push(definition);
    }
}

impl RuntimeAppearanceState {
    pub(crate) fn output_object_handle(
        &self,
        id: u64,
    ) -> Result<RenderHandle, CsharpEngineServicesError> {
        self.projector.object_handle(id).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_OUTPUT_SOURCE",
                "output source object is absent from the retained appearance snapshot",
            )
        })
    }
}

pub(crate) fn native_light_descriptor(
    descriptor: NativeLightDescriptor,
) -> Result<LightDescriptor, CsharpEngineServicesError> {
    let shadow_intent = match descriptor.shadow_intent {
        NativeLightShadowIntent::Disabled => LightShadowIntent::Disabled,
        NativeLightShadowIntent::Requested => LightShadowIntent::Requested,
    };
    let color = native_vec3_array(descriptor.color);
    let position = native_vec3_array(descriptor.position);
    let direction = native_vec3_array(descriptor.direction);
    let range = descriptor.has_range.then_some(descriptor.range);
    let shadow = LightShadowSettings {
        resolution: descriptor.shadow_resolution,
        priority: descriptor.shadow_priority,
        soft: descriptor.shadow_soft,
    };
    let light = match descriptor.kind {
        NativeLightKind::Ambient => LightDescriptor::Ambient {
            color,
            intensity: descriptor.intensity,
            enabled: descriptor.enabled,
            range,
            shadow_intent,
            shadow,
        },
        NativeLightKind::Hemisphere => LightDescriptor::Hemisphere {
            color,
            ground_color: native_vec3_array(descriptor.ground_color),
            intensity: descriptor.intensity,
            enabled: descriptor.enabled,
        },
        NativeLightKind::Directional => LightDescriptor::Directional {
            color,
            intensity: descriptor.intensity,
            enabled: descriptor.enabled,
            direction,
            range,
            shadow_intent,
            shadow,
        },
        NativeLightKind::Point => LightDescriptor::Point {
            color,
            intensity: descriptor.intensity,
            enabled: descriptor.enabled,
            position,
            range,
            decay: descriptor.decay,
            shadow_intent,
            shadow,
        },
        NativeLightKind::Spot => LightDescriptor::Spot {
            color,
            intensity: descriptor.intensity,
            enabled: descriptor.enabled,
            position,
            direction,
            range,
            decay: descriptor.decay,
            outer_angle_radians: descriptor.outer_angle_radians,
            penumbra: descriptor.penumbra,
            shadow_intent,
            shadow,
        },
    };
    light.validate().map_err(|error| {
        CsharpEngineServicesError::new(
            "CSHARP_LIGHT_DESCRIPTOR",
            format!("invalid light descriptor: {error:?}"),
        )
    })?;
    Ok(light)
}

#[cfg(test)]
fn atlas_sprite_request(
    atlas: NativeSpriteAtlasHandle,
    frame_id: u32,
) -> NativeSpriteFromAtlasRequest {
    NativeSpriteFromAtlasRequest {
        atlas,
        frame_id,
        pivot: NativeVec2::default(),
        size: NativeVec2 { x: 1.0, y: 1.0 },
        billboard: NativeBillboardMode::Spherical,
        size_mode: NativeSpriteSizeMode::World,
        render_order: 3,
        depth: NativeSpriteDepthPolicy::Default,
        tint: NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        material: NativeSpriteMaterialDescriptor::default(),
    }
}

#[cfg(test)]
fn legacy_sprite_request(texture: NativeRenderResourceHandle) -> NativeSpriteAppearanceRequest {
    NativeSpriteAppearanceRequest {
        texture,
        uv_min: NativeVec2::default(),
        uv_max: NativeVec2 { x: 1.0, y: 1.0 },
        pivot: NativeVec2::default(),
        size: NativeVec2 { x: 1.0, y: 1.0 },
        billboard: NativeBillboardMode::None,
        size_mode: NativeSpriteSizeMode::World,
        render_order: 0,
        depth: NativeSpriteDepthPolicy::Default,
        tint: NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        material: NativeSpriteMaterialDescriptor::default(),
    }
}

#[cfg(test)]
#[test]
fn sprite_atlas_admits_more_frames_than_the_former_cap() {
    // 5,000 frames: past the former 4,096.
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let frames: Vec<_> = (0..5_000_u32)
        .map(|frame_id| NativeSpriteAtlasFrame {
            frame_id,
            uv_min: NativeVec2::default(),
            uv_max: NativeVec2 { x: 1.0, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        })
        .collect();
    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    unsafe {
        bridge.create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
            texture,
            frames: frames.as_ptr(),
            frames_len: frames.len(),
        })
    }
    .expect("a 5,000-frame atlas");
    bridge.end_call();
}

#[cfg(test)]
#[test]
fn sprite_atlas_copies_frames_resolves_readout_and_releases_with_appearance() {
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let frames = [
        NativeSpriteAtlasFrame {
            frame_id: 7,
            uv_min: NativeVec2::default(),
            uv_max: NativeVec2 { x: 0.5, y: 1.0 },
            has_size: true,
            size: NativeVec2 { x: 16.0, y: 32.0 },
        },
        NativeSpriteAtlasFrame {
            frame_id: 9,
            uv_min: NativeVec2 { x: 0.5, y: 0.0 },
            uv_max: NativeVec2 { x: 1.0, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        },
    ];
    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    assert_eq!(
        unsafe {
            bridge.create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture: NativeRenderResourceHandle::default(),
                frames: frames.as_ptr(),
                frames_len: frames.len(),
            })
        }
        .expect_err("zero cannot alias the first admitted texture")
        .code(),
        "CSHARP_RENDER_RESOURCE_HANDLE"
    );
    let atlas = unsafe {
        bridge
            .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture,
                frames: frames.as_ptr(),
                frames_len: frames.len(),
            })
            .expect("atlas")
    };
    let sprite = bridge
        .create_sprite_from_atlas(atlas_sprite_request(atlas, 7))
        .expect("atlas sprite");
    assert_eq!(
        bridge.read_sprite(sprite).expect("initial frame").size.x,
        16.0
    );
    bridge
        .set_sprite_frame(NativeSpriteFrameUpdateRequest {
            appearance: sprite,
            frame_id: 9,
        })
        .expect("select second frame");
    let readout = bridge.read_sprite(sprite).expect("selected frame");
    assert_eq!(readout.atlas.value, atlas.value);
    assert_eq!(readout.frame_id, 9);
    assert_eq!(readout.uv_min.x, 0.5);
    assert!(!readout.has_size);
    assert_eq!(
        bridge
            .destroy_sprite_atlas(atlas)
            .expect_err("atlas in use")
            .code(),
        "CSHARP_SPRITE_ATLAS_IN_USE"
    );
    bridge
        .destroy_appearance(sprite)
        .expect("release sprite lease");
    bridge.destroy_sprite_atlas(atlas).expect("release atlas");
}

#[cfg(test)]
#[test]
fn sprite_viewport_updates_retain_valid_placement_and_clear_it_when_disabled() {
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let frames = [NativeSpriteAtlasFrame {
        frame_id: 7,
        uv_min: NativeVec2::default(),
        uv_max: NativeVec2 { x: 1.0, y: 1.0 },
        has_size: true,
        size: NativeVec2 { x: 16.0, y: 8.0 },
    }];
    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    let atlas = unsafe {
        bridge
            .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture,
                frames: frames.as_ptr(),
                frames_len: frames.len(),
            })
            .expect("atlas")
    };
    let sprite = bridge
        .create_sprite_from_atlas(atlas_sprite_request(atlas, 7))
        .expect("atlas sprite");
    bridge
        .set_sprite_viewport(NativeSpriteViewportUpdateRequest {
            appearance: sprite,
            enabled: true,
            minimum: NativeVec2 { x: -0.1, y: 0.0 },
            size: NativeVec2 { x: 0.5, y: 0.25 },
            alignment: NativeVec2 { x: 0.5, y: 0.0 },
            fit: NativeSpriteViewportFit::Contain,
        })
        .expect("valid overscan placement");
    assert_eq!(
        bridge
            .set_sprite_viewport(NativeSpriteViewportUpdateRequest {
                appearance: sprite,
                enabled: true,
                minimum: NativeVec2::default(),
                size: NativeVec2 { x: 0.0, y: 1.0 },
                alignment: NativeVec2::default(),
                fit: NativeSpriteViewportFit::Stretch,
            })
            .expect_err("zero size is rejected")
            .code(),
        "CSHARP_SPRITE_VIEWPORT"
    );
    bridge
        .set_sprite_viewport(NativeSpriteViewportUpdateRequest {
            appearance: sprite,
            enabled: false,
            minimum: NativeVec2::default(),
            size: NativeVec2::default(),
            alignment: NativeVec2::default(),
            fit: NativeSpriteViewportFit::Stretch,
        })
        .expect("disabled placement clears the optional fact");
}

#[cfg(test)]
#[test]
fn atlas_sprite_failures_and_legacy_replacement_leave_or_release_the_lease() {
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let frames = [NativeSpriteAtlasFrame {
        frame_id: 4,
        uv_min: NativeVec2::default(),
        uv_max: NativeVec2 { x: 1.0, y: 1.0 },
        has_size: false,
        size: NativeVec2::default(),
    }];
    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    let atlas = unsafe {
        bridge
            .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture,
                frames: frames.as_ptr(),
                frames_len: frames.len(),
            })
            .expect("atlas")
    };
    let sprite = bridge
        .create_sprite_from_atlas(atlas_sprite_request(atlas, 4))
        .expect("atlas sprite");
    assert_eq!(
        bridge
            .set_sprite_frame(NativeSpriteFrameUpdateRequest {
                appearance: sprite,
                frame_id: 99,
            })
            .expect_err("unknown frame")
            .code(),
        "CSHARP_SPRITE_ATLAS_FRAME"
    );
    assert_eq!(
        bridge
            .read_sprite(sprite)
            .expect("unchanged sprite")
            .frame_id,
        4
    );
    assert_eq!(
        bridge
            .replace_sprite_from_atlas(NativeSpriteFromAtlasReplaceRequest {
                appearance: sprite,
                replacement: atlas_sprite_request(atlas, 99),
            })
            .expect_err("unknown replacement frame")
            .code(),
        "CSHARP_SPRITE_ATLAS_FRAME"
    );
    assert_eq!(
        bridge
            .read_sprite(sprite)
            .expect("replacement left sprite intact")
            .frame_id,
        4
    );
    let mut invalid_descriptor = atlas_sprite_request(atlas, 4);
    invalid_descriptor.size.x = 0.0;
    assert_eq!(
        bridge
            .replace_sprite_from_atlas(NativeSpriteFromAtlasReplaceRequest {
                appearance: sprite,
                replacement: invalid_descriptor,
            })
            .expect_err("invalid replacement descriptor")
            .code(),
        "CSHARP_SPRITE_ATLAS_FRAME"
    );
    assert_eq!(
        bridge
            .read_sprite(sprite)
            .expect("invalid descriptor left sprite intact")
            .frame_id,
        4
    );
    let primitive = bridge
        .create_primitive(tests::primitive_request())
        .expect("primitive");
    assert_eq!(
        bridge
            .set_sprite_frame(NativeSpriteFrameUpdateRequest {
                appearance: primitive,
                frame_id: 4,
            })
            .expect_err("wrong appearance kind")
            .code(),
        "CSHARP_SPRITE_ATLAS_APPEARANCE"
    );
    let replacement = bridge
        .replace_sprite(NativeSpriteAppearanceReplaceRequest {
            appearance: sprite,
            replacement: legacy_sprite_request(texture),
        })
        .expect("legacy replacement validates before releasing atlas sprite");
    assert_ne!(replacement.value, sprite.value);
    bridge
        .destroy_sprite_atlas(atlas)
        .expect("legacy replacement released atlas lease");
}

#[cfg(test)]
fn realtime_sprite_update(step: u64, delta: f64) -> NativeProductUpdateFacts {
    NativeProductUpdateFacts {
        lifecycle_state: NativeProductLifecycleState::Running,
        generation: 1,
        control_revision: 1,
        observed_host_time_nanoseconds: step * 1_000_000,
        simulation_step: step,
        fixed_step_hz: 4,
        admitted_step_count: 1,
        dropped_step_count: 0,
        fixed_delta_seconds: delta,
        gameplay_time_selected: false,
        gameplay_rate: 1.0,
        gameplay_advance_remaining_steps: 0,
        host_elapsed_seconds: 0.0,
    }
}

#[cfg(test)]
fn sprite_appearance_fact(appearance: NativeAppearanceHandle) -> NativeAppearanceFact {
    NativeAppearanceFact {
        object_id: 71,
        has_parent_object: false,
        parent_object_id: 0,
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
        shadow_casting: Default::default(),
    }
}

#[cfg(test)]
#[test]
fn sprite_playback_advances_repeated_frames_once_and_controls_lifetime() {
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let atlas_frames = [
        NativeSpriteAtlasFrame {
            frame_id: 7,
            uv_min: NativeVec2::default(),
            uv_max: NativeVec2 { x: 0.5, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        },
        NativeSpriteAtlasFrame {
            frame_id: 9,
            uv_min: NativeVec2 { x: 0.5, y: 0.0 },
            uv_max: NativeVec2 { x: 1.0, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        },
    ];
    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    let atlas = unsafe {
        bridge
            .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture,
                frames: atlas_frames.as_ptr(),
                frames_len: atlas_frames.len(),
            })
            .expect("atlas")
    };
    let appearance = bridge
        .create_sprite_from_atlas(atlas_sprite_request(atlas, 7))
        .expect("sprite");
    let frames = [
        NativeSpritePlaybackFrame {
            frame_id: 7,
            duration_seconds: 0.25,
        },
        NativeSpritePlaybackFrame {
            frame_id: 7,
            duration_seconds: 0.25,
        },
        NativeSpritePlaybackFrame {
            frame_id: 9,
            duration_seconds: 0.25,
        },
    ];
    let markers = [
        NativeSpritePlaybackMarker {
            marker_id: 11,
            frame_index: 1,
        },
        NativeSpritePlaybackMarker {
            marker_id: 12,
            frame_index: 2,
        },
    ];
    let playback = unsafe {
        bridge
            .create_sprite_playback(&NativeSpritePlaybackCreateRequest {
                appearance,
                atlas,
                frames: frames.as_ptr(),
                frames_len: frames.len(),
                markers: markers.as_ptr(),
                markers_len: markers.len(),
                loop_mode: NativeSpritePlaybackLoopMode::OneShot,
                playback_rate: 1.0,
            })
            .expect("playback")
    };
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Start,
        })
        .expect("start");
    let appearance_fact = sprite_appearance_fact(appearance);
    unsafe { bridge.stage_snapshot(&appearance_fact, 1) }.expect("initial retained snapshot");
    let initial_call = bridge.take_staged_call();
    bridge.commit(initial_call);
    bridge.begin_call();
    assert_eq!(
        bridge
            .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
            .expect_err("non-update callback cannot advance")
            .code(),
        "CSHARP_SPRITE_PLAYBACK_UPDATE"
    );
    bridge.end_call();
    bridge.begin_update_call(realtime_sprite_update(1, 0.25));
    let first = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("exact first boundary");
    let first_crossings =
        unsafe { std::slice::from_raw_parts(first.crossings, first.crossings_len) };
    assert_eq!(first.readout.frame_index, 1);
    assert_eq!(first.readout.frame_id, 7);
    assert_eq!(first_crossings.len(), 1);
    assert_eq!(first_crossings[0].marker_id, 11);
    let duplicate = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("duplicate is idempotent");
    assert!(!duplicate.advanced);
    assert_eq!(duplicate.crossings_len, 0);
    assert_eq!(duplicate.readout.revision, first.readout.revision);
    let first_call = bridge.take_staged_call();
    bridge.commit(first_call);
    bridge.begin_update_call(realtime_sprite_update(2, 0.25));
    let second = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("second boundary");
    assert_eq!(second.readout.frame_id, 9);
    assert_eq!(
        bridge
            .read_sprite(appearance)
            .expect("renderer fact")
            .frame_id,
        9
    );
    unsafe { bridge.stage_snapshot(&appearance_fact, 1) }.expect("updated retained snapshot");
    let updated_call = bridge.take_staged_call();
    assert!(updated_call.render_frames().iter().any(|frame| {
        frame.ops.iter().any(|operation| {
            matches!(
                operation,
                render_model::RenderDiff::UpdateSprite { frame: Some(9), .. }
            )
        })
    }));
    bridge.commit(updated_call);
    bridge.begin_update_call(realtime_sprite_update(3, 0.25));
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Pause,
        })
        .expect("pause");
    let paused = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("paused update");
    assert!(!paused.advanced);
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Resume,
        })
        .expect("resume");
    let paused_call = bridge.take_staged_call();
    bridge.commit(paused_call);
    bridge.begin_update_call(realtime_sprite_update(4, 0.25));
    let completed = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("one shot completion");
    assert!(completed.readout.completed);
    assert_eq!(
        completed.readout.state,
        NativeSpritePlaybackState::Completed
    );
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Restart,
        })
        .expect("restart after completion");
    let after_restart = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("restart preserves consumed update identity");
    assert!(!after_restart.advanced);
    assert_eq!(after_restart.crossings_len, 0);
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Stop,
        })
        .expect("stop after restart");
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Start,
        })
        .expect("start after stop");
    let after_stop_start = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("stop and start preserve consumed update identity");
    assert!(!after_stop_start.advanced);
    assert_eq!(after_stop_start.crossings_len, 0);
    unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.expect("remove retained sprite");
    let removal_call = bridge.take_staged_call();
    bridge.commit(removal_call);
    bridge.begin_update_call(realtime_sprite_update(3, 0.25));
    assert_eq!(
        bridge
            .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
            .expect_err("stale admitted update")
            .code(),
        "CSHARP_SPRITE_PLAYBACK_STALE_UPDATE"
    );
    bridge.end_call();
    bridge.begin_call();
    assert_eq!(
        bridge
            .destroy_appearance(appearance)
            .expect_err("playback retains appearance")
            .code(),
        "CSHARP_SPRITE_PLAYBACK_APPEARANCE_IN_USE"
    );
    bridge
        .destroy_sprite_playback(playback)
        .expect("dispose playback");
    bridge
        .destroy_appearance(appearance)
        .expect("dispose sprite");
    bridge.destroy_sprite_atlas(atlas).expect("dispose atlas");
}

#[cfg(test)]
#[test]
fn sprite_playback_loop_sampling_restart_and_invalid_creation_are_atomic() {
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let atlas_frames = [NativeSpriteAtlasFrame {
        frame_id: 4,
        uv_min: NativeVec2::default(),
        uv_max: NativeVec2 { x: 1.0, y: 1.0 },
        has_size: false,
        size: NativeVec2::default(),
    }];
    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    let atlas = unsafe {
        bridge
            .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture,
                frames: atlas_frames.as_ptr(),
                frames_len: atlas_frames.len(),
            })
            .expect("atlas")
    };
    let appearance = bridge
        .create_sprite_from_atlas(atlas_sprite_request(atlas, 4))
        .expect("sprite");
    let invalid = [NativeSpritePlaybackFrame {
        frame_id: 99,
        duration_seconds: 0.5,
    }];
    assert_eq!(
        unsafe {
            bridge.create_sprite_playback(&NativeSpritePlaybackCreateRequest {
                appearance,
                atlas,
                frames: invalid.as_ptr(),
                frames_len: invalid.len(),
                markers: std::ptr::null(),
                markers_len: 0,
                loop_mode: NativeSpritePlaybackLoopMode::Loop,
                playback_rate: 1.0,
            })
        }
        .expect_err("missing frame")
        .code(),
        "CSHARP_SPRITE_PLAYBACK_FRAME"
    );
    assert!(bridge
        .staged_ref()
        .unwrap()
        .state
        .sprite_playbacks
        .is_empty());
    let frames = [NativeSpritePlaybackFrame {
        frame_id: 4,
        duration_seconds: 0.5,
    }];
    let playback = unsafe {
        bridge
            .create_sprite_playback(&NativeSpritePlaybackCreateRequest {
                appearance,
                atlas,
                frames: frames.as_ptr(),
                frames_len: frames.len(),
                markers: std::ptr::null(),
                markers_len: 0,
                loop_mode: NativeSpritePlaybackLoopMode::Loop,
                playback_rate: 2.0,
            })
            .expect("loop playback")
    };
    let sample = bridge
        .sample_sprite_playback(NativeSpritePlaybackSampleRequest {
            playback,
            elapsed_seconds: 1.25,
        })
        .expect("sample");
    assert_eq!(sample.cycle, 5);
    assert_eq!(sample.frame_id, 4);
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Restart,
        })
        .expect("restart starts from zero");
    let setup_call = bridge.take_staged_call();
    bridge.commit(setup_call);
    bridge.begin_update_call(realtime_sprite_update(1, 0.5));
    let looped = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("loop advance");
    assert_eq!(looped.readout.cycle, 2);
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Stop,
        })
        .expect("stop");
    let stopped = bridge.read_sprite_playback(playback).expect("readback");
    assert_eq!(stopped.state, NativeSpritePlaybackState::Stopped);
    assert_eq!(stopped.frame_index, 0);
}

#[cfg(test)]
#[test]
fn sprite_playback_frame_selection_updates_cursor_and_renderer_atomically() {
    let mut content_resources = BTreeMap::new();
    content_resources.insert("atlas.png".to_owned(), Arc::from(tests::RGBA_PNG));
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
    let atlas_frames = [
        NativeSpriteAtlasFrame {
            frame_id: 7,
            uv_min: NativeVec2::default(),
            uv_max: NativeVec2 { x: 0.5, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        },
        NativeSpriteAtlasFrame {
            frame_id: 9,
            uv_min: NativeVec2 { x: 0.5, y: 0.0 },
            uv_max: NativeVec2 { x: 1.0, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        },
    ];
    let frames = [
        NativeSpritePlaybackFrame {
            frame_id: 7,
            duration_seconds: 0.25,
        },
        NativeSpritePlaybackFrame {
            frame_id: 9,
            duration_seconds: 0.25,
        },
    ];
    let markers = [NativeSpritePlaybackMarker {
        marker_id: 11,
        frame_index: 0,
    }];

    bridge.begin_call();
    let texture = bridge
        .open_resource(&tests::resource_request("atlas.png"))
        .expect("atlas texture")
        .handle;
    let atlas = unsafe {
        bridge
            .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                texture,
                frames: atlas_frames.as_ptr(),
                frames_len: atlas_frames.len(),
            })
            .expect("atlas")
    };
    let appearance = bridge
        .create_sprite_from_atlas(atlas_sprite_request(atlas, 7))
        .expect("sprite");
    let playback = unsafe {
        bridge
            .create_sprite_playback(&NativeSpritePlaybackCreateRequest {
                appearance,
                atlas,
                frames: frames.as_ptr(),
                frames_len: frames.len(),
                markers: markers.as_ptr(),
                markers_len: markers.len(),
                loop_mode: NativeSpritePlaybackLoopMode::Loop,
                playback_rate: 1.0,
            })
            .expect("loop playback")
    };
    bridge
        .control_sprite_playback(NativeSpritePlaybackControlRequest {
            playback,
            control: NativeSpritePlaybackControl::Start,
        })
        .expect("start");
    let setup = bridge.take_staged_call();
    bridge.commit(setup);

    bridge.begin_update_call(realtime_sprite_update(1, 0.5));
    let looped = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("loop once");
    assert_eq!(looped.readout.cycle, 1);
    assert_eq!(looped.readout.frame_index, 0);
    assert_eq!(looped.crossings_len, 1);
    let mut selected = std::mem::MaybeUninit::<NativeSpritePlaybackReadout>::uninit();
    assert_eq!(
        unsafe {
            select_sprite_playback_frame(
                (&mut bridge as *mut RuntimeAppearanceBridge).cast(),
                NativeSpritePlaybackFrameSelectionRequest {
                    playback,
                    frame_index: 1,
                },
                selected.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        },
        ABI_OK,
        "generated ABI callback selects the sequence entry"
    );
    let selected = unsafe { selected.assume_init() };
    assert_eq!(selected.state, NativeSpritePlaybackState::Playing);
    assert_eq!(selected.frame_index, 1);
    assert_eq!(selected.frame_id, 9);
    assert_eq!(selected.elapsed_in_frame_seconds, 0.0);
    assert_eq!(
        selected.cycle, 1,
        "selection preserves the current loop cycle"
    );
    assert_eq!(selected.revision, looped.readout.revision + 1);
    assert_eq!(
        bridge
            .read_sprite(appearance)
            .expect("selected sprite")
            .frame_id,
        9
    );
    let duplicate = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("same admitted update stays consumed");
    assert!(!duplicate.advanced);
    assert_eq!(duplicate.crossings_len, 0);
    assert_eq!(duplicate.readout.frame_index, 1);
    let selected_call = bridge.take_staged_call();
    bridge.commit(selected_call);

    bridge.begin_update_call(realtime_sprite_update(2, 0.25));
    let continued = bridge
        .advance_sprite_playback(NativeSpritePlaybackAdvanceRequest { playback })
        .expect("advance from selected cursor");
    assert!(continued.advanced);
    assert_eq!(continued.readout.frame_index, 0);
    assert_eq!(continued.readout.cycle, 2);
    let continued_crossings =
        unsafe { std::slice::from_raw_parts(continued.crossings, continued.crossings_len) };
    assert_eq!(continued_crossings.len(), 1);
    assert_eq!(continued_crossings[0].marker_id, 11);

    let before_invalid = bridge
        .read_sprite_playback(playback)
        .expect("before invalid");
    let before_invalid_sprite = bridge
        .read_sprite(appearance)
        .expect("before invalid sprite");
    assert_eq!(
        bridge
            .select_sprite_playback_frame(NativeSpritePlaybackFrameSelectionRequest {
                playback,
                frame_index: 2,
            })
            .expect_err("selection never wraps")
            .code(),
        "CSHARP_SPRITE_PLAYBACK_FRAME_INDEX"
    );
    assert_eq!(
        bridge
            .read_sprite_playback(playback)
            .expect("after invalid")
            .frame_index,
        before_invalid.frame_index
    );
    assert_eq!(
        bridge
            .read_sprite(appearance)
            .expect("after invalid sprite")
            .frame_id,
        before_invalid_sprite.frame_id
    );
    assert_eq!(
        sprite_playback_state_after_selection(NativeSpritePlaybackState::Stopped),
        NativeSpritePlaybackState::Stopped
    );
    assert_eq!(
        sprite_playback_state_after_selection(NativeSpritePlaybackState::Playing),
        NativeSpritePlaybackState::Playing
    );
    assert_eq!(
        sprite_playback_state_after_selection(NativeSpritePlaybackState::Paused),
        NativeSpritePlaybackState::Paused
    );
    assert_eq!(
        sprite_playback_state_after_selection(NativeSpritePlaybackState::Completed),
        NativeSpritePlaybackState::Paused
    );
}

#[cfg(test)]
thread_local! {
    pub(crate) static GRAPHICS_SNAPSHOT_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static MEDIA_SNAPSHOT_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static RESOURCE_INVENTORY_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Graphics state. A product call owns it for the call's duration, so the
/// first write never copies it.
#[derive(Clone)]
pub(crate) struct RuntimeAppearanceState(Arc<RuntimeAppearanceData>);
impl From<RuntimeAppearanceData> for RuntimeAppearanceState {
    fn from(value: RuntimeAppearanceData) -> Self {
        Self(Arc::new(value))
    }
}
impl std::ops::Deref for RuntimeAppearanceState {
    type Target = RuntimeAppearanceData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RuntimeAppearanceState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeAppearanceData {
    pub(crate) projector: RuntimeAppearanceProjector,
    appearances: BTreeMap<u64, String>,
    next_appearance: u64,
    pub(crate) generated_meshes: GeneratedMeshes,
    mesh_appearances: BTreeMap<u64, u64>,
    lights: BTreeMap<u64, RuntimeLightFact>,
    /// Live lights whose parent object is not in the published scene: they
    /// are not projected and show once a snapshot publishes the parent.
    pending_lights: BTreeSet<u64>,
    next_light: u64,
    materials: BTreeMap<u64, String>,
    appearance_materials: BTreeMap<u64, BTreeSet<u64>>,
    next_material: u64,
    pub(crate) render_resources: RenderResourceRegistry,
    /// Direct users hold exact resource handles, never projector catalog entries.
    appearance_resources: BTreeMap<u64, BTreeSet<u64>>,
    material_resources: BTreeMap<u64, BTreeSet<u64>>,
    sprite_atlas_resources: BTreeMap<u64, BTreeSet<u64>>,
    animation_graph_resources: BTreeMap<u64, BTreeSet<u64>>,
    animation_clip_pack_resources: BTreeMap<u64, BTreeSet<u64>>,
    billboard_resources: BTreeMap<u64, BTreeSet<u64>>,
    emitter_resources: BTreeMap<u64, BTreeSet<u64>>,
    sprite_atlases: BTreeMap<u64, RuntimeSpriteAtlas>,
    sprite_atlas_appearances: BTreeMap<u64, BTreeSet<u64>>,
    sprite_appearance_atlases: BTreeMap<u64, u64>,
    next_sprite_atlas: u64,
    sprite_playbacks: BTreeMap<u64, RuntimeSpritePlayback>,
    sprite_playbacks_by_atlas: BTreeMap<u64, BTreeSet<u64>>,
    sprite_playbacks_by_appearance: BTreeMap<u64, BTreeSet<u64>>,
    /// Static-mesh appearances voxel scatters grow, with how many do.
    scatter_appearances: BTreeMap<u64, u32>,
    next_sprite_playback: u64,
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
    ghost_plate_projector: GhostPlateProjector,
    ghost_plates: BTreeMap<u64, RuntimeGhostPlatePresentation>,
    next_ghost_plate: u64,
    pub(crate) tweens: crate::tween::RuntimeTweens,
}

#[derive(Clone)]
struct RuntimeGhostPlatePresentation {
    source_object_id: u64,
    placement: GhostPlatePlacement,
    capture: GhostPlateCaptureSettings,
    config: GhostPlateConfig,
}

#[derive(Clone)]
struct RuntimeSpriteAtlas {
    asset: String,
    texture_asset: String,
    frames: BTreeMap<u32, SpriteFrameRect>,
}

#[derive(Clone)]
struct RuntimeSpritePlayback {
    appearance: u64,
    atlas: u64,
    frames: Vec<NativeSpritePlaybackFrame>,
    markers: Vec<NativeSpritePlaybackMarker>,
    loop_mode: NativeSpritePlaybackLoopMode,
    playback_rate: f64,
    state: NativeSpritePlaybackState,
    frame_index: usize,
    elapsed_in_frame_seconds: f64,
    cycle: u64,
    revision: u64,
    next_crossing_sequence: u64,
    last_update: Option<SpritePlaybackUpdateIdentity>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SpritePlaybackUpdateIdentity {
    generation: u64,
    control_revision: u64,
    simulation_step: u64,
    admitted_step_count: u32,
}

#[derive(Clone)]
pub(crate) struct RuntimeAppearanceCall {
    pub(crate) state: RuntimeAppearanceState,
    admitted_update: Option<NativeProductUpdateFacts>,
    resource_releases_pending: bool,
    pub(crate) release_error: Option<CsharpEngineServicesError>,
    /// Typed renderer realization work in the order the C# product invoked the
    /// owning appearance APIs. This remains call-local: it is not a general
    /// output transport and only represents this service family's existing
    /// renderer and presentation outputs.
    pub(crate) outputs: Vec<RuntimeAppearanceCallOutput>,
    /// A retained snapshot or light projection frame was emitted in this call.
    projected_frame: bool,
    /// Presentation frames emitted in this call; their sequence numbers.
    presentation_frames: u32,
}

#[derive(Clone)]
pub(crate) enum RuntimeAppearanceCallOutput {
    Frame(render_model::RenderFrameDiff),
    Presentation(PresentationFrameDiff),
}

const MAX_PRESENTATION_DIAGNOSTICS: usize = 128;

#[derive(Clone, Copy)]
struct StoredPresentationDiagnostic {
    domain: NativePresentationDiagnosticDomain,
    receipt: NativePresentationDiagnostic,
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
    /// The projection detached because its target object was recreated; the
    /// next flush recreates it on the new target with the same clip phases.
    detached: Option<AnimationProjectionDescriptor>,
}

#[cfg(test)]
impl RuntimeAppearanceCall {
    /// Render frames emitted in this call, in order.
    pub(crate) fn render_frames(&self) -> Vec<&render_model::RenderFrameDiff> {
        self.outputs
            .iter()
            .filter_map(|output| match output {
                RuntimeAppearanceCallOutput::Frame(frame) => Some(frame),
                _ => None,
            })
            .collect()
    }

    /// Every render operation emitted in this call, in order.
    pub(crate) fn render_ops(&self) -> Vec<RenderDiff> {
        self.render_frames()
            .into_iter()
            .flat_map(|frame| frame.ops.iter().cloned())
            .collect()
    }

    /// Presentation frames emitted in this call, in order.
    pub(crate) fn presentation(&self) -> Vec<&PresentationFrameDiff> {
        self.outputs
            .iter()
            .filter_map(|output| match output {
                RuntimeAppearanceCallOutput::Presentation(frame) => Some(frame),
                _ => None,
            })
            .collect()
    }
}

impl RuntimeAppearanceCall {
    /// An image effect's shader, its textures and its descriptor, from the
    /// resources the product opened.
    pub(crate) fn image_effect(
        &self,
        request: csharp_engine_abi::NativeImageEffectRequest,
    ) -> Result<
        (
            render_model::ShaderDescriptor,
            Vec<TextureDescriptor>,
            render_model::ImageEffectDescriptor,
        ),
        CsharpEngineServicesError,
    > {
        let refused =
            |message: &'static str| CsharpEngineServicesError::new("CSHARP_IMAGE_EFFECT", message);
        let shader = self
            .state
            .render_resources
            .resource(request.shader.value)
            .ok()
            .and_then(|resource| resource.shader())
            .ok_or_else(|| {
                refused("an image effect's shader must be a resource opened from a .wgsl file")
            })?;
        let mut textures = Vec::new();
        let mut texture = |reference: csharp_engine_abi::NativeRenderResourceReference| {
            if reference.value == 0 {
                return Ok(None);
            }
            let texture = self
                .state
                .render_resources
                .resource(reference.value)
                .ok()
                .and_then(|resource| resource.texture().cloned())
                .ok_or_else(|| refused("an image effect's textures must be texture resources"))?;
            let id = texture.id.clone();
            textures.push(texture);
            Ok::<_, CsharpEngineServicesError>(Some(id))
        };
        let row = |value: csharp_engine_abi::NativeVec4| [value.x, value.y, value.z, value.w];
        let effect = render_model::ImageEffectDescriptor {
            shader: shader.id.clone(),
            parameters: [
                row(request.parameter_0),
                row(request.parameter_1),
                row(request.parameter_2),
                row(request.parameter_3),
            ],
            textures: [texture(request.texture_a)?, texture(request.texture_b)?],
        };
        Ok((shader, textures, effect))
    }

    pub(crate) fn texture_descriptor(
        &self,
        handle: u64,
    ) -> Result<TextureDescriptor, CsharpEngineServicesError> {
        let resource = self.state.render_resources.resource(handle)?;
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SKY_TEXTURE",
                "sky background requires a selected texture resource",
            ));
        }
        resource.texture().cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SKY_TEXTURE",
                "sky background texture descriptor is not retained",
            )
        })
    }
}

/// Engine-owned appearance admission and retained projection for trusted C# products.
/// `Create` selects the immutable resources the renderer reads; calls stage resource
/// selection, newly admitted appearances, and snapshots so a failure cannot partly advance
/// renderer-visible state.
pub(crate) struct RuntimeAppearanceBridge {
    pub(crate) state: RuntimeAppearanceState,
    /// Held in `state` while a product call owns the real state.
    idle_state: RuntimeAppearanceState,
    content_resources: BTreeMap<String, Arc<[u8]>>,
    imports: RenderResourceImports,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
    /// Backing of the latest borrowed appearance result.
    pub(crate) borrowed: crate::operation_diagnostics::BorrowedResult,
    content: Option<*const crate::content::RuntimeContentBridge>,
    camera_view: Option<*const crate::camera_view::RuntimeCameraViewBridge>,
    staged: Option<RuntimeAppearanceCall>,
    operation_error: Option<CsharpEngineServicesError>,
    presentation_diagnostics: Vec<StoredPresentationDiagnostic>,
    ghost_plate_realization: BTreeMap<u64, GhostPlateRealizationFact>,
    animation_realization_facts: VecDeque<AnimationRealizationFact>,
    animation_realization_evicted: u64,
    authored_content: Option<*const crate::authored_content::RuntimeAuthoredContentBridge>,
    diagnostics_sink: Option<RuntimeDiagnosticsSink>,
    reported_recoverable_codes: BTreeSet<&'static str>,
    /// The world time tweens last saw owed toward the next step.
    tween_owed_seconds: f64,
}

impl RuntimeAppearanceBridge {
    pub(crate) fn new(
        catalog: RuntimeAppearanceCatalog,
        content_resources: BTreeMap<String, Arc<[u8]>>,
    ) -> Self {
        let state = RuntimeAppearanceState::from(RuntimeAppearanceData {
            projector: RuntimeAppearanceProjector::new(catalog),
            appearances: BTreeMap::new(),
            next_appearance: 1,
            generated_meshes: GeneratedMeshes::default(),
            mesh_appearances: BTreeMap::new(),
            lights: BTreeMap::new(),
            pending_lights: BTreeSet::new(),
            next_light: 1,
            materials: BTreeMap::new(),
            appearance_materials: BTreeMap::new(),
            next_material: 1,
            render_resources: RenderResourceRegistry::default(),
            appearance_resources: BTreeMap::new(),
            material_resources: BTreeMap::new(),
            sprite_atlas_resources: BTreeMap::new(),
            animation_graph_resources: BTreeMap::new(),
            animation_clip_pack_resources: BTreeMap::new(),
            billboard_resources: BTreeMap::new(),
            emitter_resources: BTreeMap::new(),
            sprite_atlases: BTreeMap::new(),
            sprite_atlas_appearances: BTreeMap::new(),
            sprite_appearance_atlases: BTreeMap::new(),
            next_sprite_atlas: 1,
            sprite_playbacks: BTreeMap::new(),
            sprite_playbacks_by_atlas: BTreeMap::new(),
            sprite_playbacks_by_appearance: BTreeMap::new(),
            scatter_appearances: BTreeMap::new(),
            next_sprite_playback: 1,
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
            ghost_plate_projector: GhostPlateProjector::default(),
            ghost_plates: BTreeMap::new(),
            next_ghost_plate: 1,
            tweens: Default::default(),
        });
        Self {
            idle_state: state.clone(),
            state,
            content_resources,
            imports: RenderResourceImports::default(),
            operation_diagnostics: Default::default(),
            borrowed: Default::default(),
            content: None,
            camera_view: None,
            staged: None,
            operation_error: None,
            presentation_diagnostics: Vec::new(),
            ghost_plate_realization: BTreeMap::new(),
            animation_realization_facts: VecDeque::new(),
            animation_realization_evicted: 0,
            authored_content: None,
            diagnostics_sink: None,
            reported_recoverable_codes: BTreeSet::new(),
            tween_owed_seconds: 0.0,
        }
    }

    pub(crate) fn bind_content(&mut self, content: &crate::content::RuntimeContentBridge) {
        self.content = Some(content);
    }

    pub(crate) fn bind_camera_view(
        &mut self,
        camera_view: &crate::camera_view::RuntimeCameraViewBridge,
    ) {
        // The composed service set owns this boxed bridge for Appearance's lifetime.
        self.camera_view = Some(camera_view);
    }

    fn content_reference(
        &self,
        reference: NativeContentReferenceHandle,
    ) -> Result<crate::content::RetainedContent, CsharpEngineServicesError> {
        // EngineServiceSet keeps the boxed content bridge alive and immobile.
        self.content
            .and_then(|content| unsafe { &*content }.retained_content(reference))
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_RENDER_CONTENT_REFERENCE",
                    "renderer content reference is not live",
                )
            })
    }

    fn content_path(
        &self,
        path: &str,
    ) -> Result<crate::content::RetainedContent, CsharpEngineServicesError> {
        if let Some(content) = self
            .content
            .and_then(|content| unsafe { &*content }.retained_path(path))
        {
            return Ok(content);
        }
        let path = path.strip_prefix("content/").unwrap_or(path);
        let bytes = self.content_resources.get(path).cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_UNKNOWN",
                format!("product content has no renderer resource `{path}`"),
            )
        })?;
        Ok(crate::content::RetainedContent {
            path: path.to_owned(),
            identity: Default::default(),
            bytes,
            transient: false,
            files: crate::content::ContentFiles::snapshot(self.content_resources.clone()),
        })
    }

    pub(crate) fn bind_authored_content(
        &mut self,
        authored_content: &crate::authored_content::RuntimeAuthoredContentBridge,
    ) {
        self.authored_content = Some(authored_content);
    }

    pub(crate) fn bind_diagnostics_sink(&mut self, sink: RuntimeDiagnosticsSink) {
        self.diagnostics_sink = Some(sink);
    }

    #[cfg(test)]
    pub(crate) fn begin_call(&mut self) {
        self.begin_call_with_update(None, None);
    }

    #[cfg(test)]
    pub(crate) fn begin_update_call(&mut self, facts: NativeProductUpdateFacts) {
        self.begin_call_with_update(
            Some(facts),
            Some(crate::tween::TweenTime::of_update(&facts)),
        );
    }

    pub(crate) fn tweens_playing(&self) -> bool {
        crate::tween::playing(&self.state)
    }

    /// Begins a call; with a `clock` (an update, or the Engine's own tween
    /// call) its tweens advance first.
    pub(crate) fn begin_call_with_update(
        &mut self,
        admitted_update: Option<NativeProductUpdateFacts>,
        clock: Option<crate::tween::TweenTime>,
    ) {
        // Move the state into the call and leave the idle placeholder behind,
        // so the call's first write does not copy the whole graphics state.
        let mut state = std::mem::replace(&mut self.state, self.idle_state.clone());
        // Checked first so that a call with nothing released does not write.
        if state.render_resources.recently_released().next().is_some() {
            state.render_resources.begin_call();
        }
        if let Some(clock) = clock {
            let admitted_seconds = admitted_update.as_ref().map_or(0.0, |facts| {
                facts.fixed_delta_seconds * f64::from(facts.admitted_step_count)
            });
            crate::tween::advance(
                &mut state,
                &mut self.tween_owed_seconds,
                admitted_seconds,
                clock,
                admitted_update.is_some(),
            );
        }
        self.staged = Some(RuntimeAppearanceCall {
            state,
            admitted_update,
            resource_releases_pending: false,
            release_error: None,
            outputs: Vec::new(),
            projected_frame: false,
            presentation_frames: 0,
        });
        self.operation_error = None;
    }

    pub(crate) fn ingest_animation_realization_feedback(
        &mut self,
        replace_owner: bool,
        evicted_fact_count: u64,
        facts: impl IntoIterator<Item = AnimationRealizationFact>,
    ) {
        if replace_owner {
            self.animation_realization_facts.clear();
            self.animation_realization_evicted = evicted_fact_count;
        }
        self.animation_realization_evicted =
            self.animation_realization_evicted.max(evicted_fact_count);
        for fact in facts {
            if self.animation_realization_facts.len() == MAX_ANIMATION_REALIZATION_FACTS {
                self.animation_realization_facts.pop_front();
                self.animation_realization_evicted =
                    self.animation_realization_evicted.saturating_add(1);
            }
            self.animation_realization_facts.push_back(fact);
        }
    }

    fn read_animation_realization(
        &mut self,
    ) -> Result<NativeAnimationRealizationResult, CsharpEngineServicesError> {
        if self.staged.is_none() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CALL",
                "animation service was called outside a product call",
            ));
        }
        let facts = self
            .animation_realization_facts
            .iter()
            .map(animation_realization_receipt)
            .collect::<Box<[_]>>();
        let result = NativeAnimationRealizationResult {
            facts: facts.as_ptr(),
            facts_len: facts.len(),
            evicted_fact_count: self.animation_realization_evicted,
        };
        self.borrowed.hold(facts);
        Ok(result)
    }

    /// Makes the open call fail its end-of-call settlement, as a deferred
    /// resource-release failure does.
    #[cfg(test)]
    pub(crate) fn fail_settlement_for_test(&mut self) {
        self.staged.as_mut().expect("an open call").release_error =
            Some(CsharpEngineServicesError::new(
                "CSHARP_RESOURCE_RELEASE",
                "injected settlement failure",
            ));
    }

    /// Takes the finished call, with the renderer releases for resources it
    /// released. A release failure is kept on the call for the caller.
    pub(crate) fn take_staged_call(&mut self) -> RuntimeAppearanceCall {
        self.operation_error = None;
        let pending = self
            .staged
            .as_mut()
            .is_some_and(|call| std::mem::take(&mut call.resource_releases_pending));
        if pending {
            if let Err(error) = self.publish_resource_releases() {
                self.staged
                    .as_mut()
                    .expect("every product call begins an appearance call")
                    .release_error = Some(error);
            }
        }
        // After every other write of the call, so tweens show over them.
        let staged = self
            .staged
            .as_mut()
            .expect("every product call begins an appearance call");
        if let Err(error) = crate::tween::settle(staged) {
            staged.release_error.get_or_insert(error);
        }
        self.staged
            .take()
            .expect("every product call begins an appearance call")
    }

    pub(crate) fn commit(&mut self, call: RuntimeAppearanceCall) {
        self.state = call.state;
    }

    /// Ends the open call, keeping its state.
    #[cfg(test)]
    pub(crate) fn end_call(&mut self) {
        let call = self.take_staged_call();
        self.commit(call);
    }

    pub(crate) fn seal_resource_selection(&mut self) {
        // Product content has moved to RuntimeContentBridge. This only drops
        // the legacy constructor snapshot after composition is complete.
        self.content_resources.clear();
    }

    /// Reconstructs retained non-graphics presentation state for a fresh
    /// realization. Ordinary appearance `RenderFrameDiff` state belongs to
    /// the canonical graphics world and is intentionally not regenerated
    /// here, preserving its stable renderer identities.
    ///
    /// The result contains only retained creates. Direct particle emissions
    /// and animation realization events are historical signals and are
    /// deliberately excluded.
    pub(crate) fn snapshot_presentation(
        &self,
    ) -> Result<Vec<PresentationFrameDiff>, CsharpEngineServicesError> {
        Self::snapshot_presentation_state(&self.state)
    }

    fn snapshot_presentation_state(
        state: &RuntimeAppearanceState,
    ) -> Result<Vec<PresentationFrameDiff>, CsharpEngineServicesError> {
        #[cfg(test)]
        GRAPHICS_SNAPSHOT_READS.with(|count| count.set(count.get() + 1));
        let mut ops = Vec::new();
        for (handle, descriptor) in state.billboard_projector.active_billboards() {
            ops.push(PresentationOp::Billboard {
                meta: PresentationOpMeta::new(snapshot_presentation_sequence(ops.len())?),
                op: BillboardProjectionOp::Create {
                    handle,
                    descriptor: descriptor.clone(),
                },
            });
        }
        for (handle, descriptor) in state.particle_projector.active_emitters() {
            ops.push(PresentationOp::Particle {
                meta: PresentationOpMeta::new(snapshot_presentation_sequence(ops.len())?),
                op: ParticleProjectionOp::Create {
                    handle,
                    descriptor: {
                        let mut retained = descriptor.clone();
                        // Continuous emissions resume; creation bursts are historical.
                        retained.burst_count = 0;
                        retained
                    },
                },
            });
        }
        for (handle, descriptor) in state.ghost_plate_projector.active_plates() {
            ops.push(PresentationOp::GhostPlate {
                meta: PresentationOpMeta::new(snapshot_presentation_sequence(ops.len())?),
                op: GhostPlateProjectionOp::Create {
                    handle,
                    descriptor: descriptor.clone(),
                },
            });
        }
        for controller in state.animation_controllers.values() {
            for (handle, descriptor) in controller.projector.active_projections() {
                ops.push(PresentationOp::Animation {
                    meta: PresentationOpMeta::new(snapshot_presentation_sequence(ops.len())?),
                    op: render_presentation::AnimationProjectionOp::Create {
                        handle,
                        descriptor: descriptor.clone(),
                    },
                });
            }
        }
        if ops.is_empty() {
            return Ok(Vec::new());
        }
        PresentationFrameDiff::try_from_ops(ops)
            .map(|frame| vec![frame])
            .map_err(|error| {
                CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_BASELINE",
                    format!("retained presentation baseline is invalid: {error:?}"),
                )
            })
    }

    pub(crate) fn presentation_create_billboard(
        &mut self,
        request: &NativePresentationBillboardDescriptor,
    ) -> Result<NativePresentationBillboardHandle, CsharpEngineServicesError> {
        self.presentation_create_billboard_descriptor(
            request.logical_id,
            self.presentation_billboard_descriptor(request)?,
            resource_set([request.texture.value, request.font_asset.value]),
        )
    }

    pub(crate) fn presentation_create_structured_billboard(
        &mut self,
        request: &NativePresentationStructuredBillboardDescriptor,
    ) -> Result<NativePresentationBillboardHandle, CsharpEngineServicesError> {
        let status_cues = unsafe {
            borrowed_slice(
                request.status_cues,
                request.status_cues_len,
                "structured billboard status cues",
            )?
        };
        let mut resources = resource_set([request.icon.value, request.font_asset.value]);
        resources.extend(
            status_cues
                .iter()
                .filter(|cue| cue.has_icon)
                .map(|cue| cue.icon.value),
        );
        self.presentation_create_billboard_descriptor(
            request.logical_id,
            self.presentation_structured_billboard_descriptor(request)?,
            resources,
        )
    }

    fn presentation_create_billboard_descriptor(
        &mut self,
        logical_id: u64,
        descriptor: BillboardDescriptor,
        resources: BTreeSet<u64>,
    ) -> Result<NativePresentationBillboardHandle, CsharpEngineServicesError> {
        if logical_id == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_BILLBOARD",
                "billboard logical id must be nonzero",
            ));
        }
        let handle = BillboardHandle::new(logical_id);
        self.stage_billboard(BillboardProjectionOp::Create { handle, descriptor })?;
        self.staged_mut()?
            .state
            .billboards
            .insert(handle.raw(), handle);
        self.staged_mut()?
            .state
            .billboard_resources
            .insert(logical_id, resources);
        Ok(NativePresentationBillboardHandle { value: logical_id })
    }

    pub(crate) fn presentation_update_billboard(
        &mut self,
        owner: NativePresentationBillboardHandle,
        request: &NativePresentationBillboardDescriptor,
    ) -> Result<(), CsharpEngineServicesError> {
        self.presentation_update_billboard_descriptor(
            owner,
            request.logical_id,
            self.presentation_billboard_descriptor(request)?,
            resource_set([request.texture.value, request.font_asset.value]),
        )
    }

    pub(crate) fn presentation_update_structured_billboard(
        &mut self,
        owner: NativePresentationBillboardHandle,
        request: &NativePresentationStructuredBillboardDescriptor,
    ) -> Result<(), CsharpEngineServicesError> {
        let status_cues = unsafe {
            borrowed_slice(
                request.status_cues,
                request.status_cues_len,
                "structured billboard status cues",
            )?
        };
        let mut resources = resource_set([request.icon.value, request.font_asset.value]);
        resources.extend(
            status_cues
                .iter()
                .filter(|cue| cue.has_icon)
                .map(|cue| cue.icon.value),
        );
        self.presentation_update_billboard_descriptor(
            owner,
            request.logical_id,
            self.presentation_structured_billboard_descriptor(request)?,
            resources,
        )
    }

    fn presentation_update_billboard_descriptor(
        &mut self,
        owner: NativePresentationBillboardHandle,
        logical_id: u64,
        descriptor: BillboardDescriptor,
        resources: BTreeSet<u64>,
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
        if logical_id != owner.value {
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
        self.stage_billboard(BillboardProjectionOp::Update { handle, patch })?;
        self.staged_mut()?
            .state
            .billboard_resources
            .insert(owner.value, resources);
        Ok(())
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
        self.staged_mut()?
            .state
            .billboard_resources
            .remove(&owner.value);
        Ok(())
    }

    pub(crate) fn presentation_emit_particles(
        &mut self,
        signal_id: NativeUtf8Slice,
        request: &NativePresentationParticleDescriptor,
    ) -> Result<NativePresentationParticleEmissionReceipt, CsharpEngineServicesError> {
        let signal_id =
            unsafe { borrowed_utf8(signal_id.bytes, signal_id.len, "particle signal id")? }
                .to_owned();
        let descriptor = self.presentation_particle_descriptor(request)?;
        let result = {
            let staged = self.staged_mut()?;
            let assets = presentation_assets(&staged.state.render_resources);
            staged.state.particle_projector.project_optional_emit(
                &assets,
                PresentationOpMeta::new(0),
                signal_id,
                descriptor,
            )
        };
        match result {
            Ok((admission, projected)) => {
                if let Some(projected) = projected {
                    let mut frame = PresentationFrameDiff::new();
                    frame.ops.push(projected);
                    push_presentation_frame(self.staged_mut()?, frame);
                }
                let outcome = match admission.outcome {
                    ParticleEmissionAdmissionOutcome::Admitted => {
                        NativePresentationParticleEmissionOutcome::Admitted
                    }
                    ParticleEmissionAdmissionOutcome::Dropped => {
                        self.report_particle_recoverable(
                            "CSHARP_PARTICLE_EMISSION_DROPPED",
                            "optional particle emission was dropped because the retained particle budget is exhausted",
                        );
                        NativePresentationParticleEmissionOutcome::Dropped
                    }
                    ParticleEmissionAdmissionOutcome::Clamped => {
                        self.report_particle_recoverable(
                            "CSHARP_PARTICLE_EMISSION_CLAMPED",
                            "optional particle emission was clamped to the remaining retained particle budget",
                        );
                        NativePresentationParticleEmissionOutcome::Clamped
                    }
                };
                Ok(NativePresentationParticleEmissionReceipt {
                    outcome,
                    requested_particles: admission.requested_particles,
                    admitted_particles: admission.admitted_particles,
                    reserved_particles: admission.reserved_particles,
                    max_reserved_particles: admission.max_reserved_particles,
                })
            }
            Err(diagnostic) => {
                self.record_presentation_diagnostic(
                    NativePresentationDiagnosticDomain::Particle,
                    native_particle_diagnostic_code(diagnostic.code),
                    diagnostic.sequence,
                    0,
                );
                Err(CsharpEngineServicesError::new(
                    "CSHARP_PRESENTATION_PARTICLE",
                    diagnostic.message,
                ))
            }
        }
    }

    fn report_particle_recoverable(&mut self, code: &'static str, message: &'static str) {
        if let Some(sink) = self.diagnostics_sink.as_ref() {
            crate::diagnostics::publish_recoverable_once(
                sink,
                &mut self.reported_recoverable_codes,
                "presentation",
                code,
                message,
            );
        }
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
        self.staged_mut()?.state.emitter_resources.insert(
            handle.raw(),
            matches!(request.visual, NativePresentationParticleVisual::Billboard)
                .then_some(request.sprite.value)
                .into_iter()
                .collect(),
        );
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
            size_mode: Some(descriptor.size_mode),
            blend: Some(descriptor.blend),
            softness_metres: Some(descriptor.softness_metres),
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
        self.stage_particle(ParticleProjectionOp::Update { handle, patch })?;
        self.staged_mut()?.state.emitter_resources.insert(
            owner.value,
            matches!(request.visual, NativePresentationParticleVisual::Billboard)
                .then_some(request.sprite.value)
                .into_iter()
                .collect(),
        );
        Ok(())
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
        self.staged_mut()?
            .state
            .emitter_resources
            .remove(&owner.value);
        Ok(())
    }

    pub(crate) fn presentation_readout(&mut self) -> NativePresentationFactsResult {
        let state = self
            .staged
            .as_ref()
            .map(|call| &call.state)
            .unwrap_or(&self.state);
        let billboards = state.billboard_projector.readout();
        let particles = state.particle_projector.readout();
        let diagnostics = |domain| {
            self.presentation_diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.domain == domain)
                .map(|diagnostic| diagnostic.receipt)
                .collect::<Box<[_]>>()
        };
        let billboard_diagnostics = diagnostics(NativePresentationDiagnosticDomain::Billboard);
        let particle_diagnostics = diagnostics(NativePresentationDiagnosticDomain::Particle);
        let result = NativePresentationFactsResult {
            billboard_diagnostics: billboard_diagnostics.as_ptr(),
            billboard_diagnostics_len: billboard_diagnostics.len(),
            particle_diagnostics: particle_diagnostics.as_ptr(),
            particle_diagnostics_len: particle_diagnostics.len(),
            active_billboards: billboards.active_billboards,
            active_emitters: particles.active_emitters,
            reserved_particles: particles.reserved_particles,
            emitted_bursts: particles.emitted_bursts,
        };
        self.borrowed
            .hold((billboard_diagnostics, particle_diagnostics));
        result
    }

    /// Replaces the renderer-owned latest ghost snapshot for the active
    /// runtime binding. This is deliberately not a command or event queue.
    pub(crate) fn ingest_ghost_plate_realization(
        &mut self,
        replace_owner: bool,
        facts: impl IntoIterator<Item = GhostPlateRealizationFact>,
    ) {
        // Every admitted report is a complete latest snapshot. `replace_owner`
        // remains the explicit binding-reset marker at the host boundary, but
        // an ordinary empty renderer snapshot must also clear disposed plates.
        let _ = replace_owner;
        self.ghost_plate_realization.clear();
        for fact in facts {
            self.ghost_plate_realization.insert(fact.handle, fact);
        }
    }

    pub(crate) fn presentation_create_ghost_plate(
        &mut self,
        request: NativeCreateGhostPlatePresentationRequest,
    ) -> Result<NativeGhostPlatePresentationHandle, CsharpEngineServicesError> {
        let presentation = RuntimeGhostPlatePresentation {
            source_object_id: request.source_object_id,
            placement: native_ghost_plate_placement(request.placement),
            capture: native_ghost_plate_capture(request.capture),
            config: native_ghost_plate_config(request.config),
        };
        let handle = {
            let staged = self.staged_mut()?;
            let value = staged.state.next_ghost_plate;
            staged.state.next_ghost_plate = value.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_GHOST_PLATE_HANDLE",
                    "ghost plate handle space overflowed",
                )
            })?;
            value
        };
        self.stage_ghost_plate(
            handle,
            presentation.source_object_id,
            GhostPlateProjectionOp::Create {
                handle: GhostPlateHandle::new(handle),
                descriptor: ghost_plate_descriptor(
                    &presentation,
                    self.ghost_plate_source(&presentation)?,
                ),
            },
        )?;
        self.staged_mut()?
            .state
            .ghost_plates
            .insert(handle, presentation);
        Ok(NativeGhostPlatePresentationHandle { value: handle })
    }

    pub(crate) fn presentation_update_ghost_plate(
        &mut self,
        request: NativeUpdateGhostPlatePresentationRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = request.presentation.value;
        let mut next = self.ghost_plate(handle)?;
        next.placement = native_ghost_plate_placement(request.placement);
        next.config = native_ghost_plate_config(request.config);
        // The renderer applies this patch as one retained replacement,
        // recapturing when the sector count changes.
        self.stage_ghost_plate(
            handle,
            next.source_object_id,
            GhostPlateProjectionOp::Update {
                handle: GhostPlateHandle::new(handle),
                patch: GhostPlatePatch {
                    placement: Some(next.placement.clone()),
                    config: Some(next.config.clone()),
                },
            },
        )?;
        self.staged_mut()?.state.ghost_plates.insert(handle, next);
        Ok(())
    }

    pub(crate) fn presentation_recapture_ghost_plate(
        &mut self,
        request: NativeRecaptureGhostPlatePresentationRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = request.presentation.value;
        let mut next = self.ghost_plate(handle)?;
        next.capture = native_ghost_plate_capture(request.capture);
        self.stage_ghost_plate(
            handle,
            next.source_object_id,
            GhostPlateProjectionOp::Recapture {
                handle: GhostPlateHandle::new(handle),
                capture: Some(next.capture.clone()),
                captured_scene: None,
            },
        )?;
        self.staged_mut()?.state.ghost_plates.insert(handle, next);
        Ok(())
    }

    pub(crate) fn presentation_read_ghost_plate(
        &self,
        presentation: NativeGhostPlatePresentationHandle,
    ) -> Result<NativeGhostPlatePresentationReadout, CsharpEngineServicesError> {
        let staged = self.staged_ref()?;
        let ghost = staged
            .state
            .ghost_plates
            .get(&presentation.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_GHOST_PLATE_HANDLE",
                    "ghost plate presentation is not live",
                )
            })?;
        let source_present = staged
            .state
            .projector
            .object_handle(ghost.source_object_id)
            .is_some();
        let observed = self.ghost_plate_realization.get(&presentation.value);
        Ok(NativeGhostPlatePresentationReadout {
            source_object_id: ghost.source_object_id,
            source_present,
            has_renderer_observation: observed.is_some(),
            source_matches: observed.is_some_and(|fact| fact.source_matches),
            current_sector: observed.map_or(0, |fact| fact.current_sector),
            has_local_angular_offset: observed
                .and_then(|fact| fact.local_angular_offset_degrees)
                .is_some(),
            local_angular_offset_degrees: observed
                .and_then(|fact| fact.local_angular_offset_degrees)
                .unwrap_or_default(),
            fallback_active: observed.is_some_and(|fact| fact.fallback_active),
            fallback_reason: observed.map_or(NativeGhostPlateFallbackReason::None, |fact| {
                fact.fallback_reason
            }),
            limitation_mask: observed.map_or(NativeGhostPlateLimitationMask::None, |fact| {
                fact.limitation_mask
            }),
            has_preparation_cpu_milliseconds: observed
                .and_then(|fact| fact.preparation_cpu_milliseconds)
                .is_some(),
            preparation_cpu_milliseconds: observed
                .and_then(|fact| fact.preparation_cpu_milliseconds)
                .unwrap_or_default(),
            has_capture_cpu_submission_milliseconds: observed
                .and_then(|fact| fact.capture_cpu_submission_milliseconds)
                .is_some(),
            capture_cpu_submission_milliseconds: observed
                .and_then(|fact| fact.capture_cpu_submission_milliseconds)
                .unwrap_or_default(),
            retained_sector_count: observed.map_or(u32::from(ghost.config.sector_count), |fact| {
                fact.retained_sector_count
            }),
            retained_mesh_count: observed.map_or(0, |fact| fact.retained_mesh_count),
            retained_material_count: observed.map_or(0, |fact| fact.retained_material_count),
            retained_borrowed_texture_count: observed
                .map_or(0, |fact| fact.retained_borrowed_texture_count),
            capture: native_ghost_plate_capture_readout(&ghost.capture),
            config: native_ghost_plate_config_readout(&ghost.config),
        })
    }

    pub(crate) fn presentation_destroy_ghost_plate(
        &mut self,
        presentation: NativeGhostPlatePresentationHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = presentation.value;
        self.ghost_plate(handle)?;
        self.stage_ghost_plate(
            handle,
            self.ghost_plate(handle)?.source_object_id,
            GhostPlateProjectionOp::Destroy {
                handle: GhostPlateHandle::new(handle),
            },
        )?;
        self.staged_mut()?.state.ghost_plates.remove(&handle);
        self.ghost_plate_realization.remove(&handle);
        Ok(())
    }

    pub(crate) fn record_operation_error(&mut self, error: CsharpEngineServicesError) {
        self.operation_error = Some(error);
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
                receipt: NativePresentationDiagnostic {
                    code,
                    sequence,
                    logical_id,
                },
            });
    }

    /// Resolves one live C# material into an Engine-owned descriptor for a
    /// separate retained presentation family. The caller copies the returned
    /// value; it never retains this Appearance handle or any product pointer.
    /// A live material for a voxel presentation, with the textures and
    /// product shader its definition names.
    pub(crate) fn voxel_material_projection(
        &mut self,
        material: NativeMaterialHandle,
    ) -> Result<
        (
            RenderMaterialDescriptor,
            Vec<TextureDescriptor>,
            Option<render_model::ShaderDescriptor>,
        ),
        CsharpEngineServicesError,
    > {
        let staged = self.staged_mut()?;
        let state = &*staged.state;
        let id = state.materials.get(&material.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_MATERIAL",
                "voxel-object material handle is not live",
            )
        })?;
        let resources = state.projector.resources();
        let material = resources
            .materials
            .iter()
            .find(|candidate| candidate.id == *id)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_PRESENTATION_MATERIAL",
                    "voxel-object material descriptor is not retained",
                )
            })?;
        let textures = material
            .textures()
            .map(|identity| {
                resources
                    .textures
                    .iter()
                    .find(|texture| texture.id == *identity)
                    .cloned()
                    .ok_or_else(|| {
                        CsharpEngineServicesError::new(
                            "CSHARP_VOXEL_PRESENTATION_TEXTURE",
                            "voxel material texture descriptor is not retained",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let shader = material
            .shader
            .as_ref()
            .map(|used| {
                resources
                    .shaders
                    .iter()
                    .find(|shader| shader.id == used.shader)
                    .cloned()
                    .ok_or_else(|| {
                        CsharpEngineServicesError::new(
                            "CSHARP_VOXEL_PRESENTATION_SHADER",
                            "voxel material shader is not retained",
                        )
                    })
            })
            .transpose()?;
        Ok((material, textures, shader))
    }

    /// The static mesh a static-mesh appearance draws, and its material
    /// slots, for a voxel scatter (#9546).
    pub(crate) fn scatter_mesh(
        &mut self,
        appearance: NativeAppearanceHandle,
    ) -> Result<(String, BTreeSet<u16>), CsharpEngineServicesError> {
        let refusal = |message: &str| {
            CsharpEngineServicesError::new("CSHARP_VOXEL_SCATTER_APPEARANCE", message.to_owned())
        };
        let staged = self.staged_mut()?;
        let state = &*staged.state;
        let identity = state
            .appearances
            .get(&appearance.value)
            .ok_or_else(|| refusal("scatter appearance handle is not live"))?;
        let Some(render_projection::Appearance::StaticMesh { asset, .. }) =
            state.projector.appearance(identity)
        else {
            return Err(refusal("a scatter grows a static mesh appearance"));
        };
        let mesh = state
            .projector
            .resources()
            .static_meshes
            .iter()
            .find(|mesh| mesh.asset == *asset)
            .ok_or_else(|| refusal("scatter appearance mesh is not retained"))?;
        let slots = mesh
            .material_slots
            .iter()
            .map(|slot| slot.slot)
            .chain(mesh.payload.groups.iter().map(|group| group.material_slot))
            .collect();
        Ok((asset.clone(), slots))
    }

    /// Count a scatter growing `appearance` (or one fewer): an appearance
    /// a scatter grows is not disposed under it.
    pub(crate) fn hold_for_scatter(
        &mut self,
        appearance: u64,
        held: bool,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if held {
            // The mesh is defined with the catalog's pending changes, which
            // publish with this call even when no snapshot does.
            staged.resource_releases_pending = true;
        }
        let holds = &mut staged.state.scatter_appearances;
        if held {
            *holds.entry(appearance).or_default() += 1;
        } else if let Some(count) = holds.get_mut(&appearance) {
            *count -= 1;
            if *count == 0 {
                holds.remove(&appearance);
            }
        }
        Ok(())
    }

    pub(crate) fn voxel_material_descriptor(
        &mut self,
        material: NativeMaterialHandle,
    ) -> Result<RenderMaterialDescriptor, CsharpEngineServicesError> {
        self.voxel_material_projection(material)
            .map(|(material, _, _)| material)
    }

    pub(crate) fn staged_mut(
        &mut self,
    ) -> Result<&mut RuntimeAppearanceCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_CALL",
                "appearance service was called outside a product call",
            )
        })
    }

    pub(crate) fn staged_ref(&self) -> Result<&RuntimeAppearanceCall, CsharpEngineServicesError> {
        self.staged.as_ref().ok_or_else(|| {
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
        let (content, layout) = match request.content_kind {
            NativeBillboardContentKind::Text => (
                BillboardContent::Text {
                    localization_key: key,
                    fallback_text: fallback,
                    arguments: Vec::new(),
                },
                None,
            ),
            NativeBillboardContentKind::Value => (
                BillboardContent::Value {
                    label_key: key,
                    fallback_label: fallback,
                    value,
                    unit_key,
                    fallback_unit,
                },
                None,
            ),
            NativeBillboardContentKind::Icon => (
                BillboardContent::Icon {
                    texture: self.presentation_texture_ref(request.texture)?,
                    alt_key: key,
                    fallback_alt: fallback,
                },
                None,
            ),
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
            layout,
        })
    }

    fn presentation_structured_billboard_descriptor(
        &self,
        request: &NativePresentationStructuredBillboardDescriptor,
    ) -> Result<BillboardDescriptor, CsharpEngineServicesError> {
        Ok(BillboardDescriptor {
            anchor: native_presentation_billboard_anchor(request.anchor),
            content: BillboardContent::Structured {
                indicator: self.presentation_structured_indicator(request)?,
            },
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
            layout: Some(native_presentation_billboard_layout(request.layout)),
        })
    }

    fn presentation_structured_indicator(
        &self,
        request: &NativePresentationStructuredBillboardDescriptor,
    ) -> Result<BillboardIndicator, CsharpEngineServicesError> {
        let label = request
            .has_label
            .then(|| {
                native_presentation_localized_text(
                    request.label_key,
                    request.label_fallback_text,
                    "structured billboard label",
                )
            })
            .transpose()?;
        let icon = request
            .has_icon
            .then(|| self.presentation_texture_ref(request.icon))
            .transpose()?;
        let meters = unsafe {
            borrowed_slice(
                request.meters,
                request.meters_len,
                "structured billboard meters",
            )?
        }
        .iter()
        .map(|meter| self.presentation_billboard_meter(*meter))
        .collect::<Result<Vec<_>, _>>()?;
        let status_cues = unsafe {
            borrowed_slice(
                request.status_cues,
                request.status_cues_len,
                "structured billboard status cues",
            )?
        }
        .iter()
        .map(|cue| self.presentation_billboard_status_cue(*cue))
        .collect::<Result<Vec<_>, _>>()?;
        Ok(BillboardIndicator {
            label,
            icon,
            accessible_label: native_presentation_localized_text(
                request.accessible_label_key,
                request.accessible_fallback_text,
                "structured billboard accessible label",
            )?,
            meters,
            status_cues,
            width_pixels: request.width_pixels,
            spacing_pixels: request.spacing_pixels,
            alignment: match request.alignment {
                NativePresentationBillboardAlignment::Start => BillboardAlignment::Start,
                NativePresentationBillboardAlignment::Center => BillboardAlignment::Center,
                NativePresentationBillboardAlignment::End => BillboardAlignment::End,
            },
            style: BillboardStyle {
                opacity: request.style.opacity,
                backing: native_color(request.style.backing),
                border: native_color(request.style.border),
                radius_pixels: request.style.radius_pixels,
            },
        })
    }

    fn presentation_billboard_meter(
        &self,
        meter: NativePresentationBillboardMeter,
    ) -> Result<BillboardMeter, CsharpEngineServicesError> {
        Ok(BillboardMeter {
            id: native_presentation_text(meter.id, "structured billboard meter id")?,
            accessible_label: native_presentation_localized_text(
                meter.accessible_label_key,
                meter.accessible_fallback_text,
                "structured billboard meter label",
            )?,
            current: meter.current,
            min: meter.minimum,
            max: meter.maximum,
            preview: meter.has_preview.then_some(meter.preview),
            fill_direction: match meter.fill_direction {
                NativePresentationBillboardMeterFillDirection::LeftToRight => {
                    BillboardMeterFillDirection::LeftToRight
                }
                NativePresentationBillboardMeterFillDirection::RightToLeft => {
                    BillboardMeterFillDirection::RightToLeft
                }
                NativePresentationBillboardMeterFillDirection::BottomToTop => {
                    BillboardMeterFillDirection::BottomToTop
                }
                NativePresentationBillboardMeterFillDirection::TopToBottom => {
                    BillboardMeterFillDirection::TopToBottom
                }
            },
            segments: meter.segments,
            fill: native_color(meter.fill),
            preview_fill: native_color(meter.preview_fill),
            back: native_color(meter.back),
            border: native_color(meter.border),
        })
    }

    fn presentation_billboard_status_cue(
        &self,
        cue: NativePresentationBillboardStatusCue,
    ) -> Result<BillboardStatusCue, CsharpEngineServicesError> {
        Ok(BillboardStatusCue {
            id: native_presentation_text(cue.id, "structured billboard status cue id")?,
            label: native_presentation_localized_text(
                cue.label_key,
                cue.label_fallback_text,
                "structured billboard status cue label",
            )?,
            icon: cue
                .has_icon
                .then(|| self.presentation_texture_ref(cue.icon))
                .transpose()?,
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
        let collision = request
            .has_collision
            .then(|| {
                let volumes = unsafe {
                    borrowed_slice(
                        request.collision_volumes,
                        request.collision_volumes_len,
                        "particle collision volumes",
                    )?
                }
                .iter()
                .map(|volume| match volume.kind {
                    NativePresentationParticleCollisionVolumeKind::Plane => {
                        ParticleCollisionVolume::Plane {
                            normal: native_vec3_array(volume.normal),
                            offset: volume.offset,
                        }
                    }
                    NativePresentationParticleCollisionVolumeKind::Aabb => {
                        ParticleCollisionVolume::Aabb {
                            minimum: native_vec3_array(volume.minimum),
                            maximum: native_vec3_array(volume.maximum),
                        }
                    }
                })
                .collect();
                Ok(ParticleCollisionDescriptor {
                    radius: request.collision.radius,
                    restitution: request.collision.restitution,
                    friction: request.collision.friction,
                    maximum_impacts: request.collision.maximum_impacts,
                    sleep_speed: request.collision.sleep_speed,
                    limit_behavior: match request.collision.limit_behavior {
                        NativePresentationParticleCollisionLimitBehavior::Sleep => {
                            ParticleCollisionLimitBehavior::Sleep
                        }
                        NativePresentationParticleCollisionLimitBehavior::Kill => {
                            ParticleCollisionLimitBehavior::Kill
                        }
                    },
                    volumes,
                })
            })
            .transpose()?;
        Ok(ParticleEmitterDescriptor {
            anchor: native_presentation_particle_anchor(request.anchor),
            visual,
            size_mode: match request.size_mode {
                NativePresentationParticleSizeMode::Screen => ParticleSizeMode::Screen,
                NativePresentationParticleSizeMode::World => ParticleSizeMode::World,
            },
            blend: match request.blend {
                NativePresentationParticleBlendMode::Alpha => ParticleBlendMode::Alpha,
                NativePresentationParticleBlendMode::Additive => ParticleBlendMode::Additive,
            },
            softness_metres: request.softness_metres,
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
            collision,
        })
    }

    fn presentation_texture_ref(
        &self,
        resource: NativeRenderResourceReference,
    ) -> Result<BillboardTextureRef, CsharpEngineServicesError> {
        let resource = self.resource(resource.value)?;
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PRESENTATION_TEXTURE",
                "billboard and particle sprites require an admitted texture resource",
            ));
        }
        Ok(BillboardTextureRef {
            asset: resource.asset_identity().to_owned(),
            content_hash: resource.content_hash().to_owned(),
        })
    }

    fn presentation_font_ref(
        &self,
        kind: NativePresentationFontKind,
        asset: NativeRenderResourceReference,
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
                push_presentation_frame(self.staged_mut()?, frame);
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
                push_presentation_frame(self.staged_mut()?, frame);
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

    fn ghost_plate(
        &self,
        handle: u64,
    ) -> Result<RuntimeGhostPlatePresentation, CsharpEngineServicesError> {
        self.staged_ref()?
            .state
            .ghost_plates
            .get(&handle)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_GHOST_PLATE_HANDLE",
                    "ghost plate presentation is not live",
                )
            })
    }

    fn ghost_plate_source(
        &self,
        presentation: &RuntimeGhostPlatePresentation,
    ) -> Result<RenderHandle, CsharpEngineServicesError> {
        self.staged_ref()?
            .state
            .projector
            .object_handle(presentation.source_object_id)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_GHOST_PLATE_SOURCE",
                    "ghost plate source object must be present in the current Appearance snapshot",
                )
            })
    }

    fn stage_ghost_plate(
        &mut self,
        handle: u64,
        source_object_id: u64,
        op: GhostPlateProjectionOp,
    ) -> Result<(), CsharpEngineServicesError> {
        let result = {
            let staged = self.staged_mut()?;
            let source = staged
                .state
                .projector
                .object_handle(source_object_id)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_GHOST_PLATE_SOURCE",
                        "ghost plate source object must be present in the current Appearance snapshot",
                    )
                })?;
            let targets = BTreeSet::from([source]);
            staged
                .state
                .ghost_plate_projector
                .project(&targets, PresentationOpMeta::new(0), op)
        };
        match result {
            Ok(projected) => {
                let mut frame = PresentationFrameDiff::new();
                frame.ops.push(projected);
                push_presentation_frame(self.staged_mut()?, frame);
                Ok(())
            }
            Err(diagnostic) => Err(CsharpEngineServicesError::new(
                "CSHARP_GHOST_PLATE",
                format!("ghost plate {handle} was rejected: {}", diagnostic.message),
            )),
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
        let content = self.content_path(&requested_path)?;
        // SAFETY: the borrowed keywords are copied before the call returns.
        let keywords = unsafe { shader_keywords(request.shader_keywords)? };
        self.admit_resource(
            content,
            request.filter,
            request.wrap,
            request.color_space,
            &keywords,
        )
    }

    pub(crate) fn admit_resource(
        &mut self,
        content: crate::content::RetainedContent,
        filter: NativeTextureFilter,
        wrap: NativeTextureWrap,
        color_space: NativeTextureColorSpace,
        shader_keywords: &[String],
    ) -> Result<NativeRenderResourceInfo, CsharpEngineServicesError> {
        let content_admitted = self.content.is_some();
        let resource = self.imports.file(
            content,
            filter,
            wrap,
            color_space,
            shader_keywords,
            content_admitted,
        )?;
        let resources = &mut self.staged_mut()?.state.render_resources;
        let handle = resources.admit(resource)?;
        resources.info(handle)
    }

    fn destroy_resource(
        &mut self,
        resource: NativeRenderResourceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = resource.value;
        let sky_uses_resource = self
            .camera_view
            .is_some_and(|camera_view| unsafe { (&*camera_view).uses_sky_texture(handle) });
        let staged = self.staged_mut()?;
        if !staged.state.render_resources.release_owner(handle)? {
            return Ok(());
        }
        if let Some(owner) = resource_live_owner(&staged.state, handle) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_IN_USE",
                format!("dispose the live {owner} before releasing this renderer resource"),
            ));
        }
        if sky_uses_resource {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_IN_USE",
                "clear the active sky background before releasing its texture resource",
            ));
        }
        remove_resource(&mut staged.state, handle)?;
        staged.resource_releases_pending = true;
        Ok(())
    }

    fn resource(&self, handle: u64) -> Result<&CsharpRenderResource, CsharpEngineServicesError> {
        let state = self
            .staged
            .as_ref()
            .map(|staged| &staged.state)
            .unwrap_or(&self.state);
        state.render_resources.resource(handle)
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

    fn set_appearance_resources(
        &mut self,
        appearance: u64,
        resources: impl IntoIterator<Item = u64>,
    ) -> Result<(), CsharpEngineServicesError> {
        let resources = resources
            .into_iter()
            .filter(|handle| *handle != 0)
            .collect::<BTreeSet<_>>();
        let staged = self.staged_mut()?;
        for resource in &resources {
            if staged.state.render_resources.get(*resource).is_none() {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_HANDLE",
                    "appearance selected an unavailable resource",
                ));
            }
        }
        staged
            .state
            .appearance_resources
            .insert(appearance, resources);
        Ok(())
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
        let next_light = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_LIGHT_HANDLE", "light handle overflow")
        })?;
        set_light(staged, None, handle, fact)?;
        staged.state.next_light = next_light;
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
        set_light(
            staged,
            Some(request.light.value),
            request.light.value,
            replacement,
        )
    }

    fn replace_light(
        &mut self,
        request: NativeLightUpdateRequest,
    ) -> Result<NativeLightHandle, CsharpEngineServicesError> {
        let replacement = runtime_light_fact(request.replacement)?;
        let staged = self.staged_mut()?;
        if staged.state.lights.iter().any(|(handle, candidate)| {
            *handle != request.light.value && candidate.light_id == replacement.light_id
        }) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_LIGHT_LOGICAL_ID",
                "logical light id is already owned by a live light",
            ));
        }
        let handle = staged.state.next_light;
        let next_light = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_LIGHT_HANDLE", "light handle overflow")
        })?;
        let previous = staged
            .state
            .lights
            .contains_key(&request.light.value)
            .then_some(request.light.value);
        set_light(staged, previous, handle, replacement)?;
        staged.state.next_light = next_light;
        Ok(NativeLightHandle { value: handle })
    }

    fn destroy_light(&mut self, light: NativeLightHandle) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if let Some(fact) = staged.state.lights.get(&light.value) {
            let id = fact.light_id;
            if !staged.state.pending_lights.contains(&light.value) {
                project_light_change(staged, &[], &[id])?;
            }
            staged.state.lights.remove(&light.value);
            staged.state.pending_lights.remove(&light.value);
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
        let next_material = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_HANDLE", "material handle overflow")
        })?;
        let id = runtime_material_id(handle);
        let descriptor = material_descriptor(id.clone(), request, &staged.state.render_resources)?;
        let textures =
            texture_descriptors_for_material(&descriptor, &staged.state.render_resources)?;
        let shader = material_shader(&staged.state.render_resources, request.shader)?;
        staged.state.next_material = next_material;
        let resources = staged.state.projector.resources_mut();
        for texture in textures {
            retain_texture_descriptor(&mut resources.textures, texture)?;
        }
        if let Some((_, shader)) = shader {
            retain_shader_descriptor(&mut resources.shaders, shader);
        }
        resources.materials.push(descriptor);
        staged.state.materials.insert(handle, id);
        staged.state.material_resources.insert(
            handle,
            resource_set([
                request.texture.value,
                request.normal_map.value,
                request.shader.shader.value,
                request.shader.texture_a.value,
                request.shader.texture_b.value,
                request.water.foam_texture.value,
                request.water.ripple_texture.value,
            ]),
        );
        Ok(NativeMaterialHandle { value: handle })
    }

    fn create_authored_material(
        &mut self,
        request: NativeAuthoredMaterialAppearanceRequest,
        material_id: &str,
    ) -> Result<NativeMaterialHandle, CsharpEngineServicesError> {
        let authored = unsafe {
            self.authored_content
                .and_then(|authored_content| authored_content.as_ref())
        }
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUTHORED_MATERIAL_COMPOSITION",
                "authored content is not composed with Appearance",
            )
        })?;
        let mut material = authored.project_renderer_material(request.catalog, material_id)?;
        let staged = self.staged_mut()?;
        let handle = staged.state.next_material;
        staged.state.next_material = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_HANDLE", "material handle overflow")
        })?;
        material.id = format!("material/csharp-authored-{handle}");

        let texture = match material.texture.as_deref() {
            None => {
                if request.texture.value != 0 {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_AUTHORED_MATERIAL_TEXTURE",
                        "untextured authored material cannot select a texture resource",
                    ));
                }
                None
            }
            Some(_) => {
                if request.texture.value == 0 {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_AUTHORED_MATERIAL_TEXTURE",
                        "textured authored material requires a selected texture resource",
                    ));
                }
                let resource = staged
                    .state
                    .render_resources
                    .get(request.texture.value)
                    .ok_or_else(|| {
                        CsharpEngineServicesError::new(
                            "CSHARP_AUTHORED_MATERIAL_TEXTURE",
                            "authored material texture handle is not live",
                        )
                    })?;
                if resource.kind() != CsharpRenderResourceKind::Texture {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_AUTHORED_MATERIAL_TEXTURE",
                        "authored material texture must be an admitted texture resource",
                    ));
                }
                if let Some(expected_hash) = authored_voxel_texture_hash(&material) {
                    if resource.content_hash() != expected_hash {
                        return Err(CsharpEngineServicesError::new(
                            "CSHARP_AUTHORED_MATERIAL_TEXTURE_HASH",
                            "selected texture bytes do not match the resolved authored texture hash",
                        ));
                    }
                }
                let mut texture = resource.texture().cloned().ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_AUTHORED_MATERIAL_TEXTURE",
                        "selected texture descriptor is not retained",
                    )
                })?;
                texture.id = format!("texture/csharp-authored-{handle}");
                if let Some(surface) = material.voxel_surface.as_mut() {
                    texture.filter = surface.filter;
                    texture.wrap = surface.wrap;
                    texture.version = authored_voxel_texture_version(surface);
                    retarget_voxel_surface(surface, &texture.id);
                }
                texture.validate().map_err(|error| {
                    CsharpEngineServicesError::new(
                        "CSHARP_AUTHORED_MATERIAL_TEXTURE",
                        format!("projected authored texture is invalid: {error:?}"),
                    )
                })?;
                material.texture = Some(texture.id.clone());
                Some(texture)
            }
        };
        material.normal_map = normal_map_descriptor(
            &staged.state.render_resources,
            request.normal_map,
            request.normal_scale,
        )?;
        material.triplanar = triplanar_descriptor(request.triplanar_sharpness);
        material.water = water_descriptor(&staged.state.render_resources, request.water)?;
        let shader = material_shader(&staged.state.render_resources, request.shader)?;
        material.shader = shader.as_ref().map(|(used, _)| used.clone());
        // The shader's own textures, beside the selected and normal ones.
        let shader_textures = texture_descriptors_for_material(
            &RenderMaterialDescriptor {
                texture: None,
                normal_map: None,
                ..material.clone()
            },
            &staged.state.render_resources,
        )?;
        let normal_texture = match &material.normal_map {
            Some(map) => Some(
                texture_descriptors_for_material(
                    &RenderMaterialDescriptor {
                        texture: Some(map.texture.clone()),
                        normal_map: None,
                        triplanar: None,
                        ..material.clone()
                    },
                    &staged.state.render_resources,
                )?
                .remove(0),
            ),
            None => None,
        };
        material.validate().map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_AUTHORED_MATERIAL",
                format!("projected authored material is invalid: {error:?}"),
            )
        })?;
        let resources = staged.state.projector.resources_mut();
        for texture in texture
            .into_iter()
            .chain(normal_texture)
            .chain(shader_textures)
        {
            retain_texture_descriptor(&mut resources.textures, texture)?;
        }
        if let Some((_, shader)) = shader {
            retain_shader_descriptor(&mut resources.shaders, shader);
        }
        resources.materials.push(material.clone());
        staged.state.materials.insert(handle, material.id);
        staged.state.material_resources.insert(
            handle,
            resource_set([
                request.texture.value,
                request.normal_map.value,
                request.shader.shader.value,
                request.shader.texture_a.value,
                request.shader.texture_b.value,
                request.water.foam_texture.value,
                request.water.ripple_texture.value,
            ]),
        );
        Ok(NativeMaterialHandle { value: handle })
    }

    fn create_terrain_layer_material(
        &mut self,
        request: NativeTerrainLayerMaterialRequest,
    ) -> Result<NativeMaterialHandle, CsharpEngineServicesError> {
        let invalid = |message: &str| {
            CsharpEngineServicesError::new("CSHARP_TERRAIN_LAYER_MATERIAL", message.to_owned())
        };
        let staged = self.staged_mut()?;
        let state = &*staged.state;
        let surface_material = |handle: NativeMaterialHandle| {
            let id = state
                .materials
                .get(&handle.value)
                .ok_or_else(|| invalid("a layer material handle is not live"))?;
            let descriptor = state
                .projector
                .resources()
                .materials
                .iter()
                .find(|candidate| candidate.id == *id)
                .ok_or_else(|| invalid("a layer material descriptor is not retained"))?;
            if descriptor.voxel_surface.is_none() || descriptor.terrain_layers.is_some() {
                return Err(invalid(
                    "every layer must be a voxel surface material without layers of its own",
                ));
            }
            Ok(descriptor.clone())
        };
        let mut material = surface_material(request.base)?;
        let handles = unsafe {
            crate::composition::borrowed_slice(
                request.layers,
                request.layers_len,
                "terrain layer materials",
            )
        }?;
        if !(1..=render_model::MAX_MATERIAL_TERRAIN_LAYERS).contains(&handles.len()) {
            return Err(invalid("name 1 to 15 layer materials"));
        }
        let layers = handles
            .iter()
            .map(|handle| {
                surface_material(*handle).map(|layer| {
                    render_model::MaterialTerrainLayerDescriptor {
                        voxel_surface: layer.voxel_surface.expect("checked above"),
                        normal_map: layer.normal_map,
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // The layers' textures stay live while this material holds them.
        let resources = std::iter::once(request.base)
            .chain(handles.iter().copied())
            .flat_map(|handle| {
                state
                    .material_resources
                    .get(&handle.value)
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect();
        let handle = state.next_material;
        let next_material = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_HANDLE", "material handle overflow")
        })?;
        material.id = runtime_material_id(handle);
        material.terrain_layers = Some(render_model::MaterialTerrainLayersDescriptor {
            layers,
            contrast: request.contrast,
        });
        material.validate().map_err(|error| {
            invalid(&format!(
                "the terrain layer material is invalid ({error:?}): the contrast must be finite and 1 or more"
            ))
        })?;
        staged.state.next_material = next_material;
        staged
            .state
            .projector
            .resources_mut()
            .materials
            .push(material.clone());
        staged.state.materials.insert(handle, material.id);
        staged.state.material_resources.insert(handle, resources);
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
        let textures =
            texture_descriptors_for_material(&descriptor, &staged.state.render_resources)?;
        let shader = material_shader(&staged.state.render_resources, request.replacement.shader)?;
        let resources = staged.state.projector.resources_mut();
        for texture in textures {
            retain_texture_descriptor(&mut resources.textures, texture)?;
        }
        if let Some((_, shader)) = shader {
            retain_shader_descriptor(&mut resources.shaders, shader);
        }
        let material = resources
            .materials
            .iter_mut()
            .find(|material| material.id == id)
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_MATERIAL", "material catalog drifted")
            })?;
        *material = descriptor;
        staged.state.material_resources.insert(
            request.material.value,
            resource_set([
                request.replacement.texture.value,
                request.replacement.normal_map.value,
                request.replacement.shader.shader.value,
                request.replacement.shader.texture_a.value,
                request.replacement.shader.texture_b.value,
            ]),
        );
        Ok(())
    }

    fn replace_material(
        &mut self,
        request: NativeMaterialUpdateRequest,
    ) -> Result<NativeMaterialHandle, CsharpEngineServicesError> {
        // Refuse an invalid replacement before the prior material is released,
        // validating it under the identity create_material will assign.
        let state = &self.staged_ref()?.state;
        let descriptor = material_descriptor(
            runtime_material_id(state.next_material),
            request.replacement,
            &state.render_resources,
        )?;
        texture_descriptors_for_material(&descriptor, &state.render_resources)?;
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
            || staged.state.generated_meshes.uses_material(material.value)
        {
            staged.state.materials.insert(material.value, id);
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_IN_USE",
                "dispose appearances and mesh resources using this material before disposing the material",
            ));
        }
        let resources = staged.state.projector.resources_mut();
        resources.materials.retain(|candidate| candidate.id != id);
        staged.state.material_resources.remove(&material.value);
        staged.resource_releases_pending = true;
        Ok(())
    }

    fn destroy_appearance(
        &mut self,
        appearance: NativeAppearanceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged
            .state
            .appearances
            .get(&appearance.value)
            .is_some_and(|identity| staged.state.projector.appearance_in_use(identity))
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
        if staged
            .state
            .sprite_playbacks_by_appearance
            .get(&appearance.value)
            .is_some_and(|playbacks| !playbacks.is_empty())
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_APPEARANCE_IN_USE",
                "dispose sprite playbacks using this appearance before disposing or replacing it",
            ));
        }
        if staged
            .state
            .scatter_appearances
            .contains_key(&appearance.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SCATTER_APPEARANCE_IN_USE",
                "remove the voxel scene scatters growing this appearance before disposing or replacing it",
            ));
        }
        let Some(identity) = staged.state.appearances.remove(&appearance.value) else {
            // Match the other generated retained owners: replacement-first
            // then owner disposal is safe and has no renderer side channel.
            return Ok(());
        };
        staged.state.appearance_resources.remove(&appearance.value);
        staged.state.appearance_materials.remove(&appearance.value);
        staged.state.mesh_appearances.remove(&appearance.value);
        staged.state.animated_appearances.remove(&appearance.value);
        staged
            .state
            .sprite_playbacks_by_appearance
            .remove(&appearance.value);
        if let Some(atlas) = staged
            .state
            .sprite_appearance_atlases
            .remove(&appearance.value)
        {
            if let Some(appearances) = staged.state.sprite_atlas_appearances.get_mut(&atlas) {
                appearances.remove(&appearance.value);
            }
        }
        staged.state.projector.remove_appearance(&identity);
        // Inline/content static meshes and legacy sprites synthesize renderer
        // catalog entries per appearance. Once the owner is gone those entries
        // must not become invisible resource retainers.
        let suffix = appearance.value.to_string();
        let mesh_asset = format!("mesh/native-{suffix}");
        let resources = staged.state.projector.resources_mut();
        resources
            .static_meshes
            .retain(|mesh| mesh.asset != mesh_asset);
        resources.materials.retain(|material| {
            !material
                .id
                .starts_with(&format!("material/native-{suffix}-"))
        });
        resources
            .textures
            .retain(|texture| texture.id != format!("texture/native-{suffix}"));
        resources
            .sprite_atlases
            .retain(|atlas| atlas.id != format!("sprite/native-{suffix}"));
        release_unowned_internal_resources(&mut staged.state);
        staged.resource_releases_pending = true;
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
                ));
            }
        }
        staged
            .state
            .appearance_materials
            .insert(request.appearance.value, material_handles);
        Ok(())
    }

    /// Replaces the per-slot factors of a static mesh appearance: base
    /// colour, texture tint and emission over each slot's material, so
    /// instances of one mesh tint apart with one material.
    unsafe fn update_static_mesh_material_factors(
        &mut self,
        request: &NativeStaticMeshMaterialFactorsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let factors = borrowed_slice(
            request.factors,
            request.factors_len,
            "static mesh material factors",
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
        let mut parameters = BTreeMap::new();
        for factor in factors {
            let slot = u16::try_from(factor.material_slot).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_SLOT",
                    "mesh material slot exceeded u16",
                )
            })?;
            let value = mesh_material_parameters(factor, "CSHARP_STATIC_MESH_FACTORS")?;
            if parameters.insert(slot, value).is_some() {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_STATIC_MESH_SLOT",
                    "static mesh material factors must not repeat a slot",
                ));
            }
        }
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::StaticMesh {
                material_parameters: current,
                ..
            }) => {
                *current = parameters;
                Ok(())
            }
            _ => Err(CsharpEngineServicesError::new(
                "CSHARP_STATIC_MESH_APPEARANCE",
                "material factors require a live static mesh appearance",
            )),
        }
    }

    fn set_mesh_inspection(
        &mut self,
        request: &NativeAnimatedMeshInspectionRequest,
    ) -> Result<(), CsharpEngineServicesError> {
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
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::AnimatedMesh { inspection, .. }) => {
                *inspection = render_model::AnimatedMeshInspection {
                    wireframe: request.wireframe,
                    matte: request.matte,
                    whole_voxel_normals: request.whole_voxel_normals,
                    bounds_request: request.bounds_request,
                };
                Ok(())
            }
            _ => Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATED_MESH_APPEARANCE",
                "inspection requires an animated mesh appearance",
            )),
        }
    }

    unsafe fn update_animated_mesh_materials(
        &mut self,
        request: &NativeAnimatedMeshMaterialUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let bindings = borrowed_slice(
            request.bindings,
            request.bindings_len,
            "animated mesh material bindings",
        )?;
        let staged = self.staged_mut()?;
        let (identity, embedded_slots) =
            animated_appearance_slots(staged, request.appearance, "material bindings")?;
        let mut slots = BTreeSet::new();
        let mut material_overrides = Vec::with_capacity(bindings.len());
        let mut material_handles = BTreeSet::new();
        for binding in bindings {
            let slot = u16::try_from(binding.material_slot).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATED_MESH_SLOT",
                    "animated mesh material slot exceeded u16",
                )
            })?;
            if !slots.insert(slot) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATED_MESH_SLOT",
                    "animated mesh material bindings must not repeat a slot",
                ));
            }
            if !embedded_slots.contains(&slot) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATED_MESH_SLOT",
                    "animated mesh material binding names an unbound embedded slot",
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
            Some(Appearance::AnimatedMesh {
                material_overrides: current,
                ..
            }) => {
                *current = material_overrides;
            }
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATED_MESH_APPEARANCE",
                    "material bindings require a live animated mesh appearance",
                ));
            }
        }
        staged
            .state
            .appearance_materials
            .insert(request.appearance.value, material_handles);
        Ok(())
    }

    /// Replaces the factor overrides of an animated appearance's embedded
    /// material slots.
    unsafe fn update_animated_mesh_material_factors(
        &mut self,
        request: &NativeAnimatedMeshMaterialFactorsRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let factors = borrowed_slice(
            request.factors,
            request.factors_len,
            "animated mesh material factors",
        )?;
        let staged = self.staged_mut()?;
        let (identity, embedded_slots) =
            animated_appearance_slots(staged, request.appearance, "material factors")?;
        let mut parameters = BTreeMap::new();
        for factor in factors {
            let slot = u16::try_from(factor.material_slot)
                .ok()
                .filter(|slot| embedded_slots.contains(slot))
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATED_MESH_SLOT",
                        "animated mesh material factors name an unbound embedded slot",
                    )
                })?;
            let value = mesh_material_parameters(factor, "CSHARP_ANIMATED_MESH_FACTORS")?;
            if parameters.insert(slot, value).is_some() {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_ANIMATED_MESH_SLOT",
                    "animated mesh material factors must not repeat a slot",
                ));
            }
        }
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::AnimatedMesh {
                material_parameters: current,
                ..
            }) => {
                *current = parameters;
                Ok(())
            }
            _ => Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATED_MESH_APPEARANCE",
                "material factors require a live animated mesh appearance",
            )),
        }
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
            NativePrimitiveGeometry::Group => Geometry::Group,
            NativePrimitiveGeometry::Line => Geometry::Line {
                a: [0.0, 0.0, 0.0],
                b: [0.0, 1.0, 0.0],
            },
        };
        self.allocate_appearance(Appearance::Primitive {
            geometry,
            material: Material {
                color: native_color(request.color),
                wireframe: request.wireframe,
            },
        })
    }

    fn create_mesh_appearance(
        &mut self,
        resource: NativeMeshResourceHandle,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let asset = self
            .staged_ref()?
            .state
            .generated_meshes
            .asset(resource.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_MESH_HANDLE", "mesh resource is not live")
            })?
            .to_owned();
        let appearance = self.allocate_appearance(Appearance::StaticMesh {
            asset,
            material_overrides: Vec::new(),
            material_parameters: Default::default(),
        })?;
        self.staged_mut()?
            .state
            .mesh_appearances
            .insert(appearance.value, resource.value);
        Ok(appearance)
    }

    fn destroy_mesh_resource(
        &mut self,
        resource: NativeMeshResourceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(asset) = staged.state.generated_meshes.asset(resource.value) else {
            return Ok(());
        };
        if staged
            .state
            .mesh_appearances
            .values()
            .any(|handle| *handle == resource.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MESH_IN_USE",
                "dispose appearances using this mesh before disposing its resource",
            ));
        }
        let asset = asset.to_owned();
        staged
            .state
            .projector
            .resources_mut()
            .static_meshes
            .retain(|mesh| mesh.asset != asset);
        staged.state.generated_meshes.remove(resource.value);
        // The release is published with the call's other resource changes.
        staged.resource_releases_pending = true;
        Ok(())
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
                ));
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
                texture_space: None,
                distance_field: None,
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
                layer_weights: false,
                layer_palette: Vec::new(),
                vertex_occlusion: false,
            },
            material_slots: material_slots.clone(),
            collision: MeshCollisionPolicy::VisualOnly,
        };
        // Every later projection revalidates catalog meshes, so refuse here rather
        // than retaining an entry that would fail each subsequent snapshot.
        asset.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_STATIC_MESH_LAYOUT", format!("{error:?}"))
        })?;
        {
            let resources = self.staged_mut()?.state.projector.resources_mut();
            resources.materials.extend(materials);
            resources.static_meshes.push(Arc::new(asset));
        }
        let appearance = self.allocate_appearance(Appearance::StaticMesh {
            asset: mesh_id,
            material_overrides: Vec::new(),
            material_parameters: Default::default(),
        })?;
        self.set_appearance_resources(appearance.value, [request.resource.value])?;
        Ok(appearance)
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
        let content = self.content_path(&requested_path)?;
        self.admit_static_mesh(content, request.color)
    }

    fn admit_static_mesh(
        &mut self,
        content: crate::content::RetainedContent,
        color: NativeColor,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let (payload, slots, resource) = self.imports.static_mesh(content)?;
        let browser_path = resource.path().to_owned();
        let resource = self
            .staged_mut()?
            .state
            .render_resources
            .stage(resource, [browser_path])?;
        let appearance = self.create_retained_static_mesh(payload, slots, color)?;
        self.set_appearance_resources(appearance.value, [resource])?;
        Ok(appearance)
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
            resources.static_meshes.push(Arc::new(StaticMeshAsset {
                asset: mesh_id.clone(),
                payload,
                material_slots,
                collision: MeshCollisionPolicy::VisualOnly,
            }));
        }
        self.allocate_appearance(Appearance::StaticMesh {
            asset: mesh_id,
            material_overrides: Vec::new(),
            material_parameters: Default::default(),
        })
    }

    unsafe fn create_sprite_atlas(
        &mut self,
        request: &NativeSpriteAtlasCreateRequest,
    ) -> Result<NativeSpriteAtlasHandle, CsharpEngineServicesError> {
        let frames = borrowed_slice(request.frames, request.frames_len, "sprite atlas frames")?;
        if frames.is_empty() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_FRAMES",
                "sprite atlas must contain at least one frame",
            ));
        }
        if request.texture.value == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_RENDER_RESOURCE_HANDLE",
                "invalid resource handle",
            ));
        }
        let resource = self.resource(request.texture.value)?.clone();
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_RESOURCE",
                "sprite atlas requires a texture resource",
            ));
        }
        let handle = self.staged_mut()?.state.next_sprite_atlas;
        let texture_asset = format!("texture/atlas-{handle}");
        let asset = format!("sprite/atlas-{handle}");
        let texture = sprite_texture_descriptor(&resource, texture_asset.clone())?;
        let atlas = SpriteAtlasDescriptor {
            id: asset.clone(),
            texture: texture_asset.clone(),
            frames: frames
                .iter()
                .map(|frame| SpriteFrameRect {
                    frame: frame.frame_id,
                    uv_min: native_vec2(frame.uv_min),
                    uv_max: native_vec2(frame.uv_max),
                    size: frame.has_size.then(|| native_vec2(frame.size)),
                })
                .collect(),
        };
        atlas.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_ATLAS_FRAME", format!("{error:?}"))
        })?;
        let copied_frames = atlas
            .frames
            .iter()
            .cloned()
            .map(|frame| (frame.frame, frame))
            .collect();
        let staged = self.staged_mut()?;
        staged.state.next_sprite_atlas = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_HANDLE",
                "sprite atlas handle overflow",
            )
        })?;
        staged
            .state
            .projector
            .resources_mut()
            .textures
            .push(texture);
        staged
            .state
            .projector
            .resources_mut()
            .sprite_atlases
            .push(atlas);
        staged.state.sprite_atlases.insert(
            handle,
            RuntimeSpriteAtlas {
                asset,
                texture_asset,
                frames: copied_frames,
            },
        );
        staged
            .state
            .sprite_atlas_appearances
            .insert(handle, BTreeSet::new());
        staged
            .state
            .sprite_atlas_resources
            .insert(handle, BTreeSet::from([request.texture.value]));
        Ok(NativeSpriteAtlasHandle { value: handle })
    }

    fn destroy_sprite_atlas(
        &mut self,
        atlas: NativeSpriteAtlasHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let entry = staged
            .state
            .sprite_atlases
            .get(&atlas.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_HANDLE",
                    "sprite atlas is not live",
                )
            })?;
        if staged
            .state
            .sprite_atlas_appearances
            .get(&atlas.value)
            .is_some_and(|appearances| !appearances.is_empty())
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_IN_USE",
                "dispose or replace appearances using this sprite atlas before disposing it",
            ));
        }
        if staged
            .state
            .sprite_playbacks_by_atlas
            .get(&atlas.value)
            .is_some_and(|playbacks| !playbacks.is_empty())
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_ATLAS_IN_USE",
                "dispose sprite playbacks using this atlas before disposing it",
            ));
        }
        staged.state.sprite_atlases.remove(&atlas.value);
        staged.state.sprite_atlas_resources.remove(&atlas.value);
        staged.state.sprite_atlas_appearances.remove(&atlas.value);
        staged.state.sprite_playbacks_by_atlas.remove(&atlas.value);
        let resources = staged.state.projector.resources_mut();
        resources
            .sprite_atlases
            .retain(|candidate| candidate.id != entry.asset);
        resources
            .textures
            .retain(|candidate| candidate.id != entry.texture_asset);
        staged.resource_releases_pending = true;
        Ok(())
    }

    fn sprite_atlas(
        &self,
        atlas: NativeSpriteAtlasHandle,
    ) -> Result<RuntimeSpriteAtlas, CsharpEngineServicesError> {
        let state = self
            .staged
            .as_ref()
            .map(|call| &call.state)
            .unwrap_or(&self.state);
        state
            .sprite_atlases
            .get(&atlas.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_HANDLE",
                    "sprite atlas is not live",
                )
            })
    }

    fn sprite_material_descriptor(
        &self,
        value: NativeSpriteMaterialDescriptor,
    ) -> Result<SpriteMaterialDescriptor, CsharpEngineServicesError> {
        let resolve_texture = |handle: NativeRenderResourceReference,
                               label: &str|
         -> Result<Option<String>, CsharpEngineServicesError> {
            if handle.value == 0 {
                return Ok(None);
            }
            let resource = self.resource(handle.value)?;
            if resource.kind() != CsharpRenderResourceKind::Texture {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_MATERIAL_TEXTURE",
                    format!("sprite {label} texture must be an admitted texture resource"),
                ));
            }
            Ok(Some(resource.asset_identity().to_owned()))
        };
        let descriptor = SpriteMaterialDescriptor {
            lighting: match value.lighting {
                NativeSpriteLightingMode::Unlit => SpriteLightingMode::Unlit,
                NativeSpriteLightingMode::AuthoredNormal => SpriteLightingMode::AuthoredNormal,
                NativeSpriteLightingMode::AuthoredDepth => SpriteLightingMode::AuthoredDepth,
                NativeSpriteLightingMode::DerivedGradient => SpriteLightingMode::DerivedGradient,
                NativeSpriteLightingMode::Synthetic => SpriteLightingMode::Synthetic,
            },
            normal_texture: resolve_texture(value.normal_texture, "normal")?,
            depth_texture: resolve_texture(value.depth_texture, "depth")?,
            normal_strength: value.normal_strength,
            normal_bias: value.normal_bias,
            alpha: match value.alpha_mode {
                NativeSpriteAlphaMode::Opaque => SpriteAlphaMode::Opaque,
                NativeSpriteAlphaMode::Mask => SpriteAlphaMode::Mask {
                    cutoff: value.alpha_cutoff,
                },
                NativeSpriteAlphaMode::Blend => SpriteAlphaMode::Blend,
            },
            shadow: match value.shadow {
                NativeSpriteShadowPolicy::None => SpriteShadowPolicy::None,
                NativeSpriteShadowPolicy::Cast => SpriteShadowPolicy::Cast,
                NativeSpriteShadowPolicy::Receive => SpriteShadowPolicy::Receive,
                NativeSpriteShadowPolicy::CastAndReceive => SpriteShadowPolicy::CastAndReceive,
            },
            blend: match value.blend {
                NativeSpriteBlendMode::Alpha => SpriteBlendMode::Alpha,
                NativeSpriteBlendMode::Additive => SpriteBlendMode::Additive,
            },
            softness_metres: value.softness_metres,
        };
        descriptor.validate().map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_MATERIAL",
                format!("sprite material is invalid: {error:?}"),
            )
        })?;
        Ok(descriptor)
    }

    fn retain_sprite_material_textures(
        &mut self,
        material: NativeSpriteMaterialDescriptor,
    ) -> Result<(), CsharpEngineServicesError> {
        for handle in [material.normal_texture, material.depth_texture] {
            if handle.value == 0 {
                continue;
            }
            // Descriptor validation has already established the resource kind.
            // Resource admission alone does not publish a texture definition.
            let texture = self
                .resource(handle.value)?
                .texture()
                .cloned()
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_SPRITE_MATERIAL_TEXTURE",
                        "sprite material texture is unavailable",
                    )
                })?;
            retain_texture_descriptor(
                &mut self.staged_mut()?.state.projector.resources_mut().textures,
                texture,
            )?;
        }
        Ok(())
    }

    fn sprite_from_atlas(
        &self,
        request: NativeSpriteFromAtlasRequest,
    ) -> Result<(RuntimeSpriteAtlas, SpriteInstanceDescriptor), CsharpEngineServicesError> {
        let atlas = self.sprite_atlas(request.atlas)?;
        if !atlas.frames.contains_key(&request.frame_id) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_FRAME",
                "sprite frame is not defined by the atlas",
            ));
        }
        let sprite = sprite_instance_descriptor(
            atlas.asset.clone(),
            request.frame_id,
            request.pivot,
            request.size,
            request.billboard,
            request.size_mode,
            request.render_order,
            request.depth,
            request.tint,
            self.sprite_material_descriptor(request.material)?,
        );
        sprite.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_ATLAS_FRAME", format!("{error:?}"))
        })?;
        Ok((atlas.clone(), sprite))
    }

    fn create_sprite_from_atlas(
        &mut self,
        request: NativeSpriteFromAtlasRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let (_, sprite) = self.sprite_from_atlas(request)?;
        self.retain_sprite_material_textures(request.material)?;
        let appearance = self.allocate_appearance(Appearance::Sprite { sprite })?;
        self.set_appearance_resources(
            appearance.value,
            [
                request.material.normal_texture.value,
                request.material.depth_texture.value,
            ],
        )?;
        let staged = self.staged_mut()?;
        staged
            .state
            .sprite_atlas_appearances
            .entry(request.atlas.value)
            .or_default()
            .insert(appearance.value);
        staged
            .state
            .sprite_appearance_atlases
            .insert(appearance.value, request.atlas.value);
        Ok(appearance)
    }

    fn replace_sprite_from_atlas(
        &mut self,
        request: NativeSpriteFromAtlasReplaceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        // Resolve the new retained atlas and frame before releasing the prior
        // owner, so stale/wrong-kind/frame failures cannot disturb it.
        self.sprite_from_atlas(request.replacement)?;
        self.ensure_live_appearance(request.appearance)?;
        self.destroy_appearance(request.appearance)?;
        self.create_sprite_from_atlas(request.replacement)
    }

    fn set_sprite_frame(
        &mut self,
        request: NativeSpriteFrameUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let atlas_handle = *staged
            .state
            .sprite_appearance_atlases
            .get(&request.appearance.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_APPEARANCE",
                    "appearance is not an atlas-backed sprite",
                )
            })?;
        let atlas = staged
            .state
            .sprite_atlases
            .get(&atlas_handle)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_HANDLE",
                    "sprite atlas is not live",
                )
            })?;
        if !atlas.frames.contains_key(&request.frame_id) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_FRAME",
                "sprite frame is not defined by the atlas",
            ));
        }
        let identity = staged
            .state
            .appearances
            .get(&request.appearance.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_APPEARANCE_HANDLE", "appearance is not live")
            })?;
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::Sprite { sprite }) => sprite.frame = request.frame_id,
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_APPEARANCE",
                    "appearance is not a sprite",
                ));
            }
        }
        Ok(())
    }

    fn set_sprite_viewport(
        &mut self,
        request: NativeSpriteViewportUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let placement = if request.enabled {
            Some(render_model::SpriteViewportPlacement {
                minimum: native_vec2(request.minimum),
                size: native_vec2(request.size),
                alignment: native_vec2(request.alignment),
                fit: match request.fit {
                    NativeSpriteViewportFit::Stretch => render_model::SpriteViewportFit::Stretch,
                    NativeSpriteViewportFit::Contain => render_model::SpriteViewportFit::Contain,
                },
            })
        } else {
            None
        };
        if let Some(placement) = placement {
            placement.validate().map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_SPRITE_VIEWPORT", format!("{error:?}"))
            })?;
        }
        let staged = self.staged_mut()?;
        let identity = staged
            .state
            .appearances
            .get(&request.appearance.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_APPEARANCE_HANDLE", "appearance is not live")
            })?;
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::Sprite { sprite }) => sprite.viewport_placement = placement,
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_VIEWPORT_APPEARANCE",
                    "appearance is not a sprite",
                ));
            }
        }
        Ok(())
    }

    fn read_sprite(
        &mut self,
        appearance: NativeAppearanceHandle,
    ) -> Result<NativeSpriteReadout, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &*staged.state;
        let atlas_handle = *state
            .sprite_appearance_atlases
            .get(&appearance.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_APPEARANCE",
                    "appearance is not an atlas-backed sprite",
                )
            })?;
        let atlas = state.sprite_atlases.get(&atlas_handle).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_ATLAS_HANDLE", "sprite atlas is not live")
        })?;
        let identity = state.appearances.get(&appearance.value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_APPEARANCE_HANDLE", "appearance is not live")
        })?;
        let frame_id = match state.projector.appearance(identity) {
            Some(Appearance::Sprite { sprite }) => sprite.frame,
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_APPEARANCE",
                    "appearance is not a sprite",
                ));
            }
        };
        let frame = atlas.frames.get(&frame_id).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_ATLAS_FRAME",
                "sprite frame is not defined by the atlas",
            )
        })?;
        Ok(NativeSpriteReadout {
            atlas: NativeSpriteAtlasReference {
                value: atlas_handle,
            },
            frame_id,
            uv_min: NativeVec2 {
                x: frame.uv_min[0],
                y: frame.uv_min[1],
            },
            uv_max: NativeVec2 {
                x: frame.uv_max[0],
                y: frame.uv_max[1],
            },
            has_size: frame.size.is_some(),
            size: frame
                .size
                .map(|size| NativeVec2 {
                    x: size[0],
                    y: size[1],
                })
                .unwrap_or_default(),
        })
    }

    unsafe fn create_sprite_playback(
        &mut self,
        request: &NativeSpritePlaybackCreateRequest,
    ) -> Result<NativeSpritePlaybackHandle, CsharpEngineServicesError> {
        let frames = borrowed_slice(request.frames, request.frames_len, "sprite playback frames")?;
        let markers = borrowed_slice(
            request.markers,
            request.markers_len,
            "sprite playback markers",
        )?;
        if frames.is_empty() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_FRAMES",
                "sprite playback must contain at least one sequence entry",
            ));
        }
        if !request.playback_rate.is_finite() || request.playback_rate <= 0.0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_RATE",
                "sprite playback rate must be finite and positive",
            ));
        }
        let state = &self.staged_ref()?.state;
        let atlas = state
            .sprite_atlases
            .get(&request.atlas.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_HANDLE",
                    "sprite atlas is not live",
                )
            })?;
        let appearance_atlas = state
            .sprite_appearance_atlases
            .get(&request.appearance.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_ATLAS_APPEARANCE",
                    "appearance is not an atlas-backed sprite",
                )
            })?;
        if appearance_atlas != request.atlas.value {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_ATLAS",
                "sprite playback atlas does not own the supplied appearance",
            ));
        }
        let mut total_duration_seconds = 0.0;
        for frame in frames {
            if !frame.duration_seconds.is_finite() || frame.duration_seconds <= 0.0 {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_DURATION",
                    "sprite playback frame duration must be finite and positive",
                ));
            }
            if !atlas.frames.contains_key(&frame.frame_id) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_FRAME",
                    "sprite playback references a frame absent from the admitted atlas",
                ));
            }
            total_duration_seconds += frame.duration_seconds;
            if !total_duration_seconds.is_finite() {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_DURATION",
                    "sprite playback total duration must be finite",
                ));
            }
        }
        let mut marker_ids = BTreeSet::new();
        for marker in markers {
            if marker.marker_id == 0 || !marker_ids.insert(marker.marker_id) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_MARKER",
                    "sprite playback marker IDs must be nonzero and unique",
                ));
            }
            if marker.frame_index as usize >= frames.len() {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_MARKER",
                    "sprite playback marker frame index is outside the sequence",
                ));
            }
        }
        let handle = state.next_sprite_playback;
        let first_frame = frames[0].frame_id;
        self.set_sprite_frame(NativeSpriteFrameUpdateRequest {
            appearance: request.appearance,
            frame_id: first_frame,
        })?;
        let staged = self.staged_mut()?;
        let state = &mut *staged.state;
        state.next_sprite_playback = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_HANDLE",
                "sprite playback handle overflow",
            )
        })?;
        state.sprite_playbacks.insert(
            handle,
            RuntimeSpritePlayback {
                appearance: request.appearance.value,
                atlas: request.atlas.value,
                frames: frames.to_vec(),
                markers: markers.to_vec(),
                loop_mode: request.loop_mode,
                playback_rate: request.playback_rate,
                state: NativeSpritePlaybackState::Stopped,
                frame_index: 0,
                elapsed_in_frame_seconds: 0.0,
                cycle: 0,
                revision: 0,
                next_crossing_sequence: 1,
                last_update: None,
            },
        );
        state
            .sprite_playbacks_by_atlas
            .entry(request.atlas.value)
            .or_default()
            .insert(handle);
        state
            .sprite_playbacks_by_appearance
            .entry(request.appearance.value)
            .or_default()
            .insert(handle);
        Ok(NativeSpritePlaybackHandle { value: handle })
    }

    fn destroy_sprite_playback(
        &mut self,
        playback: NativeSpritePlaybackHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(value) = staged.state.sprite_playbacks.remove(&playback.value) else {
            return Ok(());
        };
        if let Some(values) = staged.state.sprite_playbacks_by_atlas.get_mut(&value.atlas) {
            values.remove(&playback.value);
        }
        if let Some(values) = staged
            .state
            .sprite_playbacks_by_appearance
            .get_mut(&value.appearance)
        {
            values.remove(&playback.value);
        }
        Ok(())
    }

    fn control_sprite_playback(
        &mut self,
        request: NativeSpritePlaybackControlRequest,
    ) -> Result<NativeSpritePlaybackReadout, CsharpEngineServicesError> {
        let mut playback = self.sprite_playback(request.playback)?;
        match request.control {
            NativeSpritePlaybackControl::Start
                if playback.state == NativeSpritePlaybackState::Stopped =>
            {
                playback.state = NativeSpritePlaybackState::Playing;
            }
            NativeSpritePlaybackControl::Pause
                if playback.state == NativeSpritePlaybackState::Playing =>
            {
                playback.state = NativeSpritePlaybackState::Paused;
            }
            NativeSpritePlaybackControl::Resume
                if playback.state == NativeSpritePlaybackState::Paused =>
            {
                playback.state = NativeSpritePlaybackState::Playing;
            }
            NativeSpritePlaybackControl::Stop
                if matches!(
                    playback.state,
                    NativeSpritePlaybackState::Playing
                        | NativeSpritePlaybackState::Paused
                        | NativeSpritePlaybackState::Completed
                ) =>
            {
                reset_sprite_playback(&mut playback, NativeSpritePlaybackState::Stopped);
            }
            NativeSpritePlaybackControl::Restart => {
                reset_sprite_playback(&mut playback, NativeSpritePlaybackState::Playing);
            }
            _ => {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_TRANSITION",
                    "sprite playback control is invalid for the current state",
                ));
            }
        }
        playback.revision = playback.revision.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_REVISION",
                "sprite playback revision overflow",
            )
        })?;
        let frame_id = playback.frames[playback.frame_index].frame_id;
        self.set_sprite_frame(NativeSpriteFrameUpdateRequest {
            appearance: NativeAppearanceHandle {
                value: playback.appearance,
            },
            frame_id,
        })?;
        let readout = sprite_playback_readout(&playback);
        self.staged_mut()?
            .state
            .sprite_playbacks
            .insert(request.playback.value, playback);
        Ok(readout)
    }

    fn select_sprite_playback_frame(
        &mut self,
        request: NativeSpritePlaybackFrameSelectionRequest,
    ) -> Result<NativeSpritePlaybackReadout, CsharpEngineServicesError> {
        let mut playback = self.sprite_playback(request.playback)?;
        let frame_index = usize::try_from(request.frame_index).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_FRAME_INDEX",
                "sprite playback frame index is outside the admitted sequence",
            )
        })?;
        let frame_id = playback
            .frames
            .get(frame_index)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_FRAME_INDEX",
                    "sprite playback frame index is outside the admitted sequence",
                )
            })?
            .frame_id;
        let revision = playback.revision.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_REVISION",
                "sprite playback revision overflow",
            )
        })?;

        // Validate and update the renderer-owned sprite before committing the
        // copied playback cursor. Because both mutations live in the staged
        // appearance call, callback failure still rolls the pair back.
        self.set_sprite_frame(NativeSpriteFrameUpdateRequest {
            appearance: NativeAppearanceHandle {
                value: playback.appearance,
            },
            frame_id,
        })?;
        playback.frame_index = frame_index;
        playback.elapsed_in_frame_seconds = 0.0;
        playback.revision = revision;
        playback.state = sprite_playback_state_after_selection(playback.state);
        let readout = sprite_playback_readout(&playback);
        self.staged_mut()?
            .state
            .sprite_playbacks
            .insert(request.playback.value, playback);
        Ok(readout)
    }

    fn advance_sprite_playback(
        &mut self,
        request: NativeSpritePlaybackAdvanceRequest,
    ) -> Result<NativeSpritePlaybackAdvanceResult, CsharpEngineServicesError> {
        let facts = self.staged_ref()?.admitted_update.ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_UPDATE",
                "sprite playback can advance only during the active Product.Update callback",
            )
        })?;
        let mut playback = self.sprite_playback(request.playback)?;
        let identity = validate_sprite_playback_update(facts, playback.last_update)?;
        let duplicate = playback.last_update == Some(identity);
        let mut crossings = Vec::new();
        let mut advanced = false;
        if !duplicate {
            playback.last_update = Some(identity);
            if playback.state == NativeSpritePlaybackState::Playing && facts.admitted_step_count > 0
            {
                let mut remaining = facts.fixed_delta_seconds
                    * f64::from(facts.admitted_step_count)
                    * playback.playback_rate;
                if !remaining.is_finite() || remaining < 0.0 {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_SPRITE_PLAYBACK_TIME",
                        "admitted sprite playback time is not finite and non-negative",
                    ));
                }
                advanced = remaining > 0.0;
                let mut transitions = 0usize;
                while remaining > 0.0 && playback.state == NativeSpritePlaybackState::Playing {
                    let duration = playback.frames[playback.frame_index].duration_seconds;
                    let until_boundary = duration - playback.elapsed_in_frame_seconds;
                    if remaining < until_boundary {
                        playback.elapsed_in_frame_seconds += remaining;
                        remaining = 0.0;
                        continue;
                    }
                    remaining -= until_boundary;
                    transitions += 1;
                    if transitions > MAX_SPRITE_PLAYBACK_TRANSITIONS_PER_ADVANCE {
                        return Err(CsharpEngineServicesError::new(
                            "CSHARP_SPRITE_PLAYBACK_ADVANCE",
                            "one admitted update crossed too many sprite playback frames",
                        ));
                    }
                    if playback.frame_index + 1 == playback.frames.len() {
                        match playback.loop_mode {
                            NativeSpritePlaybackLoopMode::OneShot => {
                                playback.elapsed_in_frame_seconds = duration;
                                playback.state = NativeSpritePlaybackState::Completed;
                                remaining = 0.0;
                                continue;
                            }
                            NativeSpritePlaybackLoopMode::Loop => {
                                playback.frame_index = 0;
                                playback.cycle =
                                    playback.cycle.checked_add(1).ok_or_else(|| {
                                        CsharpEngineServicesError::new(
                                            "CSHARP_SPRITE_PLAYBACK_CYCLE",
                                            "sprite playback cycle overflow",
                                        )
                                    })?;
                            }
                        }
                    } else {
                        playback.frame_index += 1;
                    }
                    playback.elapsed_in_frame_seconds = 0.0;
                    append_sprite_playback_markers(&mut playback, &mut crossings)?;
                }
                if advanced {
                    playback.revision = playback.revision.checked_add(1).ok_or_else(|| {
                        CsharpEngineServicesError::new(
                            "CSHARP_SPRITE_PLAYBACK_REVISION",
                            "sprite playback revision overflow",
                        )
                    })?;
                }
            }
        }
        let frame_id = playback.frames[playback.frame_index].frame_id;
        if advanced {
            self.set_sprite_frame(NativeSpriteFrameUpdateRequest {
                appearance: NativeAppearanceHandle {
                    value: playback.appearance,
                },
                frame_id,
            })?;
        }
        let readout = sprite_playback_readout(&playback);
        let boxed = crossings.into_boxed_slice();
        let crossings_pointer = if boxed.is_empty() {
            std::ptr::null()
        } else {
            boxed.as_ptr()
        };
        let result = NativeSpritePlaybackAdvanceResult {
            crossings: crossings_pointer,
            crossings_len: boxed.len(),
            readout,
            advanced,
        };
        self.staged_mut()?
            .state
            .sprite_playbacks
            .insert(request.playback.value, playback);
        self.borrowed.hold(boxed);
        Ok(result)
    }

    fn sample_sprite_playback(
        &self,
        request: NativeSpritePlaybackSampleRequest,
    ) -> Result<NativeSpritePlaybackSample, CsharpEngineServicesError> {
        if !request.elapsed_seconds.is_finite() || request.elapsed_seconds < 0.0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_SAMPLE",
                "sprite playback sample time must be finite and non-negative",
            ));
        }
        let playback = self.sprite_playback(request.playback)?;
        sample_sprite_playback_at(&playback, request.elapsed_seconds * playback.playback_rate)
    }

    fn read_sprite_playback(
        &self,
        playback: NativeSpritePlaybackHandle,
    ) -> Result<NativeSpritePlaybackReadout, CsharpEngineServicesError> {
        Ok(sprite_playback_readout(&self.sprite_playback(playback)?))
    }

    fn sprite_playback(
        &self,
        playback: NativeSpritePlaybackHandle,
    ) -> Result<RuntimeSpritePlayback, CsharpEngineServicesError> {
        self.staged_ref()?
            .state
            .sprite_playbacks
            .get(&playback.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_HANDLE",
                    "sprite playback is not live",
                )
            })
    }

    fn ensure_live_appearance(
        &mut self,
        appearance: NativeAppearanceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if self
            .staged_mut()?
            .state
            .appearances
            .contains_key(&appearance.value)
        {
            Ok(())
        } else {
            Err(CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_HANDLE",
                "appearance is not live",
            ))
        }
    }

    fn validate_legacy_sprite_request(
        &self,
        request: NativeSpriteAppearanceRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let resource = self.resource(request.texture.value)?.clone();
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_SPRITE_RESOURCE",
                "sprite appearance requires a texture resource",
            ));
        }
        let texture = sprite_texture_descriptor(&resource, "texture/legacy-validation".to_owned())?;
        let atlas = SpriteAtlasDescriptor {
            id: "sprite/legacy-validation".to_owned(),
            texture: texture.id,
            frames: vec![SpriteFrameRect {
                frame: 0,
                uv_min: native_vec2(request.uv_min),
                uv_max: native_vec2(request.uv_max),
                size: None,
            }],
        };
        atlas.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_FRAME", format!("{error:?}"))
        })?;
        let sprite = sprite_instance_descriptor(
            atlas.id,
            0,
            request.pivot,
            request.size,
            request.billboard,
            request.size_mode,
            request.render_order,
            request.depth,
            request.tint,
            self.sprite_material_descriptor(request.material)?,
        );
        sprite.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_FRAME", format!("{error:?}"))
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
        let texture = sprite_texture_descriptor(&resource, texture_id.clone())?;
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
        let sprite = sprite_instance_descriptor(
            atlas_id,
            0,
            request.pivot,
            request.size,
            request.billboard,
            request.size_mode,
            request.render_order,
            request.depth,
            request.tint,
            self.sprite_material_descriptor(request.material)?,
        );
        sprite.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_FRAME", format!("{error:?}"))
        })?;
        // Catalog atlases are revalidated by every projection; refuse before retaining.
        atlas.validate().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPRITE_FRAME", format!("{error:?}"))
        })?;
        self.retain_sprite_material_textures(request.material)?;
        {
            let resources = self.staged_mut()?.state.projector.resources_mut();
            resources.textures.push(texture);
            resources.sprite_atlases.push(atlas);
        }
        let appearance = self.allocate_appearance(Appearance::Sprite { sprite })?;
        self.set_appearance_resources(
            appearance.value,
            [
                request.texture.value,
                request.material.normal_texture.value,
                request.material.depth_texture.value,
            ],
        )?;
        Ok(appearance)
    }

    fn replace_sprite(
        &mut self,
        request: NativeSpriteAppearanceReplaceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        self.validate_legacy_sprite_request(request.replacement)?;
        self.ensure_live_appearance(request.appearance)?;
        self.destroy_appearance(request.appearance)?;
        self.create_sprite(request.replacement)
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
        let content = self.content_path(&requested_path)?;
        self.admit_animated_mesh(content)
    }

    fn admit_animated_mesh(
        &mut self,
        content: crate::content::RetainedContent,
    ) -> Result<NativeRenderResourceHandle, CsharpEngineServicesError> {
        self.admit_animated_content(content, false)
    }

    fn open_animation_clip_pack(
        &mut self,
        request: &NativeAnimationClipPackResourceRequest,
    ) -> Result<NativeRenderResourceHandle, CsharpEngineServicesError> {
        let requested_path = unsafe {
            borrowed_utf8(
                request.path.bytes,
                request.path.len,
                "animation clip-pack resource path",
            )?
            .to_owned()
        };
        let content = self.content_path(&requested_path)?;
        self.admit_animation_clip_pack(content)
    }

    fn admit_animation_clip_pack(
        &mut self,
        content: crate::content::RetainedContent,
    ) -> Result<NativeRenderResourceHandle, CsharpEngineServicesError> {
        self.admit_animated_content(content, true)
    }

    fn admit_animated_content(
        &mut self,
        content: crate::content::RetainedContent,
        clip_pack: bool,
    ) -> Result<NativeRenderResourceHandle, CsharpEngineServicesError> {
        let resource = self.imports.animated(content, clip_pack)?;
        let handle = self.staged_mut()?.state.render_resources.admit(resource)?;
        Ok(NativeRenderResourceHandle { value: handle })
    }

    fn associate_animation_clip_pack(
        &mut self,
        request: &NativeAnimationClipPackAssociationRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let producer = borrowed_request_utf8(request.producer, "animation clip-pack producer")?;
        let license = borrowed_request_utf8(request.license, "animation clip-pack license")?;

        let primary = self.resource(request.primary_mesh.value)?.clone();
        if primary.kind() != CsharpRenderResourceKind::AnimatedMesh {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_PRIMARY",
                "clip-pack association requires an admitted primary animated mesh",
            ));
        }
        let pack = self.resource(request.clip_pack.value)?.clone();
        if pack.kind() != CsharpRenderResourceKind::AnimationClipPack {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_RESOURCE_KIND",
                "clip-pack association requires an admitted animation clip-pack resource",
            ));
        }
        let primary_mesh = primary.animated_mesh().cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_PRIMARY",
                "primary animated mesh resource did not retain an animated descriptor",
            )
        })?;
        let pack_mesh = pack.animated_mesh().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_RESOURCE_KIND",
                "clip-pack resource did not retain an imported animated descriptor",
            )
        })?;
        let primary_rig = primary_mesh.rig.clone().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_PRIMARY_RIG",
                "primary animated mesh has no importer-derived named skin rig",
            )
        })?;
        let pack_rig = pack_mesh.rig.clone().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_RIG",
                "animation clip-pack has no importer-derived named skin rig",
            )
        })?;
        if !primary_rig.is_clip_compatible_with(&pack_rig) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_RIG",
                "primary animated mesh and clip-pack importer-derived rig signatures differ",
            ));
        }
        let asset = format!(
            "animation-clip-pack/{}",
            pack.content_hash()
                .strip_prefix("sha256:")
                .expect("admitted clip-pack hashes use SHA-256")
        );
        let clip_pack = AnimationClipPack {
            asset,
            runtime_format: pack_mesh.runtime_format,
            content_hash: pack.content_hash().to_owned(),
            rig: pack_rig,
            clips: pack_mesh.clips.clone(),
            provenance: AnimationClipPackProvenance {
                producer,
                source_hash: pack.content_hash().to_owned(),
                target_hash: primary.content_hash().to_owned(),
                license,
            },
        };
        let mut assembled = primary_mesh;
        assembled.clip_packs.push(clip_pack);
        assembled.validate().map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_ASSOCIATION",
                format!("clip-pack association is incompatible with the primary animated mesh: {error:?}"),
            )
        })?;

        let staged = self.staged_mut()?;
        if staged
            .state
            .animated_appearances
            .values()
            .any(|handle| *handle == request.primary_mesh.value)
            || staged
                .state
                .animation_graphs
                .values()
                .any(|graph| graph.resource == request.primary_mesh.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_CLIP_PACK_ASSOCIATION_CLOSED",
                "associate clip packs before creating an animated appearance or graph for the primary mesh",
            ));
        }
        *staged
            .state
            .render_resources
            .animated_mesh_mut(request.primary_mesh.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_RENDER_RESOURCE_HANDLE",
                    "unknown primary animated mesh resource handle",
                )
            })? = assembled;
        staged
            .state
            .animation_clip_pack_resources
            .entry(request.primary_mesh.value)
            .or_default()
            .insert(request.clip_pack.value);
        Ok(())
    }

    fn create_animated_mesh_appearance(
        &mut self,
        request: NativeAnimatedMeshAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let asset = self.animated_mesh_asset(request.resource.value)?;
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
                    .push(Arc::new(asset.clone()));
            }
        }
        let appearance = self.allocate_appearance(Appearance::AnimatedMesh {
            inspection: Default::default(),
            asset: asset.asset,
            material_overrides: Vec::new(),
            playback: None,
            material_parameters: BTreeMap::new(),
        })?;
        self.staged_mut()?
            .state
            .animated_appearances
            .insert(appearance.value, request.resource.value);
        self.set_appearance_resources(appearance.value, [request.resource.value])?;
        Ok(appearance)
    }

    fn replace_animated_mesh_appearance(
        &mut self,
        appearance: NativeAppearanceHandle,
        request: NativeAnimatedMeshAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        // Refuse an unusable replacement before the prior appearance is released.
        self.animated_mesh_asset(request.resource.value)?;
        self.destroy_appearance(appearance)?;
        self.create_animated_mesh_appearance(request)
    }

    fn animated_mesh_asset(
        &self,
        resource: u64,
    ) -> Result<AnimatedMeshAsset, CsharpEngineServicesError> {
        let resource = self.resource(resource)?;
        if resource.kind() != CsharpRenderResourceKind::AnimatedMesh {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE_KIND",
                "animated mesh appearance requires an admitted primary animated GLB resource",
            ));
        }
        resource.animated_mesh().cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE_KIND",
                "animated mesh appearance requires an admitted animated GLB resource",
            )
        })
    }

    fn create_animation_instance(
        &mut self,
        request: NativeAnimationInstanceRequest,
    ) -> Result<NativeAnimationInstanceHandle, CsharpEngineServicesError> {
        let (asset, content_hash) = self.animation_instance_mesh(request, None)?;
        let state = &mut *self.staged_mut()?.state;
        let handle = state.next_animation_instance;
        state.next_animation_instance = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE",
                "animation instance handles exhausted",
            )
        })?;
        state.animation_instances.insert(
            handle,
            AnimationInstance {
                appearance: request.appearance.value,
                object_id: request.object_id,
                asset,
                content_hash,
                direct_playback: None,
                pending_playback: false,
                last_playback_target: None,
                controller: None,
            },
        );
        Ok(NativeAnimationInstanceHandle { value: handle })
    }

    /// Resolves the animated asset for an instance request. `replacing` names
    /// the instance being replaced, which may keep its object identity.
    fn animation_instance_mesh(
        &self,
        request: NativeAnimationInstanceRequest,
        replacing: Option<u64>,
    ) -> Result<(String, String), CsharpEngineServicesError> {
        let state = &self.staged_ref()?.state;
        let resource = state
            .animated_appearances
            .get(&request.appearance.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_APPEARANCE",
                    "animation instances require a live animated-mesh appearance",
                )
            })?;
        if state.animation_instances.iter().any(|(handle, instance)| {
            Some(*handle) != replacing && instance.object_id == request.object_id
        }) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE_OBJECT",
                "a product object may have only one retained animation instance",
            ));
        }
        let mesh = state
            .render_resources
            .get(resource)
            .and_then(CsharpRenderResource::animated_mesh)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_RESOURCE",
                    "animated appearance resource is unavailable",
                )
            })?;
        Ok((
            mesh.asset.clone(),
            mesh.content_hash.clone().unwrap_or_default(),
        ))
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
        // Outputs retain call order. Stop a target that still belongs to this
        // instance; if a prior snapshot already removed/replaced it, its
        // renderer teardown has done that work. Editors can replace more than
        // one selection in a single update without a callback-order gate.
        if let Some(target) = instance.last_playback_target.filter(|target| {
            staged.state.projector.object_handle(instance.object_id) == Some(*target)
                && staged.state.projector.object_appearance(instance.object_id)
                    == staged
                        .state
                        .appearances
                        .get(&instance.appearance)
                        .map(String::as_str)
        }) {
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
            push_extra_frame(staged, frame);
        }
        staged.state.animation_instances.remove(&handle.value);
        Ok(())
    }

    fn replace_animation_instance(
        &mut self,
        prior: NativeAnimationInstanceHandle,
        request: NativeAnimationInstanceRequest,
    ) -> Result<NativeAnimationInstanceHandle, CsharpEngineServicesError> {
        // Refuse an unusable replacement before the prior instance is released.
        self.animation_instance_mesh(request, Some(prior.value))?;
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
        let resource = self.resource(request.resource.value)?;
        if resource.kind() != CsharpRenderResourceKind::AnimatedMesh {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_GRAPH_RESOURCE",
                "animation graph requires an admitted primary animated GLB",
            ));
        }
        let asset_id = resource
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
        staged
            .state
            .animation_graph_resources
            .insert(handle, BTreeSet::from([request.resource.value]));
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
        staged.state.animation_graph_resources.remove(&graph.value);
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
            .get(graph.resource)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_RESOURCE",
                    "animation graph resource is unavailable",
                )
            })?;
        if resource.kind() != CsharpRenderResourceKind::AnimatedMesh {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE",
                "animation graph resource is not a primary animated GLB",
            ));
        }
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
        let mesh_asset = mesh.asset.clone();
        let mesh_content_hash = mesh.content_hash.clone();
        let effective_clips = animation_asset_clips(&staged.state.render_resources, &mesh_asset);
        let assets = BTreeMap::from([(
            mesh_asset.clone(),
            ResolvedRenderAsset {
                id: mesh_asset.clone(),
                kind: RenderAssetKind::AnimatedMesh,
                content_hash: mesh_content_hash.clone(),
                version: 0,
            },
        )]);
        let catalog = validate_animation_catalog(
            AnimationCatalog {
                schema_version: 1,
                catalog_id: format!("csharp/{}", graph.definition.graph_id),
                assets: vec![AnimationClipAsset {
                    asset_id: mesh_asset,
                    content_hash: mesh_content_hash.unwrap_or_default(),
                    clips: effective_clips,
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
                detached: None,
            },
        );
        Ok(NativeAnimationControllerHandle { value: handle })
    }

    fn destroy_animation_controller(
        &mut self,
        handle: NativeAnimationControllerHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if staged.projected_frame {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                "dispose the animation controller before publishing its removal snapshot",
            ));
        }
        let Some(controller) = staged.state.animation_controllers.get(&handle.value) else {
            return Ok(());
        };
        let instance_handle = controller.instance;
        // Build the renderer removal before releasing the controller so a
        // refusal leaves the controller and its instance binding intact.
        let frame = if controller.projected {
            let sequence = staged.presentation_frames;
            let object_id = staged
                .state
                .animation_instances
                .get(&instance_handle)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_INSTANCE",
                        "animation controller instance is not live",
                    )
                })?
                .object_id;
            let mut projector = controller.projector.clone();
            let op = projector
                .destroy_entity(object_id, PresentationOpMeta::new(sequence))
                .map_err(|diagnostic| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_PROJECTION",
                        diagnostic.message,
                    )
                })?;
            let mut frame = PresentationFrameDiff::new();
            frame.ops.push(op);
            Some(frame)
        } else {
            None
        };
        staged.state.animation_controllers.remove(&handle.value);
        if let Some(frame) = frame {
            push_presentation_frame(staged, frame);
        }
        if let Some(instance) = staged.state.animation_instances.get_mut(&instance_handle) {
            instance.controller = None;
        }
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
        require_projectable_controller(staged, request.controller.value)?;
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
        require_projectable_controller(staged, request.controller.value)?;
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
        require_projectable_controller(staged, request.controller.value)?;
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
        require_projectable_controller(staged, request.controller.value)?;
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
                start_offset_seconds: None,
                start_paused: false,
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
        let state = &mut *staged.state;
        let instance = state
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
            &state.render_resources,
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
        push_extra_frame(staged, frame);
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
            let sequence = staged.presentation_frames;
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
        if let Some(mut descriptor) = controller.detached.take() {
            // Its target was recreated: create the same projection on the new
            // target, then update it below if the state has since moved on.
            let revision = descriptor.controller.revision;
            descriptor.target = target;
            let op = controller
                .projector
                .create_from_descriptor(&assets, &targets, descriptor, meta)
                .map_err(|diagnostic| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_PROJECTION",
                        diagnostic.message,
                    )
                })?;
            controller.projected = true;
            controller.last_target = Some(target);
            controller.last_revision = Some(revision);
            let mut frame = PresentationFrameDiff::new();
            frame.ops.push(op);
            push_presentation_frame(staged, frame);
            return self.flush_animation_controller(controller_handle);
        }
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
        push_presentation_frame(staged, frame);
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
            admitted_clip_packs: u32::try_from(
                staged
                    .state
                    .render_resources
                    .iter()
                    .filter(|resource| {
                        resource.kind() == CsharpRenderResourceKind::AnimationClipPack
                    })
                    .count(),
            )
            .unwrap_or(u32::MAX),
            retained_clip_pack_associations: u32::try_from(
                staged
                    .state
                    .render_resources
                    .iter()
                    .filter_map(CsharpRenderResource::animated_mesh)
                    .filter(|mesh| mesh.runtime_format == AnimatedMeshRuntimeFormat::Glb)
                    .map(|mesh| mesh.clip_packs.len())
                    .sum::<usize>(),
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

    /// Makes the given facts the complete retained object set. A snapshot
    /// cannot name joints, so each object keeps its attachment.
    unsafe fn stage_snapshot(
        &mut self,
        facts: *const NativeAppearanceFact,
        fact_count: usize,
    ) -> Result<(), CsharpEngineServicesError> {
        // SAFETY: the callback is synchronous and the pointer/length pair came from C#.
        let facts = unsafe { borrowed_slice(facts, fact_count, "appearance facts") }?;
        let projector = &self.staged_ref()?.state.projector;
        let joints: BTreeMap<u64, String> = facts
            .iter()
            .filter_map(|fact| {
                let joint = projector.object_joint(fact.object_id)?;
                Some((fact.object_id, joint.to_owned()))
            })
            .collect();
        let attachments = joints
            .iter()
            .map(|(object, joint)| (*object, joint.as_str()))
            .collect();
        self.stage_changes(facts, None, &attachments)
    }

    /// Applies changed objects, removals and joint attachments to the
    /// retained scene. With no removal list the facts are the complete object
    /// set. Only the named objects are examined.
    fn stage_changes(
        &mut self,
        facts: &[NativeAppearanceFact],
        removals: Option<&[u64]>,
        attachments: &BTreeMap<u64, &str>,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state: &mut RuntimeAppearanceData = &mut staged.state;
        let mut owned = Vec::with_capacity(facts.len());
        for fact in facts {
            let appearance = state
                .appearances
                .get(&fact.appearance.value)
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_APPEARANCE_HANDLE",
                        "visual fact used an unknown appearance handle",
                    )
                })?;
            owned.push(RuntimeAppearanceFact {
                object_id: fact.object_id,
                parent_object_id: fact.has_parent_object.then_some(fact.parent_object_id),
                appearance,
                transform: native_transform(fact.transform),
                visible: fact.visible,
                layer: native_render_layer(fact.layer)?,
                shadow_casting: match fact.shadow_casting {
                    NativeShadowCasting::Cast => ShadowCasting::Cast,
                    NativeShadowCasting::None => ShadowCasting::None,
                },
                joint: attachments.get(&fact.object_id).copied(),
            });
        }
        for child in attachments.keys() {
            if !owned.iter().any(|fact| fact.object_id == *child) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_JOINT_ATTACHMENT",
                    format!("attachment child object {child} is not among the changed objects"),
                ));
            }
        }

        // Objects that animation controllers or ghost plates depend on must
        // keep their appearance.
        if !state.animation_controllers.is_empty() || !state.ghost_plates.is_empty() {
            let changed: BTreeMap<u64, &str> = owned
                .iter()
                .map(|fact| (fact.object_id, fact.appearance))
                .collect();
            let next_appearance = |object: u64| match changed.get(&object) {
                Some(appearance) => Some(*appearance),
                None if removals.is_none_or(|removals| removals.contains(&object)) => None,
                None => state.projector.object_appearance(object),
            };
            for controller in state.animation_controllers.values() {
                if !controller.projected {
                    continue;
                }
                let instance = state
                    .animation_instances
                    .get(&controller.instance)
                    .expect("live controller retains its instance");
                if next_appearance(instance.object_id)
                    != state
                        .appearances
                        .get(&instance.appearance)
                        .map(String::as_str)
                {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_SNAPSHOT_ORDER",
                        "remove a projected controller before removing or replacing its animated target",
                    ));
                }
            }
            for ghost in state.ghost_plates.values() {
                if next_appearance(ghost.source_object_id).is_none() {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_GHOST_PLATE_SNAPSHOT_ORDER",
                        "dispose ghost plate presentations before removing their source object",
                    ));
                }
            }
        }

        // Parented lights follow their parent in and out of the scene.
        let present = |object: u64| {
            owned.iter().any(|fact| fact.object_id == object)
                || removals.is_some_and(|removals| {
                    !removals.contains(&object) && state.projector.object_handle(object).is_some()
                })
        };
        let mut light_puts = Vec::new();
        let mut light_removals = Vec::new();
        let mut waiting = BTreeSet::new();
        for (handle, light) in &state.lights {
            let Some(parent) = light.parent_object_id else {
                continue;
            };
            let pending = state.pending_lights.contains(handle);
            match (pending, present(parent)) {
                (true, true) => light_puts.push(light.clone()),
                (true, false) => {
                    waiting.insert(*handle);
                }
                (false, false) => {
                    light_removals.push(light.light_id);
                    waiting.insert(*handle);
                }
                (false, true) => {}
            }
        }
        let projected =
            state
                .projector
                .change_with_lights(&owned, removals, &light_puts, &light_removals);
        let frame = projected.map_err(|error| match error {
            AppearanceProjectionError::JointAttachment { id, joint, problem } => {
                CsharpEngineServicesError::new(
                    "CSHARP_JOINT_ATTACHMENT",
                    format!("object {id} cannot follow joint '{joint}': {problem:?}"),
                )
            }
            error => CsharpEngineServicesError::new("CSHARP_VISUAL_SNAPSHOT", format!("{error:?}")),
        })?;
        staged.state.pending_lights = waiting;
        detach_retargeted_controllers(staged)?;
        append_projection_frame(staged, frame)?;
        self.flush_all_animations()?;
        Ok(())
    }

    /// Publishes the call's resource releases and any appearance changes
    /// still pending in the projector.
    fn publish_resource_releases(&mut self) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let frame = staged.state.projector.reconcile().map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_RESOURCE_RELEASE", format!("{error:?}"))
        })?;
        detach_retargeted_controllers(staged)?;
        push_extra_frame(staged, frame);
        self.flush_all_animations()
    }
}

fn animation_assets(resources: &RenderResourceRegistry) -> BTreeMap<String, ResolvedRenderAsset> {
    resources
        .iter()
        .filter(|resource| resource.kind() == CsharpRenderResourceKind::AnimatedMesh)
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
    resources: &RenderResourceRegistry,
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
            let asset_identity = resource.asset_identity().to_owned();
            (
                asset_identity.clone(),
                ResolvedRenderAsset {
                    id: asset_identity,
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

fn native_presentation_localized_text(
    localization_key: NativeUtf8Slice,
    fallback_text: NativeUtf8Slice,
    field: &'static str,
) -> Result<render_presentation::BillboardLocalizedText, CsharpEngineServicesError> {
    Ok(render_presentation::BillboardLocalizedText {
        localization_key: native_presentation_text(localization_key, field)?,
        fallback_text: native_presentation_text(fallback_text, field)?,
    })
}

fn native_presentation_billboard_layout(
    value: NativePresentationBillboardLayout,
) -> BillboardLayoutPolicy {
    BillboardLayoutPolicy {
        priority: value.priority,
        sizing: match value.sizing {
            NativePresentationBillboardLayoutSizing::ConstantPixels => {
                BillboardLayoutSizing::ConstantPixels
            }
            NativePresentationBillboardLayoutSizing::DistanceScaled => {
                BillboardLayoutSizing::DistanceScaled {
                    reference_distance: value.reference_distance,
                    min_scale: value.minimum_scale,
                    max_scale: value.maximum_scale,
                }
            }
        },
        safe_area: BillboardSafeArea {
            top_pixels: value.safe_area.top_pixels,
            right_pixels: value.safe_area.right_pixels,
            bottom_pixels: value.safe_area.bottom_pixels,
            left_pixels: value.safe_area.left_pixels,
        },
        edge_behavior: match value.edge_behavior {
            NativePresentationBillboardEdgeBehavior::Clamp => BillboardEdgeBehavior::Clamp,
            NativePresentationBillboardEdgeBehavior::Cull => BillboardEdgeBehavior::Cull,
        },
        overlap_behavior: match value.overlap_behavior {
            NativePresentationBillboardOverlapBehavior::Stack => BillboardOverlapBehavior::Stack,
            NativePresentationBillboardOverlapBehavior::Suppress => {
                BillboardOverlapBehavior::Suppress
            }
        },
    }
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

fn animation_asset_clips(resources: &RenderResourceRegistry, asset: &str) -> Vec<String> {
    resources
        .iter()
        .filter(|resource| resource.kind() == CsharpRenderResourceKind::AnimatedMesh)
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

fn animation_asset_has_clip(resources: &RenderResourceRegistry, asset: &str, clip: &str) -> bool {
    animation_asset_clips(resources, asset)
        .iter()
        .any(|candidate| candidate == clip)
}

fn command_clip(command: &AnimatedMeshPlaybackCommand) -> Option<&str> {
    match command {
        AnimatedMeshPlaybackCommand::Play { clip, .. }
        | AnimatedMeshPlaybackCommand::Sample { clip, .. } => Some(clip),
        AnimatedMeshPlaybackCommand::Stop { .. }
        | AnimatedMeshPlaybackCommand::SamplePose { .. }
        | AnimatedMeshPlaybackCommand::Pause
        | AnimatedMeshPlaybackCommand::Resume => None,
    }
}

fn native_transform(value: NativeTransform) -> Transform {
    Transform {
        translation: [
            value.translation.x,
            value.translation.y,
            value.translation.z,
        ],
        rotation: [
            value.rotation.x,
            value.rotation.y,
            value.rotation.z,
            value.rotation.w,
        ],
        scale: [value.scale.x, value.scale.y, value.scale.z],
    }
}

fn native_render_layer(value: NativeRenderLayer) -> Result<RenderLayer, CsharpEngineServicesError> {
    match value {
        NativeRenderLayer::Scene => Ok(RenderLayer::Scene),
        NativeRenderLayer::Debug => Ok(RenderLayer::Debug),
        NativeRenderLayer::Ui => Ok(RenderLayer::Ui),
        NativeRenderLayer::Viewmodel => Ok(RenderLayer::Viewmodel),
        NativeRenderLayer::Backdrop => Ok(RenderLayer::Backdrop),
    }
}

/// A projected controller whose target left the published snapshot cannot
fn runtime_material_id(handle: u64) -> String {
    format!("material/csharp-{handle}")
}

/// flush. Refuse before changing controller state so the refusal is local.
fn require_projectable_controller(
    staged: &RuntimeAppearanceCall,
    controller_handle: u64,
) -> Result<(), CsharpEngineServicesError> {
    let controller = staged
        .state
        .animation_controllers
        .get(&controller_handle)
        .expect("controller was checked by the caller");
    let object_id = staged
        .state
        .animation_instances
        .get(&controller.instance)
        .expect("controller instance remains live")
        .object_id;
    if controller.projected && staged.state.projector.object_handle(object_id).is_none() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_ANIMATION_SNAPSHOT_ORDER",
            "remove a projected controller before publishing a snapshot that removes or replaces its animated target",
        ));
    }
    Ok(())
}

/// Projects changed lights. A refused change leaves the retained lights unchanged.
fn project_light_change(
    staged: &mut RuntimeAppearanceCall,
    facts: &[RuntimeLightFact],
    removals: &[u64],
) -> Result<(), CsharpEngineServicesError> {
    let frame = staged
        .state
        .projector
        .apply_lights(facts, removals)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_LIGHT_PROJECTION", format!("{error:?}"))
        })?;
    append_projection_frame(staged, frame)
}

/// Makes `fact` the light under `handle`, replacing the live light under
/// `previous` (the same handle for an update). A light whose parent object is
/// not in the published scene waits, unprojected, for a snapshot that
/// publishes it; a refused change leaves the lights as they were.
fn set_light(
    staged: &mut RuntimeAppearanceCall,
    previous: Option<u64>,
    handle: u64,
    fact: RuntimeLightFact,
) -> Result<(), CsharpEngineServicesError> {
    let state = &staged.state;
    let ready = fact
        .parent_object_id
        .is_none_or(|parent| state.projector.object_handle(parent).is_some());
    // A shown previous light leaves the scene when the replacement waits or
    // takes another logical id; otherwise the replacement updates it.
    let removals: Vec<u64> = previous
        .filter(|previous| !state.pending_lights.contains(previous))
        .and_then(|previous| state.lights.get(&previous))
        .map(|shown| shown.light_id)
        .filter(|id| !ready || *id != fact.light_id)
        .into_iter()
        .collect();
    // The descriptor was validated on entry, so a waiting light cannot fail
    // the publish that shows it.
    let puts = if ready {
        std::slice::from_ref(&fact)
    } else {
        &[]
    };
    if !puts.is_empty() || !removals.is_empty() {
        project_light_change(staged, puts, &removals)?;
    }
    let state = &mut staged.state;
    if let Some(previous) = previous {
        state.lights.remove(&previous);
        state.pending_lights.remove(&previous);
    }
    if !ready {
        state.pending_lights.insert(handle);
    }
    state.lights.insert(handle, fact);
    Ok(())
}

/// Detaches every projected animation controller whose target the projector
/// has just recreated. Its Destroy goes out before the graphics frame that
/// removes the old target; the next flush recreates it on the new target.
fn detach_retargeted_controllers(
    staged: &mut RuntimeAppearanceCall,
) -> Result<(), CsharpEngineServicesError> {
    let data: &mut RuntimeAppearanceData = &mut staged.state;
    let mut frame = PresentationFrameDiff::new();
    for controller in data.animation_controllers.values_mut() {
        if !controller.projected {
            continue;
        }
        let Some(instance) = data.animation_instances.get(&controller.instance) else {
            continue;
        };
        let current = data.projector.object_handle(instance.object_id);
        if current.is_none() || current == controller.last_target {
            continue;
        }
        let (op, descriptor) = controller
            .projector
            .detach_entity(
                instance.object_id,
                PresentationOpMeta::new(staged.presentation_frames),
            )
            .map_err(|diagnostic| {
                CsharpEngineServicesError::new("CSHARP_ANIMATION_PROJECTION", diagnostic.message)
            })?;
        frame.ops.push(op);
        controller.projected = false;
        controller.last_target = None;
        controller.last_revision = None;
        controller.detached = Some(descriptor);
    }
    if !frame.ops.is_empty() {
        push_presentation_frame(staged, frame);
    }
    Ok(())
}

fn append_projection_frame(
    staged: &mut RuntimeAppearanceCall,
    next: render_model::RenderFrameDiff,
) -> Result<(), CsharpEngineServicesError> {
    staged.projected_frame = true;
    staged
        .outputs
        .push(RuntimeAppearanceCallOutput::Frame(next));
    Ok(())
}

pub(crate) fn push_extra_frame(
    staged: &mut RuntimeAppearanceCall,
    frame: render_model::RenderFrameDiff,
) {
    staged
        .outputs
        .push(RuntimeAppearanceCallOutput::Frame(frame));
}

fn push_presentation_frame(staged: &mut RuntimeAppearanceCall, frame: PresentationFrameDiff) {
    staged.presentation_frames = staged.presentation_frames.saturating_add(1);
    staged
        .outputs
        .push(RuntimeAppearanceCallOutput::Presentation(frame));
}

fn snapshot_presentation_sequence(length: usize) -> Result<u32, CsharpEngineServicesError> {
    u32::try_from(length).map_err(|_| {
        CsharpEngineServicesError::new(
            "CSHARP_PRESENTATION_BASELINE",
            "retained presentation baseline has too many operations",
        )
    })
}

fn runtime_light_fact(
    request: NativeLightRequest,
) -> Result<RuntimeLightFact, CsharpEngineServicesError> {
    let light = native_light_descriptor(request.descriptor)?;
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
        ground_color,
    ) = match &fact.light {
        LightDescriptor::Ambient {
            color,
            intensity,
            enabled,
            range,
            shadow_intent,
            ..
        } => (
            NativeLightKind::Ambient,
            *color,
            *intensity,
            *enabled,
            [0.0; 3],
            [0.0; 3],
            *range,
            0.0,
            0.0,
            0.0,
            *shadow_intent,
            [0.0; 3],
        ),
        LightDescriptor::Hemisphere {
            color,
            ground_color,
            intensity,
            enabled,
        } => (
            NativeLightKind::Hemisphere,
            *color,
            *intensity,
            *enabled,
            [0.0; 3],
            [0.0; 3],
            None,
            0.0,
            0.0,
            0.0,
            LightShadowIntent::Disabled,
            *ground_color,
        ),
        LightDescriptor::Directional {
            color,
            intensity,
            enabled,
            direction,
            range,
            shadow_intent,
            ..
        } => (
            NativeLightKind::Directional,
            *color,
            *intensity,
            *enabled,
            [0.0; 3],
            *direction,
            *range,
            0.0,
            0.0,
            0.0,
            *shadow_intent,
            [0.0; 3],
        ),
        LightDescriptor::Point {
            color,
            intensity,
            enabled,
            position,
            range,
            decay,
            shadow_intent,
            ..
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
            [0.0; 3],
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
            ..
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
            [0.0; 3],
        ),
    };
    let shadow = fact.light.shadow_settings();
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
            shadow_resolution: shadow.resolution,
            shadow_priority: shadow.priority,
            shadow_soft: shadow.soft,
            ground_color: NativeVec3 {
                x: ground_color[0],
                y: ground_color[1],
                z: ground_color[2],
            },
        },
    }
}

fn reset_sprite_playback(playback: &mut RuntimeSpritePlayback, state: NativeSpritePlaybackState) {
    playback.state = state;
    playback.frame_index = 0;
    playback.elapsed_in_frame_seconds = 0.0;
    playback.cycle = 0;
}

fn sprite_playback_state_after_selection(
    state: NativeSpritePlaybackState,
) -> NativeSpritePlaybackState {
    match state {
        NativeSpritePlaybackState::Completed => NativeSpritePlaybackState::Paused,
        state => state,
    }
}

fn sprite_playback_readout(playback: &RuntimeSpritePlayback) -> NativeSpritePlaybackReadout {
    NativeSpritePlaybackReadout {
        frame_id: playback.frames[playback.frame_index].frame_id,
        frame_index: playback.frame_index as u32,
        state: playback.state,
        elapsed_in_frame_seconds: playback.elapsed_in_frame_seconds,
        cycle: playback.cycle,
        revision: playback.revision,
        completed: playback.state == NativeSpritePlaybackState::Completed,
    }
}

fn validate_sprite_playback_update(
    facts: NativeProductUpdateFacts,
    last: Option<SpritePlaybackUpdateIdentity>,
) -> Result<SpritePlaybackUpdateIdentity, CsharpEngineServicesError> {
    if facts.lifecycle_state != NativeProductLifecycleState::Running {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_SPRITE_PLAYBACK_UPDATE",
            "sprite playback can advance only during a running product update",
        ));
    }
    if facts.admitted_step_count > 0
        && (!facts.fixed_delta_seconds.is_finite() || facts.fixed_delta_seconds <= 0.0)
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_SPRITE_PLAYBACK_UPDATE",
            "sprite playback requires positive finite Rust-admitted realtime steps",
        ));
    }
    let identity = SpritePlaybackUpdateIdentity {
        generation: facts.generation,
        control_revision: facts.control_revision,
        simulation_step: facts.simulation_step,
        admitted_step_count: facts.admitted_step_count,
    };
    if last.is_some_and(|previous| identity < previous) {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_SPRITE_PLAYBACK_STALE_UPDATE",
            "sprite playback update facts precede the last admitted update",
        ));
    }
    Ok(identity)
}

fn append_sprite_playback_markers(
    playback: &mut RuntimeSpritePlayback,
    crossings: &mut Vec<NativeSpritePlaybackMarkerCrossing>,
) -> Result<(), CsharpEngineServicesError> {
    for marker in playback
        .markers
        .iter()
        .filter(|marker| marker.frame_index as usize == playback.frame_index)
    {
        let sequence = playback.next_crossing_sequence;
        playback.next_crossing_sequence = sequence.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPRITE_PLAYBACK_MARKER",
                "sprite playback marker sequence overflow",
            )
        })?;
        crossings.push(NativeSpritePlaybackMarkerCrossing {
            marker_id: marker.marker_id,
            frame_id: playback.frames[playback.frame_index].frame_id,
            frame_index: playback.frame_index as u32,
            cycle: playback.cycle,
            crossing_sequence: sequence,
        });
    }
    Ok(())
}

fn sample_sprite_playback_at(
    playback: &RuntimeSpritePlayback,
    elapsed_seconds: f64,
) -> Result<NativeSpritePlaybackSample, CsharpEngineServicesError> {
    let total = playback
        .frames
        .iter()
        .map(|frame| frame.duration_seconds)
        .sum::<f64>();
    let (mut remaining, cycle, completed) = match playback.loop_mode {
        NativeSpritePlaybackLoopMode::OneShot if elapsed_seconds >= total => {
            let frame_index = playback.frames.len() - 1;
            return Ok(NativeSpritePlaybackSample {
                frame_id: playback.frames[frame_index].frame_id,
                frame_index: frame_index as u32,
                elapsed_in_frame_seconds: playback.frames[frame_index].duration_seconds,
                cycle: 0,
                completed: true,
            });
        }
        NativeSpritePlaybackLoopMode::OneShot => (elapsed_seconds, 0, false),
        NativeSpritePlaybackLoopMode::Loop => {
            let cycles = (elapsed_seconds / total).floor();
            if cycles > u64::MAX as f64 {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_SPRITE_PLAYBACK_SAMPLE",
                    "sprite playback sample cycle is out of range",
                ));
            }
            (elapsed_seconds % total, cycles as u64, false)
        }
    };
    for (frame_index, frame) in playback.frames.iter().enumerate() {
        if remaining < frame.duration_seconds {
            return Ok(NativeSpritePlaybackSample {
                frame_id: frame.frame_id,
                frame_index: frame_index as u32,
                elapsed_in_frame_seconds: remaining,
                cycle,
                completed,
            });
        }
        remaining -= frame.duration_seconds;
    }
    Err(CsharpEngineServicesError::new(
        "CSHARP_SPRITE_PLAYBACK_SAMPLE",
        "sprite playback sample could not resolve a sequence frame",
    ))
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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
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
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn destroy_render_resource(
    context: *mut c_void,
    resource: NativeRenderResourceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_resource(resource))
    })
}

pub(crate) unsafe extern "C" fn create_primitive_appearance(
    context: *mut c_void,
    request: NativePrimitiveAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| bridge.create_primitive(request))
    })
}

pub(crate) unsafe extern "C" fn replace_primitive_appearance(
    context: *mut c_void,
    request: NativePrimitiveAppearanceReplaceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| bridge.replace_primitive(request))
    })
}

pub(crate) unsafe extern "C" fn create_mesh_appearance(
    context: *mut c_void,
    resource: NativeMeshResourceHandle,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| {
            bridge.create_mesh_appearance(resource)
        })
    })
}

pub(crate) unsafe extern "C" fn destroy_mesh_resource(
    context: *mut c_void,
    resource: NativeMeshResourceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_mesh_resource(resource))
    })
}

pub(crate) unsafe extern "C" fn create_static_mesh_appearance(
    context: *mut c_void,
    request: *const NativeStaticMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        appearance_result(context, result, |bridge| unsafe {
            bridge.create_static_mesh(&*request)
        })
    })
}

pub(crate) unsafe extern "C" fn create_static_mesh_from_content_appearance(
    context: *mut c_void,
    request: *const NativeStaticMeshContentAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        appearance_result(context, result, |bridge| {
            bridge.create_static_mesh_from_content(unsafe { &*request })
        })
    })
}

pub(crate) unsafe extern "C" fn replace_static_mesh_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    request: *const NativeStaticMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        appearance_result(context, result, |bridge| unsafe {
            let request = &*request;
            bridge.destroy_appearance(appearance)?;
            bridge.create_static_mesh(request)
        })
    })
}

pub(crate) unsafe extern "C" fn replace_static_mesh_from_content_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    request: *const NativeStaticMeshContentAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        appearance_result(context, result, |bridge| {
            let request = unsafe { &*request };
            bridge.destroy_appearance(appearance)?;
            bridge.create_static_mesh_from_content(request)
        })
    })
}

pub(crate) unsafe extern "C" fn update_static_mesh_materials(
    context: *mut c_void,
    request: *const NativeStaticMeshMaterialUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() {
            return 0;
        }
        appearance_void(context, |bridge| unsafe {
            bridge.update_static_mesh_materials(&*request)
        })
    })
}

pub(crate) unsafe extern "C" fn update_static_mesh_material_factors(
    context: *mut c_void,
    request: *const NativeStaticMeshMaterialFactorsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() {
            return 0;
        }
        appearance_void(context, |bridge| unsafe {
            bridge.update_static_mesh_material_factors(&*request)
        })
    })
}

pub(crate) unsafe extern "C" fn create_sprite_appearance(
    context: *mut c_void,
    request: NativeSpriteAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| bridge.create_sprite(request))
    })
}

pub(crate) unsafe extern "C" fn replace_sprite_appearance(
    context: *mut c_void,
    request: NativeSpriteAppearanceReplaceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| bridge.replace_sprite(request))
    })
}

pub(crate) unsafe extern "C" fn create_sprite_atlas(
    context: *mut c_void,
    request: *const NativeSpriteAtlasCreateRequest,
    result: *mut NativeSpriteAtlasHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        sprite_atlas_result(context, result, |bridge| unsafe {
            bridge.create_sprite_atlas(&*request)
        })
    })
}

pub(crate) unsafe extern "C" fn destroy_sprite_atlas(
    context: *mut c_void,
    atlas: NativeSpriteAtlasHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_sprite_atlas(atlas))
    })
}

pub(crate) unsafe extern "C" fn create_sprite_from_atlas(
    context: *mut c_void,
    request: NativeSpriteFromAtlasRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| {
            bridge.create_sprite_from_atlas(request)
        })
    })
}

pub(crate) unsafe extern "C" fn replace_sprite_from_atlas(
    context: *mut c_void,
    request: NativeSpriteFromAtlasReplaceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_result(context, result, |bridge| {
            bridge.replace_sprite_from_atlas(request)
        })
    })
}

pub(crate) unsafe extern "C" fn set_sprite_frame(
    context: *mut c_void,
    request: NativeSpriteFrameUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.set_sprite_frame(request))
    })
}

pub(crate) unsafe extern "C" fn set_sprite_viewport(
    context: *mut c_void,
    request: NativeSpriteViewportUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.set_sprite_viewport(request))
    })
}

pub(crate) unsafe extern "C" fn read_sprite(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    result: *mut NativeSpriteReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match bridge.read_sprite(appearance) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn create_sprite_playback(
    context: *mut c_void,
    request: *const NativeSpritePlaybackCreateRequest,
    result: *mut NativeSpritePlaybackHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match unsafe { bridge.create_sprite_playback(&*request) } {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn destroy_sprite_playback(
    context: *mut c_void,
    playback: NativeSpritePlaybackHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_sprite_playback(playback))
    })
}

pub(crate) unsafe extern "C" fn control_sprite_playback(
    context: *mut c_void,
    request: NativeSpritePlaybackControlRequest,
    result: *mut NativeSpritePlaybackReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match bridge.control_sprite_playback(request) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn select_sprite_playback_frame(
    context: *mut c_void,
    request: NativeSpritePlaybackFrameSelectionRequest,
    result: *mut NativeSpritePlaybackReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match bridge.select_sprite_playback_frame(request) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn advance_sprite_playback(
    context: *mut c_void,
    request: *const NativeSpritePlaybackAdvanceRequest,
    result: *mut NativeSpritePlaybackAdvanceResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match bridge.advance_sprite_playback(unsafe { *request }) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn sample_sprite_playback(
    context: *mut c_void,
    request: NativeSpritePlaybackSampleRequest,
    result: *mut NativeSpritePlaybackSample,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match bridge.sample_sprite_playback(request) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn read_sprite_playback(
    context: *mut c_void,
    playback: NativeSpritePlaybackHandle,
    result: *mut NativeSpritePlaybackReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        match bridge.read_sprite_playback(playback) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn destroy_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_appearance(appearance))
    })
}

pub(crate) unsafe extern "C" fn create_light(
    context: *mut c_void,
    request: NativeLightRequest,
    result: *mut NativeLightHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        light_result(context, result, |bridge| bridge.create_light(request))
    })
}

pub(crate) unsafe extern "C" fn update_light(
    context: *mut c_void,
    request: NativeLightUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.update_light(request))
    })
}

pub(crate) unsafe extern "C" fn replace_light(
    context: *mut c_void,
    request: NativeLightUpdateRequest,
    result: *mut NativeLightHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        light_result(context, result, |bridge| bridge.replace_light(request))
    })
}

pub(crate) unsafe extern "C" fn destroy_light(
    context: *mut c_void,
    light: NativeLightHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_light(light))
    })
}

pub(crate) unsafe extern "C" fn read_light(
    context: *mut c_void,
    light: NativeLightHandle,
    result: *mut NativeLightReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
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
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn create_material(
    context: *mut c_void,
    request: NativeMaterialRequest,
    result: *mut NativeMaterialHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        material_result(context, result, |bridge| bridge.create_material(request))
    })
}

pub(crate) unsafe extern "C" fn create_terrain_layer_material(
    context: *mut c_void,
    request: *const NativeTerrainLayerMaterialRequest,
    result: *mut NativeMaterialHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        let request = unsafe { *request };
        material_result(context, result, |bridge| {
            bridge.create_terrain_layer_material(request)
        })
    })
}

pub(crate) unsafe extern "C" fn create_authored_material(
    context: *mut c_void,
    request: *const NativeAuthoredMaterialAppearanceRequest,
    result: *mut NativeMaterialHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() || result.is_null() {
            return 0;
        }
        let request = unsafe { *request };
        let material_id = match unsafe {
            borrowed_utf8(
                request.material_id.bytes,
                request.material_id.len,
                "authored material id",
            )
        } {
            Ok(material_id) => material_id,
            Err(error) => {
                unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() }.operation_error =
                    Some(error);
                return 0;
            }
        };
        material_result(context, result, |bridge| {
            bridge.create_authored_material(request, material_id)
        })
    })
}

pub(crate) unsafe extern "C" fn update_material(
    context: *mut c_void,
    request: NativeMaterialUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.update_material(request))
    })
}

pub(crate) unsafe extern "C" fn replace_material(
    context: *mut c_void,
    request: NativeMaterialUpdateRequest,
    result: *mut NativeMaterialHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        material_result(context, result, |bridge| bridge.replace_material(request))
    })
}

pub(crate) unsafe extern "C" fn destroy_material(
    context: *mut c_void,
    material: NativeMaterialHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_material(material))
    })
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
            bridge.operation_error = Some(error);
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
            bridge.operation_error = Some(error);
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
            bridge.operation_error = Some(error);
            0
        }
    }
}

fn sprite_atlas_result(
    context: *mut c_void,
    result: *mut NativeSpriteAtlasHandle,
    action: impl FnOnce(
        &mut RuntimeAppearanceBridge,
    ) -> Result<NativeSpriteAtlasHandle, CsharpEngineServicesError>,
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
            bridge.operation_error = Some(error);
            0
        }
    }
}

pub(crate) fn appearance_void(
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
            bridge.operation_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn publish_appearance_snapshot(
    context: *mut c_void,
    facts: *const NativeAppearanceFact,
    fact_count: usize,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() {
            return 0;
        }
        // SAFETY: context points at a box retained by `CsharpProductRuntime`.
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        // SAFETY: callback inputs are copied/validated before this method returns.
        match unsafe { bridge.stage_snapshot(facts, fact_count) } {
            Ok(()) => ABI_OK,
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn read_presentation(
    context: *mut c_void,
    result: *mut NativePresentationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        let state = bridge
            .staged
            .as_ref()
            .map(|call| &call.state)
            .unwrap_or(&bridge.state);
        let resource_count =
            match u32::try_from(state.render_resources.len() + state.generated_meshes.len()) {
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
                retained_object_count: u32::try_from(state.projector.retained_objects())
                    .unwrap_or(u32::MAX),
                appearance_count,
                material_count,
                resource_count,
            };
        }
        ABI_OK
    })
}

pub(crate) fn animation_result<T: Copy>(
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
            bridge.operation_error = Some(error);
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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.open_animated_mesh(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn open_animation_clip_pack(
    context: *mut c_void,
    request: *const NativeAnimationClipPackResourceRequest,
    result: *mut NativeRenderResourceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.open_animation_clip_pack(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn associate_animation_clip_pack(
    context: *mut c_void,
    request: *const NativeAnimationClipPackAssociationRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.associate_animation_clip_pack(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn create_animated_mesh_appearance(
    context: *mut c_void,
    request: *const NativeAnimatedMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.create_animated_mesh_appearance(unsafe { *request })
        })
    })
}
pub(crate) unsafe extern "C" fn replace_animated_mesh_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    request: *const NativeAnimatedMeshAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.replace_animated_mesh_appearance(appearance, unsafe { *request })
        })
    })
}
pub(crate) unsafe extern "C" fn set_mesh_inspection(
    context: *mut c_void,
    request: *const NativeAnimatedMeshInspectionRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| unsafe {
            bridge.set_mesh_inspection(&*request)
        })
    })
}
/// The projector identity and embedded material slots of a live animated
/// mesh appearance; `purpose` names the request in the refusal.
fn animated_appearance_slots(
    staged: &RuntimeAppearanceCall,
    appearance: NativeAppearanceHandle,
    purpose: &str,
) -> Result<(String, BTreeSet<u16>), CsharpEngineServicesError> {
    let identity = staged
        .state
        .appearances
        .get(&appearance.value)
        .cloned()
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_APPEARANCE_HANDLE",
                "appearance handle is not live",
            )
        })?;
    let resource_handle = staged
        .state
        .animated_appearances
        .get(&appearance.value)
        .copied()
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATED_MESH_APPEARANCE",
                format!("{purpose} require a live animated mesh appearance"),
            )
        })?;
    let embedded_slots = staged
        .state
        .render_resources
        .get(resource_handle)
        .and_then(CsharpRenderResource::animated_mesh)
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE_KIND",
                "animated appearance no longer has its admitted mesh resource",
            )
        })?
        .embedded_material_slots
        .iter()
        .map(|binding| binding.slot)
        .collect();
    Ok((identity, embedded_slots))
}

pub(crate) unsafe extern "C" fn update_animated_mesh_materials(
    context: *mut c_void,
    request: *const NativeAnimatedMeshMaterialUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| unsafe {
            bridge.update_animated_mesh_materials(&*request)
        })
    })
}
pub(crate) unsafe extern "C" fn update_animated_mesh_material_factors(
    context: *mut c_void,
    request: *const NativeAnimatedMeshMaterialFactorsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| unsafe {
            bridge.update_animated_mesh_material_factors(&*request)
        })
    })
}
pub(crate) unsafe extern "C" fn destroy_animated_mesh_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_void(context, |bridge| bridge.destroy_appearance(appearance))
    })
}
pub(crate) unsafe extern "C" fn create_animation_instance(
    context: *mut c_void,
    request: *const NativeAnimationInstanceRequest,
    result: *mut NativeAnimationInstanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.create_animation_instance(unsafe { *request })
        })
    })
}
pub(crate) unsafe extern "C" fn destroy_animation_instance(
    context: *mut c_void,
    value: NativeAnimationInstanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_void(context, |bridge| bridge.destroy_animation_instance(value))
    })
}
pub(crate) unsafe extern "C" fn replace_animation_instance(
    context: *mut c_void,
    prior: NativeAnimationInstanceHandle,
    request: *const NativeAnimationInstanceRequest,
    result: *mut NativeAnimationInstanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.replace_animation_instance(prior, unsafe { *request })
        })
    })
}
pub(crate) unsafe extern "C" fn set_animation_playback(
    context: *mut c_void,
    request: *const NativeAnimationPlaybackRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.set_animation_playback(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn create_animation_graph(
    context: *mut c_void,
    request: *const NativeAnimationGraphCreateRequest,
    result: *mut NativeAnimationGraphHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.create_animation_graph(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn destroy_animation_graph(
    context: *mut c_void,
    value: NativeAnimationGraphHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_void(context, |bridge| bridge.destroy_animation_graph(value))
    })
}
pub(crate) unsafe extern "C" fn define_animation_parameter(
    context: *mut c_void,
    request: *const NativeAnimationParameterDefinitionRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.define_animation_parameter(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn define_animation_state(
    context: *mut c_void,
    request: *const NativeAnimationStateDefinitionRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.define_animation_state(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn define_animation_transition(
    context: *mut c_void,
    request: *const NativeAnimationTransitionDefinitionRequest,
    result: *mut NativeAnimationTransitionHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.define_animation_transition(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn define_animation_condition(
    context: *mut c_void,
    request: *const NativeAnimationConditionDefinitionRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.define_animation_condition(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn create_animation_controller(
    context: *mut c_void,
    request: *const NativeAnimationControllerCreateRequest,
    result: *mut NativeAnimationControllerHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.create_animation_controller(unsafe { *request })
        })
    })
}
pub(crate) unsafe extern "C" fn destroy_animation_controller(
    context: *mut c_void,
    value: NativeAnimationControllerHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_void(context, |bridge| bridge.destroy_animation_controller(value))
    })
}
pub(crate) unsafe extern "C" fn set_animation_float(
    context: *mut c_void,
    request: *const NativeAnimationSetFloatRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.set_animation_float(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn set_animation_bool(
    context: *mut c_void,
    request: *const NativeAnimationSetBoolRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.set_animation_bool(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn fire_animation_trigger(
    context: *mut c_void,
    request: *const NativeAnimationFireTriggerRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.fire_animation_trigger(unsafe { &*request })
        })
    })
}
pub(crate) unsafe extern "C" fn tick_animation(
    context: *mut c_void,
    request: *const NativeAnimationTickRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| bridge.tick_animation(unsafe { *request }))
    })
}
pub(crate) unsafe extern "C" fn read_animation_controller(
    context: *mut c_void,
    value: NativeAnimationControllerHandle,
    result: *mut NativeAnimationControllerReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            bridge.read_animation_controller(value)
        })
    })
}
pub(crate) unsafe extern "C" fn read_animation(
    context: *mut c_void,
    result: *mut NativeAnimationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, RuntimeAppearanceBridge::read_animation)
    })
}

pub(crate) unsafe extern "C" fn read_animation_realization(
    context: *mut c_void,
    result: *mut NativeAnimationRealizationResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            bridge.read_animation_realization()
        })
    })
}
pub(crate) fn animation_api(bridge: &mut RuntimeAppearanceBridge) -> NativeAnimationApi {
    NativeAnimationApi {
        context: (bridge as *mut RuntimeAppearanceBridge).cast(),
        read_mesh_info: read_animated_mesh_info,
        read_clips: read_animation_clips,
        open_animated_mesh,
        open_animated_mesh_from_content,
        open_animation_clip_pack_from_content,
        open_animation_clip_pack,
        associate_animation_clip_pack,
        create_animated_mesh_appearance,
        replace_animated_mesh_appearance,
        set_mesh_inspection,
        update_animated_mesh_materials,
        update_animated_mesh_material_factors,
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
        read_realization: read_animation_realization,
    }
}

fn animation_feedback_text(value: &str) -> NativeAnimationFeedbackText {
    let bytes = value.as_bytes();
    debug_assert!(
        bytes.len() <= 96,
        "ProductHost ingress bounds inline animation text"
    );
    let mut out = NativeAnimationFeedbackText::default();
    let length = bytes.len();
    out.len = length as u32;
    out.bytes[..length].copy_from_slice(&bytes[..length]);
    out
}
fn animation_realization_receipt(
    fact: &AnimationRealizationFact,
) -> NativeAnimationRealizationFact {
    let mut out = NativeAnimationRealizationFact::default();
    match fact {
        AnimationRealizationFact::Playback {
            fact_id,
            object_id,
            generation,
            sequence,
            status,
            clip,
            sampled_millis,
        } => {
            out.kind = NativeAnimationRealizationFactKind::PlaybackObservation;
            out.fact_id = *fact_id;
            out.object_id = *object_id;
            out.generation = *generation;
            out.has_object_id = true;
            out.has_generation = true;
            out.sequence = *sequence;
            out.status = animation_feedback_text(status);
            out.clip = animation_feedback_text(clip.as_deref().unwrap_or(""));
            out.has_sampled_millis = sampled_millis.is_some();
            out.sampled_millis = sampled_millis.unwrap_or(0);
        }
        AnimationRealizationFact::MeshInspection {
            fact_id,
            object_id,
            generation,
            request,
            bounds_min,
            bounds_max,
            has_bounds,
            voxel_normal_meshes,
        } => {
            out.kind = NativeAnimationRealizationFactKind::MeshInspection;
            out.fact_id = *fact_id;
            out.object_id = *object_id;
            out.generation = *generation;
            out.has_object_id = true;
            out.has_generation = true;
            out.bounds_request = *request;
            out.bounds_min = NativeVec3 {
                x: bounds_min[0],
                y: bounds_min[1],
                z: bounds_min[2],
            };
            out.bounds_max = NativeVec3 {
                x: bounds_max[0],
                y: bounds_max[1],
                z: bounds_max[2],
            };
            out.has_bounds = *has_bounds;
            out.voxel_normal_meshes = *voxel_normal_meshes;
        }
        AnimationRealizationFact::NaturalCompletion {
            fact_id,
            object_id,
            generation,
            clip,
        } => {
            out.kind = NativeAnimationRealizationFactKind::NaturalCompletion;
            out.fact_id = *fact_id;
            out.object_id = *object_id;
            out.generation = *generation;
            out.has_object_id = true;
            out.has_generation = true;
            out.clip = animation_feedback_text(clip);
        }
        AnimationRealizationFact::Diagnostic {
            fact_id,
            object_id,
            generation,
            code,
            sequence,
        } => {
            out.kind = NativeAnimationRealizationFactKind::Diagnostic;
            out.fact_id = *fact_id;
            out.object_id = object_id.unwrap_or(0);
            out.generation = generation.unwrap_or(0);
            out.has_object_id = object_id.is_some();
            out.has_generation = generation.is_some();
            out.sequence = *sequence;
            out.diagnostic_code = animation_feedback_text(code);
        }
        AnimationRealizationFact::Stopped {
            fact_id,
            object_id,
            generation,
            sequence,
            reason,
        } => {
            out.kind = NativeAnimationRealizationFactKind::Stopped;
            out.fact_id = *fact_id;
            out.object_id = *object_id;
            out.generation = *generation;
            out.has_object_id = true;
            out.has_generation = true;
            out.sequence = *sequence;
            out.reason = animation_feedback_text(reason);
        }
    };
    out
}

fn borrowed_request_utf8(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<String, CsharpEngineServicesError> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }.map(str::to_owned)
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

fn native_ghost_plate_placement(value: NativeGhostPlatePlacement) -> GhostPlatePlacement {
    GhostPlatePlacement {
        transform: Transform {
            translation: native_vec3_array(value.transform.translation),
            rotation: [
                value.transform.rotation.x,
                value.transform.rotation.y,
                value.transform.rotation.z,
                value.transform.rotation.w,
            ],
            scale: native_vec3_array(value.transform.scale),
        },
        width: value.width,
        height: value.height,
    }
}

fn native_ghost_plate_capture(value: NativeGhostPlateCaptureSettings) -> GhostPlateCaptureSettings {
    GhostPlateCaptureSettings {
        resolution: value.resolution,
        azimuth_degrees: value.azimuth_degrees,
        elevation_degrees: value.elevation_degrees,
        near: value.near,
        far: value.far,
        field_of_view_degrees: value.field_of_view_degrees,
        lighting: GhostPlateCaptureLighting {
            mode: match value.lighting.mode {
                NativeGhostPlateCaptureLightingMode::Scene => GhostPlateCaptureLightingMode::Scene,
                NativeGhostPlateCaptureLightingMode::Isolated => {
                    GhostPlateCaptureLightingMode::Isolated
                }
            },
            ambient_color: native_vec3_array(value.lighting.ambient_color),
            ambient_intensity: value.lighting.ambient_intensity,
            key_direction: native_vec3_array(value.lighting.key_direction),
            key_color: native_vec3_array(value.lighting.key_color),
            key_intensity: value.lighting.key_intensity,
            fill_direction: native_vec3_array(value.lighting.fill_direction),
            fill_color: native_vec3_array(value.lighting.fill_color),
            fill_intensity: value.lighting.fill_intensity,
        },
    }
}

fn native_ghost_plate_config(value: NativeGhostPlateConfig) -> GhostPlateConfig {
    GhostPlateConfig {
        depth_retention: value.depth_retention,
        anchor_policy: match value.anchor_policy {
            NativeGhostPlateAnchorPolicy::BoundsCenter => GhostPlateAnchorPolicy::BoundsCenter,
            NativeGhostPlateAnchorPolicy::BoundsNormalized => {
                GhostPlateAnchorPolicy::BoundsNormalized
            }
        },
        anchor_value: value.anchor_value,
        plate_mapping: match value.plate_mapping {
            NativeGhostPlateMapping::PlateLocked => GhostPlateMapping::PlateLocked,
            NativeGhostPlateMapping::ProjectiveSurface => GhostPlateMapping::ProjectiveSurface,
        },
        shell_mode: match value.shell_mode {
            NativeGhostPlateShellMode::WholeMesh => GhostPlateShellMode::WholeMesh,
            NativeGhostPlateShellMode::StrictSource => GhostPlateShellMode::StrictSource,
            NativeGhostPlateShellMode::RepairedSource => GhostPlateShellMode::RepairedSource,
        },
        shell_depth_epsilon: value.shell_depth_epsilon,
        sector_count: value.sector_count,
        sector_hysteresis_degrees: value.sector_hysteresis_degrees,
    }
}

fn ghost_plate_descriptor(
    presentation: &RuntimeGhostPlatePresentation,
    source: RenderHandle,
) -> GhostPlateDescriptor {
    GhostPlateDescriptor {
        source,
        captured_scene: None,
        placement: presentation.placement.clone(),
        capture: presentation.capture.clone(),
        config: presentation.config.clone(),
    }
}

fn native_ghost_plate_capture_readout(
    value: &GhostPlateCaptureSettings,
) -> NativeGhostPlateCaptureSettings {
    NativeGhostPlateCaptureSettings {
        resolution: value.resolution,
        azimuth_degrees: value.azimuth_degrees,
        elevation_degrees: value.elevation_degrees,
        near: value.near,
        far: value.far,
        field_of_view_degrees: value.field_of_view_degrees,
        lighting: NativeGhostPlateCaptureLighting {
            mode: match value.lighting.mode {
                GhostPlateCaptureLightingMode::Scene => NativeGhostPlateCaptureLightingMode::Scene,
                GhostPlateCaptureLightingMode::Isolated => {
                    NativeGhostPlateCaptureLightingMode::Isolated
                }
            },
            ambient_color: NativeVec3 {
                x: value.lighting.ambient_color[0],
                y: value.lighting.ambient_color[1],
                z: value.lighting.ambient_color[2],
            },
            ambient_intensity: value.lighting.ambient_intensity,
            key_direction: NativeVec3 {
                x: value.lighting.key_direction[0],
                y: value.lighting.key_direction[1],
                z: value.lighting.key_direction[2],
            },
            key_color: NativeVec3 {
                x: value.lighting.key_color[0],
                y: value.lighting.key_color[1],
                z: value.lighting.key_color[2],
            },
            key_intensity: value.lighting.key_intensity,
            fill_direction: NativeVec3 {
                x: value.lighting.fill_direction[0],
                y: value.lighting.fill_direction[1],
                z: value.lighting.fill_direction[2],
            },
            fill_color: NativeVec3 {
                x: value.lighting.fill_color[0],
                y: value.lighting.fill_color[1],
                z: value.lighting.fill_color[2],
            },
            fill_intensity: value.lighting.fill_intensity,
        },
    }
}

fn native_ghost_plate_config_readout(value: &GhostPlateConfig) -> NativeGhostPlateConfig {
    NativeGhostPlateConfig {
        depth_retention: value.depth_retention,
        anchor_policy: match value.anchor_policy {
            GhostPlateAnchorPolicy::BoundsCenter => NativeGhostPlateAnchorPolicy::BoundsCenter,
            GhostPlateAnchorPolicy::BoundsNormalized => {
                NativeGhostPlateAnchorPolicy::BoundsNormalized
            }
        },
        anchor_value: value.anchor_value,
        plate_mapping: match value.plate_mapping {
            GhostPlateMapping::PlateLocked => NativeGhostPlateMapping::PlateLocked,
            GhostPlateMapping::ProjectiveSurface => NativeGhostPlateMapping::ProjectiveSurface,
        },
        shell_mode: match value.shell_mode {
            GhostPlateShellMode::WholeMesh => NativeGhostPlateShellMode::WholeMesh,
            GhostPlateShellMode::StrictSource => NativeGhostPlateShellMode::StrictSource,
            GhostPlateShellMode::RepairedSource => NativeGhostPlateShellMode::RepairedSource,
        },
        shell_depth_epsilon: value.shell_depth_epsilon,
        sector_count: value.sector_count,
        sector_hysteresis_degrees: value.sector_hysteresis_degrees,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the descriptor maps the fixed copied C ABI sprite fields directly into Engine facts"
)]
fn sprite_instance_descriptor(
    asset: String,
    frame: u32,
    pivot: NativeVec2,
    size: NativeVec2,
    billboard: NativeBillboardMode,
    size_mode: NativeSpriteSizeMode,
    render_order: i32,
    depth: NativeSpriteDepthPolicy,
    tint: NativeColor,
    material: SpriteMaterialDescriptor,
) -> SpriteInstanceDescriptor {
    SpriteInstanceDescriptor {
        asset,
        frame,
        pivot: native_vec2(pivot),
        size: native_vec2(size),
        size_mode: match size_mode {
            NativeSpriteSizeMode::World => SpriteSizeMode::World,
            NativeSpriteSizeMode::Pixel => SpriteSizeMode::Pixel,
        },
        billboard: match billboard {
            NativeBillboardMode::None => BillboardMode::None,
            NativeBillboardMode::Spherical => BillboardMode::Spherical,
            NativeBillboardMode::Cylindrical => BillboardMode::Cylindrical,
        },
        tint: native_color(tint),
        render_order,
        depth: match depth {
            NativeSpriteDepthPolicy::Default => SpriteDepthPolicy::Default,
            NativeSpriteDepthPolicy::DepthTestOff => SpriteDepthPolicy::DepthTestOff,
            NativeSpriteDepthPolicy::DepthWriteOff => SpriteDepthPolicy::DepthWriteOff,
        },
        layer: render_model::RenderLayer::Scene,
        viewport_placement: None,
        shading: match material.lighting {
            SpriteLightingMode::Unlit => SpriteShading::Unlit,
            _ => SpriteShading::Lit,
        },
        material,
        visible: true,
        transform: Transform::IDENTITY,
        attachment: SpriteAttachment::default(),
        metadata: RenderMetadata::default(),
    }
}

fn sprite_texture_descriptor(
    resource: &CsharpRenderResource,
    texture_id: String,
) -> Result<TextureDescriptor, CsharpEngineServicesError> {
    let mut texture = resource.texture().cloned().ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_SPRITE_TEXTURE",
            "sprite texture resource did not retain an admitted texture descriptor",
        )
    })?;
    texture.id = texture_id;
    texture.validate().map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_SPRITE_TEXTURE", format!("{error:?}"))
    })?;
    Ok(texture)
}

fn render_material(id: String, color: NativeColor) -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        texture_transform: None,
        stochastic_tiling: None,
        terrain_layers: None,
        shader: None,
        id,
        color: native_color(color),
        texture: None,
        roughness: 1.0,
        metalness: 0.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: MaterialAlphaModeDescriptor::Opaque,
        double_sided: false,
        voxel_surface: None,
        normal_map: None,
        triplanar: None,
        emission_map: Default::default(),
        occlusion_map: Default::default(),
        unlit: false,
        flat_shading: false,
        wind: None,
        water: None,
        translucent_shadow: false,
    }
}

fn resource_set(handles: impl IntoIterator<Item = u64>) -> BTreeSet<u64> {
    handles.into_iter().filter(|handle| *handle != 0).collect()
}

fn resource_live_owner(state: &RuntimeAppearanceState, handle: u64) -> Option<&'static str> {
    if state
        .appearance_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("appearance");
    }
    if state
        .material_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("material");
    }
    if state
        .sprite_atlas_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("sprite atlas");
    }
    if state
        .animation_graph_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("animation graph");
    }
    if state
        .animation_clip_pack_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("animation clip-pack association");
    }
    if state
        .billboard_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("billboard");
    }
    if state
        .emitter_resources
        .values()
        .any(|resources| resources.contains(&handle))
    {
        return Some("particle emitter");
    }
    None
}

fn remove_resource(
    state: &mut RuntimeAppearanceState,
    handle: u64,
) -> Result<CsharpRenderResource, CsharpEngineServicesError> {
    let resource = state.render_resources.remove(handle)?;
    if let Some(texture) = resource.texture() {
        state
            .projector
            .resources_mut()
            .textures
            .retain(|entry| entry.id != texture.id);
    }
    if let Some(animated) = resource.animated_mesh() {
        state
            .projector
            .resources_mut()
            .animated_meshes
            .retain(|entry| entry.asset != animated.asset);
    }
    if let Some(shader) = resource.shader() {
        state
            .projector
            .resources_mut()
            .shaders
            .retain(|entry| entry.id != shader.id);
    }
    state.animation_clip_pack_resources.remove(&handle);
    Ok(resource)
}

fn release_unowned_internal_resources(state: &mut RuntimeAppearanceState) {
    loop {
        let orphan = state
            .render_resources
            .unowned(|handle| resource_live_owner(state, handle).is_some());
        let Some(handle) = orphan else { break };
        // The handle came from a monotonic internal slot and has no exposed
        // owner. `resource_live_owner` establishes that no retained fact can
        // still name it.
        remove_resource(state, handle).expect("live internal resource slot");
    }
}

fn material_descriptor(
    id: String,
    request: NativeMaterialRequest,
    resources: &RenderResourceRegistry,
) -> Result<RenderMaterialDescriptor, CsharpEngineServicesError> {
    let texture = if request.texture.value == 0 {
        None
    } else {
        let resource = resources.get(request.texture.value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_MATERIAL_TEXTURE", "unknown texture handle")
        })?;
        if resource.kind() != CsharpRenderResourceKind::Texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_TEXTURE",
                "material texture must be an admitted texture resource",
            ));
        }
        Some(resource.asset_identity().to_owned())
    };
    let normal_map = normal_map_descriptor(resources, request.normal_map, request.normal_scale)?;
    let emission_map = map_texture(
        resources,
        request.emission_map,
        false,
        "CSHARP_MATERIAL_EMISSION_MAP",
        "a material's emission map must be an admitted texture resource",
    )?
    .map(|texture| render_model::MaterialEmissionMapDescriptor { texture });
    if request.occlusion_map.value != 0 && request.orm_map.value != 0 {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_MATERIAL_OCCLUSION_MAP",
            "a material takes an occlusion map or an ORM map, not both",
        ));
    }
    let occlusion_strength = if request.occlusion_strength == 0.0 {
        1.0
    } else {
        request.occlusion_strength
    };
    let occlusion_map = map_texture(
        resources,
        request.occlusion_map,
        true,
        "CSHARP_MATERIAL_OCCLUSION_MAP",
        "a material's occlusion map must be a texture opened with TextureColorSpace.Linear",
    )?
    .map(|texture| render_model::MaterialOcclusionMapDescriptor {
        texture,
        strength: occlusion_strength,
        roughness_metalness: false,
    })
    .or(map_texture(
        resources,
        request.orm_map,
        true,
        "CSHARP_MATERIAL_ORM_MAP",
        "a material's ORM map must be a texture opened with TextureColorSpace.Linear",
    )?
    .map(|texture| {
        render_model::MaterialOcclusionMapDescriptor::occlusion_roughness_metalness(
            texture,
            occlusion_strength,
        )
    }));
    let descriptor = RenderMaterialDescriptor {
        texture_transform: texture_transform_descriptor(
            request.texture_scale,
            request.texture_offset,
        ),
        stochastic_tiling: (request.stochastic_tiling != 0.0).then_some(
            render_model::MaterialStochasticTilingDescriptor {
                contrast: request.stochastic_tiling,
            },
        ),
        terrain_layers: None,
        id,
        color: native_color(request.color),
        texture,
        roughness: request.roughness,
        metalness: request.metalness,
        texture_tint: native_color(request.texture_tint),
        emission_color: native_vec3_array(request.emission_color),
        emission_intensity: request.emission_intensity,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: match request.alpha_mode {
            NativeMaterialAlphaMode::Opaque => MaterialAlphaModeDescriptor::Opaque,
            NativeMaterialAlphaMode::Mask => MaterialAlphaModeDescriptor::Mask {
                cutoff: request.alpha_cutoff,
            },
            NativeMaterialAlphaMode::Blend => MaterialAlphaModeDescriptor::Blend,
        },
        double_sided: request.double_sided,
        voxel_surface: None,
        normal_map,
        unlit: request.unlit,
        flat_shading: request.flat_shading,
        wind: (request.wind_bend != 0.0 || request.wind_flutter != 0.0).then_some(
            render_model::MaterialWindDescriptor {
                bend: request.wind_bend,
                flutter: request.wind_flutter,
            },
        ),
        water: water_descriptor(resources, request.water)?,
        translucent_shadow: request.translucent_shadow,
        emission_map,
        occlusion_map,
        triplanar: triplanar_descriptor(request.triplanar_sharpness),
        shader: material_shader(resources, request.shader)?.map(|(shader, _)| shader),
    };
    descriptor.validate().map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_MATERIAL", format!("material is invalid: {error:?}"))
    })?;
    Ok(descriptor)
}

/// The water surface a request asks for: none at depth scale 0. Its foam
/// texture is any texture; its ripple texture a normal map opened with a
/// linear colour space.
fn water_descriptor(
    resources: &RenderResourceRegistry,
    water: NativeMaterialWater,
) -> Result<Option<render_model::MaterialWaterDescriptor>, CsharpEngineServicesError> {
    if water.depth_scale == 0.0 {
        return Ok(None);
    }
    let foam_texture = if water.foam_texture.value == 0 {
        None
    } else {
        Some(
            resources
                .get(water.foam_texture.value)
                .and_then(CsharpRenderResource::texture)
                .map(|texture| texture.id.clone())
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_MATERIAL_WATER",
                        "a water material's foam texture must be a texture resource",
                    )
                })?,
        )
    };
    let ripple_texture = normal_map_descriptor(resources, water.ripple_texture, 1.0)
        .map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_WATER",
                "a water material's ripple texture must be a texture opened with TextureColorSpace.Linear",
            )
        })?
        .map(|map| map.texture);
    Ok(Some(render_model::MaterialWaterDescriptor {
        shallow_color: [
            water.shallow_color.r,
            water.shallow_color.g,
            water.shallow_color.b,
        ],
        deep_color: [water.deep_color.r, water.deep_color.g, water.deep_color.b],
        depth_scale: water.depth_scale,
        shoreline_width: water.shoreline_width,
        foam_threshold: water.foam_threshold,
        foam_scroll: [water.foam_scroll.x, water.foam_scroll.y],
        normal_scroll_a: [water.normal_scroll_a.x, water.normal_scroll_a.y],
        normal_scroll_b: [water.normal_scroll_b.x, water.normal_scroll_b.y],
        wave_scale: water.wave_scale,
        foam_texture,
        ripple_texture,
    }))
}

fn authored_voxel_texture_hash(material: &RenderMaterialDescriptor) -> Option<&str> {
    material
        .voxel_surface
        .as_ref()
        .map(|surface| match &surface.mapping {
            VoxelSurfaceMappingDescriptor::Repeat {
                texture_content_hash,
                ..
            }
            | VoxelSurfaceMappingDescriptor::Atlas {
                texture_content_hash,
                ..
            } => texture_content_hash.as_str(),
        })
}

fn authored_voxel_texture_version(surface: &VoxelSurfaceDescriptor) -> u32 {
    match &surface.mapping {
        VoxelSurfaceMappingDescriptor::Repeat {
            texture_version, ..
        }
        | VoxelSurfaceMappingDescriptor::Atlas {
            texture_version, ..
        } => *texture_version,
    }
}

fn retarget_voxel_surface(surface: &mut VoxelSurfaceDescriptor, texture_id: &str) {
    match &mut surface.mapping {
        VoxelSurfaceMappingDescriptor::Repeat { texture, .. }
        | VoxelSurfaceMappingDescriptor::Atlas { texture, .. } => {
            *texture = texture_id.to_owned();
        }
    }
}

/// A material map's texture (its retained asset identity), or none for
/// reference 0. `linear` requires a texture opened with a linear colour
/// space, as data maps are.
fn map_texture(
    resources: &RenderResourceRegistry,
    reference: NativeRenderResourceReference,
    linear: bool,
    code: &'static str,
    message: &'static str,
) -> Result<Option<String>, CsharpEngineServicesError> {
    if reference.value == 0 {
        return Ok(None);
    }
    let resource = resources
        .get(reference.value)
        .filter(|resource| {
            resource.texture().is_some_and(|texture| {
                !linear
                    || texture.payload.as_ref().is_some_and(|payload| {
                        payload.color_space == render_model::TextureColorSpace::Linear
                    })
            })
        })
        .ok_or_else(|| CsharpEngineServicesError::new(code, message))?;
    Ok(Some(resource.asset_identity().to_owned()))
}

/// A material's normal map: a texture opened as linear data, or none for
/// reference 0.
fn normal_map_descriptor(
    resources: &RenderResourceRegistry,
    reference: NativeRenderResourceReference,
    scale: f32,
) -> Result<Option<render_model::MaterialNormalMapDescriptor>, CsharpEngineServicesError> {
    if reference.value == 0 {
        return Ok(None);
    }
    let resource = resources
        .get(reference.value)
        .filter(|resource| {
            resource
                .texture()
                .and_then(|texture| texture.payload.as_ref())
                .is_some_and(|payload| {
                    payload.color_space == render_model::TextureColorSpace::Linear
                })
        })
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_NORMAL_MAP",
                "a material's normal map must be a texture opened with TextureColorSpace.Linear",
            )
        })?;
    Ok(Some(render_model::MaterialNormalMapDescriptor {
        texture: resource.asset_identity().to_owned(),
        scale,
    }))
}

/// A shader resource request's keywords: space separated, as a sorted set,
/// so one set opens one resource whatever its order.
///
/// # Safety
/// The slice must be valid for this call.
unsafe fn shader_keywords(
    keywords: csharp_engine_abi::NativeUtf8Slice,
) -> Result<Vec<String>, CsharpEngineServicesError> {
    if keywords.len == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: the caller keeps the slice valid for this call.
    let text = unsafe { borrowed_utf8(keywords.bytes, keywords.len, "shader keywords")? };
    let mut keywords: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
    keywords.sort();
    keywords.dedup();
    Ok(keywords)
}

/// A material's product shader and its definition, or none for handle 0.
fn material_shader(
    resources: &RenderResourceRegistry,
    request: csharp_engine_abi::NativeMaterialShader,
) -> Result<
    Option<(
        render_model::MaterialShaderDescriptor,
        render_model::ShaderDescriptor,
    )>,
    CsharpEngineServicesError,
> {
    if request.shader.value == 0 {
        return Ok(None);
    }
    let shader = resources
        .get(request.shader.value)
        .and_then(|resource| resource.shader())
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_SHADER",
                "a material's shader must be a resource opened from a .wgsl file",
            )
        })?;
    let row = |value: csharp_engine_abi::NativeVec4| [value.x, value.y, value.z, value.w];
    let texture = |texture: csharp_engine_abi::NativeRenderResourceReference| {
        if texture.value == 0 {
            return Ok(None);
        }
        resources
            .get(texture.value)
            .and_then(CsharpRenderResource::texture)
            .map(|texture| Some(texture.id.clone()))
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_MATERIAL_SHADER",
                    "a material shader's textures must be texture resources",
                )
            })
    };
    let textures = [texture(request.texture_a)?, texture(request.texture_b)?];
    Ok(Some((
        render_model::MaterialShaderDescriptor {
            shader: shader.id.clone(),
            parameters: [
                row(request.parameter_0),
                row(request.parameter_1),
                row(request.parameter_2),
                row(request.parameter_3),
            ],
            textures,
        },
        shader,
    )))
}

/// Keep `shader` among the definitions published with the materials.
fn retain_shader_descriptor(
    shaders: &mut Vec<render_model::ShaderDescriptor>,
    shader: render_model::ShaderDescriptor,
) {
    if !shaders.iter().any(|retained| retained.id == shader.id) {
        shaders.push(shader);
    }
}

/// The texture repeat and offset, or none for one unshifted repeat; a zero
/// scale component repeats once.
fn texture_transform_descriptor(
    scale: NativeVec2,
    offset: NativeVec2,
) -> Option<render_model::MaterialTextureTransformDescriptor> {
    let once = |value: f32| if value == 0.0 { 1.0 } else { value };
    let transform = render_model::MaterialTextureTransformDescriptor {
        scale: [once(scale.x), once(scale.y)],
        offset: [offset.x, offset.y],
    };
    (transform.scale != [1.0, 1.0] || transform.offset != [0.0, 0.0]).then_some(transform)
}

/// Triplanar sampling at this sharpness, or none for 0.
fn triplanar_descriptor(sharpness: f32) -> Option<render_model::MaterialTriplanarDescriptor> {
    (sharpness != 0.0).then_some(render_model::MaterialTriplanarDescriptor { sharpness })
}

/// One slot's per-instance parameters from a product's factors: a base
/// colour, texture tint and emission each override the material's when asked.
fn mesh_material_parameters(
    factor: &NativeMeshMaterialFactors,
    code: &'static str,
) -> Result<MaterialInstanceParameters, CsharpEngineServicesError> {
    let base = factor.base_color;
    let tint = factor.texture_tint;
    let emissive = factor.emissive_factor;
    let value = MaterialInstanceParameters {
        base_color: factor
            .override_base_color
            .then_some([base.r, base.g, base.b, base.a]),
        texture_tint: if factor.override_texture_tint {
            [tint.r, tint.g, tint.b, tint.a]
        } else {
            [1.0; 4]
        },
        emission: factor
            .override_emission
            .then_some(MaterialInstanceEmission {
                color: [emissive.x, emissive.y, emissive.z],
                intensity: factor.emissive_strength,
            }),
    };
    value.validate().map_err(|_| {
        CsharpEngineServicesError::new(
            code,
            "base colour, texture tint and emissive factor channels are 0 to 1 and emissive strength is finite and not negative",
        )
    })?;
    Ok(value)
}

fn texture_descriptors_for_material(
    material: &RenderMaterialDescriptor,
    resources: &RenderResourceRegistry,
) -> Result<Vec<TextureDescriptor>, CsharpEngineServicesError> {
    material
        .textures()
        .map(|identity| {
            resources
                .iter()
                .find(|resource| resource.asset_identity() == identity)
                .and_then(CsharpRenderResource::texture)
                .cloned()
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_MATERIAL_TEXTURE",
                        "material texture descriptor is not retained",
                    )
                })
        })
        .collect()
}

fn retain_texture_descriptor(
    textures: &mut Vec<TextureDescriptor>,
    texture: TextureDescriptor,
) -> Result<(), CsharpEngineServicesError> {
    if let Some(retained) = textures.iter().find(|retained| retained.id == texture.id) {
        if retained != &texture {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_MATERIAL_TEXTURE",
                "retained texture identity resolved to different immutable facts",
            ));
        }
        return Ok(());
    }
    textures.push(texture);
    Ok(())
}

pub(crate) unsafe extern "C" fn open_render_resource_from_content(
    context: *mut c_void,
    request: *const NativeRenderResourceContentRequest,
    result: *mut NativeRenderResourceInfo,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        let request = unsafe { *request };
        match bridge
            .content_reference(request.content)
            .and_then(|content| {
                // SAFETY: the borrowed keywords are copied before the call returns.
                let keywords = unsafe { shader_keywords(request.shader_keywords)? };
                bridge.admit_resource(
                    content,
                    request.filter,
                    request.wrap,
                    request.color_space,
                    &keywords,
                )
            }) {
            Ok(value) => {
                unsafe { *result = value };
                ABI_OK
            }
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

pub(crate) unsafe extern "C" fn create_static_mesh_from_content_reference(
    context: *mut c_void,
    request: *const NativeStaticMeshContentReferenceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        let request = unsafe { *request };
        appearance_result(context, result, |bridge| {
            let content = bridge.content_reference(request.content)?;
            bridge.admit_static_mesh(content, request.color)
        })
    })
}

pub(crate) unsafe extern "C" fn read_animated_mesh_info(
    context: *mut c_void,
    resource: NativeRenderResourceHandle,
    result: *mut NativeAnimatedMeshInfo,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            let mesh = bridge
                .resource(resource.value)?
                .animated_mesh()
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_RESOURCE",
                        "resource is not an animated mesh",
                    )
                })?;
            let vector = |v: [f32; 3]| NativeVec3 {
                x: v[0],
                y: v[1],
                z: v[2],
            };
            Ok(NativeAnimatedMeshInfo {
                bounds_min: vector(mesh.bounds.min),
                bounds_max: vector(mesh.bounds.max),
                clip_count: mesh.clips.len() as u32,
                material_count: mesh.embedded_material_slots.len() as u32,
                joint_count: mesh.rig.as_ref().map_or(0, |rig| rig.joints.len() as u32),
            })
        })
    })
}

pub(crate) unsafe extern "C" fn read_animation_clips(
    context: *mut c_void,
    resource: NativeRenderResourceHandle,
    result: *mut NativeAnimationClipInfoResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            let clips = bridge
                .resource(resource.value)?
                .animated_mesh()
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_ANIMATION_RESOURCE",
                        "resource is not an animated mesh",
                    )
                })?
                .clips
                .clone();
            let utf8 = |value: &str| NativeUtf8Slice {
                bytes: value.as_ptr(),
                len: value.len(),
            };
            let readout = clips
                .iter()
                .map(|clip| NativeAnimationClipInfo {
                    id: utf8(&clip.id),
                    name: utf8(clip.name.as_deref().unwrap_or(&clip.id)),
                    duration_seconds: clip.duration_seconds.unwrap_or_default(),
                    has_duration: clip.duration_seconds.is_some(),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let result = NativeAnimationClipInfoResult {
                clips: readout.as_ptr(),
                clips_len: readout.len(),
            };
            bridge.borrowed.hold((clips, readout));
            Ok(result)
        })
    })
}

// Content import is a recoverable editing operation. A rejected source has not
// staged a resource and must not poison the surrounding product callback.
fn animation_content_result(
    context: *mut c_void,
    request: *const NativeAnimationContentRequest,
    result: *mut NativeRenderResourceHandle,
    receipt: *mut NativeOperationErrorReceipt,
    clip_pack: bool,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    let outcome = bridge
        .content_reference(unsafe { (*request).content })
        .and_then(|content| {
            if clip_pack {
                bridge.admit_animation_clip_pack(content)
            } else {
                bridge.admit_animated_mesh(content)
            }
        });
    match outcome {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn open_animated_mesh_from_content(
    context: *mut c_void,
    request: *const NativeAnimationContentRequest,
    result: *mut NativeRenderResourceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    animation_content_result(context, request, result, receipt, false)
}

pub(crate) unsafe extern "C" fn open_animation_clip_pack_from_content(
    context: *mut c_void,
    request: *const NativeAnimationContentRequest,
    result: *mut NativeRenderResourceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    animation_content_result(context, request, result, receipt, true)
}

pub(crate) unsafe extern "C" fn read_texture_info(
    context: *mut c_void,
    resource: NativeRenderResourceHandle,
    result: *mut NativeTextureResourceInfo,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || result.is_null() {
            return 0;
        }
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        let Some(texture) = bridge
            .resource(resource.value)
            .ok()
            .and_then(CsharpRenderResource::texture)
        else {
            return 0;
        };
        unsafe {
            *result = NativeTextureResourceInfo {
                width: texture.width,
                height: texture.height,
            };
        }
        ABI_OK
    })
}

pub(crate) unsafe extern "C" fn publish_appearance_changes(
    context: *mut c_void,
    request: *const NativeAppearanceChangesRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if context.is_null() || request.is_null() {
            return 0;
        }
        // SAFETY: context points at a box retained by `CsharpProductRuntime`.
        let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
        // SAFETY: the request and its spans are borrowed for this synchronous call.
        let request = unsafe { &*request };
        let outcome = (|| {
            let facts = unsafe {
                borrowed_slice(request.upserts, request.upserts_len, "appearance facts")
            }?;
            let removals =
                unsafe { borrowed_slice(request.removals, request.removals_len, "removals") }?;
            let rows = unsafe {
                borrowed_slice(
                    request.attachments,
                    request.attachments_len,
                    "joint attachments",
                )
            }?;
            let mut attachments = BTreeMap::new();
            for row in rows {
                let joint = unsafe { borrowed_utf8(row.joint.bytes, row.joint.len, "joint name") }?;
                if joint.trim().is_empty()
                    || attachments.insert(row.child_object_id, joint).is_some()
                {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_JOINT_ATTACHMENT",
                        format!(
                            "empty joint or duplicate attachment for child {}",
                            row.child_object_id
                        ),
                    ));
                }
            }
            bridge.stage_changes(facts, Some(removals), &attachments)
        })();
        match outcome {
            Ok(()) => ABI_OK,
            Err(error) => {
                bridge.operation_error = Some(error);
                0
            }
        }
    })
}

/// Runs one graphics operation and returns its refusal through the receipt.
/// A refusal is operation-local: the operation leaves staged state unchanged,
/// and the generated caller owns the resulting exception. Nothing latches.
pub(crate) fn appearance_operation(
    context: *mut c_void,
    receipt: *mut NativeOperationErrorReceipt,
    call: impl FnOnce() -> i32,
) -> i32 {
    if !receipt.is_null() {
        unsafe { *receipt = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() }.operation_error = None;
    let status = call();
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    let operation_error = bridge.operation_error.take();
    if status != ABI_OK && !receipt.is_null() {
        let error = operation_error
            .as_ref()
            .map(|error| CsharpEngineServicesError::new(error.code(), error.detail()))
            .unwrap_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_GRAPHICS_OPERATION",
                    "graphics operation refused its supplied handle or request",
                )
            });
        bridge.operation_diagnostics.retain(&error, receipt);
    }
    status
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::render_resources::tests::external_image_glb;
    use sha2::Digest;

    fn inert_particle_collision() -> NativePresentationParticleCollision {
        NativePresentationParticleCollision {
            radius: 0.0,
            restitution: 0.0,
            friction: 0.0,
            maximum_impacts: 0,
            sleep_speed: 0.0,
            limit_behavior: NativePresentationParticleCollisionLimitBehavior::Sleep,
        }
    }

    pub(crate) const RGBA_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 0, 68,
        255, 25, 26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
        130,
    ];

    pub(crate) fn resource_request(path: &'static str) -> NativeRenderResourceRequest {
        NativeRenderResourceRequest {
            shader_keywords: csharp_engine_abi::NativeUtf8Slice {
                bytes: std::ptr::null(),
                len: 0,
            },
            path: NativeUtf8Slice {
                bytes: path.as_ptr(),
                len: path.len(),
            },
            filter: NativeTextureFilter::Nearest,
            wrap: NativeTextureWrap::Clamp,
            color_space: NativeTextureColorSpace::Srgb,
        }
    }

    pub(super) fn primitive_request() -> NativePrimitiveAppearanceRequest {
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

    #[test]
    fn refusal_returns_diagnostic_without_latching_the_call() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let context = (&mut bridge as *mut RuntimeAppearanceBridge).cast();
        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        let status = appearance_operation(context, &mut receipt, || {
            unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() }.record_operation_error(
                CsharpEngineServicesError::new("REFUSED", "invalid particle"),
            );
            0
        });
        assert_eq!(status, 0);
        assert_eq!(receipt.diagnostics_len, 1);
        let diagnostic = unsafe { &*receipt.diagnostics };
        assert_eq!(
            unsafe { borrowed_utf8(diagnostic.code.bytes, diagnostic.code.len, "code") }.unwrap(),
            "REFUSED"
        );
        assert!(bridge.operation_error.is_none());
        bridge.end_call();
    }

    #[test]
    fn joint_change_rejects_missing_joint_without_poisoning_or_mutating_call() {
        const BODY: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut bridge = RuntimeAppearanceBridge::new(
            RuntimeAppearanceCatalog::default(),
            BTreeMap::from([("body.glb".to_owned(), Arc::from(BODY))]),
        );
        bridge.begin_call();
        let text = |s: &str| NativeUtf8Slice {
            bytes: s.as_ptr(),
            len: s.len(),
        };
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: text("body.glb"),
            })
            .unwrap();
        let body = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .unwrap();
        let child = bridge.create_primitive(primitive_request()).unwrap();
        let body_fact = appearance_fact(body);
        let mut child_fact = appearance_fact(child);
        child_fact.object_id = 8;
        child_fact.has_parent_object = true;
        child_fact.parent_object_id = body_fact.object_id;
        let facts = [body_fact, child_fact];
        let attachments = [NativeMeshJointAttachment {
            child_object_id: 8,
            joint: text("RightHand"),
        }];
        let mut request = NativeAppearanceChangesRequest {
            upserts: facts.as_ptr(),
            upserts_len: facts.len(),
            removals: std::ptr::null(),
            removals_len: 0,
            attachments: attachments.as_ptr(),
            attachments_len: 1,
        };
        let context = (&mut bridge as *mut RuntimeAppearanceBridge).cast();
        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe { publish_appearance_changes(context, &request, &mut receipt) },
            ABI_OK
        );
        let outputs = bridge.staged_ref().unwrap().outputs.len();
        let missing = [NativeMeshJointAttachment {
            child_object_id: 8,
            joint: text("MissingHand"),
        }];
        request.attachments = missing.as_ptr();
        assert_eq!(
            unsafe { publish_appearance_changes(context, &request, &mut receipt) },
            0
        );
        let message = unsafe {
            borrowed_utf8(
                (*receipt.diagnostics).message.bytes,
                (*receipt.diagnostics).message.len,
                "diagnostic",
            )
        }
        .unwrap();
        assert!(message.contains("MissingHand"));
        assert!(message.contains("MissingJoint"));
        assert_eq!(bridge.staged_ref().unwrap().outputs.len(), outputs);
        assert!(bridge.operation_error.is_none());
        // A snapshot cannot name joints, so it keeps the attachment.
        unsafe { bridge.stage_snapshot(facts.as_ptr(), facts.len()) }.unwrap();
        let call = bridge.take_staged_call();
        let mut world = render_presentation::PresentationWorld::default();
        for output in call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        assert!(world.snapshot().frame.ops.iter().any(|op| matches!(op, RenderDiff::SetParentJoint { joint: Some(joint), .. } if joint == "RightHand")));
    }

    #[test]
    fn product_call_snapshot_shares_immutable_resource_bodies() {
        let bytes: Arc<[u8]> = Arc::from(vec![0x5a; 8 * 1024 * 1024]);
        let pointer = bytes.as_ptr();
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge
            .state
            .render_resources
            .stage(CsharpRenderResource::test_mesh(bytes), [])
            .unwrap();

        bridge.begin_call();
        let staged = bridge.staged_ref().expect("product call stage");
        let resource = staged.state.render_resources.get(1).unwrap();
        assert_eq!(resource.bytes().as_ptr(), pointer);
        assert_eq!(resource.bytes().len(), 8 * 1024 * 1024);
    }

    #[test]
    #[ignore = "run by scripts/run-performance-regression.sh"]
    fn performance_probe_appearance_call_stage() {
        const ITERATIONS: usize = 10_000;
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge
            .state
            .render_resources
            .stage(
                CsharpRenderResource::test_mesh(Arc::from(vec![0x5a; 8 * 1024 * 1024])),
                [],
            )
            .unwrap();
        for _ in 0..100 {
            bridge.begin_call();
            bridge.end_call();
        }
        let allocated_pointer = bridge
            .state
            .render_resources
            .get(1)
            .unwrap()
            .bytes()
            .as_ptr();
        let mut durations = Vec::with_capacity(ITERATIONS);
        for _ in 0..ITERATIONS {
            let started = std::time::Instant::now();
            bridge.begin_call();
            assert_eq!(
                bridge
                    .staged_ref()
                    .unwrap()
                    .state
                    .render_resources
                    .get(1)
                    .unwrap()
                    .bytes()
                    .as_ptr(),
                allocated_pointer,
            );
            bridge.end_call();
            durations.push(started.elapsed().as_nanos());
        }
        durations.sort_unstable();
        let median = durations[(durations.len() - 1) / 2] as f64 / 1_000_000.0;
        let p95 =
            durations[((durations.len() - 1) as f64 * 0.95).round() as usize] as f64 / 1_000_000.0;
        let mean = durations.iter().sum::<u128>() as f64 / durations.len() as f64 / 1_000_000.0;
        println!(
            "RUSTY_PERF {}",
            serde_json::json!({
                "schemaVersion": 1,
                "lane": "rust-appearance-call-stage",
                "iterations": ITERATIONS,
                "unit": "milliseconds",
                "resourceBytes": 8 * 1024 * 1024,
                "minimum": durations[0] as f64 / 1_000_000.0,
                "median": median,
                "p95": p95,
                "maximum": durations[durations.len() - 1] as f64 / 1_000_000.0,
                "mean": mean,
            })
        );
    }

    #[test]
    fn emission_and_occlusion_maps_and_unlit_reach_the_material_descriptor() {
        let mut content = BTreeMap::new();
        content.insert("glow.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        bridge.begin_call();
        let colour = bridge.open_resource(&resource_request("glow.png")).unwrap();
        let mut data_request = resource_request("glow.png");
        data_request.color_space = NativeTextureColorSpace::Linear;
        let data = bridge.open_resource(&data_request).unwrap();
        let resources = &bridge.staged_ref().unwrap().state.render_resources;
        let white = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let request =
            |unlit: bool,
             emission_map: NativeRenderResourceHandle,
             occlusion_map: NativeRenderResourceHandle| NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                shader: Default::default(),
                triplanar_sharpness: 0.0,
                color: white,
                texture: NativeRenderResourceReference::default(),
                roughness: 0.8,
                texture_tint: white,
                emission_color: NativeVec3 {
                    x: 1.0,
                    y: 0.5,
                    z: 0.0,
                },
                emission_intensity: 3.0,
                double_sided: false,
                alpha_mode: NativeMaterialAlphaMode::Opaque,
                alpha_cutoff: 0.5,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                unlit,
                emission_map: NativeRenderResourceReference {
                    value: emission_map.value,
                },
                occlusion_map: NativeRenderResourceReference {
                    value: occlusion_map.value,
                },
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            };
        let none = NativeRenderResourceHandle::default();
        let plain = material_descriptor(
            "material/plain".to_owned(),
            request(false, none, none),
            resources,
        )
        .unwrap();
        assert!(!plain.unlit && plain.emission_map.is_none() && plain.occlusion_map.is_none());

        let mapped = material_descriptor(
            "material/mapped".to_owned(),
            request(true, colour.handle, data.handle),
            resources,
        )
        .unwrap();
        assert!(mapped.unlit);
        let emission = mapped.emission_map.as_ref().expect("emission map");
        let occlusion = mapped.occlusion_map.as_ref().expect("occlusion map");
        assert!(
            occlusion.texture.ends_with("-linear"),
            "{}",
            occlusion.texture
        );
        assert_ne!(
            emission.texture, occlusion.texture,
            "one PNG, two colour spaces"
        );
        let textures = texture_descriptors_for_material(&mapped, resources).unwrap();
        assert_eq!(
            textures.len(),
            2,
            "both maps are retained with the material"
        );

        let refused = material_descriptor(
            "material/colour-occlusion".to_owned(),
            request(false, none, colour.handle),
            resources,
        )
        .unwrap_err();
        assert_eq!(
            refused.code(),
            "CSHARP_MATERIAL_OCCLUSION_MAP",
            "occlusion maps are data"
        );
        let refused = material_descriptor(
            "material/unknown-emission".to_owned(),
            request(false, NativeRenderResourceHandle { value: 999 }, none),
            resources,
        )
        .unwrap_err();
        assert_eq!(refused.code(), "CSHARP_MATERIAL_EMISSION_MAP");

        // A packed occlusion, roughness, metalness map takes the occlusion
        // slot with the strength; 0 means full strength; it excludes a plain
        // occlusion map and must be data too.
        let orm =
            |occlusion_map: NativeRenderResourceHandle, strength: f32| NativeMaterialRequest {
                orm_map: NativeRenderResourceReference {
                    value: data.handle.value,
                },
                occlusion_strength: strength,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
                ..request(false, none, occlusion_map)
            };
        let packed =
            material_descriptor("material/orm".to_owned(), orm(none, 0.5), resources).unwrap();
        let map = packed.occlusion_map.as_ref().expect("orm map");
        assert!(map.roughness_metalness && map.strength == 0.5 && map.texture.ends_with("-linear"));
        let full =
            material_descriptor("material/orm-full".to_owned(), orm(none, 0.0), resources).unwrap();
        assert_eq!(full.occlusion_map.as_ref().unwrap().strength, 1.0);
        let refused =
            material_descriptor("material/both".to_owned(), orm(data.handle, 1.0), resources)
                .unwrap_err();
        assert_eq!(refused.code(), "CSHARP_MATERIAL_OCCLUSION_MAP");
        let refused = material_descriptor(
            "material/orm-colour".to_owned(),
            NativeMaterialRequest {
                orm_map: NativeRenderResourceReference {
                    value: colour.handle.value,
                },
                ..request(false, none, none)
            },
            resources,
        )
        .unwrap_err();
        assert_eq!(refused.code(), "CSHARP_MATERIAL_ORM_MAP");
    }

    #[test]
    fn static_mesh_instances_take_per_slot_factors_over_one_material() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let positions = [
            NativeVec3::default(),
            NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        let normals = [NativeVec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }; 3];
        let indices = [0_u32, 1, 2];
        let groups = [NativeMeshGroup {
            material_slot: 0,
            start: 0,
            count: 3,
        }];
        let white = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let material = bridge
            .create_material(NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                shader: Default::default(),
                triplanar_sharpness: 0.0,
                color: white,
                texture: NativeRenderResourceReference::default(),
                roughness: 0.7,
                texture_tint: white,
                emission_color: NativeVec3::default(),
                emission_intensity: 0.0,
                double_sided: false,
                alpha_mode: NativeMaterialAlphaMode::Opaque,
                alpha_cutoff: 0.5,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                unlit: false,
                emission_map: NativeRenderResourceReference::default(),
                occlusion_map: NativeRenderResourceReference::default(),
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            })
            .unwrap();
        let bindings = [NativeMeshMaterialBinding {
            material_slot: 0,
            material,
        }];
        let request = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: std::ptr::null(),
            colors_len: 0,
            indices: indices.as_ptr(),
            indices_len: indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
        };
        let resource =
            unsafe { crate::render_resources::create_generated_mesh(&mut bridge, &request) }
                .unwrap();
        let tinted = bridge.create_mesh_appearance(resource).unwrap();
        let plain = bridge.create_mesh_appearance(resource).unwrap();

        let tint = |slot, r: f32| NativeMeshMaterialFactors {
            material_slot: slot,
            override_base_color: false,
            base_color: NativeColor::default(),
            override_emission: false,
            emissive_factor: NativeVec3::default(),
            emissive_strength: 0.0,
            override_texture_tint: true,
            texture_tint: NativeColor {
                r,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
        };
        let update = |bridge: &mut RuntimeAppearanceBridge,
                      appearance,
                      factors: &[NativeMeshMaterialFactors]| unsafe {
            bridge.update_static_mesh_material_factors(&NativeStaticMeshMaterialFactorsRequest {
                appearance,
                factors: factors.as_ptr(),
                factors_len: factors.len(),
            })
        };
        update(&mut bridge, tinted, &[tint(0, 1.0)]).expect("slot 0 factors");
        for (factors, code) in [
            (vec![tint(0, 1.5)], "CSHARP_STATIC_MESH_FACTORS"),
            (vec![tint(0, 0.5), tint(0, 0.5)], "CSHARP_STATIC_MESH_SLOT"),
            (vec![tint(70_000, 0.5)], "CSHARP_STATIC_MESH_SLOT"),
        ] {
            assert_eq!(
                update(&mut bridge, tinted, &factors)
                    .expect_err("refused")
                    .code(),
                code
            );
        }
        let state = &bridge.staged_ref().unwrap().state;
        let parameters = |handle: NativeAppearanceHandle| match state
            .projector
            .appearance(&state.appearances[&handle.value])
        {
            Some(Appearance::StaticMesh {
                material_parameters,
                ..
            }) => material_parameters.clone(),
            _ => panic!("static mesh appearance"),
        };
        assert_eq!(parameters(tinted)[&0].texture_tint, [1.0, 0.5, 0.5, 1.0]);
        assert_eq!(
            parameters(tinted)[&0].base_color,
            None,
            "the material's colour stays"
        );
        assert!(
            parameters(plain).is_empty(),
            "appearances of one mesh each carry their own factors"
        );

        let facts = [appearance_fact(tinted), {
            let mut fact = appearance_fact(plain);
            fact.object_id = 8;
            fact
        }];
        unsafe { bridge.stage_snapshot(facts.as_ptr(), facts.len()) }.unwrap();
        let call = bridge.take_staged_call();
        let ops = call.render_ops();
        let parameter_ops: Vec<_> = ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    render_model::RenderDiff::SetMaterialInstanceParameters { .. }
                )
            })
            .collect();
        assert_eq!(
            parameter_ops.len(),
            1,
            "one instance carries factors: {ops:?}"
        );
        assert!(matches!(
            parameter_ops[0],
            render_model::RenderDiff::SetMaterialInstanceParameters {
                slot: 0,
                parameters: Some(parameters),
                ..
            } if parameters.texture_tint == [1.0, 0.5, 0.5, 1.0]
        ));
    }

    fn appearance_fact(appearance: NativeAppearanceHandle) -> NativeAppearanceFact {
        NativeAppearanceFact {
            object_id: 7,
            has_parent_object: false,
            parent_object_id: 0,
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
            shadow_casting: Default::default(),
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
                shadow_resolution: 0,
                shadow_priority: 0,
                shadow_soft: false,
                ground_color: Default::default(),
            },
        }
    }

    #[test]
    fn mesh_partition_transfers_parts_and_retains_materials_independently() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let color = NativeColor {
            r: 1.0,
            g: 0.4,
            b: 0.1,
            a: 1.0,
        };
        let material = bridge
            .create_material(NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                shader: Default::default(),
                triplanar_sharpness: 0.0,
                color,
                texture: NativeRenderResourceReference { value: 0 },
                roughness: 0.7,
                texture_tint: color,
                emission_color: NativeVec3::default(),
                emission_intensity: 0.0,
                double_sided: true,
                alpha_mode: NativeMaterialAlphaMode::Mask,
                alpha_cutoff: 0.4,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                unlit: false,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            })
            .unwrap();
        let positions = [
            NativeVec3::default(),
            NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        let normals = [NativeVec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }; 3];
        let colors = [
            NativeColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            NativeColor {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            NativeColor {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
        ];
        let indices = [0, 1, 2];
        let groups = [NativeMeshGroup {
            material_slot: 3,
            start: 0,
            count: 3,
        }];
        let bindings = [NativeMeshMaterialBinding {
            material_slot: 3,
            material,
        }];
        let request = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: colors.as_ptr(),
            colors_len: colors.len(),
            indices: indices.as_ptr(),
            indices_len: indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
        };
        let resource =
            unsafe { crate::render_resources::create_generated_mesh(&mut bridge, &request) }
                .unwrap();
        let context = (&mut bridge as *mut RuntimeAppearanceBridge).cast();
        let mut partition = NativeMeshPartitionHandle::default();
        assert_eq!(
            unsafe {
                crate::render_resources::partition_mesh(
                    context,
                    NativeMeshPartitionRequest {
                        source: resource,
                        origin: NativeVec3::default(),
                        cell_size: NativeVec3 {
                            x: 1.0,
                            y: 1.0,
                            z: 1.0,
                        },
                    },
                    &mut partition,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut readout = NativeMeshPartitionReadout::default();
        assert_eq!(
            unsafe {
                crate::render_resources::read_mesh_partition(
                    context,
                    partition,
                    &mut readout,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(readout.part_count, 1);
        bridge.destroy_mesh_resource(resource).unwrap();
        assert!(
            bridge.destroy_material(material).is_err(),
            "prepared parts retain materials without the source"
        );
        let part_request = NativeMeshPartitionPartRequest {
            partition,
            index: 0,
        };
        let take_part = |part: &mut NativeMeshResourceHandle| unsafe {
            crate::render_resources::take_mesh_partition_part(
                context,
                part_request,
                part,
                std::ptr::null_mut(),
            )
        };
        let mut part = NativeMeshResourceHandle::default();
        assert_eq!(take_part(&mut part), ABI_OK);
        assert_eq!(
            take_part(&mut NativeMeshResourceHandle::default()),
            0,
            "ownership transfers exactly once"
        );
        assert_eq!(
            unsafe {
                crate::render_resources::destroy_mesh_partition(
                    context,
                    partition,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert!(
            bridge.destroy_material(material).is_err(),
            "taken mesh retains its material independently"
        );
        let appearance = bridge.create_mesh_appearance(part).unwrap();
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let call = bridge.take_staged_call();
        let mut world = render_presentation::PresentationWorld::default();
        for output in &call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        bridge.commit(call);
        bridge.begin_call();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        bridge.destroy_appearance(appearance).unwrap();
        bridge.destroy_mesh_resource(part).unwrap();
        bridge.destroy_material(material).unwrap();
        let mut readout = NativeMeshPartitionReadout::default();
        assert_eq!(
            unsafe {
                crate::render_resources::read_mesh_partition(
                    context,
                    partition,
                    &mut readout,
                    std::ptr::null_mut(),
                )
            },
            0,
            "the destroyed partition is gone"
        );
    }

    #[test]
    fn generated_mesh_appearance_projects_copied_streams_and_releases_exactly() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let color = NativeColor {
            r: 1.0,
            g: 0.4,
            b: 0.1,
            a: 1.0,
        };
        let material = bridge
            .create_material(NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                shader: Default::default(),
                triplanar_sharpness: 0.0,
                color,
                texture: NativeRenderResourceReference { value: 0 },
                roughness: 0.7,
                texture_tint: color,
                emission_color: NativeVec3::default(),
                emission_intensity: 0.0,
                double_sided: true,
                alpha_mode: NativeMaterialAlphaMode::Mask,
                alpha_cutoff: 0.4,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                unlit: false,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            })
            .unwrap();
        let mut positions = [
            NativeVec3::default(),
            NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        let normals = [NativeVec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }; 3];
        let mut colors = [
            NativeColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            NativeColor {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            NativeColor {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
        ];
        let mut indices = [0, 1, 2];
        let groups = [NativeMeshGroup {
            material_slot: 3,
            start: 0,
            count: 3,
        }];
        let bindings = [NativeMeshMaterialBinding {
            material_slot: 3,
            material,
        }];
        let request = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: std::ptr::null(),
            uvs_len: 0,
            colors: colors.as_ptr(),
            colors_len: colors.len(),
            indices: indices.as_ptr(),
            indices_len: indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
        };
        let resource =
            unsafe { crate::render_resources::create_generated_mesh(&mut bridge, &request) }
                .unwrap();
        positions[1].x = 99.0;
        colors[0].r = 0.25;
        indices[1] = 99;
        assert_eq!(positions[1].x, 99.0);
        assert_eq!(colors[0].r, 0.25);
        assert_eq!(indices[1], 99);
        let appearance = bridge.create_mesh_appearance(resource).unwrap();
        assert!(bridge.destroy_mesh_resource(resource).is_err());
        assert!(bridge.destroy_material(material).is_err());
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let call = bridge.take_staged_call();
        let mut world = render_presentation::PresentationWorld::default();
        for output in &call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        let baseline = world.snapshot();
        let encoded = serde_json::to_value(&baseline.frame).unwrap();
        assert!(encoded.to_string().contains("mesh/runtime-1"));
        assert!(encoded.to_string().contains("mask"));
        assert!(matches!(
            call.state.projector.clone().resources_mut().materials[0].alpha_mode,
            MaterialAlphaModeDescriptor::Mask { cutoff } if cutoff == 0.4
        ));
        let asset = &call.state.projector.clone().resources_mut().static_meshes[0].clone();
        match &asset.payload.source {
            MeshPayloadSource::Inline {
                positions,
                colors,
                indices,
                ..
            } => {
                assert_eq!(positions[3], 1.0);
                assert_eq!(
                    colors.as_deref(),
                    Some(&[1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0][..])
                );
                assert_eq!(indices, &[0, 1, 2]);
            }
            _ => panic!("generated stream must be retained and reconstructible"),
        }
        bridge.commit(call);
        bridge.begin_call();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        bridge.destroy_appearance(appearance).unwrap();
        bridge.destroy_mesh_resource(resource).unwrap();
        bridge.destroy_mesh_resource(resource).unwrap();
        bridge.destroy_material(material).unwrap();
        assert!(bridge.create_mesh_appearance(resource).is_err());
        let removal = bridge.take_staged_call();
        let mut releases = 0;
        for output in &removal.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                releases += frame
                    .ops
                    .iter()
                    .filter(|op| matches!(op, RenderDiff::ReleaseStaticMesh { .. }))
                    .count();
                world.apply(frame.clone()).unwrap();
            }
        }
        assert_eq!(releases, 1);
        assert_eq!(removal.state.generated_meshes.len(), 0);
        assert!(!serde_json::to_string(&world.snapshot().frame)
            .unwrap()
            .contains("mesh/runtime-1"));
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
    fn line_primitive_uses_existing_retained_geometry_and_snapshot_lifecycle() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let line = bridge
            .create_primitive(NativePrimitiveAppearanceRequest {
                geometry: NativePrimitiveGeometry::Line,
                ..primitive_request()
            })
            .unwrap();
        let fact = appearance_fact(line);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        assert!(bridge.destroy_appearance(line).is_err());
        let staged = bridge.take_staged_call();
        assert!(matches!(
            staged.render_ops().as_slice(),
            [render_model::RenderDiff::Create {
                node: render_model::RenderNode {
                    geometry: Geometry::Line {
                        a: [0.0, 0.0, 0.0],
                        b: [0.0, 1.0, 0.0]
                    },
                    ..
                },
                ..
            }]
        ));
    }

    #[test]
    fn appearance_snapshot_copies_parent_identity_into_the_retained_graph() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let parent = bridge
            .create_primitive(NativePrimitiveAppearanceRequest {
                geometry: NativePrimitiveGeometry::Group,
                ..primitive_request()
            })
            .unwrap();
        let child = bridge.create_primitive(primitive_request()).unwrap();
        let parent_fact = appearance_fact(parent);
        let mut child_fact = appearance_fact(child);
        child_fact.object_id = 8;
        child_fact.has_parent_object = true;
        child_fact.parent_object_id = parent_fact.object_id;

        let facts = [child_fact, parent_fact];
        unsafe { bridge.stage_snapshot(facts.as_ptr(), facts.len()) }
            .expect("hierarchical snapshot");
        let staged = bridge.take_staged_call();
        assert!(matches!(
            staged.render_ops().as_slice(),
            [
                render_model::RenderDiff::Create {
                    parent: None,
                    node: render_model::RenderNode {
                        geometry: Geometry::Group,
                        ..
                    },
                    ..
                },
                render_model::RenderDiff::Create {
                    parent: Some(_),
                    ..
                },
            ]
        ));
    }

    fn ghost_plate_request(source_object_id: u64) -> NativeCreateGhostPlatePresentationRequest {
        NativeCreateGhostPlatePresentationRequest {
            source_object_id,
            placement: NativeGhostPlatePlacement {
                transform: NativeTransform {
                    translation: NativeVec3 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
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
                width: 1.0,
                height: 1.0,
            },
            capture: NativeGhostPlateCaptureSettings {
                resolution: 64,
                azimuth_degrees: 0.0,
                elevation_degrees: 10.0,
                near: 0.1,
                far: 20.0,
                field_of_view_degrees: 55.0,
                lighting: NativeGhostPlateCaptureLighting {
                    mode: NativeGhostPlateCaptureLightingMode::Isolated,
                    ambient_color: NativeVec3 {
                        x: 0.25,
                        y: 0.25,
                        z: 0.25,
                    },
                    ambient_intensity: 0.5,
                    key_direction: NativeVec3 {
                        x: 0.5,
                        y: 1.0,
                        z: 0.25,
                    },
                    key_color: NativeVec3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                    key_intensity: 1.0,
                    fill_direction: NativeVec3 {
                        x: -0.5,
                        y: 0.25,
                        z: -1.0,
                    },
                    fill_color: NativeVec3 {
                        x: 0.5,
                        y: 0.5,
                        z: 0.5,
                    },
                    fill_intensity: 0.25,
                },
            },
            config: NativeGhostPlateConfig {
                depth_retention: 0.5,
                anchor_policy: NativeGhostPlateAnchorPolicy::BoundsCenter,
                anchor_value: 0.5,
                plate_mapping: NativeGhostPlateMapping::PlateLocked,
                shell_mode: NativeGhostPlateShellMode::WholeMesh,
                shell_depth_epsilon: 0.01,
                sector_count: 4,
                sector_hysteresis_degrees: 5.0,
            },
        }
    }

    #[test]
    fn ghost_plate_is_product_id_owned_and_preserves_live_state_on_failed_update() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let appearance = bridge.create_primitive(primitive_request()).unwrap();
        let source = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&source, 1) }.expect("source snapshot");

        let request = ghost_plate_request(source.object_id);
        let plate = bridge
            .presentation_create_ghost_plate(request)
            .expect("create ghost plate from product object id");
        let initial = bridge
            .presentation_read_ghost_plate(plate)
            .expect("initial readout");
        assert_eq!(initial.source_object_id, source.object_id);
        assert!(initial.source_present);
        assert!(!initial.has_renderer_observation);
        assert_eq!(initial.config.sector_count, 4);

        let mut update = request.config;
        update.sector_count = 8;
        bridge
            .presentation_update_ghost_plate(NativeUpdateGhostPlatePresentationRequest {
                presentation: plate,
                placement: request.placement,
                config: update,
            })
            .expect("hard-snap ghost update");
        bridge
            .presentation_recapture_ghost_plate(NativeRecaptureGhostPlatePresentationRequest {
                presentation: plate,
                capture: NativeGhostPlateCaptureSettings {
                    azimuth_degrees: 45.0,
                    ..request.capture
                },
            })
            .expect("recapture ghost plate");
        assert_eq!(
            bridge
                .presentation_read_ghost_plate(plate)
                .unwrap()
                .config
                .sector_count,
            8
        );

        update.sector_count = 0;
        assert!(bridge
            .presentation_update_ghost_plate(NativeUpdateGhostPlatePresentationRequest {
                presentation: plate,
                placement: request.placement,
                config: update,
            })
            .is_err());
        assert_eq!(
            bridge
                .presentation_read_ghost_plate(plate)
                .unwrap()
                .config
                .sector_count,
            8,
            "a rejected replacement leaves the live presentation intact"
        );

        let call = bridge.take_staged_call();
        bridge.commit(call);
        bridge.begin_call();
        let error = unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap_err();
        assert_eq!(error.code(), "CSHARP_GHOST_PLATE_SNAPSHOT_ORDER");
        bridge
            .presentation_destroy_ghost_plate(plate)
            .expect("destroy before source removal");
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }
            .expect("source removal is ordered after ghost disposal");
    }

    #[test]
    fn ghost_plate_realization_snapshot_replaces_active_observations_with_empty() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.ingest_ghost_plate_realization(
            false,
            [GhostPlateRealizationFact {
                handle: 9,
                source_matches: true,
                current_sector: 2,
                local_angular_offset_degrees: Some(12.0),
                fallback_active: false,
                fallback_reason: NativeGhostPlateFallbackReason::None,
                limitation_mask: NativeGhostPlateLimitationMask::SingleCaptureViewProfile,
                preparation_cpu_milliseconds: Some(1.0),
                capture_cpu_submission_milliseconds: Some(2.0),
                retained_sector_count: 4,
                retained_mesh_count: 1,
                retained_material_count: 1,
                retained_borrowed_texture_count: 0,
            }],
        );
        assert!(bridge.ghost_plate_realization.contains_key(&9));
        bridge.ingest_ghost_plate_realization(false, []);
        assert!(bridge.ghost_plate_realization.is_empty());
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
        let staged = bridge.take_staged_call();
        assert_eq!(staged.state.projector.retained_objects(), 1);
        assert_eq!(staged.state.projector.retained_lights(), 1);
        assert!(matches!(
            staged.render_ops().as_slice(),
            [
                render_model::RenderDiff::Create { .. },
                render_model::RenderDiff::CreateLight { .. }
            ]
        ));
        bridge.commit(staged);

        bridge.begin_call();
        let mut replacement = point_light_request(91, Some(7));
        replacement.descriptor.intensity = 3.0;
        bridge
            .update_light(NativeLightUpdateRequest { light, replacement })
            .unwrap();
        let staged = bridge.take_staged_call();
        assert!(matches!(
            staged.render_ops().as_slice(),
            [render_model::RenderDiff::UpdateLight { .. }]
        ));
        bridge.commit(staged);
    }

    #[test]
    fn a_parented_light_shows_while_its_parent_is_in_the_published_scene() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let appearance = bridge.create_primitive(primitive_request()).unwrap();
        // Created before the publish that first shows its parent: it waits.
        let light = bridge
            .create_light(point_light_request(91, Some(7)))
            .expect("a light may name a parent that is not published yet");
        assert_eq!(bridge.read_light(light).unwrap().parent_object_id, 7);
        let unparented = bridge
            .create_light(point_light_request(92, None))
            .expect("an unparented light shows at once");
        let mut invalid = point_light_request(93, Some(7));
        invalid.descriptor.intensity = -1.0;
        assert_eq!(
            bridge.create_light(invalid).unwrap_err().code(),
            "CSHARP_LIGHT_DESCRIPTOR",
            "a waiting light is still validated when created"
        );
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let staged = bridge.take_staged_call();
        assert_eq!(staged.state.projector.retained_lights(), 2);
        assert!(staged.state.pending_lights.is_empty());
        bridge.commit(staged);

        // A snapshot without the parent takes its light out of the scene; a
        // later one with it shows the light again.
        bridge.begin_call();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }
            .expect("removing a parent does not need its lights disposed first");
        let staged = bridge.take_staged_call();
        assert_eq!(staged.state.projector.retained_lights(), 1);
        assert!(staged.state.pending_lights.contains(&light.value));
        bridge.commit(staged);
        bridge.begin_call();
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let staged = bridge.take_staged_call();
        assert_eq!(staged.state.projector.retained_lights(), 2);
        bridge.commit(staged);

        // Disposing a waiting light needs no projection.
        bridge.begin_call();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        bridge.destroy_light(light).unwrap();
        bridge.destroy_light(unparented).unwrap();
        let staged = bridge.take_staged_call();
        assert_eq!(staged.state.projector.retained_lights(), 0);
        assert!(staged.state.lights.is_empty() && staged.state.pending_lights.is_empty());
    }

    #[test]
    fn material_replacement_succeeds_and_an_invalid_one_keeps_the_prior_material() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let color = NativeColor {
            r: 0.2,
            g: 0.4,
            b: 0.6,
            a: 1.0,
        };
        let request = NativeMaterialRequest {
            texture_scale: NativeVec2::default(),
            texture_offset: NativeVec2::default(),
            stochastic_tiling: 0.0,
            shader: Default::default(),
            triplanar_sharpness: 0.0,
            color,
            texture: NativeRenderResourceReference { value: 0 },
            roughness: 0.5,
            texture_tint: color,
            emission_color: NativeVec3::default(),
            emission_intensity: 0.0,
            double_sided: false,
            alpha_mode: NativeMaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            metalness: 0.0,
            normal_map: NativeRenderResourceReference::default(),
            normal_scale: 1.0,
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            orm_map: Default::default(),
            occlusion_strength: 0.0,
            unlit: false,
            flat_shading: false,
            wind_bend: 0.0,
            wind_flutter: 0.0,
            water: Default::default(),
            translucent_shadow: false,
        };
        let original = bridge.create_material(request).expect("material");
        let replacement = bridge
            .replace_material(NativeMaterialUpdateRequest {
                material: original,
                replacement: request,
            })
            .expect("an identical descriptor is a valid replacement");
        assert_ne!(replacement.value, original.value);
        let state = &bridge.staged_ref().unwrap().state;
        assert!(!state.materials.contains_key(&original.value));
        assert!(state.materials.contains_key(&replacement.value));

        let invalid = NativeMaterialRequest {
            texture: NativeRenderResourceReference { value: 99 },
            ..request
        };
        bridge
            .replace_material(NativeMaterialUpdateRequest {
                material: replacement,
                replacement: invalid,
            })
            .expect_err("an unknown texture is refused");
        let state = &bridge.staged_ref().unwrap().state;
        assert!(state.materials.contains_key(&replacement.value));
    }

    #[test]
    fn material_metalness_reaches_the_descriptor_and_out_of_range_is_refused() {
        let color = NativeColor {
            r: 1.0,
            g: 0.75,
            b: 0.3,
            a: 1.0,
        };
        let metal = NativeMaterialRequest {
            texture_scale: NativeVec2::default(),
            texture_offset: NativeVec2::default(),
            stochastic_tiling: 0.0,
            shader: Default::default(),
            triplanar_sharpness: 0.0,
            color,
            texture: NativeRenderResourceReference { value: 0 },
            roughness: 0.35,
            texture_tint: color,
            emission_color: NativeVec3::default(),
            emission_intensity: 0.0,
            double_sided: false,
            alpha_mode: NativeMaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            metalness: 1.0,
            normal_map: NativeRenderResourceReference::default(),
            normal_scale: 1.0,
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            orm_map: Default::default(),
            occlusion_strength: 0.0,
            unlit: false,
            flat_shading: false,
            wind_bend: 0.0,
            wind_flutter: 0.0,
            water: Default::default(),
            translucent_shadow: false,
        };
        let resources = RenderResourceRegistry::default();
        let descriptor = material_descriptor("material/metal".to_owned(), metal, &resources)
            .expect("metal material");
        assert_eq!(descriptor.metalness, 1.0);
        let error = material_descriptor(
            "material/over".to_owned(),
            NativeMaterialRequest {
                metalness: 1.5,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                ..metal
            },
            &resources,
        )
        .unwrap_err();
        assert_eq!(error.code(), "CSHARP_MATERIAL");
    }

    #[test]
    fn material_texture_repeat_and_stochastic_tiling_reach_the_descriptor() {
        let color = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let plain = NativeMaterialRequest {
            texture_scale: NativeVec2::default(),
            texture_offset: NativeVec2::default(),
            stochastic_tiling: 0.0,
            shader: Default::default(),
            triplanar_sharpness: 4.0,
            color,
            texture: NativeRenderResourceReference { value: 0 },
            roughness: 0.9,
            texture_tint: color,
            emission_color: NativeVec3::default(),
            emission_intensity: 0.0,
            double_sided: false,
            alpha_mode: NativeMaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            metalness: 0.0,
            normal_map: NativeRenderResourceReference::default(),
            normal_scale: 1.0,
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            orm_map: Default::default(),
            occlusion_strength: 0.0,
            unlit: false,
            flat_shading: false,
            wind_bend: 0.0,
            wind_flutter: 0.0,
            water: Default::default(),
            translucent_shadow: false,
        };
        let resources = RenderResourceRegistry::default();
        let descriptor = material_descriptor("material/plain".to_owned(), plain, &resources)
            .expect("plain material");
        assert_eq!(
            descriptor.texture_transform, None,
            "a zero scale repeats once"
        );
        assert_eq!(descriptor.stochastic_tiling, None);
        let floor = NativeMaterialRequest {
            texture_scale: NativeVec2 {
                x: 1.0 / 3.0,
                y: 0.0,
            },
            texture_offset: NativeVec2 { x: 0.5, y: 0.25 },
            stochastic_tiling: 6.0,
            ..plain
        };
        let descriptor = material_descriptor("material/floor".to_owned(), floor, &resources)
            .expect("floor material");
        assert_eq!(
            descriptor.texture_transform,
            Some(render_model::MaterialTextureTransformDescriptor {
                scale: [1.0 / 3.0, 1.0],
                offset: [0.5, 0.25],
            })
        );
        assert_eq!(
            descriptor.stochastic_tiling,
            Some(render_model::MaterialStochasticTilingDescriptor { contrast: 6.0 })
        );
        let error = material_descriptor(
            "material/soft".to_owned(),
            NativeMaterialRequest {
                stochastic_tiling: 0.5,
                ..floor
            },
            &resources,
        )
        .unwrap_err();
        assert_eq!(error.code(), "CSHARP_MATERIAL");
    }

    #[test]
    fn a_directional_light_range_is_its_shadow_distance_and_reads_back() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let mut sun = point_light_request(93, None);
        sun.descriptor.kind = NativeLightKind::Directional;
        sun.descriptor.direction = NativeVec3 {
            x: -1.0,
            y: -2.0,
            z: 0.5,
        };
        sun.descriptor.range = 150.0;
        let light = bridge.create_light(sun).unwrap();
        let readout = bridge.read_light(light).unwrap();
        assert_eq!(readout.descriptor.kind, NativeLightKind::Directional);
        assert!(readout.descriptor.has_range);
        assert_eq!(readout.descriptor.range, 150.0);
        let staged = bridge.take_staged_call();
        assert!(staged.render_ops().iter().any(|op| matches!(
            op,
            render_model::RenderDiff::CreateLight {
                light: LightDescriptor::Directional {
                    range: Some(150.0),
                    ..
                },
                ..
            }
        )));
    }

    #[test]
    fn invalid_light_replacement_preserves_the_committed_owner_and_requested_facts() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.begin_call();
        let light = bridge.create_light(point_light_request(91, None)).unwrap();
        let staged = bridge.take_staged_call();
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

        let retained = bridge.read_light(light).unwrap();
        assert_eq!(retained.logical_id, 91);
        assert_eq!(retained.descriptor.kind, NativeLightKind::Point);
        bridge.destroy_light(light).unwrap();
        let staged = bridge.take_staged_call();
        assert!(matches!(
            staged.render_ops().as_slice(),
            [render_model::RenderDiff::Destroy { .. }]
        ));
        bridge.commit(staged);

        bridge.begin_call();
        bridge.destroy_light(light).unwrap();
        assert!(bridge.take_staged_call().render_frames().is_empty());
    }

    #[test]
    fn normal_maps_are_linear_textures_retained_beside_the_base_texture() {
        let mut content = BTreeMap::new();
        content.insert("stone.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        bridge.begin_call();
        let colour = bridge
            .open_resource(&resource_request("stone.png"))
            .unwrap();
        let mut data_request = resource_request("stone.png");
        data_request.color_space = NativeTextureColorSpace::Linear;
        let data = bridge.open_resource(&data_request).unwrap();
        assert_ne!(colour.handle, data.handle, "one PNG, two colour spaces");
        let resources = &bridge.staged_ref().unwrap().state.render_resources;
        let linear = resources
            .get(data.handle.value)
            .and_then(CsharpRenderResource::texture)
            .and_then(|texture| texture.payload.as_ref())
            .map(|payload| payload.color_space);
        assert_eq!(linear, Some(render_model::TextureColorSpace::Linear));

        let white = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let request = |normal_map: NativeRenderResourceHandle| NativeMaterialRequest {
            texture_scale: NativeVec2::default(),
            texture_offset: NativeVec2::default(),
            stochastic_tiling: 0.0,
            shader: Default::default(),
            triplanar_sharpness: 0.0,
            color: white,
            texture: NativeRenderResourceReference {
                value: colour.handle.value,
            },
            roughness: 0.8,
            texture_tint: white,
            emission_color: NativeVec3::default(),
            emission_intensity: 0.0,
            double_sided: false,
            alpha_mode: NativeMaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            metalness: 0.0,
            normal_map: NativeRenderResourceReference {
                value: normal_map.value,
            },
            normal_scale: 0.5,
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            orm_map: Default::default(),
            occlusion_strength: 0.0,
            unlit: false,
            flat_shading: false,
            wind_bend: 0.0,
            wind_flutter: 0.0,
            water: Default::default(),
            translucent_shadow: false,
        };
        let refused =
            material_descriptor("material/a".to_owned(), request(colour.handle), resources)
                .unwrap_err();
        assert_eq!(refused.code(), "CSHARP_MATERIAL_NORMAL_MAP");
        let descriptor =
            material_descriptor("material/b".to_owned(), request(data.handle), resources).unwrap();
        let map = descriptor.normal_map.as_ref().expect("normal map");
        assert_eq!(map.scale, 0.5);
        let textures = texture_descriptors_for_material(&descriptor, resources).unwrap();
        assert_eq!(textures.len(), 2);
        assert_eq!(textures[1].id, map.texture);
        assert!(map.texture.ends_with("-linear"));
    }

    #[test]
    fn product_shaders_are_checked_when_opened_and_retained_with_their_materials() {
        const TINT: &str = "#import rusty::types::Surface
#import rusty::material::material
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface) * material.parameters[0];
}
";
        let mut content = BTreeMap::new();
        content.insert("shaders/tint.wgsl".to_owned(), Arc::from(TINT.as_bytes()));
        content.insert(
            "shaders/broken.wgsl".to_owned(),
            Arc::from(
                TINT.replace("standard_shade(surface)", "missing")
                    .as_bytes(),
            ),
        );
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        bridge.begin_call();
        let refused = bridge
            .open_resource(&resource_request("shaders/broken.wgsl"))
            .unwrap_err();
        assert_eq!(refused.code(), "CSHARP_SHADER");
        assert!(
            refused.detail().contains("shaders/broken.wgsl:6:"),
            "{}",
            refused.detail()
        );
        let shader = bridge
            .open_resource(&resource_request("shaders/tint.wgsl"))
            .unwrap();
        assert_eq!(shader.kind, NativeRenderResourceKind::Shader);

        let white = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let parameter = csharp_engine_abi::NativeVec4 {
            x: 1.0,
            y: 0.5,
            z: 0.25,
            w: 1.0,
        };
        let material = bridge
            .create_material(NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                color: white,
                texture: NativeRenderResourceReference::default(),
                roughness: 0.8,
                texture_tint: white,
                emission_color: NativeVec3::default(),
                emission_intensity: 0.0,
                double_sided: false,
                alpha_mode: NativeMaterialAlphaMode::Opaque,
                alpha_cutoff: 0.5,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                triplanar_sharpness: 0.0,
                shader: csharp_engine_abi::NativeMaterialShader {
                    shader: NativeRenderResourceReference {
                        value: shader.handle.value,
                    },
                    parameter_0: parameter,
                    ..Default::default()
                },
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                unlit: false,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            })
            .unwrap();
        let resources = bridge.staged_ref().unwrap().state.projector.resources();
        let descriptor = resources.materials.last().unwrap();
        let used = descriptor
            .shader
            .as_ref()
            .expect("the material names its shader");
        assert_eq!(used.parameters[0], [1.0, 0.5, 0.25, 1.0]);
        assert_eq!(resources.shaders.len(), 1);
        assert_eq!(resources.shaders[0].id, used.shader);
        assert_eq!(resources.shaders[0].path, "shaders/tint.wgsl");
        assert_eq!(
            bridge.destroy_resource(shader.handle).unwrap_err().code(),
            "CSHARP_RENDER_RESOURCE_IN_USE"
        );
        bridge.destroy_material(material).unwrap();
        bridge.destroy_resource(shader.handle).unwrap();
        assert!(bridge
            .staged_ref()
            .unwrap()
            .state
            .projector
            .resources()
            .shaders
            .is_empty());
    }

    #[test]
    fn shader_keywords_open_variants_and_shader_textures_are_held_by_their_materials() {
        const GLOW: &str = "#import rusty::types::Surface
#import rusty::material::{product_map_a, product_sampler_a}
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
#ifdef GLOW
    return standard_shade(surface) + textureSample(product_map_a, product_sampler_a, surface.uv);
#else
    return standard_shade(surface);
#endif
}
";
        let mut content = BTreeMap::new();
        content.insert("shaders/glow.wgsl".to_owned(), Arc::from(GLOW.as_bytes()));
        content.insert("noise.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        bridge.begin_call();
        let open = |bridge: &mut RuntimeAppearanceBridge, keywords: &'static str| {
            let mut request = resource_request("shaders/glow.wgsl");
            request.shader_keywords = NativeUtf8Slice {
                bytes: keywords.as_ptr(),
                len: keywords.len(),
            };
            bridge.open_resource(&request)
        };
        let plain = open(&mut bridge, "").unwrap();
        let glow = open(&mut bridge, "GLOW  EXTRA").unwrap();
        assert_ne!(
            plain.handle, glow.handle,
            "a keyword set is its own resource"
        );
        assert_eq!(
            open(&mut bridge, "EXTRA GLOW").unwrap().handle,
            glow.handle,
            "in any order"
        );
        let refused = open(&mut bridge, "NORMAL_MAP").unwrap_err();
        assert_eq!(refused.code(), "CSHARP_SHADER");
        assert_eq!(
            open(&mut bridge, "glow").unwrap_err().code(),
            "CSHARP_SHADER"
        );
        let resources = &bridge.staged_ref().unwrap().state.render_resources;
        let variant = resources.get(glow.handle.value).unwrap().shader().unwrap();
        assert_eq!(variant.keywords, ["EXTRA", "GLOW"]);

        let noise = bridge
            .open_resource(&resource_request("noise.png"))
            .unwrap();
        let white = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let request = |texture_a: u64, texture_b: u64| NativeMaterialRequest {
            texture_scale: NativeVec2::default(),
            texture_offset: NativeVec2::default(),
            stochastic_tiling: 0.0,
            color: white,
            texture: NativeRenderResourceReference::default(),
            roughness: 0.8,
            texture_tint: white,
            emission_color: NativeVec3::default(),
            emission_intensity: 0.0,
            double_sided: false,
            alpha_mode: NativeMaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            metalness: 0.0,
            normal_map: NativeRenderResourceReference::default(),
            normal_scale: 1.0,
            triplanar_sharpness: 0.0,
            shader: csharp_engine_abi::NativeMaterialShader {
                shader: NativeRenderResourceReference {
                    value: glow.handle.value,
                },
                texture_a: NativeRenderResourceReference { value: texture_a },
                texture_b: NativeRenderResourceReference { value: texture_b },
                ..Default::default()
            },
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            orm_map: Default::default(),
            occlusion_strength: 0.0,
            unlit: false,
            flat_shading: false,
            wind_bend: 0.0,
            wind_flutter: 0.0,
            water: Default::default(),
            translucent_shadow: false,
        };
        assert_eq!(
            bridge
                .create_material(request(plain.handle.value, 0))
                .unwrap_err()
                .code(),
            "CSHARP_MATERIAL_SHADER",
            "a shader texture must be a texture"
        );
        let material = bridge
            .create_material(request(noise.handle.value, 0))
            .unwrap();
        let resources = bridge.staged_ref().unwrap().state.projector.resources();
        let used = resources.materials.last().unwrap().shader.clone().unwrap();
        let [Some(noise_id), None] = used.textures else {
            panic!("slot A holds the texture: {:?}", used.textures);
        };
        assert!(resources
            .textures
            .iter()
            .any(|texture| texture.id == noise_id));
        // Each texture keeps its slot: B alone leaves A white.
        let only_b = bridge
            .create_material(request(0, noise.handle.value))
            .unwrap();
        let resources = bridge.staged_ref().unwrap().state.projector.resources();
        let used = resources.materials.last().unwrap().shader.clone().unwrap();
        assert_eq!(used.textures, [None, Some(noise_id)]);
        bridge.destroy_material(only_b).unwrap();
        assert_eq!(
            bridge.destroy_resource(noise.handle).unwrap_err().code(),
            "CSHARP_RENDER_RESOURCE_IN_USE"
        );
        bridge.destroy_material(material).unwrap();
        bridge.destroy_resource(noise.handle).unwrap();
    }

    #[test]
    fn texture_sampling_variants_share_pixels_but_retain_distinct_handles_and_materials() {
        let mut content = BTreeMap::new();
        content.insert("tiles.png".to_owned(), Arc::from(RGBA_PNG));
        content.insert("alias.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        bridge.begin_call();
        let clamp = bridge
            .open_resource(&resource_request("tiles.png"))
            .unwrap();
        let mut repeat_request = resource_request("tiles.png");
        repeat_request.wrap = NativeTextureWrap::Repeat;
        let repeat = bridge.open_resource(&repeat_request).unwrap();
        let mut linear_request = repeat_request;
        linear_request.filter = NativeTextureFilter::Linear;
        let linear = bridge.open_resource(&linear_request).unwrap();
        assert_ne!(clamp.handle, repeat.handle);
        assert_ne!(repeat.handle, linear.handle);
        for path in ["tiles.png", "content/tiles.png", "alias.png"] {
            let mut alias = resource_request(path);
            alias.wrap = NativeTextureWrap::Repeat;
            assert_eq!(bridge.open_resource(&alias).unwrap().handle, repeat.handle);
            assert_eq!(
                bridge
                    .open_resource(&resource_request(path))
                    .unwrap()
                    .handle,
                clamp.handle
            );
        }
        let clamp_resource = bridge.resource(clamp.handle.value).unwrap();
        let repeat_resource = bridge.resource(repeat.handle.value).unwrap();
        assert_eq!(clamp_resource.identity(), repeat_resource.identity());
        assert_eq!(
            clamp_resource.content_hash(),
            repeat_resource.content_hash()
        );
        assert_ne!(
            clamp_resource.asset_identity(),
            repeat_resource.asset_identity()
        );
        assert_eq!(clamp_resource.texture().unwrap().wrap, TextureWrap::Clamp);
        assert_eq!(repeat_resource.texture().unwrap().wrap, TextureWrap::Repeat);
        assert_eq!(
            bridge
                .resource(linear.handle.value)
                .unwrap()
                .texture()
                .unwrap()
                .filter,
            TextureFilter::Linear
        );
        for resource in [clamp, repeat, linear] {
            bridge
                .create_material(NativeMaterialRequest {
                    texture_scale: NativeVec2::default(),
                    texture_offset: NativeVec2::default(),
                    stochastic_tiling: 0.0,
                    shader: Default::default(),
                    triplanar_sharpness: 0.0,
                    color: NativeColor {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                    texture: NativeRenderResourceReference {
                        value: resource.handle.value,
                    },
                    roughness: 1.0,
                    metalness: 0.0,
                    normal_map: NativeRenderResourceReference::default(),
                    normal_scale: 1.0,
                    texture_tint: NativeColor {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                    emission_color: NativeVec3::default(),
                    emission_intensity: 0.0,
                    double_sided: false,
                    alpha_mode: NativeMaterialAlphaMode::Opaque,
                    alpha_cutoff: 0.5,
                    emission_map: Default::default(),
                    occlusion_map: Default::default(),
                    orm_map: Default::default(),
                    occlusion_strength: 0.0,
                    unlit: false,
                    flat_shading: false,
                    wind_bend: 0.0,
                    wind_flutter: 0.0,
                    water: Default::default(),
                    translucent_shadow: false,
                })
                .unwrap();
        }
        let resources = bridge
            .staged
            .as_mut()
            .unwrap()
            .state
            .projector
            .resources_mut();
        assert_eq!(resources.textures.len(), 3);
        assert_eq!(
            resources
                .textures
                .iter()
                .filter(|texture| texture.wrap == TextureWrap::Clamp)
                .count(),
            1
        );
        assert_eq!(
            resources
                .textures
                .iter()
                .filter(|texture| texture.wrap == TextureWrap::Repeat)
                .count(),
            2
        );
    }

    #[test]
    fn sprites_retain_selected_sampling_and_typed_material_facts() {
        let mut content = BTreeMap::new();
        content.insert("sprite.png".to_owned(), Arc::from(RGBA_PNG));
        content.insert("normal.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        bridge.begin_call();
        let mut sprite_texture_request = resource_request("sprite.png");
        sprite_texture_request.filter = NativeTextureFilter::Linear;
        sprite_texture_request.wrap = NativeTextureWrap::Repeat;
        let sprite_texture = bridge
            .open_resource(&sprite_texture_request)
            .expect("selected sprite texture");
        let normal_texture = bridge
            .open_resource(&resource_request("normal.png"))
            .expect("selected normal texture");
        let frames = [NativeSpriteAtlasFrame {
            frame_id: 7,
            uv_min: NativeVec2::default(),
            uv_max: NativeVec2 { x: 1.0, y: 1.0 },
            has_size: false,
            size: NativeVec2::default(),
        }];
        let atlas = unsafe {
            bridge
                .create_sprite_atlas(&NativeSpriteAtlasCreateRequest {
                    texture: sprite_texture.handle,
                    frames: frames.as_ptr(),
                    frames_len: frames.len(),
                })
                .expect("atlas")
        };
        let mut request = atlas_sprite_request(atlas, 7);
        request.material = NativeSpriteMaterialDescriptor {
            lighting: NativeSpriteLightingMode::AuthoredNormal,
            normal_texture: NativeRenderResourceReference {
                value: normal_texture.handle.value,
            },
            depth_texture: NativeRenderResourceReference::default(),
            normal_strength: 1.5,
            normal_bias: 0.1,
            alpha_mode: NativeSpriteAlphaMode::Mask,
            alpha_cutoff: 0.4,
            shadow: NativeSpriteShadowPolicy::CastAndReceive,
            blend: NativeSpriteBlendMode::Alpha,
            softness_metres: 0.0,
        };
        let appearance = bridge
            .create_sprite_from_atlas(request)
            .expect("typed sprite material");
        let normal_identity = bridge
            .resource(normal_texture.handle.value)
            .unwrap()
            .asset_identity()
            .to_owned();
        let resources = bridge
            .staged
            .as_mut()
            .unwrap()
            .state
            .projector
            .resources_mut();
        let atlas_texture = resources
            .textures
            .iter()
            .find(|texture| texture.id == "texture/atlas-1")
            .expect("atlas texture");
        assert_eq!(atlas_texture.filter, TextureFilter::Linear);
        assert_eq!(atlas_texture.wrap, TextureWrap::Repeat);
        assert!(
            resources
                .textures
                .iter()
                .any(|texture| texture.id == normal_identity),
            "normal map must be present in the retained resource baseline"
        );
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        let call = bridge.take_staged_call();
        let mut world = render_presentation::PresentationWorld::default();
        for output in &call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        let encoded = serde_json::to_string(&world.snapshot().frame).unwrap();
        assert!(encoded.contains("sprite/atlas-1"));
        assert!(encoded.contains("authoredNormal"));
        assert!(encoded.contains("castAndReceive"));
        bridge.commit(call);
        bridge.begin_call();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        bridge.destroy_appearance(appearance).unwrap();
        bridge.destroy_sprite_atlas(atlas).unwrap();
        bridge.destroy_resource(sprite_texture.handle).unwrap();
        bridge.destroy_resource(normal_texture.handle).unwrap();
        let call = bridge.take_staged_call();
        for output in &call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        assert!(
            !world.snapshot().frame.ops.iter().any(|op| matches!(
                op,
                RenderDiff::DefineTexture { .. }
                    | RenderDiff::DefineSpriteAtlas { .. }
                    | RenderDiff::CreateSprite { .. }
            )),
            "fresh unloaded baseline cannot retain an atlas or either of its texture sources"
        );
    }

    #[test]
    fn selected_resources_are_deduplicated_and_create_time_only() {
        let mut content_resources = BTreeMap::new();
        content_resources.insert("selected.png".to_owned(), Arc::from(RGBA_PNG));
        content_resources.insert(
            "unselected.png".to_owned(),
            Arc::from(&b"not an RGBA PNG"[..]),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);

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
        let staged = bridge.take_staged_call();
        bridge.commit(staged);

        bridge.begin_call();
        assert!(bridge
            .open_resource(&resource_request("unselected.png"))
            .is_err());
        bridge.end_call();
        assert_eq!(bridge.state.render_resources.len(), 1);
    }

    #[test]
    fn render_resource_release_counts_owners_guards_live_materials_and_reopens_fresh_slot() {
        let mut content = BTreeMap::new();
        content.insert("release.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);

        bridge.begin_call();
        let first = bridge
            .open_resource(&resource_request("release.png"))
            .unwrap();
        let duplicate = bridge
            .open_resource(&resource_request("release.png"))
            .unwrap();
        assert_eq!(first.handle, duplicate.handle);
        bridge
            .destroy_resource(first.handle)
            .expect("one duplicate owner released");
        assert!(bridge.resource(first.handle.value).is_ok());
        bridge
            .create_material(NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                shader: Default::default(),
                triplanar_sharpness: 0.0,
                color: NativeColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                texture: NativeRenderResourceReference {
                    value: first.handle.value,
                },
                roughness: 1.0,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                texture_tint: NativeColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                emission_color: NativeVec3::default(),
                emission_intensity: 0.0,
                double_sided: false,
                alpha_mode: NativeMaterialAlphaMode::Opaque,
                alpha_cutoff: 0.5,
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                unlit: false,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            })
            .unwrap();
        assert_eq!(
            bridge.destroy_resource(first.handle).unwrap_err().code(),
            "CSHARP_RENDER_RESOURCE_IN_USE"
        );
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        let call = bridge.take_staged_call();
        let mut world = render_presentation::PresentationWorld::default();
        for output in &call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        assert!(world
            .snapshot()
            .frame
            .ops
            .iter()
            .any(|op| matches!(op, RenderDiff::DefineTexture { .. })));
        bridge.commit(call);
        bridge.begin_call();
        bridge
            .destroy_material(NativeMaterialHandle { value: 1 })
            .unwrap();
        bridge.destroy_resource(first.handle).unwrap();
        assert!(bridge.resource(first.handle.value).is_err());
        let call = bridge.take_staged_call();
        for output in &call.outputs {
            if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                world.apply(frame.clone()).unwrap();
            }
        }
        assert!(
            !world.snapshot().frame.ops.iter().any(|op| matches!(
                op,
                RenderDiff::DefineTexture { .. } | RenderDiff::DefineMaterial { .. }
            )),
            "fresh unloaded baseline has no stale texture or material definition"
        );
        bridge.commit(call);

        bridge.begin_call();
        let reopened = bridge
            .open_resource(&resource_request("release.png"))
            .unwrap();
        assert_ne!(
            reopened.handle, first.handle,
            "released handles stay tombstoned"
        );
    }

    #[test]
    fn render_resource_release_waits_for_the_staged_or_retained_sky() {
        let mut content = BTreeMap::new();
        content.insert("sky.png".to_owned(), Arc::from(RGBA_PNG));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        let mut camera = crate::camera_view::RuntimeCameraViewBridge::new();
        bridge.bind_camera_view(&camera);

        bridge.begin_call();
        let sky = bridge.open_resource(&resource_request("sky.png")).unwrap();
        let call = bridge.take_staged_call();
        bridge.commit(call);

        camera.begin_call();
        assert_eq!(
            unsafe {
                crate::camera_view::set_sky_background(
                    (&mut camera as *mut crate::camera_view::RuntimeCameraViewBridge).cast(),
                    sky.handle,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let call = camera.take_staged_call().unwrap();
        camera.commit(call);

        bridge.begin_call();
        assert_eq!(
            bridge.destroy_resource(sky.handle).unwrap_err().code(),
            "CSHARP_RENDER_RESOURCE_IN_USE"
        );
        bridge.end_call();

        camera.begin_call();
        let clear = NativeClearSkyBackgroundRequest::default();
        assert_eq!(
            unsafe {
                crate::camera_view::clear_sky_background(
                    (&mut camera as *mut crate::camera_view::RuntimeCameraViewBridge).cast(),
                    &clear,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let call = camera.take_staged_call().unwrap();
        camera.commit(call);
        bridge.begin_call();
        bridge.destroy_resource(sky.handle).unwrap();
    }

    #[test]
    fn inline_static_mesh_content_packs_one_selected_resource_for_retained_appearances() {
        let document = StaticMeshAsset {
            asset: "mesh/test".to_owned(),
            payload: MeshPayloadDescriptor {
                texture_space: None,
                distance_field: None,
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
                layer_weights: false,
                layer_palette: Vec::new(),
                vertex_occlusion: false,
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
        let first = bridge
            .create_static_mesh_from_content(&request)
            .expect("first retained appearance");
        let imported_bytes = bridge.imports.cached("mesh.json").unwrap().shared_bytes();
        let second = bridge
            .create_static_mesh_from_content(&request)
            .expect("second retained appearance");
        assert!(Arc::ptr_eq(
            &imported_bytes,
            &bridge.imports.cached("mesh.json").unwrap().shared_bytes()
        ));
        let fact = appearance_fact(first);
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("publish the first mesh");
        let staged = bridge.take_staged_call();
        bridge.commit(staged);

        assert_eq!(bridge.state.render_resources.len(), 1);
        let resources = bridge.state.projector.resources_mut();
        assert_eq!(resources.static_meshes.len(), 2);
        assert_eq!(resources.materials.len(), 2);

        bridge.begin_call();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.expect("remove visible mesh");
        bridge
            .destroy_appearance(first)
            .expect("first static appearance release");
        assert_eq!(
            bridge.staged.as_ref().unwrap().state.render_resources.len(),
            1
        );
        bridge
            .destroy_appearance(second)
            .expect("last static appearance release");
        // Releases are published with the call's other resource changes.
        let mut staged = bridge.take_staged_call();
        assert!(staged.state.render_resources.is_empty());
        assert!(staged.render_frames().iter().any(|frame| frame.ops.iter().any(|op| {
            matches!(op, render_model::RenderDiff::ReleaseStaticMesh { asset } if asset == &format!("mesh/native-{}", first.value))
        })), "disposing the published appearance releases its GPU mesh definition");
        let resources = staged.state.projector.resources_mut();
        assert!(resources.static_meshes.is_empty());
        assert!(resources.materials.is_empty());
    }

    #[test]
    fn animated_resource_release_removes_baseline_and_reopen_redefines_before_instance() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut content = BTreeMap::new();
        content.insert("character.glb".to_owned(), Arc::from(CHARACTER_GLB));
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content);
        let mut world = render_presentation::PresentationWorld::default();
        for _ in 0..2 {
            bridge.begin_call();
            let path = b"character.glb";
            let resource = bridge
                .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                    path: NativeUtf8Slice {
                        bytes: path.as_ptr(),
                        len: path.len(),
                    },
                })
                .unwrap();
            let appearance = bridge
                .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
                .unwrap();
            let fact = appearance_fact(appearance);
            unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
            let call = bridge.take_staged_call();
            let ops: Vec<_> = call
                .outputs
                .iter()
                .filter_map(|output| match output {
                    RuntimeAppearanceCallOutput::Frame(frame) => Some(&frame.ops),
                    _ => None,
                })
                .flatten()
                .collect();
            let definition = ops
                .iter()
                .position(|op| matches!(op, RenderDiff::DefineAnimatedMesh { .. }))
                .unwrap();
            let instance = ops
                .iter()
                .position(|op| matches!(op, RenderDiff::CreateAnimatedMeshInstance { .. }))
                .unwrap();
            assert!(
                definition < instance,
                "reopening restores the logical definition before its first use"
            );
            for output in &call.outputs {
                if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                    world.apply(frame.clone()).unwrap();
                }
            }
            bridge.commit(call);
            bridge.begin_call();
            unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
            bridge.destroy_appearance(appearance).unwrap();
            bridge.destroy_resource(resource).unwrap();
            let call = bridge.take_staged_call();
            assert!(call.outputs.iter().any(|output| matches!(output, RuntimeAppearanceCallOutput::Frame(frame) if frame.ops.iter().any(|op| matches!(op, RenderDiff::ReleaseAnimatedMesh { .. })))));
            for output in &call.outputs {
                if let RuntimeAppearanceCallOutput::Frame(frame) = output {
                    world.apply(frame.clone()).unwrap();
                }
            }
            assert!(
                !world.snapshot().frame.ops.iter().any(|op| matches!(
                    op,
                    RenderDiff::DefineAnimatedMesh { .. }
                        | RenderDiff::CreateAnimatedMeshInstance { .. }
                )),
                "fresh unloaded baseline cannot require the retired GLB body"
            );
            assert!(call.state.render_resources.is_empty());
            bridge.commit(call);
        }
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
        let playback_ops = |bridge: &RuntimeAppearanceBridge| {
            bridge
                .staged
                .as_ref()
                .expect("staged call")
                .render_ops()
                .iter()
                .filter(|op| matches!(op, RenderDiff::SetAnimatedMeshPlayback { .. }))
                .count()
        };
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("appearance snapshot");
        assert_eq!(playback_ops(&bridge), 1);
        let first_call = bridge.take_staged_call();
        bridge.commit(first_call);

        bridge.begin_call();
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("unchanged appearance snapshot");
        assert_eq!(playback_ops(&bridge), 0);
        bridge
            .destroy_animation_instance(instance)
            .expect("teardown after an unchanged snapshot keeps output order");
        assert_eq!(playback_ops(&bridge), 1);
        let completed = bridge.take_staged_call();
        bridge.commit(completed);
        bridge.begin_call();
        let replacement = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: fact.object_id,
            })
            .unwrap();
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
        let frames_before = bridge.staged.as_ref().unwrap().render_frames().len();
        bridge
            .destroy_animation_instance(replacement)
            .expect("teardown after removal snapshot");
        assert_eq!(
            bridge.staged.as_ref().unwrap().render_frames().len(),
            frames_before,
            "do not send Stop to a renderer target already removed by the snapshot"
        );
    }

    #[test]
    fn animated_filename_spelling_is_admitted_through_csharp_service() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let expected_asset = format!("mesh-animation/{:x}", sha2::Sha256::digest(CHARACTER_GLB));
        for relative_path in [
            "actors/UAL1_Standard.glb",
            "actors/UAL1 Standard.glb",
            "actors/ual1-standard.glb",
        ] {
            let mut content_resources = BTreeMap::new();
            content_resources.insert(relative_path.to_owned(), Arc::from(CHARACTER_GLB));
            let mut bridge = RuntimeAppearanceBridge::new(
                RuntimeAppearanceCatalog::default(),
                content_resources,
            );
            let request_path = format!("content/{relative_path}");

            bridge.begin_call();
            bridge
                .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                    path: NativeUtf8Slice {
                        bytes: request_path.as_ptr(),
                        len: request_path.len(),
                    },
                })
                .expect("filename spelling is not an identity requirement");

            let resource = bridge
                .staged
                .as_ref()
                .expect("staged animated resource")
                .state
                .render_resources
                .first()
                .expect("one admitted animated resource");
            assert_eq!(resource.path(), request_path);
            assert_eq!(
                resource.animated_mesh().expect("animated descriptor").asset,
                expected_asset
            );
        }
    }

    #[test]
    fn animated_external_image_closure_is_packed_before_direct_playback() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let external = external_image_glb(CHARACTER_GLB, "Textures/character.png");
        let mut content_resources = BTreeMap::new();
        content_resources.insert(
            "actors/character.glb".to_owned(),
            Arc::from(external.as_slice()),
        );
        content_resources.insert(
            "actors/Textures/character.png".to_owned(),
            Arc::from(RGBA_PNG),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let resource_path = b"content/actors/character.glb";
        let clip = b"idle";

        bridge.begin_call();
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: resource_path.as_ptr(),
                    len: resource_path.len(),
                },
            })
            .expect("external image closure is admitted");
        let packed = bridge
            .staged
            .as_ref()
            .unwrap()
            .state
            .render_resources
            .first()
            .unwrap();
        assert!(asset_import::glb_relative_resource_uris(packed.bytes())
            .unwrap()
            .is_empty());
        assert_ne!(packed.bytes(), external.as_slice());
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .expect("animated appearance");
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 77,
            })
            .expect("animation instance");
        bridge
            .set_animation_playback(&NativeAnimationPlaybackRequest {
                instance,
                kind: NativeAnimationPlaybackKind::Play,
                clip: NativeUtf8Slice {
                    bytes: clip.as_ptr(),
                    len: clip.len(),
                },
                loop_mode: NativeAnimationLoopMode::Repeat,
                speed: 1.0,
                weight: 1.0,
                restart: true,
                fade_seconds: 0.0,
                has_fade: false,
                normalized_time: 0.0,
            })
            .expect("embedded clip playback");
    }

    #[test]
    fn animation_realization_marks_present_object_and_generation() {
        let receipt = animation_realization_receipt(&AnimationRealizationFact::Playback {
            fact_id: 1,
            object_id: 42,
            generation: 3,
            sequence: 1,
            status: "playing".into(),
            clip: Some("walk".into()),
            sampled_millis: Some(500),
        });
        assert!(receipt.has_object_id && receipt.has_generation);
        assert_eq!(receipt.object_id, 42);
        let completion =
            animation_realization_receipt(&AnimationRealizationFact::NaturalCompletion {
                fact_id: 2,
                object_id: 42,
                generation: 3,
                clip: "walk".into(),
            });
        assert!(completion.has_object_id && completion.has_generation);
    }

    #[test]
    fn live_animated_admission_releases_resources_and_recovers_after_bad_source() {
        const CHARACTER: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut content = crate::content::RuntimeContentBridge::new(BTreeMap::new());
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.bind_content(&content);
        bridge.begin_call();
        let admit = |content: &mut crate::content::RuntimeContentBridge, bytes: &[u8]| {
            let mut handle = NativeContentReferenceHandle::default();
            let request = NativeContentAdmissionRequest {
                path: NativeUtf8Slice {
                    bytes: b"live.glb".as_ptr(),
                    len: 8,
                },
                bytes: NativeByteSlice {
                    bytes: bytes.as_ptr(),
                    len: bytes.len(),
                },
                dependencies: std::ptr::null(),
                dependencies_len: 0,
            };
            assert_eq!(
                unsafe {
                    crate::content::admit_reference(
                        (content as *mut crate::content::RuntimeContentBridge).cast(),
                        &request,
                        &mut handle,
                    )
                },
                ABI_OK
            );
            handle
        };
        let reference = admit(&mut content, CHARACTER);
        let resource = bridge
            .admit_animated_mesh(content.retained_content(reference).unwrap())
            .unwrap();
        assert!(bridge.imports.cached("live.glb").is_none());
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .unwrap();
        let bad = admit(&mut content, b"not a GLB");
        let mut result = NativeRenderResourceHandle::default();
        let mut receipt = std::mem::MaybeUninit::<NativeOperationErrorReceipt>::uninit();
        let context = (&mut bridge as *mut RuntimeAppearanceBridge).cast();
        assert_eq!(
            unsafe {
                open_animated_mesh_from_content(
                    context,
                    &NativeAnimationContentRequest { content: bad },
                    &mut result,
                    receipt.as_mut_ptr(),
                )
            },
            0
        );
        let receipt = unsafe { receipt.assume_init() };
        assert_eq!(receipt.diagnostics_len, 1);
        assert!(bridge.operation_error.is_none());
        assert!(bridge.resource(resource.value).is_ok());
        let external = external_image_glb(CHARACTER, "texture.png");
        let external_reference = admit(&mut content, &external);
        let mut external_content = content.retained_content(external_reference).unwrap();
        // The missing companion cannot resolve from the previous model.
        assert!(bridge
            .admit_animated_mesh(external_content.clone())
            .is_err());
        external_content.files = crate::content::ContentFiles::snapshot(BTreeMap::from([(
            "texture.png".into(),
            Arc::from(RGBA_PNG),
        )]));
        let external_resource = bridge.admit_animated_mesh(external_content).unwrap();
        assert!(asset_import::glb_relative_resource_uris(
            bridge.resource(external_resource.value).unwrap().bytes()
        )
        .unwrap()
        .is_empty());
        assert!(bridge.imports.cached("live.glb").is_none());
        bridge.destroy_resource(external_resource).unwrap();
        bridge.destroy_appearance(appearance).unwrap();
        bridge.destroy_resource(resource).unwrap();
        assert!(bridge.resource(resource.value).is_err());
        bridge.end_call();
    }

    #[test]
    fn loading_bay_button_external_texture_reaches_direct_playback() {
        const BUTTON_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-factory-kit/button-floor-square.glb"
        );
        let mut content_resources = BTreeMap::new();
        content_resources.insert(
            "loading-bay/button-floor-square.glb".to_owned(),
            Arc::from(BUTTON_GLB),
        );
        content_resources.insert(
            "loading-bay/Textures/colormap.png".to_owned(),
            Arc::from(RGBA_PNG),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let resource_path = b"content/loading-bay/button-floor-square.glb";
        let clip = b"toggle-on";

        bridge.begin_call();
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: resource_path.as_ptr(),
                    len: resource_path.len(),
                },
            })
            .expect("Loading Bay GLB closure is admitted");
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .expect("Loading Bay animated appearance");
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 91,
            })
            .expect("Loading Bay animation instance");
        bridge
            .set_animation_playback(&NativeAnimationPlaybackRequest {
                instance,
                kind: NativeAnimationPlaybackKind::Play,
                clip: NativeUtf8Slice {
                    bytes: clip.as_ptr(),
                    len: clip.len(),
                },
                loop_mode: NativeAnimationLoopMode::Repeat,
                speed: 1.0,
                weight: 1.0,
                restart: true,
                fade_seconds: 0.0,
                has_fade: false,
                normalized_time: 0.0,
            })
            .expect("Loading Bay embedded clip playback");
    }

    #[test]
    fn animated_external_image_closure_missing_or_wrong_case_is_fail_atomic() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let external = external_image_glb(CHARACTER_GLB, "Textures/character.png");
        let mut content_resources = BTreeMap::new();
        content_resources.insert(
            "actors/character.glb".to_owned(),
            Arc::from(external.as_slice()),
        );
        content_resources.insert(
            "actors/textures/character.png".to_owned(),
            Arc::from(RGBA_PNG),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let resource_path = b"content/actors/character.glb";

        bridge.begin_call();
        let error = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: resource_path.as_ptr(),
                    len: resource_path.len(),
                },
            })
            .expect_err("dependency lookup preserves authored case");
        assert_eq!(error.code(), "CSHARP_ANIMATION_GLB_CLOSURE");
        let staged = bridge.staged.as_ref().unwrap();
        assert!(staged.state.render_resources.is_empty());
        assert!(staged.outputs.is_empty());
    }

    #[test]
    fn animated_mesh_material_bindings_retain_selected_material_handles() {
        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut content_resources = BTreeMap::new();
        content_resources.insert("character.glb".to_owned(), Arc::from(CHARACTER_GLB));
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let path = b"character.glb";

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
        let material = bridge
            .create_material(NativeMaterialRequest {
                texture_scale: NativeVec2::default(),
                texture_offset: NativeVec2::default(),
                stochastic_tiling: 0.0,
                shader: Default::default(),
                triplanar_sharpness: 0.0,
                color: NativeColor {
                    r: 0.8,
                    g: 0.2,
                    b: 0.1,
                    a: 1.0,
                },
                texture: NativeRenderResourceReference { value: 0 },
                roughness: 0.5,
                metalness: 0.0,
                normal_map: NativeRenderResourceReference::default(),
                normal_scale: 1.0,
                texture_tint: NativeColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                emission_color: NativeVec3::default(),
                emission_intensity: 0.0,
                double_sided: false,
                alpha_mode: NativeMaterialAlphaMode::Opaque,
                alpha_cutoff: 0.5,
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                orm_map: Default::default(),
                occlusion_strength: 0.0,
                unlit: false,
                flat_shading: false,
                wind_bend: 0.0,
                wind_flutter: 0.0,
                water: Default::default(),
                translucent_shadow: false,
            })
            .expect("material");
        let bindings = [NativeMeshMaterialBinding {
            material_slot: 0,
            material,
        }];
        unsafe {
            bridge
                .update_animated_mesh_materials(&NativeAnimatedMeshMaterialUpdateRequest {
                    appearance,
                    bindings: bindings.as_ptr(),
                    bindings_len: bindings.len(),
                })
                .expect("animated material binding");
        }
        let invalid_bindings = [NativeMeshMaterialBinding {
            material_slot: 1,
            material,
        }];
        assert_eq!(
            unsafe {
                bridge.update_animated_mesh_materials(&NativeAnimatedMeshMaterialUpdateRequest {
                    appearance,
                    bindings: invalid_bindings.as_ptr(),
                    bindings_len: invalid_bindings.len(),
                })
            }
            .expect_err("unbound embedded material slot is rejected")
            .code(),
            "CSHARP_ANIMATED_MESH_SLOT"
        );
        assert_eq!(
            bridge
                .destroy_material(material)
                .expect_err("bound material remains live")
                .code(),
            "CSHARP_MATERIAL_IN_USE"
        );

        unsafe {
            bridge
                .update_animated_mesh_materials(&NativeAnimatedMeshMaterialUpdateRequest {
                    appearance,
                    bindings: std::ptr::null(),
                    bindings_len: 0,
                })
                .expect("clear animated material bindings");
        }
        bridge
            .destroy_material(material)
            .expect("cleared material is releasable");

        let factor = |material_slot, base: f32| NativeMeshMaterialFactors {
            material_slot,
            override_base_color: true,
            base_color: NativeColor {
                r: base,
                g: 0.25,
                b: 0.0,
                a: 1.0,
            },
            override_emission: false,
            emissive_factor: NativeVec3::default(),
            emissive_strength: 0.0,
            override_texture_tint: false,
            texture_tint: Default::default(),
        };
        let update = |bridge: &mut RuntimeAppearanceBridge,
                      factors: &[NativeMeshMaterialFactors]| unsafe {
            bridge.update_animated_mesh_material_factors(
                &NativeAnimatedMeshMaterialFactorsRequest {
                    appearance,
                    factors: factors.as_ptr(),
                    factors_len: factors.len(),
                },
            )
        };
        update(&mut bridge, &[factor(0, 0.75)]).expect("slot 0 factors");
        let identity = bridge.staged_ref().unwrap().state.appearances[&appearance.value].clone();
        let parameters = match bridge
            .staged_ref()
            .unwrap()
            .state
            .projector
            .appearance(&identity)
        {
            Some(Appearance::AnimatedMesh {
                material_parameters,
                ..
            }) => material_parameters.clone(),
            _ => panic!("animated appearance"),
        };
        assert_eq!(parameters[&0].base_color, Some([0.75, 0.25, 0.0, 1.0]));
        assert_eq!(
            parameters[&0].emission, None,
            "the GLB's own emission stays"
        );
        for (factors, code) in [
            (vec![factor(1, 0.5)], "CSHARP_ANIMATED_MESH_SLOT"),
            (vec![factor(0, 1.5)], "CSHARP_ANIMATED_MESH_FACTORS"),
            (
                vec![factor(0, 0.5), factor(0, 0.5)],
                "CSHARP_ANIMATED_MESH_SLOT",
            ),
        ] {
            assert_eq!(
                update(&mut bridge, &factors).expect_err("refused").code(),
                code
            );
        }
        update(&mut bridge, &[]).expect("clear factors");
    }

    #[test]
    fn admitted_clip_pack_is_retained_separately_and_augments_effective_graph_clips() {
        use sha2::{Digest, Sha256};

        const CHARACTER_GLB: &[u8] = include_bytes!(
            "../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        let mut content_resources = BTreeMap::new();
        content_resources.insert("primary.glb".to_owned(), Arc::from(CHARACTER_GLB));
        content_resources.insert("pack.glb".to_owned(), Arc::from(CHARACTER_GLB));
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let primary_path = b"primary.glb";
        let pack_path = b"pack.glb";
        let hash = format!("sha256:{:x}", Sha256::digest(CHARACTER_GLB));
        let producer = b"test-import";
        let license = b"CC0-1.0";

        bridge.begin_call();
        let primary = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: primary_path.as_ptr(),
                    len: primary_path.len(),
                },
            })
            .expect("primary animated mesh admission");
        let pack = bridge
            .open_animation_clip_pack(&NativeAnimationClipPackResourceRequest {
                path: NativeUtf8Slice {
                    bytes: pack_path.as_ptr(),
                    len: pack_path.len(),
                },
            })
            .expect("clip-pack admission");
        assert_ne!(
            primary.value, pack.value,
            "same bytes retain distinct roles"
        );
        let associate = NativeAnimationClipPackAssociationRequest {
            primary_mesh: primary,
            clip_pack: pack,
            producer: NativeUtf8Slice {
                bytes: producer.as_ptr(),
                len: producer.len(),
            },
            license: NativeUtf8Slice {
                bytes: license.as_ptr(),
                len: license.len(),
            },
        };
        let collision = bridge
            .associate_animation_clip_pack(&associate)
            .expect_err("same clip identities must remain incompatible");
        assert_eq!(collision.code(), "CSHARP_ANIMATION_CLIP_PACK_ASSOCIATION");
        assert!(bridge
            .resource(primary.value)
            .expect("primary resource")
            .animated_mesh()
            .expect("primary descriptor")
            .clip_packs
            .is_empty());

        // The repository has no second compatible animated GLB fixture with
        // different clips. Keep the exercised successful association typed and
        // in-memory after proving the real admitted GLBs reject their overlap.
        bridge
            .staged
            .as_mut()
            .expect("staged state")
            .state
            .render_resources
            .animated_mesh_mut(pack.value)
            .expect("clip-pack descriptor")
            .clips
            .iter_mut()
            .enumerate()
            .for_each(|(index, clip)| clip.id = format!("pack-clip-{index}"));
        bridge
            .associate_animation_clip_pack(&associate)
            .expect("typed compatible in-memory clip-pack association");
        let primary_mesh = bridge
            .resource(primary.value)
            .expect("primary resource")
            .animated_mesh()
            .expect("primary descriptor");
        assert_eq!(primary_mesh.clip_packs.len(), 1);
        assert_eq!(
            primary_mesh.clip_packs[0].asset,
            format!("animation-clip-pack/{}", &hash["sha256:".len()..])
        );
        assert_eq!(primary_mesh.clip_packs[0].provenance.source_hash, hash);
        assert_eq!(
            primary_mesh.clip_packs[0].provenance.target_hash,
            primary_mesh
                .content_hash
                .clone()
                .expect("primary content hash")
        );
        let primary_asset = primary_mesh.asset.clone();
        let expected_effective_clip_count =
            primary_mesh.clips.len() + primary_mesh.clip_packs[0].clips.len();
        assert_eq!(
            animation_asset_clips(
                &bridge
                    .staged
                    .as_ref()
                    .expect("staged state")
                    .state
                    .render_resources,
                &primary_asset,
            )
            .len(),
            expected_effective_clip_count,
        );
        let graph_id = b"clip-pack-graph";
        let state_id = b"wave";
        let clip_id = b"pack-clip-0";
        let graph = bridge
            .create_animation_graph(&NativeAnimationGraphCreateRequest {
                resource: primary,
                graph_id: NativeUtf8Slice {
                    bytes: graph_id.as_ptr(),
                    len: graph_id.len(),
                },
                version: 1,
                initial_state_id: NativeUtf8Slice {
                    bytes: state_id.as_ptr(),
                    len: state_id.len(),
                },
            })
            .expect("graph retains the assembled primary mesh");
        bridge
            .define_animation_state(&NativeAnimationStateDefinitionRequest {
                graph,
                state_id: NativeUtf8Slice {
                    bytes: state_id.as_ptr(),
                    len: state_id.len(),
                },
                motion_kind: NativeAnimationMotionKind::Clip,
                clip_a: NativeUtf8Slice {
                    bytes: clip_id.as_ptr(),
                    len: clip_id.len(),
                },
                clip_b: NativeUtf8Slice::default(),
                parameter_id: NativeUtf8Slice::default(),
                minimum_milli: 0,
                maximum_milli: 0,
                speed_milli: 1000,
            })
            .expect("graph state can name an effective clip-pack clip");
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest {
                resource: primary,
            })
            .expect("primary animated appearance");
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 17,
            })
            .expect("animation instance");
        bridge
            .create_animation_controller(NativeAnimationControllerCreateRequest {
                graph,
                instance,
                tick_duration_millis: 16,
            })
            .expect("controller validates the effective clip list");
        let readout = bridge.read_animation().expect("animation readout");
        assert_eq!(readout.admitted_meshes, 1);
        assert_eq!(readout.admitted_clip_packs, 1);
        assert_eq!(readout.retained_clip_pack_associations, 1);
    }

    /// Opens a call and creates an animated appearance with a live animation
    /// controller for object 7. Returns the appearance and the controller.
    fn animated_controller_setup() -> (
        RuntimeAppearanceBridge,
        NativeAppearanceHandle,
        NativeAnimationControllerHandle,
    ) {
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
        (bridge, appearance, controller)
    }

    fn animation_ops(output: &RuntimeAppearanceCallOutput) -> Vec<&'static str> {
        let RuntimeAppearanceCallOutput::Presentation(frame) = output else {
            return Vec::new();
        };
        frame
            .ops
            .iter()
            .filter_map(|op| match op {
                PresentationOp::Animation { op, .. } => Some(match op {
                    render_presentation::AnimationProjectionOp::Create { .. } => "create",
                    render_presentation::AnimationProjectionOp::Update { .. } => "update",
                    render_presentation::AnimationProjectionOp::Destroy { .. } => "destroy",
                }),
                _ => None,
            })
            .collect()
    }

    /// The controller's projected target after the open call.
    fn controller_target(bridge: &RuntimeAppearanceBridge) -> Option<RenderHandle> {
        bridge.staged.as_ref().and_then(|call| {
            call.outputs.iter().rev().find_map(|output| match output {
                RuntimeAppearanceCallOutput::Presentation(frame) => {
                    frame.ops.iter().rev().find_map(|op| match op {
                        PresentationOp::Animation {
                            op:
                                render_presentation::AnimationProjectionOp::Create {
                                    descriptor, ..
                                },
                            ..
                        } => Some(descriptor.target),
                        _ => None,
                    })
                }
                _ => None,
            })
        })
    }

    #[test]
    fn animation_controller_follows_its_target_when_the_target_is_recreated() {
        // #8737 review: a layer change recreates the object with a new
        // renderer handle, and the controller must follow it.
        let (mut bridge, appearance, _controller) = animated_controller_setup();
        let fact = appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("controller target");
        bridge.end_call();

        bridge.begin_call();
        let old = bridge
            .staged_ref()
            .unwrap()
            .state
            .projector
            .object_handle(7);
        let viewmodel = NativeAppearanceFact {
            layer: NativeRenderLayer::Viewmodel,
            ..fact
        };
        bridge
            .stage_changes(&[viewmodel], Some(&[]), &BTreeMap::new())
            .expect("a layer change keeps the controller");
        let new = bridge
            .staged_ref()
            .unwrap()
            .state
            .projector
            .object_handle(7);
        assert_ne!(new, old);
        let outputs = &bridge.staged_ref().unwrap().outputs;
        let order: Vec<_> = outputs
            .iter()
            .map(|output| match output {
                RuntimeAppearanceCallOutput::Frame(_) => vec!["frame"],
                other => animation_ops(other),
            })
            .filter(|ops| !ops.is_empty())
            .collect();
        // The old projection goes before the frame that removes its target;
        // the new one follows the frame that creates the new target.
        assert_eq!(order, vec![vec!["destroy"], vec!["frame"], vec!["create"]]);
        assert_eq!(controller_target(&bridge), new);
        bridge.end_call();

        // Recreating the parent recreates the animated child too.
        bridge.begin_call();
        let parent_appearance = bridge.create_primitive(primitive_request()).unwrap();
        let parent = NativeAppearanceFact {
            object_id: 3,
            appearance: parent_appearance,
            ..appearance_fact(parent_appearance)
        };
        let child = NativeAppearanceFact {
            has_parent_object: true,
            parent_object_id: 3,
            ..viewmodel
        };
        bridge
            .stage_changes(&[parent, child], Some(&[]), &BTreeMap::new())
            .expect("reparent under a new parent");
        bridge.end_call();
        bridge.begin_call();
        let before = bridge
            .staged_ref()
            .unwrap()
            .state
            .projector
            .object_handle(7);
        let hidden_layer_parent = NativeAppearanceFact {
            layer: NativeRenderLayer::Viewmodel,
            ..parent
        };
        bridge
            .stage_changes(&[hidden_layer_parent], Some(&[]), &BTreeMap::new())
            .expect("recreating the parent keeps the child's controller");
        let after = bridge
            .staged_ref()
            .unwrap()
            .state
            .projector
            .object_handle(7);
        assert_ne!(after, before);
        assert_eq!(controller_target(&bridge), after);
        bridge.end_call();
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
                .presentation()
                .len(),
            1
        );
        let setup = bridge.take_staged_call();
        bridge.commit(setup);

        bridge.begin_call();
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("snapshot before teardown");
        assert_eq!(
            bridge
                .destroy_animation_controller(controller)
                .expect_err("snapshot-before-teardown remains rejected")
                .code(),
            "CSHARP_ANIMATION_SNAPSHOT_ORDER"
        );
        bridge.end_call();

        bridge.begin_call();
        bridge
            .destroy_animation_controller(controller)
            .expect("controller teardown");
        bridge
            .destroy_animation_instance(instance)
            .expect("instance teardown after its controller");
        unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }
            .expect("teardown before target-removal snapshot is ordered");
        assert_eq!(
            bridge
                .staged
                .as_ref()
                .expect("controller teardown call")
                .presentation()
                .len(),
            1
        );
        let outputs = &bridge
            .staged
            .as_ref()
            .expect("ordered teardown call")
            .outputs;
        assert!(matches!(
            outputs.first(),
            Some(RuntimeAppearanceCallOutput::Presentation(_))
        ));
        assert!(matches!(
            outputs.last(),
            Some(RuntimeAppearanceCallOutput::Frame(_))
        ));
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
                size_mode: Default::default(),
                blend: Default::default(),
                softness_metres: 0.0,
                sprite: NativeRenderResourceReference::default(),
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
                has_collision: false,
                collision: inert_particle_collision(),
                collision_volumes: std::ptr::null(),
                collision_volumes_len: 0,
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
                    texture: NativeRenderResourceReference::default(),
                    font_kind: NativePresentationFontKind::System,
                    font_asset: NativeRenderResourceReference::default(),
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
                    size_mode: Default::default(),
                    blend: Default::default(),
                    softness_metres: 0.0,
                    sprite: NativeRenderResourceReference::default(),
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
                    has_collision: false,
                    collision: inert_particle_collision(),
                    collision_volumes: std::ptr::null(),
                    collision_volumes_len: 0,
                },
            )
            .expect("direct burst");
        assert_eq!(bridge.presentation_readout().active_billboards, 1);
        assert_eq!(bridge.presentation_readout().active_emitters, 1);
        assert_eq!(bridge.presentation_readout().emitted_bursts, 1);
        let call = bridge.take_staged_call();
        assert_eq!(call.presentation().len(), 4);
        assert!(call
            .presentation()
            .into_iter()
            .all(|frame| frame.validate().is_ok()));
        bridge.commit(call);

        let before = bridge.presentation_readout();
        let baseline = RuntimeAppearanceBridge::snapshot_presentation_state(&bridge.state)
            .expect("retained presentation baseline");
        assert_eq!(baseline.len(), 1);
        assert!(baseline[0].ops.iter().all(|op| !matches!(
            op,
            PresentationOp::Particle {
                op: ParticleProjectionOp::Emit { .. },
                ..
            }
        )));
        assert_eq!(
            baseline[0].ops.len(),
            2,
            "retained billboard and emitter only"
        );
        assert!(matches!(
            baseline[0].ops[0],
            PresentationOp::Billboard {
                op: BillboardProjectionOp::Create { .. },
                ..
            }
        ));
        assert!(matches!(
            baseline[0].ops[1],
            PresentationOp::Particle {
                op: ParticleProjectionOp::Create { .. },
                ..
            }
        ));
        let after = bridge.presentation_readout();
        assert_eq!(after.active_billboards, before.active_billboards);
        assert_eq!(after.active_emitters, before.active_emitters);
        assert_eq!(after.reserved_particles, before.reserved_particles);
        assert_eq!(after.emitted_bursts, before.emitted_bursts);
        assert_eq!(
            after.billboard_diagnostics_len,
            before.billboard_diagnostics_len
        );
        assert_eq!(
            after.particle_diagnostics_len,
            before.particle_diagnostics_len
        );
        let _ = emitter;
    }

    #[test]
    fn rejected_presentation_fact_keeps_bounded_diagnostic_after_call_commit() {
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
            texture: NativeRenderResourceReference::default(),
            font_kind: NativePresentationFontKind::System,
            font_asset: NativeRenderResourceReference::default(),
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
        let initial_call = bridge.take_staged_call();
        bridge.commit(initial_call);
        bridge.begin_call();
        let error = bridge
            .presentation_create_billboard(&descriptor)
            .expect_err("duplicate billboard");
        bridge.record_operation_error(error);
        let facts = bridge.presentation_readout();
        assert_eq!(facts.billboard_diagnostics_len, 1);
        // Copy the borrowed diagnostic before the next call, as generated C# does.
        assert_eq!(unsafe { *facts.billboard_diagnostics }.logical_id, 7);
        let call = bridge.take_staged_call();
        bridge.commit(call);
        assert_eq!(bridge.presentation_readout().billboard_diagnostics_len, 1);
    }

    #[test]
    fn particle_recoverable_admission_diagnostics_use_the_product_sink_once_per_code() {
        let sink =
            RuntimeDiagnosticsSink::new(Default::default()).expect("bounded diagnostics sink");
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        bridge.bind_diagnostics_sink(sink.clone());
        bridge.report_particle_recoverable(
            "CSHARP_PARTICLE_EMISSION_DROPPED",
            "optional particle emission was dropped because the retained particle budget is exhausted",
        );
        bridge.report_particle_recoverable(
            "CSHARP_PARTICLE_EMISSION_DROPPED",
            "optional particle emission was dropped because the retained particle budget is exhausted",
        );
        let snapshot = sink.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(
            snapshot.events[0].code(),
            "CSHARP_PARTICLE_EMISSION_DROPPED"
        );
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
                texture: NativeRenderResourceReference::default(),
                font_kind: NativePresentationFontKind::Asset,
                font_asset: NativeRenderResourceReference {
                    value: font.handle.value,
                },
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
        let call = bridge.take_staged_call();
        assert!(matches!(
            &call.presentation()[0].ops[0],
            render_presentation::PresentationOp::Billboard { op: BillboardProjectionOp::Create { descriptor: BillboardDescriptor { font: BillboardFontRef::Asset { family: resolved_family, .. }, .. }, .. }, .. }
                if resolved_family == "Ui Font"
        ));
    }

    #[test]
    fn structured_billboard_updates_atomically_and_keeps_projector_diagnostic() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let key = b"shield";
        let fallback = b"Shield";
        let meter_id = b"armor";
        let cue_id = b"blessed";
        let cue_label = b"Blessed";
        let font = b"sans-serif";
        let slice = |value: &[u8]| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        let color = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let meters = [NativePresentationBillboardMeter {
            id: slice(meter_id),
            accessible_label_key: slice(key),
            accessible_fallback_text: slice(fallback),
            current: 4.0,
            minimum: 0.0,
            maximum: 6.0,
            has_preview: true,
            preview: 5.0,
            fill_direction: NativePresentationBillboardMeterFillDirection::LeftToRight,
            segments: 2,
            fill: color,
            preview_fill: color,
            back: NativeColor::default(),
            border: color,
        }];
        let cues = [NativePresentationBillboardStatusCue {
            id: slice(cue_id),
            label_key: slice(cue_id),
            label_fallback_text: slice(cue_label),
            has_icon: false,
            icon: NativeRenderResourceReference::default(),
        }];
        let descriptor = || NativePresentationStructuredBillboardDescriptor {
            logical_id: 99,
            anchor: NativePresentationAnchor {
                kind: NativePresentationAnchorKind::World,
                position: NativeVec3::default(),
                entity: 0,
                offset: NativeVec3::default(),
            },
            has_label: true,
            label_key: slice(key),
            label_fallback_text: slice(fallback),
            has_icon: false,
            icon: NativeRenderResourceReference::default(),
            accessible_label_key: slice(key),
            accessible_fallback_text: slice(fallback),
            meters: meters.as_ptr(),
            meters_len: meters.len(),
            status_cues: cues.as_ptr(),
            status_cues_len: cues.len(),
            width_pixels: 120.0,
            spacing_pixels: 4.0,
            alignment: NativePresentationBillboardAlignment::Center,
            style: NativePresentationBillboardStyle {
                opacity: 1.0,
                backing: NativeColor::default(),
                border: color,
                radius_pixels: 3.0,
            },
            layout: NativePresentationBillboardLayout {
                priority: 2,
                sizing: NativePresentationBillboardLayoutSizing::DistanceScaled,
                reference_distance: 8.0,
                minimum_scale: 0.5,
                maximum_scale: 2.0,
                safe_area: NativePresentationBillboardSafeArea {
                    top_pixels: 2.0,
                    right_pixels: 2.0,
                    bottom_pixels: 2.0,
                    left_pixels: 2.0,
                },
                edge_behavior: NativePresentationBillboardEdgeBehavior::Clamp,
                overlap_behavior: NativePresentationBillboardOverlapBehavior::Stack,
            },
            font_kind: NativePresentationFontKind::System,
            font_asset: NativeRenderResourceReference::default(),
            font_family: slice(font),
            height_pixels: 16.0,
            color,
            background: NativeColor::default(),
            max_distance: 100.0,
            layer: NativePresentationBillboardLayer::AlwaysOnTop,
            visible: true,
        };
        bridge.begin_call();
        let owner = bridge
            .presentation_create_structured_billboard(&descriptor())
            .expect("structured create");
        let create = bridge.take_staged_call();
        bridge.commit(create);
        assert_eq!(
            bridge
                .state
                .billboard_projector
                .descriptor(BillboardHandle::new(99))
                .expect("retained indicator")
                .layout
                .as_ref()
                .expect("structured layout")
                .priority,
            2
        );

        bridge.begin_call();
        let mut invalid = descriptor();
        let invalid_meters = [NativePresentationBillboardMeter {
            segments: 0,
            ..meters[0]
        }];
        invalid.meters = invalid_meters.as_ptr();
        let error = bridge
            .presentation_update_structured_billboard(owner, &invalid)
            .expect_err("invalid meter update");
        bridge.record_operation_error(error);
        assert_eq!(bridge.presentation_readout().billboard_diagnostics_len, 1);
        let call = bridge.take_staged_call();
        bridge.commit(call);
        let retained = bridge
            .state
            .billboard_projector
            .descriptor(BillboardHandle::new(99))
            .expect("unchanged retained indicator");
        let BillboardContent::Structured { indicator } = &retained.content else {
            panic!("retained content remains structured");
        };
        assert_eq!(indicator.meters[0].segments, 2);
    }

    #[test]
    fn particle_collision_create_update_emit_and_rejection_preserve_projected_facts() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let signal = b"collision-burst";
        let slice = |value: &[u8]| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        let color = NativeColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
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
        let plane = [NativePresentationParticleCollisionVolume {
            kind: NativePresentationParticleCollisionVolumeKind::Plane,
            normal: NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            offset: 0.0,
            minimum: NativeVec3::default(),
            maximum: NativeVec3::default(),
        }];
        let descriptor = |volumes: &[NativePresentationParticleCollisionVolume]| {
            NativePresentationParticleDescriptor {
                logical_id: 31,
                signal_id: slice(signal),
                anchor: NativePresentationAnchor {
                    kind: NativePresentationAnchorKind::World,
                    position: NativeVec3::default(),
                    entity: 0,
                    offset: NativeVec3::default(),
                },
                visual: NativePresentationParticleVisual::Cube,
                size_mode: Default::default(),
                blend: Default::default(),
                softness_metres: 0.0,
                sprite: NativeRenderResourceReference::default(),
                sprite_frame_count: 0,
                rate_per_second: 1.0,
                burst_count: 2,
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
                seed: 7,
                max_particles: 8,
                visible: true,
                has_collision: true,
                collision: NativePresentationParticleCollision {
                    radius: 0.1,
                    restitution: 0.5,
                    friction: 0.25,
                    maximum_impacts: 3,
                    sleep_speed: 0.5,
                    limit_behavior: NativePresentationParticleCollisionLimitBehavior::Sleep,
                },
                collision_volumes: volumes.as_ptr(),
                collision_volumes_len: volumes.len(),
            }
        };

        bridge.begin_call();
        let owner = bridge
            .presentation_create_emitter(&descriptor(&plane))
            .expect("collision emitter create");
        let create = bridge.take_staged_call();
        bridge.commit(create);
        let retained = bridge
            .state
            .particle_projector
            .descriptor(ParticleEmitterHandle::new(31))
            .expect("retained collision emitter");
        assert!(matches!(
            retained
                .collision
                .as_ref()
                .expect("collision")
                .volumes
                .as_slice(),
            [ParticleCollisionVolume::Plane { .. }]
        ));

        let aabb = [NativePresentationParticleCollisionVolume {
            kind: NativePresentationParticleCollisionVolumeKind::Aabb,
            normal: NativeVec3::default(),
            offset: 0.0,
            minimum: NativeVec3 {
                x: -1.0,
                y: -1.0,
                z: -1.0,
            },
            maximum: NativeVec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        }];
        let mut update = descriptor(&aabb);
        update.collision.limit_behavior = NativePresentationParticleCollisionLimitBehavior::Kill;
        bridge.begin_call();
        bridge
            .presentation_update_emitter(owner, &update)
            .expect("collision emitter update");
        let update_call = bridge.take_staged_call();
        bridge.commit(update_call);
        let retained = bridge
            .state
            .particle_projector
            .descriptor(ParticleEmitterHandle::new(31))
            .expect("updated collision emitter");
        assert!(matches!(
            retained
                .collision
                .as_ref()
                .expect("collision")
                .volumes
                .as_slice(),
            [ParticleCollisionVolume::Aabb { .. }]
        ));
        assert_eq!(
            retained
                .collision
                .as_ref()
                .expect("collision")
                .limit_behavior,
            ParticleCollisionLimitBehavior::Kill
        );

        bridge.begin_call();
        bridge
            .presentation_emit_particles(slice(signal), &update)
            .expect("collision particle emit");
        let emit = bridge.take_staged_call();
        assert!(matches!(
            &emit.presentation()[0].ops[0],
            render_presentation::PresentationOp::Particle {
                op: ParticleProjectionOp::Emit { descriptor, .. },
                ..
            } if matches!(descriptor.collision.as_ref().map(|collision| collision.volumes.as_slice()), Some([ParticleCollisionVolume::Aabb { .. }]))
        ));
        bridge.commit(emit);

        let invalid = [NativePresentationParticleCollisionVolume {
            normal: NativeVec3::default(),
            ..plane[0]
        }];
        bridge.begin_call();
        let error = bridge
            .presentation_update_emitter(owner, &descriptor(&invalid))
            .expect_err("invalid collision update");
        bridge.record_operation_error(error);
        assert_eq!(bridge.presentation_readout().particle_diagnostics_len, 1);
        let call = bridge.take_staged_call();
        bridge.commit(call);
        let retained = bridge
            .state
            .particle_projector
            .descriptor(ParticleEmitterHandle::new(31))
            .expect("collision update remains atomic");
        assert!(matches!(
            retained
                .collision
                .as_ref()
                .expect("collision")
                .volumes
                .as_slice(),
            [ParticleCollisionVolume::Aabb { .. }]
        ));
    }
}

#[cfg(test)]
mod graphics_call_tests {
    use super::*;
    #[test]
    fn a_call_owns_the_graphics_state_so_writes_never_copy_it() {
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        // The first call detaches the state from the idle placeholder once.
        bridge.begin_call();
        bridge.staged.as_mut().unwrap().state.next_appearance += 1;
        bridge.end_call();
        let state = Arc::as_ptr(&bridge.state.0);
        for _ in 0..3 {
            bridge.begin_call();
            bridge.staged.as_mut().unwrap().state.next_appearance += 1;
            assert_eq!(Arc::as_ptr(&bridge.staged.as_ref().unwrap().state.0), state);
            bridge.end_call();
        }
        assert_eq!(Arc::as_ptr(&bridge.state.0), state);
        assert_eq!(bridge.state.next_appearance, 5);
    }
}

#[cfg(test)]
mod light_and_shadow_facts {
    use super::*;

    fn native(kind: NativeLightKind) -> NativeLightDescriptor {
        NativeLightDescriptor {
            kind,
            color: NativeVec3 {
                x: 0.3,
                y: 0.5,
                z: 0.9,
            },
            intensity: 1.5,
            enabled: true,
            position: NativeVec3::default(),
            direction: NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            has_range: true,
            range: 80.0,
            decay: 0.0,
            outer_angle_radians: 0.0,
            penumbra: 0.0,
            shadow_intent: NativeLightShadowIntent::Requested,
            shadow_resolution: 0,
            shadow_priority: 0,
            shadow_soft: false,
            ground_color: NativeVec3 {
                x: 0.4,
                y: 0.3,
                z: 0.2,
            },
        }
    }

    #[test]
    fn a_hemisphere_light_round_trips_its_sky_and_ground_colours() {
        let light = native_light_descriptor(native(NativeLightKind::Hemisphere)).unwrap();
        assert_eq!(
            light,
            LightDescriptor::Hemisphere {
                color: [0.3, 0.5, 0.9],
                ground_color: [0.4, 0.3, 0.2],
                intensity: 1.5,
                enabled: true,
            }
        );
        let readout = native_light_readout(&RuntimeLightFact {
            light_id: 7,
            parent_object_id: None,
            light,
        });
        assert_eq!(readout.descriptor.kind, NativeLightKind::Hemisphere);
        assert_eq!(readout.descriptor.ground_color.x, 0.4);
        assert_eq!(readout.descriptor.color.z, 0.9);
        assert!(
            !readout.descriptor.has_range,
            "a hemisphere light has no range"
        );
        assert_eq!(
            readout.descriptor.shadow_intent,
            NativeLightShadowIntent::Disabled,
            "it casts no shadow"
        );

        let mut dark_ground = native(NativeLightKind::Hemisphere);
        dark_ground.ground_color.x = 1.5;
        assert!(
            native_light_descriptor(dark_ground).is_err(),
            "the ground colour is held to 0..=1 like the sky's"
        );
    }

    #[test]
    fn an_ambient_light_range_is_its_sky_square_and_reads_back() {
        let light = native_light_descriptor(native(NativeLightKind::Ambient)).unwrap();
        assert!(matches!(
            light,
            LightDescriptor::Ambient {
                range: Some(range),
                shadow_intent: LightShadowIntent::Requested,
                ..
            } if range == 80.0
        ));
        let readout = native_light_readout(&RuntimeLightFact {
            light_id: 8,
            parent_object_id: None,
            light,
        });
        assert!(readout.descriptor.has_range);
        assert_eq!(readout.descriptor.range, 80.0);

        let mut unranged = native(NativeLightKind::Ambient);
        unranged.has_range = false;
        let light = native_light_descriptor(unranged).unwrap();
        assert!(matches!(
            light,
            LightDescriptor::Ambient { range: None, .. }
        ));
    }
}

#[cfg(test)]
#[path = "tween_tests.rs"]
mod tween_tests;
