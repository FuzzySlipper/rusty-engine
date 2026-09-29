// Ghost plates: the frozen source's parts re-drawn at the plate with a relief
// warp toward an anchor depth along each capture ray, textured from the
// selected sector's capture by their original capture-space position
// (ghost-plate.ts's shader patch). Unlit and opaque.

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    counts: vec4<u32>,
};

struct Part {
    model: mat4x4<f32>,
    normal0: vec4<f32>,
    normal1: vec4<f32>,
    normal2: vec4<f32>,
    color: vec4<f32>,
    emission: vec4<f32>,
};

struct Ghost {
    // Capture camera space to world, through the plate placement.
    display: mat4x4<f32>,
    // Source world to capture camera space.
    capture_view: mat4x4<f32>,
    capture_projection: mat4x4<f32>,
    // anchor depth, depth retention, capture near, capture far
    relief: vec4<f32>,
    // shell tolerance, shell mode (0 whole, 1 strict, 2 repaired),
    // mapping (0 plate-locked, 1 projective), texel size
    shell: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(1) @binding(0) var<uniform> ghost: Ghost;
@group(1) @binding(1) var capture_color: texture_2d<f32>;
@group(1) @binding(2) var capture_depth: texture_depth_2d;
@group(1) @binding(3) var capture_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // Plate-locked: the source uv, interpolated linearly on screen.
    @location(0) @interpolate(linear) locked_uv: vec2<f32>,
    // Projective: homogeneous source uv, divided per fragment.
    @location(1) projective_uv: vec3<f32>,
    @location(2) original_depth: f32,
};

@vertex
fn vs_ghost(
    @location(0) position: vec3<f32>,
    @builtin(instance_index) part: u32,
) -> VsOut {
    let original = ghost.capture_view * parts[part].model * vec4<f32>(position, 1.0);
    let depth = max(-original.z, 0.0001);
    let anchor = ghost.relief.x;
    let warped_depth = max(0.0001, anchor + ghost.relief.y * (depth - anchor));
    let warped = vec4<f32>(original.xy * (warped_depth / depth), -warped_depth, 1.0);
    var out: VsOut;
    out.clip = frame.view_proj * ghost.display * warped;
    let source = ghost.capture_projection * original;
    // Capture textures are top-left based: v runs down.
    out.locked_uv = vec2<f32>(0.5 + 0.5 * source.x / source.w, 0.5 - 0.5 * source.y / source.w);
    out.projective_uv = vec3<f32>(0.5 * (source.w + source.x), 0.5 * (source.w - source.y), source.w);
    out.original_depth = depth;
    return out;
}

fn inside(uv: vec2<f32>) -> bool {
    return all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
}

// The capture's view depth at uv, and whether anything drew there.
fn captured_depth(uv: vec2<f32>) -> vec2<f32> {
    let size = vec2<i32>(textureDimensions(capture_depth));
    let texel = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    let d = textureLoad(capture_depth, texel, 0);
    let near = ghost.relief.z;
    let far = ghost.relief.w;
    return vec2<f32>(near * far / (far - d * (far - near)), select(0.0, 1.0, d < 0.999999));
}

fn shell_agrees(uv: vec2<f32>, depth: f32) -> bool {
    if !inside(uv) {
        return false;
    }
    let sampled = captured_depth(uv);
    return sampled.y > 0.5 && abs(depth - sampled.x) <= ghost.shell.x;
}

@fragment
fn fs_ghost(in: VsOut) -> @location(0) vec4<f32> {
    var uv = in.locked_uv;
    if ghost.shell.z > 0.5 {
        uv = in.projective_uv.xy / in.projective_uv.z;
    }
    if !inside(uv) {
        discard;
    }
    let color = textureSampleLevel(capture_color, capture_sampler, uv, 0.0);
    let covered = captured_depth(uv).y > 0.5 || color.a >= 0.001;
    if !covered || color.a < 0.01 {
        discard;
    }
    if ghost.shell.y > 0.5 {
        var agrees = shell_agrees(uv, in.original_depth);
        if !agrees && ghost.shell.y > 1.5 {
            let step = ghost.shell.w;
            agrees = shell_agrees(uv + vec2<f32>(step, 0.0), in.original_depth)
                || shell_agrees(uv - vec2<f32>(step, 0.0), in.original_depth)
                || shell_agrees(uv + vec2<f32>(0.0, step), in.original_depth)
                || shell_agrees(uv - vec2<f32>(0.0, step), in.original_depth);
        }
        if !agrees {
            discard;
        }
    }
    return vec4<f32>(color.rgb, 1.0);
}
