//! Candidate (b): kiss3d 0.46 driven headless on the runtime's device.
//!
//! The runtime creates the wgpu instance/adapter/device/queue and installs them
//! with `Context::init` (kiss3d's thread-local global) before the offscreen
//! surface exists. The `PresentationWorld` baseline is mirrored into kiss3d
//! scene nodes, one node per (instance, mesh group).

use std::collections::HashMap;
use std::sync::Arc;

use bootstrap_fixture::{camera_forward_up, neutral_light, write_png, Fixture, HEIGHT, WIDTH};
use kiss3d::prelude::*;
use render_model::{MeshPayloadSource, RenderDiff, RenderMaterialDescriptor};

/// Mirror tables: retained ids to kiss3d resources.
#[derive(Default)]
struct Mirror {
    textures: HashMap<String, Arc<Texture>>,
    materials: HashMap<String, RenderMaterialDescriptor>,
    /// Per mesh asset: one kiss3d mesh per group, with its material slot.
    meshes: HashMap<String, Vec<(u16, Rc<RefCell<GpuMesh3d>>)>>,
    slots: HashMap<String, HashMap<u16, String>>,
    nodes: Vec<SceneNode3d>,
    sky: Option<String>,
}

impl Mirror {
    fn apply(
        &mut self,
        fixture: &Fixture,
        scene: &mut SceneNode3d,
        ops: &[RenderDiff],
    ) -> Result<(), String> {
        for op in ops {
            match op {
                RenderDiff::DefineTexture { texture } => {
                    let image = fixture.texture_pixels(texture)?;
                    let rgba = image::RgbaImage::from_raw(image.width, image.height, image.pixels)
                        .ok_or("texture size mismatch")?;
                    // Only the sRGB path wraps with Repeat; it filters linearly.
                    let uploaded = TextureManager::get_global_manager(|manager| {
                        manager.add_image_with_color_space(
                            image::DynamicImage::ImageRgba8(rgba.clone()),
                            &texture.id,
                            true,
                        )
                    });
                    self.textures.insert(texture.id.clone(), uploaded);
                }
                RenderDiff::DefineMaterial { material } => {
                    self.materials.insert(material.id.clone(), material.clone());
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
                    let coords: Vec<Vec3> =
                        positions.chunks_exact(3).map(Vec3::from_slice).collect();
                    let normals: Vec<Vec3> =
                        normals.chunks_exact(3).map(Vec3::from_slice).collect();
                    let uvs: Option<Vec<Vec2>> = uvs
                        .as_ref()
                        .map(|uvs| uvs.chunks_exact(2).map(Vec2::from_slice).collect());
                    let groups = asset
                        .payload
                        .groups
                        .iter()
                        .map(|group| {
                            let range = group.start as usize..(group.start + group.count) as usize;
                            let faces = indices[range]
                                .chunks_exact(3)
                                .map(|face| [face[0], face[1], face[2]])
                                .collect();
                            let mesh = GpuMesh3d::new(
                                coords.clone(),
                                faces,
                                Some(normals.clone()),
                                uvs.clone(),
                                false,
                            );
                            (group.material_slot, Rc::new(RefCell::new(mesh)))
                        })
                        .collect();
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
                    let transform = &instance.transform;
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
                            .and_then(|id| self.materials.get(id));
                        let mut node = scene.add_mesh(mesh.clone(), Vec3::from(transform.scale));
                        node.set_position(Vec3::from(transform.translation));
                        node.set_rotation(Quat::from_array(transform.rotation));
                        node.set_visible(instance.visible);
                        node.set_metallic(0.0);
                        if let Some(material) = material {
                            let c = material.color;
                            let t = material.texture_tint;
                            node.set_color(Color::new(
                                c[0] * t[0],
                                c[1] * t[1],
                                c[2] * t[2],
                                c[3] * t[3],
                            ));
                            node.set_roughness(material.roughness);
                            if let Some(texture) = material
                                .texture
                                .as_ref()
                                .and_then(|id| self.textures.get(id))
                            {
                                node.set_texture(texture.clone());
                            }
                        }
                        self.nodes.push(node);
                    }
                }
                RenderDiff::SetSkyBackground { background } => {
                    self.sky = background.as_ref().map(|sky| sky.texture.clone());
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn runtime_gpu() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("adapter");
    eprintln!("adapter: {:?}", adapter.get_info().name);
    // kiss3d's shadow-mapped material needs more bind groups and storage
    // buffers than the defaults.
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("device");
    Context::init(
        instance,
        device,
        queue,
        adapter,
        wgpu::TextureFormat::Rgba8Unorm,
    );
}

fn main() -> Result<(), String> {
    let fixture = Fixture::load(&Fixture::default_dir())?;
    runtime_gpu();
    let mut surface = pollster::block_on(OffscreenSurface::new(WIDTH, HEIGHT));
    surface.set_background_color(BLACK);
    surface.set_tonemap(kiss3d::post_processing::Tonemap::None);
    // No hemisphere light exists; a flat ambient term stands in for it.
    surface.set_ambient(0.5 * neutral_light::HEMISPHERE_INTENSITY / std::f32::consts::PI);

    let mut scene = SceneNode3d::empty();
    let key = neutral_light::KEY_POSITION;
    scene.add_light(
        Light::directional(-Vec3::from(key).normalize())
            .with_intensity(neutral_light::KEY_INTENSITY)
            // The Three lane renders this rig without shadows; the room's
            // ceiling would otherwise occlude the key light entirely.
            .with_casts_shadows(false),
    );
    let mut mirror = Mirror::default();
    mirror.apply(&fixture, &mut scene, &fixture.baseline())?;
    // A kiss3d skybox also switches on image-based lighting from that image,
    // which the Three lane does not do; set KISS3D_SKY=1 to see its effect.
    if let Some(sky) = mirror
        .sky
        .as_ref()
        .filter(|_| std::env::var_os("KISS3D_SKY").is_some())
    {
        let path = fixture_resource_png(&fixture, sky)?;
        surface.window_mut().set_skybox_from_memory(&path);
    }

    let camera_pose = fixture.camera;
    let (forward, _) = camera_forward_up(&camera_pose);
    let eye = Vec3::from(camera_pose.position);
    let mut camera = FirstPersonCamera3d::new_with_frustum(
        camera_pose.fov_y_degrees.to_radians(),
        camera_pose.near,
        camera_pose.far,
        eye,
        eye + Vec3::from(forward),
    );
    let image = pollster::block_on(surface.render_image_3d(&mut scene, &mut camera));
    let rgba: Vec<u8> = image
        .pixels()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    write_png(std::path::Path::new("out/kiss3d.png"), WIDTH, HEIGHT, &rgba)?;
    eprintln!("kiss3d: {} nodes mirrored", mirror.nodes.len());
    Ok(())
}

/// The sky setter takes encoded bytes; re-encode the decoded texture.
fn fixture_resource_png(fixture: &Fixture, texture_id: &str) -> Result<Vec<u8>, String> {
    let snapshot = fixture.baseline();
    let texture = snapshot
        .iter()
        .find_map(|op| match op {
            RenderDiff::DefineTexture { texture } if texture.id == texture_id => {
                Some(texture.clone())
            }
            _ => None,
        })
        .ok_or_else(|| format!("sky texture {texture_id} not defined"))?;
    let image = fixture.texture_pixels(&texture)?;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &image.pixels,
            image.width,
            image.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

use image::ImageEncoder;
