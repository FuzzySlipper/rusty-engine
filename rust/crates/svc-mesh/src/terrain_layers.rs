//! Per-vertex terrain layer weights for reconstructed voxel surfaces.
//!
//! A terrain layer set names up to four material slots, in layer order. Each
//! reconstructed vertex gets one weight per layer: the share, under a tent
//! filter `transition_cells` voxels wide around the vertex, of the solid
//! voxels of that layer's slot. Weights come from absolute voxel positions
//! and the same voxels on both sides of a chunk seam, so neighbouring chunks
//! give a shared vertex the same weights, and a world-origin rebase changes
//! none. They only colour the surface: geometry, material slots and
//! collision are unchanged.

use core_space::{ChunkCoord, LocalVoxelCoord, VoxelCoord, VoxelGridSpec};
use svc_spatial::VoxelWorld;

use crate::MeshError;

/// The most layers one set blends.
pub const MAX_TERRAIN_LAYERS: usize = 4;
/// The widest transition, in voxels on each side of a vertex.
pub const MAX_TERRAIN_TRANSITION_CELLS: u8 = 4;

const NO_LAYER: u8 = u8::MAX;

/// The material slots a terrain layer material blends, in layer order, and
/// how far a transition reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainLayers {
    slots: Vec<u16>,
    transition_cells: u8,
}

impl TerrainLayers {
    /// One to [`MAX_TERRAIN_LAYERS`] distinct slots and a transition of 1 to
    /// [`MAX_TERRAIN_TRANSITION_CELLS`] voxels. A transition of 1 blends
    /// across the one voxel between two materials; wider ones blend further.
    pub fn new(slots: Vec<u16>, transition_cells: u8) -> Result<Self, MeshError> {
        let distinct = slots
            .iter()
            .enumerate()
            .all(|(index, slot)| !slots[..index].contains(slot));
        if slots.is_empty()
            || slots.len() > MAX_TERRAIN_LAYERS
            || !distinct
            || !(1..=MAX_TERRAIN_TRANSITION_CELLS).contains(&transition_cells)
        {
            return Err(MeshError::InvalidTerrainLayers);
        }
        Ok(Self {
            slots,
            transition_cells,
        })
    }

    pub fn slots(&self) -> &[u16] {
        &self.slots
    }

    pub const fn transition_cells(&self) -> u8 {
        self.transition_cells
    }

    fn layer(&self, slot: u16) -> Option<usize> {
        self.slots.iter().position(|candidate| *candidate == slot)
    }

    /// All weight on `slot`'s layer: a vertex with no layer voxel in reach,
    /// or a cube face. A slot outside the set reads layer 0.
    pub(crate) fn one_hot(&self, slot: u16) -> [f32; 4] {
        let mut weights = [0.0; 4];
        weights[self.layer(slot).unwrap_or(0)] = 1.0;
        weights
    }
}

/// The layer of every voxel around one chunk, as far as its vertices' tents
/// reach.
pub(crate) struct LayerField<'a> {
    layers: &'a TerrainLayers,
    low: [i64; 3],
    dims: [usize; 3],
    cells: Vec<u8>,
}

impl<'a> LayerField<'a> {
    /// The field for a chunk at `origin` of `size` voxels. Reconstructed
    /// vertices lie within one voxel of the chunk, so it spans the chunk, one
    /// voxel and the transition reach on each side. Absent chunks read as
    /// empty, as they do for the surface.
    pub(crate) fn around_chunk(
        world: &VoxelWorld,
        spec: &VoxelGridSpec,
        layers: &'a TerrainLayers,
        origin: [i64; 3],
        size: [i64; 3],
    ) -> Self {
        let reach = i64::from(layers.transition_cells) + 1;
        let low = origin.map(|value| value - reach);
        let dims: [usize; 3] = std::array::from_fn(|axis| (size[axis] + 2 * reach + 1) as usize);
        let mut cells = vec![NO_LAYER; dims[0] * dims[1] * dims[2]];
        let high: [i64; 3] = std::array::from_fn(|axis| low[axis] + dims[axis] as i64 - 1);
        let first = spec.voxel_to_chunk(VoxelCoord::new(low[0], low[1], low[2]));
        let last = spec.voxel_to_chunk(VoxelCoord::new(high[0], high[1], high[2]));
        let extent = spec.chunk_dims().to_array().map(i64::from);
        for cz in first.z..=last.z {
            for cy in first.y..=last.y {
                for cx in first.x..=last.x {
                    let coord = ChunkCoord::new(cx, cy, cz);
                    let Some(chunk) = world.get(coord) else {
                        continue;
                    };
                    let chunk_origin = spec.chunk_origin_voxel(coord).to_array();
                    let from: [i64; 3] =
                        std::array::from_fn(|axis| low[axis].max(chunk_origin[axis]));
                    let to: [i64; 3] = std::array::from_fn(|axis| {
                        high[axis].min(chunk_origin[axis] + extent[axis] - 1)
                    });
                    for z in from[2]..=to[2] {
                        for y in from[1]..=to[1] {
                            for x in from[0]..=to[0] {
                                let local = LocalVoxelCoord::new(
                                    (x - chunk_origin[0]) as u32,
                                    (y - chunk_origin[1]) as u32,
                                    (z - chunk_origin[2]) as u32,
                                );
                                let layer = chunk
                                    .get(local)
                                    .and_then(|value| value.material())
                                    .and_then(|material| layers.layer(material.raw()));
                                if let Some(layer) = layer {
                                    let index = ((z - low[2]) as usize * dims[1]
                                        + (y - low[1]) as usize)
                                        * dims[0]
                                        + (x - low[0]) as usize;
                                    cells[index] = layer as u8;
                                }
                            }
                        }
                    }
                }
            }
        }
        Self {
            layers,
            low,
            dims,
            cells,
        }
    }

    /// The weights at `point`, or all on `slot`'s layer with no layer voxel
    /// in reach.
    pub(crate) fn weights_or_slot(&self, point: [f64; 3], slot: u16) -> [f32; 4] {
        self.weights(point)
            .unwrap_or_else(|| self.layers.one_hot(slot))
    }

    /// The layer weights at `point` in lattice units (voxel `c`'s centre at
    /// `c + 0.5`), summing to 1, or `None` with no layer voxel in reach.
    pub(crate) fn weights(&self, point: [f64; 3]) -> Option<[f32; 4]> {
        let radius = f64::from(self.layers.transition_cells);
        let range = |axis: usize| {
            let first = ((point[axis] - 0.5 - radius).ceil() as i64).max(self.low[axis]);
            let last = ((point[axis] - 0.5 + radius).floor() as i64)
                .min(self.low[axis] + self.dims[axis] as i64 - 1);
            first..=last
        };
        let tent =
            |axis: usize, sample: i64| 1.0 - (point[axis] - 0.5 - sample as f64).abs() / radius;
        let mut sums = [0.0_f64; 4];
        for z in range(2) {
            let wz = tent(2, z);
            if wz <= 0.0 {
                continue;
            }
            for y in range(1) {
                let wy = wz * tent(1, y);
                if wy <= 0.0 {
                    continue;
                }
                let row = ((z - self.low[2]) as usize * self.dims[1] + (y - self.low[1]) as usize)
                    * self.dims[0];
                for x in range(0) {
                    let layer = self.cells[row + (x - self.low[0]) as usize];
                    let weight = wy * tent(0, x);
                    if layer != NO_LAYER && weight > 0.0 {
                        sums[layer as usize] += weight;
                    }
                }
            }
        }
        let total: f64 = sums.iter().sum();
        (total > 0.0).then(|| sums.map(|sum| (sum / total) as f32))
    }
}
