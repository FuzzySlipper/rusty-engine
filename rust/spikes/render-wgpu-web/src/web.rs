//! The page's side: one viewer over one canvas. JavaScript fetches the
//! capture and drives `requestAnimationFrame`; everything drawn is drawn here.

use render_wgpu::{Gpu, Renderer, RendererOptions, WindowSurface};
use wasm_bindgen::prelude::*;

use crate::{load_capture, resource_identity, MapResources};

fn now() -> f64 {
    js_sys::Reflect::get(&js_sys::global(), &"performance".into())
        .ok()
        .and_then(|performance| {
            let now = js_sys::Reflect::get(&performance, &"now".into()).ok()?;
            let now: js_sys::Function = now.dyn_into().ok()?;
            now.call0(&performance).ok()?.as_f64()
        })
        .unwrap_or(0.0)
}

#[wasm_bindgen]
pub struct Viewer {
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    resources: MapResources,
}

#[wasm_bindgen]
impl Viewer {
    /// Adapter and device for `canvas`, and an empty renderer.
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<Viewer, JsValue> {
        console_error_panic_hook::set_once();
        let (gpu, surface) = Gpu::for_canvas(canvas)
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let renderer = Renderer::new(&gpu, RendererOptions::default());
        Ok(Viewer {
            gpu,
            surface,
            renderer,
            resources: MapResources::default(),
        })
    }

    /// `name` is the file name under the capture's `resources/`.
    pub fn add_resource(&mut self, name: &str, bytes: Vec<u8>) {
        self.resources.0.insert(resource_identity(name), bytes);
    }

    /// Apply the capture; returns a JSON load report.
    pub fn load(&mut self, world_frame: &[u8], view: &[u8]) -> Result<String, JsValue> {
        let (view, report) =
            load_capture(&mut self.renderer, world_frame, view, &self.resources, now)
                .map_err(|error| JsValue::from_str(&error))?;
        self.renderer.set_view_composition(&view, 0.0);
        Ok(format!(
            "{{\"parseMs\":{:.2},\"applyMs\":{:.2},\"skippedOps\":{},\"tables\":{:?},\"adapter\":{:?}}}",
            report.parse_ms,
            report.apply_ms,
            report.skipped_ops,
            report.tables,
            format!("{:?}", self.gpu.adapter_summary()),
        ))
    }

    /// Draw and present one frame at presentation time `time_seconds`;
    /// returns the CPU milliseconds the call took (encode and submit).
    pub fn frame(&mut self, time_seconds: f64) -> Result<f64, JsValue> {
        let started = now();
        self.renderer
            .render_view_composition_to_surface(&mut self.surface, time_seconds)
            .map_err(|skip| JsValue::from_str(&format!("{skip:?}")))?;
        Ok(now() - started)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.resize(&self.gpu, width, height);
    }
}
