//! Draws a scene snapshot (`engine.renderer.snapshot`) on a fresh renderer,
//! without the product: the same rebaseline and capture the runtime's own
//! renderer performs, on a headless device.
//!
//! ```text
//! rusty-scene-render <snapshot> <out.png> [--width W] [--height H] [--frames N]
//!                    [--walk M] [--turn D] [--ambient-occlusion off|compute|raster]
//! ```
//!
//! `--frames N` then draws N more frames into one target with readback and
//! reports their mean and median cost. With `--walk` or `--turn` the camera
//! moves M metres forward and turns D degrees right before each of those
//! frames, from the snapshot's first camera, for the cost of a moving view
//! (culling, shadow cascades). `--ambient-occlusion` overrides the
//! snapshot's path (at its strength, or 1 when it was off), so the two paths
//! compare on one scene. Select the adapter as for any wgpu program (for
//! example `WGPU_BACKEND=vulkan`).

use std::{path::PathBuf, time::Instant};

use csharp_product_runtime::scene_snapshot::{SceneSnapshot, SceneSnapshotChange};
use render_host_contracts::RendererCameraPose;
use render_presentation::PresentationWorld;
use render_wgpu::{
    encode_png, AmbientOcclusion, AmbientOcclusionPath, Gpu, OffscreenTarget, RendererOptions,
    SceneDriver,
};
use serde_json::json;

const USAGE: &str = "usage: rusty-scene-render <snapshot> <out.png> [--width W] [--height H] \
     [--frames N] [--walk M] [--turn D] [--ambient-occlusion off|compute|raster]";

fn main() {
    if let Err(error) = run() {
        eprintln!("rusty-scene-render: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let mut positional = Vec::new();
    let (mut width, mut height, mut frames) = (1280_u32, 720_u32, 0_u32);
    let (mut walk, mut turn) = (0.0_f64, 0.0_f64);
    let mut ambient_occlusion: Option<AmbientOcclusionPath> = None;
    while let Some(argument) = arguments.next() {
        let mut number = |name: &str| -> Result<f64, String> {
            arguments
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| format!("{name} needs a number\n{USAGE}"))
        };
        match argument.as_str() {
            "--width" => width = number("--width")? as u32,
            "--height" => height = number("--height")? as u32,
            "--frames" => frames = number("--frames")? as u32,
            "--walk" => walk = number("--walk")?,
            "--turn" => turn = number("--turn")?,
            "--ambient-occlusion" => {
                ambient_occlusion = Some(match arguments.next().as_deref() {
                    Some("off") => AmbientOcclusionPath::Off,
                    Some("compute") => AmbientOcclusionPath::Compute,
                    Some("raster") => AmbientOcclusionPath::Raster,
                    _ => {
                        return Err(format!(
                            "--ambient-occlusion needs off, compute or raster\n{USAGE}"
                        ))
                    }
                });
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ if argument.starts_with("--") => {
                return Err(format!("unknown flag {argument}\n{USAGE}"))
            }
            _ => positional.push(PathBuf::from(argument)),
        }
    }
    let [snapshot_path, out] = positional.as_slice() else {
        return Err(USAGE.to_owned());
    };

    let opened = Instant::now();
    let snapshot = SceneSnapshot::open(snapshot_path)?;
    let open_ms = ms(opened);
    // Attached presentation (billboards, audio emitters) follows entities
    // through the retained world, as the Engine's positions do live.
    let mut world = PresentationWorld::default();
    for change in &snapshot.changes {
        if let SceneSnapshotChange::Frame(frame) = change {
            world
                .apply(frame.clone())
                .map_err(|error| format!("world frame: {error:?}"))?;
        }
    }

    let gpu = Gpu::headless().map_err(|error| format!("{error:?}"))?;
    let adapter = gpu.adapter_summary();
    let mut options: RendererOptions = snapshot.metadata.options.into();
    if let Some(path) = ambient_occlusion {
        let strength = match options.ambient_occlusion.path {
            AmbientOcclusionPath::Off => 1.0,
            _ => options.ambient_occlusion.strength,
        };
        options.ambient_occlusion = AmbientOcclusion { path, strength };
    }
    let driver = SceneDriver::new(gpu, options);
    let applied = Instant::now();
    driver.rebaseline(
        snapshot.scene_changes(),
        &snapshot,
        &|entity| world.entity_world_position(entity),
        snapshot.metadata.state.into(),
    );
    let apply_ms = ms(applied);
    // The first frame on a new target also compiles its pipelines.
    let captured = Instant::now();
    let capture = driver.capture(Some((width, height)));
    let capture_ms = ms(captured);
    std::fs::write(out, encode_png(width, height, &capture.rgba)?)
        .map_err(|error| format!("{}: {error}", out.display()))?;

    let mut timing = serde_json::Value::Null;
    if frames > 0 {
        let start = snapshot
            .changes
            .iter()
            .rev()
            .find_map(|change| match change {
                SceneSnapshotChange::ViewComposition(composition) => {
                    composition.cameras.first().map(|camera| camera.pose)
                }
                _ => None,
            });
        let moving = walk != 0.0 || turn != 0.0;
        if moving && start.is_none() {
            return Err("--walk and --turn need a camera in the snapshot".to_owned());
        }
        let target = OffscreenTarget::new(driver.gpu(), width, height);
        let mut pixels = Vec::new();
        let mut costs = Vec::with_capacity(frames as usize);
        for frame in 1..=frames {
            if let (true, Some(start)) = (moving, start) {
                // Engine yaw zero faces -Z; positive yaw turns toward +X.
                let yaw = start.yaw_degrees + turn * f64::from(frame);
                let distance = walk * f64::from(frame);
                let [x, y, z] = start.position;
                let radians = start.yaw_degrees.to_radians();
                driver.set_observer(Some(RendererCameraPose {
                    position: [
                        x + radians.sin() * distance,
                        y,
                        z - radians.cos() * distance,
                    ],
                    pitch_degrees: start.pitch_degrees,
                    yaw_degrees: yaw,
                }));
            }
            let started = Instant::now();
            driver.draw(|renderer, now| renderer.render_view_composition(&target, now));
            target.read_rgba_into(driver.gpu(), &mut pixels);
            costs.push(ms(started));
        }
        costs.sort_by(f64::total_cmp);
        timing = json!({
            "frames": frames,
            "meanMs": costs.iter().sum::<f64>() / costs.len() as f64,
            "medianMs": costs[costs.len() / 2],
            "withReadback": true,
            "walkMetres": walk,
            "turnDegrees": turn,
        });
    }
    let (tables, memory) = driver
        .draw(|renderer, _| (renderer.table_counts(), renderer.mesh_memory()))
        .0;
    let (skipped, last_skip) = driver.skipped_ops();
    let gpu_readout = driver.gpu_readout();
    let report = json!({
        "snapshot": snapshot_path.display().to_string(),
        "product": snapshot.metadata.product,
        "writtenOnAdapter": snapshot.metadata.adapter,
        "adapter": format!("{} ({})", adapter.name, adapter.backend),
        "state": snapshot.metadata.state,
        "resources": snapshot.resource_count(),
        "openMs": open_ms,
        "applyMs": apply_ms,
        "captureMs": capture_ms,
        "tables": format!("{tables:?}"),
        "meshMemory": format!("{memory:?}"),
        "skippedOps": skipped,
        "lastSkip": last_skip,
        "image": { "path": out.display().to_string(), "width": width, "height": height },
        "timing": timing,
        // The GPU passes over the frames drawn: each timed pass's median
        // cost and the ambient occlusion path, as `engine.renderer` reports
        // them.
        "gpu": {
            "timestamps": gpu_readout.timestamps,
            "limits": format!("{:?}", gpu_readout.limits),
            "passes": gpu_readout.passes.iter().map(|pass| json!({
                "pass": pass.pass,
                "timedFrames": pass.timed_frames,
                "medianGpuMs": pass.median_gpu_ms,
            })).collect::<Vec<_>>(),
            "ambientOcclusion": {
                "path": format!("{:?}", gpu_readout.ambient_occlusion.path),
                "computeRefused": gpu_readout.ambient_occlusion.compute_refused,
                "workgroups": gpu_readout.ambient_occlusion.workgroups,
                "texture": gpu_readout.ambient_occlusion.texture,
            },
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report encodes")
    );
    Ok(())
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}
