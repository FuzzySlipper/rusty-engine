#define_import_path rusty::lighting

// A standard metallic-roughness model (Lambert diffuse plus GGX specular,
// F0 0.04 tinted by metalness) under the pass's light rows, in linear light
// with no tone mapping; the sRGB target encodes the output.

#import rusty::types::PI
#import rusty::view::{frame, lights, shadow_maps, shadow_sampler, shadow_views, clusters}

const SHADOW_MAP_SIZE: f32 = 512.0;
// `light_clusters::CLUSTER_STRIDE`.
const CLUSTER_STRIDE: u32 = 64u;

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

// Fraction of an ambient light's sky reaching `position` through its sky
// layer (`shadows.rs`): a 5×5 PCF two texels apart, about a metre across,
// so light fades in over a cave mouth. The position moves a quarter metre
// along the normal first, off the surface it lies on.
fn sky_visibility(layer: u32, position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let clip = shadow_views[layer] * vec4<f32>(position + normal * 0.25, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || ndc.z > 1.0 || ndc.z < 0.0 {
        return 1.0;
    }
    let step = 2.0 / SHADOW_MAP_SIZE;
    var lit = 0.0;
    for (var y = -2; y <= 2; y = y + 1) {
        for (var x = -2; x <= 2; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * step;
            lit += textureSampleCompareLevel(shadow_maps, shadow_sampler, uv + offset, layer, ndc.z);
        }
    }
    return lit / 25.0;
}

// Specular reflectance of a uniform environment: Karis' analytic fit of the
// split-sum environment BRDF (no lookup texture).
fn environment_brdf(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
    let r = roughness * vec4<f32>(-1.0, -0.0275, -0.572, 0.022) + vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

// The cluster of a world position in this pass's grid: its screen tile and
// its depth slice (exponential between near and far; linear when
// orthographic).
fn fragment_cluster(world_position: vec3<f32>) -> u32 {
    let grid = frame.cluster_grid;
    let clip = frame.view_proj * vec4<f32>(world_position, 1.0);
    let ndc = clip.xyz / clip.w;
    let x = clamp(u32(max(ndc.x * 0.5 + 0.5, 0.0) * f32(grid.x)), 0u, grid.x - 1u);
    let y = clamp(u32(max(0.5 - ndc.y * 0.5, 0.0) * f32(grid.y)), 0u, grid.y - 1u);
    let near = frame.cluster_depth.x;
    let far = frame.cluster_depth.y;
    var slice: u32;
    if frame.cluster_depth.w == 1.0 {
        let distance = near + clamp(ndc.z, 0.0, 1.0) * (far - near);
        slice = u32((distance - near) / (far - near) * f32(grid.z));
    } else {
        let distance = max(clip.w, near);
        slice = u32(log(distance / near) / frame.cluster_depth.z * f32(grid.z));
    }
    slice = clamp(slice, 0u, grid.z - 1u);
    return (slice * grid.y + y) * grid.x + x;
}

// One light row's contribution to a fragment's irradiance, specular and
// environment sums (`standard_radiance`).
fn add_light(
    index: u32,
    f0: vec3<f32>,
    view: vec3<f32>,
    normal: vec3<f32>,
    world_position: vec3<f32>,
    roughness: f32,
    occlusion: f32,
    reflected: vec3<f32>,
    irradiance: ptr<function, vec3<f32>>,
    specular: ptr<function, vec3<f32>>,
    environment: ptr<function, vec3<f32>>,
) {
    let light = lights[index];
        let kind = u32(light.color_kind.w);
        let color = light.color_kind.rgb;
        if kind == 0u {
            let sky_layer = u32(light.extra.w);
            var sky = 1.0;
            if sky_layer > 0u {
                sky = sky_visibility(sky_layer - 1u, world_position, normal);
            }
            *irradiance += color * occlusion * sky;
            *environment += color * occlusion * sky;
        } else if kind == 1u {
            *irradiance += mix(light.extra.rgb, color, 0.5 * normal.y + 0.5) * occlusion;
            *environment += mix(light.extra.rgb, color, 0.5 * reflected.y + 0.5) * occlusion;
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
            *irradiance += incident;
            *specular += incident * brdf_ggx(direction, view, normal, roughness, f0);
        }
}

// Diffuse plus GGX specular from every light row of the pass, before
// emission. `occlusion` scales the ambient and hemisphere (indirect) light
// only, as does an ambient light's sky layer. Metals tint specular and lose diffuse; they reflect ambient and
// hemisphere light as a uniform environment (the hemisphere along the
// reflection), while dielectrics take that light as diffuse only.
fn standard_radiance(
    albedo: vec3<f32>,
    normal: vec3<f32>,
    world_position: vec3<f32>,
    roughness: f32,
    metalness: f32,
    occlusion: f32,
) -> vec3<f32> {
    let f0 = mix(vec3<f32>(0.04), albedo, metalness);
    let view = normalize(frame.camera.xyz - world_position);
    var irradiance = vec3<f32>(0.0);
    var specular = vec3<f32>(0.0);
    // Ambient and hemisphere light seen along the reflection.
    var environment = vec3<f32>(0.0);
    let reflected = reflect(-view, normal);
    if frame.cluster_grid.w == 1u {
        // The global list, then the fragment's cluster.
        let global_base = frame.cluster_grid.x * frame.cluster_grid.y * frame.cluster_grid.z * CLUSTER_STRIDE;
        let global_count = min(clusters[global_base], CLUSTER_STRIDE - 1u);
        for (var slot = 0u; slot < global_count; slot = slot + 1u) {
            add_light(clusters[global_base + 1u + slot], f0, view, normal, world_position, roughness,
                occlusion, reflected, &irradiance, &specular, &environment);
        }
        let base = fragment_cluster(world_position) * CLUSTER_STRIDE;
        let count = min(clusters[base], CLUSTER_STRIDE - 1u);
        for (var slot = 0u; slot < count; slot = slot + 1u) {
            add_light(clusters[base + 1u + slot], f0, view, normal, world_position, roughness,
                occlusion, reflected, &irradiance, &specular, &environment);
        }
    } else {
        for (var index = frame.counts.y; index < frame.counts.y + frame.counts.x; index = index + 1u) {
            add_light(index, f0, view, normal, world_position, roughness, occlusion, reflected,
                &irradiance, &specular, &environment);
        }
    }
    let n_dot_v = clamp(dot(normal, view), 0.0, 1.0);
    let reflection = metalness * environment * environment_brdf(f0, roughness, n_dot_v);
    return albedo * (1.0 - metalness) * irradiance / PI + specular + reflection;
}
