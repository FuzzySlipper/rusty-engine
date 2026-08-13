use render_model::*;
use std::path::Path;

fn material() -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        schema_version: 2,
        id: "material/plain".to_string(),
        color: [0.5, 0.6, 0.7, 1.0],
        texture: Some("texture/checker".to_string()),
        roughness: 0.8,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Planar,
        voxel_surface: None,
    }
}

fn payload(provenance: MeshProvenance) -> MeshPayloadDescriptor {
    MeshPayloadDescriptor {
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
            min: [0.0; 3],
            max: [1.0, 1.0, 0.0],
        },
        source: MeshPayloadSource::Inline {
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            uvs: None,
            indices: vec![0, 1, 2],
        },
        provenance,
    }
}

fn metadata(source: u64, label: &str) -> RenderMetadata {
    RenderMetadata {
        source_entity: Some(source),
        source_scene_node: None,
        tags: vec!["fixture".to_string()],
        label: Some(label.to_string()),
    }
}

fn every_retained_operation_frame() -> RenderFrameDiff {
    let texture = TextureDescriptor {
        id: "texture/checker".to_string(),
        width: 2,
        height: 2,
        filter: TextureFilter::Nearest,
        wrap: TextureWrap::Repeat,
        content_hash: Some("cafe".to_string()),
        version: 1,
        payload: None,
    };
    let atlas = SpriteAtlasDescriptor {
        id: "sprite/sparks".to_string(),
        texture: texture.id.clone(),
        frames: vec![SpriteFrameRect {
            frame: 0,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            size: None,
        }],
    };
    let static_mesh = StaticMeshAsset {
        asset: "mesh/triangle".to_string(),
        payload: payload(MeshProvenance::StaticAsset),
        material_slots: vec![MeshMaterialSlot {
            slot: 0,
            material: "material/plain".to_string(),
        }],
        collision: MeshCollisionPolicy::AabbFallback,
    };
    let animated_mesh = AnimatedMeshAsset {
        asset: "mesh-animation/character".to_string(),
        runtime_format: AnimatedMeshRuntimeFormat::Glb,
        content_hash: Some("f00d".to_string()),
        clips: vec![AnimationClipDescriptor {
            id: "idle".to_string(),
            name: Some("Idle".to_string()),
            duration_seconds: Some(1.0),
        }],
        default_clip: Some("idle".to_string()),
        material_slots: vec![MeshMaterialSlot {
            slot: 0,
            material: "material/plain".to_string(),
        }],
        bounds: MeshBoundsDescriptor {
            min: [-0.5, 0.0, -0.5],
            max: [0.5, 2.0, 0.5],
        },
    };
    let playback = AnimatedMeshPlaybackCommand::Play {
        clip: "idle".to_string(),
        r#loop: AnimationLoopMode::Repeat,
        speed: 1.0,
        weight: 1.0,
        restart: true,
        fade_seconds: Some(0.2),
    };
    let sprite = SpriteInstanceDescriptor {
        asset: atlas.id.clone(),
        frame: 0,
        pivot: [0.5, 0.5],
        size: [1.0, 1.0],
        size_mode: SpriteSizeMode::World,
        billboard: BillboardMode::Spherical,
        tint: [1.0; 4],
        render_order: 3,
        depth: SpriteDepthPolicy::Default,
        shading: SpriteShading::Unlit,
        material: SpriteMaterialDescriptor::default(),
        visible: true,
        transform: Transform::IDENTITY,
        attachment: SpriteAttachment {
            source_entity: Some(7),
            source_scene_node: None,
            attachment_point: Some("head".to_string()),
        },
        metadata: metadata(7, "spark"),
    };

    RenderFrameDiff::try_from_ops(vec![
        RenderDiff::DefineTexture {
            texture: texture.clone(),
        },
        RenderDiff::DefineMaterial {
            material: material(),
        },
        RenderDiff::DefineSpriteAtlas {
            atlas: atlas.clone(),
        },
        RenderDiff::DefineStaticMesh {
            asset: static_mesh.clone(),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: animated_mesh.clone(),
        },
        RenderDiff::Create {
            handle: RenderHandle::new(1),
            parent: None,
            node: RenderNode {
                metadata: metadata(1, "primitive"),
                ..RenderNode::new(Geometry::Cube)
            },
        },
        RenderDiff::Update {
            handle: RenderHandle::new(1),
            transform: Some(Transform {
                translation: [1.0, 2.0, 3.0],
                ..Transform::IDENTITY
            }),
            material: Some(Material {
                color: [0.2, 0.3, 0.4, 1.0],
                wireframe: true,
            }),
            visible: Some(false),
            metadata: Some(metadata(1, "updated")),
        },
        RenderDiff::ReplaceMeshPayload {
            handle: RenderHandle::new(1),
            payload: payload(MeshProvenance::Generated),
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(2),
            parent: Some(RenderHandle::new(1)),
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 2.0,
                enabled: true,
                direction: [0.0, -1.0, 0.0],
                shadow_intent: LightShadowIntent::Requested,
            },
        },
        RenderDiff::UpdateLight {
            handle: RenderHandle::new(2),
            light: LightDescriptor::Directional {
                color: [0.2; 3],
                intensity: 0.4,
                enabled: true,
                direction: [1.0, -1.0, 0.0],
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
        RenderDiff::CreateStaticMeshInstance {
            handle: RenderHandle::new(3),
            parent: None,
            instance: StaticMeshInstanceDescriptor {
                asset: static_mesh.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                metadata: metadata(3, "static"),
            },
        },
        RenderDiff::SetMaterialInstanceParameters {
            handle: RenderHandle::new(3),
            slot: 0,
            parameters: Some(MaterialInstanceParameters {
                texture_tint: [1.0, 0.5, 0.5, 1.0],
                emission_color: [1.0, 0.0, 0.0],
                emission_intensity: 0.5,
            }),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(4),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                asset: animated_mesh.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: Some(playback.clone()),
                metadata: metadata(4, "animated"),
            },
        },
        RenderDiff::SetAnimatedMeshPlayback {
            handle: RenderHandle::new(4),
            playback,
        },
        RenderDiff::CreateSprite {
            handle: RenderHandle::new(5),
            parent: Some(RenderHandle::new(1)),
            sprite,
        },
        RenderDiff::UpdateSprite {
            handle: RenderHandle::new(5),
            frame: Some(0),
            tint: Some([0.5, 1.0, 1.0, 1.0]),
            render_order: Some(4),
            visible: Some(true),
        },
        RenderDiff::Destroy {
            handle: RenderHandle::new(5),
        },
    ])
    .unwrap()
}

#[test]
fn every_retained_operation_survives_the_versioned_json_border() {
    let frame = every_retained_operation_frame();
    let json = frame.encode_json().unwrap();
    let decoded = RenderFrameDiff::decode_json(&json).unwrap();
    assert_eq!(decoded, frame);
    for operation in [
        "defineTexture",
        "defineMaterial",
        "defineSpriteAtlas",
        "defineStaticMesh",
        "defineAnimatedMesh",
        "create",
        "update",
        "replaceMeshPayload",
        "createLight",
        "updateLight",
        "createStaticMeshInstance",
        "setMaterialInstanceParameters",
        "createAnimatedMeshInstance",
        "setAnimatedMeshPlayback",
        "createSprite",
        "updateSprite",
        "destroy",
    ] {
        assert!(json.contains(&format!("\"op\": \"{operation}\"")));
    }
}

#[test]
fn committed_cross_language_fixture_is_a_valid_canonical_frame() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repository root");
    let input = std::fs::read_to_string(root.join("fixtures/render/retained-frame-v1.json"))
        .expect("read cross-language render fixture");
    let frame = RenderFrameDiff::decode_json(&input).unwrap();
    assert_eq!(frame, every_retained_operation_frame());
    assert_eq!(input.trim_end(), frame.encode_json().unwrap());
}

#[test]
fn unknown_contract_fields_fail_closed() {
    let input = r#"{
      "schemaVersion": 1,
      "ops": [{
        "op": "destroy",
        "handle": 1,
        "unexpectedAuthority": true
      }]
    }"#;
    assert!(matches!(
        RenderFrameDiff::decode_json(input),
        Err(RenderJsonError::Decode(_))
    ));
}

#[test]
fn voxel_object_resource_and_frame_swap_survive_the_json_border() {
    let asset = VoxelObjectRenderAsset {
        asset: "voxel-object/runner".to_string(),
        content_hash: "sha256:runner".to_string(),
        meshes: vec![VoxelObjectRenderMesh {
            payload: payload(MeshProvenance::VoxelObject),
        }],
        frames: vec![VoxelObjectRenderFrame {
            id: "default".to_string(),
            mesh: 0,
        }],
        material_slots: vec![MeshMaterialSlot {
            slot: 0,
            material: "material/plain".to_string(),
        }],
    };
    let frame = RenderFrameDiff::try_from_ops(vec![
        RenderDiff::DefineVoxelObject {
            asset: asset.clone(),
        },
        RenderDiff::CreateVoxelObjectInstance {
            handle: RenderHandle::new(9),
            parent: None,
            instance: VoxelObjectInstanceDescriptor {
                asset: asset.asset,
                frame: 0,
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                metadata: metadata(9, "voxel object"),
            },
        },
        RenderDiff::SetVoxelObjectFrame {
            handle: RenderHandle::new(9),
            frame: 0,
        },
        RenderDiff::Destroy {
            handle: RenderHandle::new(9),
        },
        RenderDiff::ReleaseVoxelObject {
            asset: "voxel-object/runner".to_string(),
        },
    ])
    .unwrap();
    assert_eq!(
        RenderFrameDiff::decode_json(&frame.encode_json().unwrap()).unwrap(),
        frame
    );
}
