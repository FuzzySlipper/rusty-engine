//! Candidate (a): a pinned Bevy 0.19.1 render cul-de-sac.
//!
//! The runtime owns the wgpu instance, adapter, device and queue and hands them
//! to `RenderPlugin` through `RenderCreation::Manual`. Bevy owns no window and
//! no loop: the runner is discarded and the host pumps `SubApps::update`. The
//! camera renders into a texture the host created (`ManualTextureViews`) and the
//! host reads it back with its own encoder. The `PresentationWorld` baseline is
//! mirrored into Bevy assets and entities, one entity per (instance, mesh group).

use std::collections::HashMap;

use bevy::{
    app::SubApps,
    asset::RenderAssetUsages,
    camera::{Exposure, ManualTextureViewHandle, RenderTarget},
    core_pipeline::tonemapping::Tonemapping,
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, Mesh, PrimitiveTopology},
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        renderer::{
            RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue,
            WgpuWrapper,
        },
        settings::{RenderCreation, RenderResources},
        texture::{ManualTextureView, ManualTextureViews},
        RenderPlugin,
    },
    window::ExitCondition,
};
use bootstrap_fixture::{camera_forward_up, neutral_light, write_png, Fixture, HEIGHT, WIDTH};
use render_model::{MeshPayloadSource, RenderDiff, TextureFilter, TextureWrap};

const TARGET_HANDLE: ManualTextureViewHandle = ManualTextureViewHandle(1);

/// The runtime's GPU: created here, shared with Bevy by clone.
struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn runtime_gpu() -> Gpu {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("adapter");
    eprintln!("adapter: {:?}", adapter.get_info().name);
    // Bevy's own initialization asks for every adapter feature and limit, and
    // opts into experimental features with `unsafe`; this host does not.
    let mut features = adapter.features();
    features.remove(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
    features.remove(wgpu::Features::all_experimental_mask());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: features,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("device");
    Gpu {
        instance,
        adapter,
        device,
        queue,
    }
}

fn bevy_app(gpu: &Gpu) -> SubApps {
    let resources = RenderResources(
        RenderDevice::from(gpu.device.clone()),
        RenderQueue(std::sync::Arc::new(WgpuWrapper::new(gpu.queue.clone()))),
        RenderAdapterInfo(WgpuWrapper::new(gpu.adapter.get_info())),
        RenderAdapter(std::sync::Arc::new(WgpuWrapper::new(gpu.adapter.clone()))),
        RenderInstance(std::sync::Arc::new(WgpuWrapper::new(gpu.instance.clone()))),
    );
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Manual(resources),
                synchronous_pipeline_compilation: true,
                ..default()
            }),
    );
    app.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        // Stand-in for the hemisphere term; Bevy has no hemisphere light.
        brightness: 0.5 * neutral_light::HEMISPHERE_INTENSITY / std::f32::consts::PI,
        affects_lightmapped_meshes: true,
    });
    app.finish();
    app.cleanup();
    std::mem::take(app.sub_apps_mut())
}

/// Mirror tables: retained ids to Bevy handles and entities.
#[derive(Default)]
struct Mirror {
    images: HashMap<String, Handle<Image>>,
    materials: HashMap<String, Handle<StandardMaterial>>,
    meshes: HashMap<String, Vec<(u16, Handle<Mesh>)>>,
    slots: HashMap<String, HashMap<u16, String>>,
    entities: Vec<Entity>,
}

impl Mirror {
    fn apply(
        &mut self,
        fixture: &Fixture,
        world: &mut World,
        ops: &[RenderDiff],
    ) -> Result<(), String> {
        for op in ops {
            match op {
                RenderDiff::DefineTexture { texture } => {
                    let pixels = fixture.texture_pixels(texture)?;
                    let mut image = Image::new(
                        Extent3d {
                            width: pixels.width,
                            height: pixels.height,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        pixels.pixels,
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::RENDER_WORLD,
                    );
                    let filter = match texture.filter {
                        TextureFilter::Nearest => ImageFilterMode::Nearest,
                        _ => ImageFilterMode::Linear,
                    };
                    let address = match texture.wrap {
                        TextureWrap::Repeat => ImageAddressMode::Repeat,
                        _ => ImageAddressMode::ClampToEdge,
                    };
                    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: address,
                        address_mode_v: address,
                        mag_filter: filter,
                        min_filter: filter,
                        ..default()
                    });
                    let handle = world.resource_mut::<Assets<Image>>().add(image);
                    self.images.insert(texture.id.clone(), handle);
                }
                RenderDiff::DefineMaterial { material } => {
                    let c = material.color;
                    let t = material.texture_tint;
                    let standard = StandardMaterial {
                        base_color: Color::linear_rgba(
                            c[0] * t[0],
                            c[1] * t[1],
                            c[2] * t[2],
                            c[3] * t[3],
                        ),
                        base_color_texture: material
                            .texture
                            .as_ref()
                            .and_then(|id| self.images.get(id))
                            .cloned(),
                        perceptual_roughness: material.roughness,
                        metallic: 0.0,
                        // Bevy's default 0.5 reflectance is F0 0.04, as Three.
                        ..default()
                    };
                    let handle = world
                        .resource_mut::<Assets<StandardMaterial>>()
                        .add(standard);
                    self.materials.insert(material.id.clone(), handle);
                }
                RenderDiff::DefineStaticMesh { asset } => {
                    let MeshPayloadSource::Inline {
                        positions,
                        normals,
                        uvs,
                        indices,
                        ..
                    } = &asset.payload.source
                    else {
                        return Err(format!(
                            "{}: only inline payloads are mirrored",
                            asset.asset
                        ));
                    };
                    let positions: Vec<[f32; 3]> = positions
                        .chunks_exact(3)
                        .map(|p| [p[0], p[1], p[2]])
                        .collect();
                    let normals: Vec<[f32; 3]> = normals
                        .chunks_exact(3)
                        .map(|n| [n[0], n[1], n[2]])
                        .collect();
                    let uvs: Vec<[f32; 2]> = match uvs {
                        Some(uvs) => uvs.chunks_exact(2).map(|uv| [uv[0], uv[1]]).collect(),
                        None => vec![[0.0, 0.0]; positions.len()],
                    };
                    let mut groups = Vec::new();
                    for group in &asset.payload.groups {
                        let range = group.start as usize..(group.start + group.count) as usize;
                        let mesh = Mesh::new(
                            PrimitiveTopology::TriangleList,
                            RenderAssetUsages::RENDER_WORLD,
                        )
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone())
                        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone())
                        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs.clone())
                        .with_inserted_indices(Indices::U32(indices[range].to_vec()));
                        let handle = world.resource_mut::<Assets<Mesh>>().add(mesh);
                        groups.push((group.material_slot, handle));
                    }
                    self.meshes.insert(asset.asset.clone(), groups);
                    self.slots.insert(
                        asset.asset.clone(),
                        asset
                            .material_slots
                            .iter()
                            .map(|slot| (slot.slot, slot.material.clone()))
                            .collect(),
                    );
                }
                RenderDiff::CreateStaticMeshInstance { instance, .. } => {
                    let Some(groups) = self.meshes.get(&instance.asset) else {
                        continue;
                    };
                    let transform = Transform {
                        translation: Vec3::from(instance.transform.translation),
                        rotation: Quat::from_array(instance.transform.rotation),
                        scale: Vec3::from(instance.transform.scale),
                    };
                    let visibility = if instance.visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                    for (slot, mesh) in groups {
                        let material = instance
                            .material_overrides
                            .iter()
                            .find(|entry| entry.slot == *slot)
                            .map(|entry| &entry.material)
                            .or_else(|| {
                                self.slots
                                    .get(&instance.asset)
                                    .and_then(|slots| slots.get(slot))
                            })
                            .and_then(|id| self.materials.get(id))
                            .cloned()
                            .unwrap_or_default();
                        let entity = world
                            .spawn((
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material),
                                transform,
                                visibility,
                            ))
                            .id();
                        self.entities.push(entity);
                    }
                }
                // Bevy's Skybox is a cubemap; the equirectangular sky would need
                // a conversion pass. Sprites are excluded for every candidate.
                _ => {}
            }
        }
        Ok(())
    }
}

struct Target {
    texture: wgpu::Texture,
    readback: wgpu::Buffer,
    padded_row: u32,
}

fn create_target(gpu: &Gpu, world: &mut World) -> Target {
    let size = wgpu::Extent3d {
        width: WIDTH,
        height: HEIGHT,
        depth_or_array_layers: 1,
    };
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host-offscreen"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    world.resource_mut::<ManualTextureViews>().insert(
        TARGET_HANDLE,
        ManualTextureView::with_default_format(view.into(), UVec2::new(WIDTH, HEIGHT)),
    );
    let padded_row = (WIDTH * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("host-readback"),
        size: u64::from(padded_row * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    Target {
        texture,
        readback,
        padded_row,
    }
}

fn read_target(gpu: &Gpu, target: &Target) -> Vec<u8> {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &target.readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(target.padded_row),
                rows_per_image: Some(HEIGHT),
            },
        },
        target.texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = target.readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| result.expect("map readback"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for row in mapped.chunks(target.padded_row as usize) {
        pixels.extend_from_slice(&row[..(WIDTH * 4) as usize]);
    }
    drop(mapped);
    target.readback.unmap();
    pixels
}

fn main() -> Result<(), String> {
    let fixture = Fixture::load(&Fixture::default_dir())?;
    let gpu = runtime_gpu();
    let mut apps = bevy_app(&gpu);
    let world = apps.main.world_mut();
    let target = create_target(&gpu, world);

    let mut mirror = Mirror::default();
    mirror.apply(&fixture, world, &fixture.baseline())?;

    let key = Vec3::from(neutral_light::KEY_POSITION);
    world.spawn((
        DirectionalLight {
            color: Color::WHITE,
            // With exposure fixed to 1.0 below, lux maps to Three's intensity.
            illuminance: neutral_light::KEY_INTENSITY,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(key).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let camera = fixture.camera;
    let (forward, up) = camera_forward_up(&camera);
    world.spawn((
        Camera3d::default(),
        RenderTarget::TextureView(TARGET_HANDLE),
        Projection::Perspective(PerspectiveProjection {
            fov: camera.fov_y_degrees.to_radians(),
            near: camera.near,
            far: camera.far,
            aspect_ratio: WIDTH as f32 / HEIGHT as f32,
            ..default()
        }),
        Transform::from_translation(Vec3::from(camera.position))
            .looking_to(Vec3::from(forward), Vec3::from(up)),
        Tonemapping::None,
        // exposure = 1 / (1.2 * 2^ev100) = 1.0
        Exposure {
            ev100: -(1.2f32).log2(),
        },
        Msaa::Off,
    ));

    // Assets are prepared a frame after they are added; pump a few ticks.
    let updates = std::env::var("BEVY_UPDATES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(4);
    for _ in 0..updates {
        apps.update();
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    }
    let pixels = read_target(&gpu, &target);
    write_png(std::path::Path::new("out/bevy.png"), WIDTH, HEIGHT, &pixels)?;
    eprintln!("bevy: {} entities mirrored", mirror.entities.len());
    Ok(())
}
