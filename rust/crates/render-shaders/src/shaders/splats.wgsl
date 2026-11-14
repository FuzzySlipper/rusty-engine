// Gaussian splats (render-wgpu `splats.rs`): each view's splat instances in
// runs, a run being the instances that follow each other in the blended
// order. A run's Gaussians are projected to screen ellipses (`cs_project`),
// sorted back to front by a radix sort over their view depth (`cs_sort_*`,
// `cs_scan_*`), drawn into the run's layer in display space (`vs_splat`,
// `fs_splat`) and the layer composited into the HDR target at the run's place
// among the blended draws (`vs_composite`, `fs_composite*`).
//
// The kernel is the one the splats were trained with (PlayCanvas 2.23's
// gsplat): the 3D covariance projected through the view's Jacobian, 0.3 px²
// added to its diagonal, a quad reaching 2·sqrt(2λ) along each eigen-axis
// and the normalised falloff (exp(−4A) − exp(−4)) / (1 − exp(−4)) over
// A = |uv|² inside it, alpha under 1/255 dropped.
//
// Splat colours are display-referred (sRGB-encoded) and were trained by
// blending those values, so a layer blends them as they are and only the
// composite decodes the result to linear light.

// One Gaussian of a splat resource, two words of four:
// [0] centre xyz (f32 bits), colour (r, g, b, opacity as unorm8, r lowest);
// [1] rotation xy and zw (snorm16 pairs, xyzw), σ xy and σ z (f16 pairs), in
//     metres along the Gaussian's own axes.
struct Gaussian {
    centre_color: vec4<u32>,
    rotation_scale: vec4<u32>,
};

// One projected Gaussian of a run: where its quad stands and what it draws.
// `centre`: in NDC (x, y, depth); `color`: packed as the resource's;
// `axes`: the quad's two half axes in NDC (x, y, then x, y).
struct Projected {
    centre: vec3<f32>,
    color: u32,
    axes: vec4<f32>,
};

// One instance's set of Gaussians projected for a view.
struct Project {
    // Instance space to view space (camera at the origin looking down −z).
    model_view: mat4x4<f32>,
    // Instance space to the pass's clip space (jittered as the pass draws).
    clip_from_model: mat4x4<f32>,
    // x, y: focal lengths in render pixels; z, w: the viewport's size.
    lens: vec4<f32>,
    // x: the run's nearest view depth; y: 1 / its depth span; z: 1 for an
    // orthographic projection.
    depth: vec4<f32>,
    // x: the set's first slot in the run; y: its Gaussians; z: 1 for the
    // run's first set, which writes the draw's vertex count.
    range: vec4<u32>,
    // x: the set's first row in `palettes`; y: 1 when it is skinned.
    skin: vec4<u32>,
    // rgb: the colour multiplier in linear light.
    tint: vec4<f32>,
};

// The indirect draw of a run's layer: 4 vertices per visible Gaussian.
struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
};

struct Sort {
    // x: Gaussians in the run; y: its blocks of `TILE`; z: the pass's
    // digit shift.
    sizes: vec4<u32>,
};

// A sort key past every visible one: what a culled Gaussian sorts as.
const CULLED: u32 = 0xffffu;
const KEY_SPAN: f32 = 65534.0;
const ALPHA_MIN: f32 = 1.0 / 255.0;
const WORKGROUP: u32 = 256u;
const ITEMS: u32 = 4u;
const TILE: u32 = 1024u;
const DIGITS: u32 = 256u;
const DIGIT_BITS: u32 = 8u;
const INVALID: u32 = 0xffffffffu;

@group(0) @binding(0) var<uniform> project: Project;
@group(0) @binding(1) var<storage, read_write> projected: array<Projected>;
@group(0) @binding(2) var<storage, read_write> project_keys: array<u32>;
@group(0) @binding(3) var<storage, read_write> project_values: array<u32>;
@group(0) @binding(4) var<storage, read_write> draw_args: DrawArgs;
// Three rows a joint: a skinned set's joint matrices (instance space, scale
// removed, after the inverse binds).
@group(0) @binding(5) var<storage, read> palettes: array<vec4<f32>>;
@group(1) @binding(0) var<storage, read> gaussians: array<Gaussian>;
// A Gaussian's skin: four joints (a byte each, lowest first) and their four
// weights (unorm8).
@group(1) @binding(1) var<storage, read> skins: array<vec2<u32>>;

@group(0) @binding(10) var<uniform> sort: Sort;
@group(0) @binding(11) var<storage, read> keys_in: array<u32>;
@group(0) @binding(12) var<storage, read> values_in: array<u32>;
@group(0) @binding(13) var<storage, read_write> keys_out: array<u32>;
@group(0) @binding(14) var<storage, read_write> values_out: array<u32>;
@group(0) @binding(15) var<storage, read_write> counts: array<u32>;
@group(0) @binding(16) var<storage, read_write> sums: array<u32>;

@group(0) @binding(20) var<storage, read> drawn: array<Projected>;
@group(0) @binding(21) var<storage, read> order: array<u32>;

@group(1) @binding(30) var layer: texture_2d<f32>;

fn rotation_matrix(q: vec4<f32>) -> mat3x3<f32> {
    let x = q.x;
    let y = q.y;
    let z = q.z;
    let w = q.w;
    // Columns.
    return mat3x3<f32>(
        vec3<f32>(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)),
        vec3<f32>(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)),
        vec3<f32>(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)),
    );
}

// An IEEE half from its low 16 bits (`unpack2x16float` needs a capability
// not every device grants).
fn half_float(bits: u32) -> f32 {
    let exponent = (bits >> 10u) & 0x1fu;
    let mantissa = f32(bits & 0x3ffu);
    var magnitude: f32;
    if exponent == 0u {
        magnitude = mantissa * exp2(-24.0);
    } else {
        magnitude = (1.0 + mantissa / 1024.0) * exp2(f32(exponent) - 15.0);
    }
    return select(magnitude, -magnitude, (bits & 0x8000u) != 0u);
}

fn cull(slot: u32) {
    project_keys[slot] = CULLED;
    project_values[slot] = slot;
}

@compute @workgroup_size(256)
fn cs_project(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    if index >= project.range.y {
        return;
    }
    let slot = project.range.x + index;
    if index == 0u && project.range.z == 1u {
        draw_args.vertex_count = 4u;
    }
    let gaussian = gaussians[index];
    var centre = bitcast<vec3<f32>>(gaussian.centre_color.xyz);
    var color = unpack4x8unorm(gaussian.centre_color.w);
    if color.a < ALPHA_MIN {
        cull(slot);
        return;
    }
    // Skinned: the linear blend of its joints' matrices moves its centre,
    // and the blend with its columns' lengths removed turns it, so a joint's
    // scale never inflates it.
    var turn = mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0));
    if project.skin.y == 1u {
        let skin = skins[index];
        let weights = unpack4x8unorm(skin.y);
        var rows = array<vec4<f32>, 3>(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0));
        for (var influence = 0u; influence < 4u; influence = influence + 1u) {
            let weight = weights[influence];
            if weight > 0.0 {
                let joint = (skin.x >> (8u * influence)) & 0xffu;
                let row = project.skin.x + joint * 3u;
                rows[0] = rows[0] + weight * palettes[row];
                rows[1] = rows[1] + weight * palettes[row + 1u];
                rows[2] = rows[2] + weight * palettes[row + 2u];
            }
        }
        let total = weights.x + weights.y + weights.z + weights.w;
        if total > 0.0 {
            let point = vec4<f32>(centre, 1.0);
            centre = vec3<f32>(dot(rows[0], point), dot(rows[1], point), dot(rows[2], point)) / total;
            let columns = mat3x3<f32>(
                vec3<f32>(rows[0].x, rows[1].x, rows[2].x),
                vec3<f32>(rows[0].y, rows[1].y, rows[2].y),
                vec3<f32>(rows[0].z, rows[1].z, rows[2].z),
            );
            turn = mat3x3<f32>(
                columns[0] / max(length(columns[0]), 1e-8),
                columns[1] / max(length(columns[1]), 1e-8),
                columns[2] / max(length(columns[2]), 1e-8),
            );
        }
    }
    // The multiplier scales the colour in linear light; the layer keeps it
    // in display space.
    if any(project.tint.rgb != vec3<f32>(1.0)) {
        color = vec4<f32>(linear_to_srgb(srgb_to_linear(color.rgb) * project.tint.rgb), color.a);
    }
    let clip = project.clip_from_model * vec4<f32>(centre, 1.0);
    let view = (project.model_view * vec4<f32>(centre, 1.0)).xyz;
    let ortho = project.depth.z > 0.5;
    let depth = -view.z;
    if clip.w <= 1e-6 || (!ortho && depth <= 1e-6) {
        cull(slot);
        return;
    }

    // The Gaussian's covariance in view space: W·R·S·(W·R·S)ᵀ, W the
    // instance's linear part in view space (its scale scales σ too).
    let rotation = vec4<f32>(
        unpack2x16snorm(gaussian.rotation_scale.x),
        unpack2x16snorm(gaussian.rotation_scale.y),
    );
    let q = rotation / max(length(rotation), 1e-8);
    let sigma_xy = vec2<f32>(
        half_float(gaussian.rotation_scale.z & 0xffffu),
        half_float(gaussian.rotation_scale.z >> 16u),
    );
    let sigma_z = half_float(gaussian.rotation_scale.w & 0xffffu);
    let r = turn * rotation_matrix(q);
    let to_view = mat3x3<f32>(
        project.model_view[0].xyz,
        project.model_view[1].xyz,
        project.model_view[2].xyz,
    );
    let m = to_view * mat3x3<f32>(r[0] * sigma_xy.x, r[1] * sigma_xy.y, r[2] * sigma_z);
    let covariance = m * transpose(m);

    // The projection's Jacobian at the centre, in render pixels, y up.
    let focal = project.lens.xy;
    var j0: vec3<f32>;
    var j1: vec3<f32>;
    if ortho {
        j0 = vec3<f32>(focal.x, 0.0, 0.0);
        j1 = vec3<f32>(0.0, focal.y, 0.0);
    } else {
        j0 = vec3<f32>(focal.x / depth, 0.0, focal.x * view.x / (depth * depth));
        j1 = vec3<f32>(0.0, focal.y / depth, focal.y * view.y / (depth * depth));
    }
    let c0 = covariance * j0;
    let c1 = covariance * j1;
    let diagonal1 = dot(j0, c0) + 0.3;
    let off_diagonal = dot(j0, c1);
    let diagonal2 = dot(j1, c1) + 0.3;
    let mid = 0.5 * (diagonal1 + diagonal2);
    let radius = length(vec2<f32>(0.5 * (diagonal1 - diagonal2), off_diagonal));
    let lambda1 = mid + radius;
    let lambda2 = max(mid - radius, 0.1);
    let size = project.lens.zw;
    let longest = min(1024.0, min(size.x, size.y));
    let l1 = 2.0 * min(sqrt(2.0 * lambda1), longest);
    let l2 = 2.0 * min(sqrt(2.0 * lambda2), longest);

    let ndc = clip.xyz / clip.w;
    let reach = vec2<f32>(max(l1, l2)) * 2.0 / size;
    if any(abs(ndc.xy) - reach > vec2<f32>(1.0)) || ndc.z < 0.0 || ndc.z > 1.0 {
        cull(slot);
        return;
    }
    var direction = vec2<f32>(off_diagonal, lambda1 - diagonal1);
    let span = length(direction);
    direction = select(vec2<f32>(1.0, 0.0), direction / span, span > 1e-12);
    let to_ndc = 2.0 / size;
    let axis1 = l1 * direction * to_ndc;
    let axis2 = l2 * vec2<f32>(direction.y, -direction.x) * to_ndc;
    projected[slot] = Projected(ndc, pack4x8unorm(color), vec4<f32>(axis1, axis2));

    // Back to front: the farthest Gaussian sorts first.
    let along = clamp((depth - project.depth.x) * project.depth.y, 0.0, 1.0);
    project_keys[slot] = u32(round((1.0 - along) * KEY_SPAN));
    project_values[slot] = slot;
    atomicAdd(&draw_args.instance_count, 1u);
}

// --- Radix sort --------------------------------------------------------------
//
// Least significant digit first, `DIGIT_BITS` a pass, stable: each block of
// `TILE` keys counts its digits (`cs_sort_count`), the counts, digit-major
// over the blocks, are scanned into each block's first slot per digit
// (`cs_scan_*`), and each block sorts its keys by the digit with one-bit
// splits in workgroup memory and scatters them (`cs_sort_scatter`).

var<workgroup> digit_counts: array<atomic<u32>, 256>;
var<workgroup> scan_scratch: array<u32, 256>;
var<workgroup> tile_keys: array<u32, 1024>;
var<workgroup> tile_values: array<u32, 1024>;
var<workgroup> digit_starts: array<u32, 256>;

fn digit_of(key: u32) -> u32 {
    return (key >> sort.sizes.z) & (DIGITS - 1u);
}

// The workgroup's exclusive prefix sum of `value` by invocation; the total
// is in `scan_scratch[WORKGROUP - 1]` until the next scan.
fn exclusive_scan(local: u32, value: u32) -> u32 {
    scan_scratch[local] = value;
    workgroupBarrier();
    for (var offset = 1u; offset < WORKGROUP; offset = offset * 2u) {
        var before = 0u;
        if local >= offset {
            before = scan_scratch[local - offset];
        }
        workgroupBarrier();
        scan_scratch[local] = scan_scratch[local] + before;
        workgroupBarrier();
    }
    return scan_scratch[local] - value;
}

@compute @workgroup_size(256)
fn cs_sort_count(
    @builtin(local_invocation_index) local: u32,
    @builtin(workgroup_id) block: vec3<u32>,
) {
    atomicStore(&digit_counts[local], 0u);
    workgroupBarrier();
    let first = block.x * TILE + local * ITEMS;
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        if index < sort.sizes.x {
            atomicAdd(&digit_counts[digit_of(keys_in[index])], 1u);
        }
    }
    workgroupBarrier();
    counts[local * sort.sizes.y + block.x] = atomicLoad(&digit_counts[local]);
}

// `counts` scanned within each group of `TILE`, each group's total to `sums`.
@compute @workgroup_size(256)
fn cs_scan_local(
    @builtin(local_invocation_index) local: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let entries = DIGITS * sort.sizes.y;
    let first = group.x * TILE + local * ITEMS;
    var values: array<u32, 4>;
    var total = 0u;
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        var value = 0u;
        if index < entries {
            value = counts[index];
        }
        values[item] = value;
        total = total + value;
    }
    var running = exclusive_scan(local, total);
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        if index < entries {
            counts[index] = running;
        }
        running = running + values[item];
    }
    if local == WORKGROUP - 1u {
        sums[group.x] = running;
    }
}

// The groups' totals scanned in one workgroup (at most `TILE` groups).
@compute @workgroup_size(256)
fn cs_scan_sums(@builtin(local_invocation_index) local: u32) {
    let groups = (DIGITS * sort.sizes.y + TILE - 1u) / TILE;
    let first = local * ITEMS;
    var values: array<u32, 4>;
    var total = 0u;
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        var value = 0u;
        if index < groups {
            value = sums[index];
        }
        values[item] = value;
        total = total + value;
    }
    var running = exclusive_scan(local, total);
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        if index < groups {
            sums[index] = running;
        }
        running = running + values[item];
    }
}

@compute @workgroup_size(256)
fn cs_scan_add(
    @builtin(local_invocation_index) local: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let entries = DIGITS * sort.sizes.y;
    let first = group.x * TILE + local * ITEMS;
    let add = sums[group.x];
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        if index < entries {
            counts[index] = counts[index] + add;
        }
    }
}

@compute @workgroup_size(256)
fn cs_sort_scatter(
    @builtin(local_invocation_index) local: u32,
    @builtin(workgroup_id) block: vec3<u32>,
) {
    let first = block.x * TILE + local * ITEMS;
    var keys: array<u32, 4>;
    var values: array<u32, 4>;
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        let index = first + item;
        if index < sort.sizes.x {
            keys[item] = keys_in[index];
            values[item] = values_in[index];
        } else {
            // Past the run: sorts after every key of the tile, never written.
            keys[item] = INVALID;
            values[item] = INVALID;
        }
    }
    // Sort the tile by the digit, one bit at a time, stably.
    for (var bit = 0u; bit < DIGIT_BITS; bit = bit + 1u) {
        let shift = sort.sizes.z + bit;
        var zeros = 0u;
        for (var item = 0u; item < ITEMS; item = item + 1u) {
            zeros = zeros + 1u - ((keys[item] >> shift) & 1u);
        }
        var zeros_before = exclusive_scan(local, zeros);
        let total_zeros = scan_scratch[WORKGROUP - 1u];
        var ones_before = local * ITEMS - zeros_before;
        for (var item = 0u; item < ITEMS; item = item + 1u) {
            var position: u32;
            if ((keys[item] >> shift) & 1u) == 0u {
                position = zeros_before;
                zeros_before = zeros_before + 1u;
            } else {
                position = total_zeros + ones_before;
                ones_before = ones_before + 1u;
            }
            tile_keys[position] = keys[item];
            tile_values[position] = values[item];
        }
        workgroupBarrier();
        for (var item = 0u; item < ITEMS; item = item + 1u) {
            keys[item] = tile_keys[local * ITEMS + item];
            values[item] = tile_values[local * ITEMS + item];
        }
        workgroupBarrier();
    }
    // Where each digit's keys start in the sorted tile.
    atomicStore(&digit_counts[local], 0u);
    workgroupBarrier();
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        if values[item] != INVALID {
            atomicAdd(&digit_counts[digit_of(keys[item])], 1u);
        }
    }
    workgroupBarrier();
    digit_starts[local] = exclusive_scan(local, atomicLoad(&digit_counts[local]));
    workgroupBarrier();
    for (var item = 0u; item < ITEMS; item = item + 1u) {
        if values[item] == INVALID {
            continue;
        }
        let digit = digit_of(keys[item]);
        let rank = local * ITEMS + item - digit_starts[digit];
        let destination = counts[digit * sort.sizes.y + block.x] + rank;
        keys_out[destination] = keys[item];
        values_out[destination] = values[item];
    }
}

// --- The layer ---------------------------------------------------------------

struct SplatVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
};

@vertex
fn vs_splat(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> SplatVarying {
    let splat = drawn[order[instance]];
    let corner = vec2<f32>(f32(vertex & 1u), f32((vertex >> 1u) & 1u)) * 2.0 - 1.0;
    var out: SplatVarying;
    out.position = vec4<f32>(
        splat.centre.xy + corner.x * splat.axes.xy + corner.y * splat.axes.zw,
        splat.centre.z,
        1.0,
    );
    out.uv = corner;
    out.color = unpack4x8unorm(splat.color);
    return out;
}

const EXP4: f32 = 0.018315639;

@fragment
fn fs_splat(in: SplatVarying) -> @location(0) vec4<f32> {
    let a = dot(in.uv, in.uv);
    if a > 1.0 {
        discard;
    }
    let alpha = (exp(-4.0 * a) - EXP4) / (1.0 - EXP4) * in.color.a;
    if alpha < ALPHA_MIN {
        discard;
    }
    // Premultiplied, in display space.
    return vec4<f32>(in.color.rgb * alpha, alpha);
}

// --- The composite -----------------------------------------------------------

struct CompositeVarying {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_composite(@builtin(vertex_index) vertex: u32) -> CompositeVarying {
    let uv = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    var out: CompositeVarying;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((color + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}

fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let clamped = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let low = clamped * 12.92;
    let high = 1.055 * pow(clamped, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, clamped <= vec3<f32>(0.0031308));
}

// The layer's premultiplied display colour at the fragment, decoded to
// premultiplied linear light.
fn composited(position: vec4<f32>) -> vec4<f32> {
    let layered = textureLoad(layer, vec2<i32>(position.xy), 0);
    if layered.a <= 0.0 {
        discard;
    }
    let display = clamp(layered.rgb / layered.a, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(srgb_to_linear(display) * layered.a, layered.a);
}

@fragment
fn fs_composite(in: CompositeVarying) -> @location(0) vec4<f32> {
    return composited(in.position);
}

struct ReactiveComposite {
    @location(0) color: vec4<f32>,
    @location(1) motion: vec4<f32>,
};

// `rusty::types::REACTIVE_OVERLAY`: the temporal stage keeps no history
// under the splats, which write no motion of their own yet.
const REACTIVE_OVERLAY: f32 = 1.0;

@fragment
fn fs_composite_reactive(in: CompositeVarying) -> ReactiveComposite {
    let color = composited(in.position);
    return ReactiveComposite(color, vec4<f32>(0.0, 0.0, REACTIVE_OVERLAY, color.a));
}
