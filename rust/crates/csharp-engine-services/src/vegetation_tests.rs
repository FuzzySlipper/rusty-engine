use super::*;
use crate::composition::{parse_runtime_appearance_catalog, EngineServiceSet};
use runtime_diagnostics::RuntimeDiagnosticsSink;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_ui::RuntimeUiRuntimeBinding;

const GRASS: u32 = 3;
const TREES: u32 = 4;

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

fn material(api: &NativeEngineApi) -> NativeMaterialHandle {
    let white = NativeColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
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
        double_sided: true,
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
    let mut handle = NativeMaterialHandle::default();
    let graphics = &api.graphics;
    assert_eq!(
        unsafe {
            (graphics.create_material)(graphics.context, request, &mut handle, std::ptr::null_mut())
        },
        ABI_OK
    );
    handle
}

/// A blade: one quad half a metre tall.
fn blade(api: &NativeEngineApi, material: NativeMaterialHandle) -> NativeAppearanceHandle {
    let vec = |x, y, z| NativeVec3 { x, y, z };
    let positions = [
        vec(-0.1, 0.0, 0.0),
        vec(0.1, 0.0, 0.0),
        vec(0.1, 0.5, 0.0),
        vec(-0.1, 0.5, 0.0),
    ];
    let normals = [vec(0.0, 0.0, 1.0); 4];
    let indices = [0u32, 1, 2, 0, 2, 3];
    let groups = [NativeMeshGroup {
        material_slot: 0,
        start: 0,
        count: 6,
    }];
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
        layer_weights: std::ptr::null(),
        layer_weights_len: 0,
    };
    let graphics = &api.graphics;
    let mut resource = NativeMeshResourceHandle::default();
    assert_eq!(
        unsafe {
            (graphics.create_mesh_resource)(
                graphics.context,
                &request,
                &mut resource,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let mut appearance = NativeAppearanceHandle::default();
    assert_eq!(
        unsafe {
            (graphics.create_mesh_appearance)(
                graphics.context,
                resource,
                &mut appearance,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    appearance
}

fn create(api: &NativeEngineApi, x: f64, z: f64) -> NativeVegetationFieldHandle {
    let vegetation = &api.vegetation;
    let mut field = NativeVegetationFieldHandle::default();
    assert_eq!(
        unsafe {
            (vegetation.create)(
                vegetation.context,
                NativeVegetationFieldCreateRequest {
                    session: NativeOptionalSpatialSession::default(),
                    tile_size: 16.0,
                    tile_cells: 8,
                    placements_per_call: 64,
                    focus: NativeVegetationFocus {
                        camera: NativeOptionalCamera::default(),
                        x,
                        y: 2.0,
                        z,
                    },
                },
                &mut field,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    field
}

fn layer(
    field: NativeVegetationFieldHandle,
    key: u32,
    appearance: NativeAppearanceHandle,
    material: NativeMaterialHandle,
    density: f32,
) -> NativeVegetationLayerRequest {
    NativeVegetationLayerRequest {
        field,
        layer: key,
        appearance,
        material,
        density,
        grounds: 1,
        slope_limit_degrees: 40.0,
        align: 0.3,
        scale_min: 0.8,
        scale_max: 1.2,
        tint_low: NativeVec3 {
            x: 0.8,
            y: 0.8,
            z: 0.8,
        },
        tint_high: NativeVec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        seed: 9,
        reach: 30.0,
        fade: 4.0,
        near: 0.0,
        near_fade: 0.0,
        maximum_instances: 100_000,
        casts_shadows: false,
    }
}

fn set_layer(
    api: &NativeEngineApi,
    request: NativeVegetationLayerRequest,
) -> NativeVegetationReadout {
    let vegetation = &api.vegetation;
    let mut readout = NativeVegetationReadout::default();
    assert_eq!(
        unsafe {
            (vegetation.set_layer)(
                vegetation.context,
                request,
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    readout
}

fn requests(
    api: &NativeEngineApi,
    field: NativeVegetationFieldHandle,
) -> Vec<NativeVegetationTileRequest> {
    let vegetation = &api.vegetation;
    let mut result = NativeVegetationRequestsResult {
        requests: std::ptr::null(),
        requests_len: 0,
    };
    assert_eq!(
        unsafe {
            (vegetation.read_requests)(
                vegetation.context,
                NativeVegetationRequestsRequest {
                    field,
                    max_requests: 0,
                },
                &mut result,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    unsafe { std::slice::from_raw_parts(result.requests, result.requests_len) }.to_vec()
}

/// Flat ground 2 m up, class 0; the tile at the origin holds one tree.
fn commit(
    api: &NativeEngineApi,
    field: NativeVegetationFieldHandle,
    request: &NativeVegetationTileRequest,
    height: f32,
) -> bool {
    let heights = vec![height; ((request.tile_cells + 3) * (request.tile_cells + 3)) as usize];
    let grounds = vec![0u32; ((request.tile_cells + 1) * (request.tile_cells + 1)) as usize];
    let records: Vec<NativeVegetationRecord> = if (request.x, request.z) == (0, 0) {
        vec![NativeVegetationRecord {
            layer: TREES,
            position: NativeVec3 {
                x: 5.0,
                y: height,
                z: 6.0,
            },
            yaw: 0.5,
            scale: 3.0,
        }]
    } else {
        Vec::new()
    };
    let vegetation = &api.vegetation;
    let mut receipt = NativeVegetationCommitReceipt::default();
    assert_eq!(
        unsafe {
            (vegetation.commit_tile)(
                vegetation.context,
                &NativeVegetationTileCommit {
                    field,
                    x: request.x,
                    z: request.z,
                    revision: request.revision,
                    heights: heights.as_ptr(),
                    heights_len: heights.len(),
                    grounds: grounds.as_ptr(),
                    grounds_len: grounds.len(),
                    cover: std::ptr::null(),
                    cover_len: 0,
                    records: records.as_ptr(),
                    records_len: records.len(),
                },
                &mut receipt,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    receipt.accepted
}

fn read(api: &NativeEngineApi, field: NativeVegetationFieldHandle) -> NativeVegetationReadout {
    let vegetation = &api.vegetation;
    let mut readout = NativeVegetationReadout::default();
    assert_eq!(
        unsafe {
            (vegetation.read)(
                vegetation.context,
                field,
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    readout
}

/// The call's renderer work in order: early removals and Graphics' frames,
/// then the services'.
fn ops(services: &mut EngineServiceSet) -> Vec<RenderDiff> {
    let mut call = services.finish_call().expect("call finishes");
    let output = call.take_output();
    output
        .appearance
        .into_iter()
        .filter_map(|output| match output {
            crate::composition::CsharpAppearanceCallOutput::Frame(frame) => Some(frame),
            crate::composition::CsharpAppearanceCallOutput::Presentation(_) => None,
        })
        .chain(output.frames)
        .flat_map(|frame| frame.ops)
        .collect()
}

#[test]
fn a_field_asks_for_tiles_places_its_layers_as_the_call_ends_and_redoes_only_edited_tiles() {
    let mut services = services();
    services.begin_call(binding());
    let api = services.api();
    let paint = material(&api);
    let look = blade(&api, paint);
    let field = create(&api, 8.0, 8.0);
    set_layer(&api, layer(field, GRASS, look, paint, 1.0));
    set_layer(&api, layer(field, TREES, look, paint, 0.0));
    // Tiles are asked for once the call settles the focus.
    assert!(requests(&api, field).is_empty());
    let first = ops(&mut services);
    assert!(first.iter().any(|operation| matches!(
        operation,
        RenderDiff::DefineMaterial { material } if material.id == "material/vegetation-1-3"
    )));

    services.begin_call(binding());
    let api = services.api();
    let waiting = requests(&api, field);
    assert!(!waiting.is_empty());
    assert_eq!((waiting[0].x, waiting[0].z), (0, 0));
    assert_eq!(waiting[0].cell_size, 2.0);
    for request in &waiting {
        assert!(commit(&api, field, request, 2.0));
    }
    let placed = ops(&mut services);
    let patches: Vec<&render_model::ScatterPatchDescriptor> = placed
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::CreateScatterPatch { patch, .. } => Some(patch),
            _ => None,
        })
        .collect();
    assert!(patches.len() > waiting.len());
    let tree = patches
        .iter()
        .find(|patch| patch.instances.len() == 1)
        .expect("the tree's patch");
    assert_eq!(tree.instances[0].translation, [5.0, 2.0, 6.0]);
    assert_eq!(tree.instances[0].scale, 3.0);

    services.begin_call(binding());
    let api = services.api();
    let before = read(&api, field);
    assert!(before.instances > 0 && before.pending_tiles == 0 && before.deferred == 0);
    assert!(before.placement_microseconds > 0 || before.placed > 0);
    let vegetation = &api.vegetation;
    let mut readout = NativeVegetationReadout::default();
    assert_eq!(
        unsafe {
            (vegetation.invalidate)(
                vegetation.context,
                NativeVegetationInvalidateRequest {
                    field,
                    min_x: 20.0,
                    min_z: 4.0,
                    max_x: 21.0,
                    max_z: 5.0,
                },
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let again = requests(&api, field);
    assert_eq!(again.len(), 1);
    assert!(commit(&api, field, &again[0], 3.0));
    let edited = ops(&mut services);
    let destroyed = edited
        .iter()
        .filter(|operation| matches!(operation, RenderDiff::Destroy { .. }))
        .count();
    let created = edited
        .iter()
        .filter(|operation| matches!(operation, RenderDiff::CreateScatterPatch { .. }))
        .count();
    assert_eq!((destroyed, created), (1, 1));

    // Removing a layer removes its patches ahead of the call's Graphics work.
    services.begin_call(binding());
    let api = services.api();
    let vegetation = &api.vegetation;
    let mut readout = NativeVegetationReadout::default();
    assert_eq!(
        unsafe {
            (vegetation.remove_layer)(
                vegetation.context,
                NativeVegetationLayerRemoval {
                    field,
                    layer: GRASS,
                },
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let removed = ops(&mut services);
    assert!(!removed.is_empty());
    assert!(removed
        .iter()
        .all(|operation| matches!(operation, RenderDiff::Destroy { .. })));
    services.begin_call(binding());
    let api = services.api();
    let after = read(&api, field);
    assert_eq!(after.instances, 1);
    let _ = ops(&mut services);
}

#[test]
fn refuses_a_bad_layer_and_a_dead_field() {
    let mut services = services();
    services.begin_call(binding());
    let api = services.api();
    let paint = material(&api);
    let look = blade(&api, paint);
    let field = create(&api, 0.0, 0.0);
    let vegetation = &api.vegetation;
    let mut readout = NativeVegetationReadout::default();
    let mut bad = layer(field, GRASS, look, paint, 1.0);
    bad.near = 28.0;
    let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    assert_ne!(
        unsafe { (vegetation.set_layer)(vegetation.context, bad, &mut readout, &mut error) },
        ABI_OK
    );
    assert_ne!(
        unsafe {
            (vegetation.read)(
                vegetation.context,
                NativeVegetationFieldHandle { value: 99 },
                &mut readout,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let _ = ops(&mut services);
}
