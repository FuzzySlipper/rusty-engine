//! Draws a scene snapshot (`engine.renderer.snapshot`) on a fresh renderer,
//! without the product: the same rebaseline and capture the runtime's own
//! renderer performs, on a headless device.
//!
//! ```text
//! rusty-scene-render <snapshot> <out.png> [--width W] [--height H] [--frames N]
//!                    [--walk M] [--turn D] [--ambient-occlusion off|compute|raster|field]
//!                    [--clustered-lighting on|off] [--gpu-culling on|off] [--render-scale S]
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
use render_model::{IndirectLightDescriptor, RendererSettingsDescriptor};
use render_presentation::PresentationWorld;
use render_wgpu::{
    encode_png, AmbientOcclusion, AmbientOcclusionPath, Gpu, OffscreenTarget, RendererOptions,
    SceneDriver,
};
use serde_json::json;

const USAGE: &str = "usage: rusty-scene-render <snapshot> <out.png> [--width W] [--height H] \
     [--frames N] [--walk M] [--turn D] [--ambient-occlusion off|compute|raster|field] \
     [--clustered-lighting on|off] [--gpu-culling on|off] [--render-scale S] \
     [--indirect-light cx,cy,cz,ex,ey,ez,spacing,bounces[,floor]]";

fn main() {
    if let Err(error) = run() {
        eprintln!("rusty-scene-render: {error}");
        std::process::exit(1);
    }
}

/// The command line, parsed and checked.
#[derive(Debug, PartialEq)]
struct Arguments {
    snapshot: PathBuf,
    out: PathBuf,
    width: u32,
    height: u32,
    frames: u32,
    walk: f64,
    turn: f64,
    ambient_occlusion: Option<AmbientOcclusionPath>,
    clustered_lighting: Option<bool>,
    gpu_culling: Option<bool>,
    render_scale: Option<f32>,
    indirect_light: Option<IndirectLightDescriptor>,
}

/// Parses the command line; `None` when it asked for help. A render scale is
/// held to the setting's range, as the manifest and `RendererSettings.Set`
/// hold it.
fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Option<Arguments>, String> {
    let mut arguments = arguments.into_iter();
    let mut positional = Vec::new();
    let (mut width, mut height, mut frames) = (1280_u32, 720_u32, 0_u32);
    let (mut walk, mut turn) = (0.0_f64, 0.0_f64);
    let mut ambient_occlusion: Option<AmbientOcclusionPath> = None;
    let mut clustered_lighting: Option<bool> = None;
    let mut gpu_culling: Option<bool> = None;
    let mut render_scale: Option<f32> = None;
    let mut indirect_light: Option<IndirectLightDescriptor> = None;
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
            "--render-scale" => {
                let scale = number("--render-scale")? as f32;
                let min = RendererSettingsDescriptor::MIN_RENDER_SCALE;
                if !(scale.is_finite() && (min..=1.0).contains(&scale)) {
                    return Err(format!(
                        "--render-scale needs a value from {min} to 1\n{USAGE}"
                    ));
                }
                render_scale = Some(scale);
            }
            "--indirect-light" => {
                // cx,cy,cz,ex,ey,ez,spacing,bounces[,floor]
                let value = arguments.next().unwrap_or_default();
                let fields: Vec<&str> = value.split(',').collect();
                let numbers: Option<Vec<f32>> = fields
                    .iter()
                    .take(8)
                    .map(|field| field.trim().parse::<f32>().ok())
                    .collect();
                let descriptor = match numbers.as_deref() {
                    Some([cx, cy, cz, ex, ey, ez, spacing, bounces]) => IndirectLightDescriptor {
                        center: [*cx, *cy, *cz],
                        extent: [*ex, *ey, *ez],
                        spacing: *spacing,
                        bounces: *bounces as u32,
                        ambient: if fields.get(8) == Some(&"floor") {
                            render_model::IndirectAmbient::Floor
                        } else {
                            render_model::IndirectAmbient::Sky
                        },
                    },
                    _ => {
                        return Err(format!(
                        "--indirect-light needs cx,cy,cz,ex,ey,ez,spacing,bounces[,floor]\n{USAGE}"
                    ))
                    }
                };
                if !descriptor.valid() {
                    return Err(format!(
                        "--indirect-light: the box, spacing (0.5 to 8) or bounces (1 to 4) are out of range, or more than 262,144 probes\n{USAGE}"
                    ));
                }
                indirect_light = Some(descriptor);
            }
            "--gpu-culling" => {
                gpu_culling = Some(match arguments.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return Err(format!("--gpu-culling needs on or off\n{USAGE}")),
                });
            }
            "--clustered-lighting" => {
                clustered_lighting = Some(match arguments.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return Err(format!("--clustered-lighting needs on or off\n{USAGE}")),
                });
            }
            "--ambient-occlusion" => {
                ambient_occlusion = Some(match arguments.next().as_deref() {
                    Some("off") => AmbientOcclusionPath::Off,
                    Some("compute") => AmbientOcclusionPath::Compute,
                    Some("raster") => AmbientOcclusionPath::Raster,
                    Some("field") => AmbientOcclusionPath::DistanceField,
                    _ => {
                        return Err(format!(
                            "--ambient-occlusion needs off, compute, raster or field\n{USAGE}"
                        ))
                    }
                });
            }
            "-h" | "--help" => return Ok(None),
            _ if argument.starts_with("--") => {
                return Err(format!("unknown flag {argument}\n{USAGE}"))
            }
            _ => positional.push(PathBuf::from(argument)),
        }
    }
    let [snapshot, out] = positional.as_slice() else {
        return Err(USAGE.to_owned());
    };
    Ok(Some(Arguments {
        snapshot: snapshot.clone(),
        out: out.clone(),
        width,
        height,
        frames,
        walk,
        turn,
        ambient_occlusion,
        clustered_lighting,
        gpu_culling,
        render_scale,
        indirect_light,
    }))
}

fn run() -> Result<(), String> {
    let Some(Arguments {
        snapshot: snapshot_path,
        out,
        width,
        height,
        frames,
        walk,
        turn,
        ambient_occlusion,
        clustered_lighting,
        gpu_culling,
        render_scale,
        indirect_light,
    }) = parse(std::env::args().skip(1))?
    else {
        println!("{USAGE}");
        return Ok(());
    };

    let opened = Instant::now();
    let snapshot = SceneSnapshot::open(&snapshot_path)?;
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
        options.ambient_occlusion = AmbientOcclusion {
            path,
            strength,
            radius: options.ambient_occlusion.radius,
        };
    }
    if let Some(clustered) = clustered_lighting {
        options.clustered_lighting = clustered;
    }
    if let Some(culling) = gpu_culling {
        options.gpu_culling = culling;
    }
    if let Some(scale) = render_scale {
        options.render_scale = scale;
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
    // The indirect light volume, the snapshot's or the flag's, bakes before
    // the frames so they draw with it.
    let baked = Instant::now();
    let indirect = driver
        .draw(|renderer, _| {
            if indirect_light.is_some() {
                renderer.set_indirect_light(indirect_light);
            }
            renderer.bake_indirect_light_now()
        })
        .0;
    let bake_ms = indirect.map(|_| ms(baked));
    // The first frame on a new target also compiles its pipelines.
    let captured = Instant::now();
    let capture = driver.capture(Some((width, height)));
    let capture_ms = ms(captured);
    std::fs::write(&out, encode_png(width, height, &capture.rgba)?)
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
        let target = OffscreenTarget::new(driver.gpu(), width, height, driver.primary_samples());
        let mut pixels = Vec::new();
        let mut costs = Vec::with_capacity(frames as usize);
        let (mut draws, mut instances, mut batch_us) = (Vec::new(), Vec::new(), Vec::new());
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
            let (stats, _) =
                driver.draw(|renderer, now| renderer.render_view_composition(&target, now));
            target.read_rgba_into(driver.gpu(), &mut pixels);
            costs.push(ms(started));
            draws.push(stats.draws);
            instances.push(stats.instances);
            batch_us.push(stats.cpu_batch_us);
        }
        costs.sort_by(f64::total_cmp);
        timing = json!({
            "frames": frames,
            "meanMs": costs.iter().sum::<f64>() / costs.len() as f64,
            "medianMs": costs[costs.len() / 2],
            "withReadback": true,
            // Per frame, the median: draw calls, the parts they covered, and
            // the CPU microseconds building draw lists (`FrameStats`).
            "draws": median(&mut draws),
            "instances": median(&mut instances),
            "cpuBatchUs": median(&mut batch_us),
            "walkMetres": walk,
            "turnDegrees": turn,
        });
    }
    let (tables, memory) = driver
        .draw(|renderer, _| (renderer.table_counts(), renderer.mesh_memory()))
        .0;
    let (skipped, last_skip) = driver.skipped_ops();
    let gpu_readout = driver.gpu_readout();
    let settings = driver.settings_readout();
    let refused = [
        ("ambientOcclusion", settings.ambient_occlusion),
        ("antialiasing", settings.antialiasing),
        ("clusteredLighting", settings.clustered_lighting),
        ("gpuCulling", settings.gpu_culling),
    ];
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
        // The renderer settings drawn: the snapshot's, changed by the flags
        // above, and what the adapter refused.
        "settings": {
            "requested": settings.requested,
            "effective": settings.effective,
            "refused": refused.iter().filter_map(|(name, refusal)| {
                refusal.map(|refusal| json!({ "setting": name, "refusal": format!("{refusal:?}") }))
            }).collect::<Vec<_>>(),
        },
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
            "distanceFields": {
                "refused": gpu_readout.distance_fields.refused,
                "residentFields": gpu_readout.distance_fields.resident_fields,
                "atlasBricks": gpu_readout.distance_fields.atlas_bricks,
                "atlasBytes": gpu_readout.distance_fields.atlas_bytes,
                "lookupEntries": gpu_readout.distance_fields.lookup_entries,
            },
            "lightClusters": {
                "enabled": gpu_readout.light_clusters.enabled,
                "refused": gpu_readout.light_clusters.refused,
                "grid": gpu_readout.light_clusters.grid,
                "binnedLights": gpu_readout.light_clusters.binned_lights,
                "globalLights": gpu_readout.light_clusters.global_lights,
                "overflowedClusters": gpu_readout.light_clusters.overflowed_clusters,
            },
            "indirectLight": indirect.map(|readout| json!({
                "dims": readout.dims,
                "probes": readout.probes,
                "invalid": readout.invalid,
                "triangles": readout.triangles,
                "bakeMs": readout.bake_ms,
                "wallMs": bake_ms,
                "bytes": readout.bytes,
                "bricks": readout.bricks,
                "brickMs": readout.brick_ms,
                "brickMsMax": readout.brick_ms_max,
                "bakedBricks": readout.last_batch_bricks,
            })),
            "gpuCulling": {
                "enabled": gpu_readout.gpu_culling.enabled,
                "refused": gpu_readout.gpu_culling.refused,
                "candidates": gpu_readout.gpu_culling.candidates,
                "batches": gpu_readout.gpu_culling.batches,
                "visible": gpu_readout.gpu_culling.visible,
                "multiDraws": gpu_readout.gpu_culling.multi_draws,
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

fn median(values: &mut [u32]) -> u32 {
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(flags: &[&str]) -> Result<Option<Arguments>, String> {
        parse(
            ["scene.rscene", "out.png"]
                .iter()
                .chain(flags)
                .map(|argument| (*argument).to_owned()),
        )
    }

    #[test]
    fn a_render_scale_is_held_to_the_setting_range() {
        for accepted in ["0.5", "0.75", "1"] {
            let arguments = parsed(&["--render-scale", accepted]).unwrap().unwrap();
            assert_eq!(arguments.render_scale, Some(accepted.parse().unwrap()));
        }
        for refused in ["0.25", "1.5", "0", "-0.5", "nan", "inf"] {
            let error = parsed(&["--render-scale", refused]).unwrap_err();
            assert!(
                error.starts_with("--render-scale needs a value from 0.5 to 1"),
                "{refused}: {error}"
            );
        }
    }

    #[test]
    fn an_indirect_light_volume_is_a_box_a_spacing_and_bounces_within_their_ranges() {
        let arguments = parsed(&["--indirect-light", "1,2,3,8,4,8,0.5,2"])
            .unwrap()
            .unwrap();
        assert_eq!(
            arguments.indirect_light,
            Some(IndirectLightDescriptor {
                center: [1.0, 2.0, 3.0],
                extent: [8.0, 4.0, 8.0],
                spacing: 0.5,
                bounces: 2,
                ambient: render_model::IndirectAmbient::Sky,
            })
        );
        let floor = parsed(&["--indirect-light", "0,0,0,4,4,4,1,1,floor"])
            .unwrap()
            .unwrap();
        assert_eq!(
            floor.indirect_light.unwrap().ambient,
            render_model::IndirectAmbient::Floor
        );
        for refused in [
            "1,2,3",
            "0,0,0,4,4,4,0.1,1",
            "0,0,0,4,4,4,1,9",
            "0,0,0,200,200,200,0.5,1",
        ] {
            let error = parsed(&["--indirect-light", refused]).unwrap_err();
            assert!(error.starts_with("--indirect-light"), "{refused}: {error}");
        }
    }

    #[test]
    fn the_snapshot_and_image_are_required() {
        assert!(parse(["scene.rscene".to_owned()]).is_err());
        assert_eq!(parse(["--help".to_owned()]), Ok(None));
        assert_eq!(
            parsed(&[]).unwrap().unwrap().snapshot,
            PathBuf::from("scene.rscene")
        );
    }
}
