//! Heightfield terrain for render tests (`tests/heightfield.rs`,
//! `tests/snow.rs`): rings of tiles from `svc_mesh::heightfield_tiles`,
//! projected by `HeightfieldProjector` as the services settle a call.

use std::collections::BTreeMap;

use render_model::*;
use render_projection::{HeightfieldPlacement, HeightfieldProjector};
use svc_mesh::heightfield::{HeightfieldLayout, HeightfieldRingSpec, TileSamples};
use svc_mesh::heightfield_tiles::HeightfieldTiles;

fn samples(
    ground: fn(f64, f64) -> f32,
    layout: &HeightfieldLayout,
    ring: u8,
    x: i64,
    z: i64,
) -> TileSamples {
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

/// Heightfield terrain as the services draw it, sampled from `ground`.
pub struct Terrain {
    ground: fn(f64, f64) -> f32,
    tiles: HeightfieldTiles,
    projector: HeightfieldProjector,
    materials: BTreeMap<u32, String>,
}

impl Terrain {
    /// 8-cell tiles: half-metre cells within 10 m, 2 m cells within 100 m.
    pub fn new(ground: fn(f64, f64) -> f32) -> Self {
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
            ground,
            tiles: HeightfieldTiles::new(layout),
            projector: HeightfieldProjector::default(),
            materials: BTreeMap::from([(0, "material/ground".to_owned())]),
        }
    }

    /// Focus on `focus`, sample every tile asked for, and project as the
    /// services settle a call.
    pub fn settle(&mut self, focus: [f64; 2]) -> Vec<RenderDiff> {
        self.tiles.set_focus(Some(focus));
        for request in self.tiles.requests(0) {
            let samples = samples(
                self.ground,
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
            let removals = self.projector.remove_tile(1, key);
            ops.extend(removals.early.into_iter().chain(removals.late));
        }
        for key in changes.meshed {
            let origin = self.tiles.layout().origin(key);
            match self.tiles.mesh_of(key) {
                Some(mesh) => ops.extend(
                    self.projector
                        .mesh(
                            1,
                            key,
                            origin,
                            mesh,
                            &self.materials,
                            self.tiles.is_drawn(key),
                        )
                        .unwrap(),
                ),
                None => {
                    let removals = self.projector.remove_tile(1, key);
                    ops.extend(removals.early.into_iter().chain(removals.late));
                }
            }
        }
        for key in changes.shown.into_iter().chain(changes.hidden) {
            ops.extend(self.projector.show(1, key, self.tiles.is_drawn(key)));
        }
        ops
    }
}
