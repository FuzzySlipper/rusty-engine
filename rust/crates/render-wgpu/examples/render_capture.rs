//! Render a captured presentation baseline to PNG without Chromium.
//!
//! ```text
//! python3 rust/crates/render-wgpu/scripts/capture-presentation.py [origin] [dir]
//! cargo run -p render-wgpu --example render_capture -- <dir> <out.png> [width height] [source capture.png] [--no-default-world-lights]
//! ```
//!
//! Pass `--no-default-world-lights` for a product whose manifest sets
//! `defaultLights.world` to `disabled` (Dagger).
//!
//! `<dir>` holds `world-frame.json` (the fresh-attachment presentation-world
//! frame), `view.json` (the camera composition) and `resources/` (texture and
//! mesh resources, `/` in identities replaced by `__`). The frame renders
//! through the installed composition: every primary view, its viewmodel
//! pass, offscreen targets and presentations.
//!
//! With `source` (a retained node handle), an output image job then captures
//! that node's subtree from the composition's first camera, as
//! `RenderOutput.CaptureImage` does, into `capture.png`.

use std::{borrow::Cow, fs, path::PathBuf};

use render_host_contracts::{RenderOutputJob, RenderOutputOperation, RendererViewComposition};
use render_model::{RenderFrameDiff, RenderHandle};
use render_presentation::PresentationWorld;
use render_wgpu::{encode_png, Gpu, OffscreenTarget, Renderer, RendererOptions, ResourceSource};

struct DirectoryResources(PathBuf);

impl ResourceSource for DirectoryResources {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        fs::read(self.0.join(identity.replace('/', "__")))
            .ok()
            .map(Cow::Owned)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let default_world_lights = !all.iter().any(|arg| arg == "--no-default-world-lights");
    let mut args = all.into_iter().filter(|arg| !arg.starts_with("--"));
    let dir = PathBuf::from(
        args.next()
            .ok_or("usage: render_capture <dir> <out.png> [width height] [source capture.png]")?,
    );
    let out = PathBuf::from(args.next().ok_or("missing output path")?);
    let width: u32 = args.next().map_or(Ok(1280), |value| value.parse())?;
    let height: u32 = args.next().map_or(Ok(720), |value| value.parse())?;
    let capture: Option<(u64, PathBuf)> = match (args.next(), args.next()) {
        (Some(source), Some(path)) => Some((source.parse()?, PathBuf::from(path))),
        _ => None,
    };

    // Replay the capture through the retained model, then hand the renderer
    // the same baseline a fresh attachment receives.
    let frame: RenderFrameDiff = serde_json::from_slice(&fs::read(dir.join("world-frame.json"))?)?;
    let mut world = PresentationWorld::default();
    world.apply(frame)?;
    let view: RendererViewComposition = serde_json::from_slice(&fs::read(dir.join("view.json"))?)?;

    let gpu = Gpu::headless()?;
    eprintln!("adapter: {:?}", gpu.adapter_summary());
    let resources = DirectoryResources(dir.join("resources"));
    let mut renderer = Renderer::new(
        &gpu,
        RendererOptions {
            default_world_lights,
            ..RendererOptions::default()
        },
    );
    let issues = renderer.apply(&world.snapshot().frame, &resources);
    let mut skipped = std::collections::BTreeMap::<&str, usize>::new();
    for issue in &issues {
        *skipped.entry(issue.op).or_default() += 1;
    }
    eprintln!(
        "tables: {:?}; skipped ops: {skipped:?}",
        renderer.table_counts()
    );
    if let Some(issue) = issues.first() {
        eprintln!("first skipped op: {issue:?}");
    }
    let target = OffscreenTarget::new(&gpu, width, height);
    renderer.set_view_composition(&view, 0.0);
    let stats = renderer.render_view_composition(&target, 0.0);
    eprintln!("frame: {stats:?}");
    fs::write(&out, encode_png(width, height, &target.read_rgba(&gpu))?)?;
    eprintln!("wrote {}", out.display());

    if let Some((source, path)) = capture {
        let camera = view
            .cameras
            .first()
            .ok_or("view composition has no camera")?;
        let source = RenderHandle::new(source);
        let job = RenderOutputJob {
            id: 1,
            source,
            frame: world.capture_output_scene(source, true)?,
            operation: RenderOutputOperation::Image {
                camera: Box::new(camera.clone()),
                width,
                height,
                background: [0.0; 4],
                use_camera_background: true,
                exposure: 1.0,
                aces_filmic: false,
                samples: 4,
                pose: None,
            },
        };
        fs::write(&path, renderer.capture_image(&job, &resources)?)?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}
