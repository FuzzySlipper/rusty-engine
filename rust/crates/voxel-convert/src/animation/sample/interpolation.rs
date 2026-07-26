use crate::animation::{
    AnimationChannelValues, AnimationInterpolation, ImportedAnimationChannel,
    ANIMATION_TIMESTAMP_TICKS_PER_SECOND,
};
use crate::ConversionError;

pub(super) fn sample_translation(
    channel: &ImportedAnimationChannel,
    timestamp: u64,
) -> Result<[f64; 3], ConversionError> {
    let AnimationChannelValues::Translations(values) = &channel.values else {
        return Err(channel_value_mismatch(channel));
    };
    sample_vector(channel, values, timestamp)
}

pub(super) fn sample_scale(
    channel: &ImportedAnimationChannel,
    timestamp: u64,
) -> Result<[f64; 3], ConversionError> {
    let AnimationChannelValues::Scales(values) = &channel.values else {
        return Err(channel_value_mismatch(channel));
    };
    sample_vector(channel, values, timestamp)
}

pub(super) fn sample_rotation(
    channel: &ImportedAnimationChannel,
    timestamp: u64,
) -> Result<[f64; 4], ConversionError> {
    let AnimationChannelValues::Rotations(values) = &channel.values else {
        return Err(channel_value_mismatch(channel));
    };
    let segment = sample_segment(channel, timestamp);
    let value = match segment {
        SampleSegment::Key(key) => values[value_index(channel.interpolation, key)],
        SampleSegment::Between {
            left,
            right,
            fraction,
            duration_seconds,
        } => match channel.interpolation {
            AnimationInterpolation::Step => values[left],
            AnimationInterpolation::Linear => slerp(values[left], values[right], fraction),
            AnimationInterpolation::CubicSpline => {
                let left_value = values[left * 3 + 1];
                let left_out = values[left * 3 + 2];
                let right_in = values[right * 3];
                let right_value = values[right * 3 + 1];
                std::array::from_fn(|component| {
                    cubic(
                        left_value[component],
                        left_out[component],
                        right_in[component],
                        right_value[component],
                        fraction,
                        duration_seconds,
                    )
                })
            }
        },
    };
    normalize_sampled_quaternion(value, channel)
}

pub(super) fn sample_morph_weights(
    channel: &ImportedAnimationChannel,
    timestamp: u64,
) -> Result<Vec<f64>, ConversionError> {
    let AnimationChannelValues::MorphWeights {
        target_count,
        values,
    } = &channel.values
    else {
        return Err(channel_value_mismatch(channel));
    };
    let target_count = *target_count as usize;
    let segment = sample_segment(channel, timestamp);
    (0..target_count)
        .map(|component| {
            let value = match segment {
                SampleSegment::Key(key) => {
                    values[value_index(channel.interpolation, key) * target_count + component]
                }
                SampleSegment::Between {
                    left,
                    right,
                    fraction,
                    duration_seconds,
                } => match channel.interpolation {
                    AnimationInterpolation::Step => values[left * target_count + component],
                    AnimationInterpolation::Linear => lerp(
                        values[left * target_count + component],
                        values[right * target_count + component],
                        fraction,
                    ),
                    AnimationInterpolation::CubicSpline => cubic(
                        values[(left * 3 + 1) * target_count + component],
                        values[(left * 3 + 2) * target_count + component],
                        values[(right * 3) * target_count + component],
                        values[(right * 3 + 1) * target_count + component],
                        fraction,
                        duration_seconds,
                    ),
                },
            };
            if value.is_finite() {
                Ok(value)
            } else {
                Err(non_finite_channel(channel))
            }
        })
        .collect()
}

fn sample_vector<const N: usize>(
    channel: &ImportedAnimationChannel,
    values: &[[f64; N]],
    timestamp: u64,
) -> Result<[f64; N], ConversionError> {
    let segment = sample_segment(channel, timestamp);
    let sampled = match segment {
        SampleSegment::Key(key) => values[value_index(channel.interpolation, key)],
        SampleSegment::Between {
            left,
            right,
            fraction,
            duration_seconds,
        } => match channel.interpolation {
            AnimationInterpolation::Step => values[left],
            AnimationInterpolation::Linear => std::array::from_fn(|component| {
                lerp(values[left][component], values[right][component], fraction)
            }),
            AnimationInterpolation::CubicSpline => std::array::from_fn(|component| {
                cubic(
                    values[left * 3 + 1][component],
                    values[left * 3 + 2][component],
                    values[right * 3][component],
                    values[right * 3 + 1][component],
                    fraction,
                    duration_seconds,
                )
            }),
        },
    };
    if sampled.iter().any(|component| !component.is_finite()) {
        return Err(non_finite_channel(channel));
    }
    Ok(sampled)
}

#[derive(Clone, Copy)]
enum SampleSegment {
    Key(usize),
    Between {
        left: usize,
        right: usize,
        fraction: f64,
        duration_seconds: f64,
    },
}

fn sample_segment(channel: &ImportedAnimationChannel, timestamp: u64) -> SampleSegment {
    let timestamps = &channel.timestamps_microseconds;
    if timestamp <= timestamps[0] {
        return SampleSegment::Key(0);
    }
    let last = timestamps.len() - 1;
    if timestamp >= timestamps[last] {
        return SampleSegment::Key(last);
    }
    let right = timestamps.partition_point(|candidate| *candidate <= timestamp);
    let left = right - 1;
    if timestamps[left] == timestamp {
        return SampleSegment::Key(left);
    }
    let duration = timestamps[right] - timestamps[left];
    SampleSegment::Between {
        left,
        right,
        fraction: (timestamp - timestamps[left]) as f64 / duration as f64,
        duration_seconds: duration as f64 / ANIMATION_TIMESTAMP_TICKS_PER_SECOND as f64,
    }
}

fn value_index(interpolation: AnimationInterpolation, key: usize) -> usize {
    if interpolation == AnimationInterpolation::CubicSpline {
        key * 3 + 1
    } else {
        key
    }
}

fn lerp(left: f64, right: f64, amount: f64) -> f64 {
    left + (right - left) * amount
}

fn cubic(
    left_value: f64,
    left_out_tangent: f64,
    right_in_tangent: f64,
    right_value: f64,
    amount: f64,
    duration_seconds: f64,
) -> f64 {
    let squared = amount * amount;
    let cubed = squared * amount;
    (2.0 * cubed - 3.0 * squared + 1.0) * left_value
        + (cubed - 2.0 * squared + amount) * duration_seconds * left_out_tangent
        + (-2.0 * cubed + 3.0 * squared) * right_value
        + (cubed - squared) * duration_seconds * right_in_tangent
}

fn slerp(mut left: [f64; 4], mut right: [f64; 4], amount: f64) -> [f64; 4] {
    let mut dot = (0..4).map(|index| left[index] * right[index]).sum::<f64>();
    if dot < 0.0 {
        for component in &mut right {
            *component = -*component;
        }
        dot = -dot;
    }
    if dot > 0.9995 {
        return std::array::from_fn(|index| lerp(left[index], right[index], amount));
    }
    dot = dot.clamp(-1.0, 1.0);
    let theta = dot.acos();
    let sin_theta = theta.sin();
    let left_weight = ((1.0 - amount) * theta).sin() / sin_theta;
    let right_weight = (amount * theta).sin() / sin_theta;
    for index in 0..4 {
        left[index] = left[index] * left_weight + right[index] * right_weight;
    }
    left
}

fn normalize_sampled_quaternion(
    mut value: [f64; 4],
    channel: &ImportedAnimationChannel,
) -> Result<[f64; 4], ConversionError> {
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(non_finite_channel(channel));
    }
    for component in &mut value {
        *component /= length;
    }
    Ok(value)
}

fn non_finite_channel(channel: &ImportedAnimationChannel) -> ConversionError {
    ConversionError::one(
        "conversion.nonFiniteDeformation",
        format!("sample.channels[{}]", channel.source_channel_index),
        "animation interpolation produced a non-finite value",
    )
}

fn channel_value_mismatch(channel: &ImportedAnimationChannel) -> ConversionError {
    ConversionError::one(
        "conversion.invalidAnimation",
        format!(
            "source.animations.channels[{}]",
            channel.source_channel_index
        ),
        "imported channel values do not match the target property",
    )
}
