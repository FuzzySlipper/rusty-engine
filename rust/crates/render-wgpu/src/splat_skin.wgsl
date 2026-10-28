// Skinned 3D Gaussian splats (spike, `splats.rs`): skin and project every
// splat of every instance into the visible list the sort orders.

struct Rest {
    // Bind-pose centre; w = opacity (already through the sigmoid).
    position: vec4<f32>,
    // Bind-pose rotation, xyzw.
    rotation: vec4<f32>,
    // Per-axis standard deviation; w = sRGB colour as unorm4x8.
    scale: vec4<f32>,
    // Four bone indices (u16 pairs) and four unorm8 weights.
    bones: vec2<u32>,
    weights: u32,
    pad: u32,
}

struct Instance {
    // Model matrix with the eye subtracted: camera-relative world.
    model: mat4x4<f32>,
    palette: u32,
    first: u32,
    count: u32,
    pad: u32,
}

struct Frame {
    // The view with the eye at the origin, and the projection.
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    viewport: vec2<f32>,
    // Key quantization: depths [near, far] map onto `key_max..0`.
    depth_near: f32,
    depth_far: f32,
    key_max: u32,
    capacity: u32,
    pad: vec2<u32>,
}

struct Projected {
    // NDC centre and depth; w unused.
    center: vec4<f32>,
    // Major and minor axes in NDC, already scaled by sqrt(2 lambda).
    axes: vec4<f32>,
    // Linear premultiplicable colour and opacity.
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> rest: array<Rest>;
@group(0) @binding(2) var<storage, read> instances: array<Instance>;
@group(0) @binding(3) var<storage, read> palette: array<mat4x4<f32>>;
@group(0) @binding(4) var<storage, read_write> projected: array<Projected>;
@group(0) @binding(5) var<storage, read_write> keys: array<u32>;
@group(0) @binding(6) var<storage, read_write> values: array<u32>;
// [0] visible count; [4..8] the indirect draw arguments.
@group(0) @binding(7) var<storage, read_write> counter: array<atomic<u32>>;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn quat_matrix(q: vec4<f32>) -> mat3x3<f32> {
    let x = q.x; let y = q.y; let z = q.z; let w = q.w;
    return mat3x3<f32>(
        vec3<f32>(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)),
        vec3<f32>(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)),
        vec3<f32>(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)),
    );
}

fn upper3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

// One thread a splat; workgroup row y is the instance.
@compute @workgroup_size(256)
fn cs_skin(@builtin(global_invocation_id) id: vec3<u32>) {
    let instance = instances[id.y];
    if id.x >= instance.count {
        return;
    }
    let splat = rest[instance.first + id.x];
    let opacity = splat.position.w;
    if opacity < 1.0 / 255.0 {
        return;
    }

    // Linear blend skinning: the weighted palette matrices.
    let w = unpack4x8unorm(splat.weights);
    let b = vec4<u32>(
        splat.bones.x & 0xffffu, splat.bones.x >> 16u,
        splat.bones.y & 0xffffu, splat.bones.y >> 16u,
    ) + vec4<u32>(instance.palette);
    let skin = palette[b.x] * w.x + palette[b.y] * w.y + palette[b.z] * w.z + palette[b.w] * w.w;
    let to_view = frame.view * instance.model * skin;
    let t = (to_view * vec4<f32>(splat.position.xyz, 1.0)).xyz;
    // The view looks down -z.
    let depth = -t.z;
    if depth < 0.05 {
        return;
    }
    let clip = frame.projection * vec4<f32>(t, 1.0);
    let ndc = clip.xyz / clip.w;
    if any(abs(ndc.xy) > vec2<f32>(1.3)) || ndc.z < 0.0 || ndc.z > 1.0 {
        return;
    }

    // View-space covariance (R S)(R S)^T through the skinned frame.
    let m = upper3(to_view) * quat_matrix(splat.rotation) * mat3x3<f32>(
        vec3<f32>(splat.scale.x, 0.0, 0.0),
        vec3<f32>(0.0, splat.scale.y, 0.0),
        vec3<f32>(0.0, 0.0, splat.scale.z),
    );
    let sigma = m * transpose(m);

    // EWA: the projection's Jacobian at t, in pixels, y up.
    let fx = frame.projection[0][0] * frame.viewport.x * 0.5;
    let fy = frame.projection[1][1] * frame.viewport.y * 0.5;
    let j = mat3x3<f32>(
        vec3<f32>(fx / depth, 0.0, 0.0),
        vec3<f32>(0.0, fy / depth, 0.0),
        vec3<f32>(fx * t.x / (depth * depth), fy * t.y / (depth * depth), 0.0),
    );
    let cov = j * sigma * transpose(j);
    // A pixel's worth of low-pass, as 3DGS rasterizers add.
    let a = cov[0][0] + 0.3;
    let c = cov[1][1] + 0.3;
    let bxy = cov[0][1];
    let mid = 0.5 * (a + c);
    let radius = length(vec2<f32>(0.5 * (a - c), bxy));
    let lambda1 = mid + radius;
    let lambda2 = max(mid - radius, 0.1);
    var major_dir = vec2<f32>(1.0, 0.0);
    if abs(bxy) > 1e-6 || a < c {
        major_dir = normalize(vec2<f32>(bxy, lambda1 - a));
    }
    let to_ndc = 2.0 / frame.viewport;
    let major = major_dir * min(sqrt(2.0 * lambda1), 1024.0) * to_ndc;
    let minor = vec2<f32>(major_dir.y, -major_dir.x) * min(sqrt(2.0 * lambda2), 1024.0) * to_ndc;
    // Sub-pixel splats are kept: thousands of them make a surface.

    let slot = atomicAdd(&counter[0], 1u);
    if slot >= frame.capacity {
        return;
    }
    let rgb = srgb_to_linear(unpack4x8unorm(bitcast<u32>(splat.scale.w)).rgb);
    projected[slot] = Projected(vec4<f32>(ndc, 0.0), vec4<f32>(major, minor), vec4<f32>(rgb, opacity));
    // Back to front: the farthest takes the smallest key.
    let near_to_far = clamp((depth - frame.depth_near) / (frame.depth_far - frame.depth_near), 0.0, 1.0);
    keys[slot] = u32(f32(frame.key_max) * (1.0 - near_to_far));
    values[slot] = slot;
}

// The visible count clamped, and the indirect draw over it.
@compute @workgroup_size(1)
fn cs_arguments() {
    let visible = min(atomicLoad(&counter[0]), frame.capacity);
    atomicStore(&counter[0], visible);
    atomicStore(&counter[4], 4u);
    atomicStore(&counter[5], visible);
    atomicStore(&counter[6], 0u);
    atomicStore(&counter[7], 0u);
}

