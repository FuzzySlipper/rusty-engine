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

/// Open ground 60 m across under the ambient sky, and a volume of 32 bricks
/// (4 × 2 × 4 cells of 16 m) over its middle.
fn ground_with_bricks(harness: &mut Harness) -> IndirectLightDescriptor {
    let mut stone = material("material/ground", [0.6, 0.6, 0.6, 1.0], None);
    stone.roughness = 1.0;
    harness.apply(vec![
        RenderDiff::DefineMaterial { material: stone },
        static_mesh(
            "mesh/ground",
            box_mesh([-30.0, -1.0, -30.0], [30.0, 0.0, 30.0], |_| 0),
            "material/ground",
        ),
        instance(30, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        ambient(0.6),
    ]);
    let descriptor = IndirectLightDescriptor {
        center: [0.0, 2.0, 0.0],
        extent: [24.0, 4.0, 24.0],
        spacing: 2.0,
        bounces: 1,
        ambient: IndirectAmbient::Sky,
    };
    harness.apply(vec![RenderDiff::SetIndirectLight {
        indirect_light: Some(descriptor),
    }]);
    descriptor
}

/// Looking down at the ground a few metres ahead from the volume's middle.
fn over_the_ground() -> render_host_contracts::RendererCompositionCamera {
    camera([0.0, 2.0, 2.0], 0.0, -40.0)
}

#[test]
fn an_edit_rebakes_only_the_bricks_it_reaches_and_a_torch_those_within_its_range() {
    let mut harness = own_lights();
    ground_with_bricks(&mut harness);
    let first = bake(&mut harness);
    assert_eq!(first.bricks, 32);
    assert_eq!(first.last_batch_bricks, 32, "{first:?}");
    assert!(!first.pending);
    let (_, before) = harness.render(&over_the_ground());

    // A crate in one corner: its cell's BVH rebuilds and the cells around
    // it rebake, 2 × 2 × 2 of the 4 × 2 × 4.
    harness.apply(vec![
        static_mesh(
            "mesh/crate",
            box_mesh([19.0, 0.0, 19.0], [21.0, 2.0, 21.0], |_| 0),
            "material/ground",
        ),
        instance(31, None, "mesh/crate", transform([0.0; 3], 0.0, [1.0; 3])),
    ]);
    harness.render(&over_the_ground());
    let edited = bake(&mut harness);
    assert_eq!(edited.last_batch_bricks, 8, "{edited:?}");
    assert_eq!(edited.bakes, 2);
    let (_, after) = harness.render(&over_the_ground());
    assert_eq!(
        pixel(&after, DIVIDER),
        pixel(&before, DIVIDER),
        "the ground under the camera, bricks away, is untouched"
    );

    // A torch with a 6 m range in the opposite corner: the bricks within its
    // range and one around, 3 × 2 × 3.
    harness.apply(vec![RenderDiff::CreateLight {
        handle: RenderHandle::new(3),
        parent: None,
        light: LightDescriptor::Point {
            color: [1.0, 0.7, 0.4],
            intensity: 20.0,
            enabled: true,
            position: [-20.0, 2.0, -20.0],
            range: Some(6.0),
            decay: 2.0,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    }]);
    harness.render(&over_the_ground());
    let lit = bake(&mut harness);
    assert_eq!(lit.last_batch_bricks, 18, "{lit:?}");
}

#[test]
fn moving_the_volume_bakes_only_what_it_newly_covers_and_keeps_drawing() {
    let mut harness = own_lights();
    let descriptor = ground_with_bricks(&mut harness);
    bake(&mut harness);
    let (_, before) = harness.render(&over_the_ground());
    // One brick east: the probes still covered keep their values and the
    // frame does not flicker while the new cells bake.
    harness.apply(vec![RenderDiff::SetIndirectLight {
        indirect_light: Some(IndirectLightDescriptor {
            center: [16.0, 2.0, 0.0],
            ..descriptor
        }),
    }]);
    let (_, during) = harness.render(&over_the_ground());
    let moved = bake(&mut harness);
    assert_eq!(moved.bricks, 32);
    // The column of new cells, and the column the old volume only partly
    // covered.
    assert_eq!(moved.last_batch_bricks, 16, "{moved:?}");
    let (_, after) = harness.render(&over_the_ground());
    for (name, frame) in [("during", &during), ("after", &after)] {
        let (now, was) = (pixel(frame, DIVIDER), pixel(&before, DIVIDER));
        assert!(
            (0..3).all(|c| (now[c] - was[c]).abs() <= 1),
            "{name}: {now:?} vs {was:?}"
        );
    }
}

#[test]
fn a_part_moving_between_bricks_rebakes_where_it_left_and_where_it_went() {
    let mut harness = own_lights();
    ground_with_bricks(&mut harness);
    harness.apply(vec![
        static_mesh(
            "mesh/crate",
            box_mesh([-1.0, 0.0, -1.0], [1.0, 2.0, 1.0], |_| 0),
            "material/ground",
        ),
        instance(
            31,
            None,
            "mesh/crate",
            transform([20.0, 0.0, 20.0], 0.0, [1.0; 3]),
        ),
    ]);
    bake(&mut harness);
    // Across the volume: the corner it left and the corner it reaches both
    // rebake, 8 bricks each.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(31),
        transform: Some(transform([-20.0, 0.0, -20.0], 0.0, [1.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
    harness.render(&over_the_ground());
    let moved = bake(&mut harness);
    assert_eq!(moved.last_batch_bricks, 16, "{moved:?}");
}

#[test]
fn removing_one_of_two_identical_torches_rebakes_their_bricks() {
    let mut harness = own_lights();
    ground_with_bricks(&mut harness);
    let torch = |handle: u64| RenderDiff::CreateLight {
        handle: RenderHandle::new(handle),
        parent: None,
        light: LightDescriptor::Point {
            color: [1.0, 0.7, 0.4],
            intensity: 20.0,
            enabled: true,
            position: [-20.0, 2.0, -20.0],
            range: Some(6.0),
            decay: 2.0,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    };
    harness.apply(vec![torch(3), torch(4)]);
    bake(&mut harness);
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(4),
    }]);
    harness.render(&over_the_ground());
    let dimmed = bake(&mut harness);
    assert_eq!(dimmed.last_batch_bricks, 18, "{dimmed:?}");
}

#[test]
fn the_upload_bytes_are_the_last_frames_and_zero_when_nothing_uploaded() {
    let mut harness = own_lights();
    ground_with_bricks(&mut harness);
    bake(&mut harness);
    harness.render(&over_the_ground());
    let uploaded = harness.renderer.indirect_light_readout();
    assert!(uploaded.upload_bytes > 0, "{uploaded:?}");
    harness.render(&over_the_ground());
    let idle = harness.renderer.indirect_light_readout();
    assert_eq!(idle.upload_bytes, 0, "{idle:?}");
}
