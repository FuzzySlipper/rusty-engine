//! Heightfield terrain (#9842) as the services draw it: rings of tiles from
//! `svc_mesh::heightfield_tiles`, projected by `HeightfieldProjector`, each
//! finer ring geomorphing onto the next. The focus walks across a ring's
//! region boundary, where tiles join and leave the finer ring: the picture
//! changes no more at that step than at an ordinary one, and no tile seam
//! or ring join ever shows the background through the ground. Same harness
//! as `tests/screenshots.rs`.

mod support;

use std::collections::BTreeMap;

use render_model::*;
use render_projection::{HeightfieldPlacement, HeightfieldProjector};
use render_wgpu::RendererOptions;
use support::*;
use svc_mesh::heightfield::{HeightfieldLayout, HeightfieldRingSpec, TileSamples};
use svc_mesh::heightfield_tiles::HeightfieldTiles;

const BACKGROUND: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

/// Rough ground: the fine ring's half-metre cells show bumps the coarse
/// ring's 2 m cells cannot, so a swap between them would show.
fn ground(x: f64, z: f64) -> f32 {
    (0.6 * (x * 1.9).sin() * (z * 1.3).cos() + 0.4 * (x / 7.0).sin() + 0.3 * (z / 5.0).cos()) as f32
}

fn samples(layout: &HeightfieldLayout, ring: u8, x: i64, z: i64) -> TileSamples {
    let key = svc_mesh::heightfield::TileKey { ring, x, z };
    let cell = layout.rings[ring as usize].cell_size;
    let origin = layout.origin(key);
    let side = layout.height_side() as i64;
    TileSamples {
        heights: (0..side * side)
            .map(|index| {
                let (i, j) = (index % side - 1, index / side - 1);
                ground(origin[0] + i as f64 * cell, origin[1] + j as f64 * cell)
            })
            .collect(),
        ..TileSamples::default()
    }
}

struct Terrain {
    tiles: HeightfieldTiles,
    projector: HeightfieldProjector,
    materials: BTreeMap<u32, String>,
}

impl Terrain {
    /// 8-cell tiles: half-metre cells within 10 m, 2 m cells within 100 m.
    fn new() -> Self {
        let layout = HeightfieldLayout::new(
            8,
            &[
                HeightfieldRingSpec {
                    cell_size: 0.5,
                    reach: 10.0,
                },
                HeightfieldRingSpec {
                    cell_size: 2.0,
                    reach: 100.0,
                },
            ],
        )
        .unwrap();
        Self {
            tiles: HeightfieldTiles::new(layout),
            projector: HeightfieldProjector::default(),
            materials: BTreeMap::from([(0, "material/ground".to_owned())]),
        }
    }

    /// Focus on `focus`, sample every tile asked for, and project as the
    /// services settle a call.
    fn settle(&mut self, focus: [f64; 2]) -> Vec<RenderDiff> {
        self.tiles.set_focus(Some(focus));
        for request in self.tiles.requests(0) {
            let samples = samples(
                self.tiles.layout(),
                request.key.ring,
                request.key.x,
                request.key.z,
            );
            assert!(self.tiles.commit(request.key, request.revision, samples));
        }
        self.tiles.mesh();
        let changes = self.tiles.take_changes();
        let mut ops = self
            .projector
            .place(
                1,
                HeightfieldPlacement {
                    origin: [0.0; 3],
                    scale: 1.0,
                    layer: RenderLayer::Scene,
                    shadow_casting: ShadowCasting::Cast,
                },
            )
            .unwrap();
        ops.extend(self.projector.focus(1, self.tiles.focus()));
        for key in changes.evicted {
            ops.extend(self.projector.remove_tile(1, key));
        }
        for key in changes.meshed {
            let origin = self.tiles.layout().origin(key);
            ops.extend(
                self.projector
                    .mesh(
                        1,
                        key,
                        origin,
                        self.tiles.mesh_of(key).map(AsRef::as_ref),
                        &self.materials,
                        self.tiles.is_drawn(key),
                    )
                    .unwrap(),
            );
        }
        for key in changes.shown {
            ops.extend(self.projector.show(1, key, true));
        }
        for key in changes.hidden {
            ops.extend(self.projector.show(1, key, false));
        }
        ops
    }
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
    let mut terrain = Terrain::new();
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
