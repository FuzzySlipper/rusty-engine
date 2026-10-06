//! Indirect light: an irradiance probe volume the Engine bakes from the
//! renderer's own triangles and light rows, and the standard shader samples
//! as the ambient term where the volume covers a fragment (#9596).
//!
//! A product asks for one volume (`SetIndirectLight`: a box, the spacing
//! between probes, the bounces, and whether the ambient light is the sky or a
//! floor). A grid of probes over the box each gather the radiance arriving
//! from a Fibonacci sphere of rays, baked on worker threads, never the render
//! thread: a ray that leaves the scene sees the sky (the ambient and
//! hemisphere rows and, when the sky's light is on, its irradiance
//! harmonics); a ray that hits a surface sees its emission, its albedo times
//! the direct light rows reaching it through shadow rays, and from the second
//! pass its albedo times the previous pass's probe irradiance there (one more
//! bounce per pass). Each probe keeps its irradiance as L1 spherical
//! harmonics (four RGB coefficients, cosine convolved), and a probe whose rays
//! mostly hit back faces sits inside geometry and is filled from its valid
//! neighbours, so the shader's trilinear sample never reads a dark hole.
//!
//! The volume is one RGBA16F 3D texture, the three colour channels stacked
//! along its depth (a probe's four coefficients in a texel), that the shader
//! samples trilinearly at the surface offset along its normal, fading over
//! one cell past the box. Any retained
//! change inside the box rebakes it whole once the scene has been still for
//! `DEBOUNCE` (or every `MAX_WAIT` while something inside keeps moving),
//! while the old volume keeps drawing.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use glam::{Mat4, Vec3};
use render_model::{IndirectAmbient, IndirectLightDescriptor, RenderLayer};

use crate::tables::{MaterialRef, Topology};
use crate::{Gpu, Renderer};

/// Rays per probe: L1 is low-frequency, so few are enough.
const RAYS: u32 = 64;
/// How long the scene stays unchanged before a rebake starts.
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(250);
/// A scene that never stays still (something moving inside the box every
/// frame) rebakes this often.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(2);
/// A probe with more back-face hits than this sits inside geometry.
const BACKFACE_LIMIT: f32 = 0.25;
/// Rays and surface lookups start this far off a surface.
const SURFACE_OFFSET: f32 = 0.01;
/// The shader's normal offset, in probe spacings (`lighting.wgsl`).
const NORMAL_OFFSET: f32 = 0.3;
/// A probe whose open rays' mean direction is at least this long (a
/// hemisphere's is 0.5) is open to one side; its fill comes from there.
const OPEN_SIDE: f32 = 0.25;
const SH_Y0: f32 = 0.282_095;
const SH_Y1: f32 = 0.488_603;
/// Triangles per BVH leaf.
const LEAF: usize = 4;
/// Light rows are 16 floats (`frame.rs`).
const LIGHT_ROW: usize = 16;

/// What the volume reports through `engine.renderer`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct IndirectLightReadout {
    /// A volume is requested.
    pub enabled: bool,
    /// Probes per axis of the baked volume (0 before the first bake).
    pub dims: [u32; 3],
    pub probes: u32,
    /// Probes inside geometry, filled from their neighbours.
    pub invalid: u32,
    /// Triangles the last bake traced.
    pub triangles: u32,
    /// The last bake's wall time, collection included.
    pub bake_ms: f64,
    /// Bakes uploaded since the renderer was made.
    pub bakes: u32,
    /// A change is waiting for the debounce, or a bake is running.
    pub pending: bool,
    /// GPU bytes the three textures hold.
    pub bytes: u64,
}

/// The volume's grid as the frame uniform carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Grid {
    pub min: Vec3,
    pub spacing: f32,
    pub dims: [u32; 3],
    pub ambient: IndirectAmbient,
}

impl Grid {
    /// The frame uniform's two rows: origin and spacing; dims and mode (0
    /// off, 1 the ambient light is the sky, 2 a floor).
    pub fn uniform(grid: Option<&Grid>) -> [f32; 8] {
        match grid {
            None => [0.0; 8],
            Some(grid) => [
                grid.min.x,
                grid.min.y,
                grid.min.z,
                grid.spacing,
                grid.dims[0] as f32,
                grid.dims[1] as f32,
                grid.dims[2] as f32,
                match grid.ambient {
                    IndirectAmbient::Sky => 1.0,
                    IndirectAmbient::Floor => 2.0,
                },
            ],
        }
    }

    fn of(descriptor: &IndirectLightDescriptor) -> Self {
        let center = Vec3::from(descriptor.center);
        let extent = Vec3::from(descriptor.extent);
        let spacing = descriptor.spacing;
        let dims = [0, 1, 2].map(|axis| ((2.0 * extent[axis] / spacing).floor() as u32 + 1).max(2));
        // Centre the grid on the box, snapped to cell centres so a world
        // aligned to the spacing keeps its probes clear of its walls.
        let min = Vec3::from(dims.map(|d| -((d - 1) as f32) * 0.5 * spacing)) + center;
        Self {
            min,
            spacing,
            dims,
            ambient: descriptor.ambient,
        }
    }

    fn probes(&self) -> usize {
        (self.dims[0] * self.dims[1] * self.dims[2]) as usize
    }

    fn index(&self, [x, y, z]: [u32; 3]) -> usize {
        ((z * self.dims[1] + y) * self.dims[0] + x) as usize
    }

    fn position(&self, [x, y, z]: [u32; 3]) -> Vec3 {
        self.min + Vec3::new(x as f32, y as f32, z as f32) * self.spacing
    }

    /// The box the probes cover plus one cell, the fade's reach.
    fn reach(&self) -> (Vec3, Vec3) {
        let max = self.position([self.dims[0] - 1, self.dims[1] - 1, self.dims[2] - 1]);
        (
            self.min - Vec3::splat(self.spacing),
            max + Vec3::splat(self.spacing),
        )
    }
}

/// A texture's linear colour reduced to a small grid, for the bake's albedo:
/// a material's mean over its whole texture or over its voxel atlas region.
#[derive(Clone, Debug)]
pub(crate) struct Thumb {
    width: u32,
    height: u32,
    side: u32,
    rgb: Vec<[f32; 3]>,
}

impl Thumb {
    pub const SIDE: u32 = 32;

    pub fn of(rgba: &[u8], width: u32, height: u32, srgb: bool) -> Self {
        let decode = |c: u8| {
            let c = f32::from(c) / 255.0;
            if !srgb {
                c
            } else if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let table: Vec<f32> = (0..=255).map(decode).collect();
        let side = Self::SIDE.min(width.max(1)).min(height.max(1)).max(1);
        let mut sum = vec![[0.0_f64; 3]; (side * side) as usize];
        let mut counts = vec![0_u32; (side * side) as usize];
        for y in 0..height {
            let ty = (y * side / height.max(1)).min(side - 1);
            for x in 0..width {
                let tx = (x * side / width.max(1)).min(side - 1);
                let at = ((y * width + x) * 4) as usize;
                let Some(pixel) = rgba.get(at..at + 3) else {
                    continue;
                };
                let cell = (ty * side + tx) as usize;
                for (sum, c) in sum[cell].iter_mut().zip(pixel) {
                    *sum += f64::from(table[*c as usize]);
                }
                counts[cell] += 1;
            }
        }
        let rgb = sum
            .iter()
            .zip(&counts)
            .map(|(sum, count)| sum.map(|s| (s / f64::from((*count).max(1))) as f32))
            .collect();
        Self {
            width,
            height,
            side,
            rgb,
        }
    }

    /// The mean over a rectangle of the texture, in texels.
    pub fn mean(&self, min: [u32; 2], extent: [u32; 2]) -> [f32; 3] {
        let cell = |texel: u32, size: u32| (texel * self.side / size.max(1)).min(self.side - 1);
        let x0 = cell(min[0], self.width);
        let y0 = cell(min[1], self.height);
        let x1 = cell(
            min[0].saturating_add(extent[0]).saturating_sub(1),
            self.width,
        )
        .max(x0);
        let y1 = cell(
            min[1].saturating_add(extent[1]).saturating_sub(1),
            self.height,
        )
        .max(y0);
        let mut sum = [0.0_f64; 3];
        let mut count = 0_u32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let rgb = self.rgb[(y * self.side + x) as usize];
                for (sum, c) in sum.iter_mut().zip(rgb) {
                    *sum += f64::from(c);
                }
                count += 1;
            }
        }
        sum.map(|s| (s / f64::from(count.max(1))) as f32)
    }

    pub fn mean_all(&self) -> [f32; 3] {
        self.mean([0, 0], [self.width, self.height])
    }
}

/// A material's albedo factor: its texture's mean colour, a voxel surface's
/// over its atlas region, or white without a texture.
pub(crate) fn material_mean(
    descriptor: &render_model::RenderMaterialDescriptor,
    thumbs: &std::collections::HashMap<String, Thumb>,
) -> [f32; 3] {
    use render_model::VoxelSurfaceMappingDescriptor as Mapping;
    if let Some(surface) = &descriptor.voxel_surface {
        return match &surface.mapping {
            Mapping::Atlas {
                texture, region, ..
            } => thumbs.get(texture).map_or([1.0; 3], |thumb| {
                thumb.mean(region.content_min, region.content_extent)
            }),
            Mapping::Repeat { texture, .. } => {
                thumbs.get(texture).map_or([1.0; 3], Thumb::mean_all)
            }
        };
    }
    descriptor
        .texture
        .as_ref()
        .and_then(|id| thumbs.get(id))
        .map_or([1.0; 3], Thumb::mean_all)
}

#[derive(Clone, Copy)]
struct Triangle {
    v0: Vec3,
    e1: Vec3,
    e2: Vec3,
    /// Unit geometric normal on the front face.
    normal: Vec3,
    albedo: Vec3,
    emission: Vec3,
    two_sided: bool,
}

#[derive(Clone, Copy)]
struct Node {
    min: Vec3,
    max: Vec3,
    /// A leaf's first triangle, or an interior node's left child (the right
    /// follows it).
    start: u32,
    /// Triangles in a leaf; 0 for an interior node.
    count: u32,
}

struct Bvh {
    nodes: Vec<Node>,
    triangles: Vec<Triangle>,
}

impl Bvh {
    fn build(mut triangles: Vec<Triangle>) -> Self {
        let centroid = |t: &Triangle| t.v0 + (t.e1 + t.e2) / 3.0;
        let mut nodes = vec![Node {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            start: 0,
            count: 0,
        }];
        let mut pending = vec![(0_usize, 0_usize, triangles.len())];
        while let Some((node, start, end)) = pending.pop() {
            let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            let (mut cmin, mut cmax) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for t in &triangles[start..end] {
                for v in [t.v0, t.v0 + t.e1, t.v0 + t.e2] {
                    min = min.min(v);
                    max = max.max(v);
                }
                let c = centroid(t);
                cmin = cmin.min(c);
                cmax = cmax.max(c);
            }
            if start == end {
                (min, max) = (Vec3::ZERO, Vec3::ZERO);
            }
            nodes[node].min = min;
            nodes[node].max = max;
            let extent = cmax - cmin;
            if end - start <= LEAF || extent.max_element() <= 0.0 {
                nodes[node].start = start as u32;
                nodes[node].count = (end - start) as u32;
                continue;
            }
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            let middle = (start + end) / 2;
            triangles[start..end].select_nth_unstable_by(middle - start, |a, b| {
                centroid(a)[axis].total_cmp(&centroid(b)[axis])
            });
            let left = nodes.len();
            nodes.push(nodes[node]);
            nodes.push(nodes[node]);
            nodes[node].start = left as u32;
            nodes[node].count = 0;
            pending.push((left, start, middle));
            pending.push((left + 1, middle, end));
        }
        Self { nodes, triangles }
    }

    /// The nearest hit along the ray before `limit`, or with `any` the first
    /// found: (distance, triangle).
    fn trace(&self, origin: Vec3, direction: Vec3, limit: f32, any: bool) -> Option<(f32, u32)> {
        if self.triangles.is_empty() {
            return None;
        }
        let inverse = direction.recip();
        let mut best: Option<(f32, u32)> = None;
        let mut nearest = limit;
        let mut stack = [0_u32; 64];
        let mut depth = 1;
        while depth > 0 {
            depth -= 1;
            let node = &self.nodes[stack[depth] as usize];
            if slab(node, origin, inverse, nearest).is_none() {
                continue;
            }
            if node.count > 0 {
                for index in node.start..node.start + node.count {
                    let t = &self.triangles[index as usize];
                    if let Some(distance) = intersect(t, origin, direction) {
                        if distance < nearest {
                            nearest = distance;
                            best = Some((distance, index));
                            if any {
                                return best;
                            }
                        }
                    }
                }
                continue;
            }
            let (left, right) = (node.start, node.start + 1);
            let near_left = slab(&self.nodes[left as usize], origin, inverse, nearest);
            let near_right = slab(&self.nodes[right as usize], origin, inverse, nearest);
            // Visit the nearer child first: push it last.
            match (near_left, near_right) {
                (Some(a), Some(b)) => {
                    let (first, second) = if a <= b { (left, right) } else { (right, left) };
                    stack[depth] = second;
                    stack[depth + 1] = first;
                    depth += 2;
                }
                (Some(_), None) => {
                    stack[depth] = left;
                    depth += 1;
                }
                (None, Some(_)) => {
                    stack[depth] = right;
                    depth += 1;
                }
                (None, None) => {}
            }
        }
        best
    }
}

fn slab(node: &Node, origin: Vec3, inverse: Vec3, limit: f32) -> Option<f32> {
    let a = (node.min - origin) * inverse;
    let b = (node.max - origin) * inverse;
    let near = a.min(b).max_element().max(0.0);
    let far = a.max(b).min_element().min(limit);
    (near <= far).then_some(near)
}

/// Möller–Trumbore, both faces.
fn intersect(t: &Triangle, origin: Vec3, direction: Vec3) -> Option<f32> {
    let p = direction.cross(t.e2);
    let determinant = t.e1.dot(p);
    if determinant.abs() < 1e-12 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let s = origin - t.v0;
    let u = s.dot(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(t.e1);
    let v = direction.dot(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = t.e2.dot(q) * inverse;
    (distance > 1e-4).then_some(distance)
}

/// The sky's radiance along a direction: the ambient and hemisphere rows,
/// and the sky light's harmonics as radiance.
#[derive(Clone, Copy)]
struct Sky {
    uniform: Vec3,
    /// Hemisphere: radiance a + b·y.
    a: Vec3,
    b: Vec3,
    /// The sky light's nine irradiance coefficients divided by the band
    /// factors, so their sum is radiance, scaled by its intensity.
    harmonics: Option<[Vec3; 9]>,
}

impl Sky {
    fn radiance(&self, d: Vec3) -> Vec3 {
        let mut radiance = self.uniform + self.a + self.b * d.y;
        if let Some(c) = &self.harmonics {
            radiance += c[0] * 0.282_095
                + c[1] * (0.488_603 * d.y)
                + c[2] * (0.488_603 * d.z)
                + c[3] * (0.488_603 * d.x)
                + c[4] * (1.092_548 * d.x * d.y)
                + c[5] * (1.092_548 * d.y * d.z)
                + c[6] * (0.315_392 * (3.0 * d.z * d.z - 1.0))
                + c[7] * (1.092_548 * d.x * d.z)
                + c[8] * (0.546_274 * (d.x * d.x - d.y * d.y));
        }
        radiance.max(Vec3::ZERO)
    }
}

fn distance_attenuation(distance: f32, range: f32, decay: f32) -> f32 {
    let mut falloff = 1.0 / distance.powf(decay).max(0.01);
    if range > 0.0 {
        let ratio = distance / range;
        let window = (1.0 - ratio.powi(4)).clamp(0.0, 1.0);
        falloff *= window * window;
    }
    falloff
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Direct irradiance at a surface from the directional, point and spot rows,
/// each through a shadow ray.
fn direct(bvh: &Bvh, rows: &[f32], horizon: f32, position: Vec3, normal: Vec3) -> Vec3 {
    let origin = position + normal * SURFACE_OFFSET;
    let mut sum = Vec3::ZERO;
    for row in rows.as_chunks::<LIGHT_ROW>().0 {
        let kind = row[3] as u32;
        let color = Vec3::new(row[0], row[1], row[2]);
        let (direction, attenuation, distance) = match kind {
            2 => (
                -Vec3::new(row[8], row[9], row[10]).normalize_or_zero(),
                1.0,
                horizon,
            ),
            3 | 4 => {
                let to_light = Vec3::new(row[4], row[5], row[6]) - position;
                let distance = to_light.length();
                let range = row[7];
                if range > 0.0 && distance >= range {
                    continue;
                }
                let direction = to_light / distance.max(1e-6);
                let mut attenuation = distance_attenuation(distance, range, row[11]);
                if kind == 4 {
                    let axis = Vec3::new(row[8], row[9], row[10]).normalize_or_zero();
                    attenuation *= smoothstep(row[12], row[13], (-direction).dot(axis));
                }
                (direction, attenuation, distance - SURFACE_OFFSET)
            }
            _ => continue,
        };
        let facing = normal.dot(direction).max(0.0);
        if facing * attenuation <= 0.0 {
            continue;
        }
        if bvh.trace(origin, direction, distance, true).is_some() {
            continue;
        }
        sum += color * attenuation * facing;
    }
    sum
}

fn fibonacci(count: u32) -> Vec<Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    (0..count)
        .map(|i| {
            let y = 1.0 - (i as f32 + 0.5) * 2.0 / count as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let phi = i as f32 * golden;
            Vec3::new(phi.cos() * r, y, phi.sin() * r)
        })
        .collect()
}

/// Project radiance samples over uniform directions to cosine-convolved L1
/// irradiance coefficients (Y0, Y1 by y, z, x).
fn project(directions: &[Vec3], radiance: impl Iterator<Item = Vec3>) -> [Vec3; 4] {
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

/// Irradiance along `normal` from a probe's coefficients.
fn irradiance_along(sh: &[Vec3; 4], normal: Vec3) -> Vec3 {
    (sh[0] * SH_Y0
        + sh[1] * (SH_Y1 * normal.y)
        + sh[2] * (SH_Y1 * normal.z)
        + sh[3] * (SH_Y1 * normal.x))
        .max(Vec3::ZERO)
}

/// The baked probes of one pass, read by the next pass's bounce.
struct Field {
    grid: Grid,
    sh: Vec<[Vec3; 4]>,
    valid: Vec<bool>,
}

impl Field {
    /// Trilinear irradiance at a surface, from the valid probes around it;
    /// none outside the grid or among invalid probes only.
    fn irradiance(&self, position: Vec3, normal: Vec3) -> Option<Vec3> {
        let grid = &self.grid;
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
            let index = grid.index(at);
            if !self.valid[index] {
                continue;
            }
            let weight = (0..3)
                .map(|axis| {
                    if offset[axis] == 1 {
                        t[axis]
                    } else {
                        1.0 - t[axis]
                    }
                })
                .product::<f32>();
            for (sum, coefficient) in sum.iter_mut().zip(self.sh[index]) {
                *sum += coefficient * weight;
            }
            total += weight;
        }
        (total > 1e-4).then(|| irradiance_along(&sum.map(|c| c / total), normal))
    }
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

/// Everything a bake needs, collected on the render thread.
pub(crate) struct BakeJob {
    /// The renderer's scene generation the job reads.
    generation: u64,
    grid: Grid,
    bounces: u32,
    triangles: Vec<Triangle>,
    /// The world's light rows (`frame.rs` `light_row`).
    rows: Vec<f32>,
    sky: Sky,
    collect_ms: f64,
}

/// A finished bake: the three textures' texels, RGBA16F, and its facts.
pub(crate) struct Baked {
    grid: Grid,
    /// The texture's texels: red's slab of probes, then green's, then blue's,
    /// each probe its four coefficients as half floats.
    texels: Vec<u16>,
    invalid: u32,
    triangles: u32,
    ms: f64,
}

/// Run `work` over every item on the worker threads, a few at a time.
fn parallel<T: Send>(items: &mut [T], work: impl Fn(&mut T) + Sync) {
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
fn half_bits(value: f32) -> u16 {
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

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

/// Bake the job: trace every probe, relax the bounces, fill probes inside
/// geometry from their neighbours, and pack the textures.
pub(crate) fn bake(job: BakeJob) -> Baked {
    let started = Instant::now();
    let BakeJob {
        generation: _,
        grid,
        bounces,
        triangles,
        rows,
        sky,
        collect_ms,
    } = job;
    let triangle_count = triangles.len() as u32;
    let bvh = Bvh::build(triangles);
    let horizon = (bvh.nodes[0].max - bvh.nodes[0].min).length() * 2.0 + 1.0;
    let directions = fibonacci(RAYS);
    let pi = std::f32::consts::PI;
    let probes = grid.probes();
    let cells: Vec<[u32; 3]> = (0..grid.dims[2])
        .flat_map(|z| {
            (0..grid.dims[1]).flat_map(move |y| (0..grid.dims[0]).map(move |x| [x, y, z]))
        })
        .collect();
    let miss = RayRecord {
        fixed: Vec3::ZERO,
        position: Vec3::ZERO,
        normal: Vec3::ZERO,
        reflectance: Vec3::ZERO,
        kind: RayKind::Miss,
    };
    let mut records = vec![miss; probes * RAYS as usize];
    let mut field = Field {
        grid,
        sh: vec![[Vec3::ZERO; 4]; probes],
        valid: vec![true; probes],
    };
    // The mean direction of each probe's rays that did not hit a back face:
    // the side a probe on a surface is open to. And the direction of its
    // nearest back-face hit: the way out for a probe inside geometry.
    let mut opens = vec![Vec3::ZERO; probes];
    let mut exits = vec![Vec3::ZERO; probes];
    for pass in 0..bounces.max(1) {
        let previous = Field {
            grid,
            sh: field.sh.clone(),
            valid: field.valid.clone(),
        };
        let mut items: Vec<_> = records
            .chunks_mut(RAYS as usize)
            .zip(&mut field.sh)
            .zip(&mut field.valid)
            .zip(&mut opens)
            .zip(&mut exits)
            .zip(&cells)
            .collect();
        parallel(
            &mut items,
            |(((((records, sh), valid), open), exit), cell)| {
                let origin = grid.position(**cell);
                if pass == 0 {
                    let mut backs = 0;
                    let mut open_sum = Vec3::ZERO;
                    let mut nearest_back = f32::MAX;
                    for (record, direction) in records.iter_mut().zip(&directions) {
                        *record = match bvh.trace(origin, *direction, f32::MAX, false) {
                            None => {
                                open_sum += *direction;
                                RayRecord {
                                    fixed: sky.radiance(*direction),
                                    ..miss
                                }
                            }
                            Some((distance, triangle)) => {
                                let t = &bvh.triangles[triangle as usize];
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
                                                * direct(&bvh, &rows, horizon, position, normal),
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
                                * previous
                                    .irradiance(record.position, record.normal)
                                    .unwrap_or(Vec3::ZERO)
                    }
                });
                **sh = project(&directions, radiance);
            },
        );
    }
    let invalid = field.valid.iter().filter(|valid| !**valid).count() as u32;
    dilate(&mut field, &opens, &exits);
    let mut texels = Vec::with_capacity(probes * 12);
    for channel in 0..3 {
        for sh in &field.sh {
            for coefficient in sh {
                texels.push(half_bits(coefficient[channel]));
            }
        }
    }
    Baked {
        grid,
        texels,
        invalid,
        triangles: triangle_count,
        ms: collect_ms + ms(started),
    }
}

/// Fill probes inside geometry from their valid face neighbours, so a
/// trilinear sample at a wall never reads a dark hole. A probe on a surface
/// (half its rays into the wall behind it) takes the neighbours on the side
/// its rays found open; one inside a wall takes the side its nearest way
/// out faces. So the room side of a thin wall does not read the daylight
/// beyond it. A probe with neither takes every valid neighbour. Repeated
/// until every probe has a value or nothing is left to fill from.
fn dilate(field: &mut Field, opens: &[Vec3], exits: &[Vec3]) {
    let grid = field.grid;
    let dims = grid.dims;
    for _ in 0..16 {
        let mut filled = Vec::new();
        for z in 0..dims[2] {
            for y in 0..dims[1] {
                for x in 0..dims[0] {
                    let index = grid.index([x, y, z]);
                    if field.valid[index] {
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
                    let valid: Vec<(usize, Vec3)> = neighbours
                        .into_iter()
                        .filter_map(|(nx, ny, nz, toward)| {
                            let at = grid.index([nx?, ny?, nz?]);
                            field.valid[at].then_some((at, toward))
                        })
                        .collect();
                    let open = opens[index];
                    let side = if open.length() > OPEN_SIDE {
                        Some(open)
                    } else {
                        (exits[index] != Vec3::ZERO).then_some(exits[index])
                    };
                    let open_side: Vec<usize> = match side {
                        Some(side) => valid
                            .iter()
                            .filter(|(_, toward)| toward.dot(side) > 0.0)
                            .map(|(at, _)| *at)
                            .collect(),
                        None => Vec::new(),
                    };
                    let from: Vec<usize> = if open_side.is_empty() {
                        valid.iter().map(|(at, _)| *at).collect()
                    } else {
                        open_side
                    };
                    if from.is_empty() {
                        continue;
                    }
                    let mut sum = [Vec3::ZERO; 4];
                    for at in &from {
                        for (sum, c) in sum.iter_mut().zip(field.sh[*at]) {
                            *sum += c;
                        }
                    }
                    filled.push((index, sum.map(|c| c / from.len() as f32)));
                }
            }
        }
        if filled.is_empty() {
            break;
        }
        for (index, sh) in filled {
            field.sh[index] = sh;
            field.valid[index] = true;
        }
    }
}

/// When the next bake starts.
#[derive(Default)]
struct Schedule {
    /// When changes inside the box were first and last seen since the last
    /// bake started.
    dirty: Option<(Instant, Instant)>,
}

impl Schedule {
    fn touch(&mut self, now: Instant) {
        let first = self.dirty.map_or(now, |(first, _)| first);
        self.dirty = Some((first, now));
    }

    /// The scene has been still for `DEBOUNCE`, or has kept changing for
    /// `MAX_WAIT`.
    fn due(&self, now: Instant) -> bool {
        self.dirty.is_some_and(|(first, last)| {
            now.duration_since(last) >= DEBOUNCE || now.duration_since(first) >= MAX_WAIT
        })
    }
}

/// The volume on the GPU, and the bake that keeps it current.
pub(crate) struct ProbeVolume {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// The grid the textures hold; `None` while off or before the first bake.
    pub grid: Option<Grid>,
    schedule: Schedule,
    /// The scene generation the last bake started from.
    baked_generation: u64,
    /// A bake running on its worker thread.
    job: Option<Receiver<Baked>>,
    readout: IndirectLightReadout,
}

impl ProbeVolume {
    pub fn new(gpu: &Gpu) -> Self {
        let (texture, view) = texture(gpu, [1, 1, 1]);
        Self {
            texture,
            view,
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu probes"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            grid: None,
            schedule: Schedule::default(),
            baked_generation: 0,
            job: None,
            readout: IndirectLightReadout::default(),
        }
    }

    pub fn readout(&self) -> IndirectLightReadout {
        IndirectLightReadout {
            pending: self.schedule.dirty.is_some() || self.job.is_some(),
            ..self.readout
        }
    }

    /// A change inside the volume (or to the request) was seen now.
    pub fn touch(&mut self, now: Instant) {
        self.schedule.touch(now);
        self.readout.enabled = true;
    }

    /// Whether a bake should start now and none is running.
    pub fn due(&self, now: Instant) -> bool {
        self.job.is_none() && self.schedule.due(now)
    }

    /// Whether the scene has changed since the last bake started.
    pub fn stale(&self, generation: u64) -> bool {
        self.baked_generation != generation
    }

    /// Start `job` on a worker thread.
    pub fn start(&mut self, job: BakeJob) {
        self.schedule.dirty = None;
        self.baked_generation = job.generation;
        let (sender, receiver) = mpsc::channel();
        self.job = Some(receiver);
        let spawned = std::thread::Builder::new()
            .name("rusty-probe-bake".to_owned())
            .spawn(move || {
                let _ = sender.send(bake(job));
            });
        if spawned.is_err() {
            self.job = None;
        }
    }

    /// Take a finished bake and upload it. Returns whether the frame bind
    /// group must be rebuilt (the textures changed).
    pub fn poll(&mut self, gpu: &Gpu) -> bool {
        let baked = match self.job.as_ref().map(Receiver::try_recv) {
            Some(Ok(baked)) => baked,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.job = None;
                return false;
            }
            _ => return false,
        };
        self.job = None;
        self.upload(gpu, baked)
    }

    /// Bake inline, on this thread's workers, and upload: for tools and
    /// tests that want the volume before the next frame.
    pub fn bake_now(&mut self, gpu: &Gpu, job: BakeJob) -> bool {
        self.schedule.dirty = None;
        self.baked_generation = job.generation;
        self.job = None;
        let baked = bake(job);
        self.upload(gpu, baked)
    }

    fn upload(&mut self, gpu: &Gpu, baked: Baked) -> bool {
        let dims = baked.grid.dims;
        let mut rebound = false;
        if self.grid.is_none_or(|grid| grid.dims != dims) {
            let (texture, view) = texture(gpu, dims);
            self.texture = texture;
            self.view = view;
            rebound = true;
        }
        // The channels follow each other along the depth: red's slab, then
        // green's, then blue's.
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&baked.texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(dims[0] * 8),
                rows_per_image: Some(dims[1]),
            },
            wgpu::Extent3d {
                width: dims[0],
                height: dims[1],
                depth_or_array_layers: dims[2] * 3,
            },
        );
        self.grid = Some(baked.grid);
        self.readout = IndirectLightReadout {
            enabled: true,
            dims,
            probes: baked.grid.probes() as u32,
            invalid: baked.invalid,
            triangles: baked.triangles,
            bake_ms: baked.ms,
            bakes: self.readout.bakes + 1,
            pending: false,
            bytes: u64::from(dims[0]) * u64::from(dims[1]) * u64::from(dims[2]) * 8 * 3,
        };
        rebound
    }

    /// Drop the volume (the request went away). Returns whether the frame
    /// bind group must be rebuilt.
    pub fn clear(&mut self, gpu: &Gpu) -> bool {
        self.schedule.dirty = None;
        self.job = None;
        let had = self.grid.take().is_some();
        if had {
            let (texture, view) = texture(gpu, [1, 1, 1]);
            self.texture = texture;
            self.view = view;
        }
        self.readout = IndirectLightReadout {
            bakes: self.readout.bakes,
            ..IndirectLightReadout::default()
        };
        had
    }
}

/// One RGBA16F 3D texture of `dims` probes, the three colour channels
/// stacked along its depth.
fn texture(gpu: &Gpu, dims: [u32; 3]) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu probes"),
            size: wgpu::Extent3d {
                width: dims[0],
                height: dims[1],
                depth_or_array_layers: dims[2] * 3,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    };
    let view = texture.create_view(&Default::default());
    (texture, view)
}

impl Renderer {
    /// Whether a part's bounds reach into the requested volume (with its
    /// fade cell), so a change to it is a change to the light inside.
    pub(crate) fn probe_box_touches(&self, bounds: &crate::tables::Aabb) -> bool {
        let Some(descriptor) = &self.tables.indirect_light else {
            return false;
        };
        let (min, max) = Grid::of(descriptor).reach();
        !bounds.is_empty() && bounds.min.cmple(max).all() && bounds.max.cmpge(min).all()
    }

    /// Everything a bake of the requested volume needs, from the retained
    /// scene as it stands.
    pub(crate) fn probe_bake_job(&self, descriptor: &IndirectLightDescriptor) -> BakeJob {
        let started = Instant::now();
        let grid = Grid::of(descriptor);
        // Triangles well beyond the box still cast shadows and bounce light
        // into it; take everything, the BVH keeps the far ones cheap.
        let triangles = self.probe_triangles();
        let mut rows: Vec<f32> = Vec::new();
        self.world_light_rows(&mut rows);
        let pi = std::f32::consts::PI;
        let mut sky = Sky {
            uniform: Vec3::ZERO,
            a: Vec3::ZERO,
            b: Vec3::ZERO,
            harmonics: None,
        };
        for row in rows.as_chunks::<LIGHT_ROW>().0 {
            let color = Vec3::new(row[0], row[1], row[2]);
            match row[3] as u32 {
                // Uniform radiance L gives irradiance πL; a floor keeps its
                // ambient rows in the shader instead.
                0 if descriptor.ambient == IndirectAmbient::Sky => sky.uniform += color / pi,
                1 => {
                    // E(n) = mix(ground, sky, n.y/2 + 1/2) from L = a + b·y.
                    let ground = Vec3::new(row[12], row[13], row[14]);
                    sky.a += (ground + color) / (2.0 * pi);
                    sky.b += (color - ground) * (3.0 / (4.0 * pi));
                }
                _ => {}
            }
        }
        if let Some(sky_light) = self.tables.sky_light.filter(|light| light.intensity > 0.0) {
            if let Some(coefficients) = self.sky_light.read_irradiance(&self.gpu) {
                // Irradiance coefficients become radiance through the band
                // factors (π, 2π/3, π/4), scaled by the light's intensity.
                let bands = [
                    pi,
                    2.0 * pi / 3.0,
                    2.0 * pi / 3.0,
                    2.0 * pi / 3.0,
                    pi / 4.0,
                    pi / 4.0,
                    pi / 4.0,
                    pi / 4.0,
                    pi / 4.0,
                ];
                let mut harmonics = [Vec3::ZERO; 9];
                for (index, c) in coefficients.iter().enumerate() {
                    harmonics[index] =
                        Vec3::new(c[0], c[1], c[2]) * (sky_light.intensity / bands[index]);
                }
                sky.harmonics = Some(harmonics);
            }
        }
        BakeJob {
            generation: self.scene_generation,
            grid,
            bounces: descriptor.bounces.max(1),
            triangles,
            rows,
            sky,
            collect_ms: ms(started),
        }
    }

    /// Every shown, opaque, triangle part of the world in world space, with
    /// its albedo (its colour times its texture's mean) and emission.
    fn probe_triangles(&self) -> Vec<Triangle> {
        let parts = &self.tables.parts;
        let mut triangles = Vec::new();
        for (id, part) in parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let state = &parts.state[id];
            if !state.shown
                || state.layer != RenderLayer::Scene
                || state.class.blend
                || state.class.lines
                || part.wireframe
            {
                continue;
            }
            let Some(mesh) = self.mesh(&part.mesh) else {
                continue;
            };
            if mesh.topology != Topology::Triangles {
                continue;
            }
            let row = crate::tables::PART_ROW_FLOATS * id;
            let gpu = &parts.gpu[row..row + crate::tables::PART_ROW_FLOATS];
            let model = Mat4::from_cols_slice(&gpu[..16]);
            let color = Vec3::new(gpu[28], gpu[29], gpu[30]);
            let emission = Vec3::new(gpu[32], gpu[33], gpu[34]);
            let (albedo, emission) = match &part.material {
                MaterialRef::Unlit => (Vec3::ZERO, color),
                MaterialRef::LitFallback => (color, emission),
                MaterialRef::Retained(material) => {
                    let mean = self
                        .tables
                        .materials
                        .get(*material)
                        .and_then(|row| self.tables.material_means.get(&row.descriptor.id))
                        .copied()
                        .unwrap_or([1.0; 3]);
                    (color * Vec3::from(mean), emission)
                }
            };
            let cpu = &mesh.cpu;
            let first = part.first_index as usize;
            let end = (first + part.index_count as usize).min(cpu.indices.len());
            for corner in cpu.indices[first..end].as_chunks::<3>().0 {
                let [a, b, c] =
                    [0, 1, 2].map(|k| model.transform_point3(cpu.positions[corner[k] as usize]));
                // Mirrored parts wind the other way.
                let (b, c) = if state.mirrored { (c, b) } else { (b, c) };
                let e1 = b - a;
                let e2 = c - a;
                let normal = e1.cross(e2);
                if normal.length_squared() <= 0.0 {
                    continue;
                }
                triangles.push(Triangle {
                    v0: a,
                    e1,
                    e2,
                    normal: normal.normalize(),
                    albedo: albedo.min(Vec3::ONE),
                    emission,
                    two_sided: state.class.double_sided,
                });
            }
        }
        triangles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(extent: [f32; 3], spacing: f32) -> IndirectLightDescriptor {
        IndirectLightDescriptor {
            center: [10.0, 2.0, -4.0],
            extent,
            spacing,
            bounces: 1,
            ambient: IndirectAmbient::Sky,
        }
    }

    #[test]
    fn the_grid_centres_its_probes_on_the_box() {
        let grid = Grid::of(&descriptor([3.5, 1.5, 3.5], 1.0));
        assert_eq!(grid.dims, [8, 4, 8]);
        assert_eq!(grid.min, Vec3::new(6.5, 0.5, -7.5));
        assert_eq!(grid.position([7, 3, 7]), Vec3::new(13.5, 3.5, -0.5));
        let (min, max) = grid.reach();
        assert_eq!(
            (min, max),
            (Vec3::new(5.5, -0.5, -8.5), Vec3::new(14.5, 4.5, 0.5))
        );
        let uniform = Grid::uniform(Some(&grid));
        assert_eq!(&uniform[..4], &[6.5, 0.5, -7.5, 1.0]);
        assert_eq!(&uniform[4..], &[8.0, 4.0, 8.0, 1.0]);
        assert_eq!(Grid::uniform(None), [0.0; 8]);
    }

    #[test]
    fn a_bake_is_due_after_the_scene_is_still_or_has_kept_moving_long_enough() {
        let mut volume = Schedule::default();
        let start = Instant::now();
        assert!(!volume.due(start));
        volume.touch(start);
        assert!(!volume.due(start + DEBOUNCE / 2));
        assert!(volume.due(start + DEBOUNCE));
        // Every frame changes something: the first change's age decides.
        let mut now = start;
        while now < start + MAX_WAIT - DEBOUNCE / 2 {
            now += DEBOUNCE / 2;
            volume.touch(now);
            assert!(!volume.due(now), "{:?}", now - start);
        }
        assert!(volume.due(start + MAX_WAIT));
    }

    #[test]
    fn irradiance_projects_a_uniform_sky_to_its_irradiance_along_every_normal() {
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
        let grid = Grid::of(&descriptor([1.0, 0.0, 0.0], 1.0));
        assert_eq!(grid.dims, [3, 2, 2]);
        let bright = [Vec3::splat(4.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO];
        let dark = [Vec3::splat(0.1), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO];
        let probes = grid.probes();
        let mut field = Field {
            grid,
            sh: vec![dark; probes],
            valid: vec![true; probes],
        };
        let mut opens = vec![Vec3::ZERO; probes];
        // Along x: a bright outdoor probe, one on the wall between, a dark
        // room probe; the wall probe's rays were open toward the room.
        for (y, z) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            field.sh[grid.index([0, y, z])] = bright;
            field.valid[grid.index([1, y, z])] = false;
            opens[grid.index([1, y, z])] = Vec3::X * 0.5;
        }
        dilate(&mut field, &opens, &vec![Vec3::ZERO; probes]);
        for (y, z) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let at = grid.index([1, y, z]);
            assert!(field.valid[at]);
            assert_eq!(field.sh[at], dark, "the room side fills the wall probe");
        }
        // A probe inside a wall takes the side its nearest way out faces.
        let mut field = Field {
            grid,
            sh: vec![dark; probes],
            valid: vec![true; probes],
        };
        field.sh[grid.index([0, 0, 0])] = bright;
        field.valid[grid.index([1, 0, 0])] = false;
        let mut exits = vec![Vec3::ZERO; probes];
        exits[grid.index([1, 0, 0])] = Vec3::X;
        dilate(&mut field, &vec![Vec3::ZERO; probes], &exits);
        assert_eq!(field.sh[grid.index([1, 0, 0])], dark);
        // One with neither averages its neighbours.
        field.valid[grid.index([1, 0, 0])] = false;
        dilate(
            &mut field,
            &vec![Vec3::ZERO; probes],
            &vec![Vec3::ZERO; probes],
        );
        let filled = field.sh[grid.index([1, 0, 0])][0];
        assert!(filled.x > dark[0].x && filled.x < bright[0].x, "{filled}");
    }

    #[test]
    fn a_thumbnail_means_a_region_of_the_texture() {
        let mut rgba = vec![0_u8; 64 * 64 * 4];
        for y in 0..64 {
            for x in 0..64 {
                let at = (y * 64 + x) * 4;
                rgba[at] = if x < 32 { 255 } else { 0 };
                rgba[at + 1] = if y < 32 { 255 } else { 0 };
                rgba[at + 3] = 255;
            }
        }
        let thumb = Thumb::of(&rgba, 64, 64, false);
        let left_top = thumb.mean([0, 0], [32, 32]);
        assert_eq!(left_top, [1.0, 1.0, 0.0]);
        let right_bottom = thumb.mean([32, 32], [32, 32]);
        assert_eq!(right_bottom, [0.0, 0.0, 0.0]);
        let all = thumb.mean_all();
        assert!((all[0] - 0.5).abs() < 1e-5 && (all[1] - 0.5).abs() < 1e-5);
    }
}
