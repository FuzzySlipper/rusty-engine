#define_import_path rusty::tonemap

// Tone mapping operators over linear colour, before the target encodes it.

// ACES filmic (Stephen Hill's RRT and ODT fit), with three.js's exposure
// normalisation (input / 0.6).
fn aces_filmic(input: vec3<f32>) -> vec3<f32> {
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
    let color = aces_in * (input / 0.6);
    let a = color * (color + 0.0245786) - 0.000090537;
    let b = color * (0.983729 * color + 0.4329510) + 0.238081;
    return clamp(aces_out * (a / b), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Khronos PBR Neutral: colours below the compression start keep their hue
// and value (less a small toe offset); highlights compress toward white.
fn neutral(input: vec3<f32>) -> vec3<f32> {
    let start_compression = 0.8 - 0.04;
    let desaturation = 0.15;
    let lowest = min(input.r, min(input.g, input.b));
    let offset = select(0.04, lowest - 6.25 * lowest * lowest, lowest < 0.08);
    let color = input - offset;
    let peak = max(color.r, max(color.g, color.b));
    if peak < start_compression {
        return color;
    }
    let d = 1.0 - start_compression;
    let new_peak = 1.0 - d * d / (peak + d - start_compression);
    let compressed = color * (new_peak / peak);
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(compressed, vec3<f32>(new_peak), g);
}
