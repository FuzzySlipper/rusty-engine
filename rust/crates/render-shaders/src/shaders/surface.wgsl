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

// A voxel surface texture at tile coordinates `uv`, through `tile` and
// `sample_rect` as `voxel_uv` and `voxel_lod` place it.
fn tiled_texture(map: texture_2d<f32>, map_sampler: sampler, uv: vec2<f32>, tile: vec4<f32>, sample_rect: vec4<f32>) -> vec4<f32> {
    return textureSampleLevel(map, map_sampler, voxel_uv(uv, tile, sample_rect),
        voxel_lod(uv, tile, sample_rect, vec2<f32>(textureDimensions(map, 0))));
}

// Terrain layer shares from a mesh's layer weights: each to the contrast
// power, normalized; all on layer 0 without any weight. Dividing by the
// largest weight first keeps the largest raised share 1, so a high contrast
// cannot shrink the total away; an absent layer stays absent.
fn layer_shares(weights: vec4<f32>, contrast: f32) -> vec4<f32> {
    let present = max(weights, vec4<f32>(0.0));
    let largest = max(max(present.x, present.y), max(present.z, present.w));
    if largest <= 0.0 {
        return vec4<f32>(1.0, 0.0, 0.0, 0.0);
    }
    let raised = pow(present / largest, vec4<f32>(contrast));
    return raised / (raised.x + raised.y + raised.z + raised.w);
}

// A tangent-space normal map sample (x right, y up the image, z out; glTF)
// in the frame of `tangent`, `bitangent` and `normal`.
fn mapped_normal(normal: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>, sample: vec3<f32>, scale: f32) -> vec3<f32> {
    let mapped = vec3<f32>((sample.xy * 2.0 - 1.0) * scale, sample.z * 2.0 - 1.0);
    return normalize(tangent * mapped.x + bitangent * mapped.y + normal * mapped.z);
}

// The surface normal under a normal map sample, with the tangent frame from
// the screen-space derivatives of the map's uv and the position (as glTF
// viewers do for a mesh without tangents). The tangent follows +u; the
// bitangent points up the image (-v), so a mirrored uv layout flips it.
fn perturb_normal(normal: vec3<f32>, world_position: vec3<f32>, uv: vec2<f32>, sample: vec3<f32>, scale: f32) -> vec3<f32> {
    let dp_dx = dpdx(world_position);
    let dp_dy = dpdy(world_position);
    let duv_dx = dpdx(uv);
    let duv_dy = dpdy(uv);
    let determinant = duv_dx.x * duv_dy.y - duv_dy.x * duv_dx.y;
    let along_u = (duv_dy.y * dp_dx - duv_dx.y * dp_dy) * sign(determinant);
    let along_v = (duv_dx.x * dp_dy - duv_dy.x * dp_dx) * sign(determinant);
    let tangent_length = length(along_u - normal * dot(normal, along_u));
    if abs(determinant) < 1e-12 || tangent_length < 1e-12 {
        return normal;
    }
    let tangent = (along_u - normal * dot(normal, along_u)) / tangent_length;
    let crossed = cross(normal, tangent);
    let bitangent = crossed * select(-1.0, 1.0, dot(crossed, along_v) <= 0.0);
    return mapped_normal(normal, tangent, bitangent, sample, scale);
}

// The surface normal under a normal map sample in the mesh's own tangent
// frame: glTF's bitangent is cross(normal, tangent) times its handedness w.
fn tangent_normal(normal: vec3<f32>, tangent: vec4<f32>, sample: vec3<f32>, scale: f32) -> vec3<f32> {
    let along = tangent.xyz - normal * dot(normal, tangent.xyz);
    let length_along = length(along);
    if length_along < 1e-12 {
        return normal;
    }
    let unit = along / length_along;
    return mapped_normal(normal, unit, cross(normal, unit) * tangent.w, sample, scale);
}

// A triplanar material's planes at texture position `p` (cells on voxel
// meshes) and texture-space normal `n`: the x, y and z planes' uvs in the
// texture bases of the cube faces `n` points toward (svc-mesh
// `voxel_surface_texture_basis`), so an axis-aligned face matches its tile
// coordinates.
fn triplanar_uvs(p: vec3<f32>, n: vec3<f32>) -> array<vec2<f32>, 3> {
    let side = select(vec3<f32>(-1.0), vec3<f32>(1.0), n >= vec3<f32>(0.0));
    return array<vec2<f32>, 3>(
        vec2<f32>(p.z * side.x, -p.y),
        vec2<f32>(p.z, p.x * side.y),
        vec2<f32>(-p.x * side.z, -p.y),
    );
}

// Each plane's share: |n| to the material's sharpness, normalized.
fn triplanar_weights(n: vec3<f32>, sharpness: f32) -> vec3<f32> {
    let weights = pow(abs(n), vec3<f32>(sharpness));
    return weights / max(weights.x + weights.y + weights.z, 1e-6);
}

// The texture-space normal under a triplanar normal map: each plane's sample
// in its face's frame (tangent along u, bitangent up the image, -v), blended
// by the plane weights. A flat map leaves `n`.
fn triplanar_normal(n: vec3<f32>, samples: array<vec3<f32>, 3>, weights: vec3<f32>, scale: f32) -> vec3<f32> {
    let side = select(vec3<f32>(-1.0), vec3<f32>(1.0), n >= vec3<f32>(0.0));
    let x = mapped_normal(n, vec3<f32>(0.0, 0.0, side.x), vec3<f32>(0.0, 1.0, 0.0), samples[0], scale);
    let y = mapped_normal(n, vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(-side.y, 0.0, 0.0), samples[1], scale);
    let z = mapped_normal(n, vec3<f32>(-side.z, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), samples[2], scale);
    return normalize(x * weights.x + y * weights.y + z * weights.z);
}
