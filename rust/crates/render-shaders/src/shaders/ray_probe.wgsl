// Rays traced against the ray scene (render-wgpu `ray_scene.rs`): each
// ray's closest hit, for tools and tests that check what the scene's
// acceleration structures hold.

enable wgpu_ray_query;

struct Ray {
    // Origin (xyz) and the farthest distance a hit counts at (w).
    origin: vec4<f32>,
    // Direction (xyz) and the instance mask the ray sees (w).
    direction: vec4<f32>,
};

struct Hit {
    // The hit's distance along the ray, or -1 for none.
    distance: f32,
    // The instance's custom data: the hit part's id.
    part: u32,
    primitive: u32,
    front_face: u32,
};

@group(0) @binding(0) var scene: acceleration_structure;
@group(0) @binding(1) var<storage, read> rays: array<Ray>;
@group(0) @binding(2) var<storage, read_write> hits: array<Hit>;

@compute @workgroup_size(64)
fn cs_trace(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= arrayLength(&rays) {
        return;
    }
    let ray = rays[id.x];
    var query: ray_query;
    rayQueryInitialize(
        &query,
        scene,
        RayDesc(RAY_FLAG_FORCE_OPAQUE, u32(ray.direction.w), 0.0, ray.origin.w, ray.origin.xyz, ray.direction.xyz),
    );
    rayQueryProceed(&query);
    let hit = rayQueryGetCommittedIntersection(&query);
    var out = Hit(-1.0, 0u, 0u, 0u);
    if hit.kind != RAY_QUERY_INTERSECTION_NONE {
        out = Hit(hit.t, hit.instance_custom_data, hit.primitive_index, u32(hit.front_face));
    }
    hits[id.x] = out;
}
