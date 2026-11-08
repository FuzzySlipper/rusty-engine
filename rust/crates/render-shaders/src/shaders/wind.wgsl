#define_import_path rusty::wind

// The scene's wind over a vertex (the WIND feature, `lib.rs`): the part
// leans with the wind by its height above its origin, as a trunk bends, and
// each vertex flutters by its colour's alpha, as leaves and grass tips do.
// Both move with presentation time (`frame.time`), the lean phased by where
// the part stands so neighbours differ, the flutter by where the vertex is.
// The world and shadow passes call it alike, so shadows follow.

#import rusty::types::Part
#import rusty::view::{frame, wind_lean}
#import rusty::material::{material, albedo}

// The flutter's rates.
const FLUTTER_RATE: f32 = 7.0;
const FLUTTER_RATE_2: f32 = 11.0;

// A billboard (BILLBOARD) leaning in whole texels (its material's `parameters`,
// render-wgpu `billboard_parameters`), which the world and depth-only passes
// share so they see one shape. Its quad keeps its shape, widened on the side it
// leans by `share` of its width along `right`, and the fragment stage steps its
// rows (`billboard_row_uv`). Any other billboard leans smoothly, as a part does.
struct BillboardLean {
    position: vec3<f32>,
    uv: vec2<f32>,
    share: f32,
};

fn billboard_whole_texels() -> bool {
    return material.parameters[0].w > 0.5;
}

fn billboard_lean(row: Part, corner: vec3<f32>, uv: vec2<f32>, right: vec3<f32>) -> BillboardLean {
    var lean = BillboardLean(corner, uv, 0.0);
    if billboard_whole_texels() {
        let size = material.parameters[2].xy;
        let frame_uv = material.parameters[1];
        let scale = length(row.model[1].xyz);
        let top = vec3<f32>(frame.wind.x, 0.0, frame.wind.y)
            * (wind_lean(row.model[3].xyz, frame.time.x) * material.wind.x * size.y * scale);
        lean.share = dot(top, right) / max(size.x * scale, 1e-6);
        let across = (uv.x - frame_uv.x) / (frame_uv.z - frame_uv.x);
        let widened = across * (1.0 + abs(lean.share)) - max(-lean.share, 0.0);
        lean.position.x += (widened - across) * size.x;
        lean.uv.x = mix(frame_uv.x, frame_uv.z, widened);
    }
    return lean;
}

// A billboard's uv moved to its row's lean of `share` in whole texels, as a
// sprite's is (`effects.wgsl` `stepped`): the bottom row stands and the top takes
// the whole lean, toward the base so a row stays on the widened quad.
fn billboard_row_uv(uv: vec2<f32>, share: f32) -> vec2<f32> {
    if share == 0.0 {
        return uv;
    }
    let frame_uv = material.parameters[1];
    let size = vec2<f32>(textureDimensions(albedo));
    let span = frame_uv.zw - frame_uv.xy;
    let rows = max(abs(span.y) * size.y, 1.0);
    let lean = share * abs(span.x) * size.x;
    let rise = 1.0 - (uv.y - frame_uv.y) / span.y;
    let row = min(floor(rise * rows), rows - 1.0) / max(rows - 1.0, 1.0);
    return vec2<f32>(uv.x - sign(span.x) * trunc(row * lean) / size.x, uv.y);
}

// Where a row stepped past its frame's side leaves nothing.
fn billboard_off_frame(uv: vec2<f32>, share: f32) -> bool {
    let frame_uv = material.parameters[1];
    let across = (uv.x - frame_uv.x) / (frame_uv.z - frame_uv.x);
    return share != 0.0 && (across < 0.0 || across > 1.0);
}

// The world position displaced: `world` after the part's transform,
// `origin` the part's origin in the world, `weight` the vertex colour's
// alpha (1 without vertex colours).
fn wind_displace(world: vec3<f32>, origin: vec3<f32>, weight: f32) -> vec3<f32> {
    return wind_displace_at(world, origin, weight, frame.time.x);
}

// `wind_displace` at presentation time `time`: a motion vector's previous
// frame (`rusty::motion::previous_time`) sways where it swayed then.
fn wind_displace_at(world: vec3<f32>, origin: vec3<f32>, weight: f32, time: f32) -> vec3<f32> {
    let strength = frame.wind.z;
    if strength <= 0.0 {
        return world;
    }
    let direction = vec3<f32>(frame.wind.x, 0.0, frame.wind.y);
    let lean = wind_lean(origin, time) * material.wind.x * max(world.y - origin.y, 0.0);
    // The flutter: a small circling motion, phased by where the vertex is,
    // of world metres (in a backdrop pass, `Frame.backdrop`, its units).
    let phase = dot(world, vec3<f32>(1.3, 0.7, 1.9));
    let flutter = strength * material.wind.y * weight / select(1.0, frame.backdrop.w, frame.backdrop.w > 0.0);
    let sway = vec3<f32>(
        sin(time * FLUTTER_RATE + phase),
        0.4 * sin(time * FLUTTER_RATE_2 + phase * 1.3),
        cos(time * FLUTTER_RATE + phase * 0.8),
    );
    return world + direction * lean + sway * flutter;
}
