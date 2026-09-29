//! Shadow maps for lights that request them, when the host enables shadows
//! (`RendererOptions::shadows`, Three's `lighting.shadows.enabled`; C#
//! products do not enable it).
//!
//! Each shadowed light takes layers of one depth texture array: one for a
//! directional or spot light, six for a point light. There is no light quota:
//! the array grows with the lights that ask. Every shown scene part casts and
//! every lit part receives, so a new object needs no shadow setup and nothing
//! sweeps the scene; the Three lane had to flag each new mesh.
//!
//! The shadow cameras are Three's defaults, which the Three lane never
//! changed: 512² maps, no bias, back faces of single-sided parts rendered;
//! directional lights cast from their object position (`(0, 1, 0)` in the
//! light node's frame) over a ±5 orthographic box with near 0.5 and far 500;
//! spot lights over twice the cone angle; point lights over six 90° faces;
//! both with near 0.5 and far = range, or 500 without one. Receivers take a
//! 3×3 PCF sample with linear comparison filtering.
//!
//! Maps are re-rendered only when a light or a part changed.

use glam::{Mat4, Vec3};
use render_model::{LightDescriptor, LightShadowIntent};
use wgpu::util::DeviceExt;

use crate::target::DEPTH_FORMAT;

pub(crate) const SHADOW_MAP_SIZE: u32 = 512;
const SHADOW_NEAR: f32 = 0.5;
const SHADOW_FAR: f32 = 500.0;
const DIRECTIONAL_HALF_EXTENT: f32 = 5.0;
/// Three places a directional light at `Object3D.DEFAULT_UP`.
const DIRECTIONAL_POSITION: Vec3 = Vec3::Y;
/// Dynamic uniform offsets must be 256-byte aligned.
const LAYER_UNIFORM_STRIDE: u64 = 256;

/// The view-projection of each shadow layer a light casts through; empty for
/// a light that casts none.
pub(crate) fn light_views(light: &LightDescriptor, world: &Mat4) -> Vec<Mat4> {
    if light.shadow_intent() != LightShadowIntent::Requested {
        return Vec::new();
    }
    let far = |range: &Option<f32>| range.unwrap_or(SHADOW_FAR);
    match light {
        LightDescriptor::Ambient { .. }
        | LightDescriptor::Directional { enabled: false, .. }
        | LightDescriptor::Point { enabled: false, .. }
        | LightDescriptor::Spot { enabled: false, .. } => Vec::new(),
        LightDescriptor::Directional { direction, .. } => {
            let position = world.transform_point3(DIRECTIONAL_POSITION);
            let target = world.transform_point3(
                DIRECTIONAL_POSITION + crate::convert::vec3(*direction).normalize(),
            );
            let projection = Mat4::orthographic_rh(
                -DIRECTIONAL_HALF_EXTENT,
                DIRECTIONAL_HALF_EXTENT,
                -DIRECTIONAL_HALF_EXTENT,
                DIRECTIONAL_HALF_EXTENT,
                SHADOW_NEAR,
                SHADOW_FAR,
            );
            vec![projection * look_at(position, target)]
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
            vec![projection * look_at(eye, target)]
        }
        LightDescriptor::Point {
            position, range, ..
        } => {
            let eye = world.transform_point3(crate::convert::vec3(*position));
            let projection =
                Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, SHADOW_NEAR, far(range));
            CUBE_FACES
                .iter()
                .map(|(forward, up)| projection * Mat4::look_to_rh(eye, *forward, *up))
                .collect()
        }
    }
}

/// Point light faces in the order `world.wgsl`'s `point_face` selects them.
const CUBE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),
    (Vec3::NEG_X, Vec3::NEG_Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::NEG_Y),
    (Vec3::NEG_Z, Vec3::NEG_Y),
];

/// Three's `Object3D.lookAt` for cameras, with its up vector (+Y), falling
/// back to +Z when the view is vertical.
fn look_at(eye: Vec3, target: Vec3) -> Mat4 {
    let forward = (target - eye).normalize_or(Vec3::NEG_Z);
    let up = if forward.y.abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Mat4::look_to_rh(eye, forward, up)
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
    pub layers: u32,
    /// A light or caster changed since the maps were rendered.
    pub stale: bool,
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
            layers: 0,
            stale: true,
        }
    }

    /// Hold `matrices`, growing the array when there are more layers than it
    /// has. Returns true when the texture or buffers were replaced (the frame
    /// bind group must be rebuilt).
    pub fn set_layers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_layout: &wgpu::BindGroupLayout,
        matrices: &[Mat4],
    ) -> bool {
        let needed = matrices.len() as u32;
        let capacity = self.layer_views.len() as u32;
        let replaced = needed > 0 && (needed > capacity || self.size != SHADOW_MAP_SIZE);
        if replaced {
            *self = Self::with_capacity(
                device,
                layer_layout,
                needed.next_power_of_two(),
                SHADOW_MAP_SIZE,
            );
        }
        if !matrices.is_empty() {
            let floats: Vec<f32> = matrices.iter().flat_map(Mat4::to_cols_array).collect();
            queue.write_buffer(&self.matrices_buffer, 0, bytemuck::cast_slice(&floats));
        }
        self.layers = needed;
        self.stale = true;
        replaced
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
            shadow_intent: intent,
        }
    }

    #[test]
    fn only_requested_non_ambient_lights_cast() {
        assert!(light_views(
            &directional([0.0, -1.0, 0.0], LightShadowIntent::Disabled),
            &Mat4::IDENTITY
        )
        .is_empty());
        let point = LightDescriptor::Point {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            position: [0.0, 2.0, 0.0],
            range: Some(8.0),
            decay: 2.0,
            shadow_intent: LightShadowIntent::Requested,
        };
        assert_eq!(light_views(&point, &Mat4::IDENTITY).len(), 6);
    }

    #[test]
    fn directional_box_is_centred_on_the_light_object_position() {
        let views = light_views(
            &directional([0.0, -1.0, 0.0], LightShadowIntent::Requested),
            &Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        );
        // (10, 1, 0) is the light object; a point 3 below it projects to the
        // centre, and one 6 to the side falls outside the ±5 box.
        let centre = views[0].project_point3(Vec3::new(10.0, -2.0, 0.0));
        assert!(centre.x.abs() < 1e-5 && centre.y.abs() < 1e-5);
        assert!((0.0..=1.0).contains(&centre.z));
        let outside = views[0].project_point3(Vec3::new(16.0, -2.0, 0.0));
        assert!(outside.x.abs() > 1.0 || outside.y.abs() > 1.0);
    }
}
