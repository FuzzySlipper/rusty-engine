// A lit splat run's composite (render-wgpu `splats.rs`, #10815): the run's
// layer lit per pixel by the world's lights (`rusty::lighting`
// `standard_radiance`: the sun through its cascades, clustered lights, the
// ambient light, the sky and probes) at the position its blended depth
// gives and the normal its Gaussians' normals blend to, the baked colour
// taken as the albedo, diffuse only. Every Gaussian along a ray is lit
// alike. A light floor keeps a share of the baked colour however dark the
// light. A pixel grid's block is averaged, lit once at its average, then
// snapped to its palette. Its vertex stage is `splats.wgsl` `vs_composite`.

#import rusty::view::frame
#import rusty::lighting::standard_radiance
#import rusty::types::REACTIVE_OVERLAY
#import rusty::splat_layer::{layer, surface_layer, grid, srgb_to_linear, linear_to_srgb, normal_at, facing_normal, block_sums, snapped}

struct CompositeVarying {
    @builtin(position) position: vec4<f32>,
};

// The world position at target pixel `pixel` (its centre) and NDC depth.
fn world_at(pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let uv = (pixel - grid.viewport.xy) / grid.viewport.zw;
    let world = frame.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    return world.xyz / world.w;
}

// A view-space normal turned to the world.
fn world_normal(view: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(
        dot(grid.view_to_world[0].xyz, view),
        dot(grid.view_to_world[1].xyz, view),
        dot(grid.view_to_world[2].xyz, view),
    ));
}

// `color` (linear, the baked colour as albedo) lit at `position` facing
// `normal`, kept at least the floor's share of itself.
fn lit(color: vec3<f32>, normal: vec3<f32>, position: vec3<f32>) -> vec3<f32> {
    let radiance = standard_radiance(color, normal, position, 1.0, 0.0, 1.0);
    return max(radiance, color * grid.cutoff.y);
}

// The layer at the fragment lit, premultiplied by its coverage.
fn lit_composite(position: vec4<f32>) -> vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    let layered = textureLoad(layer, pixel, 0);
    if layered.a <= 0.0 {
        discard;
    }
    let display = clamp(layered.rgb / layered.a, vec3<f32>(0.0), vec3<f32>(1.0));
    let depth = 1.0 - textureLoad(surface_layer, pixel, 0).x / layered.a;
    let normal = world_normal(facing_normal(normal_at(pixel) / layered.a));
    let color = lit(srgb_to_linear(display), normal, world_at(position.xy, depth));
    return vec4<f32>(color * layered.a, layered.a);
}

@fragment
fn fs_composite_lit(in: CompositeVarying) -> @location(0) vec4<f32> {
    return lit_composite(in.position);
}

struct ReactiveComposite {
    @location(0) color: vec4<f32>,
    @location(1) motion: vec4<f32>,
};

@fragment
fn fs_composite_lit_reactive(in: CompositeVarying) -> ReactiveComposite {
    let color = lit_composite(in.position);
    return ReactiveComposite(color, vec4<f32>(0.0, 0.0, REACTIVE_OVERLAY, color.a));
}

struct Block {
    // Linear, opaque.
    color: vec3<f32>,
    depth: f32,
};

// The block under the fragment: the layer's coverage-weighted average
// colour, depth and normal, lit once at the block's centre, then snapped
// to the palette by its sRGB value. None (discarded) under the cutoff.
fn lit_block(position: vec4<f32>) -> Block {
    let sums = block_sums(position, true);
    if sums.color.a <= 0.0 || sums.color.a < grid.cutoff.x * sums.count {
        discard;
    }
    let display = clamp(sums.color.rgb / sums.color.a, vec3<f32>(0.0), vec3<f32>(1.0));
    let depth = 1.0 - sums.depth / sums.color.a;
    let centre = vec2<f32>(sums.corner) + 0.5 * f32(grid.block.z);
    let normal = world_normal(facing_normal(sums.normal / sums.color.a));
    let color = lit(srgb_to_linear(display), normal, world_at(centre, depth));
    return Block(snapped(linear_to_srgb(color), color), depth);
}

struct DepthComposite {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
};

@fragment
fn fs_composite_grid_lit(in: CompositeVarying) -> DepthComposite {
    let block = lit_block(in.position);
    return DepthComposite(block.depth, vec4<f32>(block.color, 1.0));
}

struct MotionComposite {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
    @location(1) motion: vec4<f32>,
};

@fragment
fn fs_composite_grid_lit_motion(in: CompositeVarying) -> MotionComposite {
    let block = lit_block(in.position);
    return MotionComposite(
        block.depth,
        vec4<f32>(block.color, 1.0),
        vec4<f32>(0.0, 0.0, REACTIVE_OVERLAY, 1.0),
    );
}
