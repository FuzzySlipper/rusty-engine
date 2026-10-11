//! Geomorphing (`MeshMorph`, `RenderDiff::SetMorphFocus`, `MORPH`): a
//! mesh under a focused parent draws whole near the focus, at its targets
//! past the morph's end and by the share between, in the world and shadow
//! passes alike; moving the focus or the node rewrites only its rows; no
//! focus draws it whole. Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// A 2×2 m floor in its node's space, rising 1 m at its targets, whose
/// morph runs from 10 to 20 m from the focus.
fn rising_floor() -> MeshPayloadDescriptor {
    let mut floor = payload(
        vec![
            -1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.0, -1.0,
        ],
        [0.0, 1.0, 0.0].repeat(4),
        vec![0.0; 8],
        vec![0, 1, 2, 0, 2, 3],
        &[(0, 6)],
    );
    floor.morph = Some(MeshMorph {
        targets: [1.0, 0.0, 0.0].repeat(4),
        start: 10.0,
        end: 20.0,
    });
    floor
}

fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/red", [1.0, 0.0, 0.0, 1.0], None),
        },
        static_mesh("mesh/floor", rising_floor(), "material/red"),
        group(1, None, Transform::IDENTITY),
        instance(
            2,
            Some(1),
            "mesh/floor",
            Transform {
                translation: [30.0, 0.0, -4.0],
                ..Transform::IDENTITY
            },
        ),
        // Ambient π: the floor shows exactly its colour.
        RenderDiff::CreateLight {
            handle: RenderHandle::new(3),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: std::f32::consts::PI,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]);
    harness
}

/// The topmost row of the image's middle column the red floor covers.
fn floor_top(harness: &mut Harness) -> u32 {
    let (_, pixels) = harness.render(&camera([30.0, 3.0, 0.0], 0.0, -35.0));
    (0..HEIGHT)
        .find(|y| {
            let at = ((y * WIDTH + WIDTH / 2) * 4) as usize;
            pixels[at] > 200 && pixels[at + 1] < 40 && pixels[at + 2] < 40
        })
        .expect("the floor is in view")
}

fn focus(at: Option<[f64; 2]>) -> RenderDiff {
    RenderDiff::SetMorphFocus {
        handle: RenderHandle::new(1),
        focus: at,
    }
}

#[test]
fn a_morphing_mesh_reaches_its_targets_with_distance_from_its_parents_focus() {
    let mut harness = scene();
    let unfocused = floor_top(&mut harness);
    // At the floor (30, -4 in the parent): whole.
    harness.apply(vec![focus(Some([30.0, -4.0]))]);
    assert_eq!(
        floor_top(&mut harness),
        unfocused,
        "near the focus it is whole"
    );
    // 100 m away: at its targets, a metre up, so its far edge stands higher.
    harness.apply(vec![focus(Some([-70.0, -4.0]))]);
    let raised = floor_top(&mut harness);
    assert!(
        raised + 8 < unfocused,
        "past the morph's end it is raised: {raised} against {unfocused}"
    );
    // 15 m away (14 to 16 m across the floor): about halfway.
    harness.apply(vec![focus(Some([15.0, -4.0]))]);
    let halfway = floor_top(&mut harness);
    assert!(
        raised < halfway && halfway < unfocused,
        "between the range's ends it is between: {halfway} ({raised}..{unfocused})"
    );
    // Moving the node moves it within the focus's reach: the same place
    // relative to the focus draws the same.
    harness.apply(vec![
        RenderDiff::Update {
            handle: RenderHandle::new(2),
            transform: Some(Transform {
                translation: [15.0, 0.0, -4.0],
                ..Transform::IDENTITY
            }),
            material: None,
            visible: None,
            metadata: None,
        },
        focus(Some([15.0, -4.0])),
    ]);
    let (_, pixels) = harness.render(&camera([15.0, 3.0, 0.0], 0.0, -35.0));
    let top = (0..HEIGHT)
        .find(|y| {
            let at = ((y * WIDTH + WIDTH / 2) * 4) as usize;
            pixels[at] > 200 && pixels[at + 1] < 40
        })
        .unwrap();
    assert_eq!(top, unfocused, "at the focus again it is whole");
    // No focus: whole.
    harness.apply(vec![focus(Some([-70.0, -4.0])), focus(None)]);
    let (_, pixels) = harness.render(&camera([15.0, 3.0, 0.0], 0.0, -35.0));
    let top = (0..HEIGHT)
        .find(|y| {
            let at = ((y * WIDTH + WIDTH / 2) * 4) as usize;
            pixels[at] > 200 && pixels[at + 1] < 40
        })
        .unwrap();
    assert_eq!(top, unfocused, "without a focus it is whole");
}
