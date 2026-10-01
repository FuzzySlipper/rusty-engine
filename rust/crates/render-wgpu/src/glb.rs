//! Decoding an admitted animated-mesh GLB (`animated-mesh-resource/…`) into
//! CPU-side data: the node hierarchy with rest TRS, primitives, skins, clips
//! and materials. The runtime admitted the bytes (`asset-import`); this reads
//! them and does not re-validate.

use glam::{Mat4, Quat, Vec3};
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation, Property};

use crate::convert;
use crate::resources::{decode_jpeg, decode_png, DecodedImage};

pub struct GlbNode {
    pub name: Option<String>,
    pub parent: Option<usize>,
    pub rest: Trs,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    pub(crate) translation: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) scale: Vec3,
}

impl Trs {
    pub(crate) fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn translation(&self) -> [f32; 3] {
        convert::array(self.translation)
    }

    /// `[x, y, z, w]`.
    pub fn rotation(&self) -> [f32; 4] {
        convert::quat_array(self.rotation)
    }

    pub fn scale(&self) -> [f32; 3] {
        convert::array(self.scale)
    }
}

pub struct GlbPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Option<Vec<[f32; 2]>>,
    pub colors: Option<Vec<[f32; 4]>>,
    pub joints: Option<Vec<[u16; 4]>>,
    /// Normalized to sum 1 (a zero sum becomes `(1, 0, 0, 0)`).
    pub weights: Option<Vec<[f32; 4]>>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

pub struct GlbSkin {
    pub joints: Vec<usize>,
    pub(crate) inverse_binds: Vec<Mat4>,
}

impl GlbSkin {
    /// Column-major, one per joint.
    pub fn inverse_binds(&self) -> Vec<[[f32; 4]; 4]> {
        self.inverse_binds
            .iter()
            .copied()
            .map(convert::matrix_columns)
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Interp {
    Step,
    Linear,
    CubicSpline,
}

pub struct Channel {
    pub node: usize,
    pub path: Path,
    pub interpolation: Interp,
    pub times: Vec<f32>,
    /// Flattened values: 3 or 4 per key, times 3 for cubic splines
    /// (in-tangent, value, out-tangent).
    pub values: Vec<f32>,
}

pub struct GlbClip {
    pub name: Option<String>,
    pub duration: f32,
    pub channels: Vec<Channel>,
}

pub struct GlbMaterial {
    pub base_color: [f32; 4],
    pub base_color_texture: Option<usize>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub alpha: GlbAlpha,
    pub double_sided: bool,
    pub unlit: bool,
}

#[derive(Clone, Copy)]
pub enum GlbAlpha {
    Opaque,
    Mask(f32),
    Blend,
}

pub struct GlbTexture {
    pub image: Option<DecodedImage>,
    pub nearest: bool,
    pub repeat: bool,
}

pub struct GlbModel {
    pub nodes: Vec<GlbNode>,
    /// Node indices, parents before children.
    pub order: Vec<usize>,
    /// Mesh index to its primitives.
    pub meshes: Vec<Vec<GlbPrimitive>>,
    pub skins: Vec<GlbSkin>,
    pub clips: Vec<GlbClip>,
    pub materials: Vec<GlbMaterial>,
    pub textures: Vec<GlbTexture>,
}

pub(crate) fn decode(bytes: &[u8]) -> Result<GlbModel, String> {
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|error| format!("glb: {error}"))?;
    let blob = gltf.blob.as_deref();
    let data = |buffer: gltf::Buffer<'_>| match buffer.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    };
    let document = &gltf.document;

    let mut nodes: Vec<GlbNode> = document
        .nodes()
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            GlbNode {
                name: node.name().map(str::to_owned),
                parent: None,
                rest: Trs {
                    translation: Vec3::from(translation),
                    rotation: Quat::from_array(rotation),
                    scale: Vec3::from(scale),
                },
                mesh: node.mesh().map(|mesh| mesh.index()),
                skin: node.skin().map(|skin| skin.index()),
            }
        })
        .collect();
    for node in document.nodes() {
        for child in node.children() {
            nodes[child.index()].parent = Some(node.index());
        }
    }
    // The default scene's hierarchy, parents first.
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or("glb: no scene")?;
    let mut order = Vec::new();
    let mut stack: Vec<usize> = scene.nodes().map(|node| node.index()).collect();
    stack.reverse();
    while let Some(index) = stack.pop() {
        order.push(index);
        let node = document.nodes().nth(index).expect("node index");
        let mut children: Vec<usize> = node.children().map(|child| child.index()).collect();
        children.reverse();
        stack.extend(children);
    }

    let mut meshes = Vec::new();
    for mesh in document.meshes() {
        let mut primitives = Vec::new();
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(data);
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or("glb: primitive without positions")?
                .collect();
            let normals = reader
                .read_normals()
                .map(Iterator::collect)
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let indices = match reader.read_indices() {
                Some(indices) => indices.into_u32().collect(),
                None => (0..positions.len() as u32).collect(),
            };
            let weights = reader.read_weights(0).map(|weights| {
                weights
                    .into_f32()
                    .map(|w| {
                        let sum: f32 = w.iter().sum();
                        if sum > 0.0 {
                            w.map(|value| value / sum)
                        } else {
                            [1.0, 0.0, 0.0, 0.0]
                        }
                    })
                    .collect()
            });
            primitives.push(GlbPrimitive {
                uvs: reader
                    .read_tex_coords(0)
                    .map(|uvs| uvs.into_f32().collect()),
                colors: reader
                    .read_colors(0)
                    .map(|colors| colors.into_rgba_f32().collect()),
                joints: reader
                    .read_joints(0)
                    .map(|joints| joints.into_u16().collect()),
                weights,
                positions,
                normals,
                indices,
                material: primitive.material().index(),
            });
        }
        meshes.push(primitives);
    }

    let skins = document
        .skins()
        .map(|skin| {
            let joints: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
            let inverse_binds = match skin.reader(data).read_inverse_bind_matrices() {
                Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
                None => vec![Mat4::IDENTITY; joints.len()],
            };
            GlbSkin {
                joints,
                inverse_binds,
            }
        })
        .collect();

    let mut clips = Vec::new();
    for animation in document.animations() {
        let mut channels = Vec::new();
        let mut duration = 0.0f32;
        for channel in animation.channels() {
            let reader = channel.reader(data);
            let Some(times) = reader.read_inputs() else {
                continue;
            };
            let times: Vec<f32> = times.collect();
            let (path, values): (Path, Vec<f32>) = match reader.read_outputs() {
                Some(ReadOutputs::Translations(values)) => {
                    (Path::Translation, values.flatten().collect())
                }
                Some(ReadOutputs::Rotations(values)) => {
                    (Path::Rotation, values.into_f32().flatten().collect())
                }
                Some(ReadOutputs::Scales(values)) => (Path::Scale, values.flatten().collect()),
                // Morph target weights are not realized.
                Some(ReadOutputs::MorphTargetWeights(_)) | None => continue,
            };
            if channel.target().property() == Property::MorphTargetWeights {
                continue;
            }
            duration = duration.max(times.last().copied().unwrap_or(0.0));
            channels.push(Channel {
                node: channel.target().node().index(),
                path,
                interpolation: match channel.sampler().interpolation() {
                    Interpolation::Step => Interp::Step,
                    Interpolation::Linear => Interp::Linear,
                    Interpolation::CubicSpline => Interp::CubicSpline,
                },
                times,
                values,
            });
        }
        clips.push(GlbClip {
            name: animation.name().map(str::to_owned),
            duration,
            channels,
        });
    }

    let materials = document
        .materials()
        .map(|material| {
            let pbr = material.pbr_metallic_roughness();
            let strength = material.emissive_strength().unwrap_or(1.0);
            GlbMaterial {
                base_color: pbr.base_color_factor(),
                base_color_texture: pbr.base_color_texture().map(|info| info.texture().index()),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                emissive: material.emissive_factor().map(|value| value * strength),
                alpha: match material.alpha_mode() {
                    gltf::material::AlphaMode::Opaque => GlbAlpha::Opaque,
                    gltf::material::AlphaMode::Mask => {
                        GlbAlpha::Mask(material.alpha_cutoff().unwrap_or(0.5))
                    }
                    gltf::material::AlphaMode::Blend => GlbAlpha::Blend,
                },
                double_sided: material.double_sided(),
                unlit: material.unlit(),
            }
        })
        .collect();

    let textures = document
        .textures()
        .map(|texture| {
            let image = match texture.source().map(|image| image.source()) {
                Some(gltf::image::Source::View { view, mime_type }) => {
                    let buffer = data(view.buffer());
                    buffer
                        .and_then(|bytes| bytes.get(view.offset()..view.offset() + view.length()))
                        .and_then(|bytes| match mime_type {
                            "image/png" => decode_png(bytes).ok(),
                            "image/jpeg" => decode_jpeg(bytes).ok(),
                            _ => None,
                        })
                }
                _ => None,
            };
            let sampler = texture.sampler();
            GlbTexture {
                image,
                nearest: matches!(
                    sampler.mag_filter(),
                    Some(gltf::texture::MagFilter::Nearest)
                ),
                repeat: sampler.wrap_s() == gltf::texture::WrappingMode::Repeat,
            }
        })
        .collect();

    Ok(GlbModel {
        nodes,
        order,
        meshes,
        skins,
        clips,
        materials,
        textures,
    })
}

impl Channel {
    fn width(&self) -> usize {
        match self.path {
            Path::Rotation => 4,
            _ => 3,
        }
    }

    /// The key segment for `time` (clamped to the end keys) and the fraction
    /// into it.
    fn segment(&self, time: f32) -> (usize, usize, f32, f32) {
        let last = self.times.len() - 1;
        if time <= self.times[0] {
            return (0, 0, 0.0, 0.0);
        }
        if time >= self.times[last] {
            return (last, last, 0.0, 0.0);
        }
        let right = self.times.partition_point(|key| *key <= time);
        let left = right - 1;
        let span = self.times[right] - self.times[left];
        (left, right, (time - self.times[left]) / span, span)
    }

    fn key(&self, index: usize) -> &[f32] {
        let width = self.width();
        match self.interpolation {
            Interp::CubicSpline => &self.values[(index * 3 + 1) * width..(index * 3 + 2) * width],
            _ => &self.values[index * width..(index + 1) * width],
        }
    }

    /// Cubic Hermite with glTF tangents (scaled by the segment span).
    fn cubic(&self, left: usize, right: usize, t: f32, span: f32) -> Vec<f32> {
        let width = self.width();
        let value = |key: usize, part: usize| {
            &self.values[(key * 3 + part) * width..(key * 3 + part + 1) * width]
        };
        let (p0, m0, p1, m1) = (
            value(left, 1),
            value(left, 2),
            value(right, 1),
            value(right, 0),
        );
        let (t2, t3) = (t * t, t * t * t);
        (0..width)
            .map(|i| {
                (2.0 * t3 - 3.0 * t2 + 1.0) * p0[i]
                    + (t3 - 2.0 * t2 + t) * span * m0[i]
                    + (-2.0 * t3 + 3.0 * t2) * p1[i]
                    + (t3 - t2) * span * m1[i]
            })
            .collect()
    }

    pub(crate) fn sample_vec3(&self, time: f32) -> Vec3 {
        let (left, right, t, span) = self.segment(time);
        let a = Vec3::from_slice(self.key(left));
        if left == right {
            return a;
        }
        match self.interpolation {
            Interp::Step => a,
            Interp::Linear => a.lerp(Vec3::from_slice(self.key(right)), t),
            Interp::CubicSpline => Vec3::from_slice(&self.cubic(left, right, t, span)),
        }
    }

    pub(crate) fn sample_quat(&self, time: f32) -> Quat {
        let (left, right, t, span) = self.segment(time);
        let a = Quat::from_slice(self.key(left)).normalize();
        if left == right {
            return a;
        }
        match self.interpolation {
            Interp::Step => a,
            Interp::Linear => a.slerp(Quat::from_slice(self.key(right)).normalize(), t),
            Interp::CubicSpline => Quat::from_slice(&self.cubic(left, right, t, span)).normalize(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textured_glb(image: &[u8], mime_type: &str) -> Vec<u8> {
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[]}}],"buffers":[{{"byteLength":{}}}],"bufferViews":[{{"buffer":0,"byteLength":{}}}],"images":[{{"bufferView":0,"mimeType":"{}"}}],"textures":[{{"source":0}}],"materials":[{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":0}}}}}}]}}"#,
            image.len(),
            image.len(),
            mime_type
        );
        let mut json = json.into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut bin = image.to_vec();
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut bytes = Vec::new();
        for word in [
            0x4654_6c67,
            2,
            (28 + json.len() + bin.len()) as u32,
            json.len() as u32,
            0x4e4f_534a,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend(json);
        bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x004e_4942u32.to_le_bytes());
        bytes.extend(bin);
        bytes
    }

    #[test]
    fn embedded_jpeg_and_png_material_textures_keep_their_pixels() {
        let rgb = [240, 20, 10].repeat(64);
        let mut jpeg = Vec::new();
        jpeg_encoder::Encoder::new(&mut jpeg, 100)
            .encode(&rgb, 8, 8, jpeg_encoder::ColorType::Rgb)
            .unwrap();
        let rgba = [240, 20, 10, 255].repeat(64);
        let png = crate::resources::encode_png(8, 8, &rgba).unwrap();
        for (bytes, mime) in [(&jpeg, "image/jpeg"), (&png, "image/png")] {
            let model = decode(&textured_glb(bytes, mime)).unwrap();
            assert_eq!(model.materials[0].base_color_texture, Some(0));
            let image = model.textures[0]
                .image
                .as_ref()
                .expect("embedded image decoded");
            assert_eq!((image.width, image.height), (8, 8));
            assert_eq!(image.rgba.len(), rgba.len());
            for pixel in image.rgba.chunks_exact(4) {
                for channel in 0..3 {
                    assert!(pixel[channel].abs_diff(rgba[channel]) <= 3);
                }
                assert_eq!(pixel[3], 255);
            }
        }
    }

    fn channel(interpolation: Interp, values: Vec<f32>) -> Channel {
        Channel {
            node: 0,
            path: Path::Translation,
            interpolation,
            times: vec![0.0, 1.0, 3.0],
            values,
        }
    }

    #[test]
    fn channels_clamp_to_end_keys_and_interpolate_by_mode() {
        let values = vec![0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 2.0, 4.0, 0.0];
        let linear = channel(Interp::Linear, values.clone());
        assert_eq!(linear.sample_vec3(-1.0), Vec3::ZERO);
        assert_eq!(linear.sample_vec3(0.5), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(linear.sample_vec3(2.0), Vec3::new(2.0, 2.0, 0.0));
        assert_eq!(linear.sample_vec3(9.0), Vec3::new(2.0, 4.0, 0.0));
        let step = channel(Interp::Step, values);
        assert_eq!(step.sample_vec3(0.99), Vec3::ZERO);
        // Cubic with zero tangents passes through its keys.
        let mut cubic = vec![0.0; 27];
        cubic[12..15].copy_from_slice(&[2.0, 0.0, 0.0]);
        let cubic = channel(Interp::CubicSpline, cubic);
        assert_eq!(cubic.sample_vec3(1.0), Vec3::new(2.0, 0.0, 0.0));
        assert!((cubic.sample_vec3(0.5).x - 1.0).abs() < 1e-6);
    }
}
