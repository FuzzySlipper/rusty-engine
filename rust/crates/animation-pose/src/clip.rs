//! Clips: keyed channels on nodes, sampled and blended over a rest pose.

use glam::{Mat4, Quat, Vec3};

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
