//! Direct-light sampling using the same retained light descriptors as rendering.
//! Geometry occlusion is supplied by the owning spatial service; no second light store.
use crate::{LightDescriptor, LightShadowIntent};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DirectLightSample {
    pub irradiance: [f32; 3],
    pub contributing_lights: u32,
    pub occluded_lights: u32,
}

/// Inputs are validated by the owning call. A zero normal samples incident light
/// without a cosine term; otherwise the normal must be unit length. Shadow rays
/// use the caller's finite directional horizon. This is not indirect/bounce light.
pub fn sample_direct_lighting(
    position: [f64; 3],
    normal: [f64; 3],
    directional_distance: f64,
    lights: &[LightDescriptor],
    mut occluded: impl FnMut([f64; 3], f64) -> bool,
) -> DirectLightSample {
    let mut result = DirectLightSample::default();
    for light in lights {
        let (color, intensity, enabled, direction, distance, attenuation) = match light {
            LightDescriptor::Ambient {
                color,
                intensity,
                enabled,
                ..
            } => (*color, *intensity, *enabled, [0.0; 3], 0.0, 1.0),
            LightDescriptor::Directional {
                color,
                intensity,
                enabled,
                direction,
                ..
            } => {
                let direction = normalize(direction.map(|v| -f64::from(v)));
                (
                    *color,
                    *intensity,
                    *enabled,
                    direction,
                    directional_distance,
                    1.0,
                )
            }
            LightDescriptor::Point {
                color,
                intensity,
                enabled,
                position: source,
                range,
                decay,
                ..
            }
            | LightDescriptor::Spot {
                color,
                intensity,
                enabled,
                position: source,
                range,
                decay,
                ..
            } => {
                let delta = std::array::from_fn(|i| f64::from(source[i]) - position[i]);
                let distance = length(delta);
                let direction = normalize(delta);
                // Mirrors the Engine Three backend's physically-decaying direct lights.
                let mut attenuation = 1.0 / distance.powf(f64::from(*decay)).max(0.01);
                if let Some(range) = range {
                    attenuation *= (1.0 - (distance / f64::from(*range)).powi(4))
                        .clamp(0.0, 1.0)
                        .powi(2);
                }
                if let LightDescriptor::Spot {
                    direction: axis,
                    outer_angle_radians,
                    penumbra,
                    ..
                } = light
                {
                    let axis = normalize(axis.map(f64::from));
                    let cosine = -dot(direction, axis);
                    let outer = f64::from(*outer_angle_radians).cos();
                    let inner = f64::from(*outer_angle_radians * (1.0 - penumbra)).cos();
                    let t = if inner == outer {
                        f64::from(cosine >= outer)
                    } else {
                        ((cosine - outer) / (inner - outer)).clamp(0.0, 1.0)
                    };
                    attenuation *= t * t * (3.0 - 2.0 * t);
                }
                (
                    *color,
                    *intensity,
                    *enabled,
                    direction,
                    distance,
                    attenuation,
                )
            }
        };
        if !enabled || intensity == 0.0 {
            continue;
        }
        let cosine = if normal == [0.0; 3] || distance == 0.0 {
            1.0
        } else {
            dot(normal, direction).max(0.0)
        };
        let weight = f64::from(intensity) * attenuation * cosine;
        if weight == 0.0 {
            continue;
        }
        if distance > 0.0
            && light.shadow_intent() == LightShadowIntent::Requested
            && occluded(direction, distance)
        {
            result.occluded_lights += 1;
            continue;
        }
        result.contributing_lights += 1;
        for (value, color) in result.irradiance.iter_mut().zip(color) {
            *value += (weight * f64::from(color)) as f32;
        }
    }
    result
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}
fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = length(v);
    if n == 0.0 {
        [0.0; 3]
    } else {
        v.map(|x| x / n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn torch() -> LightDescriptor {
        LightDescriptor::Point {
            color: [1.0, 0.5, 0.0],
            intensity: 4.0,
            enabled: true,
            position: [0.0, 2.0, 0.0],
            range: Some(8.0),
            decay: 2.0,
            shadow_intent: LightShadowIntent::Requested,
        }
    }
    #[test]
    fn torch_attenuates_and_walls_block_light() {
        let light = torch();
        let near = sample_direct_lighting(
            [0.0; 3],
            [0.0; 3],
            20.0,
            std::slice::from_ref(&light),
            |_, _| false,
        );
        let far = sample_direct_lighting(
            [0.0, -2.0, 0.0],
            [0.0; 3],
            20.0,
            std::slice::from_ref(&light),
            |_, _| false,
        );
        assert!(near.irradiance[0] > far.irradiance[0]);
        let dark = sample_direct_lighting([0.0; 3], [0.0; 3], 20.0, &[light], |_, _| true);
        assert_eq!(dark.irradiance, [0.0; 3]);
        assert_eq!(dark.occluded_lights, 1);
    }
    #[test]
    fn light_descriptors_round_trip_and_normal_rejects_back_face() {
        let bytes = serde_json::to_vec(&vec![torch()]).unwrap();
        let lights: Vec<LightDescriptor> = serde_json::from_slice(&bytes).unwrap();
        let sample =
            sample_direct_lighting([0.0; 3], [0.0, -1.0, 0.0], 20.0, &lights, |_, _| false);
        assert_eq!(sample.irradiance, [0.0; 3]);
        assert_eq!(
            sample_direct_lighting([0.0; 3], [0.0; 3], 20.0, &lights, |_, _| false),
            sample_direct_lighting([0.0; 3], [0.0; 3], 20.0, &[torch()], |_, _| false)
        );
    }
}
