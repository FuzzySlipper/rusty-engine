#define_import_path rusty::view

// Group 0 of the world, sprite and ghost passes (`Layouts::frame`); the
// shadow caster pass binds `parts`, `instances` and `shadow_views` at the
// same numbers (`Layouts::casters`).

#import rusty::types::{Frame, Part, Light}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(0) @binding(2) var<storage, read> lights: array<Light>;
// Part ids in draw order; a draw's instances index this list.
@group(0) @binding(3) var<storage, read> instances: array<u32>;
@group(0) @binding(4) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(5) var shadow_sampler: sampler_comparison;
@group(0) @binding(6) var<storage, read> shadow_views: array<mat4x4<f32>>;
