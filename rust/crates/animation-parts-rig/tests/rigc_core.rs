//! Poses rig-c's gallery rigs and a feature rig as rig-c's own core does.
//!
//! Each `<rig>.model.json` under `tests/fixtures` is a `rigc build --pack`
//! output; its `<rig>.expected.json` holds the poses `@rig-c/core` gave for
//! the same steps (`openTrack` for one looping clip, `openTracks` for a
//! crossfade script). Bones are compared as doubles, drawn vertices as the
//! float32 the pose holds.

use std::path::Path;

use animation_parts_rig::{BlendMode, BoneChannel, Play, Player, Pose, Rig};
use serde_json::Value;

const FIXTURES: [&str; 9] = [
    "squash",
    "turned",
    "flex",
    "portrait",
    "nod",
    "features",
    "walk",
    "constraints",
    "townsperson",
];

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn numbers(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

fn close(what: &str, got: f64, want: f64, abs: f64, rel: f64) {
    assert!(
        (got - want).abs() <= abs + rel * want.abs(),
        "{what}: got {got}, rig-c's core {want}"
    );
}

fn blend_name(blend: BlendMode) -> &'static str {
    match blend {
        BlendMode::Normal => "Normal",
        BlendMode::Additive => "Additive",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
    }
}

struct Worst {
    bone: f64,
    vertex: f64,
}

fn compare(
    rig: &Rig,
    regions: &serde_json::Map<String, Value>,
    pose: &Pose,
    want: &Value,
    at: &str,
    worst: &mut Worst,
) {
    close(
        &format!("{at} clip time"),
        pose.animation_time,
        want["time"].as_f64().unwrap(),
        1e-12,
        0.0,
    );
    let bones = numbers(&want["bones"]);
    assert_eq!(bones.len(), pose.bones.len() * 6, "{at}: bone count");
    for (i, b) in pose.bones.iter().enumerate() {
        for (k, got) in [b.a, b.b, b.c, b.d, b.x, b.y].into_iter().enumerate() {
            let want = bones[i * 6 + k];
            worst.bone = worst.bone.max((got - want).abs() / want.abs().max(1.0));
            close(
                &format!("{at} bone \"{}\" [{k}]", rig.bone_name(i)),
                got,
                want,
                1e-9,
                1e-12,
            );
        }
    }

    let drawn = want["drawn"].as_array().unwrap();
    let names = |list: &mut dyn Iterator<Item = String>| list.collect::<Vec<_>>().join(", ");
    assert_eq!(
        pose.pieces.len(),
        drawn.len(),
        "{at}: drawn slots [{}], rig-c's core [{}]",
        names(&mut pose.pieces.iter().map(|p| rig.slot_name(p.slot).to_owned())),
        names(&mut drawn.iter().map(|d| d[0].as_str().unwrap().to_owned())),
    );
    for (piece, d) in pose.pieces.iter().zip(drawn) {
        let key = d[0].as_str().unwrap();
        let mut parts = key.split('|');
        let (slot, attachment) = (parts.next().unwrap(), parts.next().unwrap());
        let at = format!("{at} slot \"{slot}\"");
        assert_eq!(rig.slot_name(piece.slot), slot, "{at}: draw order");
        assert_eq!(
            rig.attachment_name(piece.attachment),
            attachment,
            "{at}: attachment"
        );
        let region = &regions[key];
        let drawable = rig.drawable(piece.drawable);
        assert_eq!(drawable.page, piece.page);
        assert_eq!(
            rig.pages()[piece.page].name,
            region["page"].as_str().unwrap(),
            "{at}: page"
        );
        let triangles: Vec<u32> = region["triangles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(drawable.triangles, triangles, "{at}: triangles");
        let uvs = numbers(&region["uvs"]);
        assert_eq!(drawable.uvs.len() * 2, uvs.len(), "{at}: uv count");
        for (i, uv) in drawable.uvs.iter().flatten().enumerate() {
            close(&format!("{at} uv {i}"), f64::from(*uv), uvs[i], 1e-7, 0.0);
        }
        let vertices = numbers(&d[1]);
        let posed = &pose.vertices[piece.vertices.clone()];
        assert_eq!(posed.len() * 2, vertices.len(), "{at}: vertex count");
        for (i, v) in posed.iter().flatten().enumerate() {
            let (got, want) = (f64::from(*v), vertices[i]);
            worst.vertex = worst.vertex.max((got - want).abs() / want.abs().max(1.0));
            close(&format!("{at} vertex {i}"), got, want, 1e-4, 2e-7);
        }
        for (i, (got, want)) in piece.color.iter().zip(numbers(&d[2])).enumerate() {
            close(
                &format!("{at} colour {i}"),
                f64::from(*got),
                want,
                1e-6,
                0.0,
            );
        }
        match (&piece.dark, &d[3]) {
            (None, Value::Null) => {}
            (Some(dark), want) => {
                for (got, want) in dark.iter().zip(numbers(want)) {
                    close(&format!("{at} dark"), f64::from(*got), want, 1e-6, 0.0);
                }
            }
            (None, _) => panic!("{at}: no dark colour, rig-c's core has one"),
        }
        assert_eq!(
            blend_name(piece.blend),
            d[4].as_str().unwrap(),
            "{at}: blend"
        );
    }

    let events = want["events"].as_array().unwrap();
    assert_eq!(pose.events.len(), events.len(), "{at}: fired events");
    for (got, e) in pose.events.iter().zip(events) {
        assert_eq!(
            rig.event_name(got.event),
            e[0].as_str().unwrap(),
            "{at}: event"
        );
        close(
            &format!("{at} event time"),
            got.time,
            e[1].as_f64().unwrap(),
            1e-12,
            0.0,
        );
        assert_eq!(got.int, e[2].as_i64().unwrap(), "{at}: event int");
        close(
            &format!("{at} event float"),
            got.float,
            e[3].as_f64().unwrap(),
            1e-12,
            0.0,
        );
        assert_eq!(
            got.string,
            e[4].as_str().unwrap_or(""),
            "{at}: event string"
        );
    }
}

#[test]
fn poses_match_rigc_core() {
    let mut report = Vec::new();
    for name in FIXTURES {
        let rig = Rig::from_json(&fixture(&format!("{name}.model.json")))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected: Value =
            serde_json::from_str(&fixture(&format!("{name}.expected.json"))).unwrap();
        let regions = expected["regions"].as_object().unwrap();
        let mut worst = Worst {
            bone: 0.0,
            vertex: 0.0,
        };
        let mut poses = 0;
        for case in expected["cases"].as_array().unwrap() {
            let mut player = Player::new(&rig);
            if let Some(skin) = case["skin"].as_str() {
                player.set_skin(&rig, Some(rig.find_skin(skin).unwrap()));
            }
            for drive in case["drive"].as_array().into_iter().flatten() {
                let bone = rig.find_bone(drive[0].as_str().unwrap()).unwrap();
                let channel = match drive[1].as_str().unwrap() {
                    "rotation" => BoneChannel::Rotation,
                    "x" => BoneChannel::X,
                    "y" => BoneChannel::Y,
                    "scaleX" => BoneChannel::ScaleX,
                    "scaleY" => BoneChannel::ScaleY,
                    "shearX" => BoneChannel::ShearX,
                    "shearY" => BoneChannel::ShearY,
                    other => panic!("no channel {other}"),
                };
                player.drive(bone, channel, drive[2].as_f64());
            }
            let at = format!(
                "{name} {} (skin {})",
                case["name"].as_str().unwrap(),
                case["skin"]
            );
            let want = case["poses"].as_array().unwrap();
            let mut pose = Pose::default();
            let mut index = 0;
            let mut check = |player: &mut Player, dt: f64, pose: &mut Pose| {
                player.advance(&rig, dt, pose);
                compare(
                    &rig,
                    regions,
                    pose,
                    &want[index],
                    &format!("{at} pose {index}"),
                    &mut worst,
                );
                index += 1;
            };
            if case["mode"] == "track" {
                let animation = rig
                    .find_animation(case["animation"].as_str().unwrap())
                    .unwrap();
                player.play(
                    animation,
                    Play {
                        looping: case["loop"].as_bool().unwrap(),
                        crossfade: 0.0,
                    },
                );
                check(&mut player, 0.0, &mut pose);
                for dt in numbers(&case["steps"]) {
                    check(&mut player, dt, &mut pose);
                }
            } else {
                for op in case["ops"].as_array().unwrap() {
                    if let Some(play) = op.get("play") {
                        let animation = rig.find_animation(play.as_str().unwrap()).unwrap();
                        player.play(
                            animation,
                            Play {
                                looping: op["loop"].as_bool().unwrap(),
                                crossfade: op["mix"].as_f64().unwrap(),
                            },
                        );
                    } else if let Some(stop) = op.get("stop") {
                        player.stop(stop.as_f64().unwrap());
                    } else {
                        check(&mut player, op["step"].as_f64().unwrap(), &mut pose);
                    }
                }
            }
            assert_eq!(index, want.len(), "{at}: pose count");
            poses += index;
        }
        report.push(format!(
            "{name}: {poses} poses, worst bone {:.1e}, worst vertex {:.1e}",
            worst.bone, worst.vertex
        ));
    }
    println!("{}", report.join("\n"));
}

/// Every sampled pose's draw order and attachment show through `pieces`;
/// this also checks a rig no fixture shows under its named skin draws its
/// skin-only slot once the skin is applied.
#[test]
fn a_named_skin_activates_its_bone_and_slot() {
    let rig = Rig::from_json(&fixture("features.model.json")).unwrap();
    let slot = rig.find_slot("s_skin").unwrap();
    let mut player = Player::new(&rig);
    let mut pose = Pose::default();
    player.advance(&rig, 0.0, &mut pose);
    assert!(
        !pose.pieces.iter().any(|p| p.slot == slot),
        "the default skin alone does not fill s_skin"
    );
    player.set_skin(&rig, rig.find_skin("alt"));
    player.advance(&rig, 0.0, &mut pose);
    assert!(
        pose.pieces.iter().any(|p| p.slot == slot),
        "skin alt fills s_skin and activates its bone"
    );
}

#[test]
fn other_versions_and_unevaluated_constraints_are_refused_by_name() {
    let text = fixture("squash.model.json");
    let older = text.replacen("\"rigc-compiled/3\"", "\"rigc-compiled/2\"", 1);
    let error = Rig::from_json(&older).err().unwrap().to_string();
    assert!(
        error.contains("\"rigc-compiled/2\"") && error.contains("\"rigc-compiled/3\""),
        "{error}"
    );

    let mut doc: Value = serde_json::from_str(&fixture("constraints.model.json")).unwrap();
    let list = doc["constraints"].as_array_mut().unwrap();
    list.push(serde_json::json!({ "kind": "physics", "name": "hair", "bone": "src", "x": 1 }));
    list.push(serde_json::json!({ "kind": "slider", "name": "look", "animation": "hold" }));
    list.push(serde_json::json!({ "kind": "path", "name": "rail", "bones": ["tw"] }));
    let error = Rig::from_json(&doc.to_string()).err().unwrap().to_string();
    assert!(
        error.contains("physics \"hair\"")
            && error.contains("slider \"look\"")
            && error.contains("path \"rail\"")
            && !error.contains("ik")
            && !error.contains("transform"),
        "{error}"
    );
}

/// rig-c's mixer refuses clips that key constraint timelines, so a
/// crossfade from one has no oracle: the Engine blends their values like the
/// clip's other numbers, and once the fade ends the pose is the new clip's
/// alone.
#[test]
fn constraint_timelines_fade_out_with_their_clip() {
    let rig = Rig::from_json(&fixture("constraints.model.json")).unwrap();
    let (reach, hold) = (
        rig.find_animation("reach").unwrap(),
        rig.find_animation("hold").unwrap(),
    );
    let dt = 1.0 / 30.0;
    let alone = {
        let mut player = Player::new(&rig);
        player.play(
            hold,
            Play {
                looping: true,
                crossfade: 0.0,
            },
        );
        let mut pose = Pose::default();
        player.advance(&rig, 0.0, &mut pose);
        for _ in 0..30 {
            player.advance(&rig, dt, &mut pose);
        }
        pose
    };
    let mut player = Player::new(&rig);
    let mut pose = Pose::default();
    player.play(
        reach,
        Play {
            looping: true,
            crossfade: 0.0,
        },
    );
    player.advance(&rig, 0.0, &mut pose);
    for _ in 0..12 {
        player.advance(&rig, dt, &mut pose);
    }
    player.play(
        hold,
        Play {
            looping: true,
            crossfade: 0.4,
        },
    );
    player.advance(&rig, 0.0, &mut pose);
    for _ in 0..30 {
        player.advance(&rig, dt, &mut pose);
    }
    assert_eq!(pose.bones, alone.bones);
}
