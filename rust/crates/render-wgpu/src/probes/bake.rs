//! The brick bake: each of a brick's probes gathers its rays, the bounces
//! relax against the volume's current probes, probes inside geometry are
//! filled from their neighbours, and a region of the volume is packed for
//! upload.

use std::time::Instant;

use glam::Vec3;

use super::trace::{direct, fibonacci, Scene, Sky};
use super::{Grid, BACKFACE_LIMIT, NORMAL_OFFSET, OPEN_SIDE, RAYS};

pub(super) const SH_Y0: f32 = 0.282_095;
pub(super) const SH_Y1: f32 = 0.488_603;

/// A probe's L1 coefficients: Y0, then Y1 by y, z and x.
pub(super) type Sh = [Vec3; 4];

/// The volume's probes on the CPU: the texture's source.
#[derive(Clone)]
pub(super) struct Field {
    pub grid: Grid,
    /// Each probe's coefficients: its own where `raw_valid`, a fill from its
    /// neighbours where only `filled`.
    pub sh: Vec<Sh>,
    /// The probe's own rays mostly left geometry: its coefficients are its
    /// own.
    pub raw_valid: Vec<bool>,
    /// The probe has coefficients, its own or filled.
    pub filled: Vec<bool>,
    /// The mean direction of the probe's rays that did not hit a back face.
    pub opens: Vec<Vec3>,
    /// The direction of the probe's nearest back-face hit.
    pub exits: Vec<Vec3>,
    /// How far, in spacings, the probe's rays along +x, +y and +z travel
    /// before they meet a surface; 1 when none does within a spacing.
    pub reach: Vec<[f32; 3]>,
}

impl Field {
    pub fn new(grid: Grid) -> Self {
        let probes = grid.probes();
        Self {
            grid,
            sh: vec![[Vec3::ZERO; 4]; probes],
            raw_valid: vec![false; probes],
            filled: vec![false; probes],
            opens: vec![Vec3::ZERO; probes],
            exits: vec![Vec3::ZERO; probes],
            reach: vec![[1.0; 3]; probes],
        }
    }

    /// Trilinear irradiance at a surface from the probes with values around
    /// it, `lookup` giving a probe's coefficients (none for a probe without
    /// a value); `None` outside the grid or among valueless probes only.
    pub fn irradiance_with(
        grid: &Grid,
        position: Vec3,
        normal: Vec3,
        lookup: impl Fn(usize) -> Option<Sh>,
    ) -> Option<Vec3> {
        let sample = position + normal * (grid.spacing * NORMAL_OFFSET);
        let cell = (sample - grid.min) / grid.spacing;
        let extent = Vec3::new(
            (grid.dims[0] - 1) as f32,
            (grid.dims[1] - 1) as f32,
            (grid.dims[2] - 1) as f32,
        );
        if cell.cmplt(Vec3::ZERO).any() || cell.cmpgt(extent).any() {
            return None;
        }
        let base = cell.floor().min(extent - 1.0).max(Vec3::ZERO);
        let t = cell - base;
        let mut sum = [Vec3::ZERO; 4];
        let mut total = 0.0;
        for corner in 0..8_u32 {
            let offset = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
            let at = [
                (base.x as u32 + offset[0]).min(grid.dims[0] - 1),
                (base.y as u32 + offset[1]).min(grid.dims[1] - 1),
                (base.z as u32 + offset[2]).min(grid.dims[2] - 1),
            ];
            let Some(sh) = lookup(grid.index(at)) else {
                continue;
            };
            let weight = (0..3)
                .map(|axis| {
                    if offset[axis] == 1 {
                        t[axis]
                    } else {
                        1.0 - t[axis]
                    }
                })
                .product::<f32>();
            for (sum, coefficient) in sum.iter_mut().zip(sh) {
                *sum += coefficient * weight;
            }
            total += weight;
        }
        (total > 1e-4).then(|| irradiance_along(&sum.map(|c| c / total), normal))
    }
}

/// Irradiance along `normal` from a probe's coefficients.
pub(super) fn irradiance_along(sh: &Sh, normal: Vec3) -> Vec3 {
    (sh[0] * SH_Y0
        + sh[1] * (SH_Y1 * normal.y)
        + sh[2] * (SH_Y1 * normal.z)
        + sh[3] * (SH_Y1 * normal.x))
        .max(Vec3::ZERO)
}

/// Project radiance samples over uniform directions to cosine-convolved L1
/// irradiance coefficients.
pub(super) fn project(directions: &[Vec3], radiance: impl Iterator<Item = Vec3>) -> Sh {
    let mut sum = [Vec3::ZERO; 4];
    for (d, l) in directions.iter().zip(radiance) {
        sum[0] += l * SH_Y0;
        sum[1] += l * (SH_Y1 * d.y);
        sum[2] += l * (SH_Y1 * d.z);
        sum[3] += l * (SH_Y1 * d.x);
    }
    let solid_angle = 4.0 * std::f32::consts::PI / directions.len() as f32;
    let pi = std::f32::consts::PI;
    [
        sum[0] * solid_angle * pi,
        sum[1] * solid_angle * (2.0 * pi / 3.0),
        sum[2] * solid_angle * (2.0 * pi / 3.0),
        sum[3] * solid_angle * (2.0 * pi / 3.0),
    ]
}

/// What a probe ray found, kept across passes.
#[derive(Clone, Copy)]
struct RayRecord {
    /// Radiance that does not change between passes: the sky on a miss, or
    /// a hit's emission plus its albedo times the direct light.
    fixed: Vec3,
    /// A front-face hit: where, facing, and its albedo over π.
    position: Vec3,
    normal: Vec3,
    reflectance: Vec3,
    kind: RayKind,
}

#[derive(Clone, Copy, PartialEq)]
enum RayKind {
    Miss,
    Front,
    Back,
}

/// A brick's probes, baked: in the brick's own order (x fastest).
pub(super) struct BrickBaked {
    pub sh: Vec<Sh>,
    pub raw_valid: Vec<bool>,
    pub opens: Vec<Vec3>,
    pub exits: Vec<Vec3>,
    pub reach: Vec<[f32; 3]>,
    /// Wall milliseconds of this brick's bake.
    pub ms: f64,
}

/// The probe coordinates of a brick's range, x fastest.
pub(super) fn cells_in(range: [[u32; 2]; 3]) -> Vec<[u32; 3]> {
    (range[2][0]..=range[2][1])
        .flat_map(|z| {
            (range[1][0]..=range[1][1])
                .flat_map(move |y| (range[0][0]..=range[0][1]).map(move |x| [x, y, z]))
        })
        .collect()
}

/// A probe's index within a brick's range, or `None` outside it.
fn local_index(range: [[u32; 2]; 3], [x, y, z]: [u32; 3]) -> Option<usize> {
    let inside = |axis: usize, v: u32| (range[axis][0]..=range[axis][1]).contains(&v);
    (inside(0, x) && inside(1, y) && inside(2, z)).then(|| {
        let w = range[0][1] - range[0][0] + 1;
        let h = range[1][1] - range[1][0] + 1;
        (((z - range[2][0]) * h + (y - range[1][0])) * w + (x - range[0][0])) as usize
    })
}

/// Bake one brick: trace every probe in `range`, relax the bounces against
/// the brick's own first pass and the rest of the volume's current probes.
pub(super) fn bake_brick(
    scene: &Scene,
    grid: &Grid,
    range: [[u32; 2]; 3],
    bounces: u32,
    rows: &[f32],
    sky: &Sky,
    snapshot: &Field,
) -> BrickBaked {
    let started = Instant::now();
    let horizon = scene.horizon();
    let directions = fibonacci(RAYS);
    let pi = std::f32::consts::PI;
    let cells = cells_in(range);
    let probes = cells.len();
    let miss = RayRecord {
        fixed: Vec3::ZERO,
        position: Vec3::ZERO,
        normal: Vec3::ZERO,
        reflectance: Vec3::ZERO,
        kind: RayKind::Miss,
    };
    let mut records = vec![miss; probes * RAYS as usize];
    let mut sh = vec![[Vec3::ZERO; 4]; probes];
    let mut raw_valid = vec![true; probes];
    let mut opens = vec![Vec3::ZERO; probes];
    let mut exits = vec![Vec3::ZERO; probes];
    let mut reach = vec![[1.0_f32; 3]; probes];
    for pass in 0..bounces.max(1) {
        let (previous_sh, previous_valid) = (sh.clone(), raw_valid.clone());
        let lookup = |index: usize| -> Option<Sh> {
            match local_index(range, grid.coords(index)) {
                Some(local) => previous_valid[local].then(|| previous_sh[local]),
                None => snapshot.filled[index].then(|| snapshot.sh[index]),
            }
        };
        let mut items: Vec<_> = records
            .chunks_mut(RAYS as usize)
            .zip(&mut sh)
            .zip(&mut raw_valid)
            .zip(&mut opens)
            .zip(&mut exits)
            .zip(&mut reach)
            .zip(&cells)
            .collect();
        parallel(
            &mut items,
            |((((((records, sh), valid), open), exit), reach), cell)| {
                let origin = grid.position(**cell);
                if pass == 0 {
                    // Walls within a spacing along each axis, either face.
                    for (axis, direction) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
                        reach[axis] = scene
                            .trace(origin, direction, grid.spacing, false)
                            .map_or(1.0, |(distance, _)| distance / grid.spacing);
                    }
                    let mut backs = 0;
                    let mut open_sum = Vec3::ZERO;
                    let mut nearest_back = f32::MAX;
                    for (record, direction) in records.iter_mut().zip(&directions) {
                        *record = match scene.trace(origin, *direction, f32::MAX, false) {
                            None => {
                                open_sum += *direction;
                                RayRecord {
                                    fixed: sky.radiance(*direction),
                                    ..miss
                                }
                            }
                            Some((distance, t)) => {
                                let facing = direction.dot(t.normal) < 0.0;
                                if !facing && !t.two_sided {
                                    backs += 1;
                                    if distance < nearest_back {
                                        nearest_back = distance;
                                        **exit = *direction;
                                    }
                                    RayRecord {
                                        kind: RayKind::Back,
                                        ..miss
                                    }
                                } else {
                                    open_sum += *direction;
                                    let normal = if facing { t.normal } else { -t.normal };
                                    let position = origin + *direction * distance;
                                    let reflectance = t.albedo / pi;
                                    RayRecord {
                                        fixed: t.emission
                                            + reflectance
                                                * direct(scene, rows, horizon, position, normal),
                                        position,
                                        normal,
                                        reflectance,
                                        kind: RayKind::Front,
                                    }
                                }
                            }
                        };
                    }
                    **valid = (backs as f32) <= BACKFACE_LIMIT * RAYS as f32;
                    **open = open_sum / (RAYS - backs).max(1) as f32;
                }
                let radiance = records.iter().map(|record| match record.kind {
                    RayKind::Miss => record.fixed,
                    RayKind::Back => Vec3::ZERO,
                    RayKind::Front if pass == 0 => record.fixed,
                    RayKind::Front => {
                        record.fixed
                            + record.reflectance
                                * Field::irradiance_with(
                                    grid,
                                    record.position,
                                    record.normal,
                                    lookup,
                                )
                                .unwrap_or(Vec3::ZERO)
                    }
                });
                **sh = project(&directions, radiance);
            },
        );
    }
    BrickBaked {
        sh,
        raw_valid,
        opens,
        exits,
        reach,
        ms: ms(started),
    }
}

/// Fill the probes of `lo..=hi` that sit inside geometry from their
/// neighbours with values, so a trilinear sample at a wall never reads a
/// dark hole. A probe on a surface (half its rays into the wall behind it)
/// takes the neighbours on the side its rays found open; one inside a wall
/// takes the side its nearest way out faces, so the room side of a thin wall
/// does not read the daylight beyond it; one with neither takes every
/// neighbour with a value. Earlier fills in the region are redone from the
/// current values; repeated until nothing is left to fill from.
pub(super) fn dilate_region(field: &mut Field, lo: [u32; 3], hi: [u32; 3]) {
    let grid = field.grid;
    let dims = grid.dims;
    let cells = cells_in([[lo[0], hi[0]], [lo[1], hi[1]], [lo[2], hi[2]]]);
    // Has a value: its own, a fill outside the region, or a fill made here.
    let mut has = field.filled.clone();
    for cell in &cells {
        let index = grid.index(*cell);
        has[index] = field.raw_valid[index];
    }
    for _ in 0..16 {
        let mut fills = Vec::new();
        for cell in &cells {
            let [x, y, z] = *cell;
            let index = grid.index(*cell);
            if has[index] {
                continue;
            }
            let neighbours = [
                (x.checked_sub(1), Some(y), Some(z), -Vec3::X),
                (
                    (x + 1 < dims[0]).then_some(x + 1),
                    Some(y),
                    Some(z),
                    Vec3::X,
                ),
                (Some(x), y.checked_sub(1), Some(z), -Vec3::Y),
                (
                    Some(x),
                    (y + 1 < dims[1]).then_some(y + 1),
                    Some(z),
                    Vec3::Y,
                ),
                (Some(x), Some(y), z.checked_sub(1), -Vec3::Z),
                (
                    Some(x),
                    Some(y),
                    (z + 1 < dims[2]).then_some(z + 1),
                    Vec3::Z,
                ),
            ];
            let with_values: Vec<(usize, Vec3)> = neighbours
                .into_iter()
                .filter_map(|(nx, ny, nz, toward)| {
                    let at = grid.index([nx?, ny?, nz?]);
                    has[at].then_some((at, toward))
                })
                .collect();
            let open = field.opens[index];
            let side = if open.length() > OPEN_SIDE {
                Some(open)
            } else {
                (field.exits[index] != Vec3::ZERO).then_some(field.exits[index])
            };
            let mut from: Vec<usize> = match side {
                Some(side) => with_values
                    .iter()
                    .filter(|(_, toward)| toward.dot(side) > 0.0)
                    .map(|(at, _)| *at)
                    .collect(),
                None => Vec::new(),
            };
            if from.is_empty() {
                from = with_values.iter().map(|(at, _)| *at).collect();
            }
            if from.is_empty() {
                continue;
            }
            let mut sum = [Vec3::ZERO; 4];
            for at in &from {
                for (sum, c) in sum.iter_mut().zip(field.sh[*at]) {
                    *sum += c;
                }
            }
            fills.push((index, sum.map(|c| c / from.len() as f32)));
        }
        if fills.is_empty() {
            break;
        }
        for (index, sh) in fills {
            field.sh[index] = sh;
            has[index] = true;
        }
    }
    for cell in &cells {
        let index = grid.index(*cell);
        field.filled[index] = has[index];
    }
}

/// The texels of `lo..=hi` as half floats, rows along x: red's block of
/// probes, then green's, then blue's, each probe its four coefficients; or
/// `compact`, one block: the ambient coefficient's colour with the vertical
/// coefficient's luminance (`lighting.wgsl` gives it the ambient's hue; the
/// horizontal coefficients are dropped). Without `compact` a last block
/// holds each probe's cell walls ([`cell_walls`]).
pub(super) fn pack_region(field: &Field, lo: [u32; 3], hi: [u32; 3], compact: bool) -> Vec<u16> {
    let cells = cells_in([[lo[0], hi[0]], [lo[1], hi[1]], [lo[2], hi[2]]]);
    let mut texels = Vec::with_capacity(cells.len() * 16);
    if compact {
        for cell in &cells {
            let sh = field.sh[field.grid.index(*cell)];
            for value in [sh[0].x, sh[0].y, sh[0].z, luminance_of(sh[1])] {
                texels.push(half_bits(value));
            }
        }
        return texels;
    }
    for channel in 0..3 {
        for cell in &cells {
            let sh = field.sh[field.grid.index(*cell)];
            for coefficient in sh {
                texels.push(half_bits(coefficient[channel]));
            }
        }
    }
    for cell in &cells {
        let walls = cell_walls(field, *cell);
        for value in [walls[0], walls[1], walls[2], 0.0] {
            texels.push(half_bits(value));
        }
    }
    texels
}

/// No wall across a cell's axis ([`cell_walls`]).
pub(super) const NO_WALL: f32 = 0.0;

/// Where walls cross the cell whose lowest probe is `low`, along each axis:
/// which of the four probes of the cell's low face meet a surface along the
/// axis within the cell (bits 1, 2, 4 and 8 for the face's corners, the
/// next axis then the one after it varying; a probe inside geometry counts
/// as one meeting it at the face), as the value's whole part, and
/// the farthest of those hits as a fraction of the spacing, below 1, as its
/// fractional part; 0 for none. `lighting.wgsl` keeps a sample on its own
/// side of the wall by as much of the face as the hitting probes weigh at
/// the sample, so a wall thinner than the spacing does not pass the light
/// beyond it, and a wall with an opening keeps its light through the
/// opening; a face's probes are shared with the neighbouring cell, so the
/// sample stays continuous between cells.
pub(super) fn cell_walls(field: &Field, low: [u32; 3]) -> [f32; 3] {
    let grid = &field.grid;
    std::array::from_fn(|axis| {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut farthest = 0.0_f32;
        let mut mask = 0;
        for (bit, (du, dv)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
            let mut probe = low;
            probe[u] = (probe[u] + du).min(grid.dims[u] - 1);
            probe[v] = (probe[v] + dv).min(grid.dims[v] - 1);
            let index = grid.index(probe);
            let reach = field.reach[index][axis];
            if !field.raw_valid[index] {
                // Inside geometry, its rays may cross no surface, and its
                // value is a fill: it counts as a wall at the face.
                mask |= 1 << bit;
            } else if reach < 1.0 {
                mask |= 1 << bit;
                farthest = farthest.max(reach);
            }
        }
        if mask == 0 {
            NO_WALL
        } else {
            // Half floats keep 1/128 between 8 and 16: under 2 cm at 2 m.
            mask as f32 + farthest.min(0.99)
        }
    })
}

/// Rec. 709 luminance.
fn luminance_of(colour: Vec3) -> f32 {
    colour.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// Run `work` over every item on the worker threads, a few at a time.
pub(super) fn parallel<T: Send>(items: &mut [T], work: impl Fn(&mut T) + Sync) {
    const BATCH: usize = 8;
    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    // Leave cores to the product and the render thread.
    let threads = (available / 2)
        .max(1)
        .min(items.len().div_ceil(BATCH).max(1));
    let batches = std::sync::Mutex::new(items.chunks_mut(BATCH).collect::<Vec<_>>());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let Some(batch) = batches.lock().expect("batches").pop() else {
                    break;
                };
                for item in batch {
                    work(item);
                }
            });
        }
    });
}

/// IEEE half-precision bits of `value`, rounded to nearest even.
pub(super) fn half_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        // Infinity or NaN.
        return sign | 0x7c00 | if mantissa != 0 { 0x200 } else { 0 };
    }
    let unbiased = exponent - 127 + 15;
    if unbiased >= 0x1f {
        return sign | 0x7c00;
    }
    if unbiased <= 0 {
        if unbiased < -10 {
            return sign;
        }
        let mantissa = mantissa | 0x80_0000;
        let shift = (14 - unbiased) as u32;
        let half = mantissa >> shift;
        let remainder = mantissa & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        let rounded = if remainder > halfway || (remainder == halfway && half & 1 == 1) {
            half + 1
        } else {
            half
        };
        return sign | rounded as u16;
    }
    let half = ((unbiased as u32) << 10) | (mantissa >> 13);
    let remainder = mantissa & 0x1fff;
    let rounded = if remainder > 0x1000 || (remainder == 0x1000 && half & 1 == 1) {
        half + 1
    } else {
        half
    };
    sign | rounded as u16
}

pub(super) fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{IndirectAmbient, IndirectLightDescriptor};

    fn grid(extent: [f32; 3], spacing: f32) -> Grid {
        Grid::of(&IndirectLightDescriptor {
            // On the lattice, so the extents give odd probe counts.
            center: [10.5, 2.0, -4.0],
            extent,
            spacing,
            bounces: 1,
            ambient: IndirectAmbient::Sky,
        })
    }

    #[test]
    fn a_uniform_sky_projects_to_its_irradiance_along_every_normal() {
        let directions = fibonacci(RAYS);
        let sh = project(&directions, directions.iter().map(|_| Vec3::splat(0.5)));
        for normal in [Vec3::Y, Vec3::X, -Vec3::Z, Vec3::new(0.6, 0.0, 0.8)] {
            let irradiance = irradiance_along(&sh, normal);
            let expected = 0.5 * std::f32::consts::PI;
            assert!(
                (irradiance.x - expected).abs() < 0.03,
                "{normal}: {irradiance} vs {expected}"
            );
        }
    }

    #[test]
    fn a_probe_on_a_surface_is_filled_from_the_side_it_is_open_to() {
        let grid = grid([1.0, 0.5, 0.5], 1.0);
        assert_eq!(grid.dims, [3, 2, 2]);
        let bright = [Vec3::splat(4.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO];
        let dark = [Vec3::splat(0.1), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO];
        let probes = grid.probes();
        let all = ([0, 0, 0], [2, 1, 1]);
        let mut field = Field::new(grid);
        field.sh = vec![dark; probes];
        field.raw_valid = vec![true; probes];
        // Along x: a bright outdoor probe, one on the wall between, a dark
        // room probe; the wall probe's rays were open toward the room.
        for (y, z) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            field.sh[grid.index([0, y, z])] = bright;
            field.raw_valid[grid.index([1, y, z])] = false;
            field.opens[grid.index([1, y, z])] = Vec3::X * 0.5;
        }
        dilate_region(&mut field, all.0, all.1);
        for (y, z) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let at = grid.index([1, y, z]);
            assert!(field.filled[at] && !field.raw_valid[at]);
            assert_eq!(field.sh[at], dark, "the room side fills the wall probe");
        }
        // A probe inside a wall takes the side its nearest way out faces.
        let mut field = Field::new(grid);
        field.sh = vec![dark; probes];
        field.raw_valid = vec![true; probes];
        field.sh[grid.index([0, 0, 0])] = bright;
        field.raw_valid[grid.index([1, 0, 0])] = false;
        field.exits[grid.index([1, 0, 0])] = Vec3::X;
        dilate_region(&mut field, all.0, all.1);
        assert_eq!(field.sh[grid.index([1, 0, 0])], dark);
        // One with neither averages its neighbours, and a later fill of the
        // same region is redone from the current values.
        field.exits[grid.index([1, 0, 0])] = Vec3::ZERO;
        dilate_region(&mut field, all.0, all.1);
        let filled = field.sh[grid.index([1, 0, 0])][0];
        assert!(filled.x > dark[0].x && filled.x < bright[0].x, "{filled}");
    }

    #[test]
    fn a_cells_walls_name_the_face_probes_that_meet_them_and_the_farthest_hit() {
        let grid = grid([1.0, 1.0, 1.0], 1.0);
        let mut field = Field::new(grid);
        field.raw_valid.fill(true);
        // A wall across x at 0.4 from every probe of the low face of the
        // cell at (0, 0, 0).
        for (y, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            field.reach[grid.index([0, y, z])] = [0.4, 1.0, 1.0];
        }
        let walls = cell_walls(&field, [0, 0, 0]);
        assert!((walls[0] - (15.0 + 0.4)).abs() < 1e-6, "{walls:?}");
        assert_eq!(walls[1..], [NO_WALL, NO_WALL]);
        // A door: the probes at y = 1 see through it; one hit farther.
        field.reach[grid.index([0, 1, 0])] = [1.0; 3];
        field.reach[grid.index([0, 1, 1])] = [1.0; 3];
        field.reach[grid.index([0, 0, 1])] = [0.6, 1.0, 1.0];
        let walls = cell_walls(&field, [0, 0, 0]);
        // Bits: (y, z) = (0, 0) is 1 and (0, 1) is 4 (y is the next axis).
        assert!((walls[0] - (5.0 + 0.6)).abs() < 1e-6, "{walls:?}");
        // A probe inside geometry is a wall at the face on every axis.
        field.raw_valid[grid.index([0, 1, 0])] = false;
        let walls = cell_walls(&field, [0, 0, 0]);
        assert!((walls[0] - (7.0 + 0.6)).abs() < 1e-6, "{walls:?}");
        // It is not on the low face across y; across z it is the face's
        // (x, y) = (0, 1) corner.
        assert_eq!(walls[1], NO_WALL);
        assert!((walls[2] - 4.0).abs() < 1e-6, "{walls:?}");
    }

    #[test]
    fn a_region_packs_its_channels_and_cell_walls_in_blocks_of_rows() {
        let grid = grid([1.0, 0.5, 0.5], 1.0);
        let mut field = Field::new(grid);
        field.raw_valid.fill(true);
        for (index, sh) in field.sh.iter_mut().enumerate() {
            *sh = [Vec3::new(index as f32, 0.0, 0.0); 4];
        }
        let texels = pack_region(&field, [1, 0, 0], [2, 1, 1], false);
        // 2 × 2 × 2 probes, 4 halfs each, three channels and the walls.
        assert_eq!(texels.len(), 8 * 4 * 4);
        // The red block starts at probe (1, 0, 0) = index 1, then (2, 0, 0).
        assert_eq!(texels[0], half_bits(1.0));
        assert_eq!(texels[4], half_bits(2.0));
        // Green and blue hold zeros; no probe's rays met a wall.
        assert!(texels[32..96].iter().all(|t| *t == 0));
        assert_eq!(texels[96..99], [half_bits(NO_WALL); 3]);
        // Compact: one block holding the ambient coefficient's colour and the
        // vertical coefficient's luminance.
        field.sh[grid.index([1, 0, 0])] = [
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::splat(1.0),
            Vec3::splat(1.0),
        ];
        let compact = pack_region(&field, [1, 0, 0], [2, 1, 1], true);
        assert_eq!(compact.len(), 8 * 4);
        assert_eq!(
            &compact[..4],
            &[
                half_bits(1.0),
                half_bits(2.0),
                half_bits(3.0),
                half_bits(0.7152)
            ]
        );
    }
}
