//! Bounded implicit geometry, independent of product style and renderer ownership.
//!
//! Shapes are negative inside. CSG preserves a zero surface, not necessarily
//! Euclidean distance. An arena owns shape expressions; nodes are local to it.
//! Fidget supplies evaluation, interval pruning and adaptive dual-cell
//! connectivity. Engine triangulates the resulting cell-vertex polygons.

use fidget::{
    context::Tree,
    jit::JitShape,
    mesh::{Octree, Settings},
    shape::EzShape,
};
use nalgebra::{Matrix4, Vector3};
use std::time::Instant;

pub mod surface;
mod triangulate;

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

/// No geometric or aesthetic policy is hidden in the node arena.
#[derive(Default, Clone)]
pub struct Field {
    nodes: Vec<Tree>,
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
        let shape = JitShape::from(self.tree(node)?);
        sample_shape(&shape, points)
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

fn sample_shape(shape: &JitShape, points: &[[f32; 3]]) -> Result<Vec<f32>, Error> {
    let mut evaluator = JitShape::new_float_slice_eval();
    let tape = shape.ez_float_slice_tape();
    let x: Vec<_> = points.iter().map(|p| p[0]).collect();
    let y: Vec<_> = points.iter().map(|p| p[1]).collect();
    let z: Vec<_> = points.iter().map(|p| p[2]).collect();
    evaluator
        .eval(&tape, &x, &y, &z)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn assert_closed_and_oriented(mesh: &Geometry) {
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
    fn options(bounds: Bounds) -> GenerateOptions {
        GenerateOptions {
            bounds,
            cell_size: 0.08,
            max_vertices: 200_000,
            max_triangles: 400_000,
        }
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
