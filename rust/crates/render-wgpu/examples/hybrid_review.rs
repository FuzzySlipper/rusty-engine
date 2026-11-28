//! Visual review of a splat or hybrid (base mesh + splats) figure under
//! named lighting, from named views, through a clip (#11211 visual round).
//! Experiment tooling: character bytes stay out of the repository.
//!
//! ```text
//! cargo run --release -p render-wgpu --example hybrid_review -- --glb PATH --out DIR
//!     [--size 1280x1440] [--light noon|golden|overcast|night|dungeon]
//!     [--clip-pack PATH --clip ID [--fit JOINT:K,A]] [--times T1,T2,...]
//!     [--frames N --fps F --from T] [--views NAME:AROUND:PITCH:DIST:AIM,...]
//!     [--lit FLOOR] [--caster none|splats|ellipsoids|core]
//!     [--mesh-casts on|off] [--fade NEAR,FAR] [--bias METRES] [--pixel N] [--fov DEG] [--material]
//!     [--close BOOST,NEAR,FAR] [--close-bias METRES] [--shadow-offset METRES] [--dolly DIST0,DIST1,AIM0,AIM1]
//! ```
//!
//! - Views look at the pelvis (plus AIM metres up) from DIST metres,
//!   AROUND degrees round from the figure's front (+z) toward its left (+x)
//!   and PITCH degrees above it (negative: from below).
//! - Lights and walls stand where the figure is at the first time, so a
//!   walk moves past the torch.
//! - Writes `<view>-<frame>.png`.

#[path = "../tests/support/mod.rs"]
mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{OffscreenTarget, RendererOptions};
use support::*;

const FIGURE: u64 = 1;

struct View {
    name: String,
    around: f64,
    pitch: f64,
    distance: f64,
    aim: f64,
}

struct Arguments {
    glb: std::path::PathBuf,
    out: std::path::PathBuf,
    size: (u32, u32),
    light: String,
    clip_pack: Option<std::path::PathBuf>,
    clip: Option<String>,
    fit: Option<(String, f32, f32)>,
    times: Vec<f64>,
    views: Vec<View>,
    lit: f32,
    caster: SplatShadowCaster,
    mesh_casts: bool,
    fade: [f32; 2],
    bias: f32,
    pixel: u32,
    fov: f64,
    material: bool,
    close: [f32; 3],
    close_bias: f32,
    shadow_offset: f32,
    dolly: Option<[f64; 4]>,
}

fn parse_views(text: &str) -> Result<Vec<View>, String> {
    text.split(',')
        .map(|item| {
            let parts: Vec<&str> = item.split(':').collect();
            if parts.len() != 5 {
                return Err(format!("view {item} is NAME:AROUND:PITCH:DIST:AIM"));
            }
            let number = |i: usize| parts[i].parse::<f64>().map_err(|_| format!("view {item}"));
            Ok(View {
                name: parts[0].to_owned(),
                around: number(1)?,
                pitch: number(2)?,
                distance: number(3)?,
                aim: number(4)?,
            })
        })
        .collect()
}

fn arguments() -> Result<Arguments, String> {
    let mut a = Arguments {
        glb: Default::default(),
        out: Default::default(),
        size: (1280, 1440),
        light: "noon".to_owned(),
        clip_pack: None,
        clip: None,
        fit: None,
        times: vec![0.0],
        views: parse_views(
            "front:0:4:2.4:0.1,quarter:40:10:2.4:0.1,high:150:38:2.6:0.0,low:300:-18:2.0:0.25,face:20:6:0.9:0.62",
        )?,
        lit: 0.0,
        caster: SplatShadowCaster::None,
        mesh_casts: true,
        fade: [0.0, 0.0],
        bias: 0.0,
        pixel: 0,
        fov: 40.0,
        material: false,
        close: [0.0; 3],
        close_bias: 0.0,
        shadow_offset: 0.0,
        dolly: None,
    };
    let (mut frames, mut fps, mut from) = (None::<u32>, 30.0, 0.0);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--glb" => a.glb = value()?.into(),
            "--out" => a.out = value()?.into(),
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size is WxH")?;
                a.size = (w.parse().map_err(|_| "--size")?, h.parse().map_err(|_| "--size")?);
            }
            "--light" => a.light = value()?,
            "--clip-pack" => a.clip_pack = Some(value()?.into()),
            "--clip" => a.clip = Some(value()?),
            "--fit" => {
                let text = value()?;
                let (joint, rest) = text.rsplit_once(':').ok_or("--fit is JOINT:K,A")?;
                let (k, at) = rest.split_once(',').ok_or("--fit is JOINT:K,A")?;
                a.fit = Some((
                    joint.to_owned(),
                    k.parse().map_err(|_| "--fit")?,
                    at.parse().map_err(|_| "--fit")?,
                ));
            }
            "--times" => {
                a.times = value()?
                    .split(',')
                    .map(|t| t.parse().map_err(|_| "--times".to_owned()))
                    .collect::<Result<_, _>>()?;
            }
            "--frames" => frames = Some(value()?.parse().map_err(|_| "--frames")?),
            "--fps" => fps = value()?.parse().map_err(|_| "--fps")?,
            "--from" => from = value()?.parse().map_err(|_| "--from")?,
            "--views" => a.views = parse_views(&value()?)?,
            "--lit" => a.lit = value()?.parse().map_err(|_| "--lit")?,
            "--caster" => {
                a.caster = match value()?.as_str() {
                    "none" => SplatShadowCaster::None,
                    "splats" => SplatShadowCaster::Splats,
                    "ellipsoids" => SplatShadowCaster::Ellipsoids,
                    "core" => SplatShadowCaster::Core,
                    _ => return Err("--caster is none, splats, ellipsoids or core".to_owned()),
                }
            }
            "--mesh-casts" => a.mesh_casts = value()? == "on",
            "--fade" => {
                let text = value()?;
                let (n, f) = text.split_once(',').ok_or("--fade is NEAR,FAR")?;
                a.fade = [n.parse().map_err(|_| "--fade")?, f.parse().map_err(|_| "--fade")?];
            }
            "--bias" => a.bias = value()?.parse().map_err(|_| "--bias")?,
            "--pixel" => a.pixel = value()?.parse().map_err(|_| "--pixel")?,
            "--fov" => a.fov = value()?.parse().map_err(|_| "--fov")?,
            "--material" => a.material = true,
            "--close-bias" => a.close_bias = value()?.parse().map_err(|_| "--close-bias")?,
            "--shadow-offset" => a.shadow_offset = value()?.parse().map_err(|_| "--shadow-offset")?,
            "--close" => {
                let v: Vec<f32> = value()?.split(',').map(|x| x.parse().map_err(|_| "--close")).collect::<Result<_, _>>()?;
                a.close = [v[0], v[1], v[2]];
            }
            "--dolly" => {
                let v: Vec<f64> = value()?.split(',').map(|x| x.parse().map_err(|_| "--dolly")).collect::<Result<_, _>>()?;
                a.dolly = Some([v[0], v[1], v[2], v[3]]);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if let Some(n) = frames {
        a.times = (0..n).map(|i| from + f64::from(i) / fps).collect();
    }
    if a.glb.as_os_str().is_empty() || a.out.as_os_str().is_empty() {
        return Err("--glb and --out are required".to_owned());
    }
    Ok(a)
}

fn admit(path: &std::path::Path, bytes: &[u8]) -> AnimatedMeshAsset {
    let name = path.file_name().map_or("asset.glb".into(), |n| n.to_string_lossy().into_owned());
    let imported = import_animated_glb_asset(&SourceUri::RelativePath(name.clone()), bytes, &ImportContext::default());
    imported
        .assets
        .unwrap_or_else(|| panic!("{name} is not admitted: {:?}", imported.diagnostics))
        .animated_mesh
}

fn hex(hash: &str) -> &str {
    hash.trim_start_matches("sha256:")
}

fn light(handle: u64, light: LightDescriptor) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(handle),
        parent: None,
        light,
    }
}

fn sun(color: [f32; 3], intensity: f32, direction: [f32; 3]) -> LightDescriptor {
    LightDescriptor::Directional {
        color,
        intensity,
        enabled: true,
        direction,
        range: Some(12.0),
        shadow_intent: LightShadowIntent::Requested,
        shadow: Default::default(),
    }
}

fn ambient(color: [f32; 3], intensity: f32) -> LightDescriptor {
    LightDescriptor::Ambient {
        color,
        intensity,
        enabled: true,
        range: None,
        shadow_intent: LightShadowIntent::Disabled,
        shadow: Default::default(),
    }
}

fn hemisphere(sky: [f32; 3], ground: [f32; 3], intensity: f32) -> LightDescriptor {
    LightDescriptor::Hemisphere {
        color: sky,
        ground_color: ground,
        intensity,
        enabled: true,
    }
}

fn torch(position: [f32; 3], intensity: f32, range: f32) -> LightDescriptor {
    LightDescriptor::Point {
        color: [1.0, 0.55, 0.22],
        intensity,
        enabled: true,
        position,
        range: Some(range),
        decay: 2.0,
        shadow_intent: LightShadowIntent::Requested,
        shadow: Default::default(),
    }
}

/// The lights of a preset, relative to `at` (the figure's feet), with the
/// torch positions (for their markers).
fn preset(name: &str, at: [f32; 3]) -> (Vec<LightDescriptor>, Vec<[f32; 3]>) {
    let p = |x: f32, y: f32, z: f32| [at[0] + x, at[1] + y, at[2] + z];
    match name {
        // High sun from his front left, sky and bounce.
        "noon" => (
            vec![
                sun([1.0, 0.96, 0.9], 3.2, [-0.35, -0.85, -0.4]),
                hemisphere([0.55, 0.65, 0.9], [0.32, 0.28, 0.22], 0.55),
            ],
            vec![],
        ),
        // Low warm sun behind his right shoulder: rim light, a shaded front.
        "golden" => (
            vec![
                sun([1.0, 0.7, 0.42], 3.0, [0.45, -0.3, 0.85]),
                hemisphere([0.35, 0.42, 0.62], [0.25, 0.18, 0.12], 0.45),
            ],
            vec![],
        ),
        // Sky only.
        "overcast" => (
            vec![
                hemisphere([0.75, 0.78, 0.85], [0.3, 0.28, 0.25], 1.0),
                sun([0.9, 0.92, 1.0], 0.5, [-0.2, -0.95, -0.2]),
            ],
            vec![],
        ),
        // Moonlight and a wall torch at his front right.
        "night" => {
            let t = p(-1.1, 1.7, 1.2);
            (
                vec![
                    sun([0.55, 0.65, 1.0], 0.35, [0.3, -0.8, -0.5]),
                    ambient([0.25, 0.3, 0.5], 0.12),
                    torch(t, 9.0, 7.0),
                ],
                vec![t],
            )
        }
        // No sky: a torch on the wall behind his left and one ahead.
        "dungeon" => {
            let (a, b) = (p(1.3, 1.8, -0.9), p(-0.9, 1.5, 2.4));
            (
                vec![
                    ambient([0.35, 0.3, 0.28], 0.05),
                    torch(a, 8.0, 7.0),
                    torch(b, 5.0, 6.0),
                ],
                vec![a, b],
            )
        }
        // One torch at his front right, nothing else: does the figure take it?
        "torch-test" => {
            let t = p(-0.8, 1.4, 1.0);
            (vec![torch(t, 6.0, 6.0)], vec![t])
        }
        other => panic!("unknown light preset {other}"),
    }
}

fn main() {
    let a = arguments().unwrap_or_else(|error| {
        eprintln!("hybrid_review: {error}");
        std::process::exit(2);
    });
    let bytes = std::fs::read(&a.glb).expect("read the GLB");
    std::fs::create_dir_all(&a.out).expect("create the output directory");
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let (width, height) = a.size;
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 1);
    let mut asset = admit(&a.glb, &bytes);
    let hash = asset.content_hash.clone().expect("content hash");
    harness
        .resources
        .insert(&format!("animated-mesh-resource/{}", hex(&hash)), bytes);
    let playback = a.clip_pack.as_ref().map(|path| {
        let pack_bytes = std::fs::read(path).expect("read the clip pack");
        let pack = admit(path, &pack_bytes);
        let pack_hash = pack.content_hash.clone().expect("content hash");
        harness.resources.insert(
            &format!("clip-pack-resource/{}", hex(&pack_hash)),
            pack_bytes,
        );
        let clip = a.clip.clone().expect("--clip with --clip-pack");
        asset.clip_packs.push(AnimationClipPack {
            asset: format!("animation-clip-pack/{}", hex(&pack_hash)),
            runtime_format: pack.runtime_format,
            content_hash: pack_hash.clone(),
            rig: pack.rig.clone().expect("the clip pack names its rig"),
            clips: pack.clips.clone(),
            provenance: AnimationClipPackProvenance {
                producer: "hybrid_review".to_owned(),
                source_hash: pack_hash,
                target_hash: hash.clone(),
                license: "local".to_owned(),
            },
            binding: AnimationClipPackBinding::JointNames,
            translation_fits: a
                .fit
                .iter()
                .map(|(joint, k, at)| AnimationClipTranslationFit {
                    clip: clip.clone(),
                    joint: joint.clone(),
                    scale: [*k; 3],
                    offset: [0.0, *at, 0.0],
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
            start_normalized_time: 0.0,
            start_paused: false,
        }
    });
    let rig_index = |name: &str| {
        asset
            .rig
            .as_ref()
            .and_then(|rig| rig.joints.iter().position(|joint| joint.id == name))
    };
    harness.apply(vec![
        RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor {
                shadows: true,
                ..RendererSettingsDescriptor::DEFAULT
            },
        },
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
                    pixel_grid: a.pixel,
                    lighting: SplatLighting::Lit,
                    light_floor: a.lit,
                    shadow_caster: a.caster,
                    fade_near: a.fade[0],
                    fade_far: a.fade[1],
                    surface_bias: a.bias,
                    material: a.material,
                    close_boost: a.close[0],
                    close_near: a.close[1],
                    close_far: a.close[2],
                    close_bias: a.close_bias,
                    shadow_offset: a.shadow_offset,
                    ..SplatAppearance::default()
                },
                splat_groups: Vec::new(),
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
                shadow_casting: if a.mesh_casts {
                    ShadowCasting::Cast
                } else {
                    ShadowCasting::None
                },
            },
        },
    ]);

    let pelvis = |harness: &Harness| -> [f64; 3] {
        let index = rig_index("pelvis")
            .or_else(|| rig_index("mixamorig:Hips"))
            .expect("a pelvis joint");
        let reading = harness.poses.joints(RenderHandle::new(FIGURE)).expect("posed");
        let [x, y, z] = reading.latest.joints[index].world.translation;
        [f64::from(x), f64::from(y), f64::from(z)]
    };

    harness.at(a.times[0]);
    let start = pelvis(&harness);
    let feet = [start[0] as f32, 0.0, start[2] as f32];
    let (lights, torches) = preset(&a.light, feet);
    let mut scene = Vec::new();
    for (index, descriptor) in lights.into_iter().enumerate() {
        scene.push(light(60 + index as u64, descriptor));
    }
    // Floor, a wall behind him and one at his right, a marker at each torch.
    scene.push(RenderDiff::DefineMaterial {
        material: material("material/floor", [0.34, 0.31, 0.27, 1.0], None),
    });
    scene.push(RenderDiff::DefineMaterial {
        material: material("material/wall", [0.42, 0.39, 0.35, 1.0], None),
    });
    scene.push(RenderDiff::DefineMaterial {
        material: RenderMaterialDescriptor {
            unlit: true,
            ..material("material/flame", [1.0, 0.7, 0.3, 1.0], None)
        },
    });
    scene.push(static_mesh(
        "mesh/floor",
        box_mesh([-10.0, -0.1, -10.0], [10.0, 0.0, 14.0], |_| 0),
        "material/floor",
    ));
    scene.push(static_mesh(
        "mesh/back-wall",
        box_mesh([-4.0, 0.0, -0.1], [4.0, 3.5, 0.1], |_| 0),
        "material/wall",
    ));
    scene.push(static_mesh(
        "mesh/side-wall",
        box_mesh([-0.1, 0.0, -2.0], [0.1, 3.5, 6.0], |_| 0),
        "material/wall",
    ));
    scene.push(static_mesh(
        "mesh/flame",
        box_mesh([-0.04, -0.06, -0.04], [0.04, 0.06, 0.04], |_| 0),
        "material/flame",
    ));
    scene.push(instance(90, None, "mesh/floor", transform(feet, 0.0, [1.0; 3])));
    scene.push(instance(
        91,
        None,
        "mesh/back-wall",
        transform([feet[0], 0.0, feet[2] - 2.2], 0.0, [1.0; 3]),
    ));
    scene.push(instance(
        92,
        None,
        "mesh/side-wall",
        transform([feet[0] - 1.9, 0.0, feet[2]], 0.0, [1.0; 3]),
    ));
    for (index, t) in torches.iter().enumerate() {
        scene.push(instance(95 + index as u64, None, "mesh/flame", transform(*t, 0.0, [1.0; 3])));
    }
    harness.apply(scene);

    for (frame, time) in a.times.iter().enumerate() {
        harness.at(*time);
        let centre = pelvis(&harness);
        if frame == 0 {
            eprintln!("pelvis at {centre:?}");
        }
        // A dolly moves every view from one distance and aim to another over
        // the frames, eased.
        let along = if a.times.len() > 1 { frame as f64 / (a.times.len() - 1) as f64 } else { 0.0 };
        let eased = along * along * (3.0 - 2.0 * along);
        for view in &a.views {
            let (distance, aim) = match a.dolly {
                Some([d0, d1, a0, a1]) => (d0 + (d1 - d0) * eased, a0 + (a1 - a0) * eased),
                None => (view.distance, view.aim),
            };
            let view = View { name: view.name.clone(), distance, aim, ..*view };
            let target = [centre[0], centre[1] + view.aim, centre[2]];
            let (around, tilt) = (view.around.to_radians(), view.pitch.to_radians());
            let position = [
                target[0] + view.distance * tilt.cos() * around.sin(),
                target[1] + view.distance * tilt.sin(),
                target[2] + view.distance * tilt.cos() * around.cos(),
            ];
            let mut camera = camera(position, -view.around, -view.pitch);
            camera.projection = render_host_contracts::RendererCameraProjection::Perspective {
                fov_y_degrees: a.fov,
                near: 0.02,
                far: 100.0,
            };
            harness.render(&camera);
            let (_, rgba) = harness.render(&camera);
            let png = render_wgpu::encode_png(width, height, &rgba).expect("encode");
            std::fs::write(a.out.join(format!("{}-{frame:03}.png", view.name)), png)
                .expect("write the capture");
        }
    }
}
