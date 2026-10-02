#define_import_path rusty::surface

// Where a surface samples its maps, and the normal a normal map gives it.

fn transform_uv(to_u: vec4<f32>, to_v: vec4<f32>, uv: vec2<f32>) -> vec2<f32> {
    let point = vec3<f32>(uv, 1.0);
    return vec2<f32>(dot(to_u.xyz, point), dot(to_v.xyz, point));
}

// Chunk uvs are tile coordinates in cells: repeat by the tile scale (xy)
// from the origin (zw), into the texture or its inset atlas region (xy min,
// zw max).
fn voxel_uv(uv: vec2<f32>, tile: vec4<f32>, sample_rect: vec4<f32>) -> vec2<f32> {
    let repeated = fract((uv - tile.zw) / tile.xy);
    return mix(sample_rect.xy, sample_rect.zw, repeated);
}

// The mip level a voxel surface samples its texture at, from the continuous
// tile coordinate: the wrapped uv jumps at every tile seam, where screen
// derivatives would pick the smallest level and draw a line. `size` is the
// texture's base size; the level is clamped so an atlas region keeps at
// least 4×4 texels rather than blending its neighbours.
fn voxel_lod(uv: vec2<f32>, tile: vec4<f32>, sample_rect: vec4<f32>, size: vec2<f32>) -> f32 {
    let texels = (sample_rect.zw - sample_rect.xy) * size;
    let tile_texels = (uv - tile.zw) / tile.xy * texels;
    let dx = dpdx(tile_texels);
    let dy = dpdy(tile_texels);
    let lod = 0.5 * log2(max(dot(dx, dx), dot(dy, dy)));
    return clamp(lod, 0.0, max(log2(min(texels.x, texels.y)) - 2.0, 0.0));
}

// The surface normal under a tangent-space normal map sample, with the
// tangent frame from the screen-space derivatives of the map's uv and the
// position (as glTF viewers do for a mesh without tangents): the tangent
// follows +u, the bitangent completes a right-handed frame with the normal.
fn perturb_normal(normal: vec3<f32>, world_position: vec3<f32>, uv: vec2<f32>, sample: vec3<f32>, scale: f32) -> vec3<f32> {
    let dp_dx = dpdx(world_position);
    let dp_dy = dpdy(world_position);
    let duv_dx = dpdx(uv);
    let duv_dy = dpdy(uv);
    let determinant = duv_dx.x * duv_dy.y - duv_dy.x * duv_dx.y;
    let along_u = (duv_dy.y * dp_dx - duv_dx.y * dp_dy) * sign(determinant);
    let tangent_length = length(along_u - normal * dot(normal, along_u));
    if abs(determinant) < 1e-12 || tangent_length < 1e-12 {
        return normal;
    }
    let tangent = (along_u - normal * dot(normal, along_u)) / tangent_length;
    let bitangent = cross(normal, tangent);
    let mapped = vec3<f32>((sample.xy * 2.0 - 1.0) * scale, sample.z * 2.0 - 1.0);
    return normalize(tangent * mapped.x + bitangent * mapped.y + normal * mapped.z);
}
