// The splats' screen-space Gaussian quads (`splats.rs`), back to front,
// premultiplied.

struct Projected {
    // NDC centre and depth; w unused.
    center: vec4<f32>,
    // Major and minor axes in NDC, already scaled by sqrt(2 lambda).
    axes: vec4<f32>,
    // Linear premultiplicable colour and opacity.
    color: vec4<f32>,
}


@group(0) @binding(0) var<storage, read> drawn: array<Projected>;
@group(0) @binding(1) var<storage, read> order: array<u32>;

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) offset: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_splat(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> Varyings {
    let splat = drawn[order[instance]];
    // Stop where the Gaussian falls under 1/255 of full: exp(-r^2) * a.
    let reach = sqrt(max(log(255.0 * splat.color.a), 0.0));
    let corner = (vec2<f32>(f32(vertex & 1u), f32(vertex >> 1u)) * 2.0 - 1.0) * reach;
    var out: Varyings;
    let offset = corner.x * splat.axes.xy + corner.y * splat.axes.zw;
    out.position = vec4<f32>(splat.center.xy + offset, splat.center.z, 1.0);
    out.offset = corner;
    out.color = splat.color;
    return out;
}

@fragment
fn fs_splat(in: Varyings) -> @location(0) vec4<f32> {
    let alpha = in.color.a * exp(-dot(in.offset, in.offset));
    if alpha < 1.0 / 255.0 {
        discard;
    }
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
