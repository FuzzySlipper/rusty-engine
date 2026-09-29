// World pass: parts drawn with the retained material and light rows, and the
// equirectangular sky. Lighting follows the Three lane's MeshStandardMaterial
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
    // hemisphere: ground colour * intensity; spot: (cos outer, cos inner)
    extra: vec4<f32>,
};

struct MaterialUniform {
    roughness: f32,
    alpha_cutoff: f32,
    flags: u32,
    pad: u32,
};

const FLAG_UNLIT: u32 = 1u;
const FLAG_MASK: u32 = 2u;
const PI: f32 = 3.141592653589793;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> parts: array<Part>;
@group(0) @binding(2) var<storage, read> lights: array<Light>;
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var albedo: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) part: u32,
};

@vertex
fn vs_world(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @builtin(instance_index) part: u32,
) -> VsOut {
    let row = parts[part];
    let world = row.model * vec4<f32>(position, 1.0);
    var out: VsOut;
    out.clip = frame.view_proj * world;
    out.world_position = world.xyz;
    out.normal = mat3x3<f32>(row.normal0.xyz, row.normal1.xyz, row.normal2.xyz) * normal;
    out.uv = uv;
    out.part = part;
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

fn brdf_ggx(light: vec3<f32>, view: vec3<f32>, normal: vec3<f32>, roughness: f32) -> vec3<f32> {
    let alpha = roughness * roughness;
    let half_vector = normalize(light + view);
    let n_dot_l = clamp(dot(normal, light), 0.0, 1.0);
    let n_dot_v = clamp(dot(normal, view), 0.0, 1.0);
    let n_dot_h = clamp(dot(normal, half_vector), 0.0, 1.0);
    let v_dot_h = clamp(dot(view, half_vector), 0.0, 1.0);
    let fresnel_weight = exp2((-5.55473 * v_dot_h - 6.98316) * v_dot_h);
    let fresnel = vec3<f32>(0.04) * (1.0 - fresnel_weight) + vec3<f32>(fresnel_weight);
    let a2 = alpha * alpha;
    let gv = n_dot_l * sqrt(a2 + (1.0 - a2) * n_dot_v * n_dot_v);
    let gl = n_dot_v * sqrt(a2 + (1.0 - a2) * n_dot_l * n_dot_l);
    let visibility = 0.5 / max(gv + gl, 1e-6);
    let denominator = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    let distribution = a2 / (PI * denominator * denominator);
    return fresnel * visibility * distribution;
}

@fragment
fn fs_world(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let row = parts[in.part];
    let base = row.color * textureSample(albedo, albedo_sampler, in.uv);
    var normal = normalize(in.normal);
    if !front {
        normal = -normal;
    }
    let normal_change = max(abs(dpdx(normal)), abs(dpdy(normal)));
    let geometry_roughness = max(max(normal_change.x, normal_change.y), normal_change.z);
    if (material.flags & FLAG_MASK) != 0u && base.a < material.alpha_cutoff {
        discard;
    }
    if (material.flags & FLAG_UNLIT) != 0u {
        return base;
    }
    let roughness = min(max(material.roughness, 0.0525) + geometry_roughness, 1.0);
    let view = normalize(frame.camera.xyz - in.world_position);
    var irradiance = vec3<f32>(0.0);
    var specular = vec3<f32>(0.0);
    for (var index = frame.counts.y; index < frame.counts.y + frame.counts.x; index = index + 1u) {
        let light = lights[index];
        let kind = u32(light.color_kind.w);
        let color = light.color_kind.rgb;
        if kind == 0u {
            irradiance += color;
        } else if kind == 1u {
            irradiance += mix(light.extra.rgb, color, 0.5 * normal.y + 0.5);
        } else {
            var direction = -normalize(light.direction_decay.xyz);
            var attenuation = 1.0;
            if kind != 2u {
                let to_light = light.position_range.xyz - in.world_position;
                let distance = length(to_light);
                direction = to_light / max(distance, 1e-6);
                attenuation = distance_attenuation(distance, light.position_range.w, light.direction_decay.w);
                if kind == 4u {
                    let angle = dot(-direction, normalize(light.direction_decay.xyz));
                    attenuation = attenuation * smoothstep(light.extra.x, light.extra.y, angle);
                }
            }
            let incident = color * attenuation * clamp(dot(normal, direction), 0.0, 1.0);
            irradiance += incident;
            specular += incident * brdf_ggx(direction, view, normal, roughness);
        }
    }
    let radiance = base.rgb * irradiance / PI + specular + row.emission.rgb;
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
