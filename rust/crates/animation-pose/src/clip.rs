//! Clips: keyed channels on nodes, sampled and blended over a rest pose.

use glam::{Mat4, Quat, Vec3};

use render_model::{ClipLayer, ClipLayerBlend};

use crate::Hierarchy;

/// A node's local translation, rotation and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Trs {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn translation(&self) -> [f32; 3] {
        self.translation.to_array()
    }

    /// `[x, y, z, w]`.
    pub fn rotation(&self) -> [f32; 4] {
        self.rotation.to_array()
    }

    pub fn scale(&self) -> [f32; 3] {
        self.scale.to_array()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Step,
    Linear,
    CubicSpline,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub node: usize,
    pub path: Path,
    pub interpolation: Interp,
    pub times: Vec<f32>,
    /// Flattened values: 3 or 4 per key, times 3 for cubic splines
    /// (in-tangent, value, out-tangent).
    pub values: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: Option<String>,
    pub duration: f32,
    pub channels: Vec<Channel>,
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

    pub fn sample_vec3(&self, time: f32) -> Vec3 {
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

    pub fn sample_quat(&self, time: f32) -> Quat {
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

/// Local TRS per node after blending the weighted clips (`(clip index,
/// time, weight)`) over the rest pose.
pub fn sample_locals<H: Hierarchy + ?Sized>(
    model: &H,
    clips: &[Clip],
    actions: &[(usize, f32, f32)],
) -> Vec<Trs> {
    #[derive(Clone, Copy)]
    struct Mix<T> {
        value: T,
        weight: f32,
    }
    let count = model.node_count();
    let mut translation: Vec<Option<Mix<Vec3>>> = vec![None; count];
    let mut rotation: Vec<Option<Mix<Quat>>> = vec![None; count];
    let mut scale: Vec<Option<Mix<Vec3>>> = vec![None; count];
    for (clip, time, weight) in actions {
        if *weight <= 0.0 {
            continue;
        }
        let Some(clip) = clips.get(*clip) else {
            continue;
        };
        for channel in &clip.channels {
            match channel.path {
                Path::Translation | Path::Scale => {
                    let value = channel.sample_vec3(*time);
                    let slot = if channel.path == Path::Translation {
                        &mut translation[channel.node]
                    } else {
                        &mut scale[channel.node]
                    };
                    *slot = Some(match *slot {
                        None => Mix {
                            value,
                            weight: *weight,
                        },
                        Some(mix) => Mix {
                            value: mix.value.lerp(value, weight / (mix.weight + weight)),
                            weight: mix.weight + weight,
                        },
                    });
                }
                Path::Rotation => {
                    let value = channel.sample_quat(*time);
                    let slot = &mut rotation[channel.node];
                    *slot = Some(match *slot {
                        None => Mix {
                            value,
                            weight: *weight,
                        },
                        Some(mix) => Mix {
                            value: mix.value.slerp(value, weight / (mix.weight + weight)),
                            weight: mix.weight + weight,
                        },
                    });
                }
            }
        }
    }
    let settle_vec = |rest: Vec3, mix: Option<Mix<Vec3>>| match mix {
        None => rest,
        Some(mix) if mix.weight < 1.0 => rest.lerp(mix.value, mix.weight),
        Some(mix) => mix.value,
    };
    (0..count)
        .map(|index| {
            let rest = model.rest(index);
            Trs {
                translation: settle_vec(rest.translation, translation[index]),
                rotation: match rotation[index] {
                    None => rest.rotation,
                    Some(mix) if mix.weight < 1.0 => rest.rotation.slerp(mix.value, mix.weight),
                    Some(mix) => mix.value,
                },
                scale: settle_vec(rest.scale, scale[index]),
            }
        })
        .collect()
}

/// `layers` laid over `locals` in order. Each layer's clip is found by
/// `index` (a layer naming no clip changes nothing) and its joint weights
/// name rig joints, found through `rig`. Only the nodes and properties the
/// clip has channels for change.
pub fn layer_clips<H: Hierarchy + ?Sized>(
    model: &H,
    clips: &[Clip],
    locals: &mut [Trs],
    layers: &[ClipLayer],
    rig: &[Option<usize>],
    index: &dyn Fn(&str) -> Option<usize>,
) {
    let count = model.node_count();
    let mut mask = vec![0.0f32; count];
    for layer in layers {
        let Some(clip) = index(&layer.clip).and_then(|clip| clips.get(clip)) else {
            continue;
        };
        let at = |time: f32| {
            if layer.normalized {
                clip.duration * time
            } else {
                time
            }
        };
        let (time, reference) = (at(layer.time), at(layer.reference_time));
        let weighted = !layer.joint_weights.is_empty();
        if weighted {
            mask.fill(0.0);
            for joint in &layer.joint_weights {
                if let Some(node) = rig.get(joint.joint as usize).copied().flatten() {
                    mask[node] = joint.weight;
                }
            }
        }
        for channel in &clip.channels {
            let weight = layer.weight * if weighted { mask[channel.node] } else { 1.0 };
            if weight <= 0.0 {
                continue;
            }
            let local = &mut locals[channel.node];
            match (layer.blend, channel.path) {
                (ClipLayerBlend::Override, Path::Translation) => {
                    local.translation = local.translation.lerp(channel.sample_vec3(time), weight);
                }
                (ClipLayerBlend::Override, Path::Scale) => {
                    local.scale = local.scale.lerp(channel.sample_vec3(time), weight);
                }
                (ClipLayerBlend::Override, Path::Rotation) => {
                    local.rotation = local
                        .rotation
                        .slerp(channel.sample_quat(time), weight)
                        .normalize();
                }
                (ClipLayerBlend::Additive, Path::Translation) => {
                    let change = channel.sample_vec3(time) - channel.sample_vec3(reference);
                    local.translation += change * weight;
                }
                (ClipLayerBlend::Additive, Path::Scale) => {
                    let from = channel.sample_vec3(reference);
                    if from.cmpne(Vec3::ZERO).all() {
                        let change = channel.sample_vec3(time) / from;
                        local.scale *= Vec3::ONE.lerp(change, weight);
                    }
                }
                (ClipLayerBlend::Additive, Path::Rotation) => {
                    let change =
                        channel.sample_quat(reference).inverse() * channel.sample_quat(time);
                    local.rotation =
                        (local.rotation * Quat::IDENTITY.slerp(change, weight)).normalize();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(interpolation: Interp, values: Vec<f32>) -> Channel {
        Channel {
            node: 0,
            path: Path::Translation,
            interpolation,
            times: vec![0.0, 1.0, 3.0],
            values,
        }
    }

    struct Line(Vec<usize>);

    impl Hierarchy for Line {
        fn node_count(&self) -> usize {
            self.0.len()
        }
        fn parent(&self, node: usize) -> Option<usize> {
            node.checked_sub(1)
        }
        fn rest(&self, _: usize) -> Trs {
            Trs {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            }
        }
        fn name(&self, _: usize) -> Option<&str> {
            None
        }
        fn order(&self) -> &[usize] {
            &self.0
        }
        fn skin_joints(&self) -> Vec<usize> {
            self.0.clone()
        }
    }

    /// A clip moving node 0 along x from 0 to 2 over 2 s and turning node 1
    /// a quarter about y.
    fn clip() -> Clip {
        let half = std::f32::consts::FRAC_PI_4.sin();
        Clip {
            name: Some("wave".to_owned()),
            duration: 2.0,
            channels: vec![
                Channel {
                    node: 0,
                    path: Path::Translation,
                    interpolation: Interp::Linear,
                    times: vec![0.0, 2.0],
                    values: vec![0.0, 0.0, 0.0, 2.0, 0.0, 0.0],
                },
                Channel {
                    node: 1,
                    path: Path::Rotation,
                    interpolation: Interp::Linear,
                    times: vec![0.0, 2.0],
                    values: vec![0.0, 0.0, 0.0, 1.0, 0.0, half, 0.0, half],
                },
            ],
        }
    }

    fn layer(blend: ClipLayerBlend, time: f32, weight: f32, joints: Vec<(u32, f32)>) -> ClipLayer {
        ClipLayer {
            clip: "wave".to_owned(),
            time,
            normalized: true,
            weight,
            blend,
            reference_time: 0.0,
            joint_weights: joints
                .into_iter()
                .map(|(joint, weight)| render_model::JointWeight { joint, weight })
                .collect(),
        }
    }

    fn laid(layers: &[ClipLayer], below: Vec<Trs>) -> Vec<Trs> {
        let model = Line(vec![0, 1, 2]);
        let mut locals = below;
        layer_clips(
            &model,
            &[clip()],
            &mut locals,
            layers,
            &[Some(0), Some(1), Some(2)],
            &|name| (name == "wave").then_some(0),
        );
        locals
    }

    fn rest() -> Vec<Trs> {
        (0..3).map(|node| Line(vec![]).rest(node)).collect()
    }

    #[test]
    fn an_override_layer_moves_only_the_joints_its_clip_animates_by_weight_and_mask() {
        let half = laid(&[layer(ClipLayerBlend::Override, 1.0, 0.5, vec![])], rest());
        assert!(
            (half[0].translation.x - 1.0).abs() < 1e-6,
            "halfway to the clip's 2"
        );
        assert_eq!(half[2], rest()[2], "a joint the clip has no channel for");
        let masked = laid(
            &[layer(ClipLayerBlend::Override, 1.0, 1.0, vec![(1, 1.0)])],
            rest(),
        );
        assert_eq!(masked[0], rest()[0], "a joint left out of the mask");
        assert!(masked[1].rotation.angle_between(Quat::IDENTITY) > 1.5);
    }

    #[test]
    fn an_additive_layer_adds_its_change_from_the_reference_over_the_pose_below() {
        let mut below = rest();
        below[0].translation = Vec3::new(0.0, 5.0, 0.0);
        below[1].rotation = Quat::from_rotation_x(0.5);
        let added = laid(
            &[layer(ClipLayerBlend::Additive, 0.5, 1.0, vec![])],
            below.clone(),
        );
        assert!((added[0].translation - Vec3::new(1.0, 5.0, 0.0)).length() < 1e-6);
        let expected = below[1].rotation * Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        assert!(added[1].rotation.angle_between(expected) < 1e-4);
        // At its reference time an additive layer changes nothing.
        let none = laid(
            &[layer(ClipLayerBlend::Additive, 0.0, 1.0, vec![])],
            below.clone(),
        );
        assert!((none[0].translation - below[0].translation).length() < 1e-6);
        // Layers apply in order: an override after an additive replaces it.
        let both = laid(
            &[
                layer(ClipLayerBlend::Additive, 0.5, 1.0, vec![]),
                layer(ClipLayerBlend::Override, 0.0, 1.0, vec![]),
            ],
            below,
        );
        assert!(both[0].translation.length() < 1e-6);
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
