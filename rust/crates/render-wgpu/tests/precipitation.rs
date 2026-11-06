//! Precipitation around the camera (`precipitation.wgsl`): drops 0 draws
//! nothing, more drops cover more of the view, the drops fall with time and
//! hold with it, the pass is timed, and no rain falls under a roof (the sky
//! occlusion layer). With a pixel size they are whole blocks on a screen grid.
//! Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// A dark ground lit by a hemisphere fill, so pale drops stand out, with
/// the sky's occlusion layer (an ambient light whose shadow is the open sky).
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.05, 0.05, 0.07, 1.0],
        },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.2, 0.2, 0.2, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-80.0, -1.0, -80.0], [80.0, 0.0, 80.0], |_| 0),
            "material/ground",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Hemisphere {
                color: [0.6, 0.6, 0.7],
                ground_color: [0.3, 0.3, 0.3],
                intensity: 0.5,
                enabled: true,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
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

fn rain(drops: u32) -> RenderDiff {
    RenderDiff::SetPrecipitation {
        precipitation: Some(PrecipitationDescriptor {
            drops,
            shape: PrecipitationShape::Streak,
            velocity: [1.0, -14.0, 0.5],
            size: 0.03,
            streak_seconds: 0.05,
            color: [2.0, 2.0, 2.2, 0.8],
            additive: false,
            radius: 15.0,
            height: 10.0,
            pixel_size: 0.0,
        }),
    }
}

/// From head height, looking out across the ground, at time `time`.
fn look(harness: &mut Harness, time: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(time);
    harness.render(&camera([0.0, 1.7, 0.0], 0.0, -10.0)).1
}

/// How many pixels differ from `bare`.
fn changed(bare: &[u8], with: &[u8]) -> usize {
    bare.as_chunks::<4>()
        .0
        .iter()
        .zip(with.as_chunks::<4>().0)
        .filter(|(a, b)| {
            (0..3)
                .map(|channel| (i32::from(a[channel]) - i32::from(b[channel])).abs())
                .sum::<i32>()
                > 6
        })
        .count()
}

#[test]
fn no_drops_draw_as_without_precipitation() {
    let mut harness = scene();
    let bare = look(&mut harness, 3.0);
    harness.apply(vec![rain(0)]);
    assert_eq!(look(&mut harness, 3.0), bare);
    harness.apply(vec![RenderDiff::SetPrecipitation {
        precipitation: None,
    }]);
    assert_eq!(look(&mut harness, 3.0), bare);
}

#[test]
fn more_drops_fall_thicker_and_fall_with_time() {
    let mut harness = scene();
    let bare = look(&mut harness, 3.0);
    harness.apply(vec![rain(4_000)]);
    let light = changed(&bare, &look(&mut harness, 3.0));
    harness.apply(vec![rain(40_000)]);
    let heavy_frame = look(&mut harness, 3.0);
    let heavy = changed(&bare, &heavy_frame);
    let total = (WIDTH * HEIGHT) as usize;
    assert!(light > total / 400, "light rain shows: {light} of {total}");
    assert!(
        heavy > light * 3,
        "heavy rain covers more: {light} then {heavy}"
    );
    assert_eq!(
        look(&mut harness, 3.0),
        heavy_frame,
        "a held time draws the same"
    );
    assert_ne!(
        look(&mut harness, 3.4),
        heavy_frame,
        "the drops fall with time"
    );
    let passes = harness.renderer.gpu_readout().passes;
    assert!(passes.iter().any(|timing| timing.pass == "precipitation"));
}

#[test]
fn no_rain_falls_under_a_roof() {
    let open = {
        let mut harness = scene();
        let bare = look(&mut harness, 3.0);
        harness.apply(vec![rain(40_000)]);
        changed(&bare, &look(&mut harness, 3.0))
    };
    let roofed = {
        let mut harness = scene();
        // A wide low roof over the camera and the ground it looks at.
        harness.apply(vec![
            static_mesh(
                "mesh/roof",
                box_mesh([-40.0, 3.0, -40.0], [40.0, 3.5, 10.0], |_| 0),
                "material/ground",
            ),
            instance(21, None, "mesh/roof", transform([0.0; 3], 0.0, [1.0; 3])),
        ]);
        let bare = look(&mut harness, 3.0);
        harness.apply(vec![rain(40_000)]);
        changed(&bare, &look(&mut harness, 3.0))
    };
    assert!(
        roofed * 10 < open,
        "under a roof almost no rain shows: open {open}, roofed {roofed}"
    );
}

/// The side of a block in the block tests, in pixels.
const BLOCK: usize = 4;

/// Pale opaque drops over a flat clear colour, with nothing else in the view.
fn block_scene(options: RendererOptions) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..options
    });
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }]);
    harness
}

fn blocks(
    drops: u32,
    shape: PrecipitationShape,
    velocity: [f32; 3],
    pixel_size: f32,
) -> RenderDiff {
    RenderDiff::SetPrecipitation {
        precipitation: Some(PrecipitationDescriptor {
            drops,
            shape,
            velocity,
            size: 0.06,
            streak_seconds: 0.08,
            color: [1.0, 1.0, 1.0, 1.0],
            additive: false,
            radius: 12.0,
            height: 8.0,
            pixel_size,
        }),
    }
}

fn pixel(frame: &[u8], x: usize, y: usize) -> [u8; 4] {
    let at = (y * WIDTH as usize + x) * 4;
    [frame[at], frame[at + 1], frame[at + 2], frame[at + 3]]
}

/// The grid cells whose pixels are not all one colour, and those with any
/// drop in them.
fn broken_and_lit_cells(frame: &[u8]) -> (usize, usize) {
    let (mut broken, mut lit) = (0, 0);
    for cell_y in 0..HEIGHT as usize / BLOCK {
        for cell_x in 0..WIDTH as usize / BLOCK {
            let first = pixel(frame, cell_x * BLOCK, cell_y * BLOCK);
            let whole = (0..BLOCK * BLOCK).all(|at| {
                pixel(
                    frame,
                    cell_x * BLOCK + at % BLOCK,
                    cell_y * BLOCK + at / BLOCK,
                ) == first
            });
            broken += usize::from(!whole);
            lit += usize::from(!whole || first[..3] != [0, 0, 0]);
        }
    }
    (broken, lit)
}

/// The lit grid cells, as a set of (x, y).
fn lit_cells(frame: &[u8]) -> std::collections::BTreeSet<(i32, i32)> {
    let mut cells = std::collections::BTreeSet::new();
    for y in 0..HEIGHT as usize / BLOCK {
        for x in 0..WIDTH as usize / BLOCK {
            if pixel(frame, x * BLOCK, y * BLOCK)[..3] != [0, 0, 0] {
                cells.insert((x as i32, y as i32));
            }
        }
    }
    cells
}

/// The groups of lit cells touching at a side or corner.
fn clumps(mut cells: std::collections::BTreeSet<(i32, i32)>) -> Vec<Vec<(i32, i32)>> {
    let mut clumps = Vec::new();
    while let Some(first) = cells.pop_first() {
        let (mut clump, mut open) = (vec![first], vec![first]);
        while let Some((x, y)) = open.pop() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if cells.remove(&(x + dx, y + dy)) {
                        clump.push((x + dx, y + dy));
                        open.push((x + dx, y + dy));
                    }
                }
            }
        }
        clumps.push(clump);
    }
    clumps
}

fn spans(clump: &[(i32, i32)]) -> (i32, i32) {
    let span = |value: fn(&(i32, i32)) -> i32| {
        let values = clump.iter().map(value);
        values.clone().max().unwrap() - values.min().unwrap() + 1
    };
    (span(|cell| cell.0), span(|cell| cell.1))
}

/// The clumps that are one streak: a block per step of their longer side,
/// running one way. Streaks that touch make a clump that is neither.
fn lines_of(clumps: &[Vec<(i32, i32)>]) -> Vec<&Vec<(i32, i32)>> {
    clumps
        .iter()
        .filter(|clump| {
            let (across, down) = spans(clump);
            let mut cells = (*clump).clone();
            cells.sort_by_key(|cell| cell.1);
            let turns = |sign: i32| {
                cells
                    .windows(2)
                    .any(|pair| (pair[1].0 - pair[0].0) * sign > 0)
            };
            clump.len() as i32 == across.max(down) && !(turns(1) && turns(-1))
        })
        .collect()
}

fn level(harness: &mut Harness, time: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(time);
    harness.render(&camera([0.0, 1.7, 0.0], 0.0, 0.0)).1
}

#[test]
fn pixel_drops_are_whole_blocks_where_smooth_ones_are_not() {
    for shape in [PrecipitationShape::Streak, PrecipitationShape::Flake] {
        let velocity = [3.0, -9.0, 0.5];
        let mut harness = block_scene(RendererOptions {
            samples: 1,
            ..RendererOptions::default()
        });
        harness.apply(vec![blocks(6_000, shape, velocity, BLOCK as f32)]);
        let (broken, lit) = broken_and_lit_cells(&level(&mut harness, 3.0));
        assert!(lit > 100, "{shape:?} draws: {lit} cells");
        assert_eq!(
            broken, 0,
            "{shape:?}: {broken} of {lit} cells are not whole"
        );

        harness.apply(vec![blocks(6_000, shape, velocity, 0.0)]);
        let (broken, _) = broken_and_lit_cells(&level(&mut harness, 3.0));
        assert!(
            broken > 100,
            "{shape:?} smooth: {broken} cells are not whole"
        );
    }
}

#[test]
fn a_pixel_flake_stays_a_few_blocks_near_the_eye() {
    let mut harness = block_scene(RendererOptions::default());
    harness.apply(vec![RenderDiff::SetPrecipitation {
        precipitation: Some(PrecipitationDescriptor {
            drops: 40,
            shape: PrecipitationShape::Flake,
            velocity: [0.0, -1.0, 0.0],
            // Tens of pixels across at a metre, as a smooth flake.
            size: 0.3,
            streak_seconds: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            additive: false,
            radius: 4.0,
            height: 3.0,
            pixel_size: BLOCK as f32,
        }),
    }]);
    let clumps = clumps(lit_cells(&level(&mut harness, 3.0)));
    assert!(clumps.len() > 3, "flakes show: {}", clumps.len());
    let widest = clumps.iter().map(|clump| spans(clump).0).min();
    assert!(widest <= Some(3), "the smallest clump: {widest:?}");
    assert!(
        clumps.iter().filter(|clump| spans(clump).0 <= 3).count() * 10 >= clumps.len() * 8,
        "flakes stay within three blocks, unless they touch"
    );
}

#[test]
fn a_pixel_streak_is_a_stepped_line_one_block_wide() {
    let mut harness = block_scene(RendererOptions::default());
    harness.apply(vec![blocks(
        120,
        PrecipitationShape::Streak,
        [5.0, -10.0, 0.0],
        BLOCK as f32,
    )]);
    let clumps = clumps(lit_cells(&level(&mut harness, 3.0)));
    let lines = lines_of(&clumps);
    assert!(clumps.len() > 10, "streaks show: {}", clumps.len());
    assert!(
        lines.len() * 4 >= clumps.len() * 3,
        "{} of {} are one block wide",
        lines.len(),
        clumps.len()
    );
    assert!(
        lines
            .iter()
            .any(|clump| spans(clump).1 >= 3 && spans(clump).0 >= 2),
        "a streak runs at its slant"
    );
}

#[test]
fn a_pixel_streak_is_at_most_ten_blocks_however_fast_it_falls() {
    let mut harness = block_scene(RendererOptions::default());
    harness.apply(vec![blocks(
        60,
        PrecipitationShape::Streak,
        [10.0, -60.0, 0.0],
        BLOCK as f32,
    )]);
    let clumps = clumps(lit_cells(&level(&mut harness, 3.0)));
    // Two streaks can overlap along one line and read as a longer one, so
    // the cap shows in how many stop exactly at it.
    let lines = lines_of(&clumps);
    let length = |blocks: usize| lines.iter().filter(|line| line.len() == blocks).count();
    let longer = lines.iter().filter(|line| line.len() > 10).count();
    assert!(
        length(10) >= 2 && length(10) > longer,
        "streaks stop at ten blocks: {} do, {longer} run longer",
        length(10)
    );
}

#[test]
fn pixel_drops_move_in_whole_blocks() {
    let mut harness = block_scene(RendererOptions::default());
    harness.apply(vec![blocks(
        2_000,
        PrecipitationShape::Flake,
        [0.0, -0.5, 0.0],
        BLOCK as f32,
    )]);
    let first = level(&mut harness, 3.0);
    // A drop fades in and out by the same fades, in whole blocks.
    let next = level(&mut harness, 3.02);
    let (broken, _) = broken_and_lit_cells(&next);
    assert_eq!(broken, 0);
    assert_ne!(first, next, "the drops fall");
    let moved = lit_cells(&first)
        .symmetric_difference(&lit_cells(&next))
        .count();
    assert!(
        moved * 3 < lit_cells(&first).len(),
        "most hold their block between close frames: {moved} moved"
    );
}

#[test]
fn pixel_drops_are_whole_blocks_when_multisampled() {
    let mut harness = block_scene(RendererOptions {
        samples: 4,
        ..RendererOptions::default()
    });
    harness.apply(vec![blocks(
        6_000,
        PrecipitationShape::Streak,
        [3.0, -9.0, 0.5],
        BLOCK as f32,
    )]);
    let (broken, lit) = broken_and_lit_cells(&level(&mut harness, 3.0));
    assert!(lit > 100, "the drops draw: {lit} cells");
    assert_eq!(broken, 0, "{broken} of {lit} cells are not whole");
}
