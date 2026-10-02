// World pass: parts drawn with the retained material and light rows, and the
// equirectangular sky. Lighting is a standard metallic-roughness model
// with metalness 0 (Lambert diffuse plus GGX specular, F0 0.04), in linear
// light with no tone mapping; the sRGB target encodes the output.

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    // x: light count, y: first light row (world lights, then viewmodel lights)
    counts: vec4<u32>,
};

struct Part {
    model: mat4x4<f32>,
    normal0: vec4<f32>,
    normal1: vec4<f32>,
    normal2: vec4<f32>,
    color: vec4<f32>,
    emission: vec4<f32>,
};

// kind in color_kind.w: 0 ambient, 1 hemisphere, 2 directional, 3 point, 4 spot.
struct Light {
    // rgb: colour * intensity (hemisphere: sky)
    color_kind: vec4<f32>,
    // xyz: world position; w: range (0 = unbounded)
    position_range: vec4<f32>,
    // xyz: world travel direction; w: decay exponent
    direction_decay: vec4<f32>,
    // hemisphere: ground colour * intensity; spot: (cos outer, cos inner);
    // w: first shadow layer + 1, or 0 without a shadow
    extra: vec4<f32>,
};

struct MaterialUniform {
    roughness: f32,
    alpha_cutoff: f32,
    flags: u32,
    metalness: f32,
    // Voxel surface: xy tile scale, zw tile origin (cells).
    tile: vec4<f32>,
    // Voxel surface: xy sample min, zw sample max (texture uv).
    sample_rect: vec4<f32>,
    // Each texture slot's uv transform, two rows (xyz) applied to (u, v, 1):
    // identity unless a GLB material sets KHR_texture_transform.
    base_uv0: vec4<f32>,
    base_uv1: vec4<f32>,
    emissive_uv0: vec4<f32>,
    emissive_uv1: vec4<f32>,
    normal_uv0: vec4<f32>,
    normal_uv1: vec4<f32>,
    occlusion_uv0: vec4<f32>,
    occlusion_uv1: vec4<f32>,
    // x: normal map scale; y: occlusion strength (0 without an occlusion map).
    maps: vec4<f32>,
};

const FLAG_UNLIT: u32 = 1u;
const FLAG_MASK: u32 = 2u;
const FLAG_VOXEL_SURFACE: u32 = 4u;
const FLAG_NORMAL_MAP: u32 = 8u;
const PI: f32 = 3.141592653589793;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(0) @binding(2) var<storage, read> lights: array<Light>;
// Part ids in draw order; a draw's instances index this list.
@group(0) @binding(3) var<storage, read> instances: array<u32>;
@group(0) @binding(4) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(5) var shadow_sampler: sampler_comparison;
@group(0) @binding(6) var<storage, read> shadow_views: array<mat4x4<f32>>;

const SHADOW_MAP_SIZE: f32 = 512.0;
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var albedo: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;
// GLB maps; white for other materials (an emission multiplier of one, no
// occlusion), and the normal map only read under FLAG_NORMAL_MAP.
@group(1) @binding(3) var emissive_map: texture_2d<f32>;
@group(1) @binding(4) var emissive_sampler: sampler;
@group(1) @binding(5) var normal_map: texture_2d<f32>;
@group(1) @binding(6) var normal_sampler: sampler;
@group(1) @binding(7) var occlusion_map: texture_2d<f32>;
@group(1) @binding(8) var occlusion_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) part: u32,
    @location(4) color: vec4<f32>,
};

@vertex
fn vs_world(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @builtin(instance_index) instance: u32,
) -> VsOut {
    let part = instances[instance];
    let row = parts[part];
    let world = row.model * vec4<f32>(position, 1.0);
    var out: VsOut;
    out.clip = frame.view_proj * world;
    out.world_position = world.xyz;
    out.normal = mat3x3<f32>(row.normal0.xyz, row.normal1.xyz, row.normal2.xyz) * normal;
    out.uv = uv;
    out.part = part;
    out.color = color;
    return out;
}

fn distance_attenuation(distance: f32, range: f32, decay: f32) -> f32 {
    var falloff = 1.0 / max(pow(distance, decay), 0.01);
    if range > 0.0 {
        let ratio = distance / range;
        let window = clamp(1.0 - ratio * ratio * ratio * ratio, 0.0, 1.0);
        falloff = falloff * window * window;
    }
    return falloff;
}

fn brdf_ggx(light: vec3<f32>, view: vec3<f32>, normal: vec3<f32>, roughness: f32, f0: vec3<f32>) -> vec3<f32> {
    let alpha = roughness * roughness;
    let half_vector = normalize(light + view);
    let n_dot_l = clamp(dot(normal, light), 0.0, 1.0);
    let n_dot_v = clamp(dot(normal, view), 0.0, 1.0);
    let n_dot_h = clamp(dot(normal, half_vector), 0.0, 1.0);
    let v_dot_h = clamp(dot(view, half_vector), 0.0, 1.0);
    let fresnel_weight = exp2((-5.55473 * v_dot_h - 6.98316) * v_dot_h);
    let fresnel = f0 * (1.0 - fresnel_weight) + vec3<f32>(fresnel_weight);
    let a2 = alpha * alpha;
    let gv = n_dot_l * sqrt(a2 + (1.0 - a2) * n_dot_v * n_dot_v);
    let gl = n_dot_v * sqrt(a2 + (1.0 - a2) * n_dot_l * n_dot_l);
    let visibility = 0.5 / max(gv + gl, 1e-6);
    let denominator = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    let distribution = a2 / (PI * denominator * denominator);
    return fresnel * visibility * distribution;
}

// Point light faces: +X, -X, +Y, -Y, +Z, -Z (shadows.rs CUBE_FACES).
fn point_face(to_fragment: vec3<f32>) -> u32 {
    let a = abs(to_fragment);
    if a.x >= a.y && a.x >= a.z {
        return select(1u, 0u, to_fragment.x > 0.0);
    }
    if a.y >= a.z {
        return select(3u, 2u, to_fragment.y > 0.0);
    }
    return select(5u, 4u, to_fragment.z > 0.0);
}

// Fraction of a light reaching `position` through shadow layer `layer`:
// 3×3 PCF over linearly filtered comparisons. Outside the map is lit.
fn shadow_visibility(layer: u32, position: vec3<f32>) -> f32 {
    let clip = shadow_views[layer] * vec4<f32>(position, 1.0);
    if clip.w <= 0.0 {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || ndc.z > 1.0 {
        return 1.0;
    }
    let texel = 1.0 / SHADOW_MAP_SIZE;
    var lit = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            lit += textureSampleCompareLevel(shadow_maps, shadow_sampler, uv + offset, layer, ndc.z);
        }
    }
    return lit / 9.0;
}

fn transform_uv(row0: vec4<f32>, row1: vec4<f32>, uv: vec2<f32>) -> vec2<f32> {
    let point = vec3<f32>(uv, 1.0);
    return vec2<f32>(dot(row0.xyz, point), dot(row1.xyz, point));
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

// MeshStandardMaterial with metalness 0 under the pass's light rows: diffuse
// plus GGX specular, before emission. `occlusion` scales the ambient and
// hemisphere (indirect) light only.
fn standard_radiance(
    albedo: vec3<f32>,
    normal: vec3<f32>,
    world_position: vec3<f32>,
    roughness: f32,
    metalness: f32,
    occlusion: f32,
) -> vec3<f32> {
    // MeshStandardMaterial: metals tint specular and lose diffuse.
    let f0 = mix(vec3<f32>(0.04), albedo, metalness);
    let view = normalize(frame.camera.xyz - world_position);
    var irradiance = vec3<f32>(0.0);
    var specular = vec3<f32>(0.0);
    for (var index = frame.counts.y; index < frame.counts.y + frame.counts.x; index = index + 1u) {
        let light = lights[index];
        let kind = u32(light.color_kind.w);
        let color = light.color_kind.rgb;
        if kind == 0u {
            irradiance += color * occlusion;
        } else if kind == 1u {
            irradiance += mix(light.extra.rgb, color, 0.5 * normal.y + 0.5) * occlusion;
        } else {
            var direction = -normalize(light.direction_decay.xyz);
            var attenuation = 1.0;
            if kind != 2u {
                let to_light = light.position_range.xyz - world_position;
                let distance = length(to_light);
                direction = to_light / max(distance, 1e-6);
                attenuation = distance_attenuation(distance, light.position_range.w, light.direction_decay.w);
                if kind == 4u {
                    let angle = dot(-direction, normalize(light.direction_decay.xyz));
                    attenuation = attenuation * smoothstep(light.extra.x, light.extra.y, angle);
                }
            }
            let shadow = u32(light.extra.w);
            if shadow > 0u {
                var layer = shadow - 1u;
                if kind == 3u {
                    layer += point_face(world_position - light.position_range.xyz);
                }
                attenuation = attenuation * shadow_visibility(layer, world_position);
            }
            let incident = color * attenuation * clamp(dot(normal, direction), 0.0, 1.0);
            irradiance += incident;
            specular += incident * brdf_ggx(direction, view, normal, roughness, f0);
        }
    }
    return albedo * (1.0 - metalness) * irradiance / PI + specular;
}

@fragment
fn fs_world(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let row = parts[in.part];
    var uv = in.uv;
    if (material.flags & FLAG_VOXEL_SURFACE) != 0u {
        // Chunk uvs are tile coordinates in cells: repeat by the tile scale
        // from the origin, into the texture or its inset atlas region.
        let repeated = fract((uv - material.tile.zw) / material.tile.xy);
        uv = mix(material.sample_rect.xy, material.sample_rect.zw, repeated);
    }
    let base = row.color * in.color
        * textureSample(albedo, albedo_sampler, transform_uv(material.base_uv0, material.base_uv1, uv));
    // Every map is sampled, and every derivative taken, before any branch.
    let emissive = textureSample(emissive_map, emissive_sampler,
        transform_uv(material.emissive_uv0, material.emissive_uv1, in.uv)).rgb;
    let occlusion_sample = textureSample(occlusion_map, occlusion_sampler,
        transform_uv(material.occlusion_uv0, material.occlusion_uv1, in.uv)).r;
    let normal_uv = transform_uv(material.normal_uv0, material.normal_uv1, in.uv);
    let normal_sample = textureSample(normal_map, normal_sampler, normal_uv).rgb;
    let geometric = normalize(in.normal);
    var normal = geometric;
    if (material.flags & FLAG_NORMAL_MAP) != 0u {
        normal = perturb_normal(geometric, in.world_position, normal_uv, normal_sample, material.maps.x);
    }
    var facing = geometric;
    if !front {
        normal = -normal;
        facing = -facing;
    }
    let normal_change = max(abs(dpdx(facing)), abs(dpdy(facing)));
    let geometry_roughness = max(max(normal_change.x, normal_change.y), normal_change.z);
    if (material.flags & FLAG_MASK) != 0u && base.a < material.alpha_cutoff {
        discard;
    }
    if (material.flags & FLAG_UNLIT) != 0u {
        return base;
    }
    let roughness = min(max(material.roughness, 0.0525) + geometry_roughness, 1.0);
    let occlusion = 1.0 + material.maps.y * (occlusion_sample - 1.0);
    let radiance = standard_radiance(base.rgb, normal, in.world_position, roughness, material.metalness, occlusion)
        + row.emission.rgb * emissive;
    return vec4<f32>(radiance, base.a);
}

struct SkyUniform {
    // x: blend amount toward the second panorama
    amount: vec4<f32>,
};

@group(1) @binding(0) var<uniform> sky: SkyUniform;
@group(1) @binding(1) var sky_first: texture_2d<f32>;
@group(1) @binding(2) var sky_second: texture_2d<f32>;
@group(1) @binding(3) var sky_first_sampler: sampler;
@group(1) @binding(4) var sky_second_sampler: sampler;

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: SkyOut;
    out.clip = vec4<f32>(xy, 1.0, 1.0);
    out.ndc = xy;
    return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let far = frame.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let direction = normalize(far.xyz / far.w - frame.camera.xyz);
    let uv = vec2<f32>(
        atan2(direction.z, direction.x) / (2.0 * PI) + 0.5,
        0.5 - asin(clamp(direction.y, -1.0, 1.0)) / PI,
    );
    let first = textureSample(sky_first, sky_first_sampler, uv).rgb;
    let second = textureSample(sky_second, sky_second_sampler, uv).rgb;
    return vec4<f32>(mix(first, second, sky.amount.x), 1.0);
}
