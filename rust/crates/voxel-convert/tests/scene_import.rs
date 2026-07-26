use voxel_convert::{
    import_mesh_source, import_static_glb, import_static_glb_scene, source_sha256,
    MeshSourceFormat, MeshSourceImportRequest, MAX_IMPORTED_SCENE_NODES,
};

const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/kenney-wall-a.glb"
));
const HIERARCHY_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/kenney-wall-hierarchy.fixture.json"
));

#[test]
fn licensed_hierarchy_fixture_preserves_scene_mesh_primitive_and_uv_identity() {
    let source = hierarchy_fixture();
    let scene = import_static_glb_scene(&source).unwrap();
    let mesh = import_static_glb(&source).unwrap();

    assert_eq!(scene.source_scene_index, 0);
    assert_eq!(
        scene.source_scene_name.as_deref(),
        Some("licensed-hierarchy")
    );
    assert_eq!(scene.meshes.len(), 2);
    assert_eq!(
        scene
            .nodes
            .iter()
            .map(|node| (
                node.source_node_index,
                node.parent_node_index,
                node.source_mesh_index
            ))
            .collect::<Vec<_>>(),
        vec![(0, None, None), (1, Some(0), Some(0)), (2, None, Some(1))]
    );
    assert_eq!(
        translation(scene.nodes[0].model_transform),
        [10.0, 0.0, 0.0]
    );
    assert_eq!(
        translation(scene.nodes[1].model_transform),
        [12.0, 2.0, 0.0]
    );
    assert_eq!(
        translation(scene.nodes[2].model_transform),
        [-3.0, 0.0, 0.0]
    );

    assert_eq!(mesh.positions.len(), 96);
    assert_eq!(mesh.triangles.len(), 24);
    assert_eq!(mesh.primitive_groups.len(), 4);
    assert_eq!(mesh.texture_coordinates.len(), 1);
    assert!(mesh.texture_coordinates[0]
        .coordinates
        .iter()
        .all(Option::is_some));
    assert_eq!(
        mesh.primitive_groups
            .iter()
            .map(|group| {
                (
                    group.source_node_index,
                    group.source_mesh_index,
                    group.source_primitive_index,
                )
            })
            .collect::<Vec<_>>(),
        vec![(1, 0, 0), (1, 0, 1), (2, 1, 0), (2, 1, 1)]
    );
    assert_eq!(mesh, import_static_glb(&source).unwrap());

    let node = import_mesh_source(&request(&source, Some("node/1"))).unwrap();
    assert_eq!(node.mesh.positions.len(), 24);
    assert_eq!(node.mesh.triangles.len(), 12);
    assert_eq!(node.receipt.metadata.nodes.len(), 3);
    assert!(node
        .receipt
        .metadata
        .groups
        .iter()
        .all(|group| group.source_node_index == 1));

    let primitive = import_mesh_source(&request(&source, Some("group/2"))).unwrap();
    assert_eq!(primitive.mesh.positions.len(), 16);
    assert_eq!(primitive.mesh.triangles.len(), 8);
    assert_eq!(primitive.receipt.metadata.groups.len(), 1);
    assert_eq!(primitive.receipt.metadata.groups[0].source_node_index, 2);
    assert_eq!(primitive.receipt.metadata.groups[0].source_mesh_index, 1);
    assert_eq!(
        primitive.receipt.metadata.groups[0].source_primitive_index,
        0
    );
}

#[test]
fn hierarchy_primitive_and_external_buffer_rejections_are_source_locatable() {
    let cycle = mutate_glb_json(|document| {
        document["nodes"][0]["children"] = serde_json::json!([0]);
    });
    let error = import_static_glb_scene(&cycle).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.invalidSceneHierarchy"
    );
    assert_eq!(error.diagnostics()[0].path, "source.nodes[0].children[0]");

    let duplicate = mutate_glb_json(|document| {
        let mut child = document["nodes"][0].clone();
        child["name"] = "duplicate-child".into();
        document["nodes"].as_array_mut().unwrap().push(child);
        document["nodes"][0]["children"] = serde_json::json!([1]);
        document["scenes"][0]["nodes"] = serde_json::json!([0, 1]);
    });
    let error = import_static_glb_scene(&duplicate).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.ambiguousSceneNode");
    assert_eq!(error.diagnostics()[0].path, "source.nodes[1]");

    let lines = mutate_glb_json(|document| {
        document["meshes"][0]["primitives"][0]["mode"] = 1.into();
    });
    let error = import_static_glb_scene(&lines).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.unsupportedPrimitive"
    );
    assert_eq!(
        error.diagnostics()[0].path,
        "source.meshes[0].primitives[0].mode"
    );

    let external = mutate_glb_json(|document| {
        document["buffers"][0]["uri"] = "external.bin".into();
    });
    let error = import_static_glb_scene(&external).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.unsupportedFeature");
    assert_eq!(error.diagnostics()[0].path, "source.buffers[0].uri");
}

#[test]
fn scene_node_budget_is_checked_before_geometry_collection() {
    let excessive = mutate_glb_json(|document| {
        let nodes = document["nodes"].as_array_mut().unwrap();
        nodes.extend(
            (nodes.len()..=MAX_IMPORTED_SCENE_NODES)
                .map(|index| serde_json::json!({"name": format!("unused-{index}")})),
        );
    });
    let error = import_static_glb_scene(&excessive).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");
    assert_eq!(error.diagnostics()[0].path, "source.nodes");
}

fn hierarchy_fixture() -> Vec<u8> {
    mutate_glb_json(|document| {
        let fixture: serde_json::Value = serde_json::from_str(HIERARCHY_FIXTURE).unwrap();
        document["scenes"][0]["name"] = fixture["sceneName"].clone();
        document["scenes"][0]["nodes"] = fixture["sceneRoots"].clone();

        let mut first_mesh = document["meshes"][0].clone();
        first_mesh["name"] = fixture["meshNames"][0].clone();
        let mut second_mesh = first_mesh.clone();
        second_mesh["name"] = fixture["meshNames"][1].clone();
        document["meshes"] = serde_json::json!([first_mesh, second_mesh]);
        document["nodes"] = fixture["nodes"].clone();
    })
}

fn request(source: &[u8], mesh_primitive: Option<&str>) -> MeshSourceImportRequest {
    MeshSourceImportRequest {
        source_asset_id: "mesh/licensed-hierarchy".to_owned(),
        asset_version: 1,
        source_path: "fixtures/voxel-conversion/kenney-wall-hierarchy.glb".to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes: source.to_vec(),
        expected_source_sha256: Some(source_sha256(source)),
        mesh_primitive: mesh_primitive.map(str::to_owned),
    }
}

fn translation(matrix: [f64; 16]) -> [f64; 3] {
    [matrix[12], matrix[13], matrix[14]]
}

fn mutate_glb_json(change: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    assert_eq!(&SOURCE[0..4], b"glTF");
    let json_length = u32::from_le_bytes(SOURCE[12..16].try_into().unwrap()) as usize;
    assert_eq!(&SOURCE[16..20], b"JSON");
    let json_end = 20 + json_length;
    let mut document: serde_json::Value = serde_json::from_slice(&SOURCE[20..json_end]).unwrap();
    change(&mut document);
    let mut json = serde_json::to_vec(&document).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }

    let total_length = 20 + json.len() + (SOURCE.len() - json_end);
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(&SOURCE[0..8]);
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&SOURCE[json_end..]);
    glb
}
