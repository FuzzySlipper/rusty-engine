#define_import_path rusty::shade

// The standard shade stage: a surface lit by the pass's lights
// (`rusty::lighting`), plus its emission, then finished (`rusty::finish`).
// Unlit materials are finished as they are. A product shader may call it and
// change its result, or shade in its own way.

#import rusty::types::Surface
#import rusty::lighting::standard_radiance
#import rusty::finish::finish

#ifdef GUIDES
// The surface the fragment's guides describe (render-wgpu `motion.rs`): the
// one the world pass hands the shade stage, or the one a product shader
// last shades through `standard_shade`, so a product that changes its
// material and shades that gets guides of the material it drew.
var<private> guided: Surface;
#endif

fn standard_shade(surface: Surface) -> vec4<f32> {
#ifdef GUIDES
    guided = surface;
#endif
#ifdef UNLIT
    return finish(surface.base, surface.world_position);
#else
    let radiance = standard_radiance(surface.base.rgb, surface.normal, surface.world_position,
        surface.roughness, surface.metalness, surface.occlusion) + surface.emission;
    return finish(vec4<f32>(radiance, surface.base.a), surface.world_position);
#endif
}
