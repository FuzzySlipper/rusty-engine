//! Heightfield terrain (#9842) as the services draw it: rings of tiles from
//! `svc_mesh::heightfield_tiles`, projected by `HeightfieldProjector`, each
//! finer ring geomorphing onto the next. The focus walks across a ring's
//! region boundary, where tiles join and leave the finer ring: the picture
//! changes no more at that step than at an ordinary one, and no tile seam
//! or ring join ever shows the background through the ground. Six terrain
//! layers draw across tiles of at most four each by their palettes. Same
//! harness as `tests/screenshots.rs`.

mod support;
mod terrain;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;
use terrain::{Layers, Terrain};

const BACKGROUND: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

/// Rough ground: the fine ring's half-metre cells show bumps the coarse
/// ring's 2 m cells cannot, so a swap between them would show.
fn ground(x: f64, z: f64) -> f32 {
    (0.6 * (x * 1.9).sin() * (z * 1.3).cos() + 0.4 * (x / 7.0).sin() + 0.3 * (z / 5.0).cos()) as f32
}

fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::SetBackgroundColor { color: BACKGROUND },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.8, 0.8, 0.8, 1.0], None),
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(900),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 3.0,
                enabled: true,
                direction: [-0.6, -0.5, 0.3],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(901),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]);
    harness
}

/// Looking down on the ground about (16, 0) from 30 m up, so the frame
/// holds the fine ring's morph band, its edge and the coarse ring beyond.
fn view(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([16.0, 30.0, 18.0], 0.0, -60.0)).1
}

fn background_pixels(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 200 && p[1] < 40 && p[2] > 200)
        .count()
}

fn mean_difference(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn a_focus_crossing_a_ring_boundary_changes_the_picture_no_more_than_an_ordinary_step() {
    let mut harness = scene();
    let mut terrain = Terrain::new(ground);
    // The fine ring's region tiles are 16 m: crossing x = 16 shifts its
    // region a column, which joins at the morph's end, fully coarse.
    let mut frames = Vec::new();
    for x in [15.5, 15.75, 15.95, 16.15, 16.35] {
        let ops = terrain.settle([x, 0.0]);
        harness.apply(ops);
        frames.push(view(&mut harness));
    }
    for (step, frame) in frames.iter().enumerate() {
        assert_eq!(background_pixels(frame), 0, "a seam shows at step {step}");
    }
    let differences: Vec<f64> = frames
        .windows(2)
        .map(|pair| mean_difference(&pair[0], &pair[1]))
        .collect();
    let (before, crossing, after) = (differences[1], differences[2], differences[3]);
    assert!(
        crossing <= 2.0 * before.max(after) + 0.05,
        "the crossing step pops: {differences:?}"
    );
    assert!(
        differences.iter().all(|difference| *difference < 1.0),
        "{differences:?}"
    );
}

const SIX: [[u8; 4]; 6] = [
    [220, 40, 40, 255],
    [40, 220, 40, 255],
    [40, 40, 220, 255],
    [220, 220, 40, 255],
    [40, 220, 220, 255],
    [220, 40, 220, 255],
];

/// How wide a band of ground one layer has, and the blend into the next.
const BAND: f64 = 30.0;
const BLEND: f64 = 6.0;

/// Six bands of layers along x, from x = -90, blending at their edges.
fn six_bands(x: f64, _z: f64) -> [f32; 6] {
    let along = ((x + 90.0) / BAND).clamp(0.0, 5.999);
    let band = along.floor();
    let past = (along - band - 0.5) * BAND;
    let mut weights = [0.0; 6];
    let share = ((past.abs() - (BAND * 0.5 - BLEND * 0.5)) / BLEND).clamp(0.0, 0.5) as f32;
    weights[band as usize] = 1.0 - share;
    let other = band as i32 + past.signum() as i32;
    if share > 0.0 && (0..6).contains(&other) {
        weights[other as usize] = share;
    } else {
        weights[band as usize] = 1.0;
    }
    weights
}

/// Layers 0 to 3 alone, the same whether named or not.
fn four_bands(x: f64, z: f64) -> [f32; 6] {
    six_bands(x.min(-5.0), z)
}

/// Flat ground of a six-layer material, in a world of ambient π where a
/// surface shows exactly its texture.
fn layered_scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![
        RenderDiff::SetBackgroundColor { color: BACKGROUND },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(901),
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
    ];
    let mut surfaces = Vec::new();
    for (index, color) in SIX.iter().enumerate() {
        let rgba: Vec<u8> = color.repeat(16);
        let texture = harness.resources.texture(
            &format!("texture/layer-{index}"),
            4,
            4,
            &rgba,
            TextureWrap::Repeat,
        );
        surfaces.push(VoxelSurfaceDescriptor {
            schema_version: 1,
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Repeat,
            alpha_mode: VoxelSurfaceAlphaModeDescriptor::Opaque,
            mapping: VoxelSurfaceMappingDescriptor::Repeat {
                texture: texture.id.clone(),
                texture_version: texture.version,
                texture_content_hash: texture.content_hash.clone().unwrap(),
                tile_scale_cells: [1.0, 1.0],
                tile_origin_cells: [0.0, 0.0],
            },
        });
        ops.push(RenderDiff::DefineTexture { texture });
    }
    let mut ground = material("material/ground", [1.0; 4], Some("texture/layer-0"));
    ground.roughness = 1.0;
    ground.voxel_surface = Some(surfaces.remove(0));
    ground.terrain_layers = Some(MaterialTerrainLayersDescriptor {
        layers: surfaces
            .into_iter()
            .map(|voxel_surface| MaterialTerrainLayerDescriptor {
                voxel_surface,
                normal_map: None,
            })
            .collect(),
        contrast: 1.0,
    });
    ops.push(RenderDiff::DefineMaterial { material: ground });
    harness.apply(ops);
    harness
}

fn flat(_x: f64, _z: f64) -> f32 {
    0.0
}

/// Straight down on the ground about the origin, x across the frame.
fn from_above(harness: &mut Harness) -> Vec<u8> {
    let mut above = camera([0.0, 170.0, 0.0], 0.0, -89.9);
    above.projection = render_host_contracts::RendererCameraProjection::Perspective {
        fov_y_degrees: 60.0,
        near: 1.0,
        far: 400.0,
    };
    harness.render(&above).1
}

fn at_x(frame: &[u8], metres: f64) -> [u8; 3] {
    let span = 170.0 * 30f64.to_radians().tan() * 2.0 / f64::from(HEIGHT);
    let column = (f64::from(WIDTH) * 0.5 + metres / span) as u32;
    let at = ((HEIGHT / 2 * WIDTH + column) * 4) as usize;
    [frame[at], frame[at + 1], frame[at + 2]]
}

#[test]
fn six_layers_draw_across_tiles_of_four_with_no_seam_where_palettes_meet() {
    let mut harness = layered_scene();
    let mut terrain = Terrain::layered(
        flat,
        Some(Layers {
            weights: six_bands,
            palette: true,
        }),
    );
    let ops = terrain.settle([0.0, 0.0]);
    harness.apply(ops);
    let frame = from_above(&mut harness);
    for (layer, centre) in [-75.0, -45.0, -15.0, 15.0, 45.0, 75.0]
        .into_iter()
        .enumerate()
    {
        let found = at_x(&frame, centre);
        for channel in 0..3 {
            assert!(
                found[channel].abs_diff(SIX[layer][channel]) <= 14,
                "layer {layer} at {centre}: {found:?}"
            );
        }
    }
    // Across every tile edge the ground only ever blends: a palette that
    // dropped a layer there would step from one colour to another.
    let steepest = (-85..85)
        .map(|metres| {
            let (a, b) = (
                at_x(&frame, f64::from(metres)),
                at_x(&frame, f64::from(metres + 1)),
            );
            (0..3)
                .map(|channel| a[channel].abs_diff(b[channel]))
                .max()
                .unwrap()
        })
        .max()
        .unwrap();
    assert!(steepest < 70, "a seam steps by {steepest}");
}

#[test]
fn palettes_hold_through_a_ring_morph() {
    let mut harness = layered_scene();
    let mut terrain = Terrain::layered(
        flat,
        Some(Layers {
            weights: six_bands,
            palette: true,
        }),
    );
    // A band's edge at x = 30, and the fine ring's region crossing x = 32.
    let mut frames = Vec::new();
    for x in [31.5, 31.75, 31.95, 32.15, 32.35] {
        let ops = terrain.settle([x, 0.0]);
        harness.apply(ops);
        frames.push(from_above(&mut harness));
    }
    // Inside the coarse ring, whose outer edge moves a tile against the
    // background as the focus crosses. Tiles changing rings differ a little
    // where a blend bends between the coarse ring's vertices; a layer lost
    // between palettes would step by nearly a whole colour.
    let largest = frames
        .windows(2)
        .flat_map(|pair| {
            (-70..85).map(|metres| {
                let (a, b) = (
                    at_x(&pair[0], f64::from(metres)),
                    at_x(&pair[1], f64::from(metres)),
                );
                (0..3)
                    .map(|channel| a[channel].abs_diff(b[channel]))
                    .max()
                    .unwrap()
            })
        })
        .max();
    assert!(largest < Some(40), "the morph shows: {largest:?}");
}

#[test]
fn a_palette_of_the_first_layers_draws_as_none() {
    let frame = |palette| {
        let mut harness = layered_scene();
        let mut terrain = Terrain::layered(
            flat,
            Some(Layers {
                weights: four_bands,
                palette,
            }),
        );
        let ops = terrain.settle([0.0, 0.0]);
        harness.apply(ops);
        from_above(&mut harness)
    };
    let (named, plain) = (frame(true), frame(false));
    assert!(mean_difference(&named, &plain) < 0.01);
}
