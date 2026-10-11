use super::*;
use crate::composition::{parse_runtime_appearance_catalog, EngineServiceSet};
use crate::operation_diagnostics::{empty_receipt, receipt_codes};
use render_model::RenderDiff;
use runtime_diagnostics::RuntimeDiagnosticsSink;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_ui::RuntimeUiRuntimeBinding;

fn binding() -> RuntimeUiRuntimeBinding {
    RuntimeUiRuntimeBinding::new(
        RuntimeInstanceId::new(1),
        RuntimeGeneration::new(1),
        RuntimeControlRevision::new(1),
    )
}

fn services() -> EngineServiceSet {
    EngineServiceSet::new(
        parse_runtime_appearance_catalog(None).expect("default catalog"),
        BTreeMap::new(),
        None,
        RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
    )
    .expect("service set")
}

fn ground(x: f64, z: f64) -> f32 {
    (2.0 * (x / 9.0).sin() + 1.5 * (z / 5.0).cos()) as f32
}

fn material(api: &NativeEngineApi) -> NativeMaterialHandle {
    let white = NativeColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let mut handle = NativeMaterialHandle::default();
    let request = NativeMaterialRequest {
        texture_scale: NativeVec2::default(),
        texture_offset: NativeVec2::default(),
        stochastic_tiling: 0.0,
        shader: Default::default(),
        triplanar_sharpness: 0.0,
        color: white,
        texture: NativeRenderResourceReference::default(),
        roughness: 1.0,
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
        keep_dry: false,
        wind_bend: 0.0,
        wind_flutter: 0.0,
        water: Default::default(),
        translucent_shadow: false,
    };
    let graphics = &api.graphics;
    assert_eq!(
        unsafe {
            (graphics.create_material)(graphics.context, request, &mut handle, std::ptr::null_mut())
        },
        ABI_OK
    );
    handle
}

/// 8-cell tiles: 1 m cells within 10 m, 4 m within 100 m.
const RINGS: [NativeHeightfieldRing; 2] = [
    NativeHeightfieldRing {
        cell_size: 1.0,
        reach: 10.0,
    },
    NativeHeightfieldRing {
        cell_size: 4.0,
        reach: 100.0,
    },
];

fn create_request(
    materials: &[NativeHeightfieldMaterial],
    focus: NativeHeightfieldFocus,
) -> NativeHeightfieldCreateRequest {
    NativeHeightfieldCreateRequest {
        tile_cells: 8,
        rings: RINGS.as_ptr(),
        rings_len: RINGS.len(),
        materials: materials.as_ptr(),
        materials_len: materials.len(),
        layer: NativeRenderLayer::Scene,
        session: NativeOptionalSpatialSession::default(),
        anchor: NativeWorldOriginGlobalPosition::default(),
        origin: NativeVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        scale: 1.0,
        focus,
        shadow_casting: NativeShadowCasting::Cast,
    }
}

fn point(x: f64, z: f64) -> NativeHeightfieldFocus {
    NativeHeightfieldFocus {
        camera: NativeOptionalCamera::default(),
        x,
        z,
    }
}

fn requests(
    api: &NativeEngineApi,
    heightfield: NativeHeightfieldHandle,
) -> Vec<NativeHeightfieldTileRequest> {
    let mut result = NativeHeightfieldRequestsResult {
        requests: std::ptr::null(),
        requests_len: 0,
    };
    let terrain = &api.heightfield;
    assert_eq!(
        unsafe {
            (terrain.read_requests)(
                terrain.context,
                NativeHeightfieldRequestsRequest {
                    heightfield,
                    max_requests: 0,
                },
                &mut result,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    if result.requests_len == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(result.requests, result.requests_len) }.to_vec()
}

/// Sample and commit a requested tile from `ground`; whether it was taken.
fn commit(
    api: &NativeEngineApi,
    heightfield: NativeHeightfieldHandle,
    request: &NativeHeightfieldTileRequest,
) -> bool {
    let side = 8 + 3;
    let heights: Vec<f32> = (0..side * side)
        .map(|index| {
            let (i, j) = ((index % side) as f64 - 1.0, (index / side) as f64 - 1.0);
            ground(
                request.origin_x + i * request.cell_size,
                request.origin_z + j * request.cell_size,
            )
        })
        .collect();
    let mut receipt = NativeHeightfieldCommitReceipt::default();
    let terrain = &api.heightfield;
    assert_eq!(
        unsafe {
            (terrain.commit_tile)(
                terrain.context,
                &NativeHeightfieldTileCommit {
                    heightfield,
                    ring: request.ring,
                    x: request.x,
                    z: request.z,
                    revision: request.revision,
                    heights: heights.as_ptr(),
                    heights_len: heights.len(),
                    colors: std::ptr::null(),
                    colors_len: 0,
                    layer_weights: std::ptr::null(),
                    layer_weights_len: 0,
                    materials: std::ptr::null(),
                    materials_len: 0,
                },
                &mut receipt,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    receipt.accepted
}

fn read(api: &NativeEngineApi, heightfield: NativeHeightfieldHandle) -> NativeHeightfieldReadout {
    let mut readout = NativeHeightfieldReadout::default();
    let terrain = &api.heightfield;
    assert_eq!(
        unsafe {
            (terrain.read)(
                terrain.context,
                heightfield,
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    readout
}

fn ops(services: &mut EngineServiceSet) -> Vec<RenderDiff> {
    let mut call = services.finish_call().expect("call finishes");
    call.take_output()
        .frames
        .into_iter()
        .flat_map(|frame| frame.ops)
        .collect()
}

#[test]
fn a_heightfield_asks_for_its_tiles_meshes_them_as_the_call_ends_and_draws_its_rings() {
    let mut services = services();
    services.begin_call(binding());
    let api = services.api();
    let ground_material = material(&api);
    let materials = [NativeHeightfieldMaterial {
        index: 0,
        material: ground_material,
    }];
    let mut heightfield = NativeHeightfieldHandle::default();
    let terrain = &api.heightfield;
    assert_eq!(
        unsafe {
            (terrain.create)(
                terrain.context,
                &create_request(&materials, point(3.0, -2.0)),
                &mut heightfield,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let waiting = requests(&api, heightfield);
    // 100 m in 32 m tiles: 4 each side, 9² coarse; 10 m in 32 m region
    // tiles: 1 each side, 3² region tiles of 4² fine tiles.
    assert_eq!(waiting.len(), 81 + 144);
    assert_eq!(waiting[0].ring, 1);
    for request in &waiting {
        assert!(commit(&api, heightfield, request));
    }
    let created = ops(&mut services);
    let defined = created
        .iter()
        .filter(|op| {
            matches!(op, RenderDiff::DefineStaticMesh { asset } if asset.payload.morph.is_some())
        })
        .count();
    assert_eq!(defined, 144, "every fine tile morphs onto the coarse ring");
    let shown = created
        .iter()
        .filter(|op| {
            matches!(op, RenderDiff::CreateStaticMeshInstance { instance, .. } if instance.visible)
        })
        .count();
    assert_eq!(shown, 81 - 9 + 144);
    assert!(created.iter().any(|op| matches!(
        op,
        RenderDiff::SetMorphFocus { focus: Some([x, z]), .. } if *x == 3.0 && *z == -2.0
    )));
    let root = created
        .iter()
        .position(|op| matches!(op, RenderDiff::Create { .. }))
        .unwrap();
    let first_instance = created
        .iter()
        .position(|op| matches!(op, RenderDiff::CreateStaticMeshInstance { .. }))
        .unwrap();
    assert!(root < first_instance);

    // Walking into the next region tile asks only for new tiles.
    services.begin_call(binding());
    let api = services.api();
    let readout = read(&api, heightfield);
    assert!(readout.settled && readout.drawn_tiles == 81 - 9 + 144 && readout.pending_tiles == 0);
    assert!(readout.vertices > 0 && readout.sample_bytes == (81 + 144) * 121 * 4);
    let terrain = &api.heightfield;
    let mut readout = NativeHeightfieldReadout::default();
    assert_eq!(
        unsafe {
            (terrain.set_focus)(
                terrain.context,
                NativeHeightfieldFocusRequest {
                    heightfield,
                    focus: point(35.0, -2.0),
                },
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    assert_eq!(readout.pending_tiles, 9 + 3 * 16);
    let moved = ops(&mut services);
    assert!(moved.iter().any(
        |op| matches!(op, RenderDiff::SetMorphFocus { focus: Some([x, _]), .. } if *x == 35.0)
    ));
    assert!(!moved
        .iter()
        .any(|op| matches!(op, RenderDiff::Destroy { .. })));

    // A pick meets the drawn surface.
    services.begin_call(binding());
    let api = services.api();
    let mut hit = NativeHeightfieldPickResult::default();
    let terrain = &api.heightfield;
    assert_eq!(
        unsafe {
            (terrain.pick)(
                terrain.context,
                NativeHeightfieldPickRequest {
                    heightfield,
                    origin: NativeVec3 {
                        x: 40.25,
                        y: 50.0,
                        z: -5.5,
                    },
                    direction: NativeVec3 {
                        x: 0.0,
                        y: -2.0,
                        z: 0.0,
                    },
                    max_distance: 0.0,
                },
                &mut hit,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    assert!(hit.hit && (hit.x - 40.25).abs() < 1e-5 && (hit.z + 5.5).abs() < 1e-5);
    assert!((hit.height - f64::from(ground(40.25, -5.5))).abs() < 1.0);
    assert!((f64::from(hit.distance) - (50.0 - hit.height)).abs() < 1e-3);

    // The material is held while the heightfield lives.
    let graphics = &api.graphics;
    let mut receipt = empty_receipt();
    assert_eq!(
        unsafe { (graphics.destroy_material)(graphics.context, ground_material, &mut receipt) },
        0
    );
    assert_eq!(receipt_codes(&receipt), ["CSHARP_MATERIAL_IN_USE"]);
    let terrain = &api.heightfield;
    assert_eq!(
        unsafe { (terrain.destroy)(terrain.context, heightfield, std::ptr::null_mut()) },
        ABI_OK
    );
    assert_eq!(
        unsafe {
            (graphics.destroy_material)(graphics.context, ground_material, std::ptr::null_mut())
        },
        ABI_OK
    );
    let destroyed = ops(&mut services);
    assert!(
        destroyed
            .iter()
            .filter(|op| matches!(op, RenderDiff::ReleaseStaticMesh { .. }))
            .count()
            >= 81 - 9 + 144
    );
}

#[test]
fn a_camera_focus_measures_from_where_the_camera_stands_in_the_backdrop() {
    let mut services = services();
    services.begin_call(binding());
    let api = services.api();
    let ground_material = material(&api);
    let descriptor = |z: f32| NativeCameraDescriptor {
        pose: NativeCameraPose {
            position: NativeVec3 { x: 0.0, y: 1.0, z },
            pitch_degrees: 0.0,
            yaw_degrees: 0.0,
        },
        basis_mode: NativeCameraBasisMode::Derived,
        basis: NativeCameraBasis::default(),
        projection: NativeCameraProjection {
            kind: NativeCameraProjectionKind::Perspective,
            fov_y_degrees: 60.0,
            vertical_size: 1.0,
            near: 0.1,
            far: 100.0,
        },
        viewport: NativeCameraViewport {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        viewmodel_fov_y_degrees: 0.0,
    };
    let mut camera = NativeCameraHandle::default();
    let mut heightfield = NativeHeightfieldHandle::default();
    unsafe {
        let view = &api.camera_view;
        assert_eq!(
            (view.create_camera)(
                view.context,
                &descriptor(0.0),
                &mut camera,
                std::ptr::null_mut()
            ),
            ABI_OK
        );
        assert_eq!(
            (view.set_active_camera)(view.context, camera, std::ptr::null_mut()),
            ABI_OK
        );
        assert_eq!(
            (view.set_backdrop)(
                view.context,
                &NativeBackdropRequest {
                    camera: NativeOptionalCamera::default(),
                    session: NativeOptionalSpatialSession::default(),
                    anchor: NativeWorldOriginGlobalPosition::default(),
                    origin: NativeVec3::default(),
                    scale: 1000.0,
                },
                std::ptr::null_mut()
            ),
            ABI_OK
        );
        // Sampled in world metres, drawn at 1:1000 in the backdrop.
        let materials = [NativeHeightfieldMaterial {
            index: 0,
            material: ground_material,
        }];
        let request = NativeHeightfieldCreateRequest {
            layer: NativeRenderLayer::Backdrop,
            scale: 0.001,
            ..create_request(
                &materials,
                NativeHeightfieldFocus {
                    camera: NativeOptionalCamera { camera },
                    x: 0.0,
                    z: 0.0,
                },
            )
        };
        let terrain = &api.heightfield;
        assert_eq!(
            (terrain.create)(
                terrain.context,
                &request,
                &mut heightfield,
                std::ptr::null_mut()
            ),
            ABI_OK
        );
        // Measured as the call finishes: no focus yet.
        assert!(!read(&api, heightfield).focused);
        assert!(requests(&api, heightfield).is_empty());
    }
    services.finish_call().expect("link call");
    services.begin_call(binding());
    let api = services.api();
    let readout = read(&api, heightfield);
    assert!(readout.focused && readout.focus_x == 0.0 && readout.focus_z.abs() < 1e-9);
    unsafe {
        let view = &api.camera_view;
        assert_eq!(
            (view.update_camera)(
                view.context,
                &NativeCameraUpdateRequest {
                    camera,
                    descriptor: descriptor(-250.0),
                },
                std::ptr::null_mut()
            ),
            ABI_OK
        );
    }
    services.finish_call().expect("walk call");
    services.begin_call(binding());
    let api = services.api();
    // 250 m north in the world: 0.25 backdrop units, 250 heightfield metres.
    let readout = read(&api, heightfield);
    assert!(
        (readout.focus_z + 250.0).abs() < 1e-3,
        "{}",
        readout.focus_z
    );
    assert!(!requests(&api, heightfield).is_empty());
    services.finish_call().unwrap();
}

#[test]
fn bad_layouts_unbound_materials_and_stale_samples_are_refused_or_ignored() {
    let mut services = services();
    services.begin_call(binding());
    let api = services.api();
    let ground_material = material(&api);
    let materials = [NativeHeightfieldMaterial {
        index: 0,
        material: ground_material,
    }];
    let terrain = &api.heightfield;
    let refusal = |request: &NativeHeightfieldCreateRequest| {
        let mut heightfield = NativeHeightfieldHandle::default();
        let mut receipt = empty_receipt();
        let status =
            unsafe { (terrain.create)(terrain.context, request, &mut heightfield, &mut receipt) };
        assert_eq!(status, 0);
        receipt_codes(&receipt)
    };
    let short = [
        RINGS[0],
        NativeHeightfieldRing {
            cell_size: 4.0,
            reach: 20.0,
        },
    ];
    assert_eq!(
        refusal(&NativeHeightfieldCreateRequest {
            rings: short.as_ptr(),
            ..create_request(&materials, point(0.0, 0.0))
        }),
        ["CSHARP_HEIGHTFIELD"]
    );
    assert_eq!(
        refusal(&NativeHeightfieldCreateRequest {
            layer: NativeRenderLayer::Ui,
            ..create_request(&materials, point(0.0, 0.0))
        }),
        ["CSHARP_HEIGHTFIELD"]
    );
    let mut heightfield = NativeHeightfieldHandle::default();
    assert_eq!(
        unsafe {
            (terrain.create)(
                terrain.context,
                &create_request(&materials, point(0.0, 0.0)),
                &mut heightfield,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let first = requests(&api, heightfield)[0];
    // A material index nothing binds.
    let side = 8 + 3;
    let heights = vec![0.0_f32; side * side];
    let water = vec![1_u32; 81];
    let mut receipt = empty_receipt();
    let mut taken = NativeHeightfieldCommitReceipt::default();
    let unbound = NativeHeightfieldTileCommit {
        heightfield,
        ring: first.ring,
        x: first.x,
        z: first.z,
        revision: first.revision,
        heights: heights.as_ptr(),
        heights_len: heights.len(),
        colors: std::ptr::null(),
        colors_len: 0,
        layer_weights: std::ptr::null(),
        layer_weights_len: 0,
        materials: water.as_ptr(),
        materials_len: water.len(),
    };
    assert_eq!(
        unsafe { (terrain.commit_tile)(terrain.context, &unbound, &mut taken, &mut receipt) },
        0
    );
    assert_eq!(receipt_codes(&receipt), ["CSHARP_HEIGHTFIELD"]);
    // Invalidated before it lands: ignored, and asked for again.
    let mut readout = NativeHeightfieldReadout::default();
    assert_eq!(
        unsafe {
            (terrain.invalidate)(
                terrain.context,
                NativeHeightfieldInvalidateRequest {
                    heightfield,
                    min_x: -1000.0,
                    min_z: -1000.0,
                    max_x: 1000.0,
                    max_z: 1000.0,
                },
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    assert!(!commit(&api, heightfield, &first));
    let renewed = requests(&api, heightfield)[0];
    assert_ne!(renewed.revision, first.revision);
    assert!(commit(&api, heightfield, &renewed));
    services.finish_call().unwrap();
}
