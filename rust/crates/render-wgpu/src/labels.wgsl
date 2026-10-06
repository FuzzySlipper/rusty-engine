// Billboard labels: one screen-space quad per label, placed in NDC
// on the CPU. `SCENE_DEPTH` and `SCENE_SAMPLES` are substituted per sample
// count of the scene's depth.

struct LabelInstance {
    // Left, top, right, bottom in NDC.
    @location(0) rect: vec4<f32>,
    // Anchor pixel x, y, anchor depth, and 1 to hide the label when the scene
    // covers its anchor (`Occluded`).
    @location(1) anchor: vec4<f32>,
}

struct LabelVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // The label's rect in target pixels: left, top, right, bottom.
    @location(1) @interpolate(flat) rect: vec4<f32>,
}

@group(0) @binding(0) var label_image: texture_2d<f32>;
@group(0) @binding(1) var label_sampler: sampler;
@group(1) @binding(0) var scene_depth: SCENE_DEPTH;

// The image is single-sample; the scene's depth has SCENE_SAMPLES samples a
// pixel at the standard positions. A label covers the samples of its pixel
// that its rect holds (and, depth-tested, that it is no farther than the
// scene at), as a quad rasterized at those samples and resolved would.
fn sample_position(index: u32) -> vec2<f32> {
    if SCENE_SAMPLES == 1u {
        return vec2<f32>(0.5, 0.5);
    }
    switch index {
        case 0u: { return vec2<f32>(0.375, 0.125); }
        case 1u: { return vec2<f32>(0.875, 0.375); }
        case 2u: { return vec2<f32>(0.125, 0.625); }
        default: { return vec2<f32>(0.625, 0.875); }
    }
}

@vertex
fn vs_label(@builtin(vertex_index) index: u32, instance: LabelInstance) -> LabelVarying {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let size = vec2<f32>(textureDimensions(scene_depth));
    var out: LabelVarying;
    // Left and top are whole pixels; the quad reaches a pixel past the
    // right and bottom so partly covered pixels there are shaded.
    let pixels = vec4<f32>(
        (instance.rect.x + 1.0) * 0.5 * size.x,
        (1.0 - instance.rect.y) * 0.5 * size.y,
        (instance.rect.z + 1.0) * 0.5 * size.x,
        (1.0 - instance.rect.w) * 0.5 * size.y,
    );
    out.rect = pixels;
    let reach = vec2<f32>(pixels.z + 1.0, pixels.w + 1.0);
    let extent = max(pixels.zw - pixels.xy, vec2<f32>(1e-6));
    out.uv = corner * (reach - pixels.xy) / extent;
    let at = mix(pixels.xy, reach, corner);
    out.position = vec4<f32>(
        at.x / size.x * 2.0 - 1.0,
        1.0 - at.y / size.y * 2.0,
        clamp(instance.anchor.z, 0.0, 1.0),
        1.0,
    );
    if instance.anchor.w > 0.5 {
        let target_size = vec2<i32>(size);
        let pixel = vec2<i32>(floor(instance.anchor.xy));
        if all(pixel >= vec2<i32>(0)) && all(pixel < target_size)
            && textureLoad(scene_depth, pixel, 0) < instance.anchor.z {
            // In front of the near plane: the whole quad clips away.
            out.position = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        }
    }
    return out;
}

// The share of the pixel's samples the label covers.
fn coverage(in: LabelVarying, depth_tested: bool) -> f32 {
    let pixel = floor(in.position.xy);
    var covered = 0u;
    for (var index = 0u; index < SCENE_SAMPLES; index = index + 1u) {
        let at = pixel + sample_position(index);
        var inside = all(at >= in.rect.xy) && all(at < in.rect.zw);
        if inside && depth_tested {
            inside = in.position.z <= textureLoad(scene_depth, vec2<i32>(pixel), index);
        }
        covered = covered + u32(inside);
    }
    return f32(covered) / f32(SCENE_SAMPLES);
}

@fragment
fn fs_label(in: LabelVarying) -> @location(0) vec4<f32> {
    let texel = textureSample(label_image, label_sampler, in.uv);
    return vec4<f32>(texel.rgb * texel.a, texel.a) * coverage(in, false);
}

// A depth-tested label also covers only the samples it is no farther than
// the scene at (its quad is at its anchor's depth).
@fragment
fn fs_label_tested(in: LabelVarying) -> @location(0) vec4<f32> {
    let texel = textureSample(label_image, label_sampler, in.uv);
    return vec4<f32>(texel.rgb * texel.a, texel.a) * coverage(in, true);
}
