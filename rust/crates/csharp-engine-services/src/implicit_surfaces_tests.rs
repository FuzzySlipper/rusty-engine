use super::{api as implicit_api, RuntimeImplicitBridge};
use crate::{
    appearance::{self, RuntimeAppearanceBridge},
    composition::ABI_OK,
};
use csharp_engine_abi::*;
use render_projection::RuntimeAppearanceCatalog;
use std::collections::BTreeMap;

fn opaque_material() -> NativeMaterialRequest {
    NativeMaterialRequest {
        color: NativeColor {
            r: 0.35,
            g: 0.55,
            b: 0.2,
            a: 1.0,
        },
        texture: NativeRenderResourceHandle::default(),
        roughness: 0.85,
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
    }
}

fn fact(object_id: u64, appearance: NativeAppearanceHandle) -> NativeAppearanceFact {
    NativeAppearanceFact {
        object_id,
        has_parent_object: false,
        parent_object_id: 0,
        transform: NativeTransform {
            translation: NativeVec3::default(),
            rotation: NativeQuat {
                w: 1.0,
                ..NativeQuat::default()
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
    }
}

#[test]
fn native_implicit_mesh_generation_keeps_renderer_owners_alive_until_released() {
    let mut appearance =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
    let mut implicit = RuntimeImplicitBridge::new();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let appearance_context = (&mut appearance as *mut RuntimeAppearanceBridge).cast();

    let mut material = NativeMaterialHandle::default();
    assert_eq!(
        unsafe {
            appearance::create_material(appearance_context, opaque_material(), &mut material)
        },
        ABI_OK
    );

    let mut field = NativeImplicitFieldHandle { value: 0 };
    assert_eq!(
        unsafe { (api.create_field)(api.context, &mut field) },
        ABI_OK
    );
    let mut sphere = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_sphere)(
                api.context,
                NativeImplicitSphereRequest {
                    field,
                    center: NativeVec3::default(),
                    radius: 0.8,
                },
                &mut sphere,
            )
        },
        ABI_OK
    );
    let mut opening = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_box)(
                api.context,
                NativeImplicitBoxRequest {
                    field,
                    minimum: NativeVec3 {
                        x: -0.2,
                        y: -1.0,
                        z: -1.0,
                    },
                    maximum: NativeVec3 {
                        x: 0.2,
                        y: 1.0,
                        z: 1.0,
                    },
                },
                &mut opening,
            )
        },
        ABI_OK
    );
    let mut carved = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.difference)(
                api.context,
                NativeImplicitBinaryRequest {
                    field,
                    left: sphere,
                    right: opening,
                },
                &mut carved,
            )
        },
        ABI_OK
    );

    // The region makes the opt-in spacing exercise the surface-material path,
    // rather than merely accepting an unused option.
    let material_regions = [NativeImplicitMaterialRegion {
        node: sphere,
        material,
    }];
    let mut mesh = NativeMeshResourceHandle::default();
    let mut generate_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &NativeImplicitGenerateRequest {
                    field,
                    source: carved,
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
                    cell_size: 0.16,
                    crease_angle_degrees: 35.0,
                    uv_scale: 1.0,
                    default_material: material,
                    regions: material_regions.as_ptr(),
                    regions_len: material_regions.len(),
                    material_boundary_mode: NativeImplicitMaterialBoundaryMode::Interpolated,
                    material_sample_spacing: 0.001,
                    max_extraction_vertices: 0,
                    max_extraction_triangles: 0,
                },
                &mut mesh,
                &mut generate_error,
            )
        },
        0
    );
    assert_eq!(generate_error.diagnostics.diagnostics_len, 1);
    let diagnostic = unsafe { &*generate_error.diagnostics.diagnostics };
    let message =
        unsafe { std::slice::from_raw_parts(diagnostic.message.bytes, diagnostic.message.len) };
    assert!(std::str::from_utf8(message)
        .unwrap()
        .contains("budget exceeded"));
    assert_eq!(mesh.value, 0, "failed extraction never publishes a mesh");
    let lease = generate_error.diagnostics.handle;
    assert_eq!(
        unsafe { (api.destroy_operation_diagnostic_lease)(api.context, lease) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (api.destroy_operation_diagnostic_lease)(api.context, lease) },
        0
    );
    let mut bounded_request = NativeImplicitGenerateRequest {
        field,
        source: carved,
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
        cell_size: 0.16,
        crease_angle_degrees: 35.0,
        uv_scale: 1.0,
        default_material: material,
        regions: material_regions.as_ptr(),
        regions_len: material_regions.len(),
        material_boundary_mode: NativeImplicitMaterialBoundaryMode::Interpolated,
        material_sample_spacing: 0.08,
        max_extraction_vertices: 0,
        max_extraction_triangles: 1,
    };
    assert_ne!(
        unsafe {
            (api.generate)(
                api.context,
                &bounded_request,
                &mut mesh,
                &mut generate_error,
            )
        },
        ABI_OK
    );
    assert_eq!(mesh.value, 0);
    assert_eq!(
        unsafe {
            (api.destroy_operation_diagnostic_lease)(api.context, generate_error.diagnostics.handle)
        },
        ABI_OK
    );
    bounded_request.max_extraction_triangles = 10_000;
    // A caught budget rejection must allow successful generation and the
    // appearance/implicit callback commit below, without restarting the host.
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &bounded_request,
                &mut mesh,
                &mut generate_error,
            )
        },
        ABI_OK
    );
    let mut generation = NativeImplicitGenerationReadout {
        node_count: 0,
        vertices: 0,
        triangles: 0,
        material_groups: 0,
        octree_depth: 0,
        sample_spacing: 0.0,
        generation_seconds: 0.0,
        reoriented_triangles: 0,
        degenerate_triangles: 0,
        boundary_edges: 0,
        non_manifold_edges: 0,
        inconsistent_winding_edges: 0,
        bounded_leaf_vertices: 0,
    };
    assert_eq!(
        unsafe { (api.read_generation)(api.context, field, &mut generation) },
        ABI_OK
    );
    assert!(generation.vertices > 0 && generation.triangles > 0);
    assert_eq!(generation.material_groups, 1);
    assert_eq!(generation.boundary_edges, 0);
    assert_eq!(generation.non_manifold_edges, 0);
    assert_eq!(generation.inconsistent_winding_edges, 0);
    assert_eq!(generate_error.diagnostics.handle.value, 0);

    let mut first = NativeAppearanceHandle::default();
    let mut second = NativeAppearanceHandle::default();
    assert_eq!(
        unsafe { appearance::create_mesh_appearance(appearance_context, mesh, &mut first) },
        ABI_OK
    );
    assert_eq!(
        unsafe { appearance::create_mesh_appearance(appearance_context, mesh, &mut second) },
        ABI_OK
    );
    let facts = [fact(1, first), fact(2, second)];
    assert_eq!(
        unsafe {
            appearance::publish_appearance_snapshot(appearance_context, facts.as_ptr(), facts.len())
        },
        ABI_OK
    );
    let mut presentation = NativePresentationReadout::default();
    assert_eq!(
        unsafe { appearance::read_presentation(appearance_context, &mut presentation) },
        ABI_OK
    );
    assert_eq!(presentation.retained_object_count, 2);
    assert_eq!(presentation.appearance_count, 2);
    assert_eq!(presentation.material_count, 1);
    assert_eq!(presentation.resource_count, 1);

    let initial_appearance = appearance
        .take_staged_call()
        .expect("initial appearance call")
        .expect("initial appearance state");
    assert!(
        initial_appearance.frame.is_some(),
        "facts emit a retained frame"
    );
    let initial_implicit = implicit.take_call().expect("initial implicit call");
    appearance.commit(Some(initial_appearance));
    implicit.commit_call(initial_implicit);

    // Removing the authoring field is independent from the copied, retained
    // mesh resource and its two consumer appearances.
    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    assert_eq!(unsafe { (api.destroy_field)(api.context, field) }, ABI_OK);
    let after_field_appearance = appearance
        .take_staged_call()
        .expect("field destruction leaves appearance stage valid");
    let after_field_implicit = implicit.take_call().expect("field destruction commits");
    appearance.commit(after_field_appearance);
    implicit.commit_call(after_field_implicit);

    appearance.begin_call();
    implicit.begin_call();
    let appearance_context = (&mut appearance as *mut RuntimeAppearanceBridge).cast();
    assert_eq!(
        unsafe { appearance::destroy_mesh_resource(appearance_context, mesh) },
        0,
        "a mesh cannot be released while either appearance still owns it"
    );
    // ABI failures mark the product call as failed. Start a clean staged call
    // before ordinary teardown so the expected rejection cannot poison it.
    appearance.discard_call();
    implicit.discard_call();

    appearance.begin_call();
    implicit.begin_call();
    let appearance_context = (&mut appearance as *mut RuntimeAppearanceBridge).cast();
    assert_eq!(
        unsafe { appearance::publish_appearance_snapshot(appearance_context, std::ptr::null(), 0) },
        ABI_OK
    );
    assert_eq!(
        unsafe { appearance::destroy_appearance(appearance_context, first) },
        ABI_OK
    );
    assert_eq!(
        unsafe { appearance::destroy_appearance(appearance_context, second) },
        ABI_OK
    );
    assert_eq!(
        unsafe { appearance::destroy_mesh_resource(appearance_context, mesh) },
        ABI_OK
    );
    assert_eq!(
        unsafe { appearance::destroy_material(appearance_context, material) },
        ABI_OK
    );
    let cleanup_appearance = appearance
        .take_staged_call()
        .expect("cleanup appearance call")
        .expect("cleanup appearance state");
    let cleanup_implicit = implicit.take_call().expect("cleanup implicit call");
    appearance.commit(Some(cleanup_appearance));
    implicit.commit_call(cleanup_implicit);
}

#[test]
fn native_sampled_volume_copies_snapshots_and_invalidates_stale_generation() {
    let mut appearance =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
    let mut implicit = RuntimeImplicitBridge::new();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let appearance_context = (&mut appearance as *mut RuntimeAppearanceBridge).cast();

    let mut material = NativeMaterialHandle::default();
    assert_eq!(
        unsafe {
            appearance::create_material(appearance_context, opaque_material(), &mut material)
        },
        ABI_OK
    );
    let mut field = NativeImplicitFieldHandle { value: 0 };
    assert_eq!(
        unsafe { (api.create_field)(api.context, &mut field) },
        ABI_OK
    );
    let mut sphere = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_sphere)(
                api.context,
                NativeImplicitSphereRequest {
                    field,
                    center: NativeVec3::default(),
                    radius: 0.65,
                },
                &mut sphere,
            )
        },
        ABI_OK
    );

    let mut volume = NativeSampledVolumeHandle::default();
    let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    assert_eq!(
        unsafe {
            (api.create_sampled_volume)(
                api.context,
                NativeSampledVolumeCreateRequest {
                    origin: NativeVec3 {
                        x: -1.0,
                        y: -1.0,
                        z: -1.0,
                    },
                    spacing: 0.5,
                    width: 5,
                    height: 5,
                    depth: 5,
                    initial_value: 1.0,
                },
                &mut volume,
                &mut receipt,
            )
        },
        ABI_OK
    );
    assert_ne!(volume.value, 0);
    let mut descriptor = unsafe { std::mem::zeroed::<NativeSampledVolumeDescriptor>() };
    assert_eq!(
        unsafe {
            (api.describe_sampled_volume)(api.context, volume, &mut descriptor, &mut receipt)
        },
        ABI_OK
    );
    assert_eq!(
        [descriptor.width, descriptor.height, descriptor.depth],
        [5; 3]
    );
    assert_eq!(descriptor.revision, 0);

    let edited = [NativeDensitySample { value: -0.25 }];
    let write = NativeSampledVolumeWriteRequest {
        volume,
        start: 0,
        samples: edited.as_ptr(),
        samples_len: edited.len(),
    };
    assert_eq!(
        unsafe { (api.write_sampled_volume)(api.context, &write, &mut receipt) },
        ABI_OK
    );
    let mut snapshot = unsafe { std::mem::zeroed::<NativeDensitySnapshotLease>() };
    assert_eq!(
        unsafe {
            (api.read_sampled_volume)(
                api.context,
                NativeSampledVolumeReadRequest {
                    volume,
                    start: 0,
                    count: 1,
                },
                &mut snapshot,
                &mut receipt,
            )
        },
        ABI_OK
    );
    assert_eq!(snapshot.descriptor.revision, 1);
    assert_eq!(unsafe { (*snapshot.samples).value }, -0.25);
    assert_eq!(
        unsafe { (api.destroy_density_snapshot_lease)(api.context, snapshot.handle) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (api.destroy_density_snapshot_lease)(api.context, snapshot.handle) },
        0
    );

    assert_eq!(
        unsafe {
            (api.rasterize_sampled_volume)(
                api.context,
                NativeSampledVolumeRasterizeRequest {
                    volume,
                    field,
                    source: sphere,
                },
                &mut receipt,
            )
        },
        ABI_OK
    );
    let mut density = NativeDensitySample { value: 0.0 };
    assert_eq!(
        unsafe {
            (api.sample_sampled_volume)(
                api.context,
                NativeSampledVolumeSampleRequest {
                    volume,
                    position: NativeVec3::default(),
                },
                &mut density,
                &mut receipt,
            )
        },
        ABI_OK
    );
    assert!(density.value < 0.0);

    let generate = NativeSampledVolumeGenerateRequest {
        volume,
        field,
        isovalue: 0.0,
        crease_angle_degrees: 35.0,
        uv_scale: 1.0,
        default_material: material,
        regions: std::ptr::null(),
        regions_len: 0,
        material_boundary_mode: NativeImplicitMaterialBoundaryMode::Centroid,
        material_sample_spacing: 0.0,
    };
    let mut mesh = NativeMeshResourceHandle::default();
    assert_eq!(
        unsafe { (api.generate_sampled_volume)(api.context, &generate, &mut mesh, &mut receipt) },
        ABI_OK
    );
    assert_ne!(mesh.value, 0);
    let mut generation = unsafe { std::mem::zeroed::<NativeImplicitGenerationReadout>() };
    assert_eq!(
        unsafe {
            (api.read_sampled_volume_generation)(api.context, volume, &mut generation, &mut receipt)
        },
        ABI_OK
    );
    assert!(generation.vertices > 0 && generation.triangles > 0);

    assert_eq!(
        unsafe { (api.write_sampled_volume)(api.context, &write, &mut receipt) },
        ABI_OK
    );
    assert_eq!(
        unsafe {
            (api.read_sampled_volume_generation)(api.context, volume, &mut generation, &mut receipt)
        },
        0
    );
    assert_ne!(receipt.diagnostics.handle.value, 0);
    assert_eq!(
        unsafe {
            (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
        },
        ABI_OK
    );
    let retained = implicit
        .take_call()
        .expect("expected generation readout rejection leaves the call usable");
    implicit.commit_call(retained);
    appearance.discard_call();
}

#[test]
fn native_implicit_nodes_reject_foreign_and_discarded_tokens() {
    let mut appearance =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
    let mut implicit = RuntimeImplicitBridge::new();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut first_field = NativeImplicitFieldHandle { value: 0 };
    let mut second_field = NativeImplicitFieldHandle { value: 0 };
    assert_eq!(
        unsafe { (api.create_field)(api.context, &mut first_field) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (api.create_field)(api.context, &mut second_field) },
        ABI_OK
    );
    let mut first_node = NativeImplicitNode { value: 0 };
    let mut second_node = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_sphere)(
                api.context,
                NativeImplicitSphereRequest {
                    field: first_field,
                    center: NativeVec3::default(),
                    radius: 0.5,
                },
                &mut first_node,
            )
        },
        ABI_OK
    );
    assert_eq!(
        unsafe {
            (api.add_box)(
                api.context,
                NativeImplicitBoxRequest {
                    field: second_field,
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
                },
                &mut second_node,
            )
        },
        ABI_OK
    );
    let setup_appearance = appearance
        .take_staged_call()
        .expect("setup appearance call");
    let setup_implicit = implicit.take_call().expect("setup implicit call");
    appearance.commit(setup_appearance);
    implicit.commit_call(setup_implicit);

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut sample = NativeImplicitSample { value: 0.0 };
    assert_eq!(
        unsafe {
            (api.sample)(
                api.context,
                NativeImplicitSampleRequest {
                    field: second_field,
                    source: first_node,
                    position: NativeVec3::default(),
                },
                &mut sample,
            )
        },
        0
    );
    appearance.discard_call();
    implicit.discard_call();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut combined = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.union)(
                api.context,
                NativeImplicitBinaryRequest {
                    field: second_field,
                    left: first_node,
                    right: second_node,
                },
                &mut combined,
            )
        },
        0
    );
    appearance.discard_call();
    implicit.discard_call();

    let generate_request = |source, regions, regions_len| NativeImplicitGenerateRequest {
        field: second_field,
        source,
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
        cell_size: 0.5,
        crease_angle_degrees: 0.0,
        uv_scale: 1.0,
        default_material: NativeMaterialHandle::default(),
        regions,
        regions_len,
        material_boundary_mode: NativeImplicitMaterialBoundaryMode::Centroid,
        material_sample_spacing: 0.0,
        max_extraction_vertices: 0,
        max_extraction_triangles: 0,
    };
    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let malformed_request = generate_request(second_node, std::ptr::null(), 1);
    let mut malformed_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    let mut malformed_mesh = NativeMeshResourceHandle::default();
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &malformed_request,
                &mut malformed_mesh,
                &mut malformed_error,
            )
        },
        0
    );
    assert_eq!(malformed_error.diagnostics.handle.value, 0);
    let failure = implicit
        .take_call()
        .err()
        .expect("ABI pointer failure must poison callback");
    assert_eq!(failure.code(), "CSHARP_SPATIAL_POINTER");
    appearance.discard_call();
    implicit.discard_call();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let invalid_sampling_request = NativeImplicitGenerateRequest {
        material_sample_spacing: 0.1,
        ..generate_request(second_node, std::ptr::null(), 0)
    };
    let mut sampling_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &invalid_sampling_request,
                &mut NativeMeshResourceHandle::default(),
                &mut sampling_error,
            )
        },
        0
    );
    assert_eq!(sampling_error.status, 0);
    assert_eq!(sampling_error.diagnostics.diagnostics_len, 1);
    let diagnostic = unsafe { *sampling_error.diagnostics.diagnostics };
    let message = unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(
            diagnostic.message.bytes,
            diagnostic.message.len,
        ))
    }
    .expect("implicit sampling diagnostic is UTF-8");
    assert!(message.contains("requires interpolated material boundaries"));
    assert_eq!(
        unsafe {
            (api.destroy_operation_diagnostic_lease)(api.context, sampling_error.diagnostics.handle)
        },
        ABI_OK
    );
    let retained = implicit
        .take_call()
        .expect("expected sampling rejection leaves the implicit call usable");
    implicit.commit_call(retained);
    appearance.discard_call();
    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut mesh = NativeMeshResourceHandle::default();
    let mut foreign_node_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &generate_request(first_node, std::ptr::null(), 0),
                &mut mesh,
                &mut foreign_node_error,
            )
        },
        0
    );
    assert_ne!(foreign_node_error.diagnostics.handle.value, 0);
    assert_eq!(
        unsafe {
            (api.destroy_operation_diagnostic_lease)(
                api.context,
                foreign_node_error.diagnostics.handle,
            )
        },
        ABI_OK
    );
    appearance.discard_call();
    implicit.discard_call();

    let regions = [NativeImplicitMaterialRegion {
        node: first_node,
        material: NativeMaterialHandle::default(),
    }];
    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut foreign_region_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &generate_request(second_node, regions.as_ptr(), regions.len()),
                &mut mesh,
                &mut foreign_region_error,
            )
        },
        0
    );
    assert_ne!(foreign_region_error.diagnostics.handle.value, 0);
    assert_eq!(
        unsafe {
            (api.destroy_operation_diagnostic_lease)(
                api.context,
                foreign_region_error.diagnostics.handle,
            )
        },
        ABI_OK
    );
    appearance.discard_call();
    implicit.discard_call();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut discarded_node = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_sphere)(
                api.context,
                NativeImplicitSphereRequest {
                    field: second_field,
                    center: NativeVec3::default(),
                    radius: 0.25,
                },
                &mut discarded_node,
            )
        },
        ABI_OK
    );
    appearance.discard_call();
    implicit.discard_call();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut fresh_node = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_sphere)(
                api.context,
                NativeImplicitSphereRequest {
                    field: second_field,
                    center: NativeVec3::default(),
                    radius: 0.25,
                },
                &mut fresh_node,
            )
        },
        ABI_OK
    );
    assert_ne!(discarded_node.value, fresh_node.value);
    assert_eq!(
        unsafe {
            (api.sample)(
                api.context,
                NativeImplicitSampleRequest {
                    field: second_field,
                    source: discarded_node,
                    position: NativeVec3::default(),
                },
                &mut sample,
            )
        },
        0
    );
}

#[test]
fn native_implicit_frustum_samples_taper_with_start_to_end_orientation() {
    let mut appearance =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
    let mut implicit = RuntimeImplicitBridge::new();

    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);

    let mut field = NativeImplicitFieldHandle { value: 0 };
    assert_eq!(
        unsafe { (api.create_field)(api.context, &mut field) },
        ABI_OK
    );
    let mut frustum = NativeImplicitNode { value: 0 };
    assert_eq!(
        unsafe {
            (api.add_frustum)(
                api.context,
                NativeImplicitFrustumRequest {
                    field,
                    start: NativeVec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    end: NativeVec3 {
                        x: 0.0,
                        y: 2.0,
                        z: 0.0,
                    },
                    // Zero start radius exercises the reusable cone form.
                    start_radius: 0.0,
                    end_radius: 1.0,
                },
                &mut frustum,
            )
        },
        ABI_OK
    );

    let sample = |position| {
        let mut value = NativeImplicitSample { value: 0.0 };
        assert_eq!(
            unsafe {
                (api.sample)(
                    api.context,
                    NativeImplicitSampleRequest {
                        field,
                        source: frustum,
                        position,
                    },
                    &mut value,
                )
            },
            ABI_OK
        );
        value.value
    };

    assert!(
        sample(NativeVec3 {
            x: 0.7,
            y: 0.25,
            z: 0.0,
        }) > 0.0,
        "the cone is outside near its tip at this radius"
    );
    assert!(
        sample(NativeVec3 {
            x: 0.7,
            y: 1.75,
            z: 0.0,
        }) < 0.0,
        "the cone is inside near its wide end at the same radius"
    );
    assert!(
        sample(NativeVec3 {
            x: 1.1,
            y: 1.75,
            z: 0.0,
        }) > 0.0,
        "a point beyond the end radius is outside"
    );
}
