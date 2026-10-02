#define_import_path rusty::finish

// The last step of everything drawn in the world: exposure, the tone mapping
// operator, then distance fog toward its colour. The background (clear
// colour or sky) does not pass through it, so a fog colour equal to the
// background fades geometry into it exactly.

#import rusty::view::frame
#import rusty::tonemap::{aces_filmic, neutral}

const TONE_NEUTRAL: u32 = 1u;
const TONE_ACES_FILMIC: u32 = 2u;
const FOG_LINEAR: u32 = 1u;
const FOG_EXPONENTIAL: u32 = 2u;
const FOG_EXPONENTIAL_SQUARED: u32 = 3u;

fn finish(color: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    var rgb = color.rgb * frame.finish.x;
    if frame.modes.x == TONE_NEUTRAL {
        rgb = neutral(rgb);
    } else if frame.modes.x == TONE_ACES_FILMIC {
        rgb = aces_filmic(rgb);
    }
    let fog = frame.modes.y;
    if fog == 0u {
        return vec4<f32>(rgb, color.a);
    }
    let distance = length(world_position - frame.camera.xyz);
    var visibility = 1.0;
    if fog == FOG_LINEAR {
        visibility = clamp((frame.finish.z - distance) / (frame.finish.z - frame.finish.y), 0.0, 1.0);
    } else if fog == FOG_EXPONENTIAL {
        visibility = exp(-frame.finish.w * distance);
    } else if fog == FOG_EXPONENTIAL_SQUARED {
        let optical = frame.finish.w * distance;
        visibility = exp(-optical * optical);
    }
    return vec4<f32>(mix(frame.fog_color.rgb, rgb, visibility), color.a);
}
