// Precipitation around the camera (render-wgpu `precipitation.rs`): each
// drop is one instanced quad placed by its index, with no simulation. A
// drop's seed sets where it starts in a box around the camera; it falls at
// the volume's velocity and wraps within the box, which is tiled across the
// world, so drops stay put in the world as the camera moves and the density
// stays even. Streaks stretch along their velocity; flakes face the camera.
// Drops fade out toward the box's sides so the wrap never pops, are hidden
// behind the world and fade softly into it (the world's depth in group 2),
// and are not drawn where an ambient light's sky layer says the sky is
// closed overhead. With a block size (`Precipitation.grid`) a drop is hard
// blocks on a screen grid instead (`block_drop`).

#import rusty::view::{frame, projected}
#import rusty::lighting::open_sky_filtered
#ifdef MOTION
#import rusty::motion::{previous_pixel, previous_time, motion_output, target_pixel_of}
#endif

struct Precipitation {
    // xyz: the box's size (twice the radius across, twice the height up);
    // w: 0 streaks, 1 flakes.
    box_shape: vec4<f32>,
    // xyz: the velocity, metres per second; w: a streak's seconds of travel.
    velocity: vec4<f32>,
    // xyz: how far the drops have travelled, as a share of the box, for the
    // presentation time (taken in double precision on the CPU); w: a drop's
    // size in metres.
    phase: vec4<f32>,
    // rgb: linear radiance, a: alpha.
    color: vec4<f32>,
    // x: 1 when the colour adds to the frame; zw: this frame's jitter, target
    // pixels.
    params: vec4<f32>,
    // x: the side of a block, target pixels (0: smooth drops); yz: the
    // viewport's size, target pixels.
    grid: vec4<f32>,
};

@group(1) @binding(0) var<uniform> precipitation: Precipitation;
@group(2) @binding(30) var scene_depth: texture_depth_2d;
@group(2) @binding(31) var scene_depth_multisampled: texture_depth_multisampled_2d;

// A drop fades over this share of the box's half width and half height at
// its sides, and over this many metres as it nears the world behind it.
const EDGE_FADE: f32 = 0.2;
const HEIGHT_FADE: f32 = 0.5;
const SOFTNESS_METRES: f32 = 0.3;
// Drops fade in from this close to the eye to this far: a drop right at
// the eye would fill the view.
const NEAREST_METRES: f32 = 0.5;
const NEAR_FULL_METRES: f32 = 3.0;
// The most blocks across a flake and along a streak: however near the eye,
// a drop stays a few blocks.
const FLAKE_BLOCKS: f32 = 3.0;
const STREAK_BLOCKS: f32 = 10.0;
// How far in front of the eye a block drop's ends must stand to be placed.
const BLOCK_NEAREST_W: f32 = 0.05;

struct DropOut {
    @builtin(position) clip: vec4<f32>,
    // x across the drop (-1 to 1), y along it (0 tail to 1 head).
    @location(0) local: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) fade: f32,
#ifdef MOTION
    // Where the drop's corner stood in the previous frame: back along its
    // fall (`rusty::motion`).
    @location(3) previous: vec4<f32>,
#endif
    // Block drops: the fragment's place on the grid, in blocks.
    @location(4) cell: vec2<f32>,
    // A block streak's tail (xy) and head (zw) blocks.
    @location(5) @interpolate(flat) line: vec4<f32>,
};

// Three uniform values in [0, 1) for a drop's index (pcg3d).
fn drop_seed(index: u32) -> vec3<f32> {
    var v = vec3<u32>(index, index * 1664525u + 1013904223u, index ^ 0x9e3779b9u) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return vec3<f32>(v >> vec3<u32>(8u)) / 16777216.0;
}

// Whether drops draw as hard blocks.
fn blocks() -> bool {
    return precipitation.grid.x > 0.0;
}

// A clip position on the block grid, in blocks, without this frame's jitter,
// so a still drop holds its block.
fn grid_place(clip: vec4<f32>) -> vec2<f32> {
    let ndc = clip.xy / clip.w;
    let pixel = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * precipitation.grid.yz;
    return (pixel - precipitation.params.zw) / precipitation.grid.x;
}

// The lowest and highest corners of a drop's blocks, and for a streak its
// tail and head, in whole blocks. A flake is a square of blocks by its size
// on the screen. A streak runs from the block of the drop back along its
// projected travel, shortened to stand in front of the eye.
struct DropBlocks {
    low: vec2<f32>,
    high: vec2<f32>,
    line: vec4<f32>,
};

fn flake_blocks(head: vec2<f32>, position: vec3<f32>, toward: vec3<f32>) -> DropBlocks {
    var right = cross(toward, vec3<f32>(0.0, 1.0, 0.0));
    if length(right) < 1e-4 {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    let edge = grid_place(projected(position + normalize(right) * precipitation.phase.w));
    let across = clamp(round(distance(edge, head)), 1.0, FLAKE_BLOCKS);
    let low = floor(head) - floor((across - 1.0) * 0.5);
    return DropBlocks(low, low + across, vec4<f32>(0.0));
}

fn streak_blocks(head: vec2<f32>, head_clip: vec4<f32>, position: vec3<f32>) -> DropBlocks {
    let velocity = precipitation.velocity.xyz;
    var tail_clip = projected(position - velocity * precipitation.velocity.w);
    if tail_clip.w < BLOCK_NEAREST_W {
        tail_clip = mix(head_clip, tail_clip, (head_clip.w - BLOCK_NEAREST_W) / (head_clip.w - tail_clip.w));
    }
    let run = grid_place(tail_clip) - head;
    let longest = max(abs(run.x), abs(run.y));
    let steps = round(run * min((STREAK_BLOCKS - 1.0) / max(longest, 1e-4), 1.0));
    let front = floor(head);
    let back = front + steps;
    return DropBlocks(min(front, back), max(front, back) + 1.0, vec4<f32>(back, front));
}

// A drop as hard blocks: a quad over them on the screen grid at `position`'s
// depth, the same on every corner but where it lies. The block grid is the
// view's, so a drop moves a whole block at a time. Its motion is the drop's
// own, whole over the quad.
fn block_drop(uv: vec2<f32>, position: vec3<f32>, toward: vec3<f32>, fade: f32) -> DropOut {
    var out: DropOut;
    let head_clip = projected(position);
    out.world_position = position;
    if head_clip.w < BLOCK_NEAREST_W {
        // Behind the eye: outside the view on every corner.
        out.clip = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        return out;
    }
    let head = grid_place(head_clip);
    var drop: DropBlocks;
    if precipitation.box_shape.w < 0.5 {
        drop = streak_blocks(head, head_clip, position);
    } else {
        drop = flake_blocks(head, position, toward);
    }
    out.cell = mix(drop.low, drop.high, uv);
    out.line = drop.line;
    let pixel = out.cell * precipitation.grid.x / precipitation.grid.yz;
    out.clip = vec4<f32>(pixel.x * 2.0 - 1.0, 1.0 - pixel.y * 2.0, head_clip.z / head_clip.w, 1.0);
    out.fade = fade;
#ifdef MOTION
    let was = previous_pixel(position - precipitation.velocity.xyz * (frame.time.x - previous_time()));
    out.previous = was;
    if was.w > 1e-6 {
        let now = target_pixel_of(head_clip);
        let corner = target_pixel_of(out.clip);
        out.previous = vec4<f32>(was.xy / was.w + corner.xy / corner.w - now.xy / now.w, 0.0, 1.0);
    }
#endif
    return out;
}

@vertex
fn vs_drop(@builtin(vertex_index) corner: u32, @builtin(instance_index) index: u32) -> DropOut {
    let box = precipitation.box_shape.xyz;
    let camera = frame.camera.xyz;
    let low = camera - box * 0.5;
    // Where the drop is now in the world's tiling of boxes, then the copy of
    // it inside the box around the camera.
    let cell = fract(drop_seed(index) + precipitation.phase.xyz - fract(low / box));
    let position = low + cell * box;
    let from_camera = position - camera;
    let side_fade = 1.0 - smoothstep(1.0 - EDGE_FADE, 1.0, length(from_camera.xz) / (box.x * 0.5));
    let height_fade = 1.0 - smoothstep(1.0 - HEIGHT_FADE, 1.0, abs(from_camera.y) / (box.y * 0.5));
    let toward = normalize(select(from_camera, vec3<f32>(0.0, 0.0, 1.0), length(from_camera) < 1e-4));
    let size = precipitation.phase.w;
    let uv = vec2<f32>(f32(corner & 1u), f32((corner >> 1u) & 1u));
    let drop_fade = side_fade * height_fade * smoothstep(NEAREST_METRES, NEAR_FULL_METRES, length(from_camera));
    if blocks() {
        return block_drop(uv, position, toward, drop_fade);
    }
    var world = position;
    if precipitation.box_shape.w < 0.5 {
        // A streak: from where the drop was `velocity.w` seconds ago to
        // where it is, as wide as its size, turned to face the eye.
        let velocity = precipitation.velocity.xyz;
        let speed = length(velocity);
        let along = select(vec3<f32>(0.0, -1.0, 0.0), velocity / max(speed, 1e-4), speed > 1e-4);
        var across = cross(along, toward);
        if length(across) < 1e-4 {
            across = vec3<f32>(1.0, 0.0, 0.0);
        }
        across = normalize(across);
        let length_metres = max(speed * precipitation.velocity.w, size);
        world = position + across * (uv.x - 0.5) * size - along * (1.0 - uv.y) * length_metres;
    } else {
        // A flake: a square of its size facing the eye.
        var right = cross(toward, vec3<f32>(0.0, 1.0, 0.0));
        if length(right) < 1e-4 {
            right = vec3<f32>(1.0, 0.0, 0.0);
        }
        right = normalize(right);
        let up = cross(right, toward);
        world = position + (right * (uv.x - 0.5) + up * (uv.y - 0.5)) * size;
    }
    var out: DropOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
#ifdef MOTION
    out.previous = previous_pixel(world - precipitation.velocity.xyz * (frame.time.x - previous_time()));
#endif
    out.local = vec2<f32>(uv.x * 2.0 - 1.0, uv.y);
    out.world_position = position;
    out.fade = drop_fade;
    return out;
}

// Whether a block streak's stepped line misses the fragment's block: the
// line takes one block per step along its longer axis.
fn off_line(in: DropOut) -> bool {
    if !blocks() || precipitation.box_shape.w >= 0.5 {
        return false;
    }
    let cell = floor(in.cell);
    let tail = in.line.xy;
    let run = in.line.zw - tail;
    if abs(run.x) >= abs(run.y) {
        return run.x != 0.0 && cell.y != round(tail.y + run.y * (cell.x - tail.x) / run.x);
    }
    return cell.x != round(tail.x + run.x * (cell.y - tail.y) / run.y);
}

// The pixel a block takes its fade into the world at, its centre, so it
// fades whole; a smooth drop's own. Within `size`, the depth target's.
fn fade_pixel(in: DropOut, size: vec2<u32>) -> vec2<i32> {
    if !blocks() {
        return vec2<i32>(in.clip.xy);
    }
    let centre = in.clip.xy + (floor(in.cell) + 0.5 - in.cell) * precipitation.grid.x;
    return clamp(vec2<i32>(centre), vec2<i32>(0), vec2<i32>(size) - 1);
}

// The drop's colour at a pixel, faded by `fade`: a streak thin and soft
// across and brightest at its head, a flake a soft disc, a block solid;
// premultiplied when it adds to the frame.
fn drop_color(in: DropOut, fade: f32) -> vec4<f32> {
    var shape = 1.0;
    if blocks() {
        // Solid.
    } else if precipitation.box_shape.w < 0.5 {
        shape = (1.0 - in.local.x * in.local.x) * (0.35 + 0.65 * in.local.y);
    } else {
        let at = vec2<f32>(in.local.x, in.local.y * 2.0 - 1.0);
        shape = saturate(1.0 - dot(at, at));
    }
    var color = precipitation.color;
    color.a = color.a * shape * fade * in.fade;
    if precipitation.params.x > 0.5 {
        color = vec4<f32>(color.rgb * color.a, color.a);
    }
    return color;
}

// The world point at `depth` on the view's centre ray.
fn depth_point(depth: f32) -> vec3<f32> {
    let point = frame.inv_view_proj * vec4<f32>(0.0, 0.0, depth, 1.0);
    return point.xyz / point.w;
}

// Hidden behind the world, faded as it nears it.
fn world_fade(depth: f32, scene: f32) -> f32 {
    if depth > scene {
        return 0.0;
    }
    return saturate(distance(depth_point(scene), depth_point(depth)) / SOFTNESS_METRES);
}

// Under cover (an ambient light's sky layer closed overhead) no drop falls.
fn sheltered(in: DropOut) -> bool {
    return open_sky_filtered(in.world_position, vec3<f32>(0.0, 1.0, 0.0), 0) < 0.5;
}

struct DropColor {
    @location(0) color: vec4<f32>,
#ifdef MOTION
    // Blended over what lies behind by the drop's alpha.
    @location(1) motion: vec4<f32>,
#endif
};

@fragment
fn fs_drop(in: DropOut) -> DropColor {
    var fade = world_fade(in.clip.z, textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0));
    if blocks() && fade > 0.0 {
        // The world in front still cuts a block; only its fade is whole.
        fade = world_fade(in.clip.z, textureLoad(scene_depth, fade_pixel(in, textureDimensions(scene_depth)), 0));
    }
    if fade <= 0.0 || in.fade <= 0.0 || off_line(in) || sheltered(in) {
        discard;
    }
    let color = drop_color(in, fade);
    if color.a <= 0.001 {
        discard;
    }
    var out: DropColor;
    out.color = color;
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, color.a);
#endif
    return out;
}

struct DropMultisampledOut {
    @location(0) color: vec4<f32>,
#ifdef MOTION
    @location(1) motion: vec4<f32>,
#endif
    @builtin(sample_mask) mask: u32,
};

@fragment
fn fs_drop_multisampled(in: DropOut) -> DropMultisampledOut {
    if in.fade <= 0.0 || off_line(in) || sheltered(in) {
        discard;
    }
    let samples = i32(textureNumSamples(scene_depth_multisampled));
    var mask = 0u;
    var fade = 0.0;
    var covered = 0.0;
    for (var sample = 0; sample < samples; sample++) {
        let sample_fade = world_fade(in.clip.z, textureLoad(scene_depth_multisampled, vec2<i32>(in.clip.xy), sample));
        if sample_fade > 0.0 {
            mask |= 1u << u32(sample);
            fade += sample_fade;
            covered += 1.0;
        }
    }
    if covered <= 0.0 {
        discard;
    }
    var shown = fade / covered;
    if blocks() {
        shown = world_fade(in.clip.z, textureLoad(scene_depth_multisampled, fade_pixel(in, textureDimensions(scene_depth_multisampled)), 0));
    }
    let color = drop_color(in, shown);
    if color.a <= 0.001 {
        discard;
    }
    var out: DropMultisampledOut;
    out.color = color;
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, color.a);
#endif
    out.mask = mask;
    return out;
}
