#define_import_path rusty::wind

// The scene's wind over a vertex (the WIND feature, `lib.rs`): the part
// leans with the wind by its height above its origin, as a trunk bends, and
// each vertex flutters by its colour's alpha, as leaves and grass tips do.
// Both move with presentation time (`frame.time`), the lean phased by where
// the part stands so neighbours differ, the flutter by where the vertex is.
// The world and shadow passes call it alike, so shadows follow.

#import rusty::view::frame
#import rusty::material::material

// How far gusts shift the lean per second, and the flutter's rates.
const GUST_RATE: f32 = 0.9;
const GUST_RATE_2: f32 = 2.3;
const FLUTTER_RATE: f32 = 7.0;
const FLUTTER_RATE_2: f32 = 11.0;

// The world position displaced: `world` after the part's transform,
// `origin` the part's origin in the world, `weight` the vertex colour's
// alpha (1 without vertex colours).
fn wind_displace(world: vec3<f32>, origin: vec3<f32>, weight: f32) -> vec3<f32> {
    let strength = frame.wind.z;
    if strength <= 0.0 {
        return world;
    }
    let direction = vec3<f32>(frame.wind.x, 0.0, frame.wind.y);
    let time = frame.time.x;
    // The lean: a steady share and a gusting share that rises and falls
    // with two slow waves, phased by the part's place in the world.
    let place = dot(origin.xz, vec2<f32>(0.37, 0.61));
    let gust = 0.5 + 0.5 * sin(time * GUST_RATE + place) * sin(time * GUST_RATE_2 + place * 1.7);
    let lean = strength * mix(1.0, gust, frame.wind.w) * material.wind.x * max(world.y - origin.y, 0.0);
    // The flutter: a small circling motion, phased by where the vertex is.
    let phase = dot(world, vec3<f32>(1.3, 0.7, 1.9));
    let flutter = strength * material.wind.y * weight;
    let sway = vec3<f32>(
        sin(time * FLUTTER_RATE + phase),
        0.4 * sin(time * FLUTTER_RATE_2 + phase * 1.3),
        cos(time * FLUTTER_RATE + phase * 0.8),
    );
    return world + direction * lean + sway * flutter;
}
