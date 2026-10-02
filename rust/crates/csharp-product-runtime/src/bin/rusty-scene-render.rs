//! Draws a scene snapshot (`engine.renderer.snapshot`) on a fresh renderer,
//! without the product: the same rebaseline and capture the runtime's own
//! renderer performs, on a headless device.
//!
//! ```text
//! rusty-scene-render <snapshot> <out.png> [--width W] [--height H] [--frames N]
//! ```
//!
//! `--frames N` then draws N more frames into one target with readback and
//! reports their mean and median cost. Select the adapter as for any wgpu
//! program (for example `WGPU_BACKEND=vulkan`).

use std::{path::PathBuf, time::Instant};

use csharp_product_runtime::scene_snapshot::{SceneSnapshot, SceneSnapshotChange};
use render_presentation::PresentationWorld;
use render_wgpu::{encode_png, Gpu, OffscreenTarget, SceneDriver};
use serde_json::json;

const USAGE: &str =
    "usage: rusty-scene-render <snapshot> <out.png> [--width W] [--height H] [--frames N]";

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
    while let Some(argument) = arguments.next() {
        let mut number = |name: &str| -> Result<u32, String> {
            arguments
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| format!("{name} needs a number\n{USAGE}"))
        };
        match argument.as_str() {
            "--width" => width = number("--width")?,
            "--height" => height = number("--height")?,
            "--frames" => frames = number("--frames")?,
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
    let driver = SceneDriver::new(gpu, snapshot.metadata.options.into());
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
        let target = OffscreenTarget::new(driver.gpu(), width, height);
        let mut pixels = Vec::new();
        let mut costs = Vec::with_capacity(frames as usize);
        for _ in 0..frames {
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
        });
    }
    let (tables, memory) = driver
        .draw(|renderer, _| (renderer.table_counts(), renderer.mesh_memory()))
        .0;
    let (skipped, last_skip) = driver.skipped_ops();
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
