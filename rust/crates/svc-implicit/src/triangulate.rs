//! Adapt Fidget 0.5's edge-intersection fans to dual-cell polygons.
//!
//! Fidget's `dc_edge` emits a cyclic fan around a sampled edge intersection,
//! putting that intersection last in every triangle (`MeshBuilder::triangle`).
//! At adaptive transitions that point need not lie inside the polygon of QEF
//! vertices. Its fan can therefore fold even when the polygon is a triangle.
//! Use the ordered QEF boundary directly, preserving its topological winding.
//! This does not repair escaped QEF vertices or guarantee no self-intersection.

use std::collections::HashMap;

use fidget::mesh::Mesh;
use nalgebra::Vector3;

pub(super) fn dual_polygons(mesh: Mesh) -> Mesh {
    let arcs: std::collections::HashSet<_> = mesh.face_arc_vertices.iter().copied().collect();
    let mut fans: Vec<Vec<Vector3<usize>>> = Vec::new();
    let mut centers = HashMap::new();
    for triangle in mesh.triangles {
        let next = fans.len();
        let index = *centers.entry(triangle.z).or_insert_with(|| {
            fans.push(Vec::with_capacity(4));
            next
        });
        fans[index].push(triangle);
    }
    let mut triangles = Vec::new();
    for fan in fans {
        // A new diagonal between the two cell QEFs would identify the two
        // distinct arcs of a checkerboard face again. Keep these local fans.
        if fan
            .iter()
            .any(|t| arcs.contains(&t.x) || arcs.contains(&t.y))
        {
            triangles.extend(fan);
            continue;
        }
        let Some(ring) = boundary(&fan) else {
            // Do not reinterpret an unfamiliar/non-simple fan as a polygon.
            triangles.extend(fan);
            continue;
        };
        let a = ring[0];
        let b = ring[1];
        let c = ring[2];
        if ring.len() == 3 {
            triangles.push(Vector3::new(a, b, c));
        } else {
            let d = ring[3];
            let ac = [Vector3::new(a, b, c), Vector3::new(a, c, d)];
            let bd = [Vector3::new(a, b, d), Vector3::new(b, c, d)];
            // Prefer a diagonal whose triangles agree in orientation. This
            // chooses the interior diagonal of a planar concave quad; on a
            // warped quad it avoids the more severe bend. Shorter diagonal
            // breaks ties without depending on hash-map iteration order.
            let ac_score = agreement(&mesh.vertices, ac);
            let bd_score = agreement(&mesh.vertices, bd);
            let ac_length =
                (mesh.vertices[a].cast::<f64>() - mesh.vertices[c].cast::<f64>()).norm_squared();
            let bd_length =
                (mesh.vertices[b].cast::<f64>() - mesh.vertices[d].cast::<f64>()).norm_squared();
            triangles.extend(
                if ac_score > bd_score || (ac_score == bd_score && ac_length <= bd_length) {
                    ac
                } else {
                    bd
                },
            );
        }
    }

    // Edge intersections no longer used by any polygon must not inflate the
    // resource's vertex count or retain unused positions in downstream meshes.
    let mut vertices = Vec::new();
    let mut remap = vec![usize::MAX; mesh.vertices.len()];
    for triangle in &mut triangles {
        for index in triangle.iter_mut() {
            let mapped = &mut remap[*index];
            if *mapped == usize::MAX {
                *mapped = vertices.len();
                vertices.push(mesh.vertices[*index]);
            }
            *index = *mapped;
        }
    }
    let face_arc_vertices = mesh
        .face_arc_vertices
        .into_iter()
        .filter_map(|index| (remap[index] != usize::MAX).then_some(remap[index]))
        .collect();
    Mesh {
        face_arc_vertices,
        vertices,
        triangles,
    }
}

fn boundary(fan: &[Vector3<usize>]) -> Option<Vec<usize>> {
    if !(3..=4).contains(&fan.len()) {
        return None;
    }
    let mut ring = vec![fan[0].x];
    for _ in 0..fan.len() {
        let current = *ring.last()?;
        let mut outgoing = fan.iter().filter(|t| t.x == current);
        let next = outgoing.next()?.y;
        if outgoing.next().is_some() || next == fan[0].z {
            return None;
        }
        if next == ring[0] {
            return (ring.len() == fan.len()).then_some(ring);
        }
        if ring.contains(&next) {
            return None;
        }
        ring.push(next);
    }
    None
}

fn agreement(vertices: &[Vector3<f32>], triangles: [Vector3<usize>; 2]) -> f64 {
    let normals = triangles.map(|t| {
        let a = vertices[t.x].cast::<f64>();
        let b = vertices[t.y].cast::<f64>();
        let c = vertices[t.z].cast::<f64>();
        (b - a).cross(&(c - a))
    });
    let denominator = normals[0].norm() * normals[1].norm();
    if denominator == 0.0 {
        -2.0
    } else {
        (normals[0].dot(&normals[1]) / denominator).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_polygon_uses_its_interior_diagonal_and_preserves_winding() {
        // The sampled intersection lies outside the polygon. Its original
        // fan crosses the concavity; the QEF boundary itself remains valid.
        let mesh = Mesh {
            face_arc_vertices: Vec::new(),
            vertices: vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(2.0, 0.0, 0.0),
                Vector3::new(0.5, 0.5, 0.0),
                Vector3::new(0.0, 2.0, 0.0),
                Vector3::new(1.5, 1.5, 0.0),
            ],
            triangles: vec![
                Vector3::new(0, 1, 4),
                Vector3::new(1, 2, 4),
                Vector3::new(2, 3, 4),
                Vector3::new(3, 0, 4),
            ],
        };
        let result = dual_polygons(mesh);
        assert_eq!(result.vertices.len(), 4);
        assert_eq!(result.triangles.len(), 2);
        let mut twice_area = 0.0;
        for t in result.triangles {
            let a = result.vertices[t.x];
            let b = result.vertices[t.y];
            let c = result.vertices[t.z];
            let normal = (b - a).cross(&(c - a));
            assert!(normal.z > 0.0);
            twice_area += normal.z;
        }
        assert_eq!(twice_area, 2.0);
    }
}
