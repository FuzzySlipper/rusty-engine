// Sprites and particles, lit by the world pass's light rows and finished as
// the world is (`rusty::finish`). Sprite bindings in group 1 start at 10
// and particle bindings at 20, so the two layouts never collide in this
// module.

#import rusty::view::{frame, mask_coverage, wind_lean, upright_facing}
#import rusty::lighting::standard_radiance
#import rusty::finish::finish
#ifdef MOTION
#import rusty::motion::{previous_clip, target_pixel_of, previous_pixel, motion_output}
#endif

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
    // x: softness in metres (0 hard), y: 1 when added to the frame, w: 1 when
    // it leans in whole texels.
    @location(9) soft: vec4<f32>,
    // xyz: emitted light before the emission map; w: its wind bend.
    @location(10) emission: vec4<f32>,
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
    @location(7) emission: vec3<f32>,
    // A sprite leaning in whole texels: x its top's lean as a share of its
    // width (0: smooth, or still); y its frame's width in uv, negative when
    // flipped; z its height.
    @location(8) sway: vec3<f32>,
};

@group(1) @binding(10) var sprite_color: texture_2d<f32>;
@group(1) @binding(11) var sprite_color_sampler: sampler;
// The normal or height map; the colour texture again when there is none.
@group(1) @binding(12) var sprite_detail: texture_2d<f32>;
@group(1) @binding(13) var sprite_detail_sampler: sampler;
// Where the tint's colour applies, by alpha, at the colour texture's uv;
// 1×1 white (the tint everywhere) when the material has no mask.
@group(1) @binding(14) var sprite_tint_mask: texture_2d<f32>;
@group(1) @binding(15) var sprite_tint_mask_sampler: sampler;
// Where the sprite emits and in what colour, by colour times alpha, at the
// colour texture's uv; 1×1 white (all of it) when the material has no map.
@group(1) @binding(16) var sprite_emission_map: texture_2d<f32>;
@group(1) @binding(17) var sprite_emission_sampler: sampler;

// The wind on a sprite (`SetWind`): its top's move in the world, its base
// planted. A sprite leaning in whole texels keeps its shape instead, widened
// on the side it leans by `share` of its width, its lean along its own width
// alone, and `stepped` moves its rows. `origin` phases it, as a part's place
// does; `right` is unit length, `wide` and `tall` the sprite's size in metres.
struct SpriteSway {
    top: vec3<f32>,
    share: f32,
};

fn sprite_sway(origin: vec3<f32>, right: vec3<f32>, wide: f32, tall: f32, bend: f32, whole_texels: f32) -> SpriteSway {
    var sway: SpriteSway;
    if bend <= 0.0 {
        return sway;
    }
    sway.top = vec3<f32>(frame.wind.x, 0.0, frame.wind.y) * wind_lean(origin, frame.time.x) * bend * tall;
    if whole_texels > 0.5 {
        sway.share = dot(sway.top, right) / max(wide, 1e-6);
        sway.top = vec3<f32>(0.0);
    }
    return sway;
}

// How far across its quad a corner stands, widened for `share`.
fn swayed_across(corner: f32, share: f32) -> f32 {
    return corner * (1.0 + abs(share)) - max(-share, 0.0);
}

// A fragment's move in the texture for its row's lean, as a share of the
// texture's width: whole texels, toward the base so a row stays on the quad.
fn stepped(in: SpriteOut) -> f32 {
    let size = vec2<f32>(textureDimensions(sprite_color));
    let rows = max(in.sway.z * size.y, 1.0);
    let lean = in.sway.x * abs(in.sway.y) * size.x;
    // The bottom row stays planted and the top row takes the whole lean.
    let row = min(floor((1.0 - in.cell.y) * rows), rows - 1.0) / max(rows - 1.0, 1.0);
    return -trunc(row * lean) / size.x;
}

@vertex
fn vs_sprite(in: SpriteIn) -> SpriteOut {
    let model = mat4x4<f32>(in.model0, in.model1, in.model2, in.model3);
    let scale = vec2<f32>(length(in.model0.xyz), length(in.model1.xyz));
    let size = (in.quad.zw - in.quad.xy) * scale;
    let sway = sprite_sway(in.model3.xyz, in.model0.xyz / max(scale.x, 1e-6), size.x, size.y, in.emission.w, in.soft.w);
    let across = swayed_across(in.corner.x, sway.share);
    let local = mix(in.quad.xy, in.quad.zw, vec2<f32>(across, in.corner.y));
    let world = model * vec4<f32>(local, 0.0, 1.0) + vec4<f32>(sway.top * in.corner.y, 0.0);
    var out: SpriteOut;
    out.clip = frame.view_proj * world;
    out.world_position = world.xyz / world.w;
    out.normal = normalize((model * vec4<f32>(0.0, 0.0, 1.0, 0.0)).xyz);
    out.uv = vec2<f32>(
        mix(in.uv_rect.x, in.uv_rect.z, across),
        mix(in.uv_rect.w, in.uv_rect.y, in.corner.y),
    );
    let frame_min = min(in.uv_rect.xy, in.uv_rect.zw);
    let frame_size = max(abs(in.uv_rect.zw - in.uv_rect.xy), vec2<f32>(1e-6));
    out.cell = (out.uv - frame_min) / frame_size;
    out.tint = in.tint;
    out.params = in.params;
    out.soft = in.soft;
    out.emission = in.emission.xyz;
    out.sway = vec3<f32>(sway.share, in.uv_rect.z - in.uv_rect.x, abs(in.uv_rect.w - in.uv_rect.y));
    return out;
}

// Sprite batch: many quads of one atlas drawn by one instanced call. The
// rows are built once (world centre, atlas frame, quad around the centre)
// and each view turns them to face its camera here.
struct SpriteBatchIn {
    @location(0) corner: vec2<f32>,
    // What every instance shares, repeated on each corner.
    @location(1) tint: vec4<f32>,
    @location(2) params: vec4<f32>,
    // x: softness in metres, y: 1 when added to the frame, z: facing
    // (0 the node's axes, 1 spherical, 2 cylindrical, 3 cylindrical against
    // the camera's heading), w: 1 when it leans in whole texels.
    @location(3) soft: vec4<f32>,
    // The node's x and y axes, for a batch that does not face the camera;
    // right.w the share of the node's facing an upright batch keeps.
    @location(4) right: vec4<f32>,
    @location(5) up: vec4<f32>,
    @location(6) center: vec4<f32>,
    @location(7) uv_rect: vec4<f32>,
    @location(8) quad: vec4<f32>,
    // Shared like the tint: emitted light before the emission map; w its
    // wind bend.
    @location(9) emission: vec4<f32>,
};

fn unproject(ndc: vec3<f32>) -> vec3<f32> {
    let world = frame.inv_view_proj * vec4<f32>(ndc, 1.0);
    return world.xyz / world.w;
}

@vertex
fn vs_sprite_batch(in: SpriteBatchIn) -> SpriteOut {
    var right = in.right.xyz;
    var up = in.up.xyz;
    let facing = u32(in.soft.z + 0.5);
    if facing != 0u {
        // The camera's axes, from a plane between near and far in either
        // depth convention.
        let origin = unproject(vec3<f32>(0.0, 0.0, 0.5));
        let camera_right = normalize(unproject(vec3<f32>(1.0, 0.0, 0.5)) - origin);
        let camera_up = normalize(unproject(vec3<f32>(0.0, 1.0, 0.5)) - origin);
        right = camera_right;
        up = camera_up;
        if facing >= 2u {
            // Turn about world up toward the eye, or against the view
            // direction for an orthographic camera or a heading-aligned
            // batch (3), keeping `in.right.w` of the node's own facing.
            let orthographic = frame.view_proj[0].w == 0.0 && frame.view_proj[1].w == 0.0
                && frame.view_proj[2].w == 0.0;
            var toward = frame.camera.xyz - in.center.xyz;
            if orthographic || facing == 3u {
                toward = cross(camera_right, camera_up);
            }
            toward = upright_facing(toward, cross(in.right.xyz, in.up.xyz), in.right.w);
            up = vec3<f32>(0.0, 1.0, 0.0);
            right = normalize(cross(up, toward));
        }
    }
    let size = in.quad.zw - in.quad.xy;
    let sway = sprite_sway(in.center.xyz, right, size.x, size.y, in.emission.w, in.soft.w);
    let across = swayed_across(in.corner.x, sway.share);
    let local = mix(in.quad.xy, in.quad.zw, vec2<f32>(across, in.corner.y));
    let world = in.center.xyz + right * local.x + up * local.y + sway.top * in.corner.y;
    var out: SpriteOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_position = world;
    out.normal = normalize(cross(right, up));
    out.uv = vec2<f32>(
        mix(in.uv_rect.x, in.uv_rect.z, across),
        mix(in.uv_rect.w, in.uv_rect.y, in.corner.y),
    );
    let frame_min = min(in.uv_rect.xy, in.uv_rect.zw);
    let frame_size = max(abs(in.uv_rect.zw - in.uv_rect.xy), vec2<f32>(1e-6));
    out.cell = (out.uv - frame_min) / frame_size;
    out.tint = in.tint;
    out.params = in.params;
    out.soft = vec4<f32>(in.soft.xy, 0.0, 0.0);
    out.emission = in.emission.xyz;
    out.sway = vec3<f32>(sway.share, in.uv_rect.z - in.uv_rect.x, abs(in.uv_rect.w - in.uv_rect.y));
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

// What a sprite's lighting reads at a fragment, taken where control flow is
// still uniform: its place's and its frame's screen derivatives, and its
// detail texture.
struct SpriteSurface {
    world_position: vec3<f32>,
    // Toward the side the fragment is seen from: `face` (1 at the front, -1
    // at the back) times the drawn normal.
    normal: vec3<f32>,
    face: f32,
    // Where the fragment lies in its frame, in image orientation.
    cell: vec2<f32>,
    q0: vec3<f32>,
    q1: vec3<f32>,
    cell0: vec2<f32>,
    cell1: vec2<f32>,
    detail: vec4<f32>,
    height_x: f32,
    height_y: f32,
    geometry_roughness: f32,
};

fn sprite_surface(world_position: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>, cell: vec2<f32>, front: bool) -> SpriteSurface {
    var surface: SpriteSurface;
    surface.world_position = world_position;
    surface.cell = cell;
    surface.q0 = dpdx(world_position);
    surface.q1 = gl_dpdy3(world_position);
    let st0 = dpdx(uv);
    let st1 = gl_dpdy2(uv);
    // The frame's own derivatives: the whole-texture uv's, whatever the
    // frame's share of the sheet.
    surface.cell0 = dpdx(cell);
    surface.cell1 = gl_dpdy2(cell);
    surface.detail = textureSample(sprite_detail, sprite_detail_sampler, uv);
    surface.height_x = textureSample(sprite_detail, sprite_detail_sampler, uv + st0).x;
    surface.height_y = textureSample(sprite_detail, sprite_detail_sampler, uv + st1).x;
    surface.face = select(-1.0, 1.0, front);
    surface.normal = normalize(normal) * surface.face;
    let normal_change = max(abs(dpdx(surface.normal)), abs(dpdy(surface.normal)));
    surface.geometry_roughness = max(max(normal_change.x, normal_change.y), normal_change.z);
    return surface;
}

// `color` and `emitted` lit by the sprite's lighting mode (`params.x`).
fn lit_sprite(surface: SpriteSurface, params: vec4<f32>, color: vec4<f32>, emitted: vec3<f32>) -> vec4<f32> {
    let mode = u32(params.x + 0.5);
    if mode == 0u {
        return finish(vec4<f32>(color.rgb + emitted, color.a), surface.world_position);
    }
    let strength = params.z;
    var normal = surface.normal;
    let q0 = surface.q0;
    let q1 = surface.q1;
    let q1_perp = cross(q1, normal);
    let q0_perp = cross(normal, q0);
    if mode == 1u || mode == 2u {
        // Tangent frame from screen derivatives (perturbNormal2Arb).
        let tangent = q1_perp * surface.cell0.x + q0_perp * surface.cell1.x;
        let bitangent = q1_perp * surface.cell0.y + q0_perp * surface.cell1.y;
        let largest = max(dot(tangent, tangent), dot(bitangent, bitangent));
        var scale = 0.0;
        if largest > 0.0 {
            scale = inverseSqrt(largest);
        }
        let tbn = mat3x3<f32>(tangent * scale, bitangent * scale, normal);
        var local = vec3<f32>(0.0, 0.0, 1.0);
        if mode == 1u {
            // Synthetic dome over the sprite's atlas frame.
            let radius = max(0.001, 1.0 + params.w);
            let xy = (surface.cell * 2.0 - 1.0) * strength / radius;
            local = vec3<f32>(xy, sqrt(max(0.001, 1.0 - min(dot(xy, xy), 0.999))));
        } else {
            local = surface.detail.xyz * 2.0 - 1.0;
            local = vec3<f32>(local.xy * strength, local.z);
        }
        normal = normalize(tbn * local);
    } else {
        // Height map from red (perturbNormalArb).
        let height = strength * surface.detail.x;
        let gradient = vec2<f32>(strength * surface.height_x - height, strength * surface.height_y - height);
        let determinant = dot(q0, q1_perp) * surface.face;
        let direction = sign(determinant) * (gradient.x * q1_perp + gradient.y * q0_perp);
        normal = normalize(abs(determinant) * normal - direction);
    }
    let roughness = min(SPRITE_ROUGHNESS + surface.geometry_roughness, 1.0);
    return finish(
        vec4<f32>(standard_radiance(color.rgb, normal, surface.world_position, roughness, 0.0, 1.0) + emitted, color.a),
        surface.world_position,
    );
}

fn shaded_sprite(drawn: SpriteOut, front: bool, masked: bool) -> vec4<f32> {
    var in = drawn;
    let shift = stepped(in);
    in.uv.x += sign(in.sway.y) * shift;
    in.cell.x += shift / max(abs(in.sway.y), 1e-6);
    // Derivatives and samples first: they need uniform control flow.
    let surface = sprite_surface(in.world_position, in.normal, in.uv, in.cell, front);
    let texel = textureSample(sprite_color, sprite_color_sampler, in.uv);
    // An HDR tint (above 1) brightens unclamped.
    let tinted = mix(vec3<f32>(1.0), in.tint.rgb, textureSample(sprite_tint_mask, sprite_tint_mask_sampler, in.uv).a);
    let color = vec4<f32>(texel.rgb * tinted, texel.a * in.tint.a);
    // Light of its own, added after its lighting: its alpha masks it as it
    // does the rest of the sprite, and the tint's colour leaves it alone.
    let map = textureSample(sprite_emission_map, sprite_emission_sampler, in.uv);
    let emitted = in.emission * map.rgb * map.a;

    // A row stepped past its frame's side shows nothing of the next frame.
    if in.sway.x != 0.0 && (in.cell.x < 0.0 || in.cell.x > 1.0) {
        discard;
    }
    let cutoff = in.params.y;
    if masked && cutoff > 0.0 && color.a < cutoff {
        discard;
    }
    return lit_sprite(surface, in.params, color, emitted);
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

// Parts rig: a posed figure's pieces on one plane, placed per view by a
// sprite row (`model`, tint, lighting, emission) whose quad is unused. Each
// vertex is in rig units on that plane, with its piece's page uv, colours
// and page region. Pieces draw in the rig's draw order, each run of one page
// and blend mode by one call (`parts_rigs.rs`).
struct PartsRigIn {
    // x right, y up, in rig units.
    @location(0) position: vec2<f32>,
    @location(1) model0: vec4<f32>,
    @location(2) model1: vec4<f32>,
    @location(3) model2: vec4<f32>,
    @location(4) model3: vec4<f32>,
    // x: rig units per texel of the grid it snaps to, 0 for none.
    @location(5) snap: vec4<f32>,
    @location(6) tint: vec4<f32>,
    @location(7) quad: vec4<f32>,
    @location(8) params: vec4<f32>,
    @location(9) soft: vec4<f32>,
    @location(10) emission: vec4<f32>,
    @location(11) uv: vec2<f32>,
    // The piece's colour, sRGB with linear alpha.
    @location(12) light: vec4<f32>,
    // The piece's dark colour, sRGB; a: 1 when its page is premultiplied.
    @location(13) dark: vec4<f32>,
    // The piece's page region: u0, v0, u1, v1.
    @location(14) region: vec4<f32>,
};

struct PartsRigOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) rig: vec2<f32>,
    @location(6) light: vec4<f32>,
    @location(7) dark: vec4<f32>,
    @location(8) region: vec4<f32>,
    @location(9) emission: vec3<f32>,
    @location(10) snap: f32,
};

fn srgb_linear(value: vec3<f32>) -> vec3<f32> {
    let low = value / 12.92;
    let high = pow((value + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, value <= vec3<f32>(0.04045));
}

fn linear_srgb(value: vec3<f32>) -> vec3<f32> {
    let low = value * 12.92;
    let high = 1.055 * pow(value, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, value <= vec3<f32>(0.0031308));
}

@vertex
fn vs_parts_rig(in: PartsRigIn) -> PartsRigOut {
    let model = mat4x4<f32>(in.model0, in.model1, in.model2, in.model3);
    let world = model * vec4<f32>(in.position, 0.0, 1.0);
    var out: PartsRigOut;
    out.clip = frame.view_proj * world;
    out.world_position = world.xyz / world.w;
    out.normal = normalize((model * vec4<f32>(0.0, 0.0, 1.0, 0.0)).xyz);
    out.uv = in.uv;
    out.tint = in.tint;
    out.params = in.params;
    out.rig = in.position;
    out.light = vec4<f32>(srgb_linear(in.light.rgb), in.light.a);
    out.dark = vec4<f32>(srgb_linear(in.dark.rgb), in.dark.a);
    out.region = in.region;
    out.emission = in.emission.xyz;
    out.snap = in.snap.x;
    return out;
}

// A rig piece's colour, straight alpha, lit; `masked` discards below the
// alpha cutoff.
fn shaded_parts_rig(in: PartsRigOut, front: bool, masked: bool) -> vec4<f32> {
    // Derivatives and samples first: they need uniform control flow.
    let rig_x = dpdx(in.rig);
    let rig_y = dpdy(in.rig);
    let uv_x = dpdx(in.uv);
    let uv_y = dpdy(in.uv);
    let extent = max(in.region.zw - in.region.xy, vec2<f32>(1e-6));
    let surface = sprite_surface(in.world_position, in.normal, in.uv, (in.uv - in.region.xy) / extent, front);
    var texel = textureSample(sprite_color, sprite_color_sampler, in.uv);
    var outside = false;
    if in.snap > 0.0 {
        // The colour at the centre of the fragment's cell of the rig's
        // texel grid: the page uv there by the piece's affine map from the
        // rig's plane, which is the triangle's, taken from the screen
        // derivatives. A centre off the piece's region draws nothing.
        let centre = (floor(in.rig / in.snap) + 0.5) * in.snap;
        var uv = in.uv;
        let determinant = rig_x.x * rig_y.y - rig_x.y * rig_y.x;
        if abs(determinant) > 1e-20 {
            let to_rig = mat2x2<f32>(vec2<f32>(rig_y.y, -rig_x.y), vec2<f32>(-rig_y.x, rig_x.x)) * (1.0 / determinant);
            uv = in.uv + mat2x2<f32>(uv_x, uv_y) * to_rig * (centre - in.rig);
        }
        outside = any(uv < in.region.xy) || any(uv > in.region.zw);
        let size = textureDimensions(sprite_color);
        let at = clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2<i32>(0), vec2<i32>(size) - 1);
        texel = textureLoad(sprite_color, at, 0);
    }
    // Straight colour, then the slot's two-colour tint (Spine's): the light
    // colour multiplies the texel and the dark colour fills what it leaves.
    // A premultiplied page was premultiplied in its encoded (sRGB) values,
    // so it is divided there.
    var straight = texel.rgb;
    if in.dark.a > 0.5 {
        straight = srgb_linear(min(linear_srgb(texel.rgb) / max(texel.a, 1e-6), vec3<f32>(1.0)));
    }
    let rgb = (1.0 - straight) * in.dark.rgb + straight * in.light.rgb;
    let color = vec4<f32>(max(rgb, vec3<f32>(0.0)) * in.tint.rgb, texel.a * in.light.a * in.tint.a);

    if outside {
        discard;
    }
    let cutoff = in.params.y;
    if masked && cutoff > 0.0 && color.a < cutoff {
        discard;
    }
    return lit_sprite(surface, in.params, color, in.emission);
}

// Blended pieces, premultiplied: each slot blend mode is a blend state over
// this (`parts_rigs.rs`).
@fragment
fn fs_parts_rig(in: PartsRigOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let color = shaded_parts_rig(in, front, true);
    return vec4<f32>(color.rgb * color.a, color.a);
}

// Solid pieces, as solid sprites are drawn.
@fragment
fn fs_parts_rig_opaque(in: PartsRigOut, @builtin(front_facing) front: bool) -> OpaqueSpriteOut {
    let cutoff = in.params.y;
    let color = shaded_parts_rig(in, front, false);
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
#ifdef MOTION
    // xyz: where the particle was drawn in the previous frame.
    @location(5) previous: vec4<f32>,
#endif
};

struct ParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // The billboard's centre, for fog.
    @location(2) world_position: vec3<f32>,
    @location(3) params: vec4<f32>,
#ifdef MOTION
    @location(4) previous: vec4<f32>,
#endif
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
#ifdef MOTION
    // The corner about where the particle was, as it is placed about where
    // it is.
    var earlier = previous_clip(in.previous.xyz);
    earlier = vec4<f32>(earlier.xy + offset * select(earlier.w, 1.0, in.flipbook.w > 0.5), earlier.zw);
    out.previous = target_pixel_of(earlier);
#endif
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

// A billboard's or cube's colour, and with MOTION its motion blended by
// the colour's alpha.
struct ParticleColor {
    @location(0) color: vec4<f32>,
#ifdef MOTION
    @location(1) motion: vec4<f32>,
#endif
};

@fragment
fn fs_particle(in: ParticleOut) -> ParticleColor {
    let color = particle_color(in, 1.0);
    if color.a <= 0.001 {
        discard;
    }
    var out: ParticleColor;
    out.color = finish(color, in.world_position);
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, color.a);
#endif
    return out;
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
fn fs_particle_soft(in: ParticleOut) -> ParticleColor {
    let fade = soft_fade(in.clip.z, textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0), in.params.x);
    let color = particle_color(in, fade);
    if fade <= 0.0 || color.a <= 0.001 {
        discard;
    }
    var out: ParticleColor;
    out.color = finish(color, in.world_position);
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, color.a);
#endif
    return out;
}

struct SoftMultisampledOut {
    @location(0) color: vec4<f32>,
    // Only the samples the world leaves uncovered take the billboard, as the
    // fixed-function depth test of the world pass would decide per sample.
    @builtin(sample_mask) mask: u32,
};

struct SoftParticleMultisampledOut {
    @location(0) color: vec4<f32>,
#ifdef MOTION
    @location(1) motion: vec4<f32>,
#endif
    @builtin(sample_mask) mask: u32,
};

// The multisampled soft billboard: each of the world's samples at the pixel
// is tested and faded on its own, the fragment covers the samples the world
// is behind, and its colour fades by their mean gap.
@fragment
fn fs_particle_soft_multisampled(in: ParticleOut) -> SoftParticleMultisampledOut {
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
    var out: SoftParticleMultisampledOut;
    out.color = finish(color, in.world_position);
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, color.a);
#endif
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
#ifdef MOTION
    // xyz: where the particle was drawn in the previous frame.
    @location(5) previous: vec4<f32>,
#endif
};

struct CubeParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_position: vec3<f32>,
#ifdef MOTION
    @location(2) previous: vec4<f32>,
#endif
};

@vertex
fn vs_particle_cube(in: CubeParticleIn) -> CubeParticleOut {
    var out: CubeParticleOut;
    out.world_position = in.center.xyz + in.position * in.center.w;
    out.clip = frame.view_proj * vec4<f32>(out.world_position, 1.0);
    out.color = in.color;
#ifdef MOTION
    out.previous = previous_pixel(in.previous.xyz + in.position * in.center.w);
#endif
    return out;
}

@fragment
fn fs_particle_cube(in: CubeParticleOut) -> ParticleColor {
    if in.color.a <= 0.001 {
        discard;
    }
    var out: ParticleColor;
    out.color = finish(in.color, in.world_position);
#ifdef MOTION
    out.motion = motion_output(in.clip, in.previous, in.color.a);
#endif
    return out;
}
