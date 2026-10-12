//! The star field in the sky (`stars.wgsl`, `SetStars`): the catalogue's
//! stars as an instance buffer, loaded when the field names a new catalogue
//! resource, and the field's parameters for each view. The background pass
//! draws the band and then the stars over the panorama, before the sun, the
//! moon and the clouds.

use render_model::{unpack_star_catalogue, StarsDescriptor};

use crate::gpu::Gpu;
use crate::resources::ResourceSource;
use crate::tables::GpuTexture;

/// Bytes of one star in the instance buffer: direction, magnitude, colour.
pub(crate) const STAR_BYTES: u64 = 7 * 4;
/// Bytes of `stars.wgsl` `StarParams`.
const PARAMS_BYTES: u64 = 48;

pub(crate) struct StarField {
    /// The loaded catalogue: its resource identity, stars and count.
    catalogue: Option<(String, wgpu::Buffer, u32)>,
    params: wgpu::Buffer,
    /// A 1×1 black texture for a field without a band.
    no_band: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl StarField {
    pub fn new(gpu: &Gpu) -> Self {
        use wgpu::util::DeviceExt;
        let no_band = gpu
            .device
            .create_texture_with_data(
                &gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some("render-wgpu no star band"),
                    size: crate::target::extent(1, 1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[0, 0, 0, 255],
            )
            .create_view(&Default::default());
        Self {
            catalogue: None,
            params: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu stars"),
                size: PARAMS_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            no_band,
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu star band"),
                address_mode_u: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    /// Load `field`'s catalogue from `resources` unless it is loaded.
    pub fn load(
        &mut self,
        gpu: &Gpu,
        field: &StarsDescriptor,
        resources: &dyn ResourceSource,
    ) -> Result<(), String> {
        if self
            .catalogue
            .as_ref()
            .is_some_and(|(identity, _, _)| *identity == field.catalogue)
        {
            return Ok(());
        }
        let bytes = resources
            .bytes(&field.catalogue)
            .ok_or_else(|| format!("star catalogue `{}` is not a resource", field.catalogue))?;
        let stars = unpack_star_catalogue(&bytes)
            .map_err(|error| format!("star catalogue `{}`: {error:?}", field.catalogue))?;
        let values: Vec<f32> = stars
            .iter()
            .flat_map(|star| {
                let [x, y, z] = star.direction;
                let [r, g, b] = star.color;
                [x, y, z, star.magnitude, r, g, b]
            })
            .collect();
        use wgpu::util::DeviceExt;
        let buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu star catalogue"),
                contents: bytemuck::cast_slice(&values),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.catalogue = Some((field.catalogue.clone(), buffer, stars.len() as u32));
        Ok(())
    }

    /// The loaded catalogue's stars and count, when it is `field`'s.
    pub fn stars(&self, field: &StarsDescriptor) -> Option<(&wgpu::Buffer, u32)> {
        self.catalogue
            .as_ref()
            .filter(|(identity, _, _)| *identity == field.catalogue)
            .map(|(_, buffer, count)| (buffer, *count))
    }

    /// Write `field`'s parameters for a view `viewport` pixels across, and
    /// bind them with its band (`band`, or none).
    pub fn bind(
        &self,
        gpu: &Gpu,
        layout: &wgpu::BindGroupLayout,
        field: &StarsDescriptor,
        band: Option<&GpuTexture>,
        viewport: [u32; 2],
    ) -> wgpu::BindGroup {
        let [x, y, z, w] = field.orientation;
        let length = (x * x + y * y + z * z + w * w).sqrt();
        let values = [
            x / length,
            y / length,
            z / length,
            w / length,
            field.limiting_magnitude,
            field.brightness,
            if band.is_some() {
                field.band_strength
            } else {
                0.0
            },
            field.twinkle,
            viewport[0].max(1) as f32,
            viewport[1].max(1) as f32,
            0.0,
            0.0,
        ];
        gpu.queue
            .write_buffer(&self.params, 0, bytemuck::cast_slice(&values));
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu stars"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        band.map_or(&self.no_band, |band| &band.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}
