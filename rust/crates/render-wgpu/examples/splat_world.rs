//! A splat character in a world (`docs/splat-characters.md`): one figure
//! walking across a lit floor among crates, a second standing beside its
//! path, a half-transparent blended sprite it passes behind, and fog. The
//! GLB and clip packs are read from local paths: character bytes stay out
//! of the repository.
//!
//! ```text
//! cargo run --release -p render-wgpu --example splat_world -- --glb PATH
//!     --walk PATH [--walk-fit JOINT:K,A] --idle PATH [--idle-fit JOINT:K,A]
//!     --out DIR [--size 1920x1080] [--frames 90] [--save 30,60,89]
//!     [--taau SCALE] [--motion-view] [--fresh] [--no-fog]
//!     [--lights] [--lit FLOOR]
//! ```
//!
//! - `--walk`, `--idle`: clip packs bound by joint names; the walker plays
//!   the walk's first clip, the bystander the idle's, each with its fit.
//! - `--frames N`: draws N frames at 60 Hz from Engine time 0, the camera
//!   panning to follow the walker; `--save` names the frames written as
//!   `frame-<index>.png` (the last by default).
//! - `--taau SCALE`: TAAU at render scale SCALE instead of 4x MSAA.
//! - `--motion-view`: the frames show the motion target's view.
//! - `--fresh`: also draw the last frame's moment afresh with 4x MSAA,
//!   `fresh.png`, and report the last frame's difference from it.
//! - `--no-fog`: no fog.
//! - `--lights`: the scene's own lights instead of the default ones: a
//!   sun casting through its cascades, ambient light, a warm point light by
//!   the bystander, and an awning whose shadow the walker crosses.
//! - `--lit FLOOR`: both figures lit by the world's lights, keeping FLOOR
//!   of their baked colour however dark.
//!
//! Writes `readout.json`: the splat readout, every pass's median GPU time
//! over the last 30 frames, the adapter and the `--fresh` difference.

#[path = "../tests/support/mod.rs"]
mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::{OffscreenTarget, OverlayShown, RendererOptions};
use support::*;

const WALKER: u64 = 1;
const BYSTANDER: u64 = 2;
const SPRITE: u64 = 3;
const RATE: f64 = 60.0;
/// Where the walker starts, facing +x.
const START: [f32; 3] = [-2.4, 0.0, 0.0];
/// The bystander, facing the camera, half a metre behind the path.
const BYSTANDER_AT: [f32; 3] = [-0.3, 0.0, -0.5];
/// The sprite, a pane of tinted glass between the camera and the path.
const SPRITE_AT: [f32; 3] = [1.2, 0.95, 0.7];
const SPRITE_SIZE: [f32; 2] = [1.1, 1.9];
const SPRITE_TINT: [f32; 4] = [0.35, 0.75, 1.0, 0.45];
/// The fog, its colour the sky's.
const FOG_COLOR: [f32; 3] = [0.62, 0.68, 0.76];
const FOG_DENSITY: f32 = 0.06;
const CAMERA_HEIGHT: f64 = 1.55;
const CAMERA_BACK: f64 = 5.0;
const CAMERA_PITCH: f64 = -9.0;

struct Arguments {
    glb: std::path::PathBuf,
    walk: std::path::PathBuf,
    walk_fit: Option<(String, f32, f32)>,
    idle: std::path::PathBuf,
    idle_fit: Option<(String, f32, f32)>,
    out: std::path::PathBuf,
    size: (u32, u32),
    frames: u32,
    save: Vec<u32>,
    taau: Option<f32>,
    motion_view: bool,
    fresh: bool,
    fog: bool,
    lights: bool,
    lit: Option<f32>,
}

fn fit(text: &str, flag: &str) -> Result<(String, f32, f32), String> {
    let (joint, numbers) = text
        .rsplit_once(':')
        .ok_or(format!("{flag} is JOINT:K,A"))?;
    let (k, a) = numbers
        .split_once(',')
        .ok_or(format!("{flag} is JOINT:K,A"))?;
    Ok((
        joint.to_owned(),
        k.parse().map_err(|_| format!("{flag} K"))?,
        a.parse().map_err(|_| format!("{flag} A"))?,
    ))
}

fn arguments() -> Result<Arguments, String> {
    let mut args = std::env::args().skip(1);
    let (mut glb, mut walk, mut idle, mut out) = (None, None, None, None);
    let mut arguments = Arguments {
        glb: Default::default(),
        walk: Default::default(),
        walk_fit: None,
        idle: Default::default(),
        idle_fit: None,
        out: Default::default(),
        size: (1920, 1080),
        frames: 90,
        save: Vec::new(),
        taau: None,
        motion_view: false,
        fresh: false,
        fog: true,
        lights: false,
        lit: None,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--glb" => glb = Some(value()?.into()),
            "--walk" => walk = Some(value()?.into()),
            "--idle" => idle = Some(value()?.into()),
            "--walk-fit" => arguments.walk_fit = Some(fit(&value()?, "--walk-fit")?),
            "--idle-fit" => arguments.idle_fit = Some(fit(&value()?, "--idle-fit")?),
            "--out" => out = Some(value()?.into()),
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size is WIDTHxHEIGHT")?;
                arguments.size = (
                    w.parse().map_err(|_| "--size width")?,
                    h.parse().map_err(|_| "--size height")?,
                );
            }
            "--frames" => arguments.frames = value()?.parse().map_err(|_| "--frames")?,
            "--save" => {
                arguments.save = value()?
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .map_err(|_| "--save takes frame indices")?;
            }
            "--taau" => arguments.taau = Some(value()?.parse().map_err(|_| "--taau")?),
            "--motion-view" => arguments.motion_view = true,
            "--fresh" => arguments.fresh = true,
            "--no-fog" => arguments.fog = false,
            "--lights" => arguments.lights = true,
            "--lit" => arguments.lit = Some(value()?.parse().map_err(|_| "--lit")?),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    arguments.glb = glb.ok_or("--glb PATH is required")?;
    arguments.walk = walk.ok_or("--walk PATH is required")?;
    arguments.idle = idle.ok_or("--idle PATH is required")?;
    arguments.out = out.ok_or("--out DIR is required")?;
    if arguments.frames == 0 {
        return Err("--frames is at least 1".to_owned());
    }
    if arguments.save.is_empty() {
        arguments.save.push(arguments.frames - 1);
    }
    Ok(arguments)
}

fn read(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| {
        eprintln!(
            "splat_world: cannot read {}: {error} (character bytes are read from a local path)",
            path.display()
        );
        std::process::exit(2);
    })
}

fn admit(path: &std::path::Path, bytes: &[u8]) -> AnimatedMeshAsset {
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.file_name().map_or("splat.glb".into(), |name| {
            name.to_string_lossy().into_owned()
        })),
        bytes,
        &ImportContext::default(),
    );
    imported
        .assets
        .unwrap_or_else(|| panic!("{} is admitted: {:?}", path.display(), imported.diagnostics))
        .animated_mesh
}

fn hex(hash: &str) -> &str {
    hash.trim_start_matches("sha256:")
}

/// `body` under its own asset id `id`, playing the first clip of the clip
/// pack at `path` with `fit`, looping.
fn with_pack(
    harness: &mut Harness,
    body: &AnimatedMeshAsset,
    id: &str,
    path: &std::path::Path,
    fit: Option<&(String, f32, f32)>,
) -> (AnimatedMeshAsset, AnimatedMeshPlaybackCommand) {
    let bytes = read(path);
    let pack = admit(path, &bytes);
    let hash = pack.content_hash.clone().expect("content hash");
    harness
        .resources
        .insert(&format!("clip-pack-resource/{}", hex(&hash)), bytes);
    let first = pack.clips.first().expect("the pack has a clip");
    eprintln!(
        "{}: clip {:?}, {:?} s",
        path.display(),
        first.id,
        first.duration_seconds
    );
    let clip = first.id.clone();
    let mut asset = body.clone();
    asset.asset = id.to_owned();
    asset.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hex(&hash)),
        runtime_format: pack.runtime_format,
        content_hash: hash.clone(),
        rig: pack.rig.clone().expect("the clip pack names its rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "splat_world".to_owned(),
            source_hash: hash,
            target_hash: body.content_hash.clone().expect("content hash"),
            license: "local".to_owned(),
        },
        binding: AnimationClipPackBinding::JointNames,
        translation_fits: fit
            .iter()
            .map(|(joint, k, a)| AnimationClipTranslationFit {
                clip: clip.clone(),
                joint: joint.clone(),
                scale: [*k; 3],
                offset: [0.0, *a, 0.0],
            })
            .collect(),
    });
    let playback = AnimatedMeshPlaybackCommand::Play {
        clip,
        r#loop: AnimationLoopMode::Repeat,
        speed: 1.0,
        weight: 1.0,
        restart: true,
        fade_seconds: None,
        start_offset_seconds: None,
        start_paused: false,
    };
    (asset, playback)
}

fn figure(
    handle: u64,
    asset: &AnimatedMeshAsset,
    playback: AnimatedMeshPlaybackCommand,
    translation: [f32; 3],
    yaw_degrees: f32,
) -> RenderDiff {
    RenderDiff::CreateAnimatedMeshInstance {
        handle: RenderHandle::new(handle),
        parent: None,
        instance: AnimatedMeshInstanceDescriptor {
            playback_fade: None,
            inspection: AnimatedMeshInspection::default(),
            splat: SplatAppearance::default(),
            asset: asset.asset.clone(),
            transform: transform(translation, yaw_degrees, [1.0; 3]),
            visible: true,
            material_overrides: Vec::new(),
            playback: Some(playback),
            metadata: RenderMetadata {
                source_entity: Some(handle),
                ..RenderMetadata::default()
            },
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
        },
    }
}

/// The sun (casting through its cascades), ambient light, a warm point
/// light by the bystander, and an awning over the path whose shadow the
/// walker crosses between about x = 0.2 and 1.2.
fn lights() -> Vec<RenderDiff> {
    vec![
        RenderDiff::CreateLight {
            handle: RenderHandle::new(40),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.96, 0.88],
                intensity: 3.0,
                enabled: true,
                direction: [0.35, -1.0, -0.45],
                range: None,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(41),
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
            handle: RenderHandle::new(42),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0, 0.55, 0.2],
                intensity: 6.0,
                enabled: true,
                position: [-1.1, 1.4, 0.6],
                range: Some(3.5),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::DefineMaterial {
            material: material("material/awning", [0.45, 0.3, 0.2, 1.0], None),
        },
        static_mesh(
            "mesh/awning",
            box_mesh([-0.4, 2.6, 0.2], [0.6, 2.7, 1.2], |_| 0),
            "material/awning",
        ),
        instance(43, None, "mesh/awning", transform([0.0; 3], 0.0, [1.0; 3])),
    ]
}

/// The floor, crates receding into the fog, the sprite.
fn world(harness: &mut Harness, fog: bool) -> Vec<RenderDiff> {
    let texture = harness
        .resources
        .texture("texture/white", 1, 1, &[255; 4], TextureWrap::Clamp);
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [FOG_COLOR[0], FOG_COLOR[1], FOG_COLOR[2], 1.0],
        },
        RenderDiff::SetFog {
            fog: fog.then_some(FogDescriptor::Exponential {
                color: FOG_COLOR,
                density: FOG_DENSITY,
            }),
        },
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.42, 0.4, 0.36, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/crate", [0.55, 0.38, 0.22, 1.0], None),
        },
        static_mesh(
            "mesh/floor",
            box_mesh([-20.0, -0.2, -40.0], [20.0, 0.0, 12.0], |_| 0),
            "material/floor",
        ),
        static_mesh(
            "mesh/crate",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/crate",
        ),
        instance(10, None, "mesh/floor", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::DefineTexture { texture },
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/white".into(),
                texture: "texture/white".into(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0; 2],
                    uv_max: [1.0; 2],
                    size: None,
                }],
            },
        },
        RenderDiff::CreateSprite {
            handle: RenderHandle::new(SPRITE),
            parent: None,
            sprite: SpriteInstanceDescriptor {
                asset: "sprite/white".to_owned(),
                frame: 0,
                pivot: [0.5, 0.5],
                size: SPRITE_SIZE,
                size_mode: SpriteSizeMode::World,
                billboard: BillboardMode::None,
                tint: SPRITE_TINT,
                render_order: 0,
                depth: SpriteDepthPolicy::Default,
                layer: RenderLayer::Scene,
                viewport_placement: None,
                shading: SpriteShading::Unlit,
                material: SpriteMaterialDescriptor {
                    alpha: SpriteAlphaMode::Blend,
                    ..SpriteMaterialDescriptor::default()
                },
                visible: true,
                transform: transform(SPRITE_AT, 0.0, [1.0; 3]),
                attachment: SpriteAttachment::default(),
                metadata: RenderMetadata::default(),
            },
        },
    ];
    for (index, [x, z]) in [
        [-3.5, -2.0],
        [2.8, -3.0],
        [-1.0, -7.0],
        [3.5, -11.0],
        [-4.0, -16.0],
        [1.0, -24.0],
    ]
    .into_iter()
    .enumerate()
    {
        ops.push(instance(
            20 + index as u64,
            None,
            "mesh/crate",
            transform([x, 0.0, z], index as f32 * 23.0, [1.0; 3]),
        ));
    }
    ops
}

/// The camera following the walker's x at `time`, back from the path.
fn follow(time: f64) -> RendererCompositionCamera {
    let x = f64::from(START[0]) + 1.4 * time;
    camera([x * 0.6, CAMERA_HEIGHT, CAMERA_BACK], 0.0, CAMERA_PITCH)
}

/// `harness`'s world at this moment drawn afresh with 4x MSAA.
fn fresh_msaa(harness: &Harness, view: &RendererCompositionCamera, size: (u32, u32)) -> Vec<u8> {
    let mut renderer = render_wgpu::Renderer::new(&harness.gpu, RendererOptions::default());
    let issues = renderer.apply(&harness.world.snapshot().frame, &harness.resources);
    assert!(issues.is_empty(), "{issues:?}");
    let msaa = RenderFrameDiff {
        publication: None,
        ops: vec![RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor::DEFAULT,
        }],
    };
    assert!(renderer.apply(&msaa, &harness.resources).is_empty());
    let target = OffscreenTarget::new(&harness.gpu, size.0, size.1, 4);
    renderer.render_offscreen(view, &target);
    target.read_rgba(&harness.gpu)
}

fn main() {
    let arguments = arguments().unwrap_or_else(|error| {
        eprintln!("splat_world: {error}");
        std::process::exit(2);
    });
    std::fs::create_dir_all(&arguments.out).expect("create the output directory");
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: !arguments.lights,
        shadows: arguments.lights,
        ..RendererOptions::default()
    });
    let bytes = read(&arguments.glb);
    let body = admit(&arguments.glb, &bytes);
    let hash = body.content_hash.clone().expect("content hash");
    harness
        .resources
        .insert(&format!("animated-mesh-resource/{}", hex(&hash)), bytes);
    let (walker, walking) = with_pack(
        &mut harness,
        &body,
        "animated-mesh/splat-world-walker",
        &arguments.walk,
        arguments.walk_fit.as_ref(),
    );
    let (bystander, idling) = with_pack(
        &mut harness,
        &body,
        "animated-mesh/splat-world-bystander",
        &arguments.idle,
        arguments.idle_fit.as_ref(),
    );
    let mut ops = world(&mut harness, arguments.fog);
    ops.extend([
        RenderDiff::DefineAnimatedMesh {
            asset: walker.clone(),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: bystander.clone(),
        },
        // The figures face +z: a yaw of 90° turns the walker toward +x.
        figure(WALKER, &walker, walking, START, 90.0),
        figure(BYSTANDER, &bystander, idling, BYSTANDER_AT, 20.0),
    ]);
    if arguments.lights {
        ops.extend(lights());
    }
    if let Some(floor) = arguments.lit {
        for handle in [WALKER, BYSTANDER] {
            ops.push(RenderDiff::SetSplatAppearance {
                handle: RenderHandle::new(handle),
                appearance: SplatAppearance {
                    lighting: SplatLighting::Lit,
                    light_floor: floor,
                    ..SplatAppearance::default()
                },
            });
        }
    }
    let settings = RendererSettingsDescriptor {
        shadows: arguments.lights,
        ..match arguments.taau {
            Some(render_scale) => RendererSettingsDescriptor {
                antialiasing: Antialiasing::Temporal,
                render_scale,
                ..RendererSettingsDescriptor::DEFAULT
            },
            None => RendererSettingsDescriptor::DEFAULT,
        }
    };
    ops.push(RenderDiff::SetRendererSettings { settings });
    harness.apply(ops);
    if arguments.motion_view {
        harness.renderer.set_overlay_shown(OverlayShown {
            motion: true,
            ..OverlayShown::default()
        });
    }
    let (width, height) = arguments.size;
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, harness.renderer.samples());

    let mut last = (follow(0.0), Vec::new());
    for frame in 0..arguments.frames {
        let time = f64::from(frame) / RATE;
        harness.at(time);
        let view = follow(time);
        let (_, rgba) = harness.render(&view);
        if arguments.save.contains(&frame) {
            let png = render_wgpu::encode_png(width, height, &rgba).expect("encode the frame");
            std::fs::write(arguments.out.join(format!("frame-{frame:03}.png")), png)
                .expect("write the frame");
        }
        last = (view, rgba);
    }
    let readout = harness.renderer.gpu_readout();
    let difference = arguments.fresh.then(|| {
        let fresh = fresh_msaa(&harness, &last.0, arguments.size);
        let png = render_wgpu::encode_png(width, height, &fresh).expect("encode the frame");
        std::fs::write(arguments.out.join("fresh.png"), png).expect("write the frame");
        let mean = last
            .1
            .iter()
            .zip(&fresh)
            .map(|(a, b)| f64::from(a.abs_diff(*b)))
            .sum::<f64>()
            / fresh.len() as f64;
        let off = last
            .1
            .as_chunks::<4>()
            .0
            .iter()
            .zip(fresh.as_chunks::<4>().0.iter())
            .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 32))
            .count();
        format!("{{\"mean\": {mean:.4}, \"pixels_over_32\": {off}}}")
    });
    let passes: Vec<String> = readout
        .passes
        .iter()
        .filter(|timing| timing.timed_frames > 0)
        .map(|timing| {
            format!(
                "\"{}\": {{\"frames\": {}, \"median_ms\": {:.4}}}",
                timing.pass, timing.timed_frames, timing.median_gpu_ms
            )
        })
        .collect();
    let report = format!(
        "{{\"runs\": {}, \"instances\": {}, \"gaussians\": {}, \"refused\": {:?}, \"passes\": {{{}}}, \"fresh\": {}, \"adapter\": {:?}}}\n",
        readout.splats.runs,
        readout.splats.instances,
        readout.splats.gaussians,
        readout.splats.refused,
        passes.join(", "),
        difference.unwrap_or_else(|| "null".to_owned()),
        harness.gpu.adapter_summary(),
    );
    std::fs::write(arguments.out.join("readout.json"), &report).expect("write the readout");
    print!("{report}");
}
