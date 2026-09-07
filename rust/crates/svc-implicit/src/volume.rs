//! Retained scalar lattice points, independent of block-voxel occupancy.
//! Negative values are inside. Samples are x-fastest; spacing is uniform.
use super::{Error, Field, Geometry, Node};
use fidget::{jit::JitShape, shape::EzShape};
use std::time::Instant;

const MAX_SAMPLES: usize = 8_000_000;
const EVALUATION_BATCH: usize = 32_768;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeDescriptor {
    pub origin: [f32; 3],
    pub spacing: f32,
    pub dimensions: [u32; 3],
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct SampledVolume {
    descriptor: VolumeDescriptor,
    values: Vec<f32>,
}

impl SampledVolume {
    pub fn new(
        origin: [f32; 3],
        spacing: f32,
        dimensions: [u32; 3],
        initial: f32,
    ) -> Result<Self, Error> {
        super::finite(&origin)?;
        super::positive(spacing, "volume spacing")?;
        if !initial.is_finite() || dimensions.iter().any(|d| *d < 2) {
            return Err(Error(
                "volume needs finite samples and at least two lattice points per axis".into(),
            ));
        }
        let count = dimensions
            .iter()
            .try_fold(1usize, |n, d| n.checked_mul(*d as usize))
            .filter(|n| *n <= MAX_SAMPLES)
            .ok_or_else(|| Error(format!("volume exceeds {MAX_SAMPLES} retained samples")))?;
        for axis in 0..3 {
            let end = origin[axis] + (dimensions[axis] - 1) as f32 * spacing;
            if !end.is_finite() || origin[axis] + spacing <= origin[axis] || end - spacing >= end {
                return Err(Error(
                    "volume coordinates must advance at f32 precision".into(),
                ));
            }
        }
        Ok(Self {
            descriptor: VolumeDescriptor {
                origin,
                spacing,
                dimensions,
                revision: 0,
            },
            values: vec![initial; count],
        })
    }

    pub fn descriptor(&self) -> VolumeDescriptor {
        self.descriptor
    }

    fn range(&self, start: usize, count: usize) -> Result<std::ops::Range<usize>, Error> {
        let end = start
            .checked_add(count)
            .filter(|end| *end <= self.values.len())
            .ok_or_else(|| Error("volume sample range is out of bounds".into()))?;
        Ok(start..end)
    }

    pub fn read(&self, start: usize, count: usize) -> Result<&[f32], Error> {
        Ok(&self.values[self.range(start, count)?])
    }

    pub fn write(&mut self, start: usize, samples: &[f32]) -> Result<(), Error> {
        let range = self.range(start, samples.len())?;
        super::finite(samples)?;
        let revision = self.next_revision()?;
        self.values[range].copy_from_slice(samples);
        self.descriptor.revision = revision;
        Ok(())
    }

    fn next_revision(&self) -> Result<u64, Error> {
        self.descriptor
            .revision
            .checked_add(1)
            .ok_or_else(|| Error("volume revision exhausted".into()))
    }

    /// Trilinear interpolation within the explicit sample domain. Outside is
    /// rejected; neither implicit caps nor an invented background value exist.
    pub fn sample(&self, position: [f32; 3]) -> Result<f32, Error> {
        super::finite(&position)?;
        let d = self.descriptor;
        let mut base = [0usize; 3];
        let mut fraction = [0f64; 3];
        for axis in 0..3 {
            let last = (d.dimensions[axis] - 1) as usize;
            let maximum = d.origin[axis] + last as f32 * d.spacing;
            if position[axis] < d.origin[axis] || position[axis] > maximum {
                return Err(Error(
                    "sample position lies outside the volume domain".into(),
                ));
            }
            let local = ((position[axis] as f64 - d.origin[axis] as f64) / d.spacing as f64)
                .clamp(0., last as f64);
            base[axis] = (local.floor() as usize).min(last - 1);
            fraction[axis] = local - base[axis] as f64;
        }
        let [nx, ny, _] = d.dimensions.map(|v| v as usize);
        let mut result = 0f64;
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    let index = ((base[2] + z) * ny + base[1] + y) * nx + base[0] + x;
                    let weight = [x, y, z]
                        .iter()
                        .enumerate()
                        .map(|(axis, v)| {
                            if *v == 0 {
                                1. - fraction[axis]
                            } else {
                                fraction[axis]
                            }
                        })
                        .product::<f64>();
                    result += self.values[index] as f64 * weight;
                }
            }
        }
        Ok(result as f32)
    }

    /// Evaluates an expression onto this lattice with one prepared evaluator.
    /// Commits only after every sample has been produced and checked.
    pub fn rasterize(&mut self, field: &Field, source: Node) -> Result<(), Error> {
        let revision = self.next_revision()?;
        let shape = JitShape::from(field.tree(source)?);
        let tape = shape.ez_float_slice_tape();
        let mut eval = JitShape::new_float_slice_eval();
        let d = self.descriptor;
        let [nx, ny, _] = d.dimensions.map(|v| v as usize);
        let mut values = Vec::with_capacity(self.values.len());
        let mut xs = Vec::with_capacity(EVALUATION_BATCH);
        let mut ys = Vec::with_capacity(EVALUATION_BATCH);
        let mut zs = Vec::with_capacity(EVALUATION_BATCH);
        while values.len() < self.values.len() {
            xs.clear();
            ys.clear();
            zs.clear();
            let start = values.len();
            let end = (start + EVALUATION_BATCH).min(self.values.len());
            for index in start..end {
                xs.push(d.origin[0] + (index % nx) as f32 * d.spacing);
                ys.push(d.origin[1] + ((index / nx) % ny) as f32 * d.spacing);
                zs.push(d.origin[2] + (index / (nx * ny)) as f32 * d.spacing);
            }
            let batch = eval
                .eval(&tape, &xs, &ys, &zs)
                .map_err(|e| Error(format!("volume field evaluation failed: {e}")))?;
            super::finite(batch)?;
            values.extend_from_slice(batch);
        }
        self.values = values;
        self.descriptor.revision = revision;
        Ok(())
    }

    pub fn generate(
        &self,
        isovalue: f32,
        max_vertices: u32,
        max_triangles: u32,
    ) -> Result<Geometry, Error> {
        let started = Instant::now();
        let d = self.descriptor;
        let payload = svc_mesh::mesh_scalar_samples(
            d.origin.map(f64::from),
            d.spacing as f64,
            d.dimensions.map(|v| v as usize),
            &self.values,
            isovalue,
            svc_mesh::SurfaceMeshLimits {
                max_vertices,
                max_indices: max_triangles
                    .checked_mul(3)
                    .ok_or_else(|| Error("triangle budget overflow".into()))?,
                ..Default::default()
            },
        )
        .map_err(|e| Error(format!("sampled volume extraction failed: {e:?}")))?;
        let positions: Vec<[f32; 3]> = payload
            .positions
            .as_chunks::<3>()
            .0
            .iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect();
        let triangles = payload
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        Ok(Geometry {
            positions,
            triangles,
            depth: 0,
            cell_size: [d.spacing; 3],
            generation_seconds: started.elapsed().as_secs_f64(),
            reoriented_triangles: 0,
            degenerate_triangles: 0,
            // Adaptive leaf recovery is a separate diagnostic from the uniform
            // mesher's general (including rank-deficient) QEF fallbacks.
            bounded_leaf_vertices: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lattice_reads_updates_and_interpolation_preserve_scalar_magnitudes() {
        let mut v = SampledVolume::new([10., 20., 30.], 2., [2, 2, 2], 0.).unwrap();
        v.write(0, &[-2., 6., -2., 6., -2., 6., -2., 6.]).unwrap();
        assert_eq!(v.sample([10.5, 21., 31.]).unwrap(), 0.);
        assert_eq!(v.sample([12., 22., 32.]).unwrap(), 6.);
        assert_eq!(v.descriptor().revision, 1);
        assert!(v.write(7, &[1., 2.]).is_err());
        assert!(v.write(0, &[f32::NAN]).is_err());
        assert_eq!(v.read(0, 2).unwrap(), &[-2., 6.]);
        assert_eq!(v.descriptor().revision, 1);
        assert!(v.sample([9., 20., 30.]).is_err());
    }
    #[test]
    fn rasterized_fields_are_owned_snapshots() {
        let mut f = Field::new();
        let s = f.sphere([0.; 3], 0.7).unwrap();
        let mut v = SampledVolume::new([-1.; 3], 0.25, [9; 3], 1.).unwrap();
        v.rasterize(&f, s).unwrap();
        drop(f);
        assert!((v.sample([0.; 3]).unwrap() + 0.7).abs() < 1e-5);
        let mesh = v.generate(0., 20000, 40000).unwrap();
        assert!(!mesh.triangles.is_empty());
        assert_eq!(mesh.topology(), super::super::TopologyReadout::default());
        let previous = v.read(0, 1).unwrap()[0];
        v.write(0, &[7.]).unwrap();
        assert_eq!(v.read(0, 1).unwrap()[0], 7.);
        assert_ne!(previous, 7.);
    }
    #[test]
    fn invalid_allocation_and_coordinates_fail_before_allocating() {
        assert!(SampledVolume::new([0.; 3], 1., [u32::MAX; 3], 0.).is_err());
        assert!(SampledVolume::new([0.; 3], 1., [1, 2, 2], 0.).is_err());
        assert!(SampledVolume::new([1e20; 3], 0.1, [2; 3], 0.).is_err());
    }
}
