//! Ray tracing for the probe bake: triangles with their albedo and
//! emission, a BVH for each brick's triangles and one for the world beyond
//! the bricks, walked through the brick grid; the sky a missed ray sees; and
//! the direct light a hit receives through shadow rays.

use std::sync::Arc;

use glam::Vec3;

use super::{Bricks, BRICK, LEAF, LIGHT_ROW, SURFACE_OFFSET};

#[derive(Clone, Copy)]
pub(super) struct Triangle {
    pub v0: Vec3,
    pub e1: Vec3,
    pub e2: Vec3,
    /// Unit geometric normal on the front face.
    pub normal: Vec3,
    pub albedo: Vec3,
    pub emission: Vec3,
    pub two_sided: bool,
}

impl Triangle {
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (b, c) = (self.v0 + self.e1, self.v0 + self.e2);
        (self.v0.min(b).min(c), self.v0.max(b).max(c))
    }

    /// Whether the triangle's box overlaps `min..=max`.
    pub fn touches(&self, min: Vec3, max: Vec3) -> bool {
        let (lo, hi) = self.bounds();
        lo.cmple(max).all() && hi.cmpge(min).all()
    }
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

/// A bounding volume hierarchy over triangles: a brick's, or the world
/// beyond the bricks.
pub(super) struct Bvh {
    nodes: Vec<Node>,
    triangles: Vec<Triangle>,
}

impl Bvh {
    pub fn build(mut triangles: Vec<Triangle>) -> Self {
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
                let (lo, hi) = t.bounds();
                min = min.min(lo);
                max = max.max(hi);
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

    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        (self.nodes[0].min, self.nodes[0].max)
    }

    /// The nearest hit along the ray before `limit`, or with `any` the first
    /// found: (distance, triangle).
    pub fn trace(
        &self,
        origin: Vec3,
        direction: Vec3,
        limit: f32,
        any: bool,
    ) -> Option<(f32, &Triangle)> {
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
                                return Some((distance, t));
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
        best.map(|(distance, index)| (distance, &self.triangles[index as usize]))
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

/// What a batch traces: each brick's BVH, found by walking the ray through
/// the brick grid, and the world beyond the bricks.
pub(super) struct Scene {
    pub bricks: Bricks,
    /// Each brick's BVH; `None` for a brick without triangles.
    pub blas: Vec<Option<Arc<Bvh>>>,
    pub outside: Option<Arc<Bvh>>,
}

impl Scene {
    /// A ray can leave everything behind within this distance.
    pub fn horizon(&self) -> f32 {
        let (mut min, mut max) = self.bricks.world_bounds();
        if let Some((lo, hi)) = self.outside.as_ref().map(|b| b.bounds()) {
            min = min.min(lo);
            max = max.max(hi);
        }
        (max - min).length() * 2.0 + 1.0
    }

    /// The nearest hit along the ray before `limit`, or with `any` one hit.
    pub fn trace(
        &self,
        origin: Vec3,
        direction: Vec3,
        limit: f32,
        any: bool,
    ) -> Option<(f32, &Triangle)> {
        let mut nearest = limit;
        let mut best: Option<&Triangle> = None;
        // Walk the brick grid cell by cell along the ray (Amanatides–Woo);
        // a triangle in a cell's BVH may lie beyond that cell, so stop only
        // once the next cell is entered after the nearest hit so far.
        let bricks = &self.bricks;
        let grid_min = Vec3::new(
            bricks.b0[0] as f32,
            bricks.b0[1] as f32,
            bricks.b0[2] as f32,
        ) * BRICK;
        let grid_max = grid_min
            + Vec3::new(
                bricks.nb[0] as f32,
                bricks.nb[1] as f32,
                bricks.nb[2] as f32,
            ) * BRICK;
        let inverse = direction.recip();
        let a = (grid_min - origin) * inverse;
        let b = (grid_max - origin) * inverse;
        let enter = a.min(b).max_element().max(0.0);
        let exit = a.max(b).min_element();
        if enter <= exit && enter < nearest {
            let start = origin + direction * enter;
            let mut cell = ((start - grid_min) / BRICK).floor().as_ivec3();
            let last = glam::IVec3::new(
                bricks.nb[0] as i32 - 1,
                bricks.nb[1] as i32 - 1,
                bricks.nb[2] as i32 - 1,
            );
            cell = cell.clamp(glam::IVec3::ZERO, last);
            let step = glam::IVec3::new(
                if direction.x >= 0.0 { 1 } else { -1 },
                if direction.y >= 0.0 { 1 } else { -1 },
                if direction.z >= 0.0 { 1 } else { -1 },
            );
            let next_boundary = |cell: glam::IVec3| {
                let corner = cell
                    + glam::IVec3::new(
                        (step.x > 0) as i32,
                        (step.y > 0) as i32,
                        (step.z > 0) as i32,
                    );
                grid_min + corner.as_vec3() * BRICK
            };
            let mut t_max = (next_boundary(cell) - origin) * inverse;
            let t_delta = (Vec3::splat(BRICK) * inverse).abs();
            loop {
                if let Some(blas) = &self.blas[bricks.index_of(cell)] {
                    let hit = blas.trace(origin, direction, nearest, any);
                    nearer(&mut best, &mut nearest, hit);
                    if any && best.is_some() {
                        return best.map(|t| (nearest, t));
                    }
                }
                let next = t_max.min_element();
                if next >= nearest {
                    break;
                }
                if t_max.x <= t_max.y && t_max.x <= t_max.z {
                    cell.x += step.x;
                    t_max.x += t_delta.x;
                } else if t_max.y <= t_max.z {
                    cell.y += step.y;
                    t_max.y += t_delta.y;
                } else {
                    cell.z += step.z;
                    t_max.z += t_delta.z;
                }
                if cell.cmplt(glam::IVec3::ZERO).any() || cell.cmpgt(last).any() {
                    break;
                }
            }
        }
        if let Some(outside) = &self.outside {
            let hit = outside.trace(origin, direction, nearest, any);
            nearer(&mut best, &mut nearest, hit);
        }
        best.map(|t| (nearest, t))
    }
}

/// Keep `hit` when it is nearer than the nearest so far.
fn nearer<'a>(
    best: &mut Option<&'a Triangle>,
    nearest: &mut f32,
    hit: Option<(f32, &'a Triangle)>,
) {
    if let Some((distance, t)) = hit {
        if distance < *nearest {
            *nearest = distance;
            *best = Some(t);
        }
    }
}

/// The sky's radiance along a direction: the ambient and hemisphere rows,
/// and the sky light's harmonics as radiance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Sky {
    pub uniform: Vec3,
    /// Hemisphere: radiance a + b·y.
    pub a: Vec3,
    pub b: Vec3,
    /// The sky light's nine irradiance coefficients divided by the band
    /// factors, so their sum is radiance, scaled by its intensity.
    pub harmonics: Option<[Vec3; 9]>,
}

impl Sky {
    pub const NONE: Self = Self {
        uniform: Vec3::ZERO,
        a: Vec3::ZERO,
        b: Vec3::ZERO,
        harmonics: None,
    };

    pub fn radiance(&self, d: Vec3) -> Vec3 {
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
pub(super) fn direct(
    scene: &Scene,
    rows: &[f32],
    horizon: f32,
    position: Vec3,
    normal: Vec3,
) -> Vec3 {
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
        if scene.trace(origin, direction, distance, true).is_some() {
            continue;
        }
        sum += color * attenuation * facing;
    }
    sum
}

/// `count` directions spread evenly over the sphere.
pub(super) fn fibonacci(count: u32) -> Vec<Vec3> {
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
