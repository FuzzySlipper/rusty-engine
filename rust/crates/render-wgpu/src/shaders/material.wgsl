#define_import_path rusty::material

// Group 1 of the world and shadow caster passes (`Layouts::material`). Maps a
// material lacks bind white and are only read under their feature.

#import rusty::types::MaterialUniform

@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var albedo: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;
@group(1) @binding(3) var emissive_map: texture_2d<f32>;
@group(1) @binding(4) var emissive_sampler: sampler;
@group(1) @binding(5) var normal_map: texture_2d<f32>;
@group(1) @binding(6) var normal_sampler: sampler;
@group(1) @binding(7) var occlusion_map: texture_2d<f32>;
@group(1) @binding(8) var occlusion_sampler: sampler;
