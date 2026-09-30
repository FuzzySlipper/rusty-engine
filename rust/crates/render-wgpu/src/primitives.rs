//! Built-in geometry for primitive nodes:
//! a unit cube, a radius-0.5 sphere (8×8 segments), a unit quad facing +Z, and
//! points as small cubes.

use crate::pipelines::VERTEX_FLOATS;
use crate::tables::Builtin;

/// Side of the cube a `Point` node is drawn as.
const POINT_SIZE: f32 = 0.1;
const SPHERE_RADIUS: f32 = 0.5;
const SPHERE_SEGMENTS: u32 = 8;

pub(crate) struct Geometry {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

impl Geometry {
    fn push(&mut self, position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> u32 {
        let index = (self.vertices.len() / VERTEX_FLOATS) as u32;
        self.vertices.extend_from_slice(&position);
        self.vertices.extend_from_slice(&normal);
        self.vertices.extend_from_slice(&uv);
        self.vertices.extend_from_slice(&[1.0; 4]);
        index
    }
}

pub(crate) fn builtin(kind: Builtin) -> Geometry {
    match kind {
        Builtin::Cube => cube(1.0),
        Builtin::Point => cube(POINT_SIZE),
        Builtin::Sphere => sphere(),
        Builtin::Quad => quad(),
    }
}

fn cube(size: f32) -> Geometry {
    let h = size * 0.5;
    let mut geometry = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    // (normal, u axis, v axis) per face; corners wind counter-clockwise.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    for (normal, u, v) in faces {
        let corner = |su: f32, sv: f32| {
            std::array::from_fn(|axis| (normal[axis] + u[axis] * su + v[axis] * sv) * h)
        };
        let a = geometry.push(corner(-1.0, -1.0), normal, [0.0, 1.0]);
        let b = geometry.push(corner(1.0, -1.0), normal, [1.0, 1.0]);
        let c = geometry.push(corner(1.0, 1.0), normal, [1.0, 0.0]);
        let d = geometry.push(corner(-1.0, 1.0), normal, [0.0, 0.0]);
        geometry.indices.extend_from_slice(&[a, b, c, a, c, d]);
    }
    geometry
}

fn sphere() -> Geometry {
    let mut geometry = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let rings = SPHERE_SEGMENTS;
    let segments = SPHERE_SEGMENTS;
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let theta = v * std::f32::consts::PI;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let phi = u * std::f32::consts::TAU;
            let normal = [
                -phi.cos() * theta.sin(),
                theta.cos(),
                phi.sin() * theta.sin(),
            ];
            let position = normal.map(|component| component * SPHERE_RADIUS);
            geometry.push(position, normal, [u, v]);
        }
    }
    let stride = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * stride + segment;
            let b = a + stride;
            geometry
                .indices
                .extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }
    geometry
}

fn quad() -> Geometry {
    let mut geometry = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let normal = [0.0, 0.0, 1.0];
    let a = geometry.push([-0.5, -0.5, 0.0], normal, [0.0, 0.0]);
    let b = geometry.push([0.5, -0.5, 0.0], normal, [1.0, 0.0]);
    let c = geometry.push([0.5, 0.5, 0.0], normal, [1.0, 1.0]);
    let d = geometry.push([-0.5, 0.5, 0.0], normal, [0.0, 1.0]);
    geometry.indices.extend_from_slice(&[a, b, c, a, c, d]);
    geometry
}

/// A two-vertex line segment for a `Line { a, b }` node.
pub(crate) fn line(a: [f32; 3], b: [f32; 3]) -> Geometry {
    let mut geometry = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let normal = [0.0, 1.0, 0.0];
    geometry.push(a, normal, [0.0, 0.0]);
    geometry.push(b, normal, [1.0, 0.0]);
    geometry.indices.extend_from_slice(&[0, 1]);
    geometry
}
