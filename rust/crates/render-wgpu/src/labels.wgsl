// Billboard labels (#8827): one screen-space quad per label, placed in NDC
// on the CPU. `SCENE_DEPTH` is substituted per sample count.

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
}

@group(0) @binding(0) var label_image: texture_2d<f32>;
@group(0) @binding(1) var label_sampler: sampler;
@group(1) @binding(0) var scene_depth: SCENE_DEPTH;

@vertex
fn vs_label(@builtin(vertex_index) index: u32, instance: LabelInstance) -> LabelVarying {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    var out: LabelVarying;
    out.uv = corner;
    let position = mix(instance.rect.xy, instance.rect.zw, corner);
    out.position = vec4<f32>(position, clamp(instance.anchor.z, 0.0, 1.0), 1.0);
    if instance.anchor.w > 0.5 {
        let size = vec2<i32>(textureDimensions(scene_depth));
        let pixel = vec2<i32>(floor(instance.anchor.xy));
        if all(pixel >= vec2<i32>(0)) && all(pixel < size)
            && textureLoad(scene_depth, pixel, 0) < instance.anchor.z {
            // In front of the near plane: the whole quad clips away.
            out.position = vec4<f32>(0.0, 0.0, -1.0, 1.0);
        }
    }
    return out;
}

@fragment
fn fs_label(in: LabelVarying) -> @location(0) vec4<f32> {
    let texel = textureSample(label_image, label_sampler, in.uv);
    return vec4<f32>(texel.rgb * texel.a, texel.a);
}
