//! Heightfield terrain (#9842): square tiles of a product-sampled height
//! field in clipmap rings about a focus, meshed with the coarser ring's
//! surface as each vertex's morph target, so the renderer can ease every
//! ring onto the next before its edge.
//!
//! Ring 0 is the finest. Each ring's tile is a whole number of the finer
//! ring's tiles, and a ring covers a square of its next coarser ring's tiles
//! about the focus (the coarsest a square of its own), so a finer ring's
//! edge always lies on the coarser ring's grid lines. Every vertex of a ring
//! with a coarser one reaches its target (the coarser surface at its place)
//! by the morph's end, which is no farther from the focus than the ring's
//! region reaches whatever tile the focus stands in: tiles that enter or
//! leave as the focus moves are drawn wholly as the coarser surface, and
//! the ring's edge lies on it exactly. The coarser ring is itself unmorphed
//! wherever the finer ring reaches. Coordinates here are the heightfield's
//! own units; vertices are tile-local.

use std::collections::BTreeMap;

/// The most rings a heightfield has.
pub const MAX_HEIGHTFIELD_RINGS: usize = 8;

/// The most cells along a tile's side.
pub const MAX_HEIGHTFIELD_TILE_CELLS: u32 = 256;

/// One ring as a product asks for it: its cell's width and how far it
/// reaches from the focus, in heightfield units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldRingSpec {
    pub cell_size: f64,
    pub reach: f64,
}

/// A ring as laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightfieldRing {
    pub cell_size: f64,
    pub tile_size: f64,
    /// This ring's tiles along a side of its coarser ring's tile (1 for the
    /// coarsest).
    pub per_parent: i64,
    /// The region's half width, in region tiles: the coarser ring's tiles,
    /// or the coarsest ring's own.
    pub reach_tiles: i64,
    /// The morph's range: whole within the start, the coarser surface from
    /// the end, by the larger horizontal distance from the focus. None for
    /// the coarsest ring.
    pub morph: Option<(f64, f64)>,
    /// How far a tile's skirt hangs below its edges.
    pub skirt: f64,
}

impl HeightfieldRing {
    /// The width of the tiles the region is counted in.
    pub fn region_tile(&self) -> f64 {
        self.tile_size * self.per_parent as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeightfieldLayoutError {
    /// No rings, or more than [`MAX_HEIGHTFIELD_RINGS`].
    RingCount,
    /// Tile cells outside 1 to [`MAX_HEIGHTFIELD_TILE_CELLS`].
    TileCells,
    /// A ring's cell size or reach is not finite and positive.
    Ring(usize),
    /// A ring's tile is not a whole number (above one) of the finer ring's.
    NotNested(usize),
    /// A ring does not reach far enough past the finer ring's region to
    /// morph onto it before its edge.
    ReachTooShort(usize),
}

/// Rings of square tiles about a focus.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightfieldLayout {
    pub tile_cells: u32,
    pub rings: Vec<HeightfieldRing>,
}

/// A tile by ring and index: it spans `[x, x + 1) × [z, z + 1)` tiles of
/// its ring from the heightfield's origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileKey {
    pub ring: u8,
    pub x: i64,
    pub z: i64,
}

/// An inclusive rectangle of tile indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRect {
    pub min: [i64; 2],
    pub max: [i64; 2],
}

impl TileRect {
    pub fn contains(&self, x: i64, z: i64) -> bool {
        (self.min[0]..=self.max[0]).contains(&x) && (self.min[1]..=self.max[1]).contains(&z)
    }

    /// The rectangle grown by `by` tiles on every side.
    pub fn grown(&self, by: i64) -> Self {
        Self {
            min: [self.min[0] - by, self.min[1] - by],
            max: [self.max[0] + by, self.max[1] + by],
        }
    }

    pub fn tiles(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        (self.min[1]..=self.max[1])
            .flat_map(move |z| (self.min[0]..=self.max[0]).map(move |x| (x, z)))
    }
}

impl HeightfieldLayout {
    pub fn new(
        tile_cells: u32,
        specs: &[HeightfieldRingSpec],
    ) -> Result<Self, HeightfieldLayoutError> {
        if specs.is_empty() || specs.len() > MAX_HEIGHTFIELD_RINGS {
            return Err(HeightfieldLayoutError::RingCount);
        }
        if tile_cells == 0 || tile_cells > MAX_HEIGHTFIELD_TILE_CELLS {
            return Err(HeightfieldLayoutError::TileCells);
        }
        let cells = f64::from(tile_cells);
        for (index, spec) in specs.iter().enumerate() {
            let valid = |value: f64| value.is_finite() && value > 0.0;
            if !valid(spec.cell_size) || !valid(spec.reach) {
                return Err(HeightfieldLayoutError::Ring(index));
            }
        }
        let tile = |index: usize| specs[index].cell_size * cells;
        let mut per_parent = vec![1_i64; specs.len()];
        for (index, per) in per_parent.iter_mut().enumerate().take(specs.len() - 1) {
            let ratio = tile(index + 1) / tile(index);
            let whole = ratio.round();
            if whole < 2.0 || (ratio - whole).abs() > 1e-9 * whole {
                return Err(HeightfieldLayoutError::NotNested(index + 1));
            }
            *per = whole as i64;
        }
        let coarsest = specs.len() - 1;
        let reach_tiles: Vec<i64> = (0..specs.len())
            .map(|index| {
                let region = tile(index) * per_parent[index] as f64;
                ((specs[index].reach / region).ceil() as i64).max(1)
            })
            .collect();
        let mut rings = Vec::with_capacity(specs.len());
        for index in 0..specs.len() {
            // How far from the focus the finer ring's region can reach, in
            // the worst place within its centre tile: this ring is whole
            // there, so the finer ring's edge meets its unmorphed surface.
            let inner = match index {
                0 => 0.0,
                _ => (reach_tiles[index - 1] + 1) as f64 * tile(index),
            };
            let region = tile(index) * per_parent[index] as f64;
            let reach = reach_tiles[index] as f64 * region;
            let morph = if index == coarsest {
                if index > 0 && reach < inner {
                    return Err(HeightfieldLayoutError::ReachTooShort(index));
                }
                None
            } else {
                let band = if index == 0 {
                    region.min(reach / 2.0)
                } else {
                    region.min(reach - inner)
                };
                if band <= 0.0 {
                    return Err(HeightfieldLayoutError::ReachTooShort(index));
                }
                Some((reach - band, reach))
            };
            // A skirt reaches as deep as the coarser ring's cell: about
            // the most the two surfaces part by while one waits for the
            // other.
            let skirt = specs
                .get(index + 1)
                .map_or(2.0 * specs[index].cell_size, |coarser| coarser.cell_size);
            rings.push(HeightfieldRing {
                cell_size: specs[index].cell_size,
                tile_size: tile(index),
                per_parent: per_parent[index],
                reach_tiles: reach_tiles[index],
                morph,
                skirt,
            });
        }
        Ok(Self { tile_cells, rings })
    }

    /// Vertices along a tile's side.
    pub fn side(&self) -> usize {
        self.tile_cells as usize + 1
    }

    /// Height samples along a tile's side: its vertices and a ring of one
    /// more about them, for normals at its edges.
    pub fn height_side(&self) -> usize {
        self.tile_cells as usize + 3
    }

    /// The tiles of `ring` that the focus calls for.
    pub fn region(&self, ring: usize, focus: [f64; 2]) -> TileRect {
        let layout = &self.rings[ring];
        let region = layout.region_tile();
        let centre = focus.map(|value| (value / region).floor() as i64);
        let (n, k) = (layout.reach_tiles, layout.per_parent);
        TileRect {
            min: centre.map(|c| (c - n) * k),
            max: centre.map(|c| (c + n + 1) * k - 1),
        }
    }

    /// Where a tile's first vertex stands.
    pub fn origin(&self, key: TileKey) -> [f64; 2] {
        let size = self.rings[key.ring as usize].tile_size;
        [key.x as f64 * size, key.z as f64 * size]
    }

    /// The coarser ring's tile holding `key`.
    pub fn parent(&self, key: TileKey) -> Option<TileKey> {
        let k = self.rings[key.ring as usize].per_parent;
        (usize::from(key.ring) + 1 < self.rings.len()).then(|| TileKey {
            ring: key.ring + 1,
            x: key.x.div_euclid(k),
            z: key.z.div_euclid(k),
        })
    }

    /// The finer ring's tiles within `key`.
    pub fn children(&self, key: TileKey) -> impl Iterator<Item = TileKey> {
        let ring = key.ring.checked_sub(1);
        let k = ring.map_or(0, |ring| self.rings[ring as usize].per_parent);
        (0..k * k).filter_map(move |index| {
            Some(TileKey {
                ring: ring?,
                x: key.x * k + index % k,
                z: key.z * k + index / k,
            })
        })
    }
}

/// What a product samples for one tile: heights at its vertices and the
/// ring about them, `height_side()`² from sample (−1, −1), x fastest (NaN:
/// no surface); and at its vertices alone, `side()`² from (0, 0), optional
/// linear colours or terrain layer weights and material indices.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TileSamples {
    pub heights: Vec<f32>,
    pub colors: Option<Vec<[f32; 4]>>,
    pub layer_weights: Option<Vec<[f32; 4]>>,
    pub materials: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileSamplesError {
    HeightCount {
        expected: usize,
        actual: usize,
    },
    StreamCount {
        expected: usize,
        actual: usize,
    },
    ColorsAndWeights,
    /// A colour or weight outside 0 to 1, or a height that is infinite.
    Value,
}

impl TileSamples {
    pub fn validate(&self, layout: &HeightfieldLayout) -> Result<(), TileSamplesError> {
        let expected = layout.height_side().pow(2);
        if self.heights.len() != expected {
            return Err(TileSamplesError::HeightCount {
                expected,
                actual: self.heights.len(),
            });
        }
        if self.heights.iter().any(|height| height.is_infinite()) {
            return Err(TileSamplesError::Value);
        }
        if self.colors.is_some() && self.layer_weights.is_some() {
            return Err(TileSamplesError::ColorsAndWeights);
        }
        let expected = layout.side().pow(2);
        let lengths = [
            self.colors.as_ref().map(Vec::len),
            self.layer_weights.as_ref().map(Vec::len),
            self.materials.as_ref().map(Vec::len),
        ];
        if let Some(actual) = lengths.into_iter().flatten().find(|len| *len != expected) {
            return Err(TileSamplesError::StreamCount { expected, actual });
        }
        let unit = |value: &f32| (0.0..=1.0).contains(value);
        if self
            .colors
            .iter()
            .chain(&self.layer_weights)
            .flatten()
            .any(|rgba| !rgba.iter().all(unit))
        {
            return Err(TileSamplesError::Value);
        }
        Ok(())
    }

    /// The height at vertex (i, j), −1 to `cells + 1`.
    fn height(&self, side: usize, i: i64, j: i64) -> f32 {
        self.heights[(j + 1) as usize * side + (i + 1) as usize]
    }
}

/// A tile's samples with the normals its mesh is drawn with, which the
/// finer ring's targets and picking read.
#[derive(Debug, Clone, PartialEq)]
pub struct TileSurface {
    cells: usize,
    cell_size: f64,
    samples: TileSamples,
    /// Each vertex's normal, `side()`², x fastest.
    normals: Vec<[f32; 3]>,
}

impl TileSurface {
    pub fn new(layout: &HeightfieldLayout, ring: usize, samples: TileSamples) -> Self {
        let cells = layout.tile_cells as usize;
        let cell_size = layout.rings[ring].cell_size;
        let side = cells + 1;
        let heights = cells + 3;
        let mut normals = Vec::with_capacity(side * side);
        for j in 0..side as i64 {
            for i in 0..side as i64 {
                let h = |di: i64, dj: i64| samples.height(heights, i + di, j + dj);
                let slope = |before: f32, here: f32, after: f32| -> f32 {
                    match (before.is_nan(), after.is_nan()) {
                        (false, false) => (after - before) / 2.0,
                        (true, false) => after - here,
                        (false, true) => here - before,
                        (true, true) => 0.0,
                    }
                };
                let here = h(0, 0);
                let (sx, sz) = if here.is_nan() {
                    (0.0, 0.0)
                } else {
                    (
                        slope(h(-1, 0), here, h(1, 0)) / cell_size as f32,
                        slope(h(0, -1), here, h(0, 1)) / cell_size as f32,
                    )
                };
                let length = (sx * sx + 1.0 + sz * sz).sqrt();
                normals.push([-sx / length, 1.0 / length, -sz / length]);
            }
        }
        Self {
            cells,
            cell_size,
            samples,
            normals,
        }
    }

    pub fn samples(&self) -> &TileSamples {
        &self.samples
    }

    fn side(&self) -> usize {
        self.cells + 1
    }

    fn height(&self, i: usize, j: usize) -> f32 {
        self.samples.height(self.cells + 3, i as i64, j as i64)
    }

    fn normal(&self, i: usize, j: usize) -> [f32; 3] {
        self.normals[j * self.side() + i]
    }

    /// Whether cell (i, j) is split along its (0, 0)–(1, 1) diagonal: the
    /// diagonal whose ends are nearer in height, so a ridge or a gully is
    /// not cut across.
    fn falling_diagonal(&self, i: usize, j: usize) -> bool {
        let (a, b, c, d) = (
            self.height(i, j),
            self.height(i + 1, j),
            self.height(i, j + 1),
            self.height(i + 1, j + 1),
        );
        (a - d).abs() <= (b - c).abs()
    }

    /// The drawn surface's height and (unnormalized) normal at tile-local
    /// (x, z): the triangle of its cell it falls in, weighted as the GPU
    /// interpolates. None in a hole or outside the tile.
    pub fn surface(&self, x: f64, z: f64) -> Option<(f32, [f32; 3])> {
        let limit = self.cells as f64;
        // A place on the tile's edge may round a hair past it.
        let slack = limit * 1e-9;
        let (u, v) = (x / self.cell_size, z / self.cell_size);
        if ![u, v]
            .iter()
            .all(|value| (-slack..=limit + slack).contains(value))
        {
            return None;
        }
        let (u, v) = (u.clamp(0.0, limit), v.clamp(0.0, limit));
        let i = (u.floor() as usize).min(self.cells - 1);
        let j = (v.floor() as usize).min(self.cells - 1);
        let (fu, fv) = ((u - i as f64) as f32, (v - j as f64) as f32);
        let corners = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)];
        if corners.iter().any(|(a, b)| self.height(*a, *b).is_nan()) {
            return None;
        }
        // Weights of corners a (i, j), b (i+1, j), c (i, j+1), d (i+1, j+1).
        let weights = if self.falling_diagonal(i, j) {
            if fv >= fu {
                [1.0 - fv, 0.0, fv - fu, fu]
            } else {
                [1.0 - fu, fu - fv, 0.0, fv]
            }
        } else if fu + fv <= 1.0 {
            [1.0 - fu - fv, fu, fv, 0.0]
        } else {
            [0.0, 1.0 - fv, 1.0 - fu, fu + fv - 1.0]
        };
        let mut height = 0.0;
        let mut normal = [0.0; 3];
        for ((a, b), weight) in corners.into_iter().zip(weights) {
            height += self.height(a, b) * weight;
            let n = self.normal(a, b);
            for axis in 0..3 {
                normal[axis] += n[axis] * weight;
            }
        }
        Some((height, normal))
    }
}

/// A meshed tile in its own coordinates (its first vertex at the origin).
#[derive(Debug, Clone, PartialEq)]
pub struct TileMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    /// The heightfield's own x and z at each vertex: the layers' tile
    /// coordinates.
    pub uvs: Vec<f32>,
    /// Linear RGBA colours, or terrain layer weights (`layer_weights`).
    pub colors: Option<Vec<f32>>,
    pub layer_weights: bool,
    pub indices: Vec<u32>,
    /// (material index, first index, index count), by material.
    pub groups: Vec<(u32, u32, u32)>,
    /// Each vertex's height change and target normal x and z, for a ring
    /// with a coarser one.
    pub morph: Option<Vec<f32>>,
    /// The ring's morph range (`HeightfieldRing::morph`), with the targets.
    pub morph_range: Option<(f64, f64)>,
    /// Bounds of every vertex and its target.
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Each grid vertex's index, `side()`², x fastest; −1 in a hole.
    grid: Vec<i32>,
    cells: usize,
    cell_size: f64,
}

/// Mesh `key` from its samples, with targets on the coarser ring's surface
/// (`parent`, its tile and where that tile stands) where it has one. None
/// where nothing in it is sampled.
pub fn mesh_tile(
    layout: &HeightfieldLayout,
    key: TileKey,
    surface: &TileSurface,
    parent: Option<(&TileSurface, [f64; 2])>,
) -> Option<TileMesh> {
    let ring = &layout.rings[key.ring as usize];
    let cells = layout.tile_cells as usize;
    let side = cells + 1;
    let cell = ring.cell_size;
    let origin = layout.origin(key);
    let samples = surface.samples();
    let mut grid = vec![-1_i32; side * side];
    let mut positions = Vec::with_capacity(side * side * 3);
    let mut normals = Vec::with_capacity(side * side * 3);
    let mut uvs = Vec::with_capacity(side * side * 2);
    let streamed = samples.colors.as_ref().or(samples.layer_weights.as_ref());
    let mut colors = streamed.map(|_| Vec::with_capacity(side * side * 4));
    let mut materials = Vec::with_capacity(side * side);
    let mut morph = parent.map(|_| Vec::with_capacity(side * side * 3));
    for j in 0..side {
        for i in 0..side {
            let height = surface.height(i, j);
            if height.is_nan() {
                continue;
            }
            let at = j * side + i;
            grid[at] = (positions.len() / 3) as i32;
            let (x, z) = (i as f64 * cell, j as f64 * cell);
            positions.extend_from_slice(&[x as f32, height, z as f32]);
            let normal = surface.normal(i, j);
            normals.extend_from_slice(&normal);
            uvs.extend_from_slice(&[(origin[0] + x) as f32, (origin[1] + z) as f32]);
            if let (Some(colors), Some(stream)) = (colors.as_mut(), streamed) {
                colors.extend_from_slice(&stream[at]);
            }
            materials.push(samples.materials.as_ref().map_or(0, |values| values[at]));
            if let (Some(morph), Some((coarser, at))) = (morph.as_mut(), parent) {
                // The coarser surface where this vertex stands.
                let target = coarser.surface(origin[0] + x - at[0], origin[1] + z - at[1]);
                morph.extend_from_slice(&match target {
                    Some((target, n)) => {
                        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                        [target - height, n[0] / length, n[2] / length]
                    }
                    None => [0.0, normal[0], normal[2]],
                });
            }
        }
    }
    let mut by_material: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for j in 0..cells {
        for i in 0..cells {
            let corner = |di: usize, dj: usize| grid[(j + dj) * side + i + di];
            let (a, b, c, d) = (corner(0, 0), corner(1, 0), corner(0, 1), corner(1, 1));
            if a < 0 || b < 0 || c < 0 || d < 0 {
                continue;
            }
            let triangles = if surface.falling_diagonal(i, j) {
                [[a, c, d], [a, d, b]]
            } else {
                [[a, c, b], [b, c, d]]
            };
            for triangle in triangles {
                let material = triangle_material(triangle.map(|vertex| materials[vertex as usize]));
                by_material
                    .entry(material)
                    .or_default()
                    .extend(triangle.map(|vertex| vertex as u32));
            }
        }
    }
    if by_material.is_empty() {
        return None;
    }
    // A skirt hangs below each edge, facing out, drawn as its top's material.
    // (first vertex, step along the edge, wound the other way)
    #[allow(clippy::type_complexity)]
    let edges: [((usize, usize), (usize, usize), bool); 4] = [
        ((0, 0), (1, 0), false),
        ((cells, 0), (0, 1), false),
        ((0, cells), (1, 0), true),
        ((0, 0), (0, 1), true),
    ];
    let drop = ring.skirt as f32;
    for ((x0, z0), (dx, dz), flip) in edges {
        for step in 0..cells {
            let (i, j) = (x0 + step * dx, z0 + step * dz);
            let (a, b) = (grid[j * side + i], grid[(j + dz) * side + i + dx]);
            if a < 0 || b < 0 {
                continue;
            }
            let start = (positions.len() / 3) as u32;
            for top in [a as usize, b as usize] {
                positions.extend_from_slice(&[
                    positions[top * 3],
                    positions[top * 3 + 1] - drop,
                    positions[top * 3 + 2],
                ]);
                normals.extend_from_slice(&[
                    normals[top * 3],
                    normals[top * 3 + 1],
                    normals[top * 3 + 2],
                ]);
                uvs.extend_from_slice(&[uvs[top * 2], uvs[top * 2 + 1]]);
                if let Some(colors) = colors.as_mut() {
                    let rgba: [f32; 4] = colors[top * 4..top * 4 + 4].try_into().unwrap();
                    colors.extend_from_slice(&rgba);
                }
                if let Some(morph) = morph.as_mut() {
                    let target: [f32; 3] = morph[top * 3..top * 3 + 3].try_into().unwrap();
                    morph.extend_from_slice(&target);
                }
            }
            let (a, b) = (a as u32, b as u32);
            let quad = if flip {
                [b, a, start + 1, start + 1, a, start]
            } else {
                [a, b, start, start, b, start + 1]
            };
            by_material
                .entry(materials[a as usize])
                .or_default()
                .extend(quad);
        }
    }
    let mut indices = Vec::new();
    let mut groups = Vec::new();
    for (material, triangles) in by_material {
        groups.push((material, indices.len() as u32, triangles.len() as u32));
        indices.extend(triangles);
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for (index, point) in positions.as_chunks::<3>().0.iter().enumerate() {
        let rise = morph.as_ref().map_or(0.0, |morph| morph[index * 3]);
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
        min[1] = min[1].min(point[1] + rise);
        max[1] = max[1].max(point[1] + rise);
    }
    Some(TileMesh {
        positions,
        normals,
        uvs,
        colors,
        layer_weights: samples.layer_weights.is_some(),
        indices,
        groups,
        morph_range: ring.morph.filter(|_| morph.is_some()),
        morph,
        min,
        max,
        grid,
        cells,
        cell_size: cell,
    })
}

/// A triangle's material: the one most of its corners have, else the
/// highest, so a product orders its materials by which wins a three-way
/// meeting.
fn triangle_material(corners: [u32; 3]) -> u32 {
    let [a, b, c] = corners;
    if a == b || a == c {
        a
    } else if b == c {
        b
    } else {
        a.max(b).max(c)
    }
}

/// Where a ray first meets a tile's drawn surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileHit {
    /// Along the ray, in units of its direction's length.
    pub distance: f64,
    /// Tile-local.
    pub point: [f64; 3],
    pub normal: [f64; 3],
}

impl TileMesh {
    /// The first point along a tile-local ray where the drawn surface is
    /// met from above, its vertices raised by `share(x, z)` of their morph
    /// (the morph share at a tile-local place, as the renderer reckons it).
    pub fn pick(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        share: impl Fn(f64, f64) -> f64,
    ) -> Option<TileHit> {
        let side = self.cells + 1;
        let extent = self.cells as f64 * self.cell_size;
        // The ray's span over the tile's square, in x and z.
        let (mut enter, mut leave) = (0.0_f64, f64::INFINITY);
        for axis in [0, 2] {
            if direction[axis].abs() < 1e-12 {
                if !(0.0..=extent).contains(&origin[axis]) {
                    return None;
                }
            } else {
                let a = (0.0 - origin[axis]) / direction[axis];
                let b = (extent - origin[axis]) / direction[axis];
                enter = enter.max(a.min(b));
                leave = leave.min(a.max(b));
            }
        }
        if enter > leave {
            return None;
        }
        let vertex = |i: usize, j: usize| -> Option<[f64; 3]> {
            let index = self.grid[j * side + i];
            (index >= 0).then(|| {
                let index = index as usize;
                let p = &self.positions[index * 3..index * 3 + 3];
                let (x, z) = (f64::from(p[0]), f64::from(p[2]));
                let rise = self
                    .morph
                    .as_ref()
                    .map_or(0.0, |morph| f64::from(morph[index * 3]) * share(x, z));
                [x, f64::from(p[1]) + rise, z]
            })
        };
        // Walk the cells the ray's shadow crosses, nearest first.
        let point =
            |t: f64| std::array::from_fn::<f64, 3, _>(|axis| origin[axis] + direction[axis] * t);
        let cell_of =
            |value: f64| ((value / self.cell_size).floor() as i64).clamp(0, self.cells as i64 - 1);
        let start = point(enter);
        let (mut i, mut j) = (cell_of(start[0]), cell_of(start[2]));
        let step = |axis: usize| if direction[axis] > 0.0 { 1 } else { -1 };
        let next_edge = |cell: i64, axis: usize| -> f64 {
            if direction[axis].abs() < 1e-12 {
                return f64::INFINITY;
            }
            let edge = if direction[axis] > 0.0 {
                cell + 1
            } else {
                cell
            } as f64
                * self.cell_size;
            (edge - origin[axis]) / direction[axis]
        };
        loop {
            let (ci, cj) = (i as usize, j as usize);
            let corners = [
                vertex(ci, cj),
                vertex(ci + 1, cj),
                vertex(ci, cj + 1),
                vertex(ci + 1, cj + 1),
            ];
            if let [Some(a), Some(b), Some(c), Some(d)] = corners {
                // The diagonal `mesh_tile` drew, chosen from the unmorphed
                // heights: a morph moves the corners, not the indices.
                let base =
                    |i: usize, j: usize| self.positions[self.grid[j * side + i] as usize * 3 + 1];
                let falling = (base(ci, cj) - base(ci + 1, cj + 1)).abs()
                    <= (base(ci + 1, cj) - base(ci, cj + 1)).abs();
                let triangles = if falling {
                    [[a, c, d], [a, d, b]]
                } else {
                    [[a, c, b], [b, c, d]]
                };
                let best = triangles
                    .iter()
                    .filter_map(|triangle| intersect(origin, direction, triangle))
                    .min_by(|x, y| x.distance.total_cmp(&y.distance));
                if best.is_some() {
                    return best;
                }
            }
            let (tx, tz) = (next_edge(i, 0), next_edge(j, 2));
            if tx.min(tz) > leave {
                return None;
            }
            if tx <= tz {
                i += step(0);
            } else {
                j += step(2);
            }
            if !(0..self.cells as i64).contains(&i) || !(0..self.cells as i64).contains(&j) {
                return None;
            }
        }
    }
}

/// Where a ray meets a triangle's upper face (Möller–Trumbore).
fn intersect(origin: [f64; 3], direction: [f64; 3], triangle: &[[f64; 3]; 3]) -> Option<TileHit> {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let [p0, p1, p2] = *triangle;
    let (e1, e2) = (sub(p1, p0), sub(p2, p0));
    let normal = cross(e1, e2);
    // Only the upper face: the ray runs against the normal.
    if dot(normal, direction) >= 0.0 {
        return None;
    }
    let h = cross(direction, e2);
    let a = dot(e1, h);
    let f = 1.0 / a;
    let s = sub(origin, p0);
    let u = f * dot(s, h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = f * dot(direction, q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = f * dot(e2, q);
    if distance < 0.0 {
        return None;
    }
    let length = dot(normal, normal).sqrt();
    Some(TileHit {
        distance,
        point: std::array::from_fn(|axis| origin[axis] + direction[axis] * distance),
        normal: normal.map(|value| value / length),
    })
}

#[cfg(test)]
#[path = "heightfield_tests.rs"]
mod tests;
