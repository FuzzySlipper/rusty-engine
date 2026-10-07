// Sprites and particles, lit by the world pass's light rows and finished as
// the world is (`rusty::finish`). Sprite bindings in group 1 start at 10
// and particle bindings at 20, so the two layouts never collide in this
// module.

#import rusty::view::{frame, mask_coverage}
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
    // x: softness in metres (0 hard), y: 1 when added to the frame.
    @location(9) soft: vec4<f32>,
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
    @location(6) soft: vec4<f32>,
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
    out.soft = in.soft;
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

fn shaded_sprite(in: SpriteOut, front: bool, masked: bool) -> vec4<f32> {
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
    if masked && cutoff > 0.0 && color.a < cutoff {
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

// A sprite's colour at `fade` of its alpha, premultiplied when it adds to
// the frame so a faded sprite adds nothing.
fn faded_sprite(in: SpriteOut, front: bool, fade: f32) -> vec4<f32> {
    var color = shaded_sprite(in, front, true);
    color.a = color.a * fade;
    if in.soft.y > 0.5 {
        color = vec4<f32>(color.rgb * color.a, color.a);
    }
    return color;
}

// Blended sprites: the colour and its alpha, blended over (or added to) the world.
@fragment
fn fs_sprite(in: SpriteOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    return faded_sprite(in, front, 1.0);
}

struct OpaqueSpriteOut {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
};

// Solid sprites cover their pixel (`world.wgsl` `fs_world_opaque`); a masked
// one (a cutoff) covers its samples by its alpha, so its cut edge resolves
// anti-aliased under multisampling.
@fragment
fn fs_sprite_opaque(in: SpriteOut, @builtin(front_facing) front: bool) -> OpaqueSpriteOut {
    let cutoff = in.params.y;
    let color = shaded_sprite(in, front, false);
    var mask = 0xffffffffu;
    if cutoff > 0.0 {
        mask = mask_coverage(color.a, cutoff);
    }
    return OpaqueSpriteOut(vec4<f32>(color.rgb, 1.0), mask);
}

// Particle billboard: a screen-aligned quad, of constant pixel size
// (`size × 24` pixels) or of a world size, with a horizontal flipbook strip.
// A hard billboard draws in the world pass under its depth test; a soft one
// (`params.x` > 0) draws in the particle pass after it, with the world's
// depth bound in group 2, and tests and fades against that depth itself.
struct ParticleIn {
    @location(0) corner: vec2<f32>,
    // xyz: world position; w: half width in clip units.
    @location(1) center: vec4<f32>,
    @location(2) color: vec4<f32>,
    // x: half height in clip units; y: frame; z: frame count; w: 1 when the
    // half extents are projected world sizes (no depth scaling), 0 for screen.
    @location(3) flipbook: vec4<f32>,
    // x: the metres over which the billboard fades out as it nears the
    // world behind it (0: a hard depth edge); y: 1 when its colour adds to
    // the frame (premultiplied by its alpha here), 0 when it blends by alpha.
    @location(4) params: vec4<f32>,
};

struct ParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // The billboard's centre, for fog.
    @location(2) world_position: vec3<f32>,
    @location(3) params: vec4<f32>,
};

@group(1) @binding(20) var particle_texture: texture_2d<f32>;
@group(1) @binding(21) var particle_sampler: sampler;
// The world's depth for soft billboards, as the view has it: one of these is
// bound, by the view's sample count.
@group(2) @binding(30) var scene_depth: texture_depth_2d;
@group(2) @binding(31) var scene_depth_multisampled: texture_depth_multisampled_2d;

@vertex
fn vs_particle(in: ParticleIn) -> ParticleOut {
    var clip = frame.view_proj * vec4<f32>(in.center.xyz, 1.0);
    let offset = (in.corner * 2.0 - 1.0) * vec2<f32>(in.center.w, in.flipbook.x);
    let depth_scale = select(clip.w, 1.0, in.flipbook.w > 0.5);
    clip = vec4<f32>(clip.xy + offset * depth_scale, clip.zw);
    var out: ParticleOut;
    out.clip = clip;
    let count = max(in.flipbook.z, 1.0);
    let frame_index = clamp(floor(in.flipbook.y + 0.5), 0.0, count - 1.0);
    out.uv = vec2<f32>((frame_index + in.corner.x) / count, 1.0 - in.corner.y);
    out.color = in.color;
    out.world_position = in.center.xyz;
    out.params = in.params;
    return out;
}

// The billboard's colour at `fade` of its alpha: premultiplied when it adds
// to the frame, so a faded particle adds nothing.
fn particle_color(in: ParticleOut, fade: f32) -> vec4<f32> {
    var color = textureSample(particle_texture, particle_sampler, in.uv) * in.color;
    color.a = color.a * fade;
    if in.params.y > 0.5 {
        color = vec4<f32>(color.rgb * color.a, color.a);
    }
    return color;
}

@fragment
fn fs_particle(in: ParticleOut) -> @location(0) vec4<f32> {
    let color = particle_color(in, 1.0);
    if color.a <= 0.001 {
        discard;
    }
    return finish(color, in.world_position);
}

// The world point at `depth` on the view's centre ray: the gap between two
// such points is the view-space distance between two depths.
fn depth_point(depth: f32) -> vec3<f32> {
    let point = frame.inv_view_proj * vec4<f32>(0.0, 0.0, depth, 1.0);
    return point.xyz / point.w;
}

// How far a soft billboard at `depth` is faded by the world at `scene`:
// nothing where the world is in front of it (the depth test the world pass
// would make), else by the gap, over `softness` metres.
fn soft_fade(depth: f32, scene: f32, softness: f32) -> f32 {
    if depth > scene {
        return 0.0;
    }
    return saturate(distance(depth_point(scene), depth_point(depth)) / softness);
}

// A soft billboard against the world's depth at its pixel: hidden where the
// world is in front of it, faded out over `params.x` metres as it nears the
// world behind it.
@fragment
fn fs_particle_soft(in: ParticleOut) -> @location(0) vec4<f32> {
    let fade = soft_fade(in.clip.z, textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0), in.params.x);
    let color = particle_color(in, fade);
    if fade <= 0.0 || color.a <= 0.001 {
        discard;
    }
    return finish(color, in.world_position);
}

struct SoftMultisampledOut {
    @location(0) color: vec4<f32>,
    // Only the samples the world leaves uncovered take the billboard, as the
    // fixed-function depth test of the world pass would decide per sample.
    @builtin(sample_mask) mask: u32,
};

// The multisampled soft billboard: each of the world's samples at the pixel
// is tested and faded on its own, the fragment covers the samples the world
// is behind, and its colour fades by their mean gap.
@fragment
fn fs_particle_soft_multisampled(in: ParticleOut) -> SoftMultisampledOut {
    let samples = i32(textureNumSamples(scene_depth_multisampled));
    var mask = 0u;
    var fade = 0.0;
    var covered = 0.0;
    for (var sample = 0; sample < samples; sample++) {
        let scene = textureLoad(scene_depth_multisampled, vec2<i32>(in.clip.xy), sample);
        let sample_fade = soft_fade(in.clip.z, scene, in.params.x);
        if sample_fade > 0.0 {
            mask |= 1u << u32(sample);
            fade += sample_fade;
            covered += 1.0;
        }
    }
    if covered <= 0.0 {
        discard;
    }
    let color = particle_color(in, fade / covered);
    if color.a <= 0.001 {
        discard;
    }
    var out: SoftMultisampledOut;
    out.color = finish(color, in.world_position);
    out.mask = mask;
    return out;
}

// A soft sprite against the world's depth at its pixel, as a soft billboard:
// hidden where the world is in front, faded over `soft.x` metres toward it.
@fragment
fn fs_sprite_soft(in: SpriteOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let fade = soft_fade(in.clip.z, textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0), in.soft.x);
    let color = faded_sprite(in, front, fade);
    if fade <= 0.0 || color.a <= 0.001 {
        discard;
    }
    return color;
}

@fragment
fn fs_sprite_soft_multisampled(in: SpriteOut, @builtin(front_facing) front: bool) -> SoftMultisampledOut {
    let samples = i32(textureNumSamples(scene_depth_multisampled));
    var mask = 0u;
    var fade = 0.0;
    var covered = 0.0;
    for (var sample = 0; sample < samples; sample++) {
        let scene = textureLoad(scene_depth_multisampled, vec2<i32>(in.clip.xy), sample);
        let sample_fade = soft_fade(in.clip.z, scene, in.soft.x);
        if sample_fade > 0.0 {
            mask |= 1u << u32(sample);
            fade += sample_fade;
            covered += 1.0;
        }
    }
    // Sampled before the branch: the sprite's derivatives need uniform control flow.
    let color = faded_sprite(in, front, fade / max(covered, 1.0));
    if covered <= 0.0 || color.a <= 0.001 {
        discard;
    }
    var out: SoftMultisampledOut;
    out.color = color;
    out.mask = mask;
    return out;
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
