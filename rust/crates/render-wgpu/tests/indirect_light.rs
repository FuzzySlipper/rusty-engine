//! Indirect light (`probes.rs`): one probe volume baked from the scene's own
//! triangles and lights, standing in for the ambient rows and the sky's
//! light where it covers a surface. Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// The scene's own lights only.
fn own_lights() -> Harness {
    Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    })
}

fn pixel(rgba: &[u8], (x, y): (u32, u32)) -> [i32; 3] {
    let at = ((y * WIDTH + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]].map(i32::from)
}

/// A room 8 m wide and deep and 4 m tall around the origin, its floor at
/// y = 0, each wall a slab whose inner face is the room's, with a
/// half-height divider across its middle. `roof` closes it.
fn room(harness: &mut Harness, roof: bool) {
    let mut stone = material("material/stone", [0.7, 0.7, 0.7, 1.0], None);
    stone.roughness = 1.0;
    let mut ops = vec![RenderDiff::DefineMaterial { material: stone }];
    let slabs: [([f32; 3], [f32; 3]); 7] = [
        ([-5.0, -1.0, -5.0], [5.0, 0.0, 5.0]),
        ([-5.0, 4.0, -5.0], [5.0, 5.0, 5.0]),
        ([-5.0, 0.0, -5.0], [-4.0, 4.0, 5.0]),
        ([4.0, 0.0, -5.0], [5.0, 4.0, 5.0]),
        ([-5.0, 0.0, -5.0], [5.0, 4.0, -4.0]),
        ([-5.0, 0.0, 4.0], [5.0, 4.0, 5.0]),
        ([-3.0, 0.0, 0.0], [3.0, 2.0, 0.2]),
    ];
    for (index, (min, max)) in slabs.iter().enumerate() {
        if index == 1 && !roof {
            continue;
        }
        let asset = format!("mesh/slab{index}");
        ops.push(static_mesh(
            &asset,
            box_mesh(*min, *max, |_| 0),
            "material/stone",
        ));
        ops.push(instance(
            10 + index as u64,
            None,
            &asset,
            transform([0.0; 3], 0.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
}

fn ambient(intensity: f32) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(1),
        parent: None,
        light: LightDescriptor::Ambient {
            color: [1.0, 1.0, 1.0],
            intensity,
            enabled: true,
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    }
}

/// A torch under the roof on the far side of the divider from the camera.
fn torch() -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(2),
        parent: None,
        light: LightDescriptor::Point {
            color: [1.0, 0.8, 0.6],
            intensity: 30.0,
            enabled: true,
            position: [0.0, 3.0, 2.5],
            range: Some(12.0),
            decay: 2.0,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    }
}

fn volume(ambient: IndirectAmbient, bounces: u32) -> RenderDiff {
    RenderDiff::SetIndirectLight {
        indirect_light: Some(IndirectLightDescriptor {
            center: [0.0, 2.0, 0.0],
            extent: [3.5, 1.5, 3.5],
            spacing: 1.0,
            bounces,
            ambient,
        }),
    }
}

const NO_VOLUME: RenderDiff = RenderDiff::SetIndirectLight {
    indirect_light: None,
};

/// From the near half of the room, looking at the divider's near face,
/// which faces away from the torch.
fn at_the_divider() -> render_host_contracts::RendererCompositionCamera {
    camera([0.0, 1.0, -2.5], 180.0, 0.0)
}

const DIVIDER: (u32, u32) = (WIDTH / 2, HEIGHT / 2);

/// Bake the requested volume now and give its readout.
fn bake(harness: &mut Harness) -> render_wgpu::IndirectLightReadout {
    harness
        .renderer
        .bake_indirect_light_now()
        .expect("a volume is requested")
}

#[test]
fn a_closed_room_loses_the_ambient_sky_to_the_volume_a_floor_keeps_it_and_off_restores_the_frame() {
    let mut harness = own_lights();
    room(&mut harness, true);
    harness.apply(vec![ambient(0.6)]);
    let (_, before) = harness.render(&at_the_divider());
    let lit = pixel(&before, DIVIDER);
    assert!(lit[0] > 60, "the ambient light lights the divider: {lit:?}");

    harness.apply(vec![volume(IndirectAmbient::Sky, 1)]);
    let readout = bake(&mut harness);
    assert_eq!(readout.dims, [8, 4, 8]);
    assert_eq!(readout.probes, 256);
    assert!(readout.triangles >= 7 * 12, "{readout:?}");
    assert_eq!(readout.bakes, 1);
    let dark = pixel(&harness.render(&at_the_divider()).1, DIVIDER);
    assert!(
        dark.iter().all(|c| *c <= 4),
        "no sky reaches a closed room: {dark:?}"
    );

    harness.apply(vec![volume(IndirectAmbient::Floor, 1)]);
    bake(&mut harness);
    let floor = pixel(&harness.render(&at_the_divider()).1, DIVIDER);
    assert!(
        (0..3).all(|c| (floor[c] - lit[c]).abs() <= 3),
        "a floor keeps the ambient light: {floor:?} vs {lit:?}"
    );

    harness.apply(vec![NO_VOLUME]);
    let (_, after) = harness.render(&at_the_divider());
    assert_eq!(after, before, "off draws exactly as before");
    assert!(!harness.renderer.indirect_light_readout().enabled);
}

#[test]
fn a_torch_bounces_off_the_room_onto_a_face_it_cannot_reach() {
    let mut harness = own_lights();
    room(&mut harness, true);
    harness.apply(vec![torch()]);
    let unlit = pixel(&harness.render(&at_the_divider()).1, DIVIDER);
    assert!(
        unlit.iter().all(|c| *c <= 2),
        "the divider faces away from the torch: {unlit:?}"
    );

    harness.apply(vec![volume(IndirectAmbient::Sky, 2)]);
    let readout = bake(&mut harness);
    let bounced = pixel(&harness.render(&at_the_divider()).1, DIVIDER);
    assert!(
        bounced[0] >= 12,
        "the torch's light bounces off the roof and walls onto it: {bounced:?} {readout:?}"
    );
    assert!(
        bounced[0] > bounced[2],
        "and keeps the torch's warmth: {bounced:?}"
    );
}

#[test]
fn an_open_roof_lets_the_ambient_sky_into_the_room() {
    let mut harness = own_lights();
    room(&mut harness, false);
    harness.apply(vec![ambient(0.6)]);
    let lit = pixel(&harness.render(&at_the_divider()).1, DIVIDER);
    harness.apply(vec![volume(IndirectAmbient::Sky, 1)]);
    bake(&mut harness);
    let open = pixel(&harness.render(&at_the_divider()).1, DIVIDER);
    assert!(
        open[0] >= 12 && open[0] <= lit[0] - 12,
        "the divider sees part of the sky: {open:?} under an open sky of {lit:?}"
    );
}
