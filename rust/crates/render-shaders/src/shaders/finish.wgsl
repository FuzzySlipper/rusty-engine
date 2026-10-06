#define_import_path rusty::finish

// The finish: exposure, the tone mapping operator, then distance fog toward
// its colour. The world draws in linear HDR and the finish pass
// (`finish_pass.wgsl`) finishes each of its pixels, so a material stage's
// `finish` returns its colour unchanged.
// The background (clear colour or sky) is drawn before the world and never
// finished, so a fog colour equal to the background fades geometry into it
// exactly.

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

// Finish linear `rgb` at `distance` from the camera.
fn finish_linear(rgb: vec3<f32>, distance: f32) -> vec3<f32> {
    var toned = rgb * frame.finish.x;
    if frame.modes.x == TONE_NEUTRAL {
        toned = neutral(toned);
    } else if frame.modes.x == TONE_ACES_FILMIC {
        toned = aces_filmic(toned);
    }
    let fog = frame.modes.y;
    if fog == 0u {
        return toned;
    }
    var visibility = 1.0;
    if fog == FOG_LINEAR {
        visibility = clamp((frame.finish.z - distance) / (frame.finish.z - frame.finish.y), 0.0, 1.0);
    } else if fog == FOG_EXPONENTIAL {
        visibility = exp(-frame.finish.w * distance);
    } else if fog == FOG_EXPONENTIAL_SQUARED {
        let optical = frame.finish.w * distance;
        visibility = exp(-optical * optical);
    }
    return mix(frame.fog_color.rgb, toned, visibility);
}
