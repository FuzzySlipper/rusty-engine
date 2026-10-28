// The splats' GPU radix sort (`splats.rs`). It follows the count / scan /
// scatter structure of Brush's `brush-sort` (Apache-2.0, after AMD
// FidelityFX parallel sort): 4 bits a pass, a stable in-tile split of two
// 2-bit steps, block counts stored bin major so one exclusive scan gives
// every block its output offsets. Written here in WGSL without subgroup
// operations.

const WG: u32 = 256u;
const PER_THREAD: u32 = 4u;
const BLOCK: u32 = 1024u;
const BINS: u32 = 16u;

struct SortPass {
    shift: u32,
}

@group(0) @binding(0) var<uniform> sort_pass: SortPass;
@group(0) @binding(1) var<storage, read> sort_count: array<u32>;
@group(0) @binding(2) var<storage, read> src_keys: array<u32>;
@group(0) @binding(3) var<storage, read> src_values: array<u32>;
@group(0) @binding(4) var<storage, read_write> block_counts: array<u32>;
@group(0) @binding(5) var<storage, read_write> dst_keys: array<u32>;
@group(0) @binding(6) var<storage, read_write> dst_values: array<u32>;

var<workgroup> histogram: array<atomic<u32>, 16>;
var<workgroup> scan: array<vec4<u32>, 256>;
var<workgroup> tile_keys: array<u32, 256>;
var<workgroup> tile_values: array<u32, 256>;
var<workgroup> bin_base: array<u32, 16>;
var<workgroup> bin_start: array<u32, 16>;
var<workgroup> scan_scalar: array<u32, 256>;

fn blocks_of(count: u32) -> u32 {
    return (count + BLOCK - 1u) / BLOCK;
}

@compute @workgroup_size(256)
fn cs_sort_count(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) local: u32) {
    let count = sort_count[0];
    let blocks = blocks_of(count);
    if group.x >= blocks {
        return;
    }
    if local < BINS {
        atomicStore(&histogram[local], 0u);
    }
    workgroupBarrier();
    var index = group.x * BLOCK + local;
    for (var i = 0u; i < PER_THREAD; i++) {
        if index < count {
            atomicAdd(&histogram[(src_keys[index] >> sort_pass.shift) & 0xfu], 1u);
        }
        index += WG;
    }
    workgroupBarrier();
    if local < BINS {
        block_counts[local * blocks + group.x] = atomicLoad(&histogram[local]);
    }
}

// One workgroup: an exclusive scan over every bin's block counts in place.
@compute @workgroup_size(256)
fn cs_sort_scan(@builtin(local_invocation_index) local: u32) {
    let total = blocks_of(sort_count[0]) * BINS;
    let per = (total + WG - 1u) / WG;
    let first = local * per;
    var sum = 0u;
    for (var i = 0u; i < per; i++) {
        if first + i < total {
            sum += block_counts[first + i];
        }
    }
    scan_scalar[local] = sum;
    workgroupBarrier();
    for (var step = 1u; step < WG; step <<= 1u) {
        var add = 0u;
        if local >= step {
            add = scan_scalar[local - step];
        }
        workgroupBarrier();
        scan_scalar[local] += add;
        workgroupBarrier();
    }
    var running = scan_scalar[local] - sum;
    for (var i = 0u; i < per; i++) {
        if first + i < total {
            let v = block_counts[first + i];
            block_counts[first + i] = running;
            running += v;
        }
    }
}

// Exclusive scan of four counters across the workgroup; returns the
// exclusive prefix and leaves the totals in scan[255] + its input.
fn scan4(local: u32, value: vec4<u32>) -> vec4<u32> {
    scan[local] = value;
    workgroupBarrier();
    for (var step = 1u; step < WG; step <<= 1u) {
        var add = vec4<u32>(0u);
        if local >= step {
            add = scan[local - step];
        }
        workgroupBarrier();
        scan[local] += add;
        workgroupBarrier();
    }
    return scan[local] - value;
}

@compute @workgroup_size(256)
fn cs_sort_scatter(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) local: u32) {
    let count = sort_count[0];
    let blocks = blocks_of(count);
    if group.x >= blocks {
        return;
    }
    if local < BINS {
        bin_base[local] = block_counts[local * blocks + group.x];
    }
    workgroupBarrier();
    var index = group.x * BLOCK + local;
    for (var tile = 0u; tile < PER_THREAD; tile++) {
        if local < BINS {
            atomicStore(&histogram[local], 0u);
        }
        var key = 0xffffffffu;
        var value = 0u;
        if index < count {
            key = src_keys[index];
            value = src_values[index];
        }
        // Stable split on the digit, two bits at a time.
        for (var bit = 0u; bit < 4u; bit += 2u) {
            let pair = ((key >> sort_pass.shift) >> bit) & 3u;
            let one = vec4<u32>(vec4<bool>(pair == 0u, pair == 1u, pair == 2u, pair == 3u));
            let before = scan4(local, one);
            let totals = scan[WG - 1u];
            let starts = vec4<u32>(0u, totals.x, totals.x + totals.y, totals.x + totals.y + totals.z);
            let at = starts[pair] + before[pair];
            workgroupBarrier();
            tile_keys[at] = key;
            tile_values[at] = value;
            workgroupBarrier();
            key = tile_keys[local];
            value = tile_values[local];
            workgroupBarrier();
        }
        let digit = (key >> sort_pass.shift) & 0xfu;
        atomicAdd(&histogram[digit], 1u);
        workgroupBarrier();
        if local == 0u {
            var acc = 0u;
            for (var b = 0u; b < BINS; b++) {
                bin_start[b] = acc;
                acc += atomicLoad(&histogram[b]);
            }
        }
        workgroupBarrier();
        let out = bin_base[digit] + local - bin_start[digit];
        if out < count {
            dst_keys[out] = key;
            dst_values[out] = value;
        }
        workgroupBarrier();
        if local < BINS {
            bin_base[local] += atomicLoad(&histogram[local]);
        }
        workgroupBarrier();
        index += WG;
    }
}

