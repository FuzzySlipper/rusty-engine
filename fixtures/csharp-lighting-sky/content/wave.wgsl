// A banner on the wind (#9541): a product displace stage. The card hangs
// from its pole edge (uv.x 0) and waves further toward its free edge, by
// the scene's wind strength, with presentation time. The shadow pass calls
// it too, so the banner's shadow waves with it.
#import rusty::types::{Surface, Vertex}
#import rusty::view::frame
#import rusty::material::material
#import rusty::shade::standard_shade

fn displace(vertex: Vertex) -> vec3<f32> {
    // parameters[0]: x the wave's amplitude in metres, y its rate.
    let along = vertex.uv.x;
    let phase = frame.time.x * material.parameters[0].y - along * 4.0;
    let wave = sin(phase) * along * along * material.parameters[0].x * frame.wind.z;
    return vertex.world_position + vec3<f32>(0.0, wave * 0.35, wave);
}

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface);
}
