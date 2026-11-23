// A world view's depth reduced to a farthest-depth pyramid (render-wgpu
// `occlusion.rs`) for the occlusion tests. Level 0 is the depth over the
// view's region, each texel the farthest sample of its block of pixels (one
// pixel for the parts' test, four by four for the splats'), padded with the
// far plane to a power of two; each level above holds the farthest of the
// four texels under it.
//
// The pyramid lies in a storage buffer (`rusty::hiz_common`). The seed
// (`cs_seed`) writes level 0 from the depth. Each reduction (`cs_reduce`)
// writes four levels: the first from the level below it, each workgroup an
// eight by eight tile of it, and the three above from that tile in workgroup
// memory.

#import rusty::hiz_common::{Hiz, hiz_level_size, hiz_index}

struct Params {
    // The view's region in the depth texture: origin and size.
    origin: vec2<u32>,
    size: vec2<u32>,
    // The side of the block of pixels a level-0 texel covers.
    block: u32,
    // The levels a reduction writes, from `first`: the pyramid's last may
    // have fewer than four left.
    levels: u32,
    first: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(5) var depth_samples: texture_depth_multisampled_2d;
@group(0) @binding(2) var<storage, read_write> pyramid: Hiz;

const TILE: u32 = 8u;

var<workgroup> tile: array<f32, 64>;

fn store(level: u32, at: vec2<u32>, value: f32) {
    let width = hiz_level_size(pyramid.size, level).x;
    pyramid.depth[hiz_index(pyramid.starts[level], width, at)] = value;
}

fn load(level: u32, at: vec2<u32>) -> f32 {
    let width = hiz_level_size(pyramid.size, level).x;
    return pyramid.depth[hiz_index(pyramid.starts[level], width, at)];
}

// This invocation's texel of the reduction's first level (`value`, stored
// where it exists), then the levels above it from the workgroup's tile:
// each texel the farthest of the four under it, a level narrower than its
// square keeping its last texel along that side, as when read from the
// level below.
fn store_levels(local: vec2<u32>, group: vec2<u32>, value: f32) {
    let size = hiz_level_size(pyramid.size, params.first);
    let first = group * TILE + local;
    if all(first < size) {
        store(params.first, first, value);
    }
    tile[local.y * TILE + local.x] = value;
    var width = TILE;
    for (var level = 1u; level < 4u; level++) {
        // The tile's last texel of the level below, along each side.
        let below_size = max(size >> vec2<u32>(level - 1u), vec2<u32>(1u));
        let last = min(below_size - group * width, vec2<u32>(width)) - 1u;
        width = width / 2u;
        let inside = all(local < vec2<u32>(width));
        var above = 0.0;
        workgroupBarrier();
        if inside {
            let at = min(local * 2u, last);
            let next = min(local * 2u + 1u, last);
            above = max(
                max(tile[at.y * TILE + at.x], tile[at.y * TILE + next.x]),
                max(tile[next.y * TILE + at.x], tile[next.y * TILE + next.x]),
            );
        }
        workgroupBarrier();
        if inside {
            tile[local.y * TILE + local.x] = above;
            let at = group * width + local;
            let level_size = max(size >> vec2<u32>(level), vec2<u32>(1u));
            if level < params.levels && all(at < level_size) {
                store(params.first + level, at, above);
            }
        }
    }
}

@compute @workgroup_size(8, 8)
fn cs_seed(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= pyramid.size) {
        return;
    }
    var value = 1.0;
    let first = id.xy * params.block;
    if all(first < params.size) {
        value = 0.0;
        let last = min(first + params.block, params.size);
        for (var y = first.y; y < last.y; y++) {
            for (var x = first.x; x < last.x; x++) {
                value = max(value, textureLoad(depth, params.origin + vec2<u32>(x, y), 0));
            }
        }
    }
    store(0u, id.xy, value);
}

@compute @workgroup_size(8, 8)
fn cs_seed_samples(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= pyramid.size) {
        return;
    }
    var value = 1.0;
    let first = id.xy * params.block;
    if all(first < params.size) {
        value = 0.0;
        let last = min(first + params.block, params.size);
        for (var y = first.y; y < last.y; y++) {
            for (var x = first.x; x < last.x; x++) {
                for (var sample = 0u; sample < textureNumSamples(depth_samples); sample++) {
                    value = max(
                        value,
                        textureLoad(depth_samples, params.origin + vec2<u32>(x, y), i32(sample)),
                    );
                }
            }
        }
    }
    store(0u, id.xy, value);
}

@compute @workgroup_size(8, 8)
fn cs_reduce(
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    let id = group.xy * TILE + local.xy;
    let last = hiz_level_size(pyramid.size, params.first - 1u) - 1u;
    let at = min(id * 2u, last);
    let next = min(id * 2u + 1u, last);
    let below = params.first - 1u;
    let value = max(
        max(load(below, at), load(below, vec2<u32>(next.x, at.y))),
        max(load(below, vec2<u32>(at.x, next.y)), load(below, next)),
    );
    store_levels(local.xy, group.xy, value);
}
