#define_import_path rusty::motion

// Motion vectors (MOTION; render-wgpu `motion.rs`): where each fragment's
// point stood in the previous frame, for a temporal upscale stage. A vertex
// stage places its vertex where it stood then (`previous_pixel`) and passes
// that on; the fragment stage writes `motion_at` to the motion target at
// location 1. Motion is in render pixels, from where the point was to where
// it is, x right and y down, without this frame's sub-pixel jitter. Its
// third channel is the reactive mask: how much of the current frame a
// temporal stage must take there, 0 where the surface's detail moves as its
// motion says, up to 1 where its history is no use (`REACTIVE_*` in
// `rusty::types`).
//
// Group 0 binds these beside `rusty::view` (render-wgpu `Layouts::frame`);
// outside MOTION they hold nothing a pass reads.

#import rusty::types::ScatterCopy
#import rusty::view::copy_place

struct Motion {
    // The previous frame's projection of a point's offset from its camera:
    // `Frame.relative_view_proj` then, without its jitter.
    previous_view_proj: mat4x4<f32>,
    // Clip space to this view's target pixels, offset by this frame's
    // jitter: homogeneous, so x and y over w are pixels.
    to_pixels: mat4x4<f32>,
    // xyz: the previous frame's camera; w: its presentation time.
    previous_camera: vec4<f32>,
};

@group(0) @binding(15) var<uniform> motion: Motion;
// Each part's model matrix in the previous frame, by part id: its
// `Part.model` while it has not moved since.
@group(0) @binding(16) var<storage, read> previous_models: array<mat4x4<f32>>;

// World point `p` in the previous frame's clip space.
fn previous_clip(p: vec3<f32>) -> vec4<f32> {
    return motion.previous_view_proj * vec4<f32>(p - motion.previous_camera.xyz, 1.0);
}

// Where a point at infinity along `direction` stood in the previous frame,
// as homogeneous target pixels: it moves only as the camera turns.
fn previous_pixel_toward(direction: vec3<f32>) -> vec4<f32> {
    return target_pixel_of(motion.previous_view_proj * vec4<f32>(direction, 0.0));
}

// Where the point `offset` from this frame's camera at `eye` stood in the
// previous frame, as homogeneous target pixels: the cameras' difference
// first, so their large coordinates cancel.
fn previous_pixel_from(eye: vec3<f32>, offset: vec3<f32>) -> vec4<f32> {
    let from_previous = eye - motion.previous_camera.xyz + offset;
    return target_pixel_of(motion.previous_view_proj * vec4<f32>(from_previous, 1.0));
}

// A previous clip position as homogeneous target pixels: what a vertex
// stage passes on for `motion_at`.
fn target_pixel_of(clip: vec4<f32>) -> vec4<f32> {
    return motion.to_pixels * clip;
}

// Where world point `p` stood in the previous frame, as homogeneous target
// pixels.
fn previous_pixel(p: vec3<f32>) -> vec4<f32> {
    return target_pixel_of(previous_clip(p));
}

// The previous frame's presentation time: the wind and falling drops are
// placed at it to find where they were.
fn previous_time() -> f32 {
    return motion.previous_camera.w;
}

// A part's model matrix in the previous frame.
fn previous_model(part: u32) -> mat4x4<f32> {
    return previous_models[part];
}

// A scatter copy's model matrix in the previous frame: its place under its
// part's previous model.
fn previous_copy_model(part: u32, scattered: ScatterCopy) -> mat4x4<f32> {
    return previous_models[part] * copy_place(scattered);
}

// The motion at a fragment (`@builtin(position)`) whose point stood at
// `previous` (`previous_pixel`, interpolated); none for a point that was
// behind the previous camera.
fn motion_at(position: vec4<f32>, previous: vec4<f32>) -> vec2<f32> {
    if previous.w <= 1e-6 {
        return vec2<f32>(0.0);
    }
    return position.xy - previous.xy / previous.w;
}

// What a pass writes to the motion target: the motion, no reactive mask,
// and in `w` whether it covers what lies behind, which a blended pass
// blends it by. A blended fragment's motion covers where it is at least
// half opaque and leaves what lies behind otherwise, so motion never mixes
// with `NO_MOTION`.
fn motion_output(position: vec4<f32>, previous: vec4<f32>, coverage: f32) -> vec4<f32> {
    return reactive_motion_output(position, previous, coverage, 0.0);
}

// The same with the reactive mask `reactive`: for a surface whose detail
// moves without motion of its own.
fn reactive_motion_output(position: vec4<f32>, previous: vec4<f32>, coverage: f32, reactive: f32) -> vec4<f32> {
    return vec4<f32>(motion_at(position, previous), reactive, step(0.5, coverage));
}
