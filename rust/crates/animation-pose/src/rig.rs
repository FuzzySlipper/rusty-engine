//! A GLB's node hierarchy and clips, as pose evaluation reads them, and the
//! clips an animated mesh asset plays by descriptor id.

use std::collections::HashMap;

use glam::{Mat4, Quat, Vec3};
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation, Property};
use render_model::{AnimatedMeshAsset, PosedJoint, Transform};

use crate::{Channel, Clip, Hierarchy, Interp, Path, Trs};

/// One GLB node as pose evaluation reads it.
#[derive(Clone, Debug)]
pub struct RigNode {
    pub name: Option<String>,
    pub parent: Option<usize>,
    pub rest: Trs,
}

/// A document's nodes with their rest transforms and parents, and the order
/// that visits parents before children (the default scene's nodes only).
pub fn decode_nodes(document: &gltf::Document) -> Result<(Vec<RigNode>, Vec<usize>), String> {
    let mut nodes: Vec<RigNode> = document
        .nodes()
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            RigNode {
                name: node.name().map(str::to_owned),
                parent: None,
                rest: Trs {
                    translation: Vec3::from(translation),
                    rotation: Quat::from_array(rotation),
                    scale: Vec3::from(scale),
                },
            }
        })
        .collect();
    for node in document.nodes() {
        for child in node.children() {
            nodes[child.index()].parent = Some(node.index());
        }
    }
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
    Ok((nodes, order))
}

/// A document's clips, in document order. Morph target weights are not
/// realized.
pub fn decode_clips<'a>(
    document: &'a gltf::Document,
    data: impl Clone + Fn(gltf::Buffer<'a>) -> Option<&'a [u8]>,
) -> Vec<Clip> {
    let mut clips = Vec::new();
    for animation in document.animations() {
        let mut channels = Vec::new();
        let mut duration = 0.0f32;
        for channel in animation.channels() {
            let reader = channel.reader(data.clone());
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
        clips.push(Clip {
            name: animation.name().map(str::to_owned),
            duration,
            channels,
        });
    }
    clips
}

/// A GLB's hierarchy, skin joints and clips, without its meshes.
#[derive(Clone, Debug)]
pub struct Rig {
    pub nodes: Vec<RigNode>,
    pub order: Vec<usize>,
    /// Each skin's joint nodes.
    pub skins: Vec<Vec<usize>>,
    pub clips: Vec<Clip>,
}

impl Rig {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let gltf = gltf::Gltf::from_slice(bytes).map_err(|error| format!("glb: {error}"))?;
        let blob = gltf.blob.as_deref();
        let data = |buffer: gltf::Buffer<'_>| match buffer.source() {
            gltf::buffer::Source::Bin => blob,
            gltf::buffer::Source::Uri(_) => None,
        };
        let document = &gltf.document;
        let (nodes, order) = decode_nodes(document)?;
        let skins = document
            .skins()
            .map(|skin| skin.joints().map(|joint| joint.index()).collect())
            .collect();
        let clips = decode_clips(document, data);
        Ok(Self {
            nodes,
            order,
            skins,
            clips,
        })
    }
}

impl Hierarchy for Rig {
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn parent(&self, node: usize) -> Option<usize> {
        self.nodes[node].parent
    }

    fn rest(&self, node: usize) -> Trs {
        self.nodes[node].rest
    }

    fn name(&self, node: usize) -> Option<&str> {
        self.nodes[node].name.as_deref()
    }

    fn order(&self) -> &[usize] {
        &self.order
    }

    fn skin_joints(&self) -> Vec<usize> {
        self.skins.iter().flatten().copied().collect()
    }
}

/// The skin joints retained children attach to, by name: a name shared by
/// two different joints names neither. Nodes that are not skin joints never
/// take a joint's name. Rendering and GLB export attach by this rule.
pub fn joint_nodes<H: Hierarchy + ?Sized>(model: &H) -> HashMap<String, usize> {
    let mut joint_counts: HashMap<String, Vec<usize>> = HashMap::new();
    for joint in model.skin_joints() {
        if let Some(name) = model.name(joint) {
            let entry = joint_counts.entry(name.to_owned()).or_default();
            if !entry.contains(&joint) {
                entry.push(joint);
            }
        }
    }
    joint_counts
        .into_iter()
        .filter_map(|(name, nodes)| (nodes.len() == 1).then(|| (name, nodes[0])))
        .collect()
}

/// Where a clip pack's channels bind in the primary model, by node name.
/// `asset-import` approved the pack against the primary's rig, whose joint
/// identities are skin joint node names (unique among joints, never
/// synthesized). A skin joint therefore takes its name over any other node
/// that shares it; other named nodes bind by name as well.
fn channel_targets<H: Hierarchy + ?Sized>(model: &H) -> HashMap<&str, usize> {
    let mut targets: HashMap<&str, usize> = (0..model.node_count())
        .filter_map(|index| model.name(index).map(|name| (name, index)))
        .collect();
    for joint in model.skin_joints() {
        if let Some(name) = model.name(joint) {
            targets.insert(name, joint);
        }
    }
    targets
}

/// Resource identity of an admitted GLB (`sha256:<hex>` names `<kind>/<hex>`).
pub fn resource_identity(kind: &str, content_hash: Option<&str>) -> Option<String> {
    let hash = content_hash?;
    Some(format!(
        "{kind}/{}",
        hash.strip_prefix("sha256:").unwrap_or(hash)
    ))
}

/// The clips `asset` plays, by descriptor id: `primary`'s own clips (from
/// `own`, in document order) by name, then its clip packs', whose channels
/// bind to the primary by node name. `pack_bytes` gives a clip pack's GLB by
/// resource identity (`clip-pack-resource/<hex>`).
pub fn asset_clips<H: Hierarchy + ?Sized, B: AsRef<[u8]>>(
    asset: &AnimatedMeshAsset,
    primary: &H,
    own: Vec<Clip>,
    mut pack_bytes: impl FnMut(&str) -> Option<B>,
) -> Result<HashMap<String, Clip>, String> {
    let names = |clips: &[Clip]| -> HashMap<String, usize> {
        clips
            .iter()
            .enumerate()
            .filter_map(|(index, clip)| clip.name.clone().map(|name| (name, index)))
            .collect()
    };
    let mut clips = HashMap::new();
    let own_names = names(&own);
    let mut own: Vec<Option<Clip>> = own.into_iter().map(Some).collect();
    for descriptor in &asset.clips {
        let name = descriptor.name.as_deref().unwrap_or(&descriptor.id);
        if let Some(clip) = own_names.get(name).and_then(|index| own[*index].take()) {
            clips.insert(descriptor.id.clone(), clip);
        }
    }
    let by_name = channel_targets(primary);
    for pack in &asset.clip_packs {
        let identity = resource_identity("clip-pack-resource", Some(pack.content_hash.as_str()))
            .ok_or_else(|| format!("clip pack {} has no content hash", pack.asset))?;
        let bytes =
            pack_bytes(&identity).ok_or_else(|| format!("resource {identity} is not available"))?;
        let pack_rig = Rig::decode(bytes.as_ref())?;
        let pack_names = names(&pack_rig.clips);
        let mut pack_clips: Vec<Option<Clip>> = pack_rig.clips.iter().cloned().map(Some).collect();
        for descriptor in &pack.clips {
            let name = descriptor.name.as_deref().unwrap_or(&descriptor.id);
            let Some(mut clip) = pack_names
                .get(name)
                .and_then(|index| pack_clips[*index].take())
            else {
                continue;
            };
            clip.channels.retain_mut(|channel| {
                let target = pack_rig.nodes[channel.node]
                    .name
                    .as_deref()
                    .and_then(|name| by_name.get(name));
                match target {
                    Some(node) => {
                        channel.node = *node;
                        true
                    }
                    None => false,
                }
            });
            clips.insert(descriptor.id.clone(), clip);
        }
    }
    Ok(clips)
}

/// The node of each rig joint, in rig order (`AnimationRigSignature`): what
/// pose controls and joint reports index.
pub fn rig_nodes<H: Hierarchy + ?Sized>(
    asset: &AnimatedMeshAsset,
    model: &H,
) -> Vec<Option<usize>> {
    let joints = joint_nodes(model);
    asset
        .rig
        .iter()
        .flat_map(|rig| &rig.joints)
        .map(|joint| joints.get(&joint.id).copied())
        .collect()
}

/// A matrix as an Engine transform (translation, normalized rotation,
/// scale).
pub fn transform_of(value: Mat4) -> Transform {
    let (scale, rotation, translation) = value.to_scale_rotation_translation();
    Transform {
        translation: translation.to_array(),
        rotation: rotation.normalize().to_array(),
        scale: scale.to_array(),
    }
}

/// An Engine transform as a matrix. A zero or non-finite rotation is the
/// identity.
pub fn transform_matrix(transform: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        crate::pose::rotation_of(transform.rotation),
        Vec3::from_array(transform.translation),
    )
}

/// Each rig joint of a posed model (node transforms in instance space), in
/// the instance's space and placed in the scene by `world`. A joint the
/// model does not name is at the instance's origin.
pub fn posed_joints(rig: &[Option<usize>], pose: &[Mat4], world: Mat4) -> Vec<PosedJoint> {
    rig.iter()
        .map(|node| {
            let model = node
                .and_then(|node| pose.get(node).copied())
                .unwrap_or(Mat4::IDENTITY);
            PosedJoint {
                model: transform_of(model),
                world: transform_of(world * model),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> RigNode {
        RigNode {
            name: Some(name.to_owned()),
            parent: None,
            rest: Trs {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        }
    }

    #[test]
    fn clip_pack_channels_bind_to_the_skin_joint_that_shares_a_name() {
        // The joint comes first and a mesh node reuses its name; binding by
        // "last named node wins" would move the joint's channels to the mesh.
        let model = Rig {
            nodes: vec![node("RightHand"), node("RightHand"), node("Hips")],
            order: vec![0, 1, 2],
            skins: vec![vec![0, 2]],
            clips: Vec::new(),
        };
        let targets = channel_targets(&model);
        assert_eq!(targets["RightHand"], 0);
        assert_eq!(targets["Hips"], 2);
    }
}
