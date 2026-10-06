//! Shadow maps for lights that request them, when the host enables shadows
//! (`RendererOptions::shadows`; products opt in through their manifest).
//!
//! Each shadowed light takes layers of the shadow atlas: four cascades for a
//! directional light, one layer for a spot light, six for a point light. A
//! layer is a square tile of 256 to 2048 texels (the light's resolution, else
//! 512, or 1024 for a cascade) in a 2048² page of one depth texture array;
//! pages are cut into tiles of one size and grow with the lights that ask.
//! Every shown scene part casts and every lit part receives, so a new object
//! needs no shadow setup. Each layer draws only the casters inside its view
//! and, for a point or spot light with a range, inside that range. With a
//! budget (`RendererOptions::shadow_budget`), only the lights `choose` keeps
//! cast; the rest light without a shadow.
//!
//! The shadow cameras: spot lights over twice the cone angle; point lights
//! over six 90° faces; both with near 0.5 and far = range, or 500 without
//! one. Casters render back faces of single-sided parts with a slope-scaled
//! depth bias. Receivers look a layer up from one and a half of its texels
//! along their normal, then take a 3×3 PCF (5×5 for a soft light) with linear
//! comparison filtering, kept inside the layer's tile.
//!
//! A directional light's cascades follow each world view's camera. They split
//! the view depth from the camera's near plane to the light's range (100 m by
//! default, never past the camera's far plane) with a practical split. Each
//! cascade is an orthographic box around the bounding sphere of its slice of
//! the camera frustum, so turning the camera keeps its size, with its centre
//! snapped to whole texels so moving the camera does not shimmer. It reaches
//! 500 toward the light, so casters outside the slice still cast into it.
//! Receivers pick a cascade by view depth and blend into the next over the
//! last tenth of each; past the last there is no shadow.
//!
//! An ambient light that requests shadows casts one sky layer: straight down
//! from 250 above its object position over a ±32 box, snapped to its texels
//! so a light that follows the player does not shimmer. Its light reaches a
//! fragment as the sky does, scaled by a wider 5×5 PCF, so ground under rock
//! (a cave, an overhang, a roofed room) loses it and open ground keeps it.
//!
//! A layer is re-rendered only when its view changed or a caster in it was
//! added, removed, moved or posed; a light changing colour or intensity
//! re-renders nothing. Cascades change view as the camera moves.

use std::collections::HashSet;

use glam::{Mat4, Vec3};
use render_model::{LightDescriptor, LightShadowIntent, RenderHandle};
use wgpu::util::DeviceExt;

use crate::batch::DrawList;
use crate::camera::CameraMatrices;
use crate::target::DEPTH_FORMAT;

/// The atlas page side: every layer is a square tile of a page.
pub(crate) const PAGE_SIZE: u32 = 2048;
/// The smallest layer; a resolution rounds up to a power of two between
/// this and a page.
const MIN_LAYER_SIZE: u32 = 256;
/// Layer sizes by light kind when the light names none.
const DEFAULT_LAYER_SIZE: u32 = 512;
const DEFAULT_CASCADE_SIZE: u32 = 1024;
const SHADOW_NEAR: f32 = 0.5;
const SHADOW_FAR: f32 = 500.0;
/// A directional light's cascades, as `lighting.wgsl` reads them.
pub(crate) const CASCADES: u32 = 4;
/// How far from the camera a directional light's shadow reaches without a
/// range.
const DIRECTIONAL_SHADOW_DISTANCE: f32 = 100.0;
/// Split placement between logarithmic (1) and uniform (0).
const CASCADE_SPLIT_LAMBDA: f32 = 0.75;
const SKY_HALF_EXTENT: f32 = 32.0;
/// How far above its object position an ambient light's sky layer looks
/// down from; it sees as far below.
const SKY_HEIGHT: f32 = 250.0;
/// Dynamic uniform offsets must be 256-byte aligned.
const LAYER_UNIFORM_STRIDE: u64 = 256;

/// A layer's size in the atlas and its receivers' filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LayerSettings {
    pub size: u32,
    /// Receivers take 5×5 samples rather than 3×3.
    pub soft: bool,
}

impl LayerSettings {
    fn of(light: &LightDescriptor, default_size: u32) -> Self {
        let settings = light.shadow_settings();
        let size = match settings.resolution {
            0 => default_size,
            resolution => resolution
                .clamp(MIN_LAYER_SIZE, PAGE_SIZE)
                .next_power_of_two(),
        };
        Self {
            size,
            soft: settings.soft,
        }
    }
}

/// What places a shadow layer's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LayerSource {
    /// A view its light fixes: a spot light, a point light's face or an
    /// ambient light's sky, with the sphere a light with a range reaches
    /// (nothing outside it casts into the layer).
    Fixed {
        view_proj: Mat4,
        reach: Option<(Vec3, f32)>,
        settings: LayerSettings,
    },
    /// Cascade `index` of the directional light in light row `row`, fitted
    /// to each world view's camera.
    Cascade {
        direction: Vec3,
        distance: f32,
        index: u32,
        row: u32,
        settings: LayerSettings,
    },
}

impl LayerSource {
    pub fn settings(&self) -> LayerSettings {
        match self {
            Self::Fixed { settings, .. } | Self::Cascade { settings, .. } => *settings,
        }
    }

    /// Both are the same cascade of the same light row.
    fn same_cascade(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (
                Self::Cascade { index, row, .. },
                Self::Cascade { index: other_index, row: other_row, .. },
            ) if index == other_index && row == other_row
        )
    }
}

/// The layers a light casts through; empty for a light that casts none.
/// `row` is the light's row in the lights buffer.
pub(crate) fn light_layers(light: &LightDescriptor, world: &Mat4, row: u32) -> Vec<LayerSource> {
    if light.shadow_intent() != LightShadowIntent::Requested {
        return Vec::new();
    }
    let far = |range: &Option<f32>| range.unwrap_or(SHADOW_FAR);
    let settings = LayerSettings::of(light, DEFAULT_LAYER_SIZE);
    let reach = |position: &[f32; 3], range: &Option<f32>| {
        range.map(|range| {
            (
                world.transform_point3(crate::convert::vec3(*position)),
                range,
            )
        })
    };
    match light {
        LightDescriptor::Ambient { enabled: false, .. }
        | LightDescriptor::Directional { enabled: false, .. }
        | LightDescriptor::Point { enabled: false, .. }
        | LightDescriptor::Spot { enabled: false, .. } => Vec::new(),
        LightDescriptor::Ambient { .. } => {
            let texel = 2.0 * SKY_HALF_EXTENT / settings.size as f32;
            let centre = (world.transform_point3(Vec3::ZERO) / texel).round() * texel;
            let projection = Mat4::orthographic_rh(
                -SKY_HALF_EXTENT,
                SKY_HALF_EXTENT,
                -SKY_HALF_EXTENT,
                SKY_HALF_EXTENT,
                0.0,
                2.0 * SKY_HEIGHT,
            );
            let eye = centre + Vec3::Y * SKY_HEIGHT;
            vec![LayerSource::Fixed {
                view_proj: projection * look_at(eye, centre),
                reach: None,
                settings,
            }]
        }
        LightDescriptor::Directional {
            direction, range, ..
        } => {
            let direction = world
                .transform_vector3(crate::convert::vec3(*direction))
                .normalize_or(Vec3::NEG_Y);
            (0..CASCADES)
                .map(|index| LayerSource::Cascade {
                    direction,
                    distance: range.unwrap_or(DIRECTIONAL_SHADOW_DISTANCE),
                    index,
                    row,
                    settings: LayerSettings::of(light, DEFAULT_CASCADE_SIZE),
                })
                .collect()
        }
        LightDescriptor::Spot {
            position,
            direction,
            range,
            outer_angle_radians,
            ..
        } => {
            let position_local = crate::convert::vec3(*position);
            let eye = world.transform_point3(position_local);
            let target = world
                .transform_point3(position_local + crate::convert::vec3(*direction).normalize());
            let projection = Mat4::perspective_rh(
                (outer_angle_radians * 2.0).min(std::f32::consts::PI - 1e-3),
                1.0,
                SHADOW_NEAR,
                far(range),
            );
            vec![LayerSource::Fixed {
                view_proj: projection * look_at(eye, target),
                reach: reach(position, range),
                settings,
            }]
        }
        LightDescriptor::Point {
            position, range, ..
        } => {
            let eye = world.transform_point3(crate::convert::vec3(*position));
            let projection =
                Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, SHADOW_NEAR, far(range));
            CUBE_FACES
                .iter()
                .map(|(forward, up)| LayerSource::Fixed {
                    view_proj: projection * Mat4::look_to_rh(eye, *forward, *up),
                    reach: reach(position, range),
                    settings,
                })
                .collect()
        }
    }
}

/// The camera's near and far view depths.
pub(crate) fn view_depths(projection: &Mat4) -> (f32, f32) {
    let inverse = projection.inverse();
    let depth = |ndc_z: f32| -inverse.project_point3(Vec3::new(0.0, 0.0, ndc_z)).z;
    (depth(0.0), depth(1.0))
}

/// The far view depth of each cascade: a practical split from `near` to
/// `far`.
pub(crate) fn cascade_splits(near: f32, far: f32) -> [f32; CASCADES as usize] {
    let near = near.max(1e-3);
    let far = far.max(near * 1.001);
    std::array::from_fn(|index| {
        let fraction = (index + 1) as f32 / CASCADES as f32;
        let logarithmic = near * (far / near).powf(fraction);
        let uniform = near + (far - near) * fraction;
        CASCADE_SPLIT_LAMBDA * logarithmic + (1.0 - CASCADE_SPLIT_LAMBDA) * uniform
    })
}

/// A cascade's view-projection: an orthographic box looking along
/// `direction` around the bounding sphere of `camera`'s frustum between view
/// depths `from` and `to`, its centre snapped to whole texels of a `size`
/// map, reaching
/// `SHADOW_FAR` toward the light.
pub(crate) fn cascade_view(
    direction: Vec3,
    camera: &CameraMatrices,
    from: f32,
    to: f32,
    size: u32,
) -> Mat4 {
    // The slice's corners in view space, by depth along each frustum edge.
    let inverse = camera.projection.inverse();
    let (near, far) = view_depths(&camera.projection);
    let radial = |depth: f32| {
        let t = (depth - near) / (far - near);
        [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .iter()
            .map(|&(x, y)| {
                let a = inverse.project_point3(Vec3::new(x, y, 0.0));
                let b = inverse.project_point3(Vec3::new(x, y, 1.0));
                a.lerp(b, t).truncate().length()
            })
            .fold(0.0f32, f32::max)
    };
    let (from_radius, to_radius) = (radial(from), radial(to));
    // The sphere's centre lies on the view axis, at the depth equidistant
    // from both ends' corners (or the far end, when that is nearer).
    let centre_depth = (((to * to + to_radius * to_radius)
        - (from * from + from_radius * from_radius))
        / (2.0 * (to - from)))
        .clamp(from, to);
    let radius = (from_radius.powi(2) + (centre_depth - from).powi(2))
        .max(to_radius.powi(2) + (to - centre_depth).powi(2))
        .sqrt();
    let forward = -camera.view.row(2).truncate();
    let centre = camera.eye + forward * centre_depth;
    let light = look_at(Vec3::ZERO, direction);
    let texel = 2.0 * radius / size as f32;
    let at = light.transform_point3(centre);
    let (x, y) = (
        (at.x / texel).round() * texel,
        (at.y / texel).round() * texel,
    );
    let projection = Mat4::orthographic_rh(
        x - radius,
        x + radius,
        y - radius,
        y + radius,
        -(at.z + radius + SHADOW_FAR),
        -(at.z - radius),
    );
    projection * light
}

/// A light that requests a shadow: its light row, the layers it would cast
/// through, and what a shadow budget weighs.
pub(crate) struct ShadowCandidate {
    pub light: RenderHandle,
    pub row: u32,
    pub layers: Vec<LayerSource>,
    pub priority: i32,
    /// A point or spot light's position; a directional or ambient light
    /// counts as nearest.
    pub position: Option<Vec3>,
}

/// How much nearer a light that already casts counts, so the choice does
/// not flicker as the camera moves.
const CASTING_HYSTERESIS: f32 = 1.0;

/// Which candidates cast. Without a budget, all of them; with one, by
/// priority (higher first), then distance from `eye` (a light in `casting`
/// counted CASTING_HYSTERESIS nearer), each while its layers still fit.
pub(crate) fn choose(
    candidates: &[ShadowCandidate],
    budget: Option<u32>,
    eye: Vec3,
    casting: &HashSet<RenderHandle>,
) -> Vec<bool> {
    let Some(budget) = budget else {
        return vec![true; candidates.len()];
    };
    let distance = |candidate: &ShadowCandidate| {
        let distance = candidate
            .position
            .map_or(0.0, |position| position.distance(eye));
        if casting.contains(&candidate.light) {
            distance - CASTING_HYSTERESIS
        } else {
            distance
        }
    };
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| {
        let (a, b) = (&candidates[a], &candidates[b]);
        b.priority
            .cmp(&a.priority)
            .then(distance(a).total_cmp(&distance(b)))
    });
    let mut chosen = vec![false; candidates.len()];
    let mut used = 0;
    for index in order {
        let layers = candidates[index].layers.len() as u32;
        if used + layers <= budget {
            chosen[index] = true;
            used += layers;
        }
    }
    chosen
}

/// Point light faces in the order `lighting.wgsl`'s `point_face` selects them.
const CUBE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),
    (Vec3::NEG_X, Vec3::NEG_Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::NEG_Y),
    (Vec3::NEG_Z, Vec3::NEG_Y),
];

/// A camera look-at with up +Y, falling back to +Z when the view is
/// vertical.
fn look_at(eye: Vec3, target: Vec3) -> Mat4 {
    let forward = (target - eye).normalize_or(Vec3::NEG_Z);
    let up = if forward.y.abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Mat4::look_to_rh(eye, forward, up)
}

/// Where a layer lives in the shadow atlas: a square of `size` texels at
/// (`x`, `y`) in array layer `page`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Tile {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub size: u32,
}

impl Tile {
    /// The whole page: its pass may clear the attachment.
    pub fn is_page(&self) -> bool {
        self.size == PAGE_SIZE
    }
}

/// One shadow layer's view, its tile, and the casters drawn into it.
pub(crate) struct ShadowLayer {
    pub source: LayerSource,
    /// The source's view, or the cascade's last fit (zero before one).
    pub view_proj: Mat4,
    /// No tile yet (size 0) until `set_layers` places it.
    pub tile: Tile,
    /// Casters inside the view and reach, offset into the instance buffer.
    pub casters: DrawList,
    /// The view, tile or a caster changed since the layer was rendered.
    pub stale: bool,
}

/// The shadow atlas (pages of one depth array), each layer's view and tile,
/// and the per-layer uniform the caster pass selects its view with.
pub(crate) struct ShadowMaps {
    pub array_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// `rusty::types::ShadowView` per layer.
    pub views_buffer: wgpu::Buffer,
    page_views: Vec<wgpu::TextureView>,
    /// The pages' side: 1 for the placeholder before any layer.
    side: u32,
    /// The tile size each page is cut into; 0 for a page no layer uses.
    page_sizes: Vec<u32>,
    /// Layers the views buffer and layer uniform hold.
    layer_capacity: u32,
    pub layer_bind_group: wgpu::BindGroup,
    /// Layers in use.
    pub layers: Vec<ShadowLayer>,
    /// A product's caster stage drew the maps, at presentation `time`.
    pub timed: bool,
    pub time: f64,
}

impl ShadowLayer {
    fn new(source: LayerSource) -> Self {
        Self {
            source,
            view_proj: match source {
                LayerSource::Fixed { view_proj, .. } => view_proj,
                LayerSource::Cascade { .. } => Mat4::ZERO,
            },
            tile: Tile::default(),
            casters: DrawList::default(),
            stale: true,
        }
    }

    pub fn reach(&self) -> Option<(Vec3, f32)> {
        match self.source {
            LayerSource::Fixed { reach, .. } => reach,
            LayerSource::Cascade { .. } => None,
        }
    }

    pub fn is_cascade(&self) -> bool {
        matches!(self.source, LayerSource::Cascade { .. })
    }

    /// A cascade has a view once a world view fitted it.
    pub fn has_view(&self) -> bool {
        self.view_proj != Mat4::ZERO
    }

    /// The layer's `rusty::types::ShadowView` row.
    fn row(&self) -> [f32; VIEW_ROW_FLOATS] {
        let mut row = [0.0; VIEW_ROW_FLOATS];
        row[..16].copy_from_slice(&self.view_proj.to_cols_array());
        let page = PAGE_SIZE as f32;
        let tile = self.tile;
        row[16..20].copy_from_slice(&[
            tile.x as f32 / page,
            tile.y as f32 / page,
            tile.size as f32 / page,
            tile.page as f32,
        ]);
        let soft = if self.source.settings().soft {
            1.0
        } else {
            0.0
        };
        row[20..24].copy_from_slice(&[soft, tile.size as f32, 0.0, 0.0]);
        row
    }
}

/// Floats per `rusty::types::ShadowView`: the view, the tile, the filter.
const VIEW_ROW_FLOATS: usize = 16 + 4 + 4;

impl ShadowMaps {
    pub fn new(device: &wgpu::Device, layer_layout: &wgpu::BindGroupLayout) -> Self {
        Self::with_capacity(device, layer_layout, 1, 1, Vec::new())
    }

    fn with_capacity(
        device: &wgpu::Device,
        layer_layout: &wgpu::BindGroupLayout,
        pages: u32,
        layer_capacity: u32,
        page_sizes: Vec<u32>,
    ) -> Self {
        // A placeholder before any layer is one texel.
        let side = if page_sizes.is_empty() { 1 } else { PAGE_SIZE };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu shadow atlas"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: pages,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("render-wgpu shadow atlas"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let page_views = (0..pages)
            .map(|page| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("render-wgpu shadow page"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: page,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let views_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu shadow views"),
            size: u64::from(layer_capacity) * (VIEW_ROW_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut indices = vec![0u8; (u64::from(layer_capacity) * LAYER_UNIFORM_STRIDE) as usize];
        for layer in 0..layer_capacity {
            let at = (u64::from(layer) * LAYER_UNIFORM_STRIDE) as usize;
            indices[at..at + 4].copy_from_slice(&layer.to_le_bytes());
        }
        let layer_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render-wgpu shadow layer"),
            contents: &indices,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let layer_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu shadow layer"),
            layout: layer_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &layer_uniform,
                    offset: 0,
                    size: wgpu::BufferSize::new(16),
                }),
            }],
        });
        Self {
            array_view,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu shadow comparison"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }),
            views_buffer,
            page_views,
            side,
            page_sizes,
            layer_capacity,
            layer_bind_group,
            layers: Vec::new(),
            timed: false,
            time: 0.0,
        }
    }

    /// Hold the layers' sources and place each in the atlas. A layer whose
    /// source changed is stale and keeps no casters (and a cascade no fit),
    /// except a cascade whose light only turned or changed range; the others
    /// keep theirs, and their tiles. Pages and the per-layer buffers grow as
    /// needed. Returns whether the texture or buffers were replaced (the
    /// frame bind group must be rebuilt) and whether any layer changed
    /// (casters must be culled again).
    pub fn set_layers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_layout: &wgpu::BindGroupLayout,
        sources: &[LayerSource],
    ) -> (bool, bool) {
        let mut layers = std::mem::take(&mut self.layers);
        let mut changed = layers.len() != sources.len();
        layers.truncate(sources.len());
        for (index, &source) in sources.iter().enumerate() {
            match layers.get_mut(index) {
                Some(layer) if layer.source == source => {}
                // A turning sun or a new range keeps the cascade: the next
                // fit gives it a view, and re-renders it only if that view
                // changed.
                Some(layer) if layer.source.same_cascade(&source) => layer.source = source,
                Some(layer) => {
                    *layer = ShadowLayer::new(source);
                    changed = true;
                }
                None => layers.push(ShadowLayer::new(source)),
            }
        }
        let mut page_sizes = std::mem::take(&mut self.page_sizes);
        let placed = place(&mut layers, &mut page_sizes);
        let pages = page_sizes.len() as u32;
        let needed = layers.len() as u32;
        let grow_pages =
            pages > 0 && (self.side != PAGE_SIZE || pages > self.page_views.len() as u32);
        let replaced = grow_pages || needed > self.layer_capacity;
        if replaced {
            let page_capacity = if self.side == PAGE_SIZE {
                (self.page_views.len() as u32).max(pages.next_power_of_two())
            } else {
                pages.max(1).next_power_of_two()
            };
            *self = Self::with_capacity(
                device,
                layer_layout,
                page_capacity,
                needed.max(self.layer_capacity).max(1).next_power_of_two(),
                page_sizes.clone(),
            );
            // A new texture holds nothing.
            for layer in &mut layers {
                layer.stale = true;
            }
        }
        self.page_sizes = page_sizes;
        if (changed || placed || replaced) && !layers.is_empty() {
            let floats: Vec<f32> = layers.iter().flat_map(ShadowLayer::row).collect();
            queue.write_buffer(&self.views_buffer, 0, bytemuck::cast_slice(&floats));
        }
        self.layers = layers;
        (replaced, changed)
    }

    /// Write one layer's view after a cascade fit.
    pub fn write_view(&self, queue: &wgpu::Queue, layer: usize) {
        queue.write_buffer(
            &self.views_buffer,
            (layer * VIEW_ROW_FLOATS * 4) as u64,
            bytemuck::cast_slice(&self.layers[layer].row()),
        );
    }

    /// Caster ids across layers.
    pub fn caster_instances(&self) -> u32 {
        self.layers
            .iter()
            .map(|layer| layer.casters.instances())
            .sum()
    }

    /// Pages any layer uses.
    pub fn pages(&self) -> usize {
        self.page_sizes.iter().filter(|&&size| size > 0).count()
    }

    pub fn page_view(&self, page: u32) -> &wgpu::TextureView {
        &self.page_views[page as usize]
    }

    pub fn layer_offset(layer: u32) -> u32 {
        (u64::from(layer) * LAYER_UNIFORM_STRIDE) as u32
    }
}

/// Give every layer without a tile of its size one, keeping the others'
/// tiles: the first free tile of a page cut to that size, else a page no
/// layer uses, else a new page. A layer that moves is stale. Returns
/// whether any tile changed.
fn place(layers: &mut [ShadowLayer], page_sizes: &mut Vec<u32>) -> bool {
    let size_of = |layer: &ShadowLayer| layer.source.settings().size;
    let mut used: HashSet<(u32, u32, u32)> = layers
        .iter()
        .filter(|layer| {
            layer.tile.size == size_of(layer)
                && page_sizes.get(layer.tile.page as usize) == Some(&layer.tile.size)
        })
        .map(|layer| (layer.tile.page, layer.tile.x, layer.tile.y))
        .collect();
    let mut changed = false;
    for layer in layers.iter_mut() {
        let size = size_of(layer);
        if layer.tile.size == size && used.contains(&(layer.tile.page, layer.tile.x, layer.tile.y))
        {
            continue;
        }
        let free = |page: u32, used: &HashSet<(u32, u32, u32)>| {
            let side = PAGE_SIZE / size;
            (0..side * side)
                .map(|index| (page, index % side * size, index / side * size))
                .find(|tile| !used.contains(tile))
        };
        let tile = (0..page_sizes.len() as u32)
            .filter(|&page| page_sizes[page as usize] == size)
            .find_map(|page| free(page, &used))
            .or_else(|| {
                // A page whose tiles nobody uses is recut to this size.
                let page = (0..page_sizes.len() as u32)
                    .find(|&page| !used.iter().any(|tile| tile.0 == page))?;
                page_sizes[page as usize] = size;
                Some((page, 0, 0))
            })
            .unwrap_or_else(|| {
                page_sizes.push(size);
                (page_sizes.len() as u32 - 1, 0, 0)
            });
        used.insert(tile);
        layer.tile = Tile {
            page: tile.0,
            x: tile.1,
            y: tile.2,
            size,
        };
        layer.stale = true;
        changed = true;
    }
    // Pages no layer uses keep their place, cut to nothing.
    for (page, size) in page_sizes.iter_mut().enumerate() {
        if !used.iter().any(|tile| tile.0 == page as u32) {
            *size = 0;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directional(direction: [f32; 3], intent: LightShadowIntent) -> LightDescriptor {
        LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            direction,
            range: None,
            shadow_intent: intent,
            shadow: Default::default(),
        }
    }

    /// The cascade tests' map size.
    const SIZE: u32 = 512;

    fn fixed_view(source: &LayerSource) -> Mat4 {
        match source {
            LayerSource::Fixed { view_proj, .. } => *view_proj,
            LayerSource::Cascade { .. } => panic!("a fixed layer"),
        }
    }

    #[test]
    fn only_requested_lights_cast() {
        let layers = |light: &LightDescriptor| light_layers(light, &Mat4::IDENTITY, 0).len();
        assert_eq!(
            layers(&directional([0.0, -1.0, 0.0], LightShadowIntent::Disabled)),
            0
        );
        assert_eq!(
            layers(&directional([0.0, -1.0, 0.0], LightShadowIntent::Requested)),
            CASCADES as usize
        );
        let point = LightDescriptor::Point {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            position: [0.0, 2.0, 0.0],
            range: Some(8.0),
            decay: 2.0,
            shadow_intent: LightShadowIntent::Requested,
            shadow: Default::default(),
        };
        assert_eq!(layers(&point), 6);
        let ambient = |shadow_intent| LightDescriptor::Ambient {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            shadow_intent,
            shadow: Default::default(),
        };
        assert_eq!(layers(&ambient(LightShadowIntent::Disabled)), 0);
        assert_eq!(layers(&ambient(LightShadowIntent::Requested)), 1);
    }

    #[test]
    fn an_ambient_sky_layer_looks_down_over_its_object_position_snapped_to_texels() {
        let ambient = LightDescriptor::Ambient {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            shadow_intent: LightShadowIntent::Requested,
            shadow: Default::default(),
        };
        let at = |x: f32| {
            fixed_view(
                &light_layers(&ambient, &Mat4::from_translation(Vec3::new(x, 4.0, 0.0)), 0)[0],
            )
        };
        // Higher is nearer the sky.
        let view = at(10.0);
        let high = view.project_point3(Vec3::new(10.0, 20.0, 0.0));
        let low = view.project_point3(Vec3::new(10.0, -20.0, 0.0));
        assert!(high.x.abs() < 1e-4 && high.y.abs() < 1e-4);
        assert!(0.0 < high.z && high.z < low.z && low.z < 1.0);
        let edge = view.project_point3(Vec3::new(10.0 + 31.0, 0.0, 0.0));
        assert!(edge.x.abs().max(edge.y.abs()) < 1.0);
        // Moving less than half a texel (1/8) keeps the same layer.
        assert_eq!(at(10.0), at(10.05));
    }

    fn camera(position: Vec3, yaw: f32) -> CameraMatrices {
        let pose = crate::camera::CameraPose {
            position,
            orientation: glam::Quat::from_rotation_y(-yaw.to_radians()),
        };
        crate::camera::camera_matrices(
            pose,
            &render_host_contracts::RendererCameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 400.0,
            },
            16.0 / 9.0,
        )
    }

    fn layer(size: u32) -> ShadowLayer {
        ShadowLayer::new(LayerSource::Fixed {
            view_proj: Mat4::IDENTITY,
            reach: None,
            settings: LayerSettings { size, soft: false },
        })
    }

    #[test]
    fn layers_share_pages_by_size_and_keep_their_tiles() {
        let mut pages = Vec::new();
        // Seventeen 512 layers fill one page of sixteen and start another;
        // a 2048 layer takes a page of its own.
        let mut layers: Vec<ShadowLayer> = (0..17).map(|_| layer(512)).collect();
        layers.push(layer(PAGE_SIZE));
        assert!(place(&mut layers, &mut pages));
        assert_eq!(pages, vec![512, 512, PAGE_SIZE]);
        assert_eq!(
            (layers[15].tile.page, layers[15].tile.x, layers[15].tile.y),
            (0, 1536, 1536)
        );
        assert_eq!((layers[16].tile.page, layers[16].tile.x), (1, 0));
        assert!(layers[17].tile.is_page());
        // Placing again moves nothing and leaves the layers clean.
        for layer in &mut layers {
            layer.stale = false;
        }
        assert!(!place(&mut layers, &mut pages));
        assert!(layers.iter().all(|layer| !layer.stale));
        // A layer that grows moves alone; the page the big layer leaves is
        // recut for the next size that needs one.
        layers[3] = layer(1024);
        layers.truncate(17);
        assert!(place(&mut layers, &mut pages));
        assert_eq!(layers[3].tile.size, 1024);
        assert_eq!(pages[layers[3].tile.page as usize], 1024);
        assert!(layers
            .iter()
            .enumerate()
            .all(|(index, layer)| layer.stale == (index == 3)));
        assert_eq!(pages.len(), 3, "the freed page was reused: {pages:?}");
    }

    #[test]
    fn a_budget_keeps_priority_then_the_nearest_lights() {
        let lamp = |raw: u64, x: f32, priority: i32| ShadowCandidate {
            light: RenderHandle::new(raw),
            row: raw as u32,
            layers: (0..6).map(|_| layer(512).source).collect(),
            priority,
            position: Some(Vec3::new(x, 0.0, 0.0)),
        };
        let candidates = [lamp(1, 10.0, 0), lamp(2, 3.0, 0), lamp(3, 20.0, 1)];
        let none = HashSet::new();
        assert_eq!(choose(&candidates, None, Vec3::ZERO, &none), [true; 3]);
        // Room for two lamps: the priority one, then the nearer.
        assert_eq!(
            choose(&candidates, Some(12), Vec3::ZERO, &none),
            [false, true, true]
        );
        // A lamp casting keeps its place against one slightly nearer.
        let near_tie = [lamp(1, 5.5, 0), lamp(2, 5.0, 0)];
        let casting = HashSet::from([RenderHandle::new(1)]);
        assert_eq!(
            choose(&near_tie, Some(6), Vec3::ZERO, &casting),
            [true, false]
        );
        assert_eq!(choose(&near_tie, Some(6), Vec3::ZERO, &none), [false, true]);
    }

    #[test]
    fn cascades_split_the_view_depth_up_to_the_shadow_distance() {
        let splits = cascade_splits(0.1, 100.0);
        assert!(
            splits.windows(2).all(|pair| pair[0] < pair[1]),
            "{splits:?}"
        );
        assert!((splits[3] - 100.0).abs() < 1e-3);
        // The first cascade covers the near field closely.
        assert!(splits[0] < 10.0, "{splits:?}");
        let (near, far) = view_depths(&camera(Vec3::ZERO, 0.0).projection);
        assert!((near - 0.1).abs() < 1e-4 && (far - 400.0).abs() < 0.1);
    }

    #[test]
    fn a_cascade_covers_its_slice_of_the_view_without_shimmering() {
        let sun = Vec3::new(-0.4, -1.0, 0.3).normalize();
        let view = camera(Vec3::new(3.0, 1.7, -2.0), 30.0);
        let (from, to) = (6.0, 15.0);
        let cascade = cascade_view(sun, &view, from, to, SIZE);
        // Every corner of the slice falls inside the box.
        let inverse = view.view_proj.inverse();
        let (near, far) = view_depths(&view.projection);
        for depth in [from, to] {
            let ndc_z = view
                .projection
                .project_point3(Vec3::new(0.0, 0.0, -depth))
                .z;
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = inverse.project_point3(Vec3::new(x, y, ndc_z));
                let shadow = cascade.project_point3(corner);
                assert!(
                    shadow.x.abs() <= 1.0
                        && shadow.y.abs() <= 1.0
                        && (0.0..=1.0).contains(&shadow.z),
                    "{depth} m corner at {shadow:?}"
                );
            }
        }
        assert!(near < from && to < far);
        // A caster far toward the sun still lands in the depth range.
        let centre = view.eye + Vec3::new(0.0, 0.0, -10.0);
        assert!((0.0..1.0).contains(&cascade.project_point3(centre - sun * 300.0).z));

        // Turning the camera keeps the box's size; moving it shifts a fixed
        // world point by whole texels only.
        let turned = cascade_view(sun, &camera(view.eye, 75.0), from, to, SIZE);
        assert!((turned.x_axis.x - cascade.x_axis.x).abs() < 1e-6);
        let texels = |m: Mat4| m.project_point3(Vec3::new(4.0, 0.0, -9.0)) * (SIZE as f32 / 2.0);
        for step in [0.013, 0.21, 0.5, 1.37] {
            let moved = cascade_view(
                sun,
                &camera(view.eye + Vec3::new(step, 0.0, step * 0.5), 30.0),
                from,
                to,
                SIZE,
            );
            let shift = texels(moved) - texels(cascade);
            assert!(
                (shift.x - shift.x.round()).abs() < 1e-2
                    && (shift.y - shift.y.round()).abs() < 1e-2,
                "a {step} m move shifts the map by {shift:?} texels"
            );
        }
    }
}
