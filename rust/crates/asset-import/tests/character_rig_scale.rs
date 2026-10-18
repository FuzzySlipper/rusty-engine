//! Game-character rigs admitted at their real size: many joints and hundreds
//! of clips in one GLB, and clip packs that carry only the skeleton and
//! their clips.

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};

/// A synthetic character GLB: a chain of `joints` skin joints, `clips`
/// clips each turning every joint over `keys` keys at 30 fps (and moving
/// the root), and, with `mesh`, one skinned triangle on the first joint.
pub fn character_glb(joints: usize, clips: usize, keys: usize, mesh: bool) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut push = |bytes: &[u8], accessor: String, bin: &mut Vec<u8>| -> usize {
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
    let floats = |values: &[f32]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };

    // Inverse binds: joint i sits at y = 0.1 i in the model.
    let mut binds = Vec::new();
    for joint in 0..joints {
        let y = -0.1 * joint as f32;
        binds.extend_from_slice(&[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, y, 0.0, 1.0,
        ]);
    }
    let binds = push(
        &floats(&binds),
        format!(
            "{{\"bufferView\":VIEW,\"componentType\":5126,\"count\":{joints},\"type\":\"MAT4\"}}"
        ),
        &mut bin,
    );
    let times: Vec<f32> = (0..keys).map(|key| key as f32 / 30.0).collect();
    let end = times.last().copied().unwrap_or(0.0);
    let times = push(
        &floats(&times),
        format!(
            "{{\"bufferView\":VIEW,\"componentType\":5126,\"count\":{keys},\"type\":\"SCALAR\",\"min\":[0],\"max\":[{end}]}}"
        ),
        &mut bin,
    );
    let mut animations = Vec::new();
    for clip in 0..clips {
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        let angle = 0.01 * (clip % 50) as f32;
        let rotations: Vec<f32> = (0..keys)
            .flat_map(|key| {
                let half = angle * key as f32 / keys as f32 / 2.0;
                [0.0, half.sin(), 0.0, half.cos()]
            })
            .collect();
        let rotations = push(
            &floats(&rotations),
            format!(
                "{{\"bufferView\":VIEW,\"componentType\":5126,\"count\":{keys},\"type\":\"VEC4\"}}"
            ),
            &mut bin,
        );
        for joint in 0..joints {
            samplers.push(format!("{{\"input\":{times},\"output\":{rotations}}}"));
            channels.push(format!(
                "{{\"sampler\":{},\"target\":{{\"node\":{joint},\"path\":\"rotation\"}}}}",
                samplers.len() - 1
            ));
        }
        let moves: Vec<f32> = (0..keys)
            .flat_map(|key| [0.0, 0.0, 0.01 * key as f32])
            .collect();
        let moves = push(
            &floats(&moves),
            format!(
                "{{\"bufferView\":VIEW,\"componentType\":5126,\"count\":{keys},\"type\":\"VEC3\"}}"
            ),
            &mut bin,
        );
        samplers.push(format!("{{\"input\":{times},\"output\":{moves}}}"));
        channels.push(format!(
            "{{\"sampler\":{},\"target\":{{\"node\":0,\"path\":\"translation\"}}}}",
            samplers.len() - 1
        ));
        animations.push(format!(
            "{{\"name\":\"clip_{clip:03}\",\"samplers\":[{}],\"channels\":[{}]}}",
            samplers.join(","),
            channels.join(",")
        ));
    }
    let mut nodes: Vec<String> = (0..joints)
        .map(|joint| {
            let children = if joint + 1 < joints {
                format!(",\"children\":[{}]", joint + 1)
            } else {
                String::new()
            };
            let lift = if joint == 0 { 0.0 } else { 0.1 };
            format!("{{\"name\":\"joint_{joint:02}\",\"translation\":[0,{lift},0]{children}}}")
        })
        .collect();
    let mut scene_nodes = vec!["0".to_owned()];
    let mut meshes = String::new();
    if mesh {
        let positions = push(
            &floats(&[0.0, 0.0, 0.0, 0.1, 0.0, 0.0, 0.0, 0.1, 0.0]),
            "{\"bufferView\":VIEW,\"componentType\":5126,\"count\":3,\"type\":\"VEC3\",\"min\":[0,0,0],\"max\":[0.1,0.1,0]}".to_owned(),
            &mut bin,
        );
        let joints_attr = push(
            &[0u8; 12],
            "{\"bufferView\":VIEW,\"componentType\":5121,\"count\":3,\"type\":\"VEC4\"}".to_owned(),
            &mut bin,
        );
        let weights = push(
            &floats(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
            "{\"bufferView\":VIEW,\"componentType\":5126,\"count\":3,\"type\":\"VEC4\"}".to_owned(),
            &mut bin,
        );
        let indices = push(
            &[0u16, 1, 2]
                .iter()
                .flat_map(|i| i.to_le_bytes())
                .collect::<Vec<u8>>(),
            "{\"bufferView\":VIEW,\"componentType\":5123,\"count\":3,\"type\":\"SCALAR\"}"
                .to_owned(),
            &mut bin,
        );
        meshes = format!(
            ",\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":{positions},\"JOINTS_0\":{joints_attr},\"WEIGHTS_0\":{weights}}},\"indices\":{indices}}}]}}]"
        );
        nodes.push("{\"name\":\"body\",\"mesh\":0,\"skin\":0}".to_owned());
        scene_nodes.push(joints.to_string());
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let joint_list = (0..joints)
        .map(|j| j.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[{}]}}],\"nodes\":[{}],\"skins\":[{{\"joints\":[{joint_list}],\"inverseBindMatrices\":{binds}}}]{meshes},\"animations\":[{}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]}}",
        scene_nodes.join(","),
        nodes.join(","),
        animations.join(","),
        accessors.join(","),
        views.join(","),
        bin.len()
    );
    let mut json = json.into_bytes();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut glb = Vec::with_capacity(28 + json.len() + bin.len());
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

fn admit(name: &str, bytes: &[u8]) -> asset_import::AnimatedGlbImportOutcome {
    import_animated_glb_asset(
        &SourceUri::RelativePath(format!("{name}.glb")),
        bytes,
        &ImportContext::default(),
    )
}

#[test]
fn a_game_character_with_hundreds_of_clips_is_one_glb() {
    // The Dark Messiah orc: 54 joints, 334 sequences, about 2 s each.
    let source = character_glb(54, 334, 60, true);
    let started = std::time::Instant::now();
    let outcome = admit("orc", &source);
    let took = started.elapsed();
    let imported = outcome
        .assets
        .unwrap_or_else(|| panic!("the orc is admitted: {:?}", outcome.diagnostics));
    assert_eq!(imported.animated_mesh.clips.len(), 334);
    assert_eq!(imported.receipt.joint_count, 54);
    eprintln!(
        "orc: {} bytes, {} channels, {} keyframes, admitted in {took:?}",
        source.len(),
        imported.receipt.channel_count,
        imported.receipt.keyframe_count
    );
}

#[test]
fn a_clip_pack_carries_only_its_skeleton_and_clips() {
    let pack = character_glb(54, 120, 60, false);
    let outcome = admit("orc.clips1", &pack);
    let imported = outcome
        .assets
        .unwrap_or_else(|| panic!("a skin-only pack is admitted: {:?}", outcome.diagnostics));
    assert_eq!(imported.animated_mesh.clips.len(), 120);
    let primary = admit("orc", &character_glb(54, 4, 30, true))
        .assets
        .expect("the primary is admitted");
    assert!(
        primary
            .animated_mesh
            .rig
            .as_ref()
            .unwrap()
            .is_clip_compatible_with(imported.animated_mesh.rig.as_ref().expect("the pack's rig")),
        "the pack shares the primary's rig"
    );
}
