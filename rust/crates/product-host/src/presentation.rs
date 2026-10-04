//! The presentation-layout route: `POST /__rusty/product/runtime/presentation`.
//!
//! The page that shows the product reports how it is presented: the
//! presentation surface's size in CSS pixels, its device pixel ratio, the UI
//! scale, and the rects of the UI elements anchored for camera views
//! (`viewport.anchor(name, element)` in the UI context), normalized to the
//! surface and bottom-left based like a camera viewport. It reports on
//! change, from stream and window output alike. This is presentation, not
//! gameplay input: it takes no binding or sequence, and the latest report
//! wins. The renderer follows the anchors at once, without a product call,
//! and the product reads the rest through `Presentation.Read`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use render_host_contracts::{RendererViewport, RendererViewportAnchors};
use serde::Deserialize;
use ts_rs::TS;

pub const PRODUCT_HOST_PRESENTATION_PATH: &str = "/__rusty/product/runtime/presentation";
/// More anchors than any layout needs; a report beyond it is refused.
pub const MAX_VIEWPORT_ANCHORS: usize = 64;
pub const MAX_VIEWPORT_ANCHOR_NAME_BYTES: usize = 64;

/// What the page reports about its presentation.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostPresentationReport {
    pub css_width: f64,
    pub css_height: f64,
    pub device_pixel_ratio: f64,
    pub ui_scale: f64,
    pub anchors: Vec<ProductHostViewportAnchorReport>,
}

/// One anchored element's rect, normalized to the presentation surface,
/// bottom-left based.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostViewportAnchorReport {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The presentation as the page last reported it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductHostPresentationLayout {
    pub css_width: f64,
    pub css_height: f64,
    pub device_pixel_ratio: f64,
    pub ui_scale: f64,
    /// Anchored rects clipped to the surface; an anchor wholly outside it
    /// is absent.
    pub anchors: RendererViewportAnchors,
    /// Counts reports that changed the layout, from 1.
    pub revision: u64,
}

impl ProductHostPresentationReport {
    /// The layout this report describes, or why it is refused.
    pub(crate) fn layout(self) -> Result<ProductHostPresentationLayout, &'static str> {
        let positive = |value: f64| value.is_finite() && value > 0.0;
        if !positive(self.css_width) || !positive(self.css_height) {
            return Err("presentation size must be finite and positive");
        }
        if !positive(self.device_pixel_ratio) || !positive(self.ui_scale) {
            return Err("device pixel ratio and UI scale must be finite and positive");
        }
        if self.anchors.len() > MAX_VIEWPORT_ANCHORS {
            return Err("too many viewport anchors");
        }
        let mut anchors = BTreeMap::new();
        for anchor in self.anchors {
            if anchor.name.is_empty() || anchor.name.len() > MAX_VIEWPORT_ANCHOR_NAME_BYTES {
                return Err("a viewport anchor name must be 1 to 64 bytes");
            }
            if ![anchor.x, anchor.y, anchor.width, anchor.height]
                .iter()
                .all(|value| value.is_finite())
            {
                return Err("a viewport anchor rect must be finite");
            }
            // Clip to the surface, as the page's own overflow does.
            let (left, bottom) = (anchor.x.max(0.0), anchor.y.max(0.0));
            let right = (anchor.x + anchor.width).min(1.0);
            let top = (anchor.y + anchor.height).min(1.0);
            if right > left && top > bottom {
                anchors.insert(
                    anchor.name,
                    RendererViewport {
                        x: left,
                        y: bottom,
                        width: right - left,
                        height: top - bottom,
                    },
                );
            }
        }
        Ok(ProductHostPresentationLayout {
            css_width: self.css_width,
            css_height: self.css_height,
            device_pixel_ratio: self.device_pixel_ratio,
            ui_scale: self.ui_scale,
            anchors,
            revision: 0,
        })
    }
}

type AnchorListener = Box<dyn Fn(&RendererViewportAnchors) + Send + Sync>;

/// The latest reported layout, shared by the host route and the runtime.
#[derive(Default)]
pub struct ProductHostPresentation {
    layout: Mutex<Option<ProductHostPresentationLayout>>,
    on_anchors: Mutex<Option<AnchorListener>>,
}

impl ProductHostPresentation {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// Called with the anchors whenever a report changes them, so the
    /// renderer follows without a product call.
    pub fn set_anchor_listener(
        &self,
        listener: impl Fn(&RendererViewportAnchors) + Send + Sync + 'static,
    ) {
        *self
            .on_anchors
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(listener));
    }

    /// The layout the page last reported, if it has reported one.
    pub fn layout(&self) -> Option<ProductHostPresentationLayout> {
        self.state().clone()
    }

    pub(crate) fn report(&self, mut layout: ProductHostPresentationLayout) {
        let mut state = self.state();
        let previous = state.as_ref();
        let anchors_changed = previous.is_none_or(|previous| previous.anchors != layout.anchors);
        layout.revision = previous.map_or(0, |previous| previous.revision);
        if previous.is_some_and(|previous| {
            ProductHostPresentationLayout {
                revision: layout.revision,
                ..previous.clone()
            } == layout
        }) {
            return;
        }
        layout.revision += 1;
        let anchors = layout.anchors.clone();
        *state = Some(layout);
        drop(state);
        if anchors_changed {
            if let Some(listener) = &*self
                .on_anchors
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
            {
                listener(&anchors);
            }
        }
    }

    fn state(&self) -> MutexGuard<'_, Option<ProductHostPresentationLayout>> {
        self.layout
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(
        width: f64,
        anchors: Vec<ProductHostViewportAnchorReport>,
    ) -> ProductHostPresentationReport {
        ProductHostPresentationReport {
            css_width: width,
            css_height: 600.0,
            device_pixel_ratio: 2.0,
            ui_scale: 1.0,
            anchors,
        }
    }

    fn anchor(name: &str, x: f64, width: f64) -> ProductHostViewportAnchorReport {
        ProductHostViewportAnchorReport {
            name: name.to_owned(),
            x,
            y: 0.25,
            width,
            height: 0.5,
        }
    }

    #[test]
    fn reports_count_changes_clip_anchors_and_tell_the_renderer_only_of_anchor_changes() {
        let presentation = ProductHostPresentation::new();
        let heard = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&heard);
        presentation.set_anchor_listener(move |anchors| sink.lock().unwrap().push(anchors.clone()));

        let layout = report(
            800.0,
            vec![anchor("hero", 0.75, 0.5), anchor("gone", 1.5, 0.2)],
        )
        .layout()
        .unwrap();
        assert_eq!(
            layout.anchors,
            [(
                "hero".to_owned(),
                RendererViewport {
                    x: 0.75,
                    y: 0.25,
                    width: 0.25,
                    height: 0.5
                }
            )]
            .into()
        );
        presentation.report(layout.clone());
        presentation.report(layout);
        assert_eq!(presentation.layout().unwrap().revision, 1);
        // A size change is a new revision but the same anchors.
        presentation.report(
            report(900.0, vec![anchor("hero", 0.75, 0.5)])
                .layout()
                .unwrap(),
        );
        assert_eq!(presentation.layout().unwrap().revision, 2);
        assert_eq!(heard.lock().unwrap().len(), 1);

        assert!(report(0.0, Vec::new()).layout().is_err());
        assert!(report(800.0, vec![anchor("", 0.0, 0.5)]).layout().is_err());
        assert!(report(800.0, vec![anchor("nan", f64::NAN, 0.5)])
            .layout()
            .is_err());
    }
}
