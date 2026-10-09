// The finish pass: the view's world, drawn in linear HDR and premultiplied
// by its coverage, finished (`rusty::finish::finish_linear`, fog by the
// depth buffer's distance) and composited over the background the view drew
// first. A multisampled world's edges are finished sample by sample and
// then averaged, so a bright surface's edge resolves as its finished colour
// does.
// Blended surfaces take the fog of what lies behind them; over the
// background, where the depth buffer holds nothing, they are not fogged.
// With volumetric fog (`volumetric_fog.wgsl`), each surface is first seen
// through the fog in front of it: dimmed by its transmittance and lit by the
// light it scatters toward the camera, in linear light before exposure; the
// background is seen through the grid's whole depth.

#import rusty::view::frame
#import rusty::finish::finish_linear

struct FinishParams {
    // The view's viewport in target pixels: x, y, width, height.
    viewport: vec4<f32>,
    // x: the largest value the target holds (1 for a normalized target, so
    // each sample is clamped before the samples are averaged, as a
    // multisampled resolve of the target would); yz: the target's size.
    output: vec4<f32>,
    // x: bloom intensity (0 for none); y: 1 when auto exposure scales the
    // exposure; z: sun shafts' strength (0 for none).
    post: vec4<f32>,
    // x: 1 when the view drew volumetric fog; y: the grid's reach in
    // metres; z: its depth in cells.
    fog: vec4<f32>,
};

@group(1) @binding(0) var<uniform> params: FinishParams;
@group(1) @binding(1) var scene: texture_2d<f32>;
@group(1) @binding(2) var scene_depth: texture_depth_2d;
@group(1) @binding(3) var scene_depth_multisampled: texture_depth_multisampled_2d;
@group(1) @binding(4) var scene_multisampled: texture_multisampled_2d<f32>;
// The view's bloom (`post.wgsl`), at half its viewport's resolution, and
// its sampler.
@group(1) @binding(5) var bloom: texture_2d<f32>;
@group(1) @binding(6) var bloom_sampler: sampler;
// Auto exposure's adapted value (1×1).
@group(1) @binding(7) var adapted: texture_2d<f32>;
// The view's sun shafts (`post.wgsl`), at half its viewport's resolution.
@group(1) @binding(8) var shafts: texture_2d<f32>;
// The view's volumetric fog: rgb the light scattered toward the camera, a
// the transmittance, by cell column and quadratic depth.
@group(1) @binding(9) var fog_grid: texture_3d<f32>;
@group(1) @binding(10) var fog_sampler: sampler;

// The fog between the camera and `distance` metres along the pixel at
// `position`: scattered light and transmittance. Inside the first cell it
// grows from none. A surface reads the grid a cell nearer than itself,
// so a cell it cuts through, partly behind it (and lit there, past a wall's
// shadow), does not leak in as rings.
const SURFACE_BIAS: f32 = 1.0;

fn fog_at(position: vec4<f32>, distance: f32) -> vec4<f32> {
    let uv = (position.xy - params.viewport.xy) / params.viewport.zw;
    let slices = params.fog.z;
    var depth = sqrt(clamp(distance / params.fog.y, 0.0, 1.0)) * slices;
    if distance < params.fog.y {
        depth = max(depth - SURFACE_BIAS, 0.0);
    }
    let cell = textureSampleLevel(fog_grid, fog_sampler, vec3<f32>(uv, (max(depth, 1.0) - 0.5) / slices), 0.0);
    return mix(vec4<f32>(0.0, 0.0, 0.0, 1.0), cell, clamp(depth, 0.0, 1.0));
}

@vertex
fn vs_finish(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return vec4<f32>(xy, 0.0, 1.0);
}

// One premultiplied sample at its depth, with the pixel's bloom and sun
// shafts added as light: finished and premultiplied again, that light over
// the background finished alone where the sample does not cover it.
fn finished(color: vec4<f32>, position: vec4<f32>, depth: f32, glow: vec3<f32>, exposure: f32) -> vec4<f32> {
    let ceiling = vec3<f32>(params.output.x);
    let volumetric = params.fog.x > 0.5;
    var result = vec4<f32>(0.0);
    if color.a > 0.0 {
        var ray = vec3<f32>(0.0);
        if depth < 1.0 && (frame.modes.y != 0u || volumetric) {
            let uv = (position.xy - params.viewport.xy) / params.viewport.zw;
            let world = frame.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
            ray = world.xyz / world.w - frame.camera.xyz;
        }
        var linear = color.rgb / color.a + glow;
        if volumetric && depth < 1.0 {
            let fog = fog_at(position, length(ray));
            linear = linear * fog.a + fog.rgb;
        }
        let rgb = min(finish_linear(linear * exposure, ray), ceiling);
        result = vec4<f32>(rgb * color.a, color.a);
    }
    if (params.post.x > 0.0 || params.post.z > 0.0 || volumetric) && color.a < 1.0 {
        // Over the background: the added light, and the fog's whole depth,
        // which hides the background by its opacity.
        var light = glow;
        var opacity = 0.0;
        if volumetric {
            let fog = fog_at(position, params.fog.y);
            light += fog.rgb;
            opacity = 1.0 - fog.a;
        }
        let uncovered = 1.0 - color.a;
        result += vec4<f32>(min(finish_linear(light * exposure, vec3<f32>(0.0)), ceiling) * uncovered, opacity * uncovered);
    }
    return result;
}

// The pixel's added light: its bloom and its sun shafts in the sun's
// colour (zero without either), from the view's own targets, which span its
// viewport.
fn glow_at(position: vec4<f32>) -> vec3<f32> {
    let uv = (position.xy - params.viewport.xy) / params.viewport.zw;
    var glow = vec3<f32>(0.0);
    if params.post.x > 0.0 {
        glow = textureSampleLevel(bloom, bloom_sampler, uv, 0.0).rgb * params.post.x;
    }
    if params.post.z > 0.0 {
        let rays = textureSampleLevel(shafts, bloom_sampler, uv, 0.0).r;
        glow += frame.sun_color.rgb * min(frame.sun_color.w, 1.0) * rays * params.post.z;
    }
    return glow;
}

// The exposure scale auto exposure has adapted to (1 without it).
fn adapted_exposure() -> f32 {
    if params.post.y > 0.5 {
        return textureLoad(adapted, vec2<u32>(0u), 0).r;
    }
    return 1.0;
}

@fragment
fn fs_finish(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<u32>(position.xy);
    let color = textureLoad(scene, pixel, 0);
    let glow = glow_at(position);
    let result = finished(color, position, textureLoad(scene_depth, pixel, 0), glow, adapted_exposure());
    if result.a <= 0.0 && all(result.rgb <= vec3<f32>(0.0)) {
        discard;
    }
    return result;
}

// Depths of one pixel's samples this close lie on one surface: their fog
// differs by a fraction of a pixel's.
const SAME_DEPTH: f32 = 1e-5;

@fragment
fn fs_finish_multisampled(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<u32>(position.xy);
    let glow = glow_at(position);
    let exposure = adapted_exposure();
    let samples = textureNumSamples(scene_multisampled);
    // Inside a surface every sample has its colour (and, under fog, nearly
    // its depth): one finish serves them all. Edges finish each sample.
    let fogged = frame.modes.y != 0u || params.fog.x > 0.5;
    let first = textureLoad(scene_multisampled, pixel, 0);
    let first_depth = textureLoad(scene_depth_multisampled, pixel, 0);
    var alike = true;
    for (var sample = 1u; sample < samples; sample = sample + 1u) {
        alike = alike && all(textureLoad(scene_multisampled, pixel, i32(sample)) == first);
        if fogged {
            let depth = textureLoad(scene_depth_multisampled, pixel, i32(sample));
            alike = alike && abs(depth - first_depth) <= SAME_DEPTH;
        }
    }
    var result: vec4<f32>;
    if alike {
        result = finished(first, position, first_depth, glow, exposure);
    } else {
        var sum = vec4<f32>(0.0);
        for (var sample = 0u; sample < samples; sample = sample + 1u) {
            let color = textureLoad(scene_multisampled, pixel, i32(sample));
            let depth = textureLoad(scene_depth_multisampled, pixel, i32(sample));
            sum += finished(color, position, depth, glow, exposure);
        }
        result = sum / f32(samples);
    }
    if result.a <= 0.0 && all(result.rgb <= vec3<f32>(0.0)) {
        discard;
    }
    return result;
}

// Clearing a viewport of the HDR target: transparent, at the far plane.
@vertex
fn vs_clear(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return vec4<f32>(xy, 1.0, 1.0);
}

@fragment
fn fs_clear() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}
