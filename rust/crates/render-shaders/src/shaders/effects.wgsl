// Sprites and particles, lit by the world pass's light rows and finished as
// the world pass is (`rusty::finish`). Sprite bindings in group 1 start at 10
// and particle bindings at 20, so the two layouts never collide in this
// module.

#import rusty::view::frame
#import rusty::lighting::standard_radiance
#import rusty::finish::finish

// Sprite: one instanced quad. Rows are built per view pass (billboard
// orientation, pixel size and viewport placement depend on the camera).
struct SpriteIn {
    // Unit-quad corner: x right, y up, both 0..1.
    @location(0) corner: vec2<f32>,
    @location(1) model0: vec4<f32>,
    @location(2) model1: vec4<f32>,
    @location(3) model2: vec4<f32>,
    @location(4) model3: vec4<f32>,
    // Atlas frame: u0, v0 (top), u1, v1 (bottom), image space.
    @location(5) uv_rect: vec4<f32>,
    @location(6) tint: vec4<f32>,
    // Quad in the sprite plane: x0, y0 (bottom left), x1, y1 (top right).
    @location(7) quad: vec4<f32>,
    // x: lighting (0 unlit, 1 synthetic, 2 normal map, 3 bump map),
    // y: alpha cutoff (0 for none), z: normal or bump strength, w: synthetic bias.
    @location(8) params: vec4<f32>,
};

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) params: vec4<f32>,
    // Where the fragment lies in its atlas frame, in image orientation: the
    // whole-texture uv for a whole texture.
    @location(5) cell: vec2<f32>,
};

@group(1) @binding(10) var sprite_color: texture_2d<f32>;
@group(1) @binding(11) var sprite_color_sampler: sampler;
// The normal or height map; the colour texture again when there is none.
@group(1) @binding(12) var sprite_detail: texture_2d<f32>;
@group(1) @binding(13) var sprite_detail_sampler: sampler;

@vertex
fn vs_sprite(in: SpriteIn) -> SpriteOut {
    let model = mat4x4<f32>(in.model0, in.model1, in.model2, in.model3);
    let local = mix(in.quad.xy, in.quad.zw, in.corner);
    let world = model * vec4<f32>(local, 0.0, 1.0);
    var out: SpriteOut;
    out.clip = frame.view_proj * world;
    out.world_position = world.xyz / world.w;
    out.normal = normalize((model * vec4<f32>(0.0, 0.0, 1.0, 0.0)).xyz);
    out.uv = vec2<f32>(
        mix(in.uv_rect.x, in.uv_rect.z, in.corner.x),
        mix(in.uv_rect.w, in.uv_rect.y, in.corner.y),
    );
    let frame_min = min(in.uv_rect.xy, in.uv_rect.zw);
    let frame_size = max(abs(in.uv_rect.zw - in.uv_rect.xy), vec2<f32>(1e-6));
    out.cell = (out.uv - frame_min) / frame_size;
    out.tint = in.tint;
    out.params = in.params;
    return out;
}

const SPRITE_ROUGHNESS: f32 = 0.82;

// Screen-space derivatives in GL orientation (y up), the orientation the
// derivative tangent frames below (`perturbNormal2Arb`, `perturbNormalArb`)
// are written for.
fn gl_dpdy3(value: vec3<f32>) -> vec3<f32> {
    return -dpdy(value);
}

fn gl_dpdy2(value: vec2<f32>) -> vec2<f32> {
    return -dpdy(value);
}

@fragment
fn fs_sprite(in: SpriteOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Derivatives and samples first: they need uniform control flow.
    let q0 = dpdx(in.world_position);
    let q1 = gl_dpdy3(in.world_position);
    let st0 = dpdx(in.uv);
    let st1 = gl_dpdy2(in.uv);
    // The frame's own derivatives: the whole-texture uv's, whatever the
    // frame's share of the sheet.
    let cell0 = dpdx(in.cell);
    let cell1 = gl_dpdy2(in.cell);
    let color = textureSample(sprite_color, sprite_color_sampler, in.uv) * in.tint;
    let detail = textureSample(sprite_detail, sprite_detail_sampler, in.uv);
    let height_x = textureSample(sprite_detail, sprite_detail_sampler, in.uv + st0).x;
    let height_y = textureSample(sprite_detail, sprite_detail_sampler, in.uv + st1).x;
    var normal = normalize(in.normal);
    let face = select(-1.0, 1.0, front);
    normal = normal * face;
    let normal_change = max(abs(dpdx(normal)), abs(dpdy(normal)));
    let geometry_roughness = max(max(normal_change.x, normal_change.y), normal_change.z);

    let cutoff = in.params.y;
    if cutoff > 0.0 && color.a < cutoff {
        discard;
    }
    let mode = u32(in.params.x + 0.5);
    if mode == 0u {
        return finish(color, in.world_position);
    }
    let strength = in.params.z;
    let q1_perp = cross(q1, normal);
    let q0_perp = cross(normal, q0);
    if mode == 1u || mode == 2u {
        // Tangent frame from screen derivatives (perturbNormal2Arb).
        let tangent = q1_perp * cell0.x + q0_perp * cell1.x;
        let bitangent = q1_perp * cell0.y + q0_perp * cell1.y;
        let largest = max(dot(tangent, tangent), dot(bitangent, bitangent));
        var scale = 0.0;
        if largest > 0.0 {
            scale = inverseSqrt(largest);
        }
        let tbn = mat3x3<f32>(tangent * scale, bitangent * scale, normal);
        var local = vec3<f32>(0.0, 0.0, 1.0);
        if mode == 1u {
            // Synthetic dome over the sprite's atlas frame.
            let radius = max(0.001, 1.0 + in.params.w);
            let xy = (in.cell * 2.0 - 1.0) * strength / radius;
            local = vec3<f32>(xy, sqrt(max(0.001, 1.0 - min(dot(xy, xy), 0.999))));
        } else {
            local = detail.xyz * 2.0 - 1.0;
            local = vec3<f32>(local.xy * strength, local.z);
        }
        normal = normalize(tbn * local);
    } else {
        // Height map from red (perturbNormalArb).
        let height = strength * detail.x;
        let gradient = vec2<f32>(strength * height_x - height, strength * height_y - height);
        let determinant = dot(q0, q1_perp) * face;
        let direction = sign(determinant) * (gradient.x * q1_perp + gradient.y * q0_perp);
        normal = normalize(abs(determinant) * normal - direction);
    }
    let roughness = min(SPRITE_ROUGHNESS + geometry_roughness, 1.0);
    return finish(
        vec4<f32>(standard_radiance(color.rgb, normal, in.world_position, roughness, 0.0, 1.0), color.a),
        in.world_position,
    );
}

// Particle billboard: a screen-aligned quad of constant pixel size
// (`size × 24` pixels), with a horizontal flipbook strip.
struct ParticleIn {
    @location(0) corner: vec2<f32>,
    // xyz: world position; w: half width in clip units.
    @location(1) center: vec4<f32>,
    @location(2) color: vec4<f32>,
    // x: half height in clip units; y: frame; z: frame count.
    @location(3) flipbook: vec4<f32>,
};

struct ParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // The billboard's centre, for fog.
    @location(2) world_position: vec3<f32>,
};

@group(1) @binding(20) var particle_texture: texture_2d<f32>;
@group(1) @binding(21) var particle_sampler: sampler;

@vertex
fn vs_particle(in: ParticleIn) -> ParticleOut {
    var clip = frame.view_proj * vec4<f32>(in.center.xyz, 1.0);
    let offset = (in.corner * 2.0 - 1.0) * vec2<f32>(in.center.w, in.flipbook.x);
    clip = vec4<f32>(clip.xy + offset * clip.w, clip.zw);
    var out: ParticleOut;
    out.clip = clip;
    let count = max(in.flipbook.z, 1.0);
    let frame_index = clamp(floor(in.flipbook.y + 0.5), 0.0, count - 1.0);
    out.uv = vec2<f32>((frame_index + in.corner.x) / count, 1.0 - in.corner.y);
    out.color = in.color;
    out.world_position = in.center.xyz;
    return out;
}

@fragment
fn fs_particle(in: ParticleOut) -> @location(0) vec4<f32> {
    let color = textureSample(particle_texture, particle_sampler, in.uv) * in.color;
    if color.a <= 0.001 {
        discard;
    }
    return finish(color, in.world_position);
}

// Particle cube: the builtin unit cube per instance, flat colour.
struct CubeParticleIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // xyz: world position; w: edge length.
    @location(3) center: vec4<f32>,
    @location(4) color: vec4<f32>,
};

struct CubeParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_position: vec3<f32>,
};

@vertex
fn vs_particle_cube(in: CubeParticleIn) -> CubeParticleOut {
    var out: CubeParticleOut;
    out.world_position = in.center.xyz + in.position * in.center.w;
    out.clip = frame.view_proj * vec4<f32>(out.world_position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_particle_cube(in: CubeParticleOut) -> @location(0) vec4<f32> {
    if in.color.a <= 0.001 {
        discard;
    }
    return finish(in.color, in.world_position);
}
