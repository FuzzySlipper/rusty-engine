// Card splats (render-wgpu `splats.rs`): a run of one card asset's
// instances, each card a textured rectangle in 3D carrying one tile of the
// asset's atlas. A run's cards are skinned and projected to clip space
// (`cs_project_card`), sorted back to front by their centres' view depth with
// the Gaussians' radix sort (`splats.wgsl`), and drawn into the run's layer in
// display space (`vs_card`, `fs_card*`). The layer is composited as a
// Gaussian run's is (`splats.wgsl` `fs_composite*`).
//
// A card's corner c in {−1, +1}² stands at centre + c.x·u + c.y·v, u and v
// its half-axes. Its corners are full clip-space positions, so its tile's
// texture coordinates are perspective-correct across it and its depth varies
// across it as the plane's does. Its tile reads
// (tile·side + 0.5 + (c + 1) / 2 · (side − 1)) / atlas size, as the cards
// were trained: linear filtering never reaches a neighbouring tile.
//
// The atlas holds display-referred colour and straight alpha, as a Gaussian's
// colour is: the layer blends it as it is and only the composite decodes it.

#import rusty::splat_layer::{srgb_to_linear, linear_to_srgb}
#import rusty::splat_common::{project, draw_args, skins, ALPHA_MIN, rotation_matrix, half_float, blended_rows, moved, cull, group_tint, keep, hidden}

// One card of a card resource, two words of four:
// [0] centre xyz (f32 bits), its tile's top-left texel (x, y as u16, x
//     lowest);
// [1] rotation xy and zw (snorm16 pairs, xyzw: local x along u, y along v),
//     half-extents along x and y (an f16 pair, metres), the tile's side in
//     texels.
struct Card {
    centre_tile: vec4<u32>,
    frame: vec4<u32>,
};

// One projected card of a run. `centre`, `u`, `v`: its centre and half-axes
// in the pass's clip space; `motion`: xy its centre's motion in target
// pixels; `multiplier`: rgb its instance's colour multiplier in linear
// light; `look`: x its tile's top-left texel (as the resource's), y the
// tile's side, z its group's tint (sRGB target rgb and weight, unorm8).
struct ProjectedCard {
    centre: vec4<f32>,
    u: vec4<f32>,
    v: vec4<f32>,
    motion: vec4<f32>,
    multiplier: vec4<f32>,
    look: vec4<u32>,
};

@group(0) @binding(1) var<storage, read_write> projected: array<ProjectedCard>;
@group(1) @binding(0) var<storage, read> cards: array<Card>;

@group(0) @binding(20) var<storage, read> drawn: array<ProjectedCard>;
@group(0) @binding(21) var<storage, read> order: array<u32>;
// The run's atlas: Rgba8Unorm, one level, sampled linearly and clamped.
@group(1) @binding(40) var atlas: texture_2d<f32>;
@group(1) @binding(41) var atlas_sampler: sampler;

@compute @workgroup_size(256)
fn cs_project_card(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    if index >= project.range.y {
        return;
    }
    let slot = project.range.x + index;
    if index == 0u && project.range.z == 1u {
        draw_args.vertex_count = 4u;
    }
    let card = cards[index];
    var centre = bitcast<vec3<f32>>(card.centre_tile.xyz);
    let rotation = vec4<f32>(unpack2x16snorm(card.frame.x), unpack2x16snorm(card.frame.y));
    let r = rotation_matrix(rotation / max(length(rotation), 1e-8));
    var u = r[0] * half_float(card.frame.z & 0xffffu);
    var v = r[1] * half_float(card.frame.z >> 16u);
    // Skinned: the linear blend of its joints' matrices moves its centre and
    // carries its half-axes, as it carries the figure around the card.
    var previous_centre = centre;
    if project.skin.y == 1u {
        let skin = skins[index];
        let rows = blended_rows(project.skin.x, skin);
        if rows[3].w > 0.0 {
            let rest = centre;
            centre = moved(rows, rest);
            previous_centre = centre;
            if project.skin.w == 1u && project.skin.z != project.skin.x {
                previous_centre = moved(blended_rows(project.skin.z, skin), rest);
            }
            let blend = transpose(mat3x3<f32>(rows[0].xyz, rows[1].xyz, rows[2].xyz)) * (1.0 / rows[3].w);
            u = blend * u;
            v = blend * v;
        }
    }
    let clip = project.clip_from_model * vec4<f32>(centre, 1.0);
    let clip_u = project.clip_from_model * vec4<f32>(u, 0.0);
    let clip_v = project.clip_from_model * vec4<f32>(v, 0.0);
    let depth = -(project.model_view * vec4<f32>(centre, 1.0)).z;
    let ortho = project.depth.z > 0.5;
    if clip.w <= 1e-6 || (!ortho && depth <= 1e-6) {
        cull(slot);
        return;
    }
    // Dropped when all four corners lie beyond one side of the view, or
    // when, all in front of the eye, they lie behind the opaque depth over
    // their box from their nearest depth on.
    var beyond = vec3<bool>(true);
    var behind = vec3<bool>(true);
    var in_front = true;
    var low = vec2<f32>(1e30);
    var high = vec2<f32>(-1e30);
    var nearest = 1.0;
    for (var corner = 0u; corner < 4u; corner = corner + 1u) {
        let c = vec2<f32>(f32(corner & 1u), f32(corner >> 1u)) * 2.0 - 1.0;
        let p = clip + c.x * clip_u + c.y * clip_v;
        beyond = beyond & (p.xyz > vec3<f32>(p.w));
        behind = behind & (p.xyz < vec3<f32>(-p.w, -p.w, 0.0));
        in_front = in_front && p.w > 1e-6;
        let ndc = p.xyz / max(p.w, 1e-6);
        let pixel = project.pixels.xy + ndc.xy * project.pixels.zw;
        low = min(low, pixel);
        high = max(high, pixel);
        nearest = min(nearest, ndc.z);
    }
    if any(beyond) || any(behind) || (in_front && hidden(low, high, nearest)) {
        cull(slot);
        return;
    }
    // The centre's motion: from where it stood to where the pass draws it.
    var motion = vec2<f32>(0.0);
    if project.skin.w == 1u {
        let previous = project.previous_pixels * vec4<f32>(previous_centre, 1.0);
        if previous.w > 1e-6 {
            motion = project.pixels.xy + clip.xy / clip.w * project.pixels.zw - previous.xy / previous.w;
        }
    }
    projected[slot] = ProjectedCard(
        clip,
        clip_u,
        clip_v,
        vec4<f32>(motion, 0.0, 0.0),
        vec4<f32>(project.tint.rgb, 0.0),
        vec4<u32>(card.centre_tile.w, card.frame.w, pack4x8unorm(group_tint(index)), 0u),
    );
    keep(slot, depth);
}

struct CardVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) tint: vec4<f32>,
    @location(2) @interpolate(flat) multiplier: vec3<f32>,
    @location(3) @interpolate(flat) motion: vec2<f32>,
};

@vertex
fn vs_card(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> CardVarying {
    let card = drawn[order[instance]];
    let corner = vec2<f32>(f32(vertex & 1u), f32((vertex >> 1u) & 1u)) * 2.0 - 1.0;
    let tile = vec2<f32>(f32(card.look.x & 0xffffu), f32(card.look.x >> 16u));
    let side = f32(card.look.y);
    var out: CardVarying;
    out.position = card.centre + corner.x * card.u + corner.y * card.v;
    out.uv = (tile + 0.5 + (corner + 1.0) * 0.5 * (side - 1.0)) / vec2<f32>(textureDimensions(atlas));
    out.tint = unpack4x8unorm(card.look.z);
    out.multiplier = card.multiplier.rgb;
    out.motion = card.motion.xy;
    return out;
}

// The card's premultiplied display colour at the fragment: its texel, its
// alpha at most 0.99 as the cards were trained, moved toward its group's
// tint in display space and scaled by the multiplier in linear light.
// Fragments too faint to show are discarded.
fn card_colour(in: CardVarying) -> vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    let alpha = min(0.99, texel.a);
    if alpha < ALPHA_MIN {
        discard;
    }
    var rgb = mix(texel.rgb, in.tint.rgb, in.tint.a);
    if any(in.multiplier != vec3<f32>(1.0)) {
        rgb = linear_to_srgb(srgb_to_linear(rgb) * in.multiplier);
    }
    return vec4<f32>(rgb * alpha, alpha);
}

struct CardOut {
    // Premultiplied, in display space.
    @location(0) color: vec4<f32>,
    // 1 − its depth at the fragment and its centre's motion, premultiplied
    // by its alpha as the colour is.
    @location(1) surface: vec4<f32>,
};

@fragment
fn fs_card(in: CardVarying) -> CardOut {
    let color = card_colour(in);
    return CardOut(color, vec4<f32>(vec3<f32>(1.0 - in.position.z, in.motion) * color.a, color.a));
}

// A layer that keeps no surface: nothing after its run reads the view's
// depth or motion.
@fragment
fn fs_card_colour(in: CardVarying) -> @location(0) vec4<f32> {
    return card_colour(in);
}
