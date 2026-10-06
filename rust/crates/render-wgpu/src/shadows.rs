//! Shadow maps for lights that request them, when the host enables shadows
//! (`RendererOptions::shadows`; products opt in through their manifest).
//!
//! Each shadowed light takes layers of one depth texture array: four
//! cascades for a directional light, one layer for a spot light, six for a
//! point light. There is no light quota: the array grows with the lights that
//! ask. Every shown scene part casts and every lit part receives, so a new
//! object needs no shadow setup. Each layer draws only the casters inside its
//! view and, for a point or spot light with a range, inside that range.
//!
//! The shadow cameras: 512² maps, no bias, back faces of single-sided parts
//! rendered; spot lights over twice the cone angle; point lights over six
//! 90° faces; both with near 0.5 and far = range, or 500 without one.
//! Receivers take a 3×3 PCF sample with linear comparison filtering.
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

use glam::{Mat4, Vec3};
use render_model::{LightDescriptor, LightShadowIntent};
use wgpu::util::DeviceExt;

use crate::batch::DrawList;
use crate::camera::CameraMatrices;
use crate::target::DEPTH_FORMAT;

pub(crate) const SHADOW_MAP_SIZE: u32 = 512;
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

/// What places a shadow layer's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LayerSource {
    /// A view its light fixes: a spot light, a point light's face or an
    /// ambient light's sky, with the sphere a light with a range reaches
    /// (nothing outside it casts into the layer).
    Fixed {
        view_proj: Mat4,
        reach: Option<(Vec3, f32)>,
    },
    /// Cascade `index` of the directional light in light row `row`, fitted
    /// to each world view's camera.
    Cascade {
        direction: Vec3,
        distance: f32,
        index: u32,
        row: u32,
    },
}

impl LayerSource {
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
            let texel = 2.0 * SKY_HALF_EXTENT / SHADOW_MAP_SIZE as f32;
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
/// depths `from` and `to`, its centre snapped to whole texels, reaching
/// `SHADOW_FAR` toward the light.
pub(crate) fn cascade_view(direction: Vec3, camera: &CameraMatrices, from: f32, to: f32) -> Mat4 {
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
    let texel = 2.0 * radius / SHADOW_MAP_SIZE as f32;
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

/// One shadow layer's view and the casters drawn into it.
pub(crate) struct ShadowLayer {
    pub source: LayerSource,
    /// The source's view, or the cascade's last fit (zero before one).
    pub view_proj: Mat4,
    /// Casters inside the view and reach, offset into the instance buffer.
    pub casters: DrawList,
    /// The view or a caster changed since the layer was rendered.
    pub stale: bool,
}

/// The shadow depth array, its layer matrices, and the per-layer uniform the
/// caster pass selects its matrix with.
pub(crate) struct ShadowMaps {
    pub array_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub matrices_buffer: wgpu::Buffer,
    layer_views: Vec<wgpu::TextureView>,
    size: u32,
    pub layer_bind_group: wgpu::BindGroup,
    /// Layers in use (the texture may hold more).
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
}

impl ShadowMaps {
    pub fn new(device: &wgpu::Device, layer_layout: &wgpu::BindGroupLayout) -> Self {
        Self::with_capacity(device, layer_layout, 1, 1)
    }

    fn with_capacity(
        device: &wgpu::Device,
        layer_layout: &wgpu::BindGroupLayout,
        capacity: u32,
        size: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu shadow maps"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: capacity,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("render-wgpu shadow maps"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layer_views = (0..capacity)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("render-wgpu shadow layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let matrices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu shadow matrices"),
            size: u64::from(capacity) * 64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut indices = vec![0u8; (u64::from(capacity) * LAYER_UNIFORM_STRIDE) as usize];
        for layer in 0..capacity {
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
            matrices_buffer,
            layer_views,
            size,
            layer_bind_group,
            layers: Vec::new(),
            timed: false,
            time: 0.0,
        }
    }

    /// Hold the layers' sources, growing the array when there are more
    /// layers than it has. A layer whose source changed is stale and keeps
    /// no casters (and a cascade no fit), except a cascade whose light only
    /// turned or changed range; the others keep theirs. Returns
    /// whether the texture or buffers were replaced (the frame bind group
    /// must be rebuilt) and whether any layer changed (casters must be culled
    /// again).
    pub fn set_layers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_layout: &wgpu::BindGroupLayout,
        sources: &[LayerSource],
    ) -> (bool, bool) {
        let needed = sources.len() as u32;
        let capacity = self.layer_views.len() as u32;
        let replaced = needed > 0 && (needed > capacity || self.size != SHADOW_MAP_SIZE);
        let mut layers = std::mem::take(&mut self.layers);
        if replaced {
            *self = Self::with_capacity(
                device,
                layer_layout,
                needed.next_power_of_two(),
                SHADOW_MAP_SIZE,
            );
            // A new texture holds nothing.
            layers.clear();
        }
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
        if changed && !layers.is_empty() {
            let floats: Vec<f32> = layers
                .iter()
                .flat_map(|layer| layer.view_proj.to_cols_array())
                .collect();
            queue.write_buffer(&self.matrices_buffer, 0, bytemuck::cast_slice(&floats));
        }
        self.layers = layers;
        (replaced, changed)
    }

    /// Write one layer's view after a cascade fit.
    pub fn write_view(&self, queue: &wgpu::Queue, layer: usize) {
        queue.write_buffer(
            &self.matrices_buffer,
            layer as u64 * 64,
            bytemuck::cast_slice(&self.layers[layer].view_proj.to_cols_array()),
        );
    }

    /// Caster ids across layers.
    pub fn caster_instances(&self) -> u32 {
        self.layers
            .iter()
            .map(|layer| layer.casters.instances())
            .sum()
    }

    pub fn layer_view(&self, layer: u32) -> &wgpu::TextureView {
        &self.layer_views[layer as usize]
    }

    pub fn layer_offset(layer: u32) -> u32 {
        (u64::from(layer) * LAYER_UNIFORM_STRIDE) as u32
    }
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
        }
    }

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
        };
        assert_eq!(layers(&point), 6);
        let ambient = |shadow_intent| LightDescriptor::Ambient {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            shadow_intent,
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
        let cascade = cascade_view(sun, &view, from, to);
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
        let turned = cascade_view(sun, &camera(view.eye, 75.0), from, to);
        assert!((turned.x_axis.x - cascade.x_axis.x).abs() < 1e-6);
        let texels =
            |m: Mat4| m.project_point3(Vec3::new(4.0, 0.0, -9.0)) * (SHADOW_MAP_SIZE as f32 / 2.0);
        for step in [0.013, 0.21, 0.5, 1.37] {
            let moved = cascade_view(
                sun,
                &camera(view.eye + Vec3::new(step, 0.0, step * 0.5), 30.0),
                from,
                to,
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
