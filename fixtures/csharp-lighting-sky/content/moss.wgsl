// Moss over stone (#10913): a product shader opened during an update, its
// variants compiled on the Engine's workers. Patches of moss by a value
// noise over world position darken and green the standard surface; with
// SWAY its vertices sway with the scene's wind, as overgrowth does.
//
// material.parameters[0]: rgb the moss colour, w its coverage (0 to 1).
// material.parameters[1]: x the patches' size in metres, y the sway in
// metres at full wind.

#import rusty::types::{Surface, Vertex}
#import rusty::view::frame
#import rusty::material::material
#import rusty::shade::standard_shade

fn hash(cell: vec3<f32>) -> f32 {
    return fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
}

fn noise(at: vec3<f32>) -> f32 {
    let cell = floor(at);
    let f = fract(at);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

#ifdef SWAY
fn displace(vertex: Vertex) -> vec3<f32> {
    let height = max(vertex.position.y, 0.0);
    let phase = frame.time.x * 1.7 + dot(vertex.origin.xz, vec2<f32>(0.3, 0.7));
    let sway = sin(phase) * height * material.parameters[1].y * frame.wind.z;
    return vertex.world_position + vec3<f32>(frame.wind.x * sway, 0.0, frame.wind.y * sway);
}
#endif

fn shade(surface: Surface) -> vec4<f32> {
    let size = max(material.parameters[1].x, 0.01);
    let field = noise(surface.world_position / size) * 0.65 + noise(surface.world_position / (size * 0.3)) * 0.35;
    let moss = smoothstep(1.0 - material.parameters[0].w, 1.05 - material.parameters[0].w, field);
    var mossy = surface;
    mossy.base = vec4<f32>(mix(surface.base.rgb, material.parameters[0].rgb, moss), surface.base.a);
    mossy.roughness = mix(surface.roughness, 1.0, moss);
    return standard_shade(mossy);
}
