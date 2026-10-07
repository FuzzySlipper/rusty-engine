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

// Stochastic tiling (STOCHASTIC_TILING), after Mikkelsen's practical
// hex-tiling (JCGT 2022): the uv plane is cut into a triangle grid; each
// vertex carries a random offset and rotation of the texture, so its
// hexagonal neighbourhood shows a different patch, and a point blends its
// triangle's three vertices' patches. `turn` (the cosine and sine of each
// patch's rotation) takes uv to its texture; derivatives follow it, so
// filtering does not jump at tile edges. A turn is a vector, not a mat2x2:
// FXC (DX12) rejects a 2-row matrix inside a struct or array.
struct HexTiles {
    uv: array<vec2<f32>, 3>,
    turn: array<vec2<f32>, 3>,
    // Barycentric distance to each vertex: 1 at it, 0 on the opposite edge.
    corner: vec3<f32>,
    dx: vec2<f32>,
    dy: vec2<f32>,
}

// Grid vertices per uv unit, along one axis (2√3): about three tiles per
// texture repeat.
const HEX_GRID_SCALE: f32 = 3.4641016;
// How strongly a patch's luminance raises its share, so bright features
// stay whole rather than fading into their neighbours.
const HEX_LUMINANCE_FALLOFF: f32 = 0.6;

// Three uniform values in [0, 1) for a grid vertex (pcg3d).
fn hex_random(vertex: vec2<i32>) -> vec3<f32> {
    var v = vec3<u32>(bitcast<u32>(vertex.x), bitcast<u32>(vertex.y), 0x9e3779b9u) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return vec3<f32>(v >> vec3<u32>(8u)) / 16777216.0;
}

// `v` rotated by a patch's `turn`.
fn hex_rotate(turn: vec2<f32>, v: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(turn.x * v.x - turn.y * v.y, turn.y * v.x + turn.x * v.y);
}

fn hex_tiles(uv: vec2<f32>) -> HexTiles {
    let grid = uv * HEX_GRID_SCALE;
    let skewed = vec2<f32>(grid.x - 0.57735027 * grid.y, 1.15470054 * grid.y);
    let cell = vec2<i32>(floor(skewed));
    let local = fract(skewed);
    let remaining = 1.0 - local.x - local.y;
    // The cell's upper triangle (1) or lower (0).
    let upper = select(0.0, 1.0, remaining <= 0.0);
    let flip = 2.0 * upper - 1.0;
    let up = i32(upper);
    var tiles: HexTiles;
    tiles.corner = vec3<f32>(-remaining * flip, upper - local.y * flip, upper - local.x * flip);
    let vertices = array<vec2<i32>, 3>(cell + vec2<i32>(up, up), cell + vec2<i32>(up, 1 - up),
        cell + vec2<i32>(1 - up, up));
    tiles.dx = dpdx(uv);
    tiles.dy = dpdy(uv);
    for (var i = 0u; i < 3u; i++) {
        let vertex = vec2<f32>(vertices[i]);
        let center = vec2<f32>(vertex.x + 0.5 * vertex.y, vertex.y / 1.15470054) / HEX_GRID_SCALE;
        let random = hex_random(vertices[i]);
        let angle = random.z * 6.2831853;
        let turn = vec2<f32>(cos(angle), sin(angle));
        tiles.turn[i] = turn;
        tiles.uv[i] = hex_rotate(turn, uv - center) + center + random.xy;
    }
    return tiles;
}

// Patch `i` of `map` at `tiles`.
fn hex_patch(map: texture_2d<f32>, map_sampler: sampler, tiles: HexTiles, i: u32) -> vec4<f32> {
    let turn = tiles.turn[i];
    return textureSampleGrad(map, map_sampler, tiles.uv[i], hex_rotate(turn, tiles.dx),
        hex_rotate(turn, tiles.dy));
}

// A hex-tiled colour and the share each patch took: their corner distances
// to the `contrast` power, raised by luminance.
struct HexSample {
    color: vec4<f32>,
    shares: vec3<f32>,
}

fn hex_texture(map: texture_2d<f32>, map_sampler: sampler, tiles: HexTiles, contrast: f32) -> HexSample {
    let patches = array<vec4<f32>, 3>(hex_patch(map, map_sampler, tiles, 0u),
        hex_patch(map, map_sampler, tiles, 1u), hex_patch(map, map_sampler, tiles, 2u));
    let luma = vec3<f32>(0.299, 0.587, 0.114);
    let brightness = vec3<f32>(dot(patches[0].rgb, luma), dot(patches[1].rgb, luma),
        dot(patches[2].rgb, luma));
    let raised = mix(vec3<f32>(1.0), brightness, HEX_LUMINANCE_FALLOFF)
        * pow(tiles.corner, vec3<f32>(contrast));
    var sample: HexSample;
    sample.shares = raised / max(raised.x + raised.y + raised.z, 1e-6);
    sample.color = patches[0] * sample.shares.x + patches[1] * sample.shares.y
        + patches[2] * sample.shares.z;
    return sample;
}

// A data map (occlusion, roughness, metalness) read through the same patches
// and shares as a colour.
fn hex_data(map: texture_2d<f32>, map_sampler: sampler, tiles: HexTiles, shares: vec3<f32>) -> vec3<f32> {
    return hex_patch(map, map_sampler, tiles, 0u).rgb * shares.x
        + hex_patch(map, map_sampler, tiles, 1u).rgb * shares.y
        + hex_patch(map, map_sampler, tiles, 2u).rgb * shares.z;
}

// A tangent-space normal map read through the same patches and shares as a
// colour, as a map sample (0 to 1): each patch's tilt turned back from its
// rotation into the uv's frame (y up the image mirrors v, so the turn is the
// patch's own rotation).
fn hex_normal(map: texture_2d<f32>, map_sampler: sampler, tiles: HexTiles, shares: vec3<f32>) -> vec3<f32> {
    var blended = vec3<f32>(0.0);
    for (var i = 0u; i < 3u; i++) {
        let mapped = hex_patch(map, map_sampler, tiles, i).xyz * 2.0 - 1.0;
        blended += vec3<f32>(hex_rotate(tiles.turn[i], mapped.xy), mapped.z) * shares[i];
    }
    return blended * 0.5 + 0.5;
}
