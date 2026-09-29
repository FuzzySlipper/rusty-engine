//! Billboard labels (#8827): `PresentationOp::Billboard` drawn by wgpu, in
//! place of renderer-host's DOM billboard host.
//!
//! A label's content is rasterized on the CPU when it is created or updated,
//! into an image laid out as the DOM host's CSS laid out its element (font
//! size = `height_pixels`, line height 1.2, the structured indicator's flex
//! column), then uploaded as one texture. Each primary view projects the
//! anchors, applies the DOM host's visibility and layout policy, and draws
//! the images as screen-space quads:
//!
//! - `DepthTested` and `Occluded` labels draw after the world pass, before the
//!   viewmodel. `DepthTested` depth-tests every pixel against the scene;
//!   `Occluded` hides the whole label when the scene covers its anchor (the
//!   anchor's depth is sampled in the vertex shader). The DOM host had no
//!   depth readback, so both always showed there.
//! - `AlwaysOnTop` labels draw after the viewmodel, over everything.
//!
//! Output captures and offscreen composition targets draw no labels, as the
//! DOM overlay never reached them.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use glam::{Vec3, Vec4};
use render_presentation::{
    BillboardAlignment, BillboardAnchor, BillboardContent, BillboardDescriptor,
    BillboardEdgeBehavior, BillboardFontRef, BillboardHandle, BillboardIndicator, BillboardLayer,
    BillboardLayoutPolicy, BillboardLayoutSizing, BillboardMeter, BillboardMeterFillDirection,
    BillboardOverlapBehavior, BillboardProjectionOp, BillboardTextureRef,
};

use crate::camera::CameraMatrices;
use crate::frame::PixelRect;
use crate::particles::EntityPositions;
use crate::resources::{self, DecodedImage, ResourceSource};
use crate::target::{ColorTarget, TargetView, DEPTH_FORMAT};
use crate::Renderer;

/// Every system family resolves to this bundled face (`fonts/LICENSE`).
const DEFAULT_FONT: &[u8] = include_bytes!("../fonts/DejaVuSans.ttf");
/// CSS `line-height: 1.2`, which the DOM host set on every label.
const LINE_HEIGHT: f32 = 1.2;
/// The DOM host's `border-radius` for text, value and icon labels.
const LABEL_RADIUS_PIXELS: f32 = 4.0;
/// Meter rows are `height: 0.5em` with a 1px border.
const METER_HEIGHT_EM: f32 = 0.5;
const BORDER_PIXELS: f32 = 1.0;
/// The meter segment divider colour of the DOM host's gradient.
const SEGMENT_DIVIDER: [f32; 4] = [0.0, 0.0, 0.0, 0.72];
/// The DOM host laid out at most this many structured labels.
const MAX_VISIBLE_STRUCTURED: usize = 256;
/// A placement closer than this to the previous one keeps the previous one.
const PLACEMENT_HYSTERESIS_PIXELS: f32 = 0.5;
const SCALE_HYSTERESIS: f32 = 0.005;
/// Smallest upward step when stacking overlapping labels.
const STACK_MIN_STEP_PIXELS: f32 = 4.0;

/// A rasterized label: straight-alpha sRGB RGBA8 rows. Empty when its font
/// or icon failed to load.
#[derive(Default)]
pub(crate) struct LabelImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub(crate) struct ActiveLabel {
    pub descriptor: BillboardDescriptor,
    /// The anchor's world position, resolved on apply and on each effects
    /// advance; `None` while an entity anchor is unavailable.
    pub anchor: Option<Vec3>,
    pub image: LabelImage,
    /// Structured placement kept across frames for hysteresis.
    pub placement: Option<(f32, f32, f32)>,
    /// The uploaded image; `None` until drawn after a change.
    texture: Option<wgpu::BindGroup>,
}

pub(crate) struct Labels {
    pub active: BTreeMap<BillboardHandle, ActiveLabel>,
    fonts: HashMap<String, Arc<fontdue::Font>>,
    icons: HashMap<String, Arc<DecodedImage>>,
    default_font: Option<Arc<fontdue::Font>>,
    gpu: Option<LabelGpu>,
    /// Target pixels per CSS pixel (`Renderer::set_pixel_ratio`). Labels are
    /// authored in CSS pixels and rasterized at this ratio.
    pixel_ratio: f32,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            fonts: HashMap::new(),
            icons: HashMap::new(),
            default_font: None,
            gpu: None,
            pixel_ratio: 1.0,
        }
    }
}

impl Labels {
    pub fn pixel_ratio(&self) -> f32 {
        self.pixel_ratio
    }

    /// Rasterize every label again at `ratio`, from the fonts and icons their
    /// creation already loaded.
    pub fn set_pixel_ratio(&mut self, ratio: f32) {
        if ratio == self.pixel_ratio {
            return;
        }
        self.pixel_ratio = ratio;
        let handles: Vec<_> = self.active.keys().copied().collect();
        for handle in handles {
            let descriptor = self.active[&handle].descriptor.clone();
            let image = self
                .rasterize(&descriptor, &resources::NoResources)
                .unwrap_or_default();
            let label = self.active.get_mut(&handle).expect("an active label");
            label.image = image;
            label.texture = None;
        }
    }

    fn default_font(&mut self) -> Result<Arc<fontdue::Font>, String> {
        if let Some(font) = &self.default_font {
            return Ok(font.clone());
        }
        let font = Arc::new(fontdue::Font::from_bytes(DEFAULT_FONT, Default::default())?);
        self.default_font = Some(font.clone());
        Ok(font)
    }

    /// Apply one billboard op. Engine Presentation's `BillboardProjector`
    /// admitted it against the same retained set, so handles and patched
    /// descriptors are consistent; what can fail here is loading a font or
    /// icon. A label that fails keeps its descriptor, drawing nothing until an
    /// update realizes it.
    pub fn apply(
        &mut self,
        op: &BillboardProjectionOp,
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
    ) -> Result<(), String> {
        let (handle, descriptor) = match op {
            BillboardProjectionOp::Create { handle, descriptor } => (*handle, descriptor.clone()),
            BillboardProjectionOp::Update { handle, patch } => {
                let Some(current) = self.active.get(handle) else {
                    return Err("billboard handle is not active in this renderer".to_owned());
                };
                (*handle, current.descriptor.clone().patched(patch))
            }
            BillboardProjectionOp::Destroy { handle } => {
                self.active.remove(handle);
                return Ok(());
            }
        };
        // Doom updates its indicator every frame; only what the image shows
        // needs a new raster.
        if let Some(label) = self.active.get_mut(&handle).filter(|label| {
            same_image(&label.descriptor, &descriptor) && !label.image.rgba.is_empty()
        }) {
            label.anchor = resolve_anchor(&descriptor.anchor, entities);
            label.descriptor = descriptor;
            return Ok(());
        }
        let rasterized = self.rasterize(&descriptor, resources);
        let placement = self.active.get(&handle).and_then(|label| label.placement);
        let (image, result) = match rasterized {
            Ok(image) => (image, Ok(())),
            Err(error) => (LabelImage::default(), Err(error)),
        };
        self.active.insert(
            handle,
            ActiveLabel {
                anchor: resolve_anchor(&descriptor.anchor, entities),
                descriptor,
                image,
                placement,
                texture: None,
            },
        );
        result
    }

    /// Re-resolve entity anchors from the Engine's current positions.
    pub fn refresh_anchors(&mut self, entities: EntityPositions<'_>) {
        for label in self.active.values_mut() {
            if matches!(
                label.descriptor.anchor,
                BillboardAnchor::EntityAttached { .. }
            ) {
                label.anchor = resolve_anchor(&label.descriptor.anchor, entities);
            }
        }
    }

    fn rasterize(
        &mut self,
        descriptor: &BillboardDescriptor,
        resources: &dyn ResourceSource,
    ) -> Result<LabelImage, String> {
        let font = self.font(&descriptor.font, resources)?;
        let size = descriptor.height_pixels;
        let ratio = self.pixel_ratio;
        match &descriptor.content {
            BillboardContent::Text {
                fallback_text,
                arguments,
                ..
            } => {
                let mut text = fallback_text.clone();
                for argument in arguments {
                    text = text.replace(&format!("{{{}}}", argument.name), &argument.value);
                }
                Ok(text_label(&font, size, &text, descriptor, None, ratio))
            }
            BillboardContent::Value {
                fallback_label,
                value,
                fallback_unit,
                ..
            } => {
                let unit = fallback_unit.as_deref().unwrap_or("");
                let text = if unit.is_empty() {
                    format!("{fallback_label}: {value}")
                } else {
                    format!("{fallback_label}: {value} {unit}")
                };
                Ok(text_label(&font, size, &text, descriptor, None, ratio))
            }
            BillboardContent::Icon {
                texture,
                fallback_alt,
                ..
            } => {
                let icon = self.icon(texture, resources)?;
                Ok(text_label(
                    &font,
                    size,
                    fallback_alt,
                    descriptor,
                    Some(&icon),
                    ratio,
                ))
            }
            BillboardContent::Structured { indicator } => {
                let mut icons = HashMap::new();
                for texture in indicator.icon.iter().chain(
                    indicator
                        .status_cues
                        .iter()
                        .filter_map(|cue| cue.icon.as_ref()),
                ) {
                    icons.insert(texture.content_hash.clone(), self.icon(texture, resources)?);
                }
                Ok(structured_label(
                    &font, size, indicator, descriptor, &icons, ratio,
                ))
            }
        }
    }

    fn font(
        &mut self,
        font: &BillboardFontRef,
        resources: &dyn ResourceSource,
    ) -> Result<Arc<fontdue::Font>, String> {
        let BillboardFontRef::Asset {
            asset,
            content_hash,
            ..
        } = font
        else {
            // No system font lookup: every family draws with the bundled face.
            return self.default_font();
        };
        if let Some(font) = self.fonts.get(content_hash) {
            return Ok(font.clone());
        }
        let bytes = resources
            .bytes(asset)
            .ok_or_else(|| format!("fontLoadFailed: font resource {asset} is unavailable"))?;
        let face = match bytes.get(..4) {
            Some(b"wOF2") => wuff::decompress_woff2(&bytes)
                .map_err(|error| format!("fontLoadFailed: {error:?}"))?,
            Some(b"wOFF") => wuff::decompress_woff1(&bytes)
                .map_err(|error| format!("fontLoadFailed: {error:?}"))?,
            _ => bytes.to_vec(),
        };
        let font = Arc::new(
            fontdue::Font::from_bytes(face, Default::default())
                .map_err(|error| format!("fontLoadFailed: {error}"))?,
        );
        self.fonts.insert(content_hash.clone(), font.clone());
        Ok(font)
    }

    fn icon(
        &mut self,
        texture: &BillboardTextureRef,
        resources: &dyn ResourceSource,
    ) -> Result<Arc<DecodedImage>, String> {
        if let Some(icon) = self.icons.get(&texture.content_hash) {
            return Ok(icon.clone());
        }
        let identity = format!(
            "texture-resource/{}",
            texture
                .content_hash
                .strip_prefix("sha256:")
                .unwrap_or(&texture.content_hash)
        );
        // The resource identity the texture was admitted under, as for
        // particle textures; `asset` names the texture definition.
        let bytes = resources.bytes(&identity).ok_or_else(|| {
            format!(
                "iconLoadFailed: icon resource {} is unavailable",
                texture.asset
            )
        })?;
        let icon = Arc::new(
            resources::decode_png(&bytes).map_err(|error| format!("iconLoadFailed: {error}"))?,
        );
        self.icons
            .insert(texture.content_hash.clone(), icon.clone());
        Ok(icon)
    }
}

/// Whether two descriptors rasterize to the same image.
fn same_image(a: &BillboardDescriptor, b: &BillboardDescriptor) -> bool {
    a.content == b.content
        && a.font == b.font
        && a.height_pixels == b.height_pixels
        && a.color == b.color
        && a.background == b.background
}

fn resolve_anchor(anchor: &BillboardAnchor, entities: EntityPositions<'_>) -> Option<Vec3> {
    match anchor {
        BillboardAnchor::World { position } => Some(crate::convert::vec3(*position)),
        BillboardAnchor::EntityAttached { entity, offset } => entities(*entity)
            .map(|position| crate::convert::vec3(position) + crate::convert::vec3(*offset)),
    }
}

// ── Rasterizing ─────────────────────────────────────────────────────────────

/// A straight-alpha canvas in sRGB space, composited source-over as a browser
/// composites its page. Callers draw in CSS pixels; the canvas holds `scale`
/// pixels per CSS pixel, as a browser rasterizes at its device pixel ratio.
struct Canvas {
    width: u32,
    height: u32,
    scale: f32,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(width: f32, height: f32, scale: f32) -> Self {
        let (width, height) = (
            (width * scale).ceil().max(1.0) as u32,
            (height * scale).ceil().max(1.0) as u32,
        );
        Self {
            width,
            height,
            scale,
            pixels: vec![[0.0; 4]; (width * height) as usize],
        }
    }

    /// The canvas's extent in CSS pixels.
    fn bounds(&self) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: self.width as f32 / self.scale,
            height: self.height as f32 / self.scale,
        }
    }

    fn blend(&mut self, x: i32, y: i32, color: [f32; 4], coverage: f32) {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return;
        }
        let source = color[3] * coverage.clamp(0.0, 1.0);
        if source <= 0.0 {
            return;
        }
        let pixel = &mut self.pixels[(y as u32 * self.width + x as u32) as usize];
        let alpha = source + pixel[3] * (1.0 - source);
        for channel in 0..3 {
            pixel[channel] =
                (color[channel] * source + pixel[channel] * pixel[3] * (1.0 - source)) / alpha;
        }
        pixel[3] = alpha;
    }

    /// Fill a rectangle, rounded by `radius`, antialiased at its edges.
    fn fill_rounded(&mut self, rect: Rect, radius: f32, color: [f32; 4]) {
        let (rect, radius) = (rect.scaled(self.scale), radius * self.scale);
        if color[3] <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
        for y in rect.y.floor() as i32..(rect.y + rect.height).ceil() as i32 {
            for x in rect.x.floor() as i32..(rect.x + rect.width).ceil() as i32 {
                let coverage = rounded_coverage(rect, radius, x as f32 + 0.5, y as f32 + 0.5);
                self.blend(x, y, color, coverage);
            }
        }
    }

    /// Stroke a 1px border inside a rounded rectangle.
    fn stroke_rounded(&mut self, rect: Rect, radius: f32, color: [f32; 4]) {
        if color[3] <= 0.0 {
            return;
        }
        let (rect, radius) = (rect.scaled(self.scale), radius * self.scale);
        let border = BORDER_PIXELS * self.scale;
        let inner = rect.inset(border);
        let inner_radius = (radius - border).max(0.0);
        let outer_radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
        for y in rect.y.floor() as i32..(rect.y + rect.height).ceil() as i32 {
            for x in rect.x.floor() as i32..(rect.x + rect.width).ceil() as i32 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let coverage = rounded_coverage(rect, outer_radius, px, py)
                    - rounded_coverage(inner, inner_radius, px, py);
                self.blend(x, y, color, coverage);
            }
        }
    }

    /// Draw an image scaled into `rect` (bilinear), as CSS draws an image or
    /// a background at a given size.
    fn draw_image(&mut self, image: &DecodedImage, rect: Rect) {
        let rect = rect.scaled(self.scale);
        if image.width == 0 || image.height == 0 || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let sample = |sx: f32, sy: f32| -> [f32; 4] {
            let x = (sx - 0.5).clamp(0.0, (image.width - 1) as f32);
            let y = (sy - 0.5).clamp(0.0, (image.height - 1) as f32);
            let (x0, y0) = (x.floor() as u32, y.floor() as u32);
            let (x1, y1) = (
                (x0 + 1).min(image.width - 1),
                (y0 + 1).min(image.height - 1),
            );
            let (fx, fy) = (x - x0 as f32, y - y0 as f32);
            let texel = |tx: u32, ty: u32| {
                let offset = ((ty * image.width + tx) * 4) as usize;
                std::array::from_fn::<f32, 4, _>(|c| f32::from(image.rgba[offset + c]) / 255.0)
            };
            let (a, b, c, d) = (texel(x0, y0), texel(x1, y0), texel(x0, y1), texel(x1, y1));
            std::array::from_fn(|channel| {
                let top = a[channel] + (b[channel] - a[channel]) * fx;
                let bottom = c[channel] + (d[channel] - c[channel]) * fx;
                top + (bottom - top) * fy
            })
        };
        for y in rect.y.floor() as i32..(rect.y + rect.height).ceil() as i32 {
            for x in rect.x.floor() as i32..(rect.x + rect.width).ceil() as i32 {
                let u = (x as f32 + 0.5 - rect.x) / rect.width * image.width as f32;
                let v = (y as f32 + 0.5 - rect.y) / rect.height * image.height as f32;
                if u < 0.0 || v < 0.0 || u > image.width as f32 || v > image.height as f32 {
                    continue;
                }
                self.blend(x, y, sample(u, v), 1.0);
            }
        }
    }

    /// Draw one line of text with its line box's top at `top`.
    fn draw_text(
        &mut self,
        font: &fontdue::Font,
        size: f32,
        text: &str,
        left: f32,
        top: f32,
        color: [f32; 4],
    ) {
        let (size, left, top) = (size * self.scale, left * self.scale, top * self.scale);
        let baseline = top + baseline_offset(font, size);
        let mut pen = left;
        let mut previous = None;
        for character in text.chars() {
            if let Some(previous) = previous {
                pen += font
                    .horizontal_kern(previous, character, size)
                    .unwrap_or(0.0);
            }
            let (metrics, coverage) = font.rasterize(character, size);
            let origin_x = pen.round() as i32 + metrics.xmin;
            let origin_y = (baseline.round() as i32) - metrics.ymin - metrics.height as i32;
            for row in 0..metrics.height {
                for column in 0..metrics.width {
                    let value = coverage[row * metrics.width + column];
                    if value > 0 {
                        self.blend(
                            origin_x + column as i32,
                            origin_y + row as i32,
                            color,
                            f32::from(value) / 255.0,
                        );
                    }
                }
            }
            pen += metrics.advance_width;
            previous = Some(character);
        }
    }

    fn into_image(self, opacity: f32) -> LabelImage {
        let rgba = self
            .pixels
            .iter()
            .flat_map(|pixel| {
                let alpha = (pixel[3] * opacity).clamp(0.0, 1.0);
                [
                    (pixel[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (pixel[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (pixel[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (alpha * 255.0).round() as u8,
                ]
            })
            .collect();
        LabelImage {
            width: self.width,
            height: self.height,
            rgba,
        }
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn scaled(self, by: f32) -> Self {
        Self {
            x: self.x * by,
            y: self.y * by,
            width: self.width * by,
            height: self.height * by,
        }
    }

    fn inset(self, by: f32) -> Self {
        Self {
            x: self.x + by,
            y: self.y + by,
            width: (self.width - 2.0 * by).max(0.0),
            height: (self.height - 2.0 * by).max(0.0),
        }
    }
}

/// Coverage of a pixel centre by a rounded rectangle, 1px antialiased.
fn rounded_coverage(rect: Rect, radius: f32, px: f32, py: f32) -> f32 {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return 0.0;
    }
    let half = (rect.width / 2.0, rect.height / 2.0);
    let centre = (rect.x + half.0, rect.y + half.1);
    let qx = (px - centre.0).abs() - (half.0 - radius);
    let qy = (py - centre.1).abs() - (half.1 - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius;
    (0.5 - outside).clamp(0.0, 1.0)
}

/// Distance from a CSS line box's top to its baseline: half-leading plus the
/// ascent, with the line box `LINE_HEIGHT` × the font size.
fn baseline_offset(font: &fontdue::Font, size: f32) -> f32 {
    let (ascent, descent) = font
        .horizontal_line_metrics(size)
        .map_or((size * 0.8, -size * 0.2), |metrics| {
            (metrics.ascent, metrics.descent)
        });
    (LINE_HEIGHT * size - (ascent - descent)) / 2.0 + ascent
}

fn text_width(font: &fontdue::Font, size: f32, text: &str) -> f32 {
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        if let Some(previous) = previous {
            width += font
                .horizontal_kern(previous, character, size)
                .unwrap_or(0.0);
        }
        width += font.metrics(character, size).advance_width;
        previous = Some(character);
    }
    width
}

/// Text, value and icon labels: one nowrap line of text on the background,
/// radius 4px. An icon label draws its image behind its alternative text,
/// fitted and centred, as the DOM host set it as the element's background.
fn text_label(
    font: &fontdue::Font,
    size: f32,
    text: &str,
    descriptor: &BillboardDescriptor,
    icon: Option<&DecodedImage>,
    scale: f32,
) -> LabelImage {
    let width = text_width(font, size, text);
    let height = LINE_HEIGHT * size;
    let mut canvas = Canvas::new(width, height, scale);
    let bounds = canvas.bounds();
    canvas.fill_rounded(bounds, LABEL_RADIUS_PIXELS, descriptor.background);
    if let Some(icon) = icon {
        canvas.draw_image(icon, contain(icon, bounds, Fit::Centre));
    }
    canvas.draw_text(font, size, text, 0.0, 0.0, descriptor.color);
    canvas.into_image(1.0)
}

#[derive(Clone, Copy)]
enum Fit {
    Centre,
    Left,
}

/// CSS `background-size: contain` of an image within `rect`.
fn contain(image: &DecodedImage, rect: Rect, fit: Fit) -> Rect {
    let scale = (rect.width / image.width as f32).min(rect.height / image.height as f32);
    let (width, height) = (image.width as f32 * scale, image.height as f32 * scale);
    let x = match fit {
        Fit::Centre => rect.x + (rect.width - width) / 2.0,
        Fit::Left => rect.x,
    };
    Rect {
        x,
        y: rect.y + (rect.height - height) / 2.0,
        width,
        height,
    }
}

/// The DOM host's structured indicator: a flex column `width_pixels` wide
/// with a 1px border, `spacing_pixels` padding and gap, and items aligned by
/// `alignment`: the label, the icon at its natural size, full-width meters
/// and the status cues.
fn structured_label(
    font: &fontdue::Font,
    size: f32,
    indicator: &BillboardIndicator,
    descriptor: &BillboardDescriptor,
    icons: &HashMap<String, Arc<DecodedImage>>,
    scale: f32,
) -> LabelImage {
    enum Item<'a> {
        Text(&'a str, Option<&'a DecodedImage>),
        Icon(&'a DecodedImage),
        Meter(&'a BillboardMeter),
    }
    let line = LINE_HEIGHT * size;
    let meter_height = METER_HEIGHT_EM * size + 2.0 * BORDER_PIXELS;
    let mut items: Vec<Item<'_>> = Vec::new();
    if let Some(label) = &indicator.label {
        items.push(Item::Text(&label.fallback_text, None));
    }
    if let Some(icon) = indicator
        .icon
        .as_ref()
        .and_then(|icon| icons.get(&icon.content_hash))
    {
        items.push(Item::Icon(icon));
    }
    items.extend(indicator.meters.iter().map(Item::Meter));
    for cue in &indicator.status_cues {
        let icon = cue
            .icon
            .as_ref()
            .and_then(|icon| icons.get(&icon.content_hash))
            .map(Arc::as_ref);
        items.push(Item::Text(&cue.label.fallback_text, icon));
    }
    let spacing = indicator.spacing_pixels;
    let inset = BORDER_PIXELS + spacing;
    let content_width = (indicator.width_pixels - 2.0 * inset).max(0.0);
    let heights: Vec<f32> = items
        .iter()
        .map(|item| match item {
            Item::Text(..) => line,
            Item::Icon(icon) => icon.height as f32,
            Item::Meter(_) => meter_height,
        })
        .collect();
    let gaps = spacing * items.len().saturating_sub(1) as f32;
    let height = 2.0 * inset + heights.iter().sum::<f32>() + gaps;
    let mut canvas = Canvas::new(indicator.width_pixels, height, scale);
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: indicator.width_pixels,
        height,
    };
    let radius = indicator.style.radius_pixels;
    canvas.fill_rounded(bounds, radius, indicator.style.backing);
    canvas.stroke_rounded(bounds, radius, indicator.style.border);
    let align = |width: f32| match indicator.alignment {
        BillboardAlignment::Start => inset,
        BillboardAlignment::Center => inset + (content_width - width) / 2.0,
        BillboardAlignment::End => inset + content_width - width,
    };
    let mut top = inset;
    for (item, item_height) in items.iter().zip(&heights) {
        match item {
            Item::Text(text, icon) => {
                let width = text_width(font, size, text);
                let left = align(width);
                if let Some(icon) = icon {
                    let rect = Rect {
                        x: left,
                        y: top,
                        width,
                        height: line,
                    };
                    canvas.draw_image(icon, contain(icon, rect, Fit::Left));
                }
                canvas.draw_text(font, size, text, left, top, descriptor.color);
            }
            Item::Icon(icon) => {
                let rect = Rect {
                    x: align(icon.width as f32),
                    y: top,
                    width: icon.width as f32,
                    height: icon.height as f32,
                };
                canvas.draw_image(icon, rect);
            }
            Item::Meter(meter) => {
                // `width: 100%` plus a content-box border: the meter overflows
                // the content box by its border on each side.
                let rect = Rect {
                    x: inset - BORDER_PIXELS,
                    y: top,
                    width: content_width + 2.0 * BORDER_PIXELS,
                    height: meter_height,
                };
                draw_meter(&mut canvas, meter, rect);
            }
        }
        top += item_height + spacing;
    }
    canvas.into_image(indicator.style.opacity)
}

fn draw_meter(canvas: &mut Canvas, meter: &BillboardMeter, rect: Rect) {
    // CSS paints the background under the border as well.
    canvas.fill_rounded(rect, 0.0, meter.back);
    canvas.stroke_rounded(rect, 0.0, meter.border);
    let inner = rect.inset(BORDER_PIXELS);
    // Presentation admits only min < max with values in range.
    let fraction = |value: f32| (value - meter.min) / (meter.max - meter.min);
    let filled = |fraction: f32| match meter.fill_direction {
        BillboardMeterFillDirection::LeftToRight => Rect {
            width: inner.width * fraction,
            ..inner
        },
        BillboardMeterFillDirection::RightToLeft => Rect {
            x: inner.x + inner.width * (1.0 - fraction),
            width: inner.width * fraction,
            ..inner
        },
        BillboardMeterFillDirection::BottomToTop => Rect {
            y: inner.y + inner.height * (1.0 - fraction),
            height: inner.height * fraction,
            ..inner
        },
        BillboardMeterFillDirection::TopToBottom => Rect {
            height: inner.height * fraction,
            ..inner
        },
    };
    canvas.fill_rounded(
        filled(fraction(meter.preview.unwrap_or(meter.current))),
        0.0,
        meter.preview_fill,
    );
    canvas.fill_rounded(filled(fraction(meter.current)), 0.0, meter.fill);
    if meter.segments > 1 {
        let vertical = matches!(
            meter.fill_direction,
            BillboardMeterFillDirection::BottomToTop | BillboardMeterFillDirection::TopToBottom
        );
        for segment in 1..=u32::from(meter.segments) {
            let at = segment as f32 / f32::from(meter.segments);
            let divider = if vertical {
                Rect {
                    y: inner.y + inner.height * at - 1.0,
                    height: 1.0,
                    ..inner
                }
            } else {
                Rect {
                    x: inner.x + inner.width * at - 1.0,
                    width: 1.0,
                    ..inner
                }
            };
            canvas.fill_rounded(divider, 0.0, SEGMENT_DIVIDER);
        }
    }
}

// ── Layout per view ────────────────────────────────────────────────────────

/// One label to draw in a view pass, in target pixels.
pub(crate) struct Placed {
    pub handle: BillboardHandle,
    /// Left, top, width, height.
    pub rect: [f32; 4],
    /// The anchor in target pixels and its depth (0..1).
    pub anchor: [f32; 3],
    /// Whether the anchor is in the view, where the scene's depth can hide it.
    pub anchor_in_view: bool,
    pub layer: BillboardLayer,
    /// Sort depth for painter's order among depth-layer labels.
    pub depth: f32,
}

struct Projection {
    x: f32,
    y: f32,
    depth: f32,
    distance: f32,
    inside: bool,
}

/// Where the view sees an anchor, as the DOM host's CPU projection computed it
/// (the perspective divide applies even behind the camera, so a clamped
/// label follows the mirrored point as it did there).
fn project(view_proj: &glam::Mat4, eye: Vec3, point: Vec3, area: [f32; 4]) -> Projection {
    let clip = *view_proj * Vec4::new(point.x, point.y, point.z, 1.0);
    let ndc = clip.truncate() / clip.w;
    Projection {
        x: area[0] + (ndc.x + 1.0) / 2.0 * area[2],
        y: area[1] + (1.0 - ndc.y) / 2.0 * area[3],
        // Behind the camera no scene depth compares: the nearest.
        depth: if clip.w > 0.0 { ndc.z } else { 0.0 },
        distance: point.distance(eye),
        inside: clip.w > 0.0
            && (-1.0..=1.0).contains(&ndc.x)
            && (-1.0..=1.0).contains(&ndc.y)
            && (0.0..=1.0).contains(&ndc.z),
    }
}

impl Labels {
    /// The labels one view draws, visibility and layout applied as the DOM
    /// host's `refreshLayout` did. `area` is the view's viewport in target
    /// pixels: x, y, width, height.
    pub fn place(&mut self, view_proj: &glam::Mat4, eye: Vec3, area: [f32; 4]) -> Vec<Placed> {
        let mut placed = Vec::new();
        let mut structured: Vec<(BillboardHandle, Projection, BillboardLayoutPolicy)> = Vec::new();
        for (handle, label) in &self.active {
            let descriptor = &label.descriptor;
            let (Some(anchor), false) = (label.anchor, label.image.rgba.is_empty()) else {
                continue;
            };
            let projection = project(view_proj, eye, anchor, area);
            let is_structured = matches!(descriptor.content, BillboardContent::Structured { .. });
            if !descriptor.visible
                || (!is_structured && !projection.inside)
                || projection.distance > descriptor.max_distance
            {
                continue;
            }
            if is_structured {
                if let Some(layout) = &descriptor.layout {
                    structured.push((*handle, projection, layout.clone()));
                }
                continue;
            }
            let (width, height) = (label.image.width as f32, label.image.height as f32);
            placed.push(Placed {
                handle: *handle,
                rect: [
                    projection.x - width / 2.0,
                    projection.y - height,
                    width,
                    height,
                ],
                anchor: [projection.x, projection.y, projection.depth],
                anchor_in_view: projection.inside,
                layer: descriptor.layer,
                depth: projection.depth,
            });
        }
        // Structured labels: priority first, then handle, as the DOM host.
        structured.sort_by(|a, b| b.2.priority.cmp(&a.2.priority).then(a.0.cmp(&b.0)));
        let mut occupied: Vec<[f32; 4]> = Vec::new();
        for (handle, projection, policy) in structured {
            if occupied.len() >= MAX_VISIBLE_STRUCTURED {
                break;
            }
            let Some(label) = self.active.get_mut(&handle) else {
                continue;
            };
            let BillboardContent::Structured { indicator } = &label.descriptor.content else {
                continue;
            };
            let scale = layout_scale(&policy, projection.distance);
            // The image is already in target pixels; the policy's lengths are
            // CSS pixels.
            let css = self.pixel_ratio;
            let unscaled = label.image.height as f32;
            let width = label.image.width as f32 * scale;
            let height = unscaled * scale;
            // Placement uses the DOM host's row estimate, as its layout did.
            let layout_height =
                estimated_height(indicator, label.descriptor.height_pixels) * scale * css;
            let half_width = indicator.width_pixels * scale * css / 2.0;
            let safe = policy.safe_area;
            let (left, top, right, bottom) = (
                area[0] + safe.left_pixels * css,
                area[1] + safe.top_pixels * css,
                area[0] + area[2] - safe.right_pixels * css,
                area[1] + area[3] - safe.bottom_pixels * css,
            );
            let (mut x, mut y) = (projection.x, projection.y);
            let outside = x + half_width < left
                || x - half_width > right
                || y < top
                || y - layout_height > bottom;
            match policy.edge_behavior {
                BillboardEdgeBehavior::Cull if !projection.inside || outside => continue,
                BillboardEdgeBehavior::Cull => {}
                BillboardEdgeBehavior::Clamp => {
                    x = clamp(x, left + half_width, right - half_width);
                    y = clamp(y, top + layout_height, bottom);
                }
            }
            if let Some((px, py, pscale)) = label.placement {
                if (px - x).abs() < PLACEMENT_HYSTERESIS_PIXELS * css
                    && (py - y).abs() < PLACEMENT_HYSTERESIS_PIXELS * css
                    && (pscale - scale).abs() < SCALE_HYSTERESIS
                {
                    (x, y) = (px, py);
                }
            }
            let footprint = |x: f32, y: f32| [x - half_width, y - layout_height, x + half_width, y];
            let mut rect = footprint(x, y);
            match policy.overlap_behavior {
                BillboardOverlapBehavior::Stack => {
                    let step = (indicator.spacing_pixels * css + layout_height)
                        .max(STACK_MIN_STEP_PIXELS * css);
                    while occupied.iter().any(|other| overlaps(other, &rect))
                        && y - step - layout_height >= top
                    {
                        y -= step;
                        rect = footprint(x, y);
                    }
                }
                BillboardOverlapBehavior::Suppress => {
                    if occupied.iter().any(|other| overlaps(other, &rect)) {
                        continue;
                    }
                }
            }
            occupied.push(rect);
            label.placement = Some((x, y, scale));
            placed.push(Placed {
                handle,
                // CSS scaled the element about its centre, which
                // `translate(-50%, -100%)` had put `unscaled` above (x, y).
                rect: [
                    x - width / 2.0,
                    y - (unscaled + height) / 2.0,
                    width,
                    height,
                ],
                anchor: [projection.x, projection.y, projection.depth],
                anchor_in_view: projection.inside,
                layer: label.descriptor.layer,
                depth: projection.depth,
            });
        }
        // Painter's order: farther first, then creation order, as the DOM
        // host's depth z-index and element order stacked them.
        placed.sort_by(|a, b| b.depth.total_cmp(&a.depth).then(a.handle.cmp(&b.handle)));
        placed
    }
}

fn layout_scale(policy: &BillboardLayoutPolicy, distance: f32) -> f32 {
    match policy.sizing {
        BillboardLayoutSizing::ConstantPixels => 1.0,
        BillboardLayoutSizing::DistanceScaled {
            reference_distance,
            min_scale,
            max_scale,
        } => clamp(
            reference_distance / distance.max(f32::EPSILON),
            min_scale,
            max_scale,
        ),
    }
}

/// The DOM host's `indicatorHeight`: rows × height plus spacing around them.
fn estimated_height(indicator: &BillboardIndicator, height_pixels: f32) -> f32 {
    let rows = usize::from(indicator.label.is_some())
        + indicator.meters.len()
        + usize::from(!indicator.status_cues.is_empty());
    let rows = rows as f32;
    height_pixels.max(rows * height_pixels + (rows + 1.0) * indicator.spacing_pixels)
}

/// The DOM host's clamp: a range narrower than zero centres.
fn clamp(value: f32, minimum: f32, maximum: f32) -> f32 {
    if minimum > maximum {
        (minimum + maximum) / 2.0
    } else {
        value.clamp(minimum, maximum)
    }
}

fn overlaps(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

// ── Drawing ─────────────────────────────────────────────────────────────────

/// Which labels a pass draws: the depth layers after the world, or
/// `AlwaysOnTop` after the viewmodel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelPass {
    Depth,
    OnTop,
}

/// Floats per instance: the NDC rect, then the anchor.
const INSTANCE_FLOATS: usize = 8;

struct LabelGpu {
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Per multisampled scene depth: its layout, pipeline layout and shader.
    depth_variants: HashMap<bool, DepthVariant>,
    /// Per target and depth-tested (`true`) or not.
    pipelines: HashMap<(ColorTarget, bool), wgpu::RenderPipeline>,
    instances: wgpu::Buffer,
}

struct DepthVariant {
    depth_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
}

impl LabelGpu {
    fn new(device: &wgpu::Device) -> Self {
        let visible = wgpu::ShaderStages::FRAGMENT;
        Self {
            image_layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu label image"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: visible,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: visible,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu label"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            depth_variants: HashMap::new(),
            pipelines: HashMap::new(),
            instances: instance_buffer(device, 64),
        }
    }

    fn variant(&mut self, device: &wgpu::Device, multisampled: bool) -> &DepthVariant {
        let image_layout = &self.image_layout;
        self.depth_variants.entry(multisampled).or_insert_with(|| {
            let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu label scene depth"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled,
                    },
                    count: None,
                }],
            });
            let depth_type = if multisampled {
                "texture_depth_multisampled_2d"
            } else {
                "texture_depth_2d"
            };
            DepthVariant {
                pipeline_layout: device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render-wgpu label"),
                    bind_group_layouts: &[Some(image_layout), Some(&depth_layout)],
                    immediate_size: 0,
                }),
                shader: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("render-wgpu label"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("labels.wgsl")
                            .replace("SCENE_DEPTH", depth_type)
                            .into(),
                    ),
                }),
                depth_layout,
            }
        })
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        target: ColorTarget,
        depth_tested: bool,
    ) -> &wgpu::RenderPipeline {
        if !self.pipelines.contains_key(&(target, depth_tested)) {
            let variant = self.variant(device, target.samples > 1);
            let blend = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            };
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu label"),
                layout: Some(&variant.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &variant.shader,
                    entry_point: Some("vs_label"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: (INSTANCE_FLOATS * 4) as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                // The scene's depth is attached read-only: labels never write it.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(if depth_tested {
                        wgpu::CompareFunction::LessEqual
                    } else {
                        wgpu::CompareFunction::Always
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: target.multisample(),
                fragment: Some(wgpu::FragmentState {
                    module: &variant.shader,
                    entry_point: Some("fs_label"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target.format,
                        blend: Some(wgpu::BlendState {
                            color: blend,
                            alpha: blend,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.pipelines.insert((target, depth_tested), pipeline);
        }
        &self.pipelines[&(target, depth_tested)]
    }

    fn upload(&self, gpu: &crate::Gpu, image: &LabelImage) -> wgpu::BindGroup {
        use wgpu::util::DeviceExt;
        let texture = gpu.device.create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("render-wgpu label"),
                size: crate::target::extent(image.width, image.height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &image.rgba,
        );
        let view = texture.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu label"),
            layout: &self.image_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

fn instance_buffer(device: &wgpu::Device, labels: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu label instances"),
        size: (labels * INSTANCE_FLOATS * 4) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl Renderer {
    /// Lay out the labels one primary view draws.
    pub(crate) fn place_labels(&mut self, camera: &CameraMatrices, area: PixelRect) -> Vec<Placed> {
        if self.labels.active.is_empty() {
            return Vec::new();
        }
        let area = [
            area.x as f32,
            area.y as f32,
            area.width as f32,
            area.height as f32,
        ];
        self.labels.place(&camera.view_proj, camera.eye, area)
    }

    /// Draw one pass's labels over the target, within the view's area.
    /// Returns the draws.
    pub(crate) fn draw_labels(
        &mut self,
        target: &TargetView<'_>,
        area: PixelRect,
        placed: &[Placed],
        pass: LabelPass,
    ) -> u32 {
        let drawn: Vec<&Placed> = placed
            .iter()
            .filter(|label| {
                (label.layer == BillboardLayer::AlwaysOnTop) == (pass == LabelPass::OnTop)
            })
            .collect();
        if drawn.is_empty() {
            return 0;
        }
        let Labels { active, gpu, .. } = &mut self.labels;
        let device = &self.gpu.device;
        let gpu_labels = gpu.get_or_insert_with(|| LabelGpu::new(device));
        let (width, height) = (target.width as f32, target.height as f32);
        let mut rows: Vec<f32> = Vec::with_capacity(drawn.len() * INSTANCE_FLOATS);
        for label in &drawn {
            if let Some(active) = active.get_mut(&label.handle) {
                if active.texture.is_none() {
                    active.texture = Some(gpu_labels.upload(&self.gpu, &active.image));
                }
            }
            // Whole pixels, so an unscaled label maps texels to pixels.
            let [left, top, label_width, label_height] = label.rect;
            let (left, top) = (left.round(), top.round());
            rows.extend_from_slice(&[
                left / width * 2.0 - 1.0,
                1.0 - top / height * 2.0,
                (left + label_width) / width * 2.0 - 1.0,
                1.0 - (top + label_height) / height * 2.0,
                label.anchor[0],
                label.anchor[1],
                label.anchor[2],
                if label.layer == BillboardLayer::Occluded && label.anchor_in_view {
                    1.0
                } else {
                    0.0
                },
            ]);
        }
        let bytes: &[u8] = bytemuck::cast_slice(&rows);
        if gpu_labels.instances.size() < bytes.len() as u64 {
            gpu_labels.instances = instance_buffer(device, drawn.len().next_power_of_two());
        }
        self.gpu.queue.write_buffer(&gpu_labels.instances, 0, bytes);
        let key = target.key();
        gpu_labels.pipeline(device, key, true);
        gpu_labels.pipeline(device, key, false);
        let variant = gpu_labels.variant(device, key.samples > 1);
        let depth_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu label scene depth"),
            layout: &variant.depth_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(target.depth),
            }],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu labels"),
        });
        let mut draws = 0;
        {
            let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu labels"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    depth_slice: None,
                    resolve_target: target.resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // Read-only, so the vertex stage may also sample it.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render.set_scissor_rect(area.x, area.y, area.width, area.height);
            render.set_vertex_buffer(0, gpu_labels.instances.slice(..));
            render.set_bind_group(1, &depth_group, &[]);
            let mut bound = None;
            for (index, label) in drawn.iter().enumerate() {
                let Some(texture) = active
                    .get(&label.handle)
                    .and_then(|active| active.texture.as_ref())
                else {
                    continue;
                };
                let depth_tested = label.layer == BillboardLayer::DepthTested;
                if bound != Some(depth_tested) {
                    render.set_pipeline(&gpu_labels.pipelines[&(key, depth_tested)]);
                    bound = Some(depth_tested);
                }
                render.set_bind_group(0, texture, &[]);
                let instance = index as u32;
                render.draw(0..4, instance..instance + 1);
                draws += 1;
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        draws
    }
}
