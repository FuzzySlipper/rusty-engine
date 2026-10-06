#define_import_path rusty::lighting

// A standard metallic-roughness model (Lambert diffuse plus GGX specular,
// F0 0.04 tinted by metalness) under the pass's light rows, in linear light
// with no tone mapping; the sRGB target encodes the output.

#import rusty::types::PI
#import rusty::view::{frame, lights, shadow_maps, shadow_sampler, shadow_views, clusters, sky_specular, sky_sampler, sky_irradiance, probes, probes_sampler}

// The shadow atlas page's side in texels (`shadows.rs` PAGE_SIZE).
const SHADOW_PAGE_SIZE: f32 = 2048.0;
// A directional light's cascades (`shadows.rs`).
const CASCADES: u32 = 4u;
// Over the last tenth of each cascade, blend into the next; past the last,
// out to no shadow.
const CASCADE_BLEND: f32 = 0.1;
// How far along its normal a receiver looks up a layer, in that layer's
// texels at the receiver: a texel spans more depth than a surface's offset
// to its back faces, so without it lit surfaces shadow themselves.
const SHADOW_NORMAL_OFFSET: f32 = 1.5;

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

// Where a receiver looks shadow layer `layer` up: moved along its normal by
// SHADOW_NORMAL_OFFSET of the layer's texels at that point (a texel's width
// grows with the view's w over its x scale), then mapped into the layer's
// tile. `valid` is false outside the layer's view, which is lit.
struct ShadowLookup {
    valid: bool,
    uv: vec2<f32>,
    // The tile's texel centres the filter stays between.
    low: vec2<f32>,
    high: vec2<f32>,
    page: u32,
    depth: f32,
    soft: bool,
};

fn shadow_lookup(layer: u32, position: vec3<f32>, normal: vec3<f32>) -> ShadowLookup {
    let view = shadow_views[layer];
    let x_scale = length(vec3<f32>(view.view_proj[0].x, view.view_proj[1].x, view.view_proj[2].x));
    let at = view.view_proj * vec4<f32>(position, 1.0);
    let texel = 2.0 * at.w / (x_scale * view.params.y);
    let clip = view.view_proj * vec4<f32>(position + normal * texel * SHADOW_NORMAL_OFFSET, 1.0);
    var lookup: ShadowLookup;
    lookup.valid = false;
    if clip.w <= 0.0 {
        return lookup;
    }
    let ndc = clip.xyz / clip.w;
    let local = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if any(local < vec2<f32>(0.0)) || any(local > vec2<f32>(1.0)) || ndc.z > 1.0 || ndc.z < 0.0 {
        return lookup;
    }
    let half_texel = 0.5 / SHADOW_PAGE_SIZE;
    lookup.valid = true;
    lookup.uv = view.tile.xy + local * view.tile.z;
    lookup.low = view.tile.xy + vec2<f32>(half_texel);
    lookup.high = view.tile.xy + vec2<f32>(view.tile.z - half_texel);
    lookup.page = u32(view.tile.w);
    lookup.depth = ndc.z;
    lookup.soft = view.params.x > 0.5;
    return lookup;
}

// Linearly filtered comparisons over a square of (2 radius + 1)² texels,
// `spacing` texels apart, kept inside the tile.
fn shadow_filter(lookup: ShadowLookup, radius: i32, spacing: f32) -> f32 {
    let step = spacing / SHADOW_PAGE_SIZE;
    var lit = 0.0;
    for (var y = -radius; y <= radius; y = y + 1) {
        for (var x = -radius; x <= radius; x = x + 1) {
            let uv = clamp(lookup.uv + vec2<f32>(f32(x), f32(y)) * step, lookup.low, lookup.high);
            lit += textureSampleCompareLevel(shadow_maps, shadow_sampler, uv, lookup.page, lookup.depth);
        }
    }
    let side = f32(2 * radius + 1);
    return lit / (side * side);
}

// Fraction of a light reaching `position` through shadow layer `layer`: a
// 3×3 PCF, or 5×5 for a soft light. Outside the layer's view is lit.
fn shadow_visibility(layer: u32, position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let lookup = shadow_lookup(layer, position, normal);
    if !lookup.valid {
        return 1.0;
    }
    return shadow_filter(lookup, select(1, 2, lookup.soft), 1.0);
}

// Fraction of a directional light reaching `position` through its cascades
// from layer `first`, picked by the view depth along `forward` (the axis the
// cascades were fitted to); `splits` holds each cascade's far depth.
fn cascade_visibility(
    first: u32,
    position: vec3<f32>,
    normal: vec3<f32>,
    splits: vec4<f32>,
    forward: vec3<f32>,
) -> f32 {
    let depth = dot(position - frame.camera.xyz, forward);
    var start = 0.0;
    for (var index = 0u; index < CASCADES; index = index + 1u) {
        let end = splits[index];
        if depth <= end {
            var visibility = shadow_visibility(first + index, position, normal);
            let blend = end - (end - start) * CASCADE_BLEND;
            if depth > blend {
                var next = 1.0;
                if index + 1u < CASCADES {
                    next = shadow_visibility(first + index + 1u, position, normal);
                }
                visibility = mix(visibility, next, (depth - blend) / (end - blend));
            }
            return visibility;
        }
        start = end;
    }
    return 1.0;
}

// Fraction of an ambient light's sky reaching `position` through its sky
// layer (`shadows.rs`): a 5×5 PCF two texels apart, about a metre across, so
// light fades in over a cave mouth.
fn sky_visibility(layer: u32, position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let lookup = shadow_lookup(layer, position, normal);
    if !lookup.valid {
        return 1.0;
    }
    return shadow_filter(lookup, 2, 2.0);
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
// What the indirect light volume (render-wgpu `probes.rs`) gives a fragment:
// irradiance along its normal and along its reflection, how much the volume
// covers it (fading to nothing a cell beyond its edge), and the shares the
// ambient rows and the hemisphere, sky and other outside light keep.
struct ProbeLight {
    diffuse: vec3<f32>,
    reflected: vec3<f32>,
    coverage: f32,
    // The ambient rows' share: 1 - coverage where the probes see the sky, 1
    // where the ambient light is a floor the probes add to.
    ambient: f32,
    // The hemisphere rows' and the sky light's share: 1 - coverage.
    outside: f32,
};

// In probe spacings: how far along the normal the volume is sampled.
const PROBE_NORMAL_OFFSET: f32 = 0.3;

// `cell` is the sample in probe cells from the first probe's centre plus a
// half, within one slab; `dims` is probes per axis. `compact` reads the
// one-slab encoding a software adapter uploads: the ambient coefficient's
// colour with the vertical coefficient's luminance, which takes the
// ambient's hue; one filtered read instead of three, and no horizontal
// direction.
fn probe_sh(cell: vec3<f32>, dims: vec3<f32>, d: vec3<f32>, compact: bool) -> vec3<f32> {
    let slab = vec3<f32>(0.0, 0.0, dims.z);
    let basis = vec4<f32>(0.282095, 0.488603 * d.y, 0.488603 * d.z, 0.488603 * d.x);
    if compact {
        let a = textureSampleLevel(probes, probes_sampler, cell / dims, 0.0);
        let luminance = max(dot(a.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-4);
        return max(a.rgb * (basis.x + a.w * basis.y / luminance), vec3<f32>(0.0));
    }
    let size = vec3<f32>(dims.x, dims.y, 3.0 * dims.z);
    let r = textureSampleLevel(probes, probes_sampler, cell / size, 0.0);
    let g = textureSampleLevel(probes, probes_sampler, (cell + slab) / size, 0.0);
    let b = textureSampleLevel(probes, probes_sampler, (cell + 2.0 * slab) / size, 0.0);
    return max(vec3<f32>(dot(r, basis), dot(g, basis), dot(b, basis)), vec3<f32>(0.0));
}

// `want_reflected` asks for the irradiance along the reflection too (metals,
// and every surface while the sky's light is on); the other sample is skipped.
fn probe_light(position: vec3<f32>, normal: vec3<f32>, reflected: vec3<f32>, want_reflected: bool) -> ProbeLight {
    var result = ProbeLight(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, 1.0, 1.0);
    let mode = frame.probe_grid.w;
    if mode < 0.5 {
        return result;
    }
    let origin = frame.probes.xyz;
    let spacing = frame.probes.w;
    let dims = frame.probe_grid.xyz;
    let extent = dims - vec3<f32>(1.0);
    let grid = (position + normal * (spacing * PROBE_NORMAL_OFFSET) - origin) / spacing;
    let outside = max(max(-grid, grid - extent), vec3<f32>(0.0));
    let coverage = clamp(1.0 - max(outside.x, max(outside.y, outside.z)), 0.0, 1.0);
    if coverage <= 0.0 {
        return result;
    }
    let cell = clamp(grid, vec3<f32>(0.0), extent) + vec3<f32>(0.5);
    let compact = mode > 2.5;
    let floor_ambient = mode == 2.0 || mode == 4.0;
    result.diffuse = probe_sh(cell, dims, normal, compact);
    if want_reflected {
        result.reflected = probe_sh(cell, dims, reflected, compact);
    }
    result.coverage = coverage;
    result.outside = 1.0 - coverage;
    result.ambient = select(1.0 - coverage, 1.0, floor_ambient);
    return result;
}

fn add_light(
    index: u32,
    f0: vec3<f32>,
    view: vec3<f32>,
    normal: vec3<f32>,
    world_position: vec3<f32>,
    roughness: f32,
    occlusion: f32,
    reflected: vec3<f32>,
    probe_ambient: f32,
    probe_outside: f32,
    irradiance: ptr<function, vec3<f32>>,
    specular: ptr<function, vec3<f32>>,
    environment: ptr<function, vec3<f32>>,
    sky_open: ptr<function, f32>,
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
            *irradiance += color * occlusion * sky * probe_ambient;
            *environment += color * occlusion * sky * probe_ambient;
            if sky_layer > 0u {
                *sky_open = min(*sky_open, sky);
            }
        } else if kind == 1u {
            *irradiance += mix(light.extra.rgb, color, 0.5 * normal.y + 0.5) * occlusion * probe_outside;
            *environment += mix(light.extra.rgb, color, 0.5 * reflected.y + 0.5) * occlusion * probe_outside;
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
            let facing = clamp(dot(normal, direction), 0.0, 1.0);
            // A light that cannot reach the fragment needs no shadow lookup.
            let shadow = u32(light.extra.w);
            if shadow > 0u && attenuation * facing > 0.0 {
                if kind == 2u {
                    attenuation = attenuation
                        * cascade_visibility(shadow - 1u, world_position, normal, light.position_range, light.extra.xyz);
                } else {
                    var layer = shadow - 1u;
                    if kind == 3u {
                        layer += point_face(world_position - light.position_range.xyz);
                    }
                    attenuation = attenuation * shadow_visibility(layer, world_position, normal);
                }
            }
            let incident = color * attenuation * facing;
            *irradiance += incident;
            *specular += incident * brdf_ggx(direction, view, normal, roughness, f0);
        }
}

// The sky's irradiance along `normal`: its harmonics' sum there.
fn sky_irradiance_along(normal: vec3<f32>) -> vec3<f32> {
    let d = normal;
    return sky_irradiance[0].rgb * 0.282095
        + sky_irradiance[1].rgb * (0.488603 * d.y)
        + sky_irradiance[2].rgb * (0.488603 * d.z)
        + sky_irradiance[3].rgb * (0.488603 * d.x)
        + sky_irradiance[4].rgb * (1.092548 * d.x * d.y)
        + sky_irradiance[5].rgb * (1.092548 * d.y * d.z)
        + sky_irradiance[6].rgb * (0.315392 * (3.0 * d.z * d.z - 1.0))
        + sky_irradiance[7].rgb * (1.092548 * d.x * d.z)
        + sky_irradiance[8].rgb * (0.546274 * (d.x * d.x - d.y * d.y));
}

// Diffuse plus GGX specular from every light row of the pass, before
// emission. `occlusion` scales the ambient and hemisphere (indirect) light
// only, as does an ambient light's sky layer. Metals tint specular and lose
// diffuse; they reflect ambient and hemisphere light as a uniform
// environment (the hemisphere along the reflection), while dielectrics take
// that light as diffuse only. With the sky's light on, the sky adds its
// irradiance to the diffuse light and every surface reflects the sky,
// prefiltered by its roughness, in place of that uniform environment; the
// ambient light's sky layer and `occlusion` scale both, as they scale
// ambient light.
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
    // How open the sky above is, through an ambient light's sky layer.
    var sky_open = 1.0;
    let reflected = reflect(-view, normal);
    // Inside the indirect light volume its probes stand in for the ambient
    // and hemisphere rows and the sky's light.
    let probe = probe_light(world_position, normal, reflected, metalness > 0.0 || frame.sky_light.x > 0.0);
    if frame.cluster_grid.w == 1u {
        // The global list, then the fragment's cluster.
        let global_base = frame.cluster_grid.x * frame.cluster_grid.y * frame.cluster_grid.z * CLUSTER_STRIDE;
        let global_count = min(clusters[global_base], CLUSTER_STRIDE - 1u);
        for (var slot = 0u; slot < global_count; slot = slot + 1u) {
            add_light(clusters[global_base + 1u + slot], f0, view, normal, world_position, roughness,
                occlusion, reflected, probe.ambient, probe.outside, &irradiance, &specular, &environment, &sky_open);
        }
        let base = fragment_cluster(world_position) * CLUSTER_STRIDE;
        let count = min(clusters[base], CLUSTER_STRIDE - 1u);
        for (var slot = 0u; slot < count; slot = slot + 1u) {
            add_light(clusters[base + 1u + slot], f0, view, normal, world_position, roughness,
                occlusion, reflected, probe.ambient, probe.outside, &irradiance, &specular, &environment, &sky_open);
        }
    } else {
        for (var index = frame.counts.y; index < frame.counts.y + frame.counts.x; index = index + 1u) {
            add_light(index, f0, view, normal, world_position, roughness, occlusion, reflected,
                probe.ambient, probe.outside, &irradiance, &specular, &environment, &sky_open);
        }
    }
    irradiance += probe.diffuse * occlusion * probe.coverage;
    environment += probe.reflected * occlusion * probe.coverage;
    let n_dot_v = clamp(dot(normal, view), 0.0, 1.0);
    var reflection = metalness * environment * environment_brdf(f0, roughness, n_dot_v);
    let sky_intensity = frame.sky_light.x;
    if sky_intensity > 0.0 {
        let sky = sky_intensity * occlusion * sky_open * probe.outside;
        irradiance += max(sky_irradiance_along(normal), vec3<f32>(0.0)) * sky;
        let prefiltered = textureSampleLevel(sky_specular, sky_sampler, reflected, roughness * frame.sky_light.y).rgb;
        reflection = (prefiltered * sky + probe.reflected / PI * occlusion * probe.coverage)
            * environment_brdf(f0, roughness, n_dot_v);
    }
    return albedo * (1.0 - metalness) * irradiance / PI + specular + reflection;
}
