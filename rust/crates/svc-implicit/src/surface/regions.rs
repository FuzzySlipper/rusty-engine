//! Material partitioning on an already attributed surface. Cutting after normal
//! and UV construction keeps material boundaries from changing the shading or
//! projection charts of the original geometry.

use super::{BTreeMap, Error, Field, HashMap, MaterialRegion, Surface, SurfaceGroup};

#[derive(Clone)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    values: Vec<f64>,
}

impl Vertex {
    fn key(&self) -> [u32; 8] {
        let [x, y, z] = self.position;
        let [nx, ny, nz] = self.normal;
        let [u, v] = self.uv;
        [x, y, z, nx, ny, nz, u, v].map(|v| if v == 0.0 { 0 } else { v.to_bits() })
    }
}

pub(super) fn split(
    field: &Field,
    surface: Surface,
    regions: &[MaterialRegion],
    default_slot: u32,
) -> Result<Surface, Error> {
    let samples = regions
        .iter()
        .map(|region| field.sample(region.node, &surface.positions))
        .collect::<Result<Vec<_>, _>>()?;
    if samples.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error("material region produced a non-finite sample".into()));
    }
    let source: Vec<_> = (0..surface.positions.len())
        .map(|i| Vertex {
            position: surface.positions[i],
            normal: surface.normals[i],
            uv: surface.uvs[i],
            values: samples.iter().map(|s| f64::from(s[i])).collect(),
        })
        .collect();
    let mut output = Output::default();
    for triangle in surface.indices.as_chunks::<3>().0 {
        let mut remaining: Vec<_> = triangle
            .iter()
            .map(|&i| source[i as usize].clone())
            .collect();
        for (index, region) in regions.iter().enumerate() {
            let (inside, outside) = partition(remaining, index);
            output.polygon(inside, region.slot)?;
            remaining = outside;
            if remaining.is_empty() {
                break;
            }
        }
        output.polygon(remaining, default_slot)?;
    }
    output.finish()
}

fn partition(polygon: Vec<Vertex>, region: usize) -> (Vec<Vertex>, Vec<Vertex>) {
    if polygon.iter().all(|v| v.values[region] <= 0.0) {
        return (polygon, Vec::new());
    }
    if polygon.iter().all(|v| v.values[region] >= 0.0) {
        return (Vec::new(), polygon);
    }
    let mut inside = Vec::new();
    let mut outside = Vec::new();
    for i in 0..polygon.len() {
        let a = &polygon[i];
        let b = &polygon[(i + 1) % polygon.len()];
        let av = a.values[region];
        let bv = b.values[region];
        if av <= 0.0 {
            inside.push(a.clone());
        }
        if av >= 0.0 {
            outside.push(a.clone());
        }
        if (av < 0.0 && bv > 0.0) || (av > 0.0 && bv < 0.0) {
            let crossing = intersection(a, b, region);
            inside.push(crossing.clone());
            outside.push(crossing);
        }
    }
    (inside, outside)
}

fn intersection<'a>(mut a: &'a Vertex, mut b: &'a Vertex, region: usize) -> Vertex {
    // The same geometric edge must yield the same position when encountered
    // in opposite directions or on separate normal/UV charts.
    if a.position.map(f32::to_bits) > b.position.map(f32::to_bits) {
        std::mem::swap(&mut a, &mut b);
    }
    let t = a.values[region] / (a.values[region] - b.values[region]);
    let blend = |a: f32, b: f32| (f64::from(a) * (1.0 - t) + f64::from(b) * t) as f32;
    let mut values: Vec<_> = a
        .values
        .iter()
        .zip(&b.values)
        .map(|(a, b)| a * (1.0 - t) + b * t)
        .collect();
    values[region] = 0.0;
    Vertex {
        position: std::array::from_fn(|i| blend(a.position[i], b.position[i])),
        // Preserve the original interpolated normal field; normalization here
        // would alter shading merely because a material cut added a vertex.
        normal: std::array::from_fn(|i| blend(a.normal[i], b.normal[i])),
        uv: std::array::from_fn(|i| blend(a.uv[i], b.uv[i])),
        values,
    }
}

#[derive(Default)]
struct Output {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    vertices: HashMap<[u32; 8], u32>,
    groups: BTreeMap<u32, Vec<u32>>,
}

impl Output {
    fn polygon(&mut self, polygon: Vec<Vertex>, slot: u32) -> Result<(), Error> {
        for i in 1..polygon.len().saturating_sub(1) {
            let triangle = [&polygon[0], &polygon[i], &polygon[i + 1]];
            let [a, b, c] = triangle.map(|v| v.position.map(f64::from));
            let ab: [f64; 3] = std::array::from_fn(|i| b[i] - a[i]);
            let ac: [f64; 3] = std::array::from_fn(|i| c[i] - a[i]);
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            if cross == [0.0; 3] {
                continue;
            }
            for vertex in triangle {
                let key = vertex.key();
                let index = if let Some(&index) = self.vertices.get(&key) {
                    index
                } else {
                    let index = u32::try_from(self.positions.len())
                        .map_err(|_| Error("surface vertex capacity exceeded".into()))?;
                    self.positions.push(vertex.position);
                    self.normals.push(vertex.normal);
                    self.uvs.push(vertex.uv);
                    self.vertices.insert(key, index);
                    index
                };
                self.groups.entry(slot).or_default().push(index);
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Surface, Error> {
        let mut indices = Vec::new();
        let mut groups = Vec::new();
        for (slot, group) in self.groups {
            let index_start = u32::try_from(indices.len())
                .map_err(|_| Error("surface index capacity exceeded".into()))?;
            let index_count = u32::try_from(group.len())
                .map_err(|_| Error("surface index capacity exceeded".into()))?;
            indices.extend(group);
            groups.push(SurfaceGroup {
                slot,
                index_start,
                index_count,
            });
        }
        Ok(Surface {
            positions: self.positions,
            normals: self.normals,
            uvs: self.uvs,
            indices,
            groups,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_vertices_preserve_the_original_smooth_normal_interpolation() {
        let mut field = Field::new();
        let node = field.plane([0.0, 1.0, 0.0], 0.25).unwrap();
        let surface = Surface {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            indices: vec![0, 1, 2],
            groups: vec![SurfaceGroup {
                slot: 0,
                index_start: 0,
                index_count: 3,
            }],
        };
        let output = split(&field, surface, &[MaterialRegion { node, slot: 1 }], 0).unwrap();
        assert!(output.positions.len() > 3);
        for (p, n) in output.positions.iter().zip(&output.normals) {
            assert_eq!(*n, [1.0 - p[0] - p[1], p[0], p[1]]);
        }
    }
}
