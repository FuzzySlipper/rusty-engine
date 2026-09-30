//! The native half of the capture pair: the same capture, loaded by the same
//! `load_capture`, drawn offscreen and read back as a PNG, then timed.
//!
//! ```text
//! cargo run --release --bin native_reference -- <capture dir> <out.png> [width height] [frames]
//! ```

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, path::PathBuf, time::Instant};

    use render_wgpu::{encode_png, Gpu, OffscreenTarget, Renderer, RendererOptions};
    use render_wgpu_web::{load_capture, resource_identity, MapResources};

    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(
        args.next()
            .ok_or("usage: native_reference <dir> <out.png>")?,
    );
    let out = PathBuf::from(args.next().ok_or("missing output path")?);
    let width: u32 = args.next().map_or(Ok(1280), |value| value.parse())?;
    let height: u32 = args.next().map_or(Ok(720), |value| value.parse())?;
    let frames: u32 = args.next().map_or(Ok(300), |value| value.parse())?;

    let mut resources = MapResources::default();
    for entry in fs::read_dir(dir.join("resources"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        resources
            .0
            .insert(resource_identity(&name), fs::read(entry.path())?);
    }
    let world_frame = fs::read(dir.join("world-frame.json"))?;
    let view = fs::read(dir.join("view.json"))?;

    let clock = Instant::now();
    let now = || clock.elapsed().as_secs_f64() * 1000.0;
    let gpu = Gpu::headless()?;
    eprintln!("adapter: {:?}", gpu.adapter_summary());
    let mut renderer = Renderer::new(&gpu, RendererOptions::default());
    let (view, report) = load_capture(&mut renderer, &world_frame, &view, &resources, now)?;
    eprintln!("load: {report:?}");
    renderer.set_view_composition(&view, 0.0);
    let target = OffscreenTarget::new(&gpu, width, height);
    renderer.render_view_composition(&target, 0.0);
    fs::write(&out, encode_png(width, height, &target.read_rgba(&gpu))?)?;
    eprintln!("wrote {}", out.display());

    // Frame time as the page measures it: encode and submit per frame, then
    // one wait for the GPU at the end (the page's rAF never blocks on it).
    let started = now();
    let mut cpu = Vec::with_capacity(frames as usize);
    for frame in 0..frames {
        let before = now();
        renderer.render_view_composition(&target, f64::from(frame + 1) / 60.0);
        cpu.push(now() - before);
    }
    let _ = target.read_rgba(&gpu);
    let total = now() - started;
    cpu.sort_by(f64::total_cmp);
    eprintln!(
        "{frames} frames: cpu median {:.3} ms, p90 {:.3} ms; wall {:.3} ms per frame incl. GPU",
        cpu[cpu.len() / 2],
        cpu[cpu.len() * 9 / 10],
        total / f64::from(frames)
    );
    Ok(())
}
