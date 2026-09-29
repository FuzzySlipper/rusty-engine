//! Render a captured presentation baseline to PNG without Chromium.
//!
//! ```text
//! python3 rust/crates/render-wgpu/scripts/capture-presentation.py [origin] [dir]
//! cargo run -p render-wgpu --example render_capture -- <dir> <out.png> [width height]
//! ```
//!
//! `<dir>` holds `world-frame.json` (the fresh-attachment presentation-world
//! frame), `view.json` (the camera composition) and `resources/` (texture and
//! mesh resources, `/` in identities replaced by `__`).

use std::{borrow::Cow, fs, path::PathBuf};

use render_host_contracts::RendererViewComposition;
use render_model::RenderFrameDiff;
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
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(
        args.next()
            .ok_or("usage: render_capture <dir> <out.png> [width height]")?,
    );
    let out = PathBuf::from(args.next().ok_or("missing output path")?);
    let width: u32 = args.next().map_or(Ok(1280), |value| value.parse())?;
    let height: u32 = args.next().map_or(Ok(720), |value| value.parse())?;

    // Replay the capture through the retained model, then hand the renderer
    // the same baseline a fresh attachment receives.
    let frame: RenderFrameDiff = serde_json::from_slice(&fs::read(dir.join("world-frame.json"))?)?;
    let mut world = PresentationWorld::default();
    world.apply(frame)?;
    let view: RendererViewComposition = serde_json::from_slice(&fs::read(dir.join("view.json"))?)?;
    let camera = view
        .cameras
        .first()
        .ok_or("view composition has no camera")?;

    let gpu = Gpu::headless()?;
    eprintln!("adapter: {:?}", gpu.adapter_summary());
    let mut renderer = Renderer::new(&gpu, RendererOptions::default());
    let issues = renderer.apply(
        &world.snapshot().frame,
        &DirectoryResources(dir.join("resources")),
    );
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
    let stats = renderer.render_offscreen(camera, &target);
    eprintln!("frame: {stats:?}");
    fs::write(&out, encode_png(width, height, &target.read_rgba(&gpu))?)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}
