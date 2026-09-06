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

    let mut mesh = NativeMeshResourceHandle::default();
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
                    regions: std::ptr::null(),
                    regions_len: 0,
                    material_boundary_mode: NativeImplicitMaterialBoundaryMode::Interpolated,
                },
                &mut mesh,
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
    };
    assert_eq!(
        unsafe { (api.read_generation)(api.context, field, &mut generation) },
        ABI_OK
    );
    assert!(generation.vertices > 0 && generation.triangles > 0);
    assert_eq!(generation.material_groups, 1);

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
    };
    appearance.begin_call();
    implicit.begin_call();
    let api = implicit_api(&mut implicit, &mut appearance);
    let mut mesh = NativeMeshResourceHandle::default();
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &generate_request(first_node, std::ptr::null(), 0),
                &mut mesh,
            )
        },
        0
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
    assert_eq!(
        unsafe {
            (api.generate)(
                api.context,
                &generate_request(second_node, regions.as_ptr(), regions.len()),
                &mut mesh,
            )
        },
        0
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
