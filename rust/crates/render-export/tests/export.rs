//! GLB export (#8826): the frozen frame of an output job written as binary
//! glTF, read back by the `gltf` crate and reopened through `asset-import` as
//! `Animation.OpenAnimatedMesh` admits it.

// render-wgpu's scene harness: the reopened export is drawn beside the
// original.
#[path = "../../render-wgpu/tests/support/mod.rs"]
mod support;

use std::path::PathBuf;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_export::export_glb;
use render_host_contracts::{RenderOutputJob, RenderOutputOperation};
use render_model::*;
use render_presentation::PresentationWorld;
use render_wgpu::RendererOptions;
use support::*;

const BODY: u64 = 1;
const WEAPON: u64 = 2;

fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    std::fs::read(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn world(ops: Vec<RenderDiff>) -> PresentationWorld {
    let mut world = PresentationWorld::default();
    world
        .apply(RenderFrameDiff {
            publication: None,
            ops,
        })
        .expect("retained model admits the fixture");
    world
}

fn export(
    world: &PresentationWorld,
    source: u64,
    include_animations: bool,
    resources: &Resources,
) -> Result<Vec<u8>, String> {
    let source = RenderHandle::new(source);
    export_glb(
        &RenderOutputJob {
            id: 1,
            source,
            frame: world.capture_output_scene(source, false).unwrap(),
            operation: RenderOutputOperation::Glb { include_animations },
        },
        resources,
    )
}

/// Admit GLB bytes as the runtime does; the admitted asset and its bytes.
fn admit_bytes(name: &str, bytes: &[u8]) -> (AnimatedMeshAsset, Vec<u8>) {
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(name.to_owned()),
        bytes,
        &ImportContext::default(),
    );
    let imported = imported
        .assets
        .unwrap_or_else(|| panic!("{name} is admitted: {:?}", imported.diagnostics));
    (imported.animated_mesh, imported.runtime_resource_bytes)
}

fn hold(resources: &mut Resources, asset: &AnimatedMeshAsset, bytes: Vec<u8>) {
    let hash = asset.content_hash.as_deref().expect("content hash");
    resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
}

fn primitive(handle: u64, parent: Option<u64>, geometry: Geometry, color: [f32; 4]) -> RenderDiff {
    let mut node = RenderNode::new(geometry);
    node.material.color = color;
    node.transform = transform([0.0, 1.0, 0.0], 0.0, [0.5; 3]);
    node.metadata.label = Some(format!("primitive {handle}"));
    RenderDiff::Create {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        node,
    }
}

fn node_named<'a>(document: &'a gltf::Gltf, name: &str) -> gltf::Node<'a> {
    document
        .nodes()
        .find(|node| node.name() == Some(name))
        .unwrap_or_else(|| panic!("no node {name}"))
}

#[test]
fn primitives_export_with_their_ancestor_path_and_no_unrelated_nodes() {
    let world = world(vec![
        group(1, None, transform([2.0, 0.0, 0.0], 90.0, [1.0; 3])),
        group(2, Some(1), transform([0.0, 0.0, -1.0], 0.0, [2.0; 3])),
        primitive(3, Some(2), Geometry::Cube, [0.8, 0.2, 0.1, 1.0]),
        primitive(4, Some(1), Geometry::Sphere, [0.1, 0.2, 0.8, 1.0]),
        primitive(5, Some(3), Geometry::Sphere, [0.2, 0.9, 0.3, 0.5]),
        primitive(
            6,
            Some(3),
            Geometry::Line {
                a: [0.0; 3],
                b: [1.0, 0.0, 0.0],
            },
            [1.0; 4],
        ),
    ]);
    let bytes = export(&world, 2, false, &Resources::default()).unwrap();
    let document = gltf::Gltf::from_slice(&bytes).expect("a valid GLB");

    // Root 1 holds only the path to 2, whose subtree is whole.
    let scene = document.default_scene().unwrap();
    let roots: Vec<_> = scene.nodes().collect();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].name(), Some("node 1"));
    let names: Vec<_> = roots[0].children().map(|node| node.name()).collect();
    assert_eq!(names, [Some("node 2")]);
    assert!(document
        .nodes()
        .all(|node| node.name() != Some("primitive 4")));
    let (translation, _, scale) = node_named(&document, "node 2").transform().decomposed();
    assert_eq!((translation, scale), ([0.0, 0.0, -1.0], [2.0; 3]));

    // Primitives are unlit meshes in their view colour; lines are lines.
    let cube = node_named(&document, "primitive 3");
    let material = cube.mesh().unwrap().primitives().next().unwrap().material();
    assert!(material.unlit());
    assert_eq!(
        material.pbr_metallic_roughness().base_color_factor(),
        [0.8, 0.2, 0.1, 1.0]
    );
    let sphere = node_named(&document, "primitive 5").mesh().unwrap();
    let blended = sphere.primitives().next().unwrap().material();
    assert_eq!(blended.alpha_mode(), gltf::material::AlphaMode::Blend);
    let line = node_named(&document, "primitive 6").mesh().unwrap();
    assert_eq!(
        line.primitives().next().unwrap().mode(),
        gltf::mesh::Mode::Lines
    );
}

#[test]
fn static_meshes_keep_their_slot_materials_and_embed_texture_pngs() {
    let mut resources = Resources::default();
    let texture = resources.texture(
        "texture/checker",
        4,
        4,
        &checker(4, [255, 255, 255, 255], [30, 30, 30, 255]),
        TextureWrap::Repeat,
    );
    let png = match &texture.payload.as_ref().unwrap().source {
        TexturePayloadSource::Resource { resource } => {
            render_wgpu::ResourceSource::bytes(&resources, resource)
                .unwrap()
                .into_owned()
        }
        TexturePayloadSource::Inline { encoded_bytes } => encoded_bytes.clone(),
    };
    let mut glowing = material(
        "material/checker",
        [0.9, 0.8, 0.7, 1.0],
        Some("texture/checker"),
    );
    glowing.emission_color = [1.0, 0.5, 0.0];
    glowing.emission_intensity = 2.0;
    let world = world(vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::DefineMaterial { material: glowing },
        static_mesh(
            "mesh/box",
            box_mesh([-0.5; 3], [0.5; 3], |_| 0),
            "material/checker",
        ),
        instance(7, None, "mesh/box", transform([0.0; 3], 0.0, [1.0; 3])),
    ]);
    let bytes = export(&world, 7, false, &resources).unwrap();
    let document = gltf::Gltf::from_slice(&bytes).expect("a valid GLB");
    let primitive = node_named(&document, "node 7")
        .mesh()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let material = primitive.material();
    let pbr = material.pbr_metallic_roughness();
    assert_eq!(pbr.base_color_factor(), [0.9, 0.8, 0.7, 1.0]);
    assert_eq!(pbr.metallic_factor(), 0.0);
    assert!((pbr.roughness_factor() - 0.9).abs() < 1e-6);
    assert_eq!(material.emissive_factor(), [1.0, 0.5, 0.0]);
    assert_eq!(material.emissive_strength(), Some(2.0));
    let texture = pbr.base_color_texture().expect("the checker texture");
    let gltf::image::Source::View { view, mime_type } =
        texture.texture().source().expect("an image").source()
    else {
        panic!("an embedded image");
    };
    assert_eq!(mime_type, "image/png");
    let blob = document.blob.as_ref().unwrap();
    assert_eq!(
        &blob[view.offset()..view.offset() + view.length()],
        png.as_slice()
    );
    assert_eq!(
        texture.texture().sampler().wrap_s(),
        gltf::texture::WrappingMode::Repeat
    );
    assert!(primitive.get(&gltf::Semantic::TexCoords(0)).is_some());

    // The export reopens as `OpenAnimatedMesh` admits a static GLB.
    let (reopened, _) = admit_bytes("export-static.glb", &bytes);
    assert!(reopened.clips.is_empty());
}

fn character(resources: &mut Resources) -> (Vec<RenderDiff>, AnimatedMeshAsset) {
    let (body, body_bytes) = admit_bytes(
        "body.glb",
        &fixture("csharp-joint-attachments/content/body.glb"),
    );
    let (weapon, weapon_bytes) = admit_bytes(
        "weapon.glb",
        &fixture("csharp-joint-attachments/content/weapon.glb"),
    );
    hold(resources, &body, body_bytes);
    hold(resources, &weapon, weapon_bytes);
    let animated = |handle: u64, parent: Option<u64>, asset: &AnimatedMeshAsset, scale: f32| {
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(handle),
            parent: parent.map(RenderHandle::new),
            instance: AnimatedMeshInstanceDescriptor {
                inspection: AnimatedMeshInspection::default(),
                asset: asset.asset.clone(),
                transform: transform([0.0; 3], 0.0, [scale; 3]),
                visible: true,
                material_overrides: Vec::new(),
                playback: None,
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
            },
        }
    };
    let ops = vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: weapon.clone(),
        },
        animated(BODY, None, &body, 1.0),
        animated(WEAPON, Some(BODY), &weapon, 0.01),
        RenderDiff::SetParentJoint {
            handle: RenderHandle::new(WEAPON),
            joint: Some("RightHand".to_owned()),
        },
    ];
    (ops, body)
}

#[test]
fn animated_meshes_keep_skins_clips_and_joint_attachments_and_reopen() {
    let mut resources = Resources::default();
    let (ops, body) = character(&mut resources);
    let world = world(ops);

    let bytes = export(&world, BODY, true, &resources).unwrap();
    let document = gltf::Gltf::from_slice(&bytes).expect("a valid GLB");
    assert!(document.skins().count() >= 1);
    let mut exported: Vec<String> = document
        .animations()
        .map(|animation| animation.name().unwrap().to_owned())
        .collect();
    exported.sort();
    let mut clips: Vec<String> = body.clips.iter().map(|clip| clip.id.clone()).collect();
    clips.sort();
    assert_eq!(exported, clips);
    // The weapon hangs from the exported RightHand joint.
    let hand = node_named(&document, "RightHand");
    assert!(hand.children().any(|node| node.name() == Some("node 2")));

    // Reopened, the rig and its clips are admitted again.
    let (reopened, _) = admit_bytes("export-animated.glb", &bytes);
    let mut reopened_clips: Vec<String> =
        reopened.clips.iter().map(|clip| clip.id.clone()).collect();
    reopened_clips.sort();
    assert_eq!(reopened_clips, clips);
    assert!(reopened.rig.is_some());

    // Without animations, only the rig is written.
    let still = export(&world, BODY, false, &resources).unwrap();
    let document = gltf::Gltf::from_slice(&still).expect("a valid GLB");
    assert_eq!(document.animations().count(), 0);
    assert!(document.skins().count() >= 1);
}

/// `glb` with one more scene root, an empty node named `name`.
fn with_extra_root(glb: &[u8], name: &str) -> Vec<u8> {
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let mut document: serde_json::Value =
        serde_json::from_slice(&glb[20..20 + json_length]).unwrap();
    let nodes = document["nodes"].as_array_mut().unwrap();
    nodes.push(serde_json::json!({ "name": name }));
    let index = nodes.len() - 1;
    let scene = document["scene"].as_u64().unwrap_or(0) as usize;
    document["scenes"][scene]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(index));
    let mut text = serde_json::to_vec(&document).unwrap();
    text.resize(text.len().next_multiple_of(4), b' ');
    let bin = &glb[20 + json_length..];
    let total = 12 + 8 + text.len() + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&text);
    out.extend_from_slice(bin);
    out
}

#[test]
fn a_joint_attachment_follows_the_skin_joint_when_another_node_shares_its_name() {
    // A non-joint root also named RightHand: the renderer attaches to the
    // skin joint, and so must the export.
    let bytes = with_extra_root(
        &fixture("csharp-joint-attachments/content/body.glb"),
        "RightHand",
    );
    let (body, body_bytes) = admit_bytes("body.glb", &bytes);
    let mut resources = Resources::default();
    hold(&mut resources, &body, body_bytes);
    let world = world(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(BODY),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: AnimatedMeshInspection::default(),
                asset: body.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: None,
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
            },
        },
        primitive(5, Some(BODY), Geometry::Cube, [0.8, 0.2, 0.1, 1.0]),
        RenderDiff::SetParentJoint {
            handle: RenderHandle::new(5),
            joint: Some("RightHand".to_owned()),
        },
    ]);
    let exported = export(&world, BODY, true, &resources).unwrap();
    let document = gltf::Gltf::from_slice(&exported).expect("a valid GLB");
    assert_eq!(
        document
            .nodes()
            .filter(|node| node.name() == Some("RightHand"))
            .count(),
        2
    );
    let hand = document
        .skins()
        .flat_map(|skin| skin.joints().collect::<Vec<_>>())
        .find(|joint| joint.name() == Some("RightHand"))
        .expect("the RightHand skin joint");
    assert!(hand
        .children()
        .any(|node| node.name() == Some("primitive 5")));
}

#[test]
fn a_reopened_character_renders_the_sampled_pose_as_the_original_did() {
    let mut harness = Harness::new(RendererOptions::default());
    let (ops, _) = character(&mut harness.resources);
    let pose = AnimatedMeshPlaybackCommand::Sample {
        clip: "run".to_owned(),
        normalized_time: 0.5,
    };
    harness.apply(ops);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: pose.clone(),
    }]);
    let view = camera([4.0, 3.0, 6.0], -33.690_067, -11.750_674);
    let (_, original) = harness.render(&view);
    let background = &original[..4];
    let character = original
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel != &background)
        .count();
    assert!(character > 1_000, "the character covers {character} pixels");

    let bytes = export(&harness.world, BODY, true, &harness.resources).unwrap();
    let (reopened, reopened_bytes) = admit_bytes("export-animated.glb", &bytes);
    hold(&mut harness.resources, &reopened, reopened_bytes);
    harness.apply(vec![
        RenderDiff::Destroy {
            handle: RenderHandle::new(WEAPON),
        },
        RenderDiff::Destroy {
            handle: RenderHandle::new(BODY),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: reopened.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(10),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: AnimatedMeshInspection::default(),
                asset: reopened.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: Some(pose),
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
            },
        },
    ]);
    let (_, again) = harness.render(&view);
    // The weapon is a node of the export: the whole pose matches within
    // the screenshot tolerance on all but 2% of the character's pixels.
    let differing = original
        .as_chunks::<4>()
        .0
        .iter()
        .zip(again.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 12))
        .count();
    assert!(
        differing * 50 < character,
        "{differing} of the character's {character} pixels differ"
    );
}

#[test]
fn sprites_voxel_surfaces_and_ambient_lights_fail_the_export_by_name() {
    let mut resources = Resources::default();
    let atlas_texture = resources.texture(
        "texture/atlas",
        2,
        2,
        &checker(2, [255; 4], [0, 0, 0, 255]),
        TextureWrap::Clamp,
    );
    let sprite = SpriteInstanceDescriptor {
        asset: "sprite/atlas".to_owned(),
        frame: 0,
        pivot: [0.5, 0.5],
        size: [1.0, 1.0],
        size_mode: SpriteSizeMode::World,
        billboard: BillboardMode::Spherical,
        tint: [1.0; 4],
        render_order: 0,
        depth: SpriteDepthPolicy::Default,
        layer: RenderLayer::Scene,
        viewport_placement: None,
        shading: SpriteShading::Unlit,
        material: SpriteMaterialDescriptor::default(),
        visible: true,
        transform: Transform::IDENTITY,
        attachment: SpriteAttachment::default(),
        metadata: RenderMetadata::default(),
    };
    let world = world(vec![
        RenderDiff::DefineTexture {
            texture: atlas_texture,
        },
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/atlas".to_owned(),
                texture: "texture/atlas".to_owned(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0, 0.0],
                    uv_max: [1.0, 1.0],
                    size: None,
                }],
            },
        },
        group(1, None, Transform::IDENTITY),
        RenderDiff::CreateSprite {
            handle: RenderHandle::new(2),
            parent: Some(RenderHandle::new(1)),
            sprite,
        },
        group(3, None, Transform::IDENTITY),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(4),
            parent: Some(RenderHandle::new(3)),
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        group(5, None, Transform::IDENTITY),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(6),
            parent: Some(RenderHandle::new(5)),
            light: LightDescriptor::Point {
                color: [1.0, 0.8, 0.6],
                intensity: 3.0,
                enabled: true,
                position: [0.0, 2.0, 0.0],
                range: Some(10.0),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
    ]);
    let sprite = export(&world, 1, false, &resources).unwrap_err();
    assert!(sprite.contains("sprite 2"), "{sprite}");
    let ambient = export(&world, 3, false, &resources).unwrap_err();
    assert!(ambient.contains("ambient light 4"), "{ambient}");

    // A point light in the selection is a punctual light at its position.
    let bytes = export(&world, 5, false, &resources).unwrap();
    let document = gltf::Gltf::from_slice(&bytes).expect("a valid GLB");
    assert!(document
        .extensions_used()
        .any(|extension| extension == "KHR_lights_punctual"));
    let light = node_named(&document, "light 6");
    assert_eq!(light.transform().decomposed().0, [0.0, 2.0, 0.0]);
}
