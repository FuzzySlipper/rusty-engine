//! Bounded implicit geometry, independent of product style and renderer ownership.
//!
//! Shapes are negative inside. CSG preserves a zero surface, not necessarily
//! Euclidean distance. An arena owns shape expressions; nodes are local to it.
//! Fidget supplies evaluation, interval pruning and adaptive dual-cell
//! connectivity. Engine triangulates the resulting cell-vertex polygons.

use fidget::{
    context::Tree,
    jit::{JitBulkFn, JitShape},
    mesh::{Octree, Settings},
    shape::{EzShape, ShapeTape},
};
use nalgebra::{Matrix4, Vector3};
use std::{sync::Mutex, time::Instant};

pub mod surface;
mod triangulate;
pub mod volume;

pub type Node = u32;

#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    pub fn validate(self) -> Result<Self, Error> {
        if (0..3).any(|i| {
            !self.min[i].is_finite()
                || !self.max[i].is_finite()
                || self.max[i] <= self.min[i]
                || !(self.max[i] - self.min[i]).is_finite()
        }) {
            return Err(Error("bounds must have finite positive extents".into()));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WaveDisplacement {
    pub frequency: [f32; 3],
    pub amplitude: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub gain: f32,
    pub seed: u64,
}

/// No geometric or aesthetic policy is hidden in the node arena.
#[derive(Default)]
pub struct Field {
    nodes: Vec<Tree>,
    // One immutable tape avoids recompiling the same expression for scalar
    // probes. Bounded to one entry; clones own independent cache lifetimes.
    sample_tape: Mutex<Option<(Node, ShapeTape<JitBulkFn<f32>>)>>,
}

impl Clone for Field {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            sample_tape: Mutex::new(None),
        }
    }
}

impl Field {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn tree(&self, node: Node) -> Result<Tree, Error> {
        self.nodes
            .get(node as usize)
            .cloned()
            .ok_or_else(|| Error("unknown field node".into()))
    }

    fn push(&mut self, tree: Tree) -> Result<Node, Error> {
        let id = u32::try_from(self.nodes.len())
            .map_err(|_| Error("field node capacity exceeded".into()))?;
        self.nodes.push(tree);
        Ok(id)
    }

    pub fn box_shape(&mut self, bounds: Bounds) -> Result<Node, Error> {
        bounds.validate()?;
        let x = Tree::x();
        let y = Tree::y();
        let z = Tree::z();
        self.push(
            (bounds.min[0] - x.clone())
                .max(x.clone() - bounds.max[0])
                .max((bounds.min[1] - y.clone()).max(y.clone() - bounds.max[1]))
                .max((bounds.min[2] - z.clone()).max(z.clone() - bounds.max[2])),
        )
    }

    pub fn sphere(&mut self, center: [f32; 3], radius: f32) -> Result<Node, Error> {
        finite(&center)?;
        positive(radius, "radius")?;
        let p = coordinates(center);
        self.push((p[0].square() + p[1].square() + p[2].square()).sqrt() - radius)
    }

    pub fn ellipsoid(&mut self, center: [f32; 3], radii: [f32; 3]) -> Result<Node, Error> {
        finite(&center)?;
        for r in radii {
            positive(r, "ellipsoid radius")?;
        }
        let p = coordinates(center);
        self.push(
            ((p[0].clone() / radii[0]).square()
                + (p[1].clone() / radii[1]).square()
                + (p[2].clone() / radii[2]).square())
            .sqrt()
                - 1.0,
        )
    }

    /// A half-space: dot(normal, position) - offset <= 0. Normal is normalized
    /// along with offset, preserving the selected plane.
    pub fn plane(&mut self, normal: [f32; 3], offset: f32) -> Result<Node, Error> {
        finite(&normal)?;
        finite(&[offset])?;
        let n = Vector3::from(normal);
        let len = n.norm();
        positive(len, "plane normal length")?;
        let n = n / len;
        self.push(Tree::x() * n.x + Tree::y() * n.y + Tree::z() * n.z - offset / len)
    }

    pub fn capsule(&mut self, start: [f32; 3], end: [f32; 3], radius: f32) -> Result<Node, Error> {
        finite(&start)?;
        finite(&end)?;
        positive(radius, "radius")?;
        let d = Vector3::from(end) - Vector3::from(start);
        let length2 = d.norm_squared();
        if length2 == 0.0 {
            return self.sphere(start, radius);
        }
        positive(length2, "capsule length squared")?;
        let p = coordinates(start);
        let h = ((p[0].clone() * d.x + p[1].clone() * d.y + p[2].clone() * d.z) / length2)
            .max(0.0)
            .min(1.0);
        self.push(
            ((p[0].clone() - h.clone() * d.x).square()
                + (p[1].clone() - h.clone() * d.y).square()
                + (p[2].clone() - h.clone() * d.z).square())
            .sqrt()
                - radius,
        )
    }

    /// Capped circular taper along an arbitrary axis. Equal radii give a
    /// cylinder; one zero radius gives a cone. This preserves the selected
    /// zero surface, but is not generally Euclidean signed distance.
    pub fn frustum(
        &mut self,
        start: [f32; 3],
        end: [f32; 3],
        start_radius: f32,
        end_radius: f32,
    ) -> Result<Node, Error> {
        finite(&start)?;
        finite(&end)?;
        finite(&[start_radius, end_radius])?;
        if start_radius < 0.0 || end_radius < 0.0 || start_radius.max(end_radius) == 0.0 {
            return Err(Error(
                "frustum radii must be non-negative and at least one positive".into(),
            ));
        }
        let axis = Vector3::from(end) - Vector3::from(start);
        let length = axis.norm();
        positive(length, "frustum axis length")?;
        let axis = axis / length;
        let slope = (end_radius - start_radius) / length;
        finite(&[slope])?;
        let p = coordinates(start);
        let along = p[0].clone() * axis.x + p[1].clone() * axis.y + p[2].clone() * axis.z;
        let radial = ((p[0].clone() - along.clone() * axis.x).square()
            + (p[1].clone() - along.clone() * axis.y).square()
            + (p[2].clone() - along.clone() * axis.z).square())
        .sqrt();
        let side = radial - (start_radius + along.clone() * slope);
        self.push(side.max(-along.clone()).max(along - length))
    }

    pub fn union(&mut self, a: Node, b: Node) -> Result<Node, Error> {
        self.push(self.tree(a)?.min(self.tree(b)?))
    }
    pub fn intersection(&mut self, a: Node, b: Node) -> Result<Node, Error> {
        self.push(self.tree(a)?.max(self.tree(b)?))
    }
    pub fn difference(&mut self, a: Node, b: Node) -> Result<Node, Error> {
        self.push(self.tree(a)?.max(-self.tree(b)?))
    }

    /// Level-set offset. Its spatial thickness depends on the source field.
    pub fn offset(&mut self, source: Node, amount: f32) -> Result<Node, Error> {
        finite(&[amount])?;
        self.push(self.tree(source)? - amount)
    }

    /// Smooth seeded spectral noise, normalized to [-amplitude, amplitude].
    /// Frequency is cycles per coordinate unit; amplitude is in source field
    /// units, not necessarily distance. This is a finite wave sum, not Perlin
    /// noise or a hydraulic erosion simulation. Products select sampling scale.
    pub fn displace_waves(&mut self, source: Node, noise: WaveDisplacement) -> Result<Node, Error> {
        let source = self.tree(source)?;
        finite(&noise.frequency)?;
        finite(&[noise.amplitude, noise.lacunarity, noise.gain])?;
        if noise.frequency.iter().any(|f| *f <= 0.0)
            || noise.amplitude < 0.0
            || !(1..=8).contains(&noise.octaves)
            || noise.lacunarity < 1.0
            || !(0.0..=1.0).contains(&noise.gain)
        {
            return Err(Error("wave displacement requires positive frequencies, nonnegative amplitude, 1..8 octaves, lacunarity >= 1 and gain in 0..1".into()));
        }
        let highest = noise.lacunarity.powi(noise.octaves as i32 - 1);
        finite(&noise.frequency.map(|f| f * highest * std::f32::consts::TAU))?;
        if noise.amplitude == 0.0 {
            return self.push(source);
        }
        let mut state = noise.seed;
        let mut draw = || {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            ((z ^ (z >> 31)) >> 40) as f32 / 16777216.0
        };
        let mut sum = Tree::from(0.0_f32);
        let mut weight = 1.0_f32;
        let mut frequency = 1.0_f32;
        let mut total = 0.0_f32;
        for _ in 0..noise.octaves {
            // Three independently oriented waves avoid a privileged grid axis.
            for _ in 0..3 {
                let z = draw() * 2.0 - 1.0;
                let angle = draw() * std::f32::consts::TAU;
                let radial = (1.0 - z * z).sqrt();
                let direction = [radial * angle.cos(), radial * angle.sin(), z];
                let phase = draw() * std::f32::consts::TAU;
                let scale = frequency * std::f32::consts::TAU;
                let wave = (Tree::x() * (direction[0] * noise.frequency[0] * scale)
                    + Tree::y() * (direction[1] * noise.frequency[1] * scale)
                    + Tree::z() * (direction[2] * noise.frequency[2] * scale)
                    + phase)
                    .sin();
                sum += wave * weight;
                total += weight;
            }
            weight *= noise.gain;
            frequency *= noise.lacunarity;
        }
        self.push(source + sum * (noise.amplitude / total))
    }

    /// Polynomial smooth union; radius is in field-value units.
    pub fn smooth_union(&mut self, a: Node, b: Node, radius: f32) -> Result<Node, Error> {
        positive(radius, "blend radius")?;
        let a = self.tree(a)?;
        let b = self.tree(b)?;
        let h = ((b.clone() - a.clone()) / radius * 0.5_f32 + 0.5_f32)
            .max(0.0)
            .min(1.0);
        self.push(
            b.clone() + h.clone() * (a.clone() - b.clone())
                - radius * h.clone() * (1.0 - h.clone()),
        )
    }

    /// Affine placement using a column-major local-to-world matrix. Nonuniform
    /// scaling preserves the zero surface, not a distance metric.
    pub fn transform(&mut self, source: Node, local_to_world: [f32; 16]) -> Result<Node, Error> {
        finite(&local_to_world)?;
        let m = Matrix4::from_column_slice(&local_to_world);
        if m[(3, 0)] != 0.0 || m[(3, 1)] != 0.0 || m[(3, 2)] != 0.0 || m[(3, 3)] != 1.0 {
            return Err(Error("shape placement must be affine".into()));
        }
        let inverse = m
            .try_inverse()
            .ok_or_else(|| Error("shape placement must be invertible".into()))?;
        finite(inverse.as_slice())?;
        self.push(
            self.tree(source)?
                .remap_affine(nalgebra::Affine3::from_matrix_unchecked(
                    inverse.cast::<f64>(),
                )),
        )
    }

    pub fn transform_trs(
        &mut self,
        source: Node,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    ) -> Result<Node, Error> {
        finite(&translation)?;
        finite(&rotation)?;
        finite(&scale)?;
        let q = nalgebra::Quaternion::new(rotation[3], rotation[0], rotation[1], rotation[2]);
        positive(q.norm(), "rotation norm")?;
        let m = Matrix4::new_translation(&Vector3::from(translation))
            * nalgebra::UnitQuaternion::new_normalize(q).to_homogeneous()
            * Matrix4::new_nonuniform_scaling(&Vector3::from(scale));
        self.transform(source, m.as_slice().try_into().expect("4x4 matrix"))
    }

    pub fn sample(&self, node: Node, points: &[[f32; 3]]) -> Result<Vec<f32>, Error> {
        let tape = {
            let mut cached = self
                .sample_tape
                .lock()
                .map_err(|_| Error("field sample cache poisoned".into()))?;
            if cached.as_ref().is_none_or(|(key, _)| *key != node) {
                let shape = JitShape::from(self.tree(node)?);
                *cached = Some((node, shape.ez_float_slice_tape()));
            }
            cached.as_ref().expect("tape populated").1.clone()
        };
        sample_tape(&tape, points)
    }

    pub fn generate(&self, node: Node, options: GenerateOptions) -> Result<Geometry, Error> {
        let started = Instant::now();
        let bounds = options.bounds.validate()?;
        positive(options.cell_size, "cell size")?;
        let min = Vector3::from(bounds.min);
        let max = Vector3::from(bounds.max);
        let extent = max - min;
        let depth = (extent.max() / options.cell_size).log2().ceil().max(1.0);
        // This limits recursion, not peak memory. Dense high-frequency fields
        // can still be expensive; call sites choose bounds and sampling policy.
        if !depth.is_finite() || depth > 12.0 {
            return Err(Error("requested sampling exceeds maximum octree depth 12; reduce bounds or increase cell size".into()));
        }
        let center = min + extent * 0.5;
        let matrix = Matrix4::new_translation(&center) * Matrix4::new_scaling(extent.max() * 0.5);
        let shape = JitShape::from(self.tree(node)?);
        let bound = shape
            .clone()
            .try_into()
            .map_err(|_| Error("unbound field variables".into()))?;
        let settings = Settings {
            depth: depth as u8,
            world_to_model: matrix,
            ..Settings::default()
        };
        let octree = Octree::build(&bound, &settings)
            .ok_or_else(|| Error("surface generation cancelled".into()))?;
        let bounded_leaf_vertices = octree.bounded_leaf_vertices();
        let mesh = triangulate::dual_polygons(octree.walk_dual());
        if mesh.vertices.len() > options.max_vertices as usize
            || mesh.triangles.len() > options.max_triangles as usize
        {
            return Err(Error("extracted surface exceeds output capacity; increase cell size or split the authored composition".into()));
        }
        if mesh
            .vertices
            .iter()
            .any(|p| p.iter().any(|v| !v.is_finite()))
        {
            return Err(Error("extraction produced a non-finite vertex".into()));
        }
        let positions = mesh.vertices.iter().map(|p| [p.x, p.y, p.z]).collect();
        let mut triangles = Vec::with_capacity(mesh.triangles.len());
        let mut degenerate_triangles = 0;
        for t in &mesh.triangles {
            let normal = (mesh.vertices[t.y] - mesh.vertices[t.x])
                .cross(&(mesh.vertices[t.z] - mesh.vertices[t.x]));
            if normal.norm_squared() == 0.0 {
                degenerate_triangles += 1;
                continue;
            }
            // Keep the polygon's consistent shared-edge winding. Independently
            // flipping faces against centroid gradients neither repairs a fold
            // nor reliably classifies facets spanning an unresolved CSG detail.
            triangles.push([t.x as u32, t.y as u32, t.z as u32]);
        }
        Ok(Geometry {
            positions,
            triangles,
            depth: depth as u32,
            cell_size: [extent.max() / 2_f32.powi(depth as i32); 3],
            generation_seconds: started.elapsed().as_secs_f64(),
            reoriented_triangles: 0,
            degenerate_triangles,
            bounded_leaf_vertices,
        })
    }
}

fn coordinates(center: [f32; 3]) -> [Tree; 3] {
    [
        Tree::x() - center[0],
        Tree::y() - center[1],
        Tree::z() - center[2],
    ]
}
fn finite(values: &[f32]) -> Result<(), Error> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(Error("field parameters must be finite".into()))
    }
}
fn positive(value: f32, name: &str) -> Result<(), Error> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error(format!("{name} must be finite and positive")))
    }
}

fn sample_tape(tape: &ShapeTape<JitBulkFn<f32>>, points: &[[f32; 3]]) -> Result<Vec<f32>, Error> {
    let mut evaluator = JitShape::new_float_slice_eval();
    let x: Vec<_> = points.iter().map(|p| p[0]).collect();
    let y: Vec<_> = points.iter().map(|p| p[1]).collect();
    let z: Vec<_> = points.iter().map(|p| p[2]).collect();
    evaluator
        .eval(tape, &x, &y, &z)
        .map(|v| v.to_vec())
        .map_err(|e| Error(format!("field evaluation failed: {e}")))
}

#[cfg(test)]
fn gradients_shape(shape: &JitShape, points: &[[f32; 3]]) -> Result<Vec<[f32; 3]>, Error> {
    use fidget::types::Grad;
    let mut evaluator = JitShape::new_grad_slice_eval();
    let tape = shape.ez_grad_slice_tape();
    let x: Vec<_> = points
        .iter()
        .map(|p| Grad::new(p[0], 1.0, 0.0, 0.0))
        .collect();
    let y: Vec<_> = points
        .iter()
        .map(|p| Grad::new(p[1], 0.0, 1.0, 0.0))
        .collect();
    let z: Vec<_> = points
        .iter()
        .map(|p| Grad::new(p[2], 0.0, 0.0, 1.0))
        .collect();
    evaluator
        .eval(&tape, &x, &y, &z)
        .map(|v| v.iter().map(|g| [g.dx, g.dy, g.dz]).collect())
        .map_err(|e| Error(format!("field gradient failed: {e}")))
}

#[derive(Clone, Copy, Debug)]
pub struct GenerateOptions {
    /// Enclosure expanded about its center to a cube with the longest side.
    /// This preserves uniform world-space samples for thin or long shapes.
    /// Keep the intended surface inside this cube; domain edges are not caps.
    /// To clip a field to a rectangular volume, explicitly intersect a box.
    pub bounds: Bounds,
    /// Maximum leaf sample spacing in world units; not a feature guarantee.
    pub cell_size: f32,
    pub max_vertices: u32,
    pub max_triangles: u32,
}

#[derive(Debug)]
pub struct Geometry {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub depth: u32,
    pub cell_size: [f32; 3],
    pub generation_seconds: f64,
    /// Retained readout compatibility: polygon winding is preserved, so the
    /// current generator performs no independent per-triangle reorientation.
    pub reoriented_triangles: u32,
    pub degenerate_triangles: u32,
    /// Escaped leaf QEF solutions recovered within their source cells.
    pub bounded_leaf_vertices: u32,
}

/// Index topology of extracted geometry, before normal/UV/material splitting.
/// These counts do not establish absence of geometric self-intersections.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TopologyReadout {
    pub boundary_edges: u32,
    pub non_manifold_edges: u32,
    pub inconsistent_winding_edges: u32,
}

impl Geometry {
    pub fn topology(&self) -> TopologyReadout {
        let mut edges = std::collections::HashMap::<(u32, u32), (u32, i32)>::new();
        for &[a, b, c] in &self.triangles {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let entry = edges.entry((a.min(b), a.max(b))).or_default();
                entry.0 += 1;
                entry.1 += if a < b { 1 } else { -1 };
            }
        }
        let mut result = TopologyReadout::default();
        for (uses, balance) in edges.into_values() {
            match uses {
                1 => result.boundary_edges += 1,
                2 if balance != 0 => result.inconsistent_winding_edges += 1,
                3.. => result.non_manifold_edges += 1,
                _ => {}
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sampling_cache_preserves_node_and_clone_ownership() {
        let mut field = Field::new();
        let first = field.sphere([0.0; 3], 1.0).unwrap();
        assert_eq!(field.sample(first, &[[2.0, 0.0, 0.0]]).unwrap(), vec![1.0]);
        let mut clone = field.clone();
        let next = field.sphere([0.0; 3], 3.0).unwrap();
        let other = clone.sphere([0.0; 3], 4.0).unwrap();
        assert_eq!(next, other);
        assert_eq!(field.sample(next, &[[2.0, 0.0, 0.0]]).unwrap(), vec![-1.0]);
        assert_eq!(clone.sample(other, &[[2.0, 0.0, 0.0]]).unwrap(), vec![-2.0]);
        assert_eq!(field.sample(first, &[[2.0, 0.0, 0.0]]).unwrap(), vec![1.0]);
        assert!(field.sample(9999, &[[0.0; 3]]).is_err());
        assert_eq!(field.sample(first, &[[2.0, 0.0, 0.0]]).unwrap(), vec![1.0]);
    }

    #[test]
    fn waves_are_bounded_repeatable_continuous_and_meshable() {
        let mut field = Field::new();
        let sphere = field.sphere([0.0; 3], 1.0).unwrap();
        let options = WaveDisplacement {
            frequency: [0.6, 0.8, 0.5],
            amplitude: 0.12,
            octaves: 3,
            lacunarity: 2.0,
            gain: 0.5,
            seed: 29,
        };
        let rough = field.displace_waves(sphere, options).unwrap();
        let repeat = field.displace_waves(sphere, options).unwrap();
        let other = field
            .displace_waves(
                sphere,
                WaveDisplacement {
                    seed: 47,
                    ..options
                },
            )
            .unwrap();
        let zero = field
            .displace_waves(
                sphere,
                WaveDisplacement {
                    amplitude: 0.0,
                    ..options
                },
            )
            .unwrap();
        let points: Vec<_> = (0..100)
            .map(|i| [i as f32 * 0.031 - 1.5, 0.3, -0.7])
            .collect();
        let base = field.sample(sphere, &points).unwrap();
        let values = field.sample(rough, &points).unwrap();
        assert_eq!(values, field.sample(repeat, &points).unwrap());
        assert_eq!(base, field.sample(zero, &points).unwrap());
        assert_ne!(values, field.sample(other, &points).unwrap());
        assert!(values
            .iter()
            .zip(&base)
            .all(|(a, b)| (a - b).abs() <= options.amplitude + 1e-6));
        let adjacent: Vec<_> = points.iter().map(|p| [p[0] + 1e-5, p[1], p[2]]).collect();
        assert!(values
            .iter()
            .zip(field.sample(rough, &adjacent).unwrap())
            .all(|(a, b)| (a - b).abs() < 1e-3));
        assert!(field
            .displace_waves(
                sphere,
                WaveDisplacement {
                    octaves: 0,
                    ..options
                }
            )
            .is_err());
        assert!(field
            .displace_waves(
                sphere,
                WaveDisplacement {
                    frequency: [f32::MAX; 3],
                    ..options
                }
            )
            .is_err());
        let mesh = field
            .generate(
                rough,
                GenerateOptions {
                    bounds: Bounds {
                        min: [-1.4; 3],
                        max: [1.4; 3],
                    },
                    cell_size: 0.08,
                    max_vertices: 100_000,
                    max_triangles: 200_000,
                },
            )
            .unwrap();
        assert!(!mesh.triangles.is_empty());
        assert_eq!(mesh.topology(), TopologyReadout::default());
    }

    use super::*;
    use std::collections::BTreeMap;

    fn assert_closed_and_oriented(mesh: &Geometry) {
        assert_eq!(mesh.topology(), TopologyReadout::default());
        let mut edges = BTreeMap::<(u32, u32), (usize, i32)>::new();
        for &[a, b, c] in &mesh.triangles {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let entry = edges.entry((a.min(b), a.max(b))).or_default();
                entry.0 += 1;
                entry.1 += if a < b { 1 } else { -1 };
            }
        }
        for (edge, (count, orientation)) in edges {
            assert_eq!(count, 2, "open or nonmanifold edge {edge:?}");
            assert_eq!(orientation, 0, "inconsistent winding at {edge:?}");
        }
    }
    #[test]
    fn topology_distinguishes_open_nonmanifold_and_reversed_edges() {
        let mut mesh = Geometry {
            positions: vec![],
            triangles: vec![[0, 1, 2]],
            depth: 0,
            cell_size: [1.; 3],
            generation_seconds: 0.,
            reoriented_triangles: 0,
            degenerate_triangles: 0,
            bounded_leaf_vertices: 0,
        };
        assert_eq!(mesh.topology().boundary_edges, 3);
        mesh.triangles.push([0, 1, 3]);
        assert_eq!(
            mesh.topology(),
            TopologyReadout {
                boundary_edges: 4,
                inconsistent_winding_edges: 1,
                non_manifold_edges: 0,
            }
        );
        mesh.triangles.push([1, 0, 4]);
        assert_eq!(
            mesh.topology(),
            TopologyReadout {
                boundary_edges: 6,
                inconsistent_winding_edges: 0,
                non_manifold_edges: 1,
            }
        );
        mesh.triangles = vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
        assert_eq!(mesh.topology(), TopologyReadout::default());
    }

    fn options(bounds: Bounds) -> GenerateOptions {
        GenerateOptions {
            bounds,
            cell_size: 0.08,
            max_vertices: 200_000,
            max_triangles: 400_000,
        }
    }

    #[test]
    fn capped_tapers_preserve_caps_radius_and_orientation() {
        let mut f = Field::new();
        let taper = f
            .frustum([1.0, 0.0, 0.0], [1.0, 2.0, 0.0], 1.0, 0.5)
            .unwrap();
        let samples = [
            [1.0, 1.0, 0.0],
            [1.0, -0.1, 0.0],
            [1.0, 2.1, 0.0],
            [1.75, 1.0, 0.0],
            [1.8, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
        ];
        let values = f.sample(taper, &samples).unwrap();
        assert!(values[0] < 0.0 && values[1] > 0.0 && values[2] > 0.0 && values[4] > 0.0);
        for i in [3, 5, 6] {
            assert!(values[i].abs() < 1e-5);
        }
        let reverse = f
            .frustum([1.0, 2.0, 0.0], [1.0, 0.0, 0.0], 0.5, 1.0)
            .unwrap();
        for (a, b) in values.iter().zip(f.sample(reverse, &samples).unwrap()) {
            assert!((a - b).abs() < 1e-5);
        }
        let diagonal = f.frustum([0.0; 3], [2.0, 2.0, 0.0], 0.5, 0.5).unwrap();
        let values = f
            .sample(
                diagonal,
                &[[1.0, 1.0, 0.4], [1.0, 1.0, 0.6], [-0.1, -0.1, 0.0]],
            )
            .unwrap();
        assert!(values[0] < 0.0 && values[1] > 0.0 && values[2] > 0.0);
        let cone = f.frustum([0.0; 3], [0.0, 2.0, 0.0], 1.0, 0.0).unwrap();
        let values = f.sample(cone, &[[0.0, 2.0, 0.0], [0.1, 1.9, 0.0]]).unwrap();
        assert!(values[0].abs() < 1e-5 && values[1] > 0.0);
        for node in [taper, diagonal, cone] {
            let mesh = f
                .generate(
                    node,
                    options(Bounds {
                        min: [-1.2, -0.2, -1.2],
                        max: [2.8, 2.8, 1.2],
                    }),
                )
                .unwrap();
            assert!(!mesh.triangles.is_empty());
            assert_closed_and_oriented(&mesh);
        }
        assert!(f.frustum([0.0; 3], [0.0; 3], 1.0, 1.0).is_err());
        assert!(f.frustum([0.0; 3], [0.0, 1.0, 0.0], -1.0, 1.0).is_err());
        assert!(f.frustum([0.0; 3], [0.0, 1.0, 0.0], 0.0, 0.0).is_err());
    }

    #[test]
    fn constructive_opening_preserves_scalar_shape_and_winding() {
        let mut f = Field::new();
        let wall = f
            .box_shape(Bounds {
                min: [-2.0, 0.0, -0.3],
                max: [2.0, 3.0, 0.3],
            })
            .unwrap();
        let opening = f
            .box_shape(Bounds {
                min: [-0.6, -0.1, -1.0],
                max: [0.6, 2.0, 1.0],
            })
            .unwrap();
        let root = f.difference(wall, opening).unwrap();
        let v = f
            .sample(root, &[[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 0.45]])
            .unwrap();
        assert!(v[0] > 0.0 && v[1] < 0.0 && v[2] > 0.0);
        let m = f
            .generate(
                root,
                options(Bounds {
                    min: [-2.2, -0.2, -0.5],
                    max: [2.2, 3.2, 0.5],
                }),
            )
            .unwrap();
        assert!(!m.triangles.is_empty());
        for t in &m.triangles {
            let a = Vector3::from(m.positions[t[0] as usize]);
            let b = Vector3::from(m.positions[t[1] as usize]);
            let c = Vector3::from(m.positions[t[2] as usize]);
            let p = (a + b + c) / 3.0;
            assert!(
                !(p.x.abs() < 0.5 && p.y > 0.1 && p.y < 1.9),
                "triangle bridged the opening: {p:?}"
            );
        }
        assert!(m.cell_size.iter().all(|v| *v <= 0.08));
        assert_closed_and_oriented(&m);
    }

    #[test]
    fn sphere_normals_face_outward_and_affine_placement_preserves_sign() {
        let mut f = Field::new();
        let sphere = f.sphere([0.0; 3], 0.7).unwrap();
        let mesh = f
            .generate(
                sphere,
                options(Bounds {
                    min: [-1.0; 3],
                    max: [1.0; 3],
                }),
            )
            .unwrap();
        let mut positive = 0;
        let mut negative = 0;
        let mut worst = 0.0_f32;
        for t in &mesh.triangles {
            let a = Vector3::from(mesh.positions[t[0] as usize]);
            let b = Vector3::from(mesh.positions[t[1] as usize]);
            let c = Vector3::from(mesh.positions[t[2] as usize]);
            let dot = (b - a).cross(&(c - a)).dot(&(a + b + c));
            if dot < -1e-6 {
                negative += 1;
                worst = worst.min(dot);
            } else {
                positive += 1;
            }
        }
        assert_eq!(
            negative,
            0,
            "outward {positive}, inward {negative}, worst {worst}, triangles {}",
            mesh.triangles.len()
        );
        assert_closed_and_oriented(&mesh);
        let matrix = Matrix4::new_translation(&Vector3::new(3.0, 2.0, 1.0))
            * Matrix4::new_nonuniform_scaling(&Vector3::new(2.0, 1.0, 0.5));
        let placed = f
            .transform(sphere, matrix.as_slice().try_into().unwrap())
            .unwrap();
        let values = f
            .sample(placed, &[[3.0, 2.0, 1.0], [4.0, 2.0, 1.0], [5.0, 2.0, 1.0]])
            .unwrap();
        assert!(values[0] < 0.0 && values[1] < 0.0 && values[2] > 0.0);
    }

    #[test]
    fn adaptive_box_and_cap_have_coherent_outward_facets() {
        for cap in [false, true] {
            let mut field = Field::new();
            let mut root = field
                .box_shape(Bounds {
                    min: [-2.0, 0.0, -0.3],
                    max: [2.0, 3.0, 0.3],
                })
                .unwrap();
            if cap {
                let trim = field
                    .box_shape(Bounds {
                        min: [-2.14, 2.78, -0.44],
                        max: [2.14, 3.0 + 0.14, 0.44],
                    })
                    .unwrap();
                root = field.union(root, trim).unwrap();
            }
            let geometry = field
                .generate(
                    root,
                    GenerateOptions {
                        bounds: Bounds {
                            min: [-2.45, -0.95, -2.45],
                            max: [2.45, 3.95, 2.45],
                        },
                        cell_size: 0.2,
                        max_vertices: 200_000,
                        max_triangles: 400_000,
                    },
                )
                .unwrap();
            assert_closed_and_oriented(&geometry);
            let centroids: Vec<[f32; 3]> = geometry
                .triangles
                .iter()
                .map(|t| {
                    let p = t.map(|i| Vector3::from(geometry.positions[i as usize]));
                    ((p[0] + p[1] + p[2]) / 3.0).into()
                })
                .collect();
            let gradients =
                gradients_shape(&JitShape::from(field.tree(root).unwrap()), &centroids).unwrap();
            for (t, gradient) in geometry.triangles.iter().zip(gradients) {
                let p = t.map(|i| Vector3::from(geometry.positions[i as usize]));
                let normal = (p[1] - p[0]).cross(&(p[2] - p[0]));
                assert!(
                    normal.dot(&Vector3::from(gradient)) >= -1e-6,
                    "inward cap={cap} triangle={p:?}"
                );
            }
        }
    }

    #[test]
    fn disconnected_inner_boundary_retains_cavity_winding() {
        let mut field = Field::new();
        let outer = field.sphere([0.0; 3], 0.9).unwrap();
        let inner = field.sphere([0.0; 3], 0.4).unwrap();
        let shell = field.difference(outer, inner).unwrap();
        let mesh = field
            .generate(
                shell,
                options(Bounds {
                    min: [-1.1; 3],
                    max: [1.1; 3],
                }),
            )
            .unwrap();
        assert_closed_and_oriented(&mesh);
        let mut inner_faces = 0;
        let mut outer_faces = 0;
        for t in &mesh.triangles {
            let p = t.map(|i| Vector3::from(mesh.positions[i as usize]));
            let center = (p[0] + p[1] + p[2]) / 3.0;
            let normal = (p[1] - p[0]).cross(&(p[2] - p[0]));
            if center.norm() < 0.65 {
                inner_faces += 1;
                assert!(normal.dot(&center) < 0.0, "cavity faces into solid");
            } else {
                outer_faces += 1;
                assert!(normal.dot(&center) > 0.0, "outer face points inward");
            }
        }
        assert!(inner_faces > 0 && outer_faces > 0);
    }
}
