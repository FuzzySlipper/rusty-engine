//! Heightfield terrain for render tests (`tests/heightfield.rs`,
//! `tests/snow.rs`): rings of tiles from `svc_mesh::heightfield_tiles`,
//! projected by `HeightfieldProjector` as the services settle a call.

use std::collections::BTreeMap;

use render_model::*;
use render_projection::{HeightfieldPlacement, HeightfieldProjector};
use svc_mesh::heightfield::{HeightfieldLayout, HeightfieldRingSpec, TileSamples};
use svc_mesh::heightfield_tiles::HeightfieldTiles;

/// How a tile weighs a terrain layer material: six layers' weights at a
/// point, and whether the tile names the layers it touches in a palette or
/// leaves its weights as layers 0 to 3.
#[derive(Clone, Copy)]
pub struct Layers {
    pub weights: fn(f64, f64) -> [f32; 6],
    pub palette: bool,
}

fn samples(
    ground: fn(f64, f64) -> f32,
    layers: Option<Layers>,
    layout: &HeightfieldLayout,
    ring: u8,
    x: i64,
    z: i64,
) -> TileSamples {
    let key = svc_mesh::heightfield::TileKey { ring, x, z };
    let cell = layout.rings[ring as usize].cell_size;
    let origin = layout.origin(key);
    let side = layout.height_side() as i64;
    let (layer_weights, layer_palette) = layers.map_or((None, Vec::new()), |layers| {
        let vertices = layout.side() as i64;
        let at: Vec<[f32; 6]> = (0..vertices * vertices)
            .map(|index| {
                let (i, j) = (index % vertices, index / vertices);
                (layers.weights)(origin[0] + i as f64 * cell, origin[1] + j as f64 * cell)
            })
            .collect();
        let palette: Vec<u8> = if layers.palette {
            (0..6)
                .filter(|layer| at.iter().any(|weights| weights[usize::from(*layer)] > 0.0))
                .collect()
        } else {
            vec![0, 1, 2, 3]
        };
        assert!(palette.len() <= 4, "a tile touches {palette:?}");
        let weights = at
            .iter()
            .map(|weights| {
                let mut kept = [0.0; 4];
                for (slot, layer) in kept.iter_mut().zip(&palette) {
                    *slot = weights[usize::from(*layer)];
                }
                kept
            })
            .collect();
        (
            Some(weights),
            if layers.palette { palette } else { Vec::new() },
        )
    });
    TileSamples {
        layer_weights,
        layer_palette,
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
    layers: Option<Layers>,
    tiles: HeightfieldTiles,
    projector: HeightfieldProjector,
    materials: BTreeMap<u32, String>,
}

impl Terrain {
    /// 8-cell tiles: half-metre cells within 10 m, 2 m cells within 100 m.
    pub fn new(ground: fn(f64, f64) -> f32) -> Self {
        Self::layered(ground, None)
    }

    /// The same, weighing `layers` of a terrain layer material.
    pub fn layered(ground: fn(f64, f64) -> f32, layers: Option<Layers>) -> Self {
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
            layers,
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
                self.layers,
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
                    visible: true,
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
