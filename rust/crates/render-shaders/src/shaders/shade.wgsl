#define_import_path rusty::shade

// The standard shade stage: a surface lit by the pass's lights
// (`rusty::lighting`), plus its emission, then finished (`rusty::finish`).
// Unlit materials are finished as they are. A product shader may call it and
// change its result, or shade in its own way.

#import rusty::types::Surface
#import rusty::lighting::standard_radiance
#import rusty::finish::finish

fn standard_shade(surface: Surface) -> vec4<f32> {
#ifdef UNLIT
    return finish(surface.base, surface.world_position);
#else
    let radiance = standard_radiance(surface.base.rgb, surface.normal, surface.world_position,
        surface.roughness, surface.metalness, surface.occlusion) + surface.emission;
    return finish(vec4<f32>(radiance, surface.base.a), surface.world_position);
#endif
}
