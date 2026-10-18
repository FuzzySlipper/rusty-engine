//! Snow cover (`lighting.wgsl` `snowed`, `CameraView.SetSnow`): snow lies on
//! heightfield terrain, meshes and scattered copies that face up under the
//! open sky, more as the amount grows; walls, the underside of an overhang
//! and the ground and a crate under it stay clear; amount 0 draws exactly as
//! none; and a texel density steps the cover's edge. Same harness as
//! `tests/screenshots.rs`.

mod support;
mod terrain;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;
use terrain::Terrain;

/// Gently rolling ground about y = 0, steepening toward the far right.
fn ground(x: f64, z: f64) -> f32 {
    (0.25 * (x * 0.7).sin() * (z * 0.5).cos() + 0.15 * (z * 0.3).sin()) as f32
}

const SNOW_WHITE: [f32; 3] = [0.82, 0.86, 0.92];

/// Brown heightfield ground, a wall at the back, a roof overhanging the
/// left of the view with a crate under it, and short posts scattered on the
/// right, lit by a sun, a hemisphere fill and an ambient light whose shadow
/// is the open sky.
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut terrain = Terrain::new(ground);
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.35, 0.28, 0.2, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/stone", [0.45, 0.45, 0.48, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/dark", [0.1, 0.12, 0.1, 1.0], None),
        },
    ]);
    harness.apply(terrain.settle([0.0, 0.0]));
    harness.apply(vec![
        static_mesh(
            "mesh/wall",
            box_mesh([-6.0, -1.0, -14.0], [6.0, 6.0, -13.0], |_| 0),
            "material/stone",
        ),
        static_mesh(
            "mesh/roof",
            box_mesh([-14.0, 3.0, -10.0], [-1.5, 3.4, 10.0], |_| 0),
            "material/stone",
        ),
        static_mesh(
            "mesh/crate",
            box_mesh([-6.75, -0.5, -0.25], [-5.25, 0.9, 1.25], |_| 0),
            "material/stone",
        ),
        static_mesh(
            "mesh/post",
            box_mesh([-0.2, -0.5, -0.2], [0.2, 0.35, 0.2], |_| 0),
            "material/dark",
        ),
        instance(20, None, "mesh/wall", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(21, None, "mesh/roof", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(22, None, "mesh/crate", transform([0.0; 3], 0.0, [1.0; 3])),
        group(23, None, transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateScatterPatch {
            handle: RenderHandle::new(24),
            parent: Some(RenderHandle::new(23)),
            patch: ScatterPatchDescriptor {
                asset: "mesh/post".into(),
                material_overrides: Vec::new(),
                instances: (0..20)
                    .map(|index| ScatterInstance {
                        translation: [
                            2.0 + (index % 5) as f32 * 0.9,
                            0.0,
                            -1.0 - (index / 5) as f32 * 0.9,
                        ],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: 1.0,
                        tint: [1.0; 3],
                        rank: 0.5,
                    })
                    .collect(),
                fade: ScatterFade::default(),
                shadow_casting: ShadowCasting::Cast,
                lods: Vec::new(),
                lod_band: 0.0,
                thinning: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.97, 0.9],
                intensity: 1.2,
                enabled: true,
                direction: [0.3, -0.8, -0.5],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
            parent: None,
            light: LightDescriptor::Hemisphere {
                color: [0.6, 0.7, 0.9],
                ground_color: [0.3, 0.25, 0.2],
                intensity: 0.4,
                enabled: true,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(12),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [0.6, 0.7, 0.9],
                intensity: 0.1,
                enabled: true,
                range: Some(40.0),
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
    ]);
    harness
}

fn snow(amount: f32, texels_per_metre: f32) -> RenderDiff {
    RenderDiff::SetSnow {
        snow: Some(SnowDescriptor {
            amount,
            color: SNOW_WHITE,
            texels_per_metre,
        }),
    }
}

fn render(
    harness: &mut Harness,
    view: &render_host_contracts::RendererCompositionCamera,
) -> Vec<u8> {
    harness.render(view).1
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> f64 {
    let at = ((y * WIDTH + x) * 4) as usize;
    (f64::from(rgba[at]) + f64::from(rgba[at + 1]) + f64::from(rgba[at + 2])) / 3.0
}

/// Mean brightness of a pixel rectangle.
fn mean(rgba: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for row in y {
        for column in x.clone() {
            sum += pixel(rgba, column, row);
            count += 1.0;
        }
    }
    sum / count
}

/// Mean absolute difference between two renders over a pixel rectangle.
fn change(a: &[u8], b: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for row in y {
        for column in x.clone() {
            sum += (pixel(a, column, row) - pixel(b, column, row)).abs();
            count += 1.0;
        }
    }
    sum / count
}

/// Across the open ground and the posts, toward the wall.
fn across() -> render_host_contracts::RendererCompositionCamera {
    camera([2.0, 2.0, 7.0], 0.0, -14.0)
}

/// Down onto open ground right of the roof.
fn open_ground() -> render_host_contracts::RendererCompositionCamera {
    camera([2.0, 2.5, 4.0], 0.0, -60.0)
}

/// Under the roof, up at its underside.
fn underside() -> render_host_contracts::RendererCompositionCamera {
    camera([-7.0, 1.5, 0.0], 0.0, 70.0)
}

/// Under the roof, down onto the crate and the ground around it.
fn sheltered() -> render_host_contracts::RendererCompositionCamera {
    camera([-6.0, 2.6, 0.5], 0.0, -89.0)
}

/// Before the wall, level with it.
fn wall() -> render_host_contracts::RendererCompositionCamera {
    camera([0.0, 2.5, -8.0], 0.0, 0.0)
}

/// Down onto the posts, so their tops fill the view.
fn posts() -> render_host_contracts::RendererCompositionCamera {
    camera([3.8, 3.0, -2.3], 0.0, -89.0)
}

/// Close above open ground: about five pixels per 1/32 m texel.
fn close() -> render_host_contracts::RendererCompositionCamera {
    camera([1.0, 1.0, 3.0], 0.0, -89.0)
}

const WHOLE: (std::ops::Range<u32>, std::ops::Range<u32>) = (0..WIDTH, 0..HEIGHT);

#[test]
fn zero_snow_draws_as_none() {
    let mut harness = scene();
    let bare = render(&mut harness, &across());
    harness.apply(vec![snow(0.0, 0.0)]);
    assert_eq!(render(&mut harness, &across()), bare);
    harness.apply(vec![snow(0.0, 32.0)]);
    assert_eq!(render(&mut harness, &across()), bare);
    harness.apply(vec![RenderDiff::SetSnow { snow: None }]);
    assert_eq!(render(&mut harness, &across()), bare);
}

#[test]
fn open_ground_whitens_with_the_amount() {
    let mut harness = scene();
    let mut means = Vec::new();
    for amount in [0.0, 0.5, 1.0] {
        harness.apply(vec![snow(amount, 0.0)]);
        means.push(mean(
            &render(&mut harness, &open_ground()),
            WHOLE.0,
            WHOLE.1,
        ));
    }
    assert!(
        means[0] < means[1] && means[1] < means[2] && means[2] > means[0] * 1.5,
        "the open ground whitens with the amount: {means:.1?}"
    );
}

#[test]
fn an_overhangs_underside_and_what_it_shelters_stay_clear() {
    let mut harness = scene();
    let views = [
        ("underside", underside()),
        ("sheltered crate and ground", sheltered()),
    ];
    let bare: Vec<_> = views
        .iter()
        .map(|(_, view)| render(&mut harness, view))
        .collect();
    harness.apply(vec![snow(1.0, 0.0)]);
    for ((name, view), bare) in views.iter().zip(&bare) {
        let changed = change(bare, &render(&mut harness, view), WHOLE.0, WHOLE.1);
        assert!(changed < 0.5, "the roof's {name} stays clear: {changed:.2}");
    }
    // While the open ground beside the roof is covered.
    harness.apply(vec![RenderDiff::SetSnow { snow: None }]);
    let open_bare = render(&mut harness, &open_ground());
    harness.apply(vec![snow(1.0, 0.0)]);
    let open = change(
        &open_bare,
        &render(&mut harness, &open_ground()),
        WHOLE.0,
        WHOLE.1,
    );
    assert!(open > 30.0, "the open ground is covered: {open:.1}");
}

#[test]
fn walls_stay_clear() {
    let mut harness = scene();
    let bare = render(&mut harness, &wall());
    harness.apply(vec![snow(1.0, 0.0)]);
    let full = render(&mut harness, &wall());
    // The wall's face fills the top two thirds; the ground below whitens.
    let face = change(&bare, &full, 0..WIDTH, 0..HEIGHT * 2 / 3 - 10);
    assert!(face < 0.5, "the wall's face stays clear: {face:.2}");
}

#[test]
fn scattered_copies_take_the_cover() {
    let mut harness = scene();
    let bare = render(&mut harness, &posts());
    harness.apply(vec![snow(1.0, 0.0)]);
    let full = render(&mut harness, &posts());
    // The middle post's top, straight below the camera.
    let (x, y) = (WIDTH / 2 - 6..WIDTH / 2 + 6, HEIGHT / 2 - 6..HEIGHT / 2 + 6);
    let (before, after) = (mean(&bare, x.clone(), y.clone()), mean(&full, x, y));
    assert!(
        after > before * 1.5,
        "the post's top whitens: {before:.1} to {after:.1}"
    );
}

#[test]
fn snow_lies_over_wet_ground() {
    let mut harness = scene();
    harness.apply(vec![snow(1.0, 0.0)]);
    let snowy = render(&mut harness, &open_ground());
    harness.apply(vec![RenderDiff::SetWetness {
        wetness: Some(WetnessDescriptor {
            wetness: 1.0,
            puddles: 0.0,
        }),
    }]);
    let wet = render(&mut harness, &open_ground());
    // Where the cover is whole, the wet ground beneath does not show.
    let mut covered = 0;
    let mut differ = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if pixel(&snowy, x, y) > 150.0 {
                covered += 1;
                if (pixel(&snowy, x, y) - pixel(&wet, x, y)).abs() > 1.0 {
                    differ += 1;
                }
            }
        }
    }
    assert!(
        covered > (WIDTH * HEIGHT / 2) as usize,
        "covered: {covered}"
    );
    assert!(
        differ * 50 < covered,
        "wetness darkens no covered ground: {differ} of {covered}"
    );
}

/// The share of pixels neither within `margin` of the bare ground nor of
/// full snow where half is covered: the soft edge's blend.
fn blended_share(bare: &[u8], full: &[u8], half: &[u8], margin: f64) -> f64 {
    let mut blended = 0;
    let mut count = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (low, high, value) = (pixel(bare, x, y), pixel(full, x, y), pixel(half, x, y));
            if high - low < 4.0 * margin {
                continue;
            }
            count += 1;
            if value > low + margin && value < high - margin {
                blended += 1;
            }
        }
    }
    f64::from(blended) / f64::from(count.max(1))
}

#[test]
fn a_texel_density_steps_the_covers_edge() {
    let mut harness = scene();
    let bare = render(&mut harness, &close());
    harness.apply(vec![snow(1.0, 0.0)]);
    let full = render(&mut harness, &close());
    harness.apply(vec![snow(0.5, 0.0)]);
    let smooth = render(&mut harness, &close());
    harness.apply(vec![snow(0.5, 32.0)]);
    let stepped = render(&mut harness, &close());
    let (smooth_share, stepped_share) = (
        blended_share(&bare, &full, &smooth, 6.0),
        blended_share(&bare, &full, &stepped, 6.0),
    );
    assert!(
        smooth_share > 0.05 && stepped_share < smooth_share * 0.5,
        "stepped texels blend less than a smooth edge: smooth {smooth_share:.3}, stepped {stepped_share:.3}"
    );
}
