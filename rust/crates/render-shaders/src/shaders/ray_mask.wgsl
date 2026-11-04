// The sun's traced shadow for a world view, one ray per pixel (render-wgpu
// `ray_shadows.rs`): from the view's opaque depth, drawn by the occlusion
// pre-pass's pipelines, each pixel's surface is found again and traced
// toward the sun (`rusty::ray`). The world pass reads the result where its
// fragment is that surface, so a pixel the world pass shades several times
// traces once. Each pixel keeps its surface's distance from the camera,
// negative where the sun is hidden, or the largest float where nothing is.
//
// `cs_occlusion` is the ambient occlusion pass of the `RayTraced` path: from
// the occlusion pre-pass's half-resolution depth, a few short rays over the
// hemisphere of each texel's surface, each taking as much light away as it
// falls short of the radius. The blur and the world pass read the result as
// they read the other paths'.

#import rusty::ray::{ray_distance, ray_noise, ray_params, ray_sun_visibility}

struct MaskParams {
    // The world from the pre-pass's clip space.
    inverse_view_proj: mat4x4<f32>,
    // xyz: the camera; w unused.
    camera: vec4<f32>,
    // xyz: toward the sun; w: the occlusion radius, metres.
    toward: vec4<f32>,
    // The viewport's origin and size, pixels.
    viewport: vec4<u32>,
};

@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var mask: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> params: MaskParams;
@group(0) @binding(3) var occlusion_out: texture_storage_2d<rgba8unorm, write>;

const NOTHING: f32 = 3.4e38;
// Rays per texel of `cs_occlusion`, and how far along its normal they start,
// as a share of the radius.
const OCCLUSION_RAYS: u32 = 2u;
const OCCLUSION_START: f32 = 0.2;
const OCCLUSION_RAY_STEP: vec2<u32> = vec2<u32>(23u, 37u);

// The world position of the depth at `pixel`'s centre.
fn surface_at(pixel: vec2<u32>, d: f32) -> vec3<f32> {
    // Clip space spans the viewport, not the whole target.
    let uv = (vec2<f32>(pixel) + 0.5 - vec2<f32>(params.viewport.xy)) / vec2<f32>(params.viewport.zw);
    let clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let world = params.inverse_view_proj * clip;
    return world.xyz / world.w;
}

// The step from `centre` to the neighbour along `axis` nearer it in depth,
// facing the positive direction: a step across an edge would bend the
// normal.
fn tangent(pixel: vec2<u32>, centre: vec3<f32>, d: f32, axis: vec2<i32>) -> vec3<f32> {
    let low = vec2<i32>(params.viewport.xy);
    let high = low + vec2<i32>(params.viewport.zw) - 1;
    let ahead = clamp(vec2<i32>(pixel) + axis, low, high);
    let behind = clamp(vec2<i32>(pixel) - axis, low, high);
    let d_ahead = textureLoad(depth, ahead, 0);
    let d_behind = textureLoad(depth, behind, 0);
    if abs(d_ahead - d) <= abs(d_behind - d) && d_ahead < 1.0 {
        return surface_at(vec2<u32>(ahead), d_ahead) - centre;
    }
    if d_behind < 1.0 {
        return centre - surface_at(vec2<u32>(behind), d_behind);
    }
    return vec3<f32>(0.0);
}

// The surface's normal at `pixel`, facing the camera.
fn surface_normal(pixel: vec2<u32>, position: vec3<f32>, d: f32, to_camera: vec3<f32>) -> vec3<f32> {
    var normal = cross(
        tangent(pixel, position, d, vec2<i32>(0, 1)),
        tangent(pixel, position, d, vec2<i32>(1, 0)),
    );
    if dot(normal, normal) < 1e-20 {
        normal = to_camera;
    }
    return normalize(select(-normal, normal, dot(normal, to_camera) >= 0.0));
}

@compute @workgroup_size(8, 8)
fn cs_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= params.viewport.zw) {
        return;
    }
    let pixel = id.xy + params.viewport.xy;
    let d = textureLoad(depth, pixel, 0);
    if d >= 1.0 {
        textureStore(mask, pixel, vec4<f32>(NOTHING));
        return;
    }
    let position = surface_at(pixel, d);
    let to_camera = params.camera.xyz - position;
    let away = length(to_camera);
    let normal = surface_normal(pixel, position, d, to_camera);
    let visible = ray_sun_visibility(pixel, position, normal, params.toward.xyz, away);
    textureStore(mask, pixel, vec4<f32>(select(-away, away, visible > 0.5)));
}

@compute @workgroup_size(8, 8)
fn cs_occlusion(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= params.viewport.zw) {
        return;
    }
    let pixel = id.xy + params.viewport.xy;
    let d = textureLoad(depth, pixel, 0);
    if d >= 1.0 {
        textureStore(occlusion_out, pixel, vec4<f32>(1.0));
        return;
    }
    let position = surface_at(pixel, d);
    let to_camera = params.camera.xyz - position;
    let away = length(to_camera);
    let normal = surface_normal(pixel, position, d, to_camera);
    let side = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normal.x) > 0.9);
    let across = normalize(cross(normal, side));
    let up = cross(normal, across);
    let radius = params.toward.w;
    // Past the surface's own triangles: a thin card's normal, read from
    // half-resolution depth, is rough, and a ray that hits at once counts as
    // full occlusion.
    let offset = max(ray_params.shape.z + ray_params.shape.w * away, OCCLUSION_START * radius);
    let origin = position + normal * offset;
    var open = 0.0;
    for (var ray = 0u; ray < OCCLUSION_RAYS; ray++) {
        // A cosine-weighted direction: the ambient light a surface sees.
        // Each ray reads the noise elsewhere in its tile.
        let noise = ray_noise(pixel + ray * OCCLUSION_RAY_STEP);
        let height = noise.x;
        let angle = 6.2831853 * noise.y;
        let lean = sqrt(height);
        let direction = across * (lean * cos(angle)) + up * (lean * sin(angle)) + normal * sqrt(1.0 - height);
        open += ray_distance(origin, direction, radius) / radius;
    }
    textureStore(occlusion_out, pixel, vec4<f32>(open / f32(OCCLUSION_RAYS)));
}
