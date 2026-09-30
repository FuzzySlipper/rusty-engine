//! #8874 spike: feed a captured presentation world to `render-wgpu` in the
//! page, on a canvas, as WebAssembly on WebGPU.
//!
//! The capture is the Doom room study the retired `render_capture` example
//! read (`world-frame.json`, `view.json`, `resources/`). The page fetches the
//! files and hands their bytes over; this crate does what the native runtime
//! does with a fresh attachment: replay the frame through `PresentationWorld`,
//! apply its snapshot to a new renderer, install the camera composition, and
//! draw it.

use std::{borrow::Cow, collections::HashMap};

use render_host_contracts::RendererViewComposition;
use render_model::RenderFrameDiff;
use render_presentation::PresentationWorld;
use render_wgpu::{Renderer, ResourceSource};

#[cfg(target_arch = "wasm32")]
mod web;

/// Resource bytes by identity (`texture-resource/…`).
#[derive(Default)]
pub struct MapResources(pub HashMap<String, Vec<u8>>);

impl ResourceSource for MapResources {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .get(identity)
            .map(|bytes| Cow::Borrowed(bytes.as_slice()))
    }
}

/// What loading a capture did, for the page and the report.
#[derive(Debug, Default, Clone)]
pub struct LoadReport {
    pub parse_ms: f64,
    pub apply_ms: f64,
    pub skipped_ops: usize,
    pub tables: String,
}

/// Replay the capture and apply its snapshot to `renderer`; returns the
/// view composition to install. `now` is the host's millisecond clock.
pub fn load_capture(
    renderer: &mut Renderer,
    world_frame: &[u8],
    view: &[u8],
    resources: &MapResources,
    now: impl Fn() -> f64,
) -> Result<(RendererViewComposition, LoadReport), String> {
    let started = now();
    let frame: RenderFrameDiff =
        serde_json::from_slice(world_frame).map_err(|error| error.to_string())?;
    let view: RendererViewComposition =
        serde_json::from_slice(view).map_err(|error| error.to_string())?;
    let mut world = PresentationWorld::default();
    world.apply(frame).map_err(|error| format!("{error:?}"))?;
    let snapshot = world.snapshot().frame;
    let parsed = now();
    let issues = renderer.apply(&snapshot, resources);
    let applied = now();
    Ok((
        view,
        LoadReport {
            parse_ms: parsed - started,
            apply_ms: applied - parsed,
            skipped_ops: issues.len(),
            tables: format!("{:?}", renderer.table_counts()),
        },
    ))
}

/// Resource files from a capture's `resources/` directory: `/` in each
/// identity was written as `__`.
pub fn resource_identity(file_name: &str) -> String {
    file_name.replace("__", "/")
}
