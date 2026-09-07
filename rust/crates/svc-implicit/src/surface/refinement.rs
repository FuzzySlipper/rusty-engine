//! Conforming edge subdivision for material sampling. Positions remain on the
//! original triangles; normals and UVs retain their original interpolation.

use super::{Error, HashMap, MaterialSampling, Surface};

pub(super) fn refine(mut surface: Surface, sampling: MaterialSampling) -> Result<Surface, Error> {
    let threshold = f64::from(sampling.max_edge_length).powi(2);
    loop {
        if surface.positions.len() > sampling.max_vertices
            || surface.indices.len() / 3 > sampling.max_triangles
        {
            return Err(Error("material sampling mesh budget exceeded".into()));
        }
        let mut midpoints = HashMap::<[u32; 2], u32>::new();
        let mut indices = Vec::with_capacity(surface.indices.len());
        for triangle in std::mem::take(&mut surface.indices).as_chunks::<3>().0 {
            let [a, b, c] = *triangle;
            let mut mids = [None; 3];
            for (edge, [u, v]) in [[a, b], [b, c], [c, a]].into_iter().enumerate() {
                let p = surface.positions[u as usize];
                let q = surface.positions[v as usize];
                let length: f64 = (0..3)
                    .map(|i| (f64::from(p[i]) - f64::from(q[i])).powi(2))
                    .sum();
                if length <= threshold {
                    continue;
                }
                let key = if u < v { [u, v] } else { [v, u] };
                let mid = if let Some(&mid) = midpoints.get(&key) {
                    mid
                } else {
                    if surface.positions.len() >= sampling.max_vertices {
                        return Err(Error("material sampling vertex budget exceeded".into()));
                    }
                    let mid = u32::try_from(surface.positions.len())
                        .map_err(|_| Error("surface vertex capacity exceeded".into()))?;
                    let average = |a: f32, b: f32| ((f64::from(a) + f64::from(b)) * 0.5) as f32;
                    let position = std::array::from_fn(|i| average(p[i], q[i]));
                    if position == p || position == q {
                        return Err(Error(
                            "material sample spacing is below position precision".into(),
                        ));
                    }
                    surface.positions.push(position);
                    surface.normals.push(std::array::from_fn(|i| {
                        average(
                            surface.normals[u as usize][i],
                            surface.normals[v as usize][i],
                        )
                    }));
                    surface.uvs.push(std::array::from_fn(|i| {
                        average(surface.uvs[u as usize][i], surface.uvs[v as usize][i])
                    }));
                    midpoints.insert(key, mid);
                    mid
                };
                mids[edge] = Some(mid);
            }
            // All triangles make the same split decision for a shared geometric
            // edge, including independently indexed normal and UV charts.
            let pieces: Vec<[u32; 3]> = match mids {
                [None, None, None] => vec![[a, b, c]],
                [Some(x), None, None] => vec![[a, x, c], [x, b, c]],
                [None, Some(y), None] => vec![[b, y, a], [y, c, a]],
                [None, None, Some(z)] => vec![[c, z, b], [z, a, b]],
                [Some(x), Some(y), None] => vec![[b, y, x], [a, x, c], [x, y, c]],
                [None, Some(y), Some(z)] => vec![[c, z, y], [b, y, a], [y, z, a]],
                [Some(x), None, Some(z)] => vec![[a, x, z], [c, z, b], [z, x, b]],
                [Some(x), Some(y), Some(z)] => vec![[a, x, z], [x, b, y], [z, y, c], [x, y, z]],
            };
            if indices.len() / 3 + pieces.len() > sampling.max_triangles {
                return Err(Error("material sampling triangle budget exceeded".into()));
            }
            indices.extend(pieces.into_iter().flatten());
        }
        surface.indices = indices;
        // Groups will be rebuilt by material clipping, which immediately follows.
        surface.groups.clear();
        if midpoints.is_empty() {
            return Ok(surface);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uneven_triangles_and_attribute_seams_subdivide_without_t_junctions() {
        // A closed tetrahedron with independently indexed faces, as after normal
        // or UV chart splitting. Unequal edges exercise partial split patterns.
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.2, 0.7, 0.0],
            [0.3, 0.2, 0.4],
        ];
        let faces = [[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
        let positions: Vec<_> = faces.into_iter().flatten().map(|i| points[i]).collect();
        let source = Surface {
            normals: vec![[0.0, 1.0, 0.0]; 12],
            uvs: positions.iter().map(|p| [p[0], p[1]]).collect(),
            positions,
            indices: (0..12).collect(),
            groups: vec![],
        };
        let output = refine(
            source,
            MaterialSampling {
                max_edge_length: 0.13,
                max_vertices: 10_000,
                max_triangles: 10_000,
            },
        )
        .unwrap();
        let mut edges = HashMap::<[[u32; 3]; 2], (usize, i32)>::new();
        for t in output.indices.as_chunks::<3>().0 {
            for [a, b] in [[t[0], t[1]], [t[1], t[2]], [t[2], t[0]]] {
                let p = output.positions[a as usize];
                let q = output.positions[b as usize];
                assert!(
                    (0..3).map(|i| (p[i] - q[i]).powi(2)).sum::<f32>()
                        <= 0.13_f32.powi(2) * 1.00001
                );
                let p = p.map(f32::to_bits);
                let q = q.map(f32::to_bits);
                let (key, sign) = if p < q { ([p, q], 1) } else { ([q, p], -1) };
                let entry = edges.entry(key).or_default();
                entry.0 += 1;
                entry.1 += sign;
            }
        }
        assert!(edges
            .values()
            .all(|&(count, winding)| count == 2 && winding == 0));
        for (p, uv) in output.positions.iter().zip(&output.uvs) {
            assert_eq!([p[0], p[1]], *uv);
        }
    }
}
