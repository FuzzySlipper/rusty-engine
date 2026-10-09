// Volumetric fog (render-wgpu `volumetric_fog.rs`): a froxel grid over a
// world view, its cells spaced quadratically in distance from the camera
// out to the medium's reach. `cs_light` gives each cell its fog (the medium,
// thinning with height, plus every fog volume it stands in) and the light
// that fog scatters toward the camera: every light row of the pass through
// its shadows, the sun and moon through the cloud layer, and the ambient and
// sky light, weighted by a Henyey–Greenstein phase. `cs_integrate` walks each
// column front to back into what lies between the camera and each depth: the
// light scattered toward the camera and the transmittance. The finish pass
// reads that at each pixel's distance (`finish_pass.wgsl`).
//
// Temporally filtered: `cs_light` samples each cell at a jittered point
// within it and blends that with the cell's centre reprojected into the
// view's previous grid (`history`), by `fog.temporal.x` (0: no history,
// the cell's centre alone).

#import rusty::types::PI
#import rusty::view::{frame, lights}
#import rusty::lighting::{cascade_visibility, shadow_visibility, sky_visibility, distance_attenuation, point_face, sky_irradiance_along}
#import rusty::clouds::cloud_light

struct FogParams {
    // xyz: cells across, down and deep; w: fog volumes.
    grid: vec4<u32>,
    // x: the medium's density at its base height, y: the base height,
    // z: the falloff height (0: one density everywhere), w: the grid's
    // reach in metres.
    medium: vec4<f32>,
    // rgb: the medium's albedo; w: the phase's anisotropy.
    albedo: vec4<f32>,
    // x: the share of ambient and sky light scattered; y: the Engine's
    // presentation time, seconds.
    extra: vec4<f32>,
    // The previous frame's view-projection and eye, for reprojection.
    prev_view_proj: mat4x4<f32>,
    prev_eye: vec4<f32>,
    // x: the history's weight (0: none); yzw: this frame's jitter within
    // each cell, across, down and deep.
    temporal: vec4<f32>,
};

struct FogVolume {
    // xyz: centre; w: 0 a box, 1 an ellipsoid.
    center: vec4<f32>,
    // xyz: half extents; w: turn about the vertical, radians.
    half_yaw: vec4<f32>,
    // x: density, y: the edge's share of the half extent, z: one noise
    // cell's size (0: none), w: the noise's strength.
    density: vec4<f32>,
    albedo: vec4<f32>,
    emission: vec4<f32>,
    // xyz: the noise's drift, metres per second.
    velocity: vec4<f32>,
};

@group(1) @binding(0) var<uniform> fog: FogParams;
@group(1) @binding(1) var<storage, read> volumes: array<FogVolume>;
@group(1) @binding(2) var scatter_out: texture_storage_3d<rgba16float, write>;
@group(1) @binding(3) var scatter_in: texture_3d<f32>;
@group(1) @binding(4) var integrated_out: texture_storage_3d<rgba16float, write>;
@group(1) @binding(5) var history: texture_3d<f32>;
@group(1) @binding(6) var history_sampler: sampler;

// The distance from the camera of depth `slices` (0 to the grid's depth),
// along a cell's ray.
fn slice_distance(slices: f32) -> f32 {
    let t = slices / f32(fog.grid.z);
    return fog.medium.w * t * t;
}

// The ray through cell column `xy` (in cells, centre at +0.5).
fn cell_ray(xy: vec2<f32>) -> vec3<f32> {
    let uv = xy / vec2<f32>(fog.grid.xy);
    let world = frame.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.5, 1.0);
    return normalize(world.xyz / world.w - frame.camera.xyz);
}

fn hash(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

fn value_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash(i), hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash(i + vec3<f32>(0.0, 1.0, 0.0)), hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash(i + vec3<f32>(0.0, 0.0, 1.0)), hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash(i + vec3<f32>(0.0, 1.0, 1.0)), hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

// Two octaves, 0 to 1.
fn fog_noise(p: vec3<f32>) -> f32 {
    return 0.65 * value_noise(p) + 0.35 * value_noise(p * 2.03 + vec3<f32>(17.0, 3.0, 29.0));
}

// Fog volume `v`'s density at `p`: full inside, fading over its edge,
// thinned by its drifting noise.
fn volume_density(v: FogVolume, p: vec3<f32>) -> f32 {
    let offset = p - v.center.xyz;
    let c = cos(v.half_yaw.w);
    let s = sin(v.half_yaw.w);
    let local = vec3<f32>(c * offset.x - s * offset.z, offset.y, s * offset.x + c * offset.z);
    let q = local / v.half_yaw.xyz;
    var inside: f32;
    if v.center.w < 0.5 {
        inside = 1.0 - max(abs(q.x), max(abs(q.y), abs(q.z)));
    } else {
        inside = 1.0 - length(q);
    }
    if inside <= 0.0 {
        return 0.0;
    }
    var density = v.density.x * smoothstep(0.0, max(v.density.y, 1e-3), inside);
    if v.density.z > 0.0 {
        let n = fog_noise((p - v.velocity.xyz * fog.extra.y) / v.density.z);
        density = density * (1.0 - v.density.w * (1.0 - n));
    }
    return density;
}

// Henyey–Greenstein: how much of light travelling along `toward_light`
// reversed scatters toward the camera along `ray`, normalized over the
// sphere.
fn phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * cos_theta, 1e-4), 1.5));
}

const UP: vec3<f32> = vec3<f32>(0.0, 1.0, 0.0);

// The light the fog at `p` scatters toward the camera along `ray`, per unit
// of scattering: each light row through its shadows and phase, plus the
// ambient and sky light (radiance from all round, so its phase integrates
// to one).
fn in_scattered(p: vec3<f32>, ray: vec3<f32>) -> vec3<f32> {
    let g = fog.albedo.w;
    var direct = vec3<f32>(0.0);
    var ambient = vec3<f32>(0.0);
    for (var index = frame.counts.y; index < frame.counts.y + frame.counts.x; index = index + 1u) {
        let light = lights[index];
        let kind = u32(light.color_kind.w);
        let color = light.color_kind.rgb;
        let shadow = u32(light.extra.w);
        if kind == 0u {
            var sky = 1.0;
            if shadow > 0u {
                sky = sky_visibility(shadow - 1u, p, UP);
            }
            ambient += color * sky;
        } else if kind == 1u {
            ambient += 0.5 * (light.extra.rgb + color);
        } else {
            var toward = -normalize(light.direction_decay.xyz);
            var attenuation = 1.0;
            if kind != 2u {
                let to_light = light.position_range.xyz - p;
                let distance = length(to_light);
                toward = to_light / max(distance, 1e-6);
                attenuation = distance_attenuation(distance, light.position_range.w, light.direction_decay.w);
                if kind == 4u {
                    let angle = dot(-toward, normalize(light.direction_decay.xyz));
                    attenuation = attenuation * smoothstep(light.extra.x, light.extra.y, angle);
                }
            }
            if attenuation <= 0.0 {
                continue;
            }
            if shadow > 0u {
                if kind == 2u {
                    attenuation = attenuation
                        * cascade_visibility(shadow - 1u, p, toward, light.position_range, light.extra.xyz);
                } else {
                    var layer = shadow - 1u;
                    if kind == 3u {
                        layer += point_face(p - light.position_range.xyz);
                    }
                    attenuation = attenuation * shadow_visibility(layer, p, toward);
                }
            }
            if kind == 2u {
                attenuation = attenuation * cloud_light(p, toward);
            }
            direct += color * attenuation * phase(dot(ray, toward), g);
        }
    }
    if frame.sky_light.x > 0.0 {
        let sky = 0.5 * (sky_irradiance_along(UP) + sky_irradiance_along(-UP));
        ambient += max(sky, vec3<f32>(0.0)) * frame.sky_light.x;
    }
    // Irradiance from all round over pi is the radiance of a uniform sky.
    return direct + ambient / PI * fog.extra.x;
}

@compute @workgroup_size(4, 4, 4)
fn cs_light(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id >= fog.grid.xyz) {
        return;
    }
    let jitter = fog.temporal.yzw;
    let ray = cell_ray(vec2<f32>(id.xy) + jitter.xy);
    let p = frame.camera.xyz + ray * slice_distance(f32(id.z) + jitter.z);
    var density = 0.0;
    if fog.medium.x > 0.0 {
        var height = 1.0;
        if fog.medium.z > 0.0 {
            height = min(exp(-(p.y - fog.medium.y) / fog.medium.z), 64.0);
        }
        density = fog.medium.x * height;
    }
    var scattering = fog.albedo.rgb * density;
    var emission = vec3<f32>(0.0);
    for (var index = 0u; index < fog.grid.w; index = index + 1u) {
        let volume = volumes[index];
        let amount = volume_density(volume, p);
        if amount > 0.0 {
            density += amount;
            scattering += volume.albedo.rgb * amount;
            emission += volume.emission.rgb * amount;
        }
    }
    var current = vec4<f32>(0.0);
    if density > 1e-6 {
        current = vec4<f32>(scattering * in_scattered(p, ray) + emission, density);
    }
    textureStore(scatter_out, id, filtered(id, current));
}

// `current` blended with the cell's centre as the view's previous grid saw
// it, where that lies inside the previous grid.
fn filtered(id: vec3<u32>, current: vec4<f32>) -> vec4<f32> {
    let weight = fog.temporal.x;
    if weight <= 0.0 {
        return current;
    }
    let centre = frame.camera.xyz
        + cell_ray(vec2<f32>(id.xy) + 0.5) * slice_distance(f32(id.z) + 0.5);
    let clip = fog.prev_view_proj * vec4<f32>(centre, 1.0);
    if clip.w <= 0.0 {
        return current;
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let depth = sqrt(clamp(distance(centre, fog.prev_eye.xyz) / fog.medium.w, 0.0, 1.0));
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || depth >= 1.0 {
        return current;
    }
    let previous = textureSampleLevel(history, history_sampler, vec3<f32>(uv, depth), 0.0);
    // A cell whose centre moved across the grid since the last frame (a
    // near one, as the camera moves) trusts its history less, so it does
    // not trail: a cell's move takes the current sample whole.
    let now = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(fog.grid.xy);
    let slices = f32(fog.grid.z);
    let moved = length(vec3<f32>((uv - now) * vec2<f32>(fog.grid.xy),
        (depth - (f32(id.z) + 0.5) / slices) * slices));
    return mix(previous, current, clamp(weight + moved, weight, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn cs_integrate(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= fog.grid.xy) {
        return;
    }
    var scattered = vec3<f32>(0.0);
    var transmittance = 1.0;
    var near = 0.0;
    for (var z = 0u; z < fog.grid.z; z = z + 1u) {
        let cell = textureLoad(scatter_in, vec3<u32>(id.xy, z), 0);
        let far = slice_distance(f32(z) + 1.0);
        let extinction = max(cell.a, 1e-7);
        let through = exp(-extinction * (far - near));
        // The cell's light, dimmed by the fog in front and integrated over
        // its own depth (energy-conserving).
        scattered += transmittance * cell.rgb * (1.0 - through) / extinction;
        transmittance *= through;
        near = far;
        textureStore(integrated_out, vec3<u32>(id.xy, z), vec4<f32>(scattered, transmittance));
    }
}
