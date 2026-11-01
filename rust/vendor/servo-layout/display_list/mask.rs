/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS masks: <https://drafts.fxtf.org/css-masking-1/#positioned-masks>
//!
//! A masked element's mask layers are composed into one alpha image covering its border
//! box, at device resolution, and that image clips the element's stacking context as a
//! WebRender image mask. Each mask image is kept while display lists use it, so an
//! unchanged mask is composed and uploaded once.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use euclid::Size2D;
use net_traits::image_cache::Image as CachedImage;
use paint_api::{CrossProcessPaintApi, SerializableImageData};
use pixels::{PixelFormat, RasterImage};
use rustc_hash::FxHashMap;
use servo_base::generic_channel::GenericSharedMemory;
use style::computed_values::background_clip::single_value::T as Clip;
use style::computed_values::mask_composite::single_value::T as Composite;
use style::computed_values::mask_mode::single_value::T as Mode;
use style::computed_values::mask_origin::single_value::T as Origin;
use style::values::computed::image::Image;
use webrender_api::units::{DeviceIntSize, LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{self as wr, ImageKey};

use super::background::{LayerTiles, get_cyclic, layout_tiles};
use super::conversions::ToWebRender;
use super::gradient::{self, GradientParts, GradientShape};
use super::{BuilderForBoxFragment, DisplayListBuilder};
use crate::context::ResolvedImage;
use crate::replaced::NaturalSizes;

/// The largest side, in device pixels, of a composed mask. A larger mask is composed at a
/// lower resolution and scaled up.
const MAX_MASK_SIDE: f32 = 4096.0;

/// Display lists that may still draw a mask image after the last one that used it. Its
/// image is deleted only after them, so the scene being shown never loses it.
const UNUSED_BUILDS_KEPT: u8 = 2;

/// The composed mask images of one layout thread, kept across display lists.
pub(crate) struct MaskImages {
    paint_api: CrossProcessPaintApi,
    images: FxHashMap<u64, MaskImage>,
}

struct MaskImage {
    key: ImageKey,
    unused_builds: u8,
}

impl MaskImages {
    pub(crate) fn new(paint_api: CrossProcessPaintApi) -> Self {
        Self {
            paint_api,
            images: Default::default(),
        }
    }

    /// The image of the mask `hash` names, composed by `compose` when no kept image has it.
    fn image(
        &mut self,
        hash: u64,
        size: DeviceIntSize,
        new_key: impl FnOnce() -> Option<ImageKey>,
        compose: impl FnOnce() -> Vec<u8>,
    ) -> Option<ImageKey> {
        if let Some(image) = self.images.get_mut(&hash) {
            image.unused_builds = 0;
            return Some(image.key);
        }
        let key = new_key()?;
        let descriptor = wr::ImageDescriptor::new(
            size.width,
            size.height,
            wr::ImageFormat::R8,
            wr::ImageDescriptorFlags::empty(),
        );
        let data = SerializableImageData::Raw(GenericSharedMemory::from_vec(compose()));
        self.paint_api.add_image(key, descriptor, data, false);
        self.images.insert(
            hash,
            MaskImage {
                key,
                unused_builds: 0,
            },
        );
        Some(key)
    }

    /// Count a finished display list, and delete the images no recent one used.
    pub(crate) fn finish_display_list(&mut self) {
        let paint_api = &self.paint_api;
        self.images.retain(|_, image| {
            image.unused_builds += 1;
            if image.unused_builds > UNUSED_BUILDS_KEPT {
                paint_api.delete_image(image.key);
                return false;
            }
            true
        });
    }
}

impl Drop for MaskImages {
    fn drop(&mut self) {
        for image in self.images.values() {
            self.paint_api.delete_image(image.key);
        }
    }
}

/// What one mask layer draws.
enum Source {
    /// A single channel of a raster image: its alpha, or its luminance times its alpha.
    Raster {
        image: Arc<RasterImage>,
        smooth: bool,
    },
    Gradient(GradientParts),
    Constant(f32),
}

struct Layer {
    source: Source,
    luminance: bool,
    tiles: LayerTiles,
    /// The layer's painting area (`mask-clip`).
    clip: LayoutRect,
    composite: Composite,
}

impl DisplayListBuilder<'_> {
    /// An image-mask clip for `fragment_builder`'s mask, in the space of `spatial_id`, or
    /// `None` when its style has no mask image.
    pub(super) fn mask_clip(
        &mut self,
        fragment_builder: &BuilderForBoxFragment,
        spatial_id: wr::SpatialId,
    ) -> Option<wr::ClipId> {
        let style = fragment_builder.fragment.style();
        let svg = style.get_svg();
        if svg.mask_image.0.iter().all(|image| *image == Image::None) {
            return None;
        }

        let area = fragment_builder.border_rect;
        if area.is_empty() {
            return None;
        }
        let scale = self
            .device_pixel_ratio
            .get()
            .min(MAX_MASK_SIDE / area.width())
            .min(MAX_MASK_SIDE / area.height());
        let size = Size2D::new(
            (area.width() * scale).ceil() as i32,
            (area.height() * scale).ceil() as i32,
        );

        let node = fragment_builder.fragment.base.tag.map(|tag| tag.node);
        let smooth = style.clone_image_rendering().to_webrender() == wr::ImageRendering::Auto;
        let mut layers = Vec::with_capacity(svg.mask_image.0.len());
        for (index, image) in svg.mask_image.0.iter().enumerate() {
            let positioning_area = match get_cyclic(&svg.mask_origin.0, index) {
                Origin::BorderBox => area,
                Origin::PaddingBox => *fragment_builder.padding_rect(),
                Origin::ContentBox => *fragment_builder.content_rect(),
            };
            let clip = match get_cyclic(&svg.mask_clip.0, index) {
                Clip::ContentBox => *fragment_builder.content_rect(),
                Clip::PaddingBox => *fragment_builder.padding_rect(),
                Clip::BorderBox | Clip::BorderArea => area,
            };
            let tiles = |natural_sizes| {
                layout_tiles(
                    clip,
                    positioning_area,
                    get_cyclic(&svg.mask_size.0, index),
                    get_cyclic(&svg.mask_repeat.0, index),
                    get_cyclic(&svg.mask_position_x.0, index),
                    get_cyclic(&svg.mask_position_y.0, index),
                    natural_sizes,
                )
            };

            // A layer whose image is `none`, still loading or broken masks everything out.
            let resolved = self.image_resolver.resolve_image(node, image).ok();
            let layer = match resolved {
                Some(ResolvedImage::Image { image, size }) => tiles(
                    NaturalSizes::from_width_and_height(size.width, size.height),
                )
                .and_then(|tiles| {
                    let image = match image {
                        CachedImage::Raster(image) => Some(image),
                        CachedImage::Vector(vector) => {
                            let raster_size = Size2D::new(
                                tiles.tile_size.width * scale,
                                tiles.tile_size.height * scale,
                            )
                            .ceil()
                            .to_i32();
                            node.and_then(|node| {
                                self.image_resolver.rasterize_vector_image(
                                    vector.id,
                                    raster_size,
                                    node,
                                    vector.svg_id,
                                )
                            })
                            .map(Arc::new)
                        },
                    }?;
                    Some((Source::Raster { image, smooth }, tiles))
                }),
                Some(ResolvedImage::Gradient(gradient)) => {
                    tiles(NaturalSizes::empty()).map(|tiles| {
                        let parts = gradient::resolve(style, gradient, tiles.tile_size);
                        (Source::Gradient(parts), tiles)
                    })
                },
                Some(ResolvedImage::Color(color)) => tiles(NaturalSizes::empty())
                    .map(|tiles| (Source::Constant(style.resolve_color(color).alpha), tiles)),
                None => None,
            };
            let (source, tiles) = layer.unwrap_or_else(|| {
                (
                    Source::Constant(0.0),
                    LayerTiles {
                        bounds: area,
                        tile_size: area.size(),
                        tile_spacing: LayoutSize::zero(),
                        repeat: false,
                    },
                )
            });
            layers.push(Layer {
                source,
                luminance: *get_cyclic(&svg.mask_mode.0, index) == Mode::Luminance,
                tiles,
                clip,
                composite: *get_cyclic(&svg.mask_composite.0, index),
            });
        }

        let hash = mask_hash(&layers, area.min, size, scale);
        let image_cache = self.image_resolver.image_cache.clone();
        let key = self.mask_images.image(
            hash,
            size,
            || image_cache.get_image_key(),
            || compose(&layers, area.min, size, scale),
        )?;
        Some(self.wr().define_clip_image_mask(
            spatial_id,
            wr::ImageMask {
                image: key,
                rect: area,
            },
            &[],
            wr::FillRule::Nonzero,
        ))
    }
}

/// Identify a mask by what it is composed of, relative to its area, so the same mask
/// anywhere on the page is composed once.
fn mask_hash(layers: &[Layer], origin: LayoutPoint, size: DeviceIntSize, scale: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    let float = |hasher: &mut DefaultHasher, value: f32| value.to_bits().hash(hasher);
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    float(&mut hasher, scale);
    for layer in layers {
        match &layer.source {
            Source::Raster { image, smooth } => {
                0u8.hash(&mut hasher);
                image.id.hash(&mut hasher);
                Arc::as_ptr(&image.bytes).hash(&mut hasher);
                image.metadata.width.hash(&mut hasher);
                image.metadata.height.hash(&mut hasher);
                smooth.hash(&mut hasher);
            },
            Source::Gradient(parts) => {
                1u8.hash(&mut hasher);
                match parts.shape {
                    GradientShape::Linear { start, end } => {
                        0u8.hash(&mut hasher);
                        for value in [start.x, start.y, end.x, end.y] {
                            float(&mut hasher, value);
                        }
                    },
                    GradientShape::Radial { center, radius } => {
                        1u8.hash(&mut hasher);
                        for value in [center.x, center.y, radius.width, radius.height] {
                            float(&mut hasher, value);
                        }
                    },
                    GradientShape::Conic { center, angle } => {
                        2u8.hash(&mut hasher);
                        for value in [center.x, center.y, angle] {
                            float(&mut hasher, value);
                        }
                    },
                }
                (parts.extend_mode == wr::ExtendMode::Repeat).hash(&mut hasher);
                for stop in &parts.stops {
                    for value in [
                        stop.offset,
                        stop.color.r,
                        stop.color.g,
                        stop.color.b,
                        stop.color.a,
                    ] {
                        float(&mut hasher, value);
                    }
                }
            },
            Source::Constant(alpha) => {
                2u8.hash(&mut hasher);
                float(&mut hasher, *alpha);
            },
        }
        layer.luminance.hash(&mut hasher);
        (layer.composite as u8).hash(&mut hasher);
        layer.tiles.repeat.hash(&mut hasher);
        let bounds = layer.tiles.bounds.translate(-origin.to_vector());
        let clip = layer.clip.translate(-origin.to_vector());
        for value in [
            bounds.min.x,
            bounds.min.y,
            bounds.max.x,
            bounds.max.y,
            clip.min.x,
            clip.min.y,
            clip.max.x,
            clip.max.y,
            layer.tiles.tile_size.width,
            layer.tiles.tile_size.height,
            layer.tiles.tile_spacing.width,
            layer.tiles.tile_spacing.height,
        ] {
            float(&mut hasher, value);
        }
    }
    hasher.finish()
}

/// Compose the mask layers into one alpha channel, a byte per device pixel.
///
/// <https://drafts.fxtf.org/css-masking-1/#the-mask-composite>: the last layer is the
/// bottom one, and each layer above it is composited onto what lies beneath by its
/// `mask-composite` operator.
fn compose(layers: &[Layer], origin: LayoutPoint, size: DeviceIntSize, scale: f32) -> Vec<u8> {
    let width = size.width as usize;
    let height = size.height as usize;
    let mut mask = vec![0.0f32; width * height];
    let mut layer_values = vec![0.0f32; width * height];
    for (index, layer) in layers.iter().enumerate().rev() {
        let sampler = LayerSampler::new(layer);
        for y in 0..height {
            let point_y = origin.y + (y as f32 + 0.5) / scale;
            for x in 0..width {
                let point = LayoutPoint::new(origin.x + (x as f32 + 0.5) / scale, point_y);
                layer_values[y * width + x] = sampler.sample(layer, point);
            }
        }
        if index == layers.len() - 1 {
            mask.copy_from_slice(&layer_values);
            continue;
        }
        for (destination, source) in mask.iter_mut().zip(&layer_values) {
            let (source, destination_value) = (*source, *destination);
            *destination = match layer.composite {
                Composite::Add => source + destination_value * (1.0 - source),
                Composite::Subtract => source * (1.0 - destination_value),
                Composite::Intersect => source * destination_value,
                Composite::Exclude => {
                    source * (1.0 - destination_value) + destination_value * (1.0 - source)
                },
            };
        }
    }
    mask.iter()
        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}

/// A layer's source made ready to sample.
enum LayerSampler {
    Raster {
        channel: Vec<f32>,
        width: usize,
        height: usize,
        smooth: bool,
    },
    Gradient {
        stops: Vec<(f32, f32)>,
        repeat: bool,
    },
    Constant(f32),
}

impl LayerSampler {
    fn new(layer: &Layer) -> Self {
        match &layer.source {
            Source::Raster { image, smooth } => {
                let frame = image.first_frame();
                let pixels = (frame.width * frame.height) as usize;
                let value = |red: u8, green: u8, blue: u8, alpha: u8| {
                    if layer.luminance {
                        // The image is premultiplied, so its luminance is already scaled by
                        // its alpha.
                        (0.2125 * red as f32 + 0.7154 * green as f32 + 0.0721 * blue as f32) / 255.0
                    } else {
                        alpha as f32 / 255.0
                    }
                };
                let bytes = frame.bytes;
                let channel = match image.format {
                    PixelFormat::BGRA8 => bytes
                        .chunks_exact(4)
                        .map(|pixel| value(pixel[2], pixel[1], pixel[0], pixel[3]))
                        .collect(),
                    PixelFormat::RGBA8 => bytes
                        .chunks_exact(4)
                        .map(|pixel| value(pixel[0], pixel[1], pixel[2], pixel[3]))
                        .collect(),
                    PixelFormat::RGB8 => bytes
                        .chunks_exact(3)
                        .map(|pixel| value(pixel[0], pixel[1], pixel[2], 255))
                        .collect(),
                    PixelFormat::KA8 => bytes
                        .chunks_exact(2)
                        .map(|pixel| value(pixel[0], pixel[0], pixel[0], pixel[1]))
                        .collect(),
                    PixelFormat::K8 => bytes
                        .iter()
                        .map(|&pixel| value(pixel, pixel, pixel, 255))
                        .collect::<Vec<_>>(),
                };
                if channel.len() < pixels || pixels == 0 {
                    return Self::Constant(0.0);
                }
                Self::Raster {
                    channel,
                    width: frame.width as usize,
                    height: frame.height as usize,
                    smooth: *smooth,
                }
            },
            Source::Gradient(parts) => Self::Gradient {
                stops: parts
                    .stops
                    .iter()
                    .map(|stop| {
                        let color = stop.color;
                        let value = if layer.luminance {
                            (0.2125 * color.r + 0.7154 * color.g + 0.0721 * color.b) * color.a
                        } else {
                            color.a
                        };
                        (stop.offset, value)
                    })
                    .collect(),
                repeat: parts.extend_mode == wr::ExtendMode::Repeat,
            },
            Source::Constant(value) => Self::Constant(*value),
        }
    }

    /// The layer's mask value at `point`, in layout space.
    fn sample(&self, layer: &Layer, point: LayoutPoint) -> f32 {
        let tiles = &layer.tiles;
        if !layer.clip.contains(point) || !tiles.bounds.contains(point) {
            return 0.0;
        }
        let stride = tiles.tile_size + tiles.tile_spacing;
        let offset = point - tiles.bounds.min;
        let tile_point = LayoutPoint::new(offset.x % stride.width, offset.y % stride.height);
        if tile_point.x >= tiles.tile_size.width || tile_point.y >= tiles.tile_size.height {
            return 0.0;
        }
        match self {
            Self::Raster {
                channel,
                width,
                height,
                smooth,
            } => {
                let x = tile_point.x / tiles.tile_size.width * *width as f32;
                let y = tile_point.y / tiles.tile_size.height * *height as f32;
                let texel = |x: isize, y: isize| {
                    let x = x.clamp(0, *width as isize - 1) as usize;
                    let y = y.clamp(0, *height as isize - 1) as usize;
                    channel[y * *width + x]
                };
                if !*smooth {
                    return texel(x as isize, y as isize);
                }
                let (x, y) = (x - 0.5, y - 0.5);
                let (left, top) = (x.floor(), y.floor());
                let (fx, fy) = (x - left, y - top);
                let (left, top) = (left as isize, top as isize);
                let upper = texel(left, top) * (1.0 - fx) + texel(left + 1, top) * fx;
                let lower = texel(left, top + 1) * (1.0 - fx) + texel(left + 1, top + 1) * fx;
                upper * (1.0 - fy) + lower * fy
            },
            Self::Gradient { stops, repeat } => {
                let Source::Gradient(parts) = &layer.source else {
                    return 0.0;
                };
                gradient_value(stops, *repeat, gradient_offset(&parts.shape, tile_point))
            },
            Self::Constant(value) => *value,
        }
    }
}

/// Where `point` lies along a gradient's line, as a stop offset.
fn gradient_offset(shape: &GradientShape, point: LayoutPoint) -> f32 {
    match *shape {
        GradientShape::Linear { start, end } => {
            let line = end - start;
            let length_squared = line.square_length();
            if length_squared == 0.0 {
                return 0.0;
            }
            (point - start).dot(line) / length_squared
        },
        GradientShape::Radial { center, radius } => {
            if radius.width <= 0.0 || radius.height <= 0.0 {
                return f32::MAX;
            }
            let offset = point - center;
            (offset.x / radius.width).hypot(offset.y / radius.height)
        },
        GradientShape::Conic { center, angle } => {
            let offset = point - center;
            // 0 points up and angles grow clockwise.
            let turn = offset.x.atan2(-offset.y) - angle;
            (turn / std::f32::consts::TAU).rem_euclid(1.0)
        },
    }
}

fn gradient_value(stops: &[(f32, f32)], repeat: bool, mut offset: f32) -> f32 {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return 0.0;
    };
    let span = last.0 - first.0;
    if repeat && span > 0.0 {
        offset = first.0 + (offset - first.0).rem_euclid(span);
    }
    if offset <= first.0 {
        return first.1;
    }
    for pair in stops.windows(2) {
        let ((start, from), (end, to)) = (pair[0], pair[1]);
        if offset < end {
            if end <= start {
                return to;
            }
            return from + (to - from) * (offset - start) / (end - start);
        }
    }
    last.1
}
