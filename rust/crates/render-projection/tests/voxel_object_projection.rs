use std::collections::BTreeMap;

use render_model::{
    MaterialUvStrategy, MeshMaterialSlot, MeshPayloadSource, RenderDiff, RenderMaterialDescriptor,
    RenderMetadata, TextureFilter, TextureWrap, Transform, VoxelSurfaceAlphaModeDescriptor,
    VoxelSurfaceDescriptor, VoxelSurfaceMappingDescriptor,
};
use render_projection::{VoxelObjectProjectionInstance, VoxelObjectRenderProjector};
use svc_mesh::SurfaceMode;
use voxel_asset::{
    with_computed_voxel_object_hashes, VoxelAssetBounds, VoxelAssetMaterialBinding,
    VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame, VoxelObjectAnimationFrame,
    VoxelObjectAsset, VoxelObjectClip, VoxelObjectGrid, VoxelObjectProvenance,
    VoxelObjectProvenanceKind, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    VOXEL_OBJECT_SCHEMA_VERSION,
};
use voxel_object_runtime::{
    admit_voxel_object, admit_voxel_object_with_options, VoxelObjectAdmissionOptions,
    VoxelObjectLoopMode, VoxelObjectPlaybackStatus, VoxelObjectPlayer, VoxelObjectRuntimeLimits,
};

#[test]
fn frame_only_projection_reuses_cached_resource_without_materialization() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut instances = vec![instance(&object, 0)];
    let created = projector.project(&instances, &materials()).unwrap();
    let handle = projector.handle("runner").unwrap();
    assert!(matches!(
        created.frame.ops[0],
        RenderDiff::DefineMaterial { .. }
    ));
    assert!(created
        .frame
        .ops
        .iter()
        .any(|op| matches!(op, RenderDiff::DefineVoxelObject { .. })));
    assert!(created
        .frame
        .ops
        .iter()
        .any(|op| matches!(op, RenderDiff::CreateVoxelObjectInstance { .. })));
    assert_eq!(
        created.readout.materialized_resources,
        vec!["voxel-object/runner"]
    );

    instances[0].frame = 1;
    let swapped = projector.project(&instances, &materials()).unwrap();
    assert_eq!(projector.handle("runner"), Some(handle));
    assert!(swapped.readout.materialized_resources.is_empty());
    assert_eq!(swapped.frame.ops.len(), 1);
    assert!(matches!(
        swapped.frame.ops[0],
        RenderDiff::SetVoxelObjectFrame { frame: 1, .. }
    ));

    let released = projector.project(&[], &BTreeMap::new()).unwrap();
    assert!(matches!(released.frame.ops[0], RenderDiff::Destroy { .. }));
    assert!(matches!(
        released.frame.ops[1],
        RenderDiff::ReleaseVoxelObject { .. }
    ));
}

#[test]
fn scrubbed_player_frame_drives_renderer_neutral_presentation_only() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut instances = vec![instance(&object, 0)];
    projector.project(&instances, &materials()).unwrap();

    let mut player = VoxelObjectPlayer::new();
    player
        .scrub(&object, "walk", 1, VoxelObjectLoopMode::Repeat)
        .unwrap();
    let sample = player.sample_at(&object, 8_000_000).unwrap();
    assert_eq!(sample.status, VoxelObjectPlaybackStatus::Paused);
    assert_eq!(sample.clip_frame, Some(1));
    assert_eq!(sample.frame, 2);

    instances[0].frame = sample.frame;
    let presented = projector.project(&instances, &materials()).unwrap();
    assert_eq!(presented.frame.ops.len(), 1);
    assert!(matches!(
        presented.frame.ops[0],
        RenderDiff::SetVoxelObjectFrame { frame: 2, .. }
    ));
    assert!(presented.readout.materialized_resources.is_empty());
}

#[test]
fn shared_instances_materialize_one_resource() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let instances = vec![
        named_instance(&object, 0, "runner-a"),
        named_instance(&object, 1, "runner-b"),
    ];
    let created = projector.project(&instances, &materials()).unwrap();
    assert_eq!(
        created.readout.materialized_resources,
        vec!["voxel-object/runner"]
    );
    assert_eq!(
        created
            .frame
            .ops
            .iter()
            .filter(|operation| matches!(operation, RenderDiff::DefineVoxelObject { .. }))
            .count(),
        1
    );

    let unchanged = projector.project(&instances, &materials()).unwrap();
    assert!(unchanged.readout.materialized_resources.is_empty());
    assert!(unchanged.frame.ops.is_empty());
}

/// An instance id named twice takes its last entry. A material that left
/// use is defined again when an instance uses it again.
#[test]
fn a_repeated_instance_takes_its_last_entry() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut moved = instance(&object, 1);
    moved.transform.translation = [2.0, 0.0, 0.0];
    let created = projector
        .project(&[instance(&object, 0), moved], &materials())
        .unwrap();
    let created_instances: Vec<_> = created
        .frame
        .ops
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::CreateVoxelObjectInstance { instance, .. } => Some(instance),
            _ => None,
        })
        .collect();
    assert_eq!(created_instances.len(), 1);
    assert_eq!(created_instances[0].frame, 1);
    assert_eq!(created_instances[0].transform.translation, [2.0, 0.0, 0.0]);

    projector.project(&[], &materials()).unwrap();
    let again = projector
        .project(&[instance(&object, 0)], &materials())
        .unwrap();
    assert!(matches!(
        again.frame.ops[0],
        RenderDiff::DefineMaterial { .. }
    ));
}

/// An overwritten entry contributes no resource or material: the repeated
/// batch resolves exactly what its last entry alone would, even when the
/// earlier entry names another object or another version of the same one.
#[test]
fn a_repeated_instance_resolves_only_its_last_object() {
    let first = admitted();
    let other = admitted_variant("voxel-object/other", "material/other", "test");
    let revised = admitted_variant("voxel-object/runner", "material/other", "test-v2");
    assert_ne!(first.content_hash(), revised.content_hash());
    let mut all_materials = materials();
    let other_material = material("material/other", [0.1, 0.2, 0.8, 1.0]);
    all_materials.insert(other_material.id.clone(), other_material);

    for survivor in [&other, &revised] {
        let repeated = VoxelObjectRenderProjector::new()
            .project(
                &[instance(&first, 0), instance(survivor, 0)],
                &all_materials,
            )
            .unwrap();
        let alone = VoxelObjectRenderProjector::new()
            .project(&[instance(survivor, 0)], &all_materials)
            .unwrap();
        assert_eq!(repeated.frame, alone.frame);
        assert_eq!(repeated.readout, alone.readout);
        assert_eq!(
            repeated.readout.materialized_resources,
            vec![survivor.asset_id().to_string()]
        );
        assert_eq!(
            repeated.readout.resource_hashes[survivor.asset_id()],
            survivor.content_hash()
        );
        let defined_materials = repeated
            .frame
            .ops
            .iter()
            .filter_map(|operation| match operation {
                RenderDiff::DefineMaterial { material } => Some(material.id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(defined_materials, vec!["material/other"]);
    }
}

#[test]
fn packed_projection_moves_mesh_streams_out_of_the_control_frame() {
    let object = admitted();
    let instances = vec![instance(&object, 0)];
    let inline = VoxelObjectRenderProjector::new()
        .project(&instances, &materials())
        .unwrap();
    let mut packed_projector = VoxelObjectRenderProjector::with_packed_mesh_resources();
    let packed = packed_projector.project(&instances, &materials()).unwrap();

    assert_eq!(packed.mesh_resources.len(), 1);
    packed.mesh_resources[0].validate().unwrap();
    let render_asset = packed
        .frame
        .ops
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::DefineVoxelObject { asset } => Some(asset),
            _ => None,
        })
        .unwrap();
    assert!(render_asset.meshes.iter().all(|mesh| matches!(
        mesh.payload.source,
        MeshPayloadSource::Resource {
            encoding: render_model::MeshResourceEncoding::PackedStreamsLeV2,
            uvs_byte_offset: Some(_),
            ..
        }
    )));
    assert!(
        serde_json::to_vec(&packed.frame).unwrap().len()
            < serde_json::to_vec(&inline.frame).unwrap().len()
    );

    let unchanged = packed_projector.project(&instances, &materials()).unwrap();
    assert!(unchanged.mesh_resources.is_empty());
    assert!(unchanged.frame.ops.is_empty());
}

#[test]
fn surface_mode_redefinition_keeps_handle_and_replaces_live_geometry() {
    let greedy = admitted();
    let marching = admitted_with_surface(SurfaceMode::MarchingCubes);
    let dual = admitted_with_surface(SurfaceMode::DualContouring);
    let mut projector = VoxelObjectRenderProjector::new();
    projector
        .project(&[instance(&greedy, 0)], &materials())
        .unwrap();
    let handle = projector.handle("runner").unwrap();

    for (object, mode) in [
        (&marching, SurfaceMode::MarchingCubes),
        (&dual, SurfaceMode::DualContouring),
    ] {
        let replaced = projector
            .project(&[instance(object, 0)], &materials())
            .unwrap();
        assert_eq!(projector.handle("runner"), Some(handle));
        assert_eq!(replaced.readout.surface_modes["voxel-object/runner"], mode);
        assert_eq!(
            replaced.readout.materialized_resources,
            vec!["voxel-object/runner"]
        );
        assert!(replaced
            .frame
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::DefineVoxelObject { .. })));
    }
}

#[test]
fn shared_canonical_asset_can_render_distinct_surface_modes_per_instance() {
    let greedy = admitted();
    let marching = admitted_with_surface(SurfaceMode::MarchingCubes);
    let mut projector = VoxelObjectRenderProjector::new();

    let projected = projector
        .project(
            &[
                named_instance(&greedy, 0, "greedy-runner"),
                named_instance(&marching, 0, "marching-runner"),
            ],
            &materials(),
        )
        .unwrap();

    assert_eq!(projected.readout.surface_modes.len(), 2);
    assert_eq!(
        projected.readout.surface_modes["voxel-object/runner#surface=greedyCubes"],
        SurfaceMode::GreedyCubes
    );
    assert_eq!(
        projected.readout.surface_modes["voxel-object/runner#surface=marchingCubes"],
        SurfaceMode::MarchingCubes
    );
    let defined = projected
        .frame
        .ops
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::DefineVoxelObject { asset } => Some(asset.asset.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        defined,
        vec![
            "voxel-object/runner#surface=greedyCubes",
            "voxel-object/runner#surface=marchingCubes",
        ]
    );
}

#[test]
fn reconstructed_meshes_pack_box_projected_uvs_and_accept_textured_materials() {
    let marching = admitted_with_surface(SurfaceMode::MarchingCubes);
    let instances = [instance(&marching, 0)];
    let packed = VoxelObjectRenderProjector::with_packed_mesh_resources()
        .project(&instances, &materials())
        .unwrap();
    let asset = packed
        .frame
        .ops
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::DefineVoxelObject { asset } => Some(asset),
            _ => None,
        })
        .unwrap();
    assert!(asset.meshes.iter().all(|mesh| matches!(
        mesh.payload.source,
        MeshPayloadSource::Resource {
            encoding: render_model::MeshResourceEncoding::PackedStreamsLeV2,
            uvs_byte_offset: Some(_),
            ..
        }
    )));

    let mut textured = material("material/runner", [0.8, 0.2, 0.1, 1.0]);
    textured.texture = Some("texture/runner-tile".to_string());
    VoxelObjectRenderProjector::new()
        .project(
            &instances,
            &BTreeMap::from([(textured.id.clone(), textured)]),
        )
        .unwrap();
}

#[test]
fn voxel_object_material_override_defines_the_selected_material() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut available_materials = materials();
    let override_material = material("material/override", [0.1, 0.4, 0.9, 1.0]);
    available_materials.insert(override_material.id.clone(), override_material);
    let mut projection_instance = instance(&object, 0);
    // A repeated slot takes its last override; the replaced one is neither
    // resolved nor published.
    projection_instance.material_overrides = ["material/replaced", "material/override"]
        .map(|material| MeshMaterialSlot {
            slot: 1,
            material: material.to_string(),
        })
        .to_vec();

    let projected = projector
        .project(&[projection_instance], &available_materials)
        .unwrap();
    let defined_materials = projected
        .frame
        .ops
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::DefineMaterial { material } => Some(material.id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        defined_materials,
        vec!["material/override", "material/runner"]
    );
    let created = projected
        .frame
        .ops
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::CreateVoxelObjectInstance { instance, .. } => Some(instance),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        created.material_overrides,
        vec![MeshMaterialSlot {
            slot: 1,
            material: "material/override".to_string(),
        }]
    );
}

/// The renderer has no in-place override change, so changing an instance's
/// overrides destroys it and creates it again with the new ones.
#[test]
fn changed_material_overrides_recreate_the_instance() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut available_materials = materials();
    let override_material = material("material/override", [0.1, 0.4, 0.9, 1.0]);
    available_materials.insert(override_material.id.clone(), override_material.clone());
    projector
        .project(&[instance(&object, 0)], &available_materials)
        .unwrap();
    let handle = projector.handle("runner").unwrap();

    let mut overridden = instance(&object, 1);
    overridden.material_overrides = vec![MeshMaterialSlot {
        slot: 1,
        material: override_material.id.clone(),
    }];
    let projected = projector
        .project(&[overridden], &available_materials)
        .unwrap();
    let recreated = projector.handle("runner").unwrap();
    let [RenderDiff::DefineMaterial { material: defined }, RenderDiff::Destroy { handle: destroyed }, RenderDiff::CreateVoxelObjectInstance {
        handle: created,
        instance: created_instance,
        ..
    }] = projected.frame.ops.as_slice()
    else {
        panic!("unexpected ops {:?}", projected.frame.ops);
    };
    assert_eq!(defined, &override_material);
    assert_eq!(*destroyed, handle);
    assert_eq!(*created, recreated);
    assert_eq!(created_instance.frame, 1);
    assert_eq!(
        created_instance.material_overrides,
        vec![MeshMaterialSlot {
            slot: 1,
            material: "material/override".to_string(),
        }]
    );
    assert!(projected.readout.materialized_resources.is_empty());
}

#[test]
fn material_slot_can_switch_from_color_to_tile_without_voxel_republication() {
    let object = admitted();
    let instances = vec![instance(&object, 0)];
    let mut projector = VoxelObjectRenderProjector::new();
    projector.project(&instances, &materials()).unwrap();

    let mut tiled = material("material/runner", [0.8, 0.2, 0.1, 1.0]);
    tiled.texture = Some("texture/runner-tile".to_string());
    tiled.uv_strategy = MaterialUvStrategy::Planar;
    tiled.voxel_surface = Some(VoxelSurfaceDescriptor {
        schema_version: 1,
        filter: TextureFilter::Nearest,
        wrap: TextureWrap::Repeat,
        alpha_mode: VoxelSurfaceAlphaModeDescriptor::Opaque,
        mapping: VoxelSurfaceMappingDescriptor::Repeat {
            texture: "texture/runner-tile".to_string(),
            texture_version: 1,
            texture_content_hash: "aa01".to_string(),
            tile_scale_cells: [1.0, 1.0],
            tile_origin_cells: [0.0, 0.0],
        },
    });
    let updated = projector
        .project(
            &instances,
            &BTreeMap::from([(tiled.id.clone(), tiled.clone())]),
        )
        .unwrap();
    assert_eq!(
        updated.frame.ops,
        vec![RenderDiff::DefineMaterial { material: tiled }]
    );
    assert!(updated.readout.materialized_resources.is_empty());
}

#[test]
fn missing_voxel_object_material_override_is_fail_atomic() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    let mut projection_instance = instance(&object, 0);
    projection_instance.material_overrides = vec![MeshMaterialSlot {
        slot: 1,
        material: "material/missing".to_string(),
    }];

    let error = projector
        .project(&[projection_instance], &materials())
        .unwrap_err();
    assert!(matches!(
        error,
        render_projection::VoxelObjectProjectionError::MissingMaterial { asset }
            if asset == "material/missing"
    ));
    assert_eq!(projector.handle("runner"), None);
}

#[test]
fn invalid_frame_is_fail_atomic() {
    let object = admitted();
    let mut projector = VoxelObjectRenderProjector::new();
    projector
        .project(&[instance(&object, 0)], &materials())
        .unwrap();
    let handle = projector.handle("runner").unwrap();
    let error = projector
        .project(&[instance(&object, 99)], &materials())
        .unwrap_err();
    assert!(matches!(
        error,
        render_projection::VoxelObjectProjectionError::FrameOutOfRange { .. }
    ));
    assert_eq!(projector.handle("runner"), Some(handle));
}

/// The settings of one projected instance in the incremental tests:
/// object, frame, x offset and an optional override for slot 1.
type InstanceEntry<'a> = (
    &'a voxel_object_runtime::AdmittedVoxelObject,
    u32,
    f32,
    Option<&'static str>,
);

fn entry_instance<'a>(
    instance_id: &str,
    entry: InstanceEntry<'a>,
) -> VoxelObjectProjectionInstance<'a> {
    let (object, frame, offset, override_material) = entry;
    let mut instance = named_instance(object, frame, instance_id);
    instance.transform.translation[0] = offset;
    instance.material_overrides = override_material
        .map(|material| MeshMaterialSlot {
            slot: 1,
            material: material.to_string(),
        })
        .into_iter()
        .collect();
    instance
}

/// Puts or removes one instance on `changed`, then projects the whole set
/// on `whole`, and checks both publish the same.
fn expect_whole_projection<'a>(
    changed: &mut VoxelObjectRenderProjector,
    whole: &mut VoxelObjectRenderProjector,
    retained: &mut BTreeMap<String, InstanceEntry<'a>>,
    materials: &BTreeMap<String, RenderMaterialDescriptor>,
    instance_id: &str,
    entry: Option<InstanceEntry<'a>>,
) {
    let (frame, mesh_resources) = match entry {
        Some(entry) => {
            let projected = changed
                .put_instance(&entry_instance(instance_id, entry), materials)
                .unwrap();
            retained.insert(instance_id.to_string(), entry);
            (projected.frame, projected.mesh_resources)
        }
        None => {
            retained.remove(instance_id);
            (changed.remove_instance(instance_id).unwrap(), Vec::new())
        }
    };
    let instances = retained
        .iter()
        .map(|(id, entry)| entry_instance(id, *entry))
        .collect::<Vec<_>>();
    let expected = whole.project(&instances, materials).unwrap();
    assert_eq!(frame, expected.frame, "{instance_id}: {entry:?}");
    assert_eq!(mesh_resources, expected.mesh_resources, "{instance_id}");
    for id in retained.keys() {
        assert_eq!(changed.handle(id), whole.handle(id), "{id}");
    }
}

/// Putting or removing one instance publishes what projecting the whole
/// set again would: shared, replaced and released resources, materials
/// defined, dropped and defined again, rebinding overrides and surface
/// modes. A refused change leaves the projector as it was.
#[test]
fn one_instance_changes_publish_what_a_whole_projection_would() {
    use render_projection::VoxelObjectProjectionError;

    let runner = admitted();
    let revised = admitted_variant("voxel-object/runner", "material/runner", "test-v2");
    let other = admitted_variant("voxel-object/other", "material/other", "test");
    let greedy = admitted_with_surface(SurfaceMode::GreedyCubes);
    let marching = admitted_with_surface(SurfaceMode::MarchingCubes);
    let mut all_materials = materials();
    for material in [
        material("material/other", [0.1, 0.2, 0.8, 1.0]),
        material("material/override", [0.1, 0.4, 0.9, 1.0]),
    ] {
        all_materials.insert(material.id.clone(), material);
    }
    let steps: [(&str, Option<InstanceEntry>); 14] = [
        ("a", Some((&runner, 0, 0.0, None))),
        ("b", Some((&runner, 1, 0.0, None))),
        ("c", Some((&other, 0, 0.0, None))),
        ("a", Some((&runner, 1, 2.0, None))),
        ("b", Some((&runner, 1, 0.0, Some("material/override")))),
        ("c", None),
        ("c", Some((&other, 1, 0.0, None))),
        ("b", None),
        ("a", None),
        ("a", Some((&revised, 0, 0.0, None))),
        ("a", Some((&runner, 0, 0.0, None))),
        ("a", Some((&marching, 0, 0.0, None))),
        ("a", Some((&other, 0, 1.0, None))),
        ("d", Some((&runner, 0, 0.0, None))),
    ];
    for packed in [false, true] {
        let projector = || {
            if packed {
                VoxelObjectRenderProjector::with_packed_mesh_resources()
            } else {
                VoxelObjectRenderProjector::new()
            }
        };
        let (mut changed, mut whole) = (projector(), projector());
        let mut retained = BTreeMap::new();
        for (id, entry) in steps {
            expect_whole_projection(
                &mut changed,
                &mut whole,
                &mut retained,
                &all_materials,
                id,
                entry,
            );
        }

        let refusals: [(&str, InstanceEntry, VoxelObjectProjectionError); 4] = [
            (
                "x",
                (&revised, 0, 0.0, None),
                VoxelObjectProjectionError::ConflictingResource {
                    asset: "voxel-object/runner".to_string(),
                },
            ),
            (
                "x",
                (&marching, 0, 0.0, None),
                VoxelObjectProjectionError::SurfaceModeRegrouping {
                    asset: "voxel-object/runner".to_string(),
                },
            ),
            (
                "d",
                (&runner, 9, 0.0, None),
                VoxelObjectProjectionError::FrameOutOfRange {
                    instance: "d".to_string(),
                    frame: 9,
                    frame_count: 3,
                },
            ),
            (
                "d",
                (&runner, 0, 0.0, Some("material/missing")),
                VoxelObjectProjectionError::MissingMaterial {
                    asset: "material/missing".to_string(),
                },
            ),
        ];
        for (id, entry, error) in refusals {
            assert_eq!(
                changed.put_instance(&entry_instance(id, entry), &all_materials),
                Err(error)
            );
        }
        expect_whole_projection(
            &mut changed,
            &mut whole,
            &mut retained,
            &all_materials,
            "d",
            Some((&runner, 1, 3.0, Some("material/override"))),
        );

        // A projection with two surface modes of one object, then changes
        // that keep it so; one that would merge them is refused.
        let mut retained = BTreeMap::from([
            ("g".to_string(), (&greedy, 0, 0.0, None)),
            ("m".to_string(), (&marching, 0, 0.0, None)),
        ]);
        let instances = retained
            .iter()
            .map(|(id, entry)| entry_instance(id, *entry))
            .collect::<Vec<_>>();
        let (mut changed, mut whole) = (projector(), projector());
        changed.project(&instances, &all_materials).unwrap();
        whole.project(&instances, &all_materials).unwrap();
        for (id, entry) in [
            ("g", Some((&greedy, 1, 0.0, None))),
            ("m2", Some((&marching, 1, 0.0, None))),
            ("m", None),
        ] {
            expect_whole_projection(
                &mut changed,
                &mut whole,
                &mut retained,
                &all_materials,
                id,
                entry,
            );
        }
        assert_eq!(
            changed.remove_instance("m2"),
            Err(VoxelObjectProjectionError::SurfaceModeRegrouping {
                asset: "voxel-object/runner".to_string(),
            })
        );
        expect_whole_projection(
            &mut changed,
            &mut whole,
            &mut retained,
            &all_materials,
            "g",
            Some((&greedy, 0, 1.0, None)),
        );
    }
}

fn instance(
    object: &voxel_object_runtime::AdmittedVoxelObject,
    frame: u32,
) -> VoxelObjectProjectionInstance<'_> {
    named_instance(object, frame, "runner")
}

fn named_instance<'a>(
    object: &'a voxel_object_runtime::AdmittedVoxelObject,
    frame: u32,
    instance_id: &str,
) -> VoxelObjectProjectionInstance<'a> {
    VoxelObjectProjectionInstance {
        instance_id: instance_id.to_string(),
        object,
        frame,
        transform: Transform::IDENTITY,
        visible: true,
        material_overrides: Vec::new(),
        metadata: RenderMetadata {
            source_entity: Some(7),
            source_scene_node: None,
            tags: vec!["voxel-object".to_string()],
            label: Some("Runner".to_string()),
        },
    }
}

fn materials() -> BTreeMap<String, RenderMaterialDescriptor> {
    let material = material("material/runner", [0.8, 0.2, 0.1, 1.0]);
    BTreeMap::from([(material.id.clone(), material)])
}

fn material(id: &str, color: [f32; 4]) -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        texture_transform: None,
        stochastic_tiling: None,
        terrain_layers: None,
        shader: None,
        id: id.to_string(),
        color,
        texture: None,
        roughness: 1.0,
        metalness: 0.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: Default::default(),
        double_sided: false,
        voxel_surface: None,
        normal_map: None,
        triplanar: None,
        emission_map: Default::default(),
        occlusion_map: Default::default(),
        unlit: false,
        flat_shading: false,
        keep_dry: false,
        wind: None,
        water: None,
        translucent_shadow: false,
    }
}

fn admitted() -> voxel_object_runtime::AdmittedVoxelObject {
    admit_voxel_object(object(), VoxelObjectRuntimeLimits::default()).unwrap()
}

fn admitted_with_surface(surface_mode: SurfaceMode) -> voxel_object_runtime::AdmittedVoxelObject {
    admit_voxel_object_with_options(
        object(),
        VoxelObjectAdmissionOptions {
            surface_mode,
            ..VoxelObjectAdmissionOptions::default()
        },
    )
    .unwrap()
}

fn admitted_variant(
    asset_id: &str,
    material_id: &str,
    converter: &str,
) -> voxel_object_runtime::AdmittedVoxelObject {
    let mut asset = object();
    asset.asset_id = asset_id.to_string();
    asset.material_palette[0].material_asset_id = material_id.to_string();
    asset.provenance.converter = converter.to_string();
    let asset = with_computed_voxel_object_hashes(asset).unwrap();
    admit_voxel_object(asset, VoxelObjectRuntimeLimits::default()).unwrap()
}

fn object() -> VoxelObjectAsset {
    let default_frame = frame([0, 0, 0]);
    with_computed_voxel_object_hashes(VoxelObjectAsset {
        schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
        asset_id: "voxel-object/runner".to_string(),
        grid: VoxelObjectGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: 1.0,
            chunk_size: 16,
            pivot: [0.0; 3],
        },
        bounds: VoxelAssetBounds {
            min: [0, 0, 0],
            max: [1, 0, 0],
        },
        default_frame: default_frame.clone(),
        clips: vec![VoxelObjectClip {
            id: "walk".to_string(),
            name: None,
            frames_per_second: 10.0,
            frames: vec![
                VoxelObjectAnimationFrame {
                    duration_seconds: None,
                    anchors: Vec::new(),
                    collision: None,
                    frame: default_frame,
                },
                VoxelObjectAnimationFrame {
                    duration_seconds: None,
                    anchors: Vec::new(),
                    collision: None,
                    frame: frame([1, 0, 0]),
                },
            ],
        }],
        default_clip: Some("walk".to_string()),
        material_palette: vec![VoxelAssetMaterialBinding {
            material_slot: 1,
            material_asset_id: "material/runner".to_string(),
            display_name: None,
        }],
        material_map: vec![VoxelAssetMaterialMapping {
            source_material_slot: 0,
            source_material_name: None,
            voxel_material_slot: 1,
        }],
        provenance: VoxelObjectProvenance {
            kind: VoxelObjectProvenanceKind::Authored,
            source_path: "runner.voxel.json".to_string(),
            source_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            source_byte_count: 1,
            converter: "test".to_string(),
            settings_sha256:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            license_path: None,
            source_clips: vec![],
        },
        content_hash: String::new(),
    })
    .unwrap()
}

fn frame(coordinate: [i64; 3]) -> VoxelFrame {
    VoxelFrame {
        bounds: VoxelAssetBounds {
            min: coordinate,
            max: coordinate,
        },
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![VoxelSparseRun {
                start: coordinate,
                length: 1,
                material_slot: 1,
            }],
        },
        voxel_data_hash: String::new(),
    }
}
