//! Hand landmarks on a drawn rig (#10402, #10403): a three-fingered right
//! hand, and a two-fingered one, closed round a handle read their palm
//! facing the handle and their knuckles along it, though the palm's point
//! lies inside the fist beside the handle's line; the same hand turned a
//! quarter, or the handle behind it, reads outside.

mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{LandmarkDrawn, RendererOptions};
use support::*;

const HAND: u64 = 3;

/// A hand's joints, each placed from the wrist: a thumb, then its fingers'
/// bases, index first, all along +y with the palm facing +z.
type Joints = [(&'static str, [f32; 3])];

/// Three fingers and a thumb.
const THREE: [(&str, [f32; 3]); 5] = [
    ("wrist", [0.0, 0.0, 0.0]),
    ("thumb", [0.035, 0.03, 0.01]),
    ("index", [0.03, 0.09, 0.0]),
    ("middle", [0.0, 0.095, 0.0]),
    ("last", [-0.03, 0.085, 0.0]),
];

/// Two fingers and a thumb.
const TWO: [(&str, [f32; 3]); 4] = [
    ("wrist", [0.0, 0.0, 0.0]),
    ("thumb", [0.035, 0.03, 0.01]),
    ("index", [0.02, 0.09, 0.0]),
    ("last", [-0.02, 0.088, 0.0]),
];

/// The hand as a GLB: its joints children of the wrist, one skinned
/// triangle on the wrist, and a clip holding the rest pose.
fn hand_glb(joints: &Joints) -> Vec<u8> {
    let floats =
        |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut push = |bytes: &[u8], accessor: &str| {
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        views.push(format!(
            "{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{}}}",
            bin.len(),
            bytes.len()
        ));
        bin.extend_from_slice(bytes);
        accessors.push(accessor.replace("VIEW", &(views.len() - 1).to_string()));
        accessors.len() - 1
    };
    let count = joints.len();
    let binds: Vec<f32> = joints
        .iter()
        .flat_map(|(_, [x, y, z])| {
            [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -x, -y, -z, 1.0,
            ]
        })
        .collect();
    let binds = push(
        &floats(&binds),
        &format!(
            "{{\"bufferView\":VIEW,\"componentType\":5126,\"count\":{count},\"type\":\"MAT4\"}}"
        ),
    );
    let positions = push(
        &floats(&[0.0, 0.0, 0.0, 0.03, 0.09, 0.0, -0.03, 0.085, 0.0]),
        "{\"bufferView\":VIEW,\"componentType\":5126,\"count\":3,\"type\":\"VEC3\",\"min\":[-0.03,0,0],\"max\":[0.03,0.09,0]}",
    );
    let skin_joints = push(
        &[0u8; 12],
        "{\"bufferView\":VIEW,\"componentType\":5121,\"count\":3,\"type\":\"VEC4\"}",
    );
    let weights = push(
        &floats(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
        "{\"bufferView\":VIEW,\"componentType\":5126,\"count\":3,\"type\":\"VEC4\"}",
    );
    let indices = push(
        &[0u16, 1, 2]
            .iter()
            .flat_map(|i| i.to_le_bytes())
            .collect::<Vec<u8>>(),
        "{\"bufferView\":VIEW,\"componentType\":5123,\"count\":3,\"type\":\"SCALAR\"}",
    );
    let times = push(
        &floats(&[0.0, 1.0]),
        "{\"bufferView\":VIEW,\"componentType\":5126,\"count\":2,\"type\":\"SCALAR\",\"min\":[0],\"max\":[1]}",
    );
    let rest = push(
        &floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        "{\"bufferView\":VIEW,\"componentType\":5126,\"count\":2,\"type\":\"VEC4\"}",
    );
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let list = |from: usize| {
        (from..count)
            .map(|at| at.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut nodes: Vec<String> = joints
        .iter()
        .enumerate()
        .map(|(at, (name, [x, y, z]))| {
            let children = if at == 0 {
                format!(",\"children\":[{}]", list(1))
            } else {
                String::new()
            };
            format!("{{\"name\":\"{name}\",\"translation\":[{x},{y},{z}]{children}}}")
        })
        .collect();
    nodes.push("{\"name\":\"hand\",\"mesh\":0,\"skin\":0}".to_owned());
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0,{count}]}}],\"nodes\":[{}],\
         \"skins\":[{{\"joints\":[{}],\"inverseBindMatrices\":{binds}}}],\
         \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":{positions},\"JOINTS_0\":{skin_joints},\"WEIGHTS_0\":{weights}}},\"indices\":{indices}}}]}}],\
         \"animations\":[{{\"name\":\"rest\",\"samplers\":[{{\"input\":{times},\"output\":{rest}}}],\"channels\":[{{\"sampler\":0,\"target\":{{\"node\":0,\"path\":\"rotation\"}}}}]}}],\
         \"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]}}",
        nodes.join(","),
        list(0),
        accessors.join(","),
        views.join(","),
        bin.len()
    );
    let mut json = json.into_bytes();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut glb = Vec::new();
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&((28 + json.len() + bin.len()) as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin);
    glb
}

/// The hand drawn at the origin in its sampled rest pose, and its joint
/// indices as landmarks name them (the rig's order, which the import sets).
fn hand_scene(harness: &mut Harness, joints: &Joints) -> impl Fn(&str) -> u32 {
    let bytes = hand_glb(joints);
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("hand.glb".to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let asset = imported
        .assets
        .unwrap_or_else(|| panic!("the hand is admitted: {:?}", imported.diagnostics))
        .animated_mesh;
    let rig: Vec<String> = asset
        .rig
        .as_ref()
        .expect("a skinned hand has a rig")
        .joints
        .iter()
        .map(|joint| joint.id.clone())
        .collect();
    let hash = asset.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: asset.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(HAND),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                playback_fade: None,
                inspection: AnimatedMeshInspection::default(),
                asset: asset.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: Some(AnimatedMeshPlaybackCommand::Sample {
                    clip: "rest".to_owned(),
                    normalized_time: 0.0,
                }),
                metadata: RenderMetadata {
                    source_entity: Some(HAND),
                    ..RenderMetadata::default()
                },
                layer: RenderLayer::Scene,
                shadow_casting: Default::default(),
            },
        },
    ]);
    move |name| {
        rig.iter()
            .position(|id| id == name)
            .unwrap_or_else(|| panic!("{name} in {rig:?}")) as u32
    }
}

fn turn(harness: &mut Harness, yaw: f32) {
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(HAND),
        transform: Some(transform([0.0; 3], yaw, [1.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
}

/// A line in the world through `point` along `axis`.
fn line(point: [f32; 3], axis: [f32; 3]) -> Landmark {
    Landmark::At {
        space: OverlaySpace::World,
        point,
        rotation: [0.0, 0.0, 0.0, 1.0],
        axis,
    }
}

fn facing(name: &str, from: Landmark, to: Landmark, aim: LandmarkAim) -> LandmarkPair {
    LandmarkPair {
        name: name.to_owned(),
        from,
        to,
        tolerance: 0.0,
        aim,
        angle_tolerance: 20.0,
    }
}

fn measured(harness: &mut Harness) -> Vec<(String, LandmarkDrawn)> {
    harness.render(&camera([0.0, 0.05, 0.5], 0.0, 0.0));
    let [set] = harness.renderer.landmark_sets().try_into().unwrap();
    set.pairs
        .into_iter()
        .map(|pair| (pair.name, pair.drawn.expect("the hand is drawn")))
        .collect()
}

/// A right hand of `joints` closed round a handle line across its fingers
/// (along the knuckles, +x), `handle` metres above and 14 mm before its
/// wrist, with the palm centred toward `middle`: the palm faces the handle
/// `palm` degrees off and the knuckles run along it; a handle behind the
/// hand, or the hand turned a quarter, reads outside.
fn grips(joints: &Joints, middle: &str, handle: f32, palm_off: f32) {
    let mut harness = Harness::new(RendererOptions::default());
    let joint = hand_scene(&mut harness, joints);
    // The palm's point, 12 mm out of the palm as the C# hand rig places it
    // by default, lies inside the fist 2 mm before the handle's line.
    let palm = Landmark::Palm {
        entity: HAND,
        wrist: joint("wrist"),
        index: joint("index"),
        middle: joint(middle),
        pinky: joint("last"),
        along: 0.55,
        depth: 0.012,
        left: false,
    };
    let knuckles = Landmark::Between {
        entity: HAND,
        from: joint("last"),
        to: joint("index"),
        amount: 0.5,
    };
    let behind = line([0.0, handle, -0.014], [1.0, 0.0, 0.0]);
    let handle = line([0.0, handle, 0.014], [1.0, 0.0, 0.0]);
    harness.apply(vec![RenderDiff::SetOverlaySet {
        name: "grip".to_owned(),
        set: OverlaySetDescriptor {
            items: Vec::new(),
            pairs: vec![
                facing("palm", palm, handle, LandmarkAim::Line),
                facing("knuckles", knuckles, handle, LandmarkAim::Axis),
                facing("behind", palm, behind, LandmarkAim::Line),
            ],
        },
    }]);
    let angle = |pairs: &[(String, LandmarkDrawn)], name: &str| {
        let (_, drawn) = pairs.iter().find(|(pair, _)| pair == name).unwrap();
        (drawn.angle.expect("an angle"), drawn.within)
    };

    let held = measured(&mut harness);
    // Both hands' fingers sit evenly about x = 0: so does the palm's centre.
    let (_, centre) = held.iter().find(|(pair, _)| pair == "palm").unwrap();
    assert!(centre.from[0].abs() < 1e-3, "palm centre {:?}", centre.from);
    let (palm_angle, within) = angle(&held, "palm");
    assert!(
        (palm_angle - palm_off).abs() < 1.0 && within,
        "palm {palm_angle}"
    );
    let (knuckles_angle, within) = angle(&held, "knuckles");
    assert!(knuckles_angle < 6.0 && within, "knuckles {knuckles_angle}");
    // A handle behind the hand is one the palm faces away from.
    let (behind_angle, within) = angle(&held, "behind");
    assert!(behind_angle > 160.0 && !within, "behind {behind_angle}");

    // The hand turned a quarter about its wrist: the palm faces across the
    // handle and the knuckles cross it.
    turn(&mut harness, 90.0);
    let turned = measured(&mut harness);
    let (palm_angle, within) = angle(&turned, "palm");
    assert!(
        (palm_angle - 90.0).abs() < 1.0 && !within,
        "turned palm {palm_angle}"
    );
    let (knuckles_angle, within) = angle(&turned, "knuckles");
    assert!(
        (knuckles_angle - 90.0).abs() < 6.0 && !within,
        "turned knuckles {knuckles_angle}"
    );
}

#[test]
fn a_three_fingered_fist_faces_its_handle_and_runs_its_knuckles_along_it() {
    // The palm's centre heads for the middle finger's base, 4 mm below the
    // handle: aimed from the palm's joints the handle is 15 degrees off its
    // normal (from the point inside the fist it would be 62).
    grips(&THREE, "middle", 0.056, 15.0);
}

#[test]
fn a_two_fingered_fist_centres_its_palm_between_its_fingers() {
    // With no finger between its two, the palm names its index as the
    // middle and its centre heads midway between the fingers' bases, 4 mm
    // below the handle.
    grips(&TWO, "index", 0.053, 16.0);
}
