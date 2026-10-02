//! Decoding an admitted animated-mesh GLB (`animated-mesh-resource/…`) into
//! CPU-side data: the node hierarchy with rest TRS, primitives, skins, clips
//! and materials. The runtime admitted the bytes (`asset-import`); this reads
//! them and does not re-validate.

use glam::{Mat4, Quat, Vec3};
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation, Property};

use crate::convert;
use crate::resources::{decode_jpeg, decode_png, decode_webp, DecodedImage};

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
    /// `TEXCOORD_1`.
    pub uvs1: Option<Vec<[f32; 2]>>,
    /// `TANGENT`: xyz and the bitangent's handedness (w = ±1).
    pub tangents: Option<Vec<[f32; 4]>>,
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
    pub base_color_texture: Option<GlbTextureSlot>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    /// Multiplies `emissive`.
    pub emissive_texture: Option<GlbTextureSlot>,
    /// With its `scale`.
    pub normal_texture: Option<(GlbTextureSlot, f32)>,
    /// With its `strength`.
    pub occlusion_texture: Option<(GlbTextureSlot, f32)>,
    pub alpha: GlbAlpha,
    pub double_sided: bool,
    pub unlit: bool,
}

/// A material's use of one texture: which texture, the primitive's uv set it
/// reads (`texCoord`, which `KHR_texture_transform` may override), and the
/// map from that set to the texture's uv (`KHR_texture_transform`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlbTextureSlot {
    pub texture: usize,
    pub transform: UvTransform,
    pub tex_coord: u32,
}

/// An affine uv map, two rows: `u' = a·u + b·v + c`, `v' = d·u + e·v + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvTransform(pub [f32; 6]);

impl UvTransform {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);

    /// `KHR_texture_transform`: translation × rotation × scale, the rotation
    /// counter-clockwise in the image (uv's v points down).
    pub fn of(offset: [f32; 2], rotation: f32, scale: [f32; 2]) -> Self {
        let (sin, cos) = rotation.sin_cos();
        Self([
            cos * scale[0],
            sin * scale[1],
            offset[0],
            -sin * scale[0],
            cos * scale[1],
            offset[1],
        ])
    }

    /// From a texture reference's `KHR_texture_transform` extension object.
    fn from_extension(value: Option<&gltf::json::Value>) -> Self {
        let Some(value) = value else {
            return Self::IDENTITY;
        };
        let pair = |key: &str, default: [f32; 2]| {
            value
                .get(key)
                .and_then(|pair| {
                    Some([pair.get(0)?.as_f64()? as f32, pair.get(1)?.as_f64()? as f32])
                })
                .unwrap_or(default)
        };
        let rotation = value.get("rotation").and_then(gltf::json::Value::as_f64);
        Self::of(
            pair("offset", [0.0, 0.0]),
            rotation.unwrap_or(0.0) as f32,
            pair("scale", [1.0, 1.0]),
        )
    }

    pub fn apply(&self, uv: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * uv[0] + b * uv[1] + c, d * uv[0] + e * uv[1] + f]
    }
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
    /// The sampler's minFilter reads mipmaps, or (unspecified) the texture
    /// is not nearest-filtered.
    pub mipmaps: bool,
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
                uvs1: reader
                    .read_tex_coords(1)
                    .map(|uvs| uvs.into_f32().collect()),
                tangents: reader.read_tangents().map(Iterator::collect),
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

    let materials: Vec<GlbMaterial> = document
        .materials()
        .map(|material| {
            let pbr = material.pbr_metallic_roughness();
            let strength = material.emissive_strength().unwrap_or(1.0);
            let slot = |info: gltf::texture::Info<'_>| GlbTextureSlot {
                texture: info.texture().index(),
                transform: info
                    .texture_transform()
                    .map_or(UvTransform::IDENTITY, |transform| {
                        UvTransform::of(transform.offset(), transform.rotation(), transform.scale())
                    }),
                tex_coord: info
                    .texture_transform()
                    .and_then(|transform| transform.tex_coord())
                    .unwrap_or(info.tex_coord()),
            };
            // Normal and occlusion references expose KHR_texture_transform
            // only as raw JSON.
            let ext_tex_coord = |value: Option<&gltf::json::Value>, own: u32| {
                value
                    .and_then(|value| value.get("texCoord"))
                    .and_then(gltf::json::Value::as_u64)
                    .map_or(own, |set| set as u32)
            };
            GlbMaterial {
                base_color: pbr.base_color_factor(),
                base_color_texture: pbr.base_color_texture().map(slot),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                emissive: material.emissive_factor().map(|value| value * strength),
                emissive_texture: material.emissive_texture().map(slot),
                normal_texture: material.normal_texture().map(|normal| {
                    let extension = normal.extension_value("KHR_texture_transform");
                    (
                        GlbTextureSlot {
                            texture: normal.texture().index(),
                            transform: UvTransform::from_extension(extension),
                            tex_coord: ext_tex_coord(extension, normal.tex_coord()),
                        },
                        normal.scale(),
                    )
                }),
                occlusion_texture: material.occlusion_texture().map(|occlusion| {
                    let extension = occlusion.extension_value("KHR_texture_transform");
                    (
                        GlbTextureSlot {
                            texture: occlusion.texture().index(),
                            transform: UvTransform::from_extension(extension),
                            tex_coord: ext_tex_coord(extension, occlusion.tex_coord()),
                        },
                        occlusion.strength(),
                    )
                }),
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
            // EXT_texture_webp names its own image, preferred over the core
            // source (a fallback, or absent) as the extension specifies.
            let webp = texture
                .extensions()
                .and_then(|extensions| extensions.get("EXT_texture_webp"))
                .and_then(|extension| extension.get("source"))
                .and_then(|source| source.as_u64())
                .and_then(|index| document.images().nth(index as usize));
            let image = webp
                .or_else(|| texture.source())
                .and_then(|image| match image.source() {
                    gltf::image::Source::View { view, mime_type } => {
                        let bytes = data(view.buffer())?
                            .get(view.offset()..view.offset() + view.length())?;
                        match mime_type {
                            "image/png" => decode_png(bytes).ok(),
                            "image/jpeg" => decode_jpeg(bytes).ok(),
                            "image/webp" => decode_webp(bytes).ok(),
                            _ => None,
                        }
                    }
                    gltf::image::Source::Uri { .. } => None,
                });
            let sampler = texture.sampler();
            let nearest = matches!(
                sampler.mag_filter(),
                Some(gltf::texture::MagFilter::Nearest)
            );
            use gltf::texture::MinFilter;
            GlbTexture {
                image,
                nearest,
                repeat: sampler.wrap_s() == gltf::texture::WrappingMode::Repeat,
                mipmaps: match sampler.min_filter() {
                    None => !nearest,
                    Some(MinFilter::Nearest | MinFilter::Linear) => false,
                    Some(_) => true,
                },
            }
        })
        .collect();

    // A normal-mapped primitive without TANGENT gets MikkTSpace tangents
    // over the normal map's uv set, the space exporters bake normal maps in.
    for primitive in meshes.iter_mut().flatten() {
        let normal_set = primitive
            .material
            .and_then(|material| materials.get(material))
            .and_then(|material| material.normal_texture)
            .map(|(slot, _)| slot.tex_coord);
        if let (Some(set), None) = (normal_set, &primitive.tangents) {
            primitive.tangents = generated_tangents(primitive, set);
        }
    }

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

/// MikkTSpace tangents over uv set `set`, or None without that set (or for
/// geometry MikkTSpace cannot process). A vertex the algorithm leaves out
/// keeps +X.
fn generated_tangents(primitive: &GlbPrimitive, set: u32) -> Option<Vec<[f32; 4]>> {
    struct Faces<'a> {
        primitive: &'a GlbPrimitive,
        uvs: &'a [[f32; 2]],
        tangents: Vec<[f32; 4]>,
    }
    impl Faces<'_> {
        fn vertex(&self, face: usize, corner: usize) -> usize {
            self.primitive.indices[face * 3 + corner] as usize
        }
    }
    impl bevy_mikktspace::Geometry for Faces<'_> {
        fn num_faces(&self) -> usize {
            self.primitive.indices.len() / 3
        }
        fn num_vertices_of_face(&self, _face: usize) -> usize {
            3
        }
        fn position(&self, face: usize, corner: usize) -> [f32; 3] {
            self.primitive.positions[self.vertex(face, corner)]
        }
        fn normal(&self, face: usize, corner: usize) -> [f32; 3] {
            self.primitive.normals[self.vertex(face, corner)]
        }
        // MikkTSpace takes v upward (Blender's convention, where glTF
        // exporters compute their tangents); glTF's v points down the image.
        fn tex_coord(&self, face: usize, corner: usize) -> [f32; 2] {
            let [u, v] = self.uvs[self.vertex(face, corner)];
            [u, 1.0 - v]
        }
        fn set_tangent(
            &mut self,
            tangent: Option<bevy_mikktspace::TangentSpace>,
            face: usize,
            corner: usize,
        ) {
            if let Some(tangent) = tangent {
                let vertex = self.vertex(face, corner);
                self.tangents[vertex] = tangent.tangent_encoded();
            }
        }
    }
    let uvs = match set {
        0 => primitive.uvs.as_ref(),
        1 => primitive.uvs1.as_ref(),
        _ => None,
    }?;
    if uvs.len() != primitive.positions.len()
        || primitive
            .indices
            .iter()
            .any(|&index| index as usize >= uvs.len())
    {
        return None;
    }
    let mut faces = Faces {
        primitive,
        uvs,
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; primitive.positions.len()],
    };
    bevy_mikktspace::generate_tangents(&mut faces).ok()?;
    Some(faces.tangents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textured_glb(image: &[u8], mime_type: &str) -> Vec<u8> {
        glb_with_texture(image, mime_type, r#"{"source":0}"#)
    }

    /// One embedded image and one texture (`texture`, as JSON) on a material.
    fn glb_with_texture(image: &[u8], mime_type: &str, texture: &str) -> Vec<u8> {
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"extensionsUsed":["EXT_texture_webp"],"scene":0,"scenes":[{{"nodes":[]}}],"buffers":[{{"byteLength":{}}}],"bufferViews":[{{"buffer":0,"byteLength":{}}}],"images":[{{"bufferView":0,"mimeType":"{}"}}],"textures":[{}],"materials":[{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":0}}}}}}]}}"#,
            image.len(),
            image.len(),
            mime_type,
            texture
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

    /// A quad facing +Z with u along +X and v down the image (glTF): the
    /// standard layout has tangent +X and handedness +1 (bitangent
    /// cross(N, T) = +Y, up the image, as glTF normal maps expect); with u
    /// mirrored the tangent follows -X and the handedness flips.
    #[test]
    fn generated_tangents_follow_gltf_uv_conventions() {
        let quad = |u: [f32; 4]| GlbPrimitive {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: Some(vec![[u[0], 1.0], [u[1], 1.0], [u[2], 0.0], [u[3], 0.0]]),
            uvs1: None,
            tangents: None,
            colors: None,
            joints: None,
            weights: None,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: None,
        };
        for tangent in generated_tangents(&quad([0.0, 1.0, 1.0, 0.0]), 0).unwrap() {
            assert!(
                (tangent[0] - 1.0).abs() < 1e-5 && tangent[3] == 1.0,
                "{tangent:?}"
            );
        }
        for tangent in generated_tangents(&quad([1.0, 0.0, 0.0, 1.0]), 0).unwrap() {
            assert!(
                (tangent[0] + 1.0).abs() < 1e-5 && tangent[3] == -1.0,
                "{tangent:?}"
            );
        }
        assert!(generated_tangents(&quad([0.0, 1.0, 1.0, 0.0]), 1).is_none());
    }

    #[test]
    fn webp_textures_decode_from_the_extension_source_or_a_webp_core_source() {
        let rgba: Vec<u8> = (0..64u8)
            .flat_map(|index| [index * 4, 255 - index * 4, 90, 255 - index])
            .collect();
        let mut webp = Vec::new();
        image_webp::WebPEncoder::new(&mut webp)
            .encode(&rgba, 8, 8, image_webp::ColorType::Rgba8)
            .unwrap();
        for texture in [
            // The extension's own source, with no core source.
            r#"{"extensions":{"EXT_texture_webp":{"source":0}}}"#,
            // A core source whose image is WebP.
            r#"{"source":0}"#,
        ] {
            let model = decode(&glb_with_texture(&webp, "image/webp", texture)).unwrap();
            let image = model.textures[0]
                .image
                .as_ref()
                .unwrap_or_else(|| panic!("{texture}: webp image decoded"));
            assert_eq!((image.width, image.height), (8, 8));
            assert_eq!(image.rgba, rgba, "{texture}: lossless pixels");
        }
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
            assert_eq!(
                model.materials[0].base_color_texture,
                Some(GlbTextureSlot {
                    texture: 0,
                    transform: UvTransform::IDENTITY,
                    tex_coord: 0,
                })
            );
            let image = model.textures[0]
                .image
                .as_ref()
                .expect("embedded image decoded");
            assert_eq!((image.width, image.height), (8, 8));
            assert_eq!(image.rgba.len(), rgba.len());
            for pixel in image.rgba.as_chunks::<4>().0 {
                for channel in 0..3 {
                    assert!(pixel[channel].abs_diff(rgba[channel]) <= 3);
                }
                assert_eq!(pixel[3], 255);
            }
        }
    }

    #[test]
    fn texture_transforms_scale_then_rotate_counter_clockwise_then_offset() {
        let transform = UvTransform::of([0.5, 0.25], std::f32::consts::FRAC_PI_2, [2.0, 3.0]);
        // +u, scaled to 2, turns counter-clockwise in the image: up, -v.
        let [u, v] = transform.apply([1.0, 0.0]);
        assert!(
            (u - 0.5).abs() < 1e-6 && (v - (0.25 - 2.0)).abs() < 1e-6,
            "{u} {v}"
        );
        // +v (down the image), scaled to 3, turns to +u.
        let [u, v] = transform.apply([0.0, 1.0]);
        assert!((u - 3.5).abs() < 1e-6 && (v - 0.25).abs() < 1e-6, "{u} {v}");
        assert_eq!(
            UvTransform::of([0.0, 0.0], 0.0, [1.0, 1.0]),
            UvTransform::IDENTITY
        );
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
