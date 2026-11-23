#define_import_path rusty::hiz_common

// A farthest-depth pyramid (`hiz.wgsl`, render-wgpu `occlusion.rs`) in a
// storage buffer: level 0 `size` texels, each level above half as many each
// way (at least one), the levels one after another from `starts[level]`,
// each row by row.
struct Hiz {
    size: vec2<u32>,
    levels: u32,
    pad: u32,
    starts: array<u32, 16>,
    depth: array<f32>,
};

// The size of `level` of a pyramid whose level 0 is `size`.
fn hiz_level_size(size: vec2<u32>, level: u32) -> vec2<u32> {
    return max(size >> vec2<u32>(level), vec2<u32>(1u));
}

// Where texel `at` of the level starting at `start`, `width` texels wide,
// lies in `depth`.
fn hiz_index(start: u32, width: u32, at: vec2<u32>) -> u32 {
    return start + at.y * width + at.x;
}
