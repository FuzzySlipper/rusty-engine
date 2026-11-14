//! Time a crowd of walking splat characters (`docs/splat-characters.md`)
//! as asset-pipeline's web crowd page (`tests/splats/crowd.js`) frames one:
//! N instances of one GLB on a grid 1.4 m apart, `ceil(sqrt(1.5 N))`
//! across, each playing the clip from its own phase (7 s apart, wrapped at
//! 60) and walking in place, seen from in front of the group at a pitch of
//! 13° with a 42° vertical field of view, far enough back to fit the rows.
//! A floor stands under them. The GLB and clip pack are read from local
//! paths: character bytes stay out of the repository.
//!
//! ```text
//! cargo run --release -p render-wgpu --example splat_crowd -- --glb PATH
//!     --clip-pack PATH --clip ID --fit JOINT:K,A --out DIR
//!     [--counts 1,8,16,32] [--size 1920x1080] [--warm 2] [--frames 240]
//! ```
//!
//! - `--fit JOINT:K,A`: the clip's translation of `JOINT` becomes
//!   (0, A + K·y, 0), so the figure walks in place as the web page's do.
//! - `--warm SECONDS`: frames drawn untimed at 60 Hz first, so the GPU
//!   reaches its clocks.
//! - `--frames N`: frames timed at 60 Hz.
//!
//! Writes `crowd.json`: per count, each timed pass's median GPU time (over
//! the last 120 frames; `world` spans the splats' passes, which run inside
//! it); the timed frames' throughput (wall
//! time per frame with CPU and GPU overlapping, the readback after the last
//! one included) and its frames a second; the median and 95th percentile
//! of a frame's CPU time (poses and recording); the splat readout and the
//! adapter. Writes `crowd-<N>.png` of each count's
//! last frame.

#[path = "../tests/support/mod.rs"]
mod support;

use std::time::Instant;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{OffscreenTarget, RendererOptions};
use support::*;

/// The web page's spacing between figures, metres.
const SPACING: f64 = 1.4;
/// Seconds between neighbouring figures' clip phases, wrapped at
/// `PHASE_WRAP` (the web page's `i * 7 % 60`).
const PHASE_STEP: f64 = 7.0;
const PHASE_WRAP: f64 = 60.0;
const FRAME_RATE: f64 = 60.0;
const FOV_Y_DEGREES: f64 = 42.0;
const PITCH_DEGREES: f64 = 13.0;
/// The web page's look-at height and nearest distance, metres.
const TARGET_HEIGHT: f64 = 0.95;
const NEAREST: f64 = 3.3;

struct Arguments {
    glb: std::path::PathBuf,
    clip_pack: std::path::PathBuf,
    clip: String,
    fit: (String, f32, f32),
    out: std::path::PathBuf,
    counts: Vec<usize>,
    size: (u32, u32),
    warm: f64,
    frames: usize,
}

fn arguments() -> Result<Arguments, String> {
    let (mut glb, mut clip_pack, mut clip, mut fit, mut out) = (None, None, None, None, None);
    let mut counts = vec![1, 8, 16, 32];
    let (mut size, mut warm, mut frames) = ((1920, 1080), 2.0, 240);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--glb" => glb = Some(value()?.into()),
            "--clip-pack" => clip_pack = Some(value()?.into()),
            "--clip" => clip = Some(value()?),
            "--fit" => {
                let text = value()?;
                let (joint, numbers) = text.rsplit_once(':').ok_or("--fit is JOINT:K,A")?;
                let (k, a) = numbers.split_once(',').ok_or("--fit is JOINT:K,A")?;
                fit = Some((
                    joint.to_owned(),
                    k.parse().map_err(|_| "--fit K")?,
                    a.parse().map_err(|_| "--fit A")?,
                ));
            }
            "--out" => out = Some(value()?.into()),
            "--counts" => {
                counts = value()?
                    .split(',')
                    .map(|count| count.trim().parse::<usize>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| "--counts takes whole numbers")?;
            }
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size is WIDTHxHEIGHT")?;
                size = (
                    w.parse().map_err(|_| "--size width")?,
                    h.parse().map_err(|_| "--size height")?,
                );
            }
            "--warm" => warm = value()?.parse().map_err(|_| "--warm")?,
            "--frames" => frames = value()?.parse().map_err(|_| "--frames")?,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Arguments {
        glb: glb.ok_or("--glb PATH is required")?,
        clip_pack: clip_pack.ok_or("--clip-pack PATH is required")?,
        clip: clip.ok_or("--clip ID is required")?,
        fit: fit.ok_or("--fit JOINT:K,A is required")?,
        out: out.ok_or("--out DIR is required")?,
        counts,
        size,
        warm,
        frames,
    })
}

fn read(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| {
        eprintln!(
            "splat_crowd: cannot read {}: {error} (character files are local, outside the repository)",
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
                "splat_crowd: {} is not admitted: {:?}",
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

fn percentile(sorted: &[f64], share: f64) -> f64 {
    sorted[((sorted.len() as f64 * share) as usize).min(sorted.len() - 1)]
}

/// The figure asset with the clip pack bound by joint names and fitted to
/// walk in place, and the pack's bytes.
fn figure(arguments: &Arguments) -> (AnimatedMeshAsset, Vec<u8>, String, Vec<u8>) {
    let bytes = read(&arguments.glb);
    let mut asset = admit(&arguments.glb, &bytes);
    let hash = asset.content_hash.clone().expect("content hash");
    let pack_bytes = read(&arguments.clip_pack);
    let pack = admit(&arguments.clip_pack, &pack_bytes);
    let pack_hash = pack.content_hash.clone().expect("content hash");
    let (joint, k, a) = arguments.fit.clone();
    asset.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hex(&pack_hash)),
        runtime_format: pack.runtime_format,
        content_hash: pack_hash.clone(),
        rig: pack.rig.clone().expect("the clip pack names its rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "splat_crowd".to_owned(),
            source_hash: pack_hash.clone(),
            target_hash: hash.clone(),
            license: "local".to_owned(),
        },
        binding: AnimationClipPackBinding::JointNames,
        translation_fits: vec![AnimationClipTranslationFit {
            clip: arguments.clip.clone(),
            joint,
            scale: [0.0, k, 0.0],
            offset: [0.0, a, 0.0],
        }],
    });
    (asset, bytes, pack_hash, pack_bytes)
}

fn main() {
    let arguments = arguments().unwrap_or_else(|error| {
        eprintln!("splat_crowd: {error}");
        std::process::exit(2);
    });
    std::fs::create_dir_all(&arguments.out).expect("create the output directory");
    let (asset, bytes, pack_hash, pack_bytes) = figure(&arguments);
    let (width, height) = arguments.size;
    let mut rows = Vec::new();
    for &count in &arguments.counts {
        let mut harness = Harness::new(RendererOptions::default());
        harness.target = OffscreenTarget::new(&harness.gpu, width, height, 1);
        harness.resources.insert(
            &format!(
                "animated-mesh-resource/{}",
                hex(asset.content_hash.as_deref().expect("content hash"))
            ),
            bytes.clone(),
        );
        harness.resources.insert(
            &format!("clip-pack-resource/{}", hex(&pack_hash)),
            pack_bytes.clone(),
        );
        let columns = (count as f64 * 1.5).sqrt().ceil() as usize;
        let rows_deep = count.div_ceil(columns);
        let mut ops = vec![
            RenderDiff::DefineMaterial {
                material: material("material/floor", [0.32, 0.33, 0.35, 1.0], None),
            },
            static_mesh(
                "mesh/floor",
                box_mesh([-30.0, -0.2, -40.0], [30.0, 0.0, 10.0], |_| 0),
                "material/floor",
            ),
            instance(1, None, "mesh/floor", transform([0.0; 3], 0.0, [1.0; 3])),
            RenderDiff::DefineAnimatedMesh {
                asset: asset.clone(),
            },
        ];
        for index in 0..count {
            let (row, column) = (index / columns, index % columns);
            let x = (column as f64 - (columns - 1) as f64 / 2.0) * SPACING;
            let z = -(row as f64) * SPACING;
            let phase = (index as f64 * PHASE_STEP) % PHASE_WRAP;
            ops.push(RenderDiff::CreateAnimatedMeshInstance {
                handle: RenderHandle::new(100 + index as u64),
                parent: None,
                instance: AnimatedMeshInstanceDescriptor {
                    playback_fade: None,
                    inspection: AnimatedMeshInspection::default(),
                    splat: SplatAppearance::default(),
                    asset: asset.asset.clone(),
                    transform: Transform {
                        translation: [x as f32, 0.0, z as f32],
                        ..Transform::IDENTITY
                    },
                    visible: true,
                    material_overrides: Vec::new(),
                    playback: Some(AnimatedMeshPlaybackCommand::Play {
                        clip: arguments.clip.clone(),
                        r#loop: AnimationLoopMode::Repeat,
                        speed: 1.0,
                        weight: 1.0,
                        restart: true,
                        fade_seconds: None,
                        start_offset_seconds: Some(phase),
                        start_paused: false,
                    }),
                    metadata: RenderMetadata {
                        source_entity: Some(100 + index as u64),
                        ..RenderMetadata::default()
                    },
                    layer: RenderLayer::Scene,
                    shadow_casting: Default::default(),
                },
            });
        }
        harness.apply(ops);
        // In front of the group (the figures face +z), far enough back to
        // fit its columns across and its rows behind.
        let aspect = f64::from(width) / f64::from(height);
        let distance = NEAREST.max(
            columns as f64 * SPACING / (2.0 * (FOV_Y_DEGREES / 2.0).to_radians().tan() * aspect)
                + rows_deep as f64 * SPACING / 2.0,
        );
        let target = [
            0.0,
            TARGET_HEIGHT,
            -((rows_deep - 1) as f64) * SPACING / 2.0,
        ];
        let pitch = PITCH_DEGREES.to_radians();
        let mut view = camera(
            [
                target[0],
                target[1] + distance * pitch.sin(),
                target[2] + distance * pitch.cos(),
            ],
            0.0,
            -PITCH_DEGREES,
        );
        view.projection = render_host_contracts::RendererCameraProjection::Perspective {
            fov_y_degrees: FOV_Y_DEGREES,
            near: 0.05,
            far: 200.0,
        };
        let warm = (arguments.warm * FRAME_RATE).round() as usize;
        let mut frame = 0usize;
        let mut cpu = Vec::with_capacity(arguments.frames);
        for _ in 0..warm {
            frame += 1;
            harness.at(frame as f64 / FRAME_RATE);
            harness.renderer.render_offscreen(&view, &harness.target);
        }
        // Wait for the warm-up's frames before timing.
        let _ = harness.target.read_rgba(&harness.gpu);
        let timed_from = Instant::now();
        for _ in 0..arguments.frames {
            let started = Instant::now();
            frame += 1;
            harness.at(frame as f64 / FRAME_RATE);
            harness.renderer.render_offscreen(&view, &harness.target);
            cpu.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        // The last frame's readback waits for the GPU: the loop's wall time
        // is the frames' throughput, CPU and GPU work overlapping as they do
        // in a running game.
        let rgba = harness.target.read_rgba(&harness.gpu);
        let throughput = timed_from.elapsed().as_secs_f64() * 1000.0 / arguments.frames as f64;
        let png = render_wgpu::encode_png(width, height, &rgba).expect("encode the capture");
        std::fs::write(arguments.out.join(format!("crowd-{count}.png")), png)
            .expect("write the capture");
        let readout = harness.renderer.gpu_readout();
        cpu.sort_by(f64::total_cmp);
        let timed: Vec<(String, f64)> = readout
            .passes
            .iter()
            .filter(|timing| timing.timed_frames > 0)
            .map(|timing| (timing.pass.to_string(), timing.median_gpu_ms))
            .collect();
        let passes: Vec<String> = timed
            .iter()
            .map(|(pass, ms)| format!("\"{pass}\": {ms:.4}"))
            .collect();
        let row = format!(
            "{{\"count\": {count}, \"gaussians\": {}, \"runs\": {}, \"size\": [{width}, {height}], \"passes\": {{{}}}, \"frame_ms\": {throughput:.4}, \"fps\": {:.1}, \"cpu_ms_p50\": {:.4}, \"cpu_ms_p95\": {:.4}, \"frames\": {}, \"distance\": {distance:.3}}}",
            readout.splats.gaussians,
            readout.splats.runs,
            passes.join(", "),
            1000.0 / throughput,
            percentile(&cpu, 0.5),
            percentile(&cpu, 0.95),
            cpu.len(),
        );
        println!("{row}");
        rows.push(row);
    }
    let gpu = Harness::new(RendererOptions::default())
        .gpu
        .adapter_summary();
    std::fs::write(
        arguments.out.join("crowd.json"),
        format!(
            "{{\"adapter\": {:?}, \"rows\": [\n{}\n]}}\n",
            format!("{gpu:?}"),
            rows.join(",\n")
        ),
    )
    .expect("write the readout");
}
