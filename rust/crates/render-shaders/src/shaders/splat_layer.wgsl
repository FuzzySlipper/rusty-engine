#define_import_path rusty::splat_layer

// A splat run's layer as its composite reads it (render-wgpu `splats.rs`):
// the colour the run's Gaussians blended in display space, premultiplied;
// beside it the surface (1 − depth, then the motion or, in a lit run without
// motion, the view-space normal's xy, each weighted by alpha as the colour
// is); in a lit run with motion the normal's xy in a target of its own; and
// the run's parameters. `splats.wgsl` composites unlit runs and
// `splat_light.wgsl` lit ones.

@group(1) @binding(30) var layer: texture_2d<f32>;
@group(1) @binding(31) var surface_layer: texture_2d<f32>;

// A run's parameters (`splats.rs` `run_params`). For a pixel grid: `block`
// is the grid's anchor (a block corner, in target pixels), its block side
// and its palette's colours; `rect` the blocks it covers (x0, y0, x1, y1,
// the far edges exclusive). `cutoff`: x the share of a block the figure
// must cover; y a lit run's light floor; z 1 when its normal is in the
// normal target rather than the surface. For a lit run: `viewport` the
// view's (x, y, width, height in target pixels) and `view_to_world` the
// view's rotation (rows of view space to world). `colours` holds each
// palette colour as its linear value, then its sRGB one.
struct Grid {
    block: vec4<i32>,
    rect: vec4<i32>,
    cutoff: vec4<f32>,
    viewport: vec4<f32>,
    view_to_world: array<vec4<f32>, 3>,
    colours: array<vec4<f32>>,
};

@group(1) @binding(32) var<storage, read> grid: Grid;
@group(1) @binding(33) var normal_layer: texture_2d<f32>;

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((color + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}

fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let clamped = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let low = clamped * 12.92;
    let high = 1.055 * pow(clamped, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, clamped <= vec3<f32>(0.0031308));
}

// The view-space normal's xy at `pixel`, weighted by alpha.
fn normal_at(pixel: vec2<i32>) -> vec2<f32> {
    if grid.cutoff.z > 0.5 {
        return textureLoad(normal_layer, pixel, 0).xy;
    }
    return textureLoad(surface_layer, pixel, 0).yz;
}

// A view-space normal from its xy, facing the viewer (+z): what a view
// sees faces it.
fn facing_normal(xy: vec2<f32>) -> vec3<f32> {
    let length_squared = dot(xy, xy);
    if length_squared > 1.0 {
        return vec3<f32>(xy * inverseSqrt(length_squared), 0.0);
    }
    return normalize(vec3<f32>(xy, sqrt(1.0 - length_squared)));
}

// One block of a run's pixel grid summed over the layer: its colour and
// coverage, its depth and normal weighted as its colour, and how many of
// its pixels lie inside the grid's rectangle.
struct BlockSums {
    color: vec4<f32>,
    depth: f32,
    normal: vec2<f32>,
    count: f32,
    corner: vec2<i32>,
};

fn block_sums(position: vec4<f32>, lit: bool) -> BlockSums {
    let pixel = vec2<i32>(position.xy);
    let side = grid.block.z;
    let anchor = grid.block.xy;
    let corner = anchor + vec2<i32>(floor(vec2<f32>(pixel - anchor) / f32(side))) * side;
    let low = grid.rect.xy;
    let high = grid.rect.zw;
    var sums = BlockSums(vec4<f32>(0.0), 0.0, vec2<f32>(0.0), 0.0, corner);
    for (var y = 0; y < side; y++) {
        for (var x = 0; x < side; x++) {
            let at = corner + vec2<i32>(x, y);
            if all(at >= low) && all(at < high) {
                sums.color += textureLoad(layer, at, 0);
                sums.depth += textureLoad(surface_layer, at, 0).x;
                if lit {
                    sums.normal += normal_at(at);
                }
                sums.count += 1.0;
            }
        }
    }
    return sums;
}

// The palette colour nearest `display` (sRGB values) as a linear colour, or
// `color` without a palette.
fn snapped(display: vec3<f32>, color: vec3<f32>) -> vec3<f32> {
    var result = color;
    let colours = u32(grid.block.w);
    var nearest = 1e30;
    for (var index = 0u; index < colours; index++) {
        let offset = display - grid.colours[index * 2u + 1u].rgb;
        let distance = dot(offset, offset);
        if distance < nearest {
            nearest = distance;
            result = grid.colours[index * 2u].rgb;
        }
    }
    return result;
}
