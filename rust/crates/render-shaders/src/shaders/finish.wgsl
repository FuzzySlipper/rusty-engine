#define_import_path rusty::finish

// The finish: exposure, colour grading, the tone mapping operator, then
// distance fog toward its colour, thinning with height and turning toward
// the haze colour near the sun (the atmosphere). The world draws in linear HDR and the
// finish pass (`finish_pass.wgsl`) finishes each of its pixels, so a
// material stage's `finish` returns its colour unchanged. The background
// (clear colour or sky) is drawn before the world and never finished, so a
// fog colour equal to the background fades geometry into it exactly.

#import rusty::view::frame
#import rusty::tonemap::{aces_filmic, neutral}

const TONE_NEUTRAL: u32 = 1u;
const TONE_ACES_FILMIC: u32 = 2u;
const FOG_LINEAR: u32 = 1u;
const FOG_EXPONENTIAL: u32 = 2u;
const FOG_EXPONENTIAL_SQUARED: u32 = 3u;

// A material stage's finish: the finish pass does the work.
fn finish(color: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    return color;
}

const GRADED: u32 = 1u;
const MIDDLE_GREY: f32 = 0.18;

// Linear Rec. 709 to LMS (CAT02) and back, for white balance; a vector
// times the matrix takes each row of the usual matrix.
const LINEAR_TO_LMS: mat3x3<f32> = mat3x3<f32>(
    3.90405e-1, 5.49941e-1, 8.92632e-3,
    7.08416e-2, 9.63172e-1, 1.35775e-3,
    2.31082e-2, 1.28021e-1, 9.36245e-1,
);
const LMS_TO_LINEAR: mat3x3<f32> = mat3x3<f32>(
    2.85847e+0, -1.62879e+0, -2.48910e-2,
    -2.10182e-1, 1.15820e+0, 3.24281e-4,
    -4.18120e-2, -1.18169e-1, 1.06867e+0,
);

// The product's colour grading, before the operator: white balance, then
// contrast about middle grey on a log scale, then saturation about the
// luminance.
fn graded(rgb: vec3<f32>) -> vec3<f32> {
    if frame.modes.z != GRADED {
        return rgb;
    }
    let balanced = ((rgb * LINEAR_TO_LMS) * frame.balance.xyz) * LMS_TO_LINEAR;
    let contrasted = MIDDLE_GREY * pow(max(balanced, vec3<f32>(1e-6)) / MIDDLE_GREY, vec3<f32>(frame.grading.x));
    let luminance = dot(contrasted, vec3<f32>(0.2126, 0.7152, 0.0722));
    return max(mix(vec3<f32>(luminance), contrasted, frame.grading.y), vec3<f32>(0.0));
}

// The fog's mean density along `ray` from the camera, relative to its
// density at the base height: density falls by `e` every falloff height up,
// so the ray's mean is (e0 - e1) / (Δy / falloff) of its ends' densities.
fn height_density(ray: vec3<f32>) -> f32 {
    let falloff = frame.atmosphere.y;
    if falloff <= 0.0 {
        return 1.0;
    }
    let start = min((frame.atmosphere.x - frame.camera.y) / falloff, 80.0);
    let climb = ray.y / falloff;
    if abs(climb) < 1e-4 {
        return exp(start - climb * 0.5);
    }
    return (exp(start) - exp(min(start - climb, 80.0))) / climb;
}

// The fog's colour along `ray`: toward the haze colour by how near the ray
// looks to the sun.
fn fog_color(ray: vec3<f32>) -> vec3<f32> {
    let exponent = frame.atmosphere.z;
    if exponent <= 0.0 || frame.sun.w <= 0.0 {
        return frame.fog_color.rgb;
    }
    let toward_sun = max(dot(normalize(ray), frame.sun.xyz), 0.0);
    return mix(frame.fog_color.rgb, frame.haze.rgb, pow(toward_sun, exponent));
}

// Finish linear `rgb` seen along `ray` from the camera (zero: not fogged).
fn finish_linear(rgb: vec3<f32>, ray: vec3<f32>) -> vec3<f32> {
    var toned = graded(rgb * frame.finish.x);
    if frame.modes.x == TONE_NEUTRAL {
        toned = neutral(toned);
    } else if frame.modes.x == TONE_ACES_FILMIC {
        toned = aces_filmic(toned);
    }
    let fog = frame.modes.y;
    if fog == 0u || all(ray == vec3<f32>(0.0)) {
        return toned;
    }
    // Thinner air above the base height reads as a shorter distance.
    let distance = length(ray) * height_density(ray);
    var visibility = 1.0;
    if fog == FOG_LINEAR {
        visibility = clamp((frame.finish.z - distance) / (frame.finish.z - frame.finish.y), 0.0, 1.0);
    } else if fog == FOG_EXPONENTIAL {
        visibility = exp(-frame.finish.w * distance);
    } else if fog == FOG_EXPONENTIAL_SQUARED {
        let optical = frame.finish.w * distance;
        visibility = exp(-optical * optical);
    }
    return mix(fog_color(ray), toned, visibility);
}
