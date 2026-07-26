use std::collections::{BTreeMap, BTreeSet};

use gltf::{
    animation::{util::ReadOutputs, Interpolation, Property},
    buffer::Source as BufferSource,
};

use super::{normalize_quaternion, validate_accessor_count};
use crate::animation::{
    AnimationChannelValues, AnimationInterpolation, AnimationProperty, ImportedAnimationChannel,
    ImportedAnimationClip, ImportedAnimationNode, ImportedNodeTransform,
    ANIMATION_TIMESTAMP_TICKS_PER_SECOND, MAX_ANIMATION_DURATION_MICROSECONDS,
    MAX_IMPORTED_ANIMATION_CHANNELS, MAX_IMPORTED_ANIMATION_CLIPS,
    MAX_IMPORTED_ANIMATION_KEYFRAMES, MAX_IMPORTED_ANIMATION_VALUES,
};
use crate::import::validate_imported_name;
use crate::ConversionError;

pub(super) fn import_clips(
    document: &gltf::Document,
    blob: &[u8],
    nodes: &[ImportedAnimationNode],
) -> Result<Vec<ImportedAnimationClip>, ConversionError> {
    let clip_count = document.animations().count();
    if clip_count == 0 || clip_count > MAX_IMPORTED_ANIMATION_CLIPS {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source.animations",
            format!("named clip count must be in 1..={MAX_IMPORTED_ANIMATION_CLIPS}"),
        ));
    }
    let node_by_index = nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect::<BTreeMap<_, _>>();
    let mut names = BTreeSet::new();
    let mut total_channels = 0usize;
    let mut total_keyframes = 0usize;
    let mut total_output_components = 0usize;
    let mut clips = Vec::with_capacity(clip_count);

    for animation in document.animations() {
        let animation_index = animation.index();
        let path = format!("source.animations[{animation_index}]");
        let name =
            validate_imported_name(animation.name(), format!("{path}.name"))?.ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidAnimation",
                    format!("{path}.name"),
                    "every imported animation clip must have a stable non-empty name",
                )
            })?;
        if !names.insert(name.clone()) {
            return Err(ConversionError::one(
                "conversion.invalidAnimation",
                format!("{path}.name"),
                format!("duplicate animation clip name {name:?}"),
            ));
        }
        let mut targets = BTreeSet::new();
        let mut channels = Vec::new();
        let mut duration_microseconds = 0u64;

        for channel in animation.channels() {
            total_channels = total_channels.saturating_add(1);
            if total_channels > MAX_IMPORTED_ANIMATION_CHANNELS {
                return Err(ConversionError::one(
                    "conversion.resourceLimit",
                    "source.animations.channels",
                    format!("total channel count exceeds {MAX_IMPORTED_ANIMATION_CHANNELS}"),
                ));
            }
            let channel_index = channel.index();
            let channel_path = format!("{path}.channels[{channel_index}]");
            let target = channel.target();
            let target_node_index = u32::try_from(target.node().index()).map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    format!("{channel_path}.target.node"),
                    "target node index exceeds u32",
                )
            })?;
            let node = node_by_index.get(&target_node_index).ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidAnimation",
                    format!("{channel_path}.target.node"),
                    "animation channel targets a node outside the selected scene",
                )
            })?;
            let property = match target.property() {
                Property::Translation => AnimationProperty::Translation,
                Property::Rotation => AnimationProperty::Rotation,
                Property::Scale => AnimationProperty::Scale,
                Property::MorphTargetWeights => AnimationProperty::MorphWeights,
            };
            if !targets.insert((target_node_index, property_ordinal(property))) {
                return Err(ConversionError::one(
                    "conversion.invalidAnimation",
                    format!("{channel_path}.target"),
                    "one clip cannot define duplicate channels for one node property",
                ));
            }
            if property != AnimationProperty::MorphWeights
                && matches!(node.base_transform, ImportedNodeTransform::Matrix(_))
            {
                return Err(ConversionError::one(
                    "conversion.unsupportedFeature",
                    format!("{channel_path}.target.node"),
                    "TRS animation of a matrix-authored node is not supported",
                ));
            }
            if property == AnimationProperty::MorphWeights && node.base_morph_weights.is_empty() {
                return Err(ConversionError::one(
                    "conversion.invalidMorphTarget",
                    format!("{channel_path}.target.path"),
                    "weight channel targets a node without morph targets",
                ));
            }

            let interpolation = match channel.sampler().interpolation() {
                Interpolation::Step => AnimationInterpolation::Step,
                Interpolation::Linear => AnimationInterpolation::Linear,
                Interpolation::CubicSpline => AnimationInterpolation::CubicSpline,
            };
            let keyframe_count = channel.sampler().input().count();
            if keyframe_count == 0 {
                return Err(ConversionError::one(
                    "conversion.invalidAnimation",
                    format!("{channel_path}.sampler.input"),
                    "animation channel contains no timestamps",
                ));
            }
            total_keyframes = total_keyframes
                .checked_add(keyframe_count)
                .ok_or_else(|| animation_keyframe_limit("total keyframe count overflowed"))?;
            if total_keyframes > MAX_IMPORTED_ANIMATION_KEYFRAMES {
                return Err(animation_keyframe_limit(&format!(
                    "total keyframe count exceeds {MAX_IMPORTED_ANIMATION_KEYFRAMES}"
                )));
            }
            let (expected_accessor_count, output_components) = expected_output_counts(
                property,
                interpolation,
                keyframe_count,
                node.base_morph_weights.len(),
            )?;
            validate_accessor_count(
                channel.sampler().output().count(),
                expected_accessor_count,
                &format!("{channel_path}.sampler.output"),
            )?;
            total_output_components = total_output_components
                .checked_add(output_components)
                .ok_or_else(|| animation_value_limit("total output component count overflowed"))?;
            if total_output_components > MAX_IMPORTED_ANIMATION_VALUES {
                return Err(animation_value_limit(&format!(
                    "total output component count exceeds {MAX_IMPORTED_ANIMATION_VALUES}"
                )));
            }

            let reader = channel.reader(|buffer| match buffer.source() {
                BufferSource::Bin => Some(blob),
                BufferSource::Uri(_) => None,
            });
            let source_timestamps = reader.read_inputs().ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidAccessor",
                    format!("{channel_path}.sampler.input"),
                    "animation input accessor could not be read",
                )
            })?;
            let timestamps_microseconds = import_timestamps(source_timestamps, &channel_path)?;
            debug_assert_eq!(timestamps_microseconds.len(), keyframe_count);
            duration_microseconds = duration_microseconds.max(
                *timestamps_microseconds
                    .last()
                    .expect("timestamp import rejects empty accessors"),
            );
            let values = import_channel_values(
                reader.read_outputs().ok_or_else(|| {
                    ConversionError::one(
                        "conversion.invalidAccessor",
                        format!("{channel_path}.sampler.output"),
                        "animation output accessor could not be read",
                    )
                })?,
                property,
                interpolation,
                keyframe_count,
                node.base_morph_weights.len(),
                &channel_path,
            )?;
            channels.push(ImportedAnimationChannel {
                source_channel_index: u32::try_from(channel_index)
                    .map_err(|_| animation_keyframe_limit("source channel index exceeds u32"))?,
                target_node_index,
                property,
                interpolation,
                timestamps_microseconds,
                values,
            });
        }
        if channels.is_empty() {
            return Err(ConversionError::one(
                "conversion.invalidAnimation",
                format!("{path}.channels"),
                "animation clip contains no channels",
            ));
        }
        clips.push(ImportedAnimationClip {
            source_animation_index: u32::try_from(animation_index)
                .map_err(|_| animation_keyframe_limit("source animation index exceeds u32"))?,
            name,
            duration_microseconds,
            channels,
        });
    }
    Ok(clips)
}

fn import_timestamps(
    values: impl Iterator<Item = f32>,
    channel_path: &str,
) -> Result<Vec<u64>, ConversionError> {
    let mut previous_seconds = None;
    let mut previous_tick = None;
    let mut imported = Vec::new();
    for (index, seconds) in values.enumerate() {
        if !seconds.is_finite()
            || seconds < 0.0
            || previous_seconds.is_some_and(|previous| seconds <= previous)
        {
            return Err(ConversionError::one(
                "conversion.invalidAnimation",
                format!("{channel_path}.sampler.input[{index}]"),
                "animation timestamps must be finite, non-negative, and strictly increasing",
            ));
        }
        let scaled = f64::from(seconds) * ANIMATION_TIMESTAMP_TICKS_PER_SECOND as f64;
        if scaled > MAX_ANIMATION_DURATION_MICROSECONDS as f64 + 0.5 {
            return Err(ConversionError::one(
                "conversion.resourceLimit",
                format!("{channel_path}.sampler.input[{index}]"),
                format!(
                    "quantized timestamp exceeds {MAX_ANIMATION_DURATION_MICROSECONDS} microseconds"
                ),
            ));
        }
        let tick = scaled.round() as u64;
        if previous_tick.is_some_and(|previous| tick <= previous) {
            return Err(ConversionError::one(
                "conversion.invalidAnimation",
                format!("{channel_path}.sampler.input[{index}]"),
                "distinct source timestamps collapse at microsecond quantization",
            ));
        }
        imported.push(tick);
        previous_seconds = Some(seconds);
        previous_tick = Some(tick);
    }
    if imported.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidAnimation",
            format!("{channel_path}.sampler.input"),
            "animation channel contains no timestamps",
        ));
    }
    Ok(imported)
}

fn expected_output_counts(
    property: AnimationProperty,
    interpolation: AnimationInterpolation,
    keyframe_count: usize,
    morph_target_count: usize,
) -> Result<(usize, usize), ConversionError> {
    let multiplier = if interpolation == AnimationInterpolation::CubicSpline {
        3
    } else {
        1
    };
    let vectors = keyframe_count
        .checked_mul(multiplier)
        .ok_or_else(|| animation_value_limit("animation output count overflowed"))?;
    let (accessor_count, component_count) = match property {
        AnimationProperty::Translation | AnimationProperty::Scale => (vectors, 3),
        AnimationProperty::Rotation => (vectors, 4),
        AnimationProperty::MorphWeights => (
            vectors
                .checked_mul(morph_target_count)
                .ok_or_else(|| animation_value_limit("morph output count overflowed"))?,
            1,
        ),
    };
    let output_components = accessor_count
        .checked_mul(component_count)
        .ok_or_else(|| animation_value_limit("animation component count overflowed"))?;
    Ok((accessor_count, output_components))
}

fn import_channel_values(
    outputs: ReadOutputs<'_>,
    property: AnimationProperty,
    interpolation: AnimationInterpolation,
    keyframe_count: usize,
    morph_target_count: usize,
    channel_path: &str,
) -> Result<AnimationChannelValues, ConversionError> {
    let multiplier = if interpolation == AnimationInterpolation::CubicSpline {
        3
    } else {
        1
    };
    let expected_vectors = keyframe_count
        .checked_mul(multiplier)
        .ok_or_else(|| animation_keyframe_limit("animation output count overflowed"))?;
    let output_path = format!("{channel_path}.sampler.output");
    let values = match (property, outputs) {
        (AnimationProperty::Translation, ReadOutputs::Translations(values)) => {
            let values = values.map(|value| value.map(f64::from)).collect::<Vec<_>>();
            validate_vector_outputs(&values, expected_vectors, &output_path)?;
            AnimationChannelValues::Translations(values)
        }
        (AnimationProperty::Rotation, ReadOutputs::Rotations(values)) => {
            let mut values = values
                .into_f32()
                .map(|value| value.map(f64::from))
                .collect::<Vec<_>>();
            if values.len() != expected_vectors || values.iter().flatten().any(|v| !v.is_finite()) {
                return Err(invalid_output_count(
                    &output_path,
                    expected_vectors,
                    values.len(),
                ));
            }
            for (index, value) in values.iter_mut().enumerate() {
                if interpolation != AnimationInterpolation::CubicSpline || index % 3 == 1 {
                    *value = normalize_quaternion(*value, &format!("{output_path}[{index}]"))?;
                }
            }
            AnimationChannelValues::Rotations(values)
        }
        (AnimationProperty::Scale, ReadOutputs::Scales(values)) => {
            let values = values.map(|value| value.map(f64::from)).collect::<Vec<_>>();
            validate_vector_outputs(&values, expected_vectors, &output_path)?;
            AnimationChannelValues::Scales(values)
        }
        (AnimationProperty::MorphWeights, ReadOutputs::MorphTargetWeights(values)) => {
            let values = values.into_f32().map(f64::from).collect::<Vec<_>>();
            let expected = expected_vectors
                .checked_mul(morph_target_count)
                .ok_or_else(|| animation_keyframe_limit("morph output count overflowed"))?;
            if values.len() != expected || values.iter().any(|value| !value.is_finite()) {
                return Err(invalid_output_count(&output_path, expected, values.len()));
            }
            AnimationChannelValues::MorphWeights {
                target_count: u32::try_from(morph_target_count)
                    .map_err(|_| animation_keyframe_limit("morph target count exceeds u32"))?,
                values,
            }
        }
        _ => {
            return Err(ConversionError::one(
                "conversion.invalidAccessor",
                output_path,
                "animation output accessor type does not match its target property",
            ));
        }
    };
    Ok(values)
}

fn validate_vector_outputs<const N: usize>(
    values: &[[f64; N]],
    expected: usize,
    path: &str,
) -> Result<(), ConversionError> {
    if values.len() != expected || values.iter().flatten().any(|value| !value.is_finite()) {
        return Err(invalid_output_count(path, expected, values.len()));
    }
    Ok(())
}

fn invalid_output_count(path: &str, expected: usize, actual: usize) -> ConversionError {
    ConversionError::one(
        "conversion.invalidAccessor",
        path,
        format!("animation output must contain {expected} finite values; found {actual}"),
    )
}

fn property_ordinal(property: AnimationProperty) -> u8 {
    match property {
        AnimationProperty::Translation => 0,
        AnimationProperty::Rotation => 1,
        AnimationProperty::Scale => 2,
        AnimationProperty::MorphWeights => 3,
    }
}

fn animation_keyframe_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.animations",
        message.to_owned(),
    )
}

fn animation_value_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.animations.values",
        message.to_owned(),
    )
}
