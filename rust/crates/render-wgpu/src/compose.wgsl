// Composition passes over whole-viewport triangles: clearing one viewport of
// a shared target, presenting an offscreen target into the primary output,
// and converting a linear capture into its encoded image.

struct Params {
    // Clear colour, linear RGBA.
    color: vec4<f32>,
    // x: exposure; y: 1 for ACES filmic tone mapping.
    tone: vec4<f32>,
    // x: supersampling factor (source texels per output pixel on each axis).
    factor: vec4<u32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct FullscreenOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// At depth 1, the far plane, so a clear writes the depth a pass clear would.
@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: FullscreenOut;
    out.clip = vec4<f32>(xy, 1.0, 1.0);
    out.uv = vec2<f32>(xy.x * 0.5 + 0.5, 0.5 - xy.y * 0.5);
    return out;
}

@fragment
fn fs_clear() -> @location(0) vec4<f32> {
    return params.color;
}

@fragment
fn fs_blit(in: FullscreenOut) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, in.uv);
}

// Three's ACESFilmicToneMapping (tonemapping_pars_fragment).
fn aces_filmic(input: vec3<f32>, exposure: f32) -> vec3<f32> {
    let aces_in = mat3x3<f32>(
        vec3<f32>(0.59719, 0.07600, 0.02840),
        vec3<f32>(0.35458, 0.90834, 0.13383),
        vec3<f32>(0.04823, 0.01566, 0.83777),
    );
    let aces_out = mat3x3<f32>(
        vec3<f32>(1.60475, -0.10208, -0.00327),
        vec3<f32>(-0.53108, 1.10813, -0.07276),
        vec3<f32>(-0.07367, -0.00605, 1.07602),
    );
    let color = aces_in * (input * exposure / 0.6);
    let a = color * (color + 0.0245786) - 0.000090537;
    let b = color * (0.983729 * color + 0.4329510) + 0.238081;
    return clamp(aces_out * (a / b), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Resolve factor x factor linear texels, un-premultiply the colour blended
// over the clear, tone map or scale by exposure, and write straight alpha to
// an sRGB target that encodes it.
@fragment
fn fs_convert(in: FullscreenOut) -> @location(0) vec4<f32> {
    let factor = params.factor.x;
    let base = vec2<u32>(in.clip.xy) * factor;
    var sum = vec4<f32>(0.0);
    for (var y = 0u; y < factor; y = y + 1u) {
        for (var x = 0u; x < factor; x = x + 1u) {
            sum += textureLoad(source, base + vec2<u32>(x, y), 0);
        }
    }
    let color = sum / f32(factor * factor);
    let alpha = clamp(color.a, 0.0, 1.0);
    var linear = vec3<f32>(0.0);
    if alpha > 0.0 {
        linear = color.rgb / alpha;
    }
    if params.tone.y > 0.5 {
        linear = aces_filmic(linear, params.tone.x);
    } else {
        linear = linear * params.tone.x;
    }
    return vec4<f32>(max(linear, vec3<f32>(0.0)), alpha);
}
