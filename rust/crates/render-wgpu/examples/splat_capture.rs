//! Capture a splat GLB (`docs/splat-characters.md`) from the cameras the
//! splat acceptance compares with the web reference: front, both sides,
//! back and three-quarter, each looking at the figure's pelvis from
//! `--distance` at `--eye-height`, with a 60° vertical field of view. The
//! GLB and any clip pack are read from local paths: character bytes stay
//! out of the repository.
//!
//! ```text
//! cargo run -p render-wgpu --example splat_capture -- --glb PATH --out DIR
//!     [--size 1920x1080] [--eye-height 0.88] [--distance 2.2] [--occluded]
//!     [--clip-pack PATH --clip ID [--fit JOINT:K,A] [--at SECONDS] [--track SECONDS]]
//!     [--arms ABDUCT,FOREARM] [--multiplier R,G,B] [--fog DENSITY]
//!     [--pixel N [--palette PATH]] [--lit FLOOR]
//! ```
//!
//! - `--occluded`: a box between the camera and the figure's legs, a wall
//!   behind it.
//! - `--clip-pack`, `--clip`: play clip `ID` of a clip pack bound by joint
//!   names, looping, captured at `--at` seconds.
//! - `--fit JOINT:K,A`: fit the clip's translation of `JOINT` to
//!   (K·x, A + K·y, K·z).
//! - `--arms ABDUCT,FOREARM`: turn the upper arms and then the forearms
//!   further out, in degrees, about the model's forward axis (+z), on top
//!   of the clip.
//! - `--track SECONDS`: write `track.json`, the world positions of the
//!   pelvis, feet and balls of the feet (or the joints `--track-joints A,B`
//!   names) at 60 Hz from 0 to `SECONDS`.
//! - `--centre JOINT`: the joint the cameras look at (`pelvis` by default).
//! - `--web-views HEIGHT --times T1,T2,...`: instead of the views above,
//!   asset-pipeline's web harness cameras (`tests/splats/character.js`) for
//!   a figure HEIGHT metres tall: a 42° vertical field of view, 1.6 × HEIGHT
//!   from a target 0.5 × HEIGHT up over the pelvis, `front` (pitch 4°),
//!   `quarter` (45°, 8°), `side` (90°, 4°), `back` (180°, 4°), `right-side`
//!   (270°, 4°) and `three-quarter` (315°, 8°) around from the figure's
//!   front, each at every time: `<view>-t<index>.png`.
//! - `--multiplier R,G,B`: the splat colour multiplier.
//! - `--fog DENSITY`: exponential fog, grey, which reads the figure's depth.
//! - `--lit FLOOR`: the figure lit by the world's lights, keeping FLOOR of
//!   its baked colour however dark, under its own lights in place of the
//!   default ones: a sun from his front left and above, ambient light and a
//!   warm point light at his right front. Unlit captures to compare take the
//!   same lights with `--lit-scene`.
//! - `--pixel N`, `--palette PATH`: the figure on a grid of N-pixel blocks,
//!   snapped to a palette JSON's `colours` and `extra` (sRGB hex).
//! - `--pose-out`: write `pose.json`, every rig joint's world transform at
//!   the capture time, for a CPU check of the skinning.
//!
//! Writes `<view>.png` per view and `readout.json` (the splat readout, the
//! `splats` pass timing over the captures and every timed pass's median).

#[path = "../tests/support/mod.rs"]
mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{OffscreenTarget, RendererOptions};
use support::*;

const FIGURE: u64 = 1;
/// The joints `--track` writes unless `--track-joints` names others.
const TRACKED: [&str; 5] = ["pelvis", "foot_l", "foot_r", "ball_l", "ball_r"];
const TRACK_RATE: f64 = 60.0;

#[derive(Default)]
struct Arguments {
    glb: std::path::PathBuf,
    out: std::path::PathBuf,
    size: (u32, u32),
    eye_height: f64,
    distance: f64,
    occluded: bool,
    clip_pack: Option<std::path::PathBuf>,
    clip: Option<String>,
    fit: Option<(String, f32, f32)>,
    at: f64,
    track: Option<f64>,
    arms: Option<(f32, f32)>,
    multiplier: Option<[f32; 3]>,
    fog: Option<f32>,
    pixel_grid: u32,
    palette: Vec<[f32; 3]>,
    lit: Option<f32>,
    lit_scene: bool,
    pose_out: bool,
    track_joints: Option<Vec<String>>,
    centre: Option<String>,
    web_views: Option<f64>,
    times: Vec<f64>,
}

fn numbers<const N: usize>(text: &str, flag: &str) -> Result<[f32; N], String> {
    let values: Vec<f32> = text
        .split(',')
        .map(|value| value.trim().parse::<f32>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("{flag} takes {N} numbers"))?;
    values
        .try_into()
        .map_err(|_| format!("{flag} takes {N} numbers"))
}

/// A palette JSON (asset-pipeline `creatures/image_palette.py`): its
/// `colours` then its `extra`, sRGB hex, as linear colours.
fn palette(path: &str) -> Result<Vec<[f32; 3]>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
    let hex = |value: &serde_json::Value| -> Result<[f32; 3], String> {
        let text = value.as_str().ok_or("a palette colour is a hex string")?;
        let bits = u32::from_str_radix(text.trim_start_matches('#'), 16)
            .map_err(|_| format!("palette colour {text}"))?;
        Ok([16, 8, 0].map(|shift| srgb_linear(((bits >> shift) & 255) as f32 / 255.0)))
    };
    ["colours", "extra"]
        .iter()
        .filter_map(|key| json.get(key).and_then(serde_json::Value::as_array))
        .flatten()
        .map(hex)
        .collect()
}

fn srgb_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn arguments() -> Result<Arguments, String> {
    let mut arguments = Arguments {
        size: (1920, 1080),
        eye_height: 0.88,
        distance: 2.2,
        ..Arguments::default()
    };
    let (mut glb, mut out) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--glb" => glb = Some(value()?.into()),
            "--out" => out = Some(value()?.into()),
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size is WIDTHxHEIGHT")?;
                arguments.size = (
                    w.parse().map_err(|_| "--size width")?,
                    h.parse().map_err(|_| "--size height")?,
                );
            }
            "--eye-height" => {
                arguments.eye_height = value()?.parse().map_err(|_| "--eye-height")?;
            }
            "--distance" => arguments.distance = value()?.parse().map_err(|_| "--distance")?,
            "--occluded" => arguments.occluded = true,
            "--clip-pack" => arguments.clip_pack = Some(value()?.into()),
            "--clip" => arguments.clip = Some(value()?),
            "--fit" => {
                let text = value()?;
                let (joint, numbers_text) = text.rsplit_once(':').ok_or("--fit is JOINT:K,A")?;
                let [k, a] = numbers::<2>(numbers_text, "--fit")?;
                arguments.fit = Some((joint.to_owned(), k, a));
            }
            "--at" => arguments.at = value()?.parse().map_err(|_| "--at")?,
            "--track" => arguments.track = Some(value()?.parse().map_err(|_| "--track")?),
            "--arms" => {
                let [abduct, forearm] = numbers::<2>(&value()?, "--arms")?;
                arguments.arms = Some((abduct, forearm));
            }
            "--multiplier" => arguments.multiplier = Some(numbers::<3>(&value()?, "--multiplier")?),
            "--fog" => arguments.fog = Some(value()?.parse().map_err(|_| "--fog")?),
            "--pixel" => arguments.pixel_grid = value()?.parse().map_err(|_| "--pixel")?,
            "--lit" => {
                arguments.lit = Some(value()?.parse().map_err(|_| "--lit")?);
                arguments.lit_scene = true;
            }
            "--lit-scene" => arguments.lit_scene = true,
            "--palette" => arguments.palette = palette(&value()?)?,
            "--pose-out" => arguments.pose_out = true,
            "--track-joints" => {
                arguments.track_joints = Some(
                    value()?
                        .split(',')
                        .map(|name| name.trim().to_owned())
                        .collect(),
                );
            }
            "--centre" => arguments.centre = Some(value()?),
            "--web-views" => {
                arguments.web_views = Some(value()?.parse().map_err(|_| "--web-views")?);
            }
            "--times" => {
                arguments.times = value()?
                    .split(',')
                    .map(|time| time.trim().parse::<f64>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| "--times takes seconds")?;
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    arguments.glb = glb.ok_or("--glb PATH is required")?;
    arguments.out = out.ok_or("--out DIR is required")?;
    if arguments.clip_pack.is_some() != arguments.clip.is_some() {
        return Err("--clip-pack and --clip go together".to_owned());
    }
    Ok(arguments)
}

fn read(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| {
        eprintln!(
            "splat_capture: cannot read {}: {error} (character files are local, outside the repository)",
            path.display()
        );
        std::process::exit(2);
    })
}

fn admit(path: &std::path::Path, bytes: &[u8]) -> AnimatedMeshAsset {
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.file_name().map_or("asset.glb".into(), |name| {
            name.to_string_lossy().into_owned()
        })),
        bytes,
        &ImportContext::default(),
    );
    imported
        .assets
        .unwrap_or_else(|| {
            eprintln!(
                "splat_capture: {} is not admitted: {:?}",
                path.display(),
                imported.diagnostics
            );
            std::process::exit(1);
        })
        .animated_mesh
}

fn hex(hash: &str) -> &str {
    hash.trim_start_matches("sha256:")
}

/// A rotation of `degrees` about `axis` (unit), xyzw.
fn turn(axis: [f32; 3], degrees: f32) -> [f32; 4] {
    let (sin, cos) = (degrees.to_radians() * 0.5).sin_cos();
    [axis[0] * sin, axis[1] * sin, axis[2] * sin, cos]
}

fn main() {
    let arguments = arguments().unwrap_or_else(|error| {
        eprintln!("splat_capture: {error}");
        std::process::exit(2);
    });
    let bytes = read(&arguments.glb);
    std::fs::create_dir_all(&arguments.out).expect("create the output directory");

    let mut harness = Harness::new(RendererOptions {
        default_world_lights: !arguments.lit_scene,
        ..RendererOptions::default()
    });
    let (width, height) = arguments.size;
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 1);
    let mut asset = admit(&arguments.glb, &bytes);
    let hash = asset.content_hash.clone().expect("content hash");
    harness
        .resources
        .insert(&format!("animated-mesh-resource/{}", hex(&hash)), bytes);
    let playback = arguments.clip_pack.as_ref().map(|path| {
        let pack_bytes = read(path);
        let pack = admit(path, &pack_bytes);
        let pack_hash = pack.content_hash.clone().expect("content hash");
        harness.resources.insert(
            &format!("clip-pack-resource/{}", hex(&pack_hash)),
            pack_bytes,
        );
        let clip = arguments.clip.clone().expect("checked with --clip-pack");
        asset.clip_packs.push(AnimationClipPack {
            asset: format!("animation-clip-pack/{}", hex(&pack_hash)),
            runtime_format: pack.runtime_format,
            content_hash: pack_hash.clone(),
            rig: pack.rig.clone().expect("the clip pack names its rig"),
            clips: pack.clips.clone(),
            provenance: AnimationClipPackProvenance {
                producer: "splat_capture".to_owned(),
                source_hash: pack_hash,
                target_hash: hash.clone(),
                license: "local".to_owned(),
            },
            binding: AnimationClipPackBinding::JointNames,
            translation_fits: arguments
                .fit
                .iter()
                .map(|(joint, k, a)| AnimationClipTranslationFit {
                    clip: clip.clone(),
                    joint: joint.clone(),
                    scale: [*k; 3],
                    offset: [0.0, *a, 0.0],
                })
                .collect(),
        });
        AnimatedMeshPlaybackCommand::Play {
            clip,
            r#loop: AnimationLoopMode::Repeat,
            speed: 1.0,
            weight: 1.0,
            restart: true,
            fade_seconds: None,
            start_offset_seconds: None,
            start_paused: false,
        }
    });
    let rig_index = |name: &str| {
        asset
            .rig
            .as_ref()
            .and_then(|rig| rig.joints.iter().position(|joint| joint.id == name))
            .map(|index| index as u32)
    };
    let mut ops = vec![
        RenderDiff::DefineAnimatedMesh {
            asset: asset.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(FIGURE),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                playback_fade: None,
                inspection: AnimatedMeshInspection::default(),
                splat: SplatAppearance {
                    multiplier: arguments.multiplier.unwrap_or([1.0; 3]),
                    pixel_grid: arguments.pixel_grid,
                    palette: arguments.palette.clone(),
                    lighting: if arguments.lit.is_some() {
                        SplatLighting::Lit
                    } else {
                        SplatLighting::Unlit
                    },
                    light_floor: arguments.lit.unwrap_or(0.0),
                    ..SplatAppearance::default()
                },
                asset: asset.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback,
                metadata: RenderMetadata {
                    source_entity: Some(FIGURE),
                    ..RenderMetadata::default()
                },
                layer: RenderLayer::Scene,
                shadow_casting: Default::default(),
            },
        },
    ];
    if let Some((abduct, forearm)) = arguments.arms {
        // The figure faces +z: turning about +z lifts its left arm (toward
        // +x) and lowers its right one, so the right turns the other way.
        let mut overrides = Vec::new();
        for (bones, degrees) in [
            (["upperarm_l", "upperarm_r"], abduct),
            (["lowerarm_l", "lowerarm_r"], forearm),
        ] {
            for (bone, sign) in bones.iter().zip([1.0, -1.0]) {
                let joint = rig_index(bone).unwrap_or_else(|| panic!("the rig has no {bone}"));
                overrides.push(JointOverride {
                    joint,
                    space: PoseSpace::Model,
                    additive: true,
                    rotation: Some(turn([0.0, 0.0, 1.0], sign * degrees)),
                    translation: None,
                    scale: None,
                    weight: 1.0,
                });
            }
        }
        ops.push(RenderDiff::SetAnimatedMeshPose {
            handle: RenderHandle::new(FIGURE),
            pose: AnimatedMeshPose {
                overrides,
                ..AnimatedMeshPose::default()
            },
        });
    }
    if arguments.occluded {
        // Unlit, so their colours read back as written.
        for (id, color) in [
            ("material/occluder", [0.1, 0.5, 0.1, 1.0]),
            ("material/wall", [0.6, 0.6, 0.7, 1.0]),
        ] {
            ops.push(RenderDiff::DefineMaterial {
                material: RenderMaterialDescriptor {
                    unlit: true,
                    ..material(id, color, None)
                },
            });
        }
        ops.push(static_mesh(
            "mesh/occluder",
            box_mesh([-0.6, 0.0, -0.05], [0.6, 0.5, 0.05], |_| 0),
            "material/occluder",
        ));
        ops.push(static_mesh(
            "mesh/wall",
            box_mesh([-3.0, 0.0, -0.05], [3.0, 3.0, 0.05], |_| 0),
            "material/wall",
        ));
    }
    if arguments.lit_scene {
        ops.extend([
            RenderDiff::CreateLight {
                handle: RenderHandle::new(60),
                parent: None,
                light: LightDescriptor::Directional {
                    color: [1.0, 0.96, 0.88],
                    intensity: 3.0,
                    enabled: true,
                    // From his front left (+z, +x) and above, down and back.
                    direction: [-0.45, -0.75, -0.5],
                    range: None,
                    shadow_intent: LightShadowIntent::Disabled,
                    shadow: Default::default(),
                },
            },
            RenderDiff::CreateLight {
                handle: RenderHandle::new(61),
                parent: None,
                light: LightDescriptor::Ambient {
                    color: [0.75, 0.82, 1.0],
                    intensity: 0.45,
                    enabled: true,
                    shadow_intent: LightShadowIntent::Disabled,
                    shadow: Default::default(),
                    range: None,
                },
            },
            RenderDiff::CreateLight {
                handle: RenderHandle::new(62),
                parent: None,
                light: LightDescriptor::Point {
                    color: [1.0, 0.45, 0.15],
                    intensity: 4.0,
                    enabled: true,
                    position: [-0.9, 1.2, 0.7],
                    range: Some(3.0),
                    decay: 2.0,
                    shadow_intent: LightShadowIntent::Disabled,
                    shadow: Default::default(),
                },
            },
        ]);
    }
    if let Some(density) = arguments.fog {
        ops.push(RenderDiff::SetFog {
            fog: Some(FogDescriptor::Exponential {
                color: [0.5, 0.5, 0.5],
                density,
            }),
        });
    }
    harness.apply(ops);

    let joint_world = |harness: &Harness, name: &str| -> Option<[f32; 3]> {
        let index = rig_index(name)? as usize;
        let reading = harness.poses.joints(RenderHandle::new(FIGURE))?;
        Some(reading.latest.joints.get(index)?.world.translation)
    };
    if let Some(seconds) = arguments.track {
        let mut rows = Vec::new();
        let frames = (seconds * TRACK_RATE).round() as u64;
        for frame in 0..=frames {
            let time = frame as f64 / TRACK_RATE;
            harness.at(time);
            let tracked: Vec<String> = arguments
                .track_joints
                .clone()
                .unwrap_or_else(|| TRACKED.iter().map(|name| (*name).to_owned()).collect());
            let joints: Vec<String> = tracked
                .iter()
                .filter_map(|name| {
                    joint_world(&harness, name)
                        .map(|[x, y, z]| format!("\"{name}\": [{x}, {y}, {z}]"))
                })
                .collect();
            rows.push(format!("{{\"t\": {time}, {}}}", joints.join(", ")));
        }
        std::fs::write(
            arguments.out.join("track.json"),
            format!("[\n{}\n]\n", rows.join(",\n")),
        )
        .expect("write the track");
    }
    if arguments.at > 0.0 {
        harness.at(arguments.at.max(arguments.track.unwrap_or(0.0)));
    }
    if arguments.pose_out {
        if let (Some(rig), Some(reading)) = (
            asset.rig.as_ref(),
            harness.poses.joints(RenderHandle::new(FIGURE)),
        ) {
            let rows: Vec<String> = rig
                .joints
                .iter()
                .zip(&reading.latest.joints)
                .map(|(joint, posed)| {
                    format!(
                        "\"{}\": {{\"translation\": {:?}, \"rotation\": {:?}, \"scale\": {:?}}}",
                        joint.id, posed.world.translation, posed.world.rotation, posed.world.scale
                    )
                })
                .collect();
            std::fs::write(
                arguments.out.join("pose.json"),
                format!("{{\n{}\n}}\n", rows.join(",\n")),
            )
            .expect("write the pose");
        }
    }
    if let Some(figure) = arguments.web_views {
        let centre_joint = arguments.centre.as_deref().unwrap_or("pelvis");
        let times = if arguments.times.is_empty() {
            vec![arguments.at]
        } else {
            arguments.times.clone()
        };
        // Around from the figure's front (+z), toward its left (+x): the
        // harness's yaw, and its pitch.
        let views = [
            ("front", 0.0, 4.0),
            ("quarter", 45.0, 8.0),
            ("side", 90.0, 4.0),
            ("back", 180.0, 4.0),
            ("right-side", 270.0, 4.0),
            ("three-quarter", 315.0, 8.0),
        ];
        for (index, time) in times.iter().enumerate() {
            harness.at(*time);
            let [x, _, z] = joint_world(&harness, centre_joint).unwrap_or([0.0; 3]);
            let target = [f64::from(x), 0.5 * figure, f64::from(z)];
            for (name, around, pitch) in views {
                let (around, tilt): (f64, f64) = (f64::to_radians(around), f64::to_radians(pitch));
                let distance = 1.6 * figure;
                let position = [
                    target[0] + distance * tilt.cos() * around.sin(),
                    target[1] + distance * tilt.sin(),
                    target[2] + distance * tilt.cos() * around.cos(),
                ];
                let mut view = camera(position, -around.to_degrees(), -pitch);
                view.projection = render_host_contracts::RendererCameraProjection::Perspective {
                    fov_y_degrees: 42.0,
                    near: 0.01,
                    far: 200.0,
                };
                harness.render(&view);
                let (_, rgba) = harness.render(&view);
                let png =
                    render_wgpu::encode_png(width, height, &rgba).expect("encode the capture");
                std::fs::write(arguments.out.join(format!("{name}-t{index}.png")), png)
                    .expect("write the capture");
            }
        }
        return;
    }
    // The cameras look at the pelvis where it stands.
    let centre = joint_world(&harness, arguments.centre.as_deref().unwrap_or("pelvis"))
        .map_or([0.0, 0.0], |[x, _, z]| [f64::from(x), f64::from(z)]);

    let d = arguments.distance;
    let h = arguments.eye_height;
    let diagonal = d * std::f64::consts::FRAC_1_SQRT_2;
    // Engine yaw 0 looks down −z and positive yaw turns toward +x; the
    // figure faces +z.
    let views = [
        ("front", [0.0, d], 0.0),
        ("left", [d, 0.0], -90.0),
        ("right", [-d, 0.0], 90.0),
        ("back", [0.0, -d], 180.0),
        ("three-quarter", [diagonal, diagonal], -45.0),
    ];
    let mut handle = 100;
    for (name, [x, z], yaw) in views {
        let position = [centre[0] + x, h, centre[1] + z];
        if arguments.occluded {
            // The box between this camera and the figure, the wall behind it.
            let toward = [x / d, z / d];
            let place = |distance: f64, id: &str, handle: u64| {
                instance(
                    handle,
                    None,
                    id,
                    transform(
                        [
                            (centre[0] + toward[0] * distance) as f32,
                            0.0,
                            (centre[1] + toward[1] * distance) as f32,
                        ],
                        // Square to the camera: a transform's yaw turns the
                        // other way from a camera's.
                        -yaw as f32,
                        [1.0; 3],
                    ),
                )
            };
            harness.apply(vec![
                place(0.9, "mesh/occluder", handle),
                place(-1.2, "mesh/wall", handle + 1),
            ]);
        }
        let camera = camera(position, yaw, 0.0);
        // Twice: the second frame has the first's readout.
        harness.render(&camera);
        let (_, rgba) = harness.render(&camera);
        let png = render_wgpu::encode_png(width, height, &rgba).expect("encode the capture");
        std::fs::write(arguments.out.join(format!("{name}.png")), png).expect("write the capture");
        if arguments.occluded {
            harness.apply(vec![
                RenderDiff::Destroy {
                    handle: RenderHandle::new(handle),
                },
                RenderDiff::Destroy {
                    handle: RenderHandle::new(handle + 1),
                },
            ]);
            handle += 2;
        }
    }
    // Time the splats over a few more frames of the front view.
    let front = camera([centre[0], h, centre[1] + d], 0.0, 0.0);
    for _ in 0..30 {
        harness.render(&front);
    }
    let readout = harness.renderer.gpu_readout();
    let timing = readout
        .passes
        .iter()
        .find(|timing| timing.pass == "splats")
        .map(|timing| (timing.timed_frames, timing.median_gpu_ms));
    let passes: Vec<String> = readout
        .passes
        .iter()
        .filter(|timing| timing.timed_frames > 0)
        .map(|timing| format!("\"{}\": {:.4}", timing.pass, timing.median_gpu_ms))
        .collect();
    let report = format!(
        "{{\"runs\": {}, \"instances\": {}, \"gaussians\": {}, \"refused\": {:?}, \"splats_pass\": {:?}, \"passes\": {{{}}}, \"adapter\": {:?}}}\n",
        readout.splats.runs,
        readout.splats.instances,
        readout.splats.gaussians,
        readout.splats.refused,
        timing,
        passes.join(", "),
        harness.gpu.adapter_summary(),
    );
    std::fs::write(arguments.out.join("readout.json"), &report).expect("write the readout");
    print!("{report}");
}
