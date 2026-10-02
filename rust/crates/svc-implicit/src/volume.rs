//! Retained scalar lattice points, independent of block-voxel occupancy.
//! Negative values are inside. Samples are x-fastest; spacing is uniform.
//!
//! Each sample may also carry a material index; a dual-contoured face takes
//! the index of its edge's inside sample. The volume remembers which samples
//! changed, so a product meshing it in blocks regenerates only the blocks a
//! write reaches.
use super::{Error, Field, Geometry, Node};
use fidget::{jit::JitShape, shape::EzShape};
use std::collections::BTreeSet;
use std::time::Instant;
use svc_mesh::{ScalarRegion, ScalarVolume, SurfaceMaterials, SurfaceMeshLimits};

/// The sample limit when a volume is created without its own.
pub const DEFAULT_MAX_SAMPLES: usize = 8_000_000;
/// Samples are addressed by `u32` indices.
pub const MAX_SAMPLES: usize = u32::MAX as usize;
const EVALUATION_BATCH: usize = 32_768;
/// A changed sample moves the cell vertices of the cells around it, and the
/// quads and border normals within two more samples.
const BLOCK_DIRTY_MARGIN: usize = 3;
/// Beyond this many separate changed boxes, they merge into one.
const MAX_DIRTY_BOXES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeDescriptor {
    pub origin: [f32; 3],
    pub spacing: f32,
    pub dimensions: [u32; 3],
    pub revision: u64,
}

/// An inclusive box of sample coordinates.
type SampleBox = ([usize; 3], [usize; 3]);

#[derive(Clone, Debug)]
pub struct SampledVolume {
    descriptor: VolumeDescriptor,
    values: Vec<f32>,
    /// A material index per sample, absent until one is written (all zero).
    materials: Option<Vec<u16>>,
    /// Sample boxes changed since dirty blocks were last read; `None` means
    /// every sample.
    dirty: Option<Vec<SampleBox>>,
}

/// One extraction of a sampled volume.
#[derive(Clone, Copy, Debug)]
pub struct VolumeGenerateOptions<'a> {
    pub isovalue: f32,
    /// Surface character by material index; absent indices use the default.
    pub materials: &'a SurfaceMaterials,
    /// The owned samples of one block, or `None` for the whole volume.
    pub region: Option<ScalarRegion>,
    pub limits: SurfaceMeshLimits,
}

impl SampledVolume {
    pub fn new(
        origin: [f32; 3],
        spacing: f32,
        dimensions: [u32; 3],
        initial: f32,
        max_samples: usize,
    ) -> Result<Self, Error> {
        super::finite(&origin)?;
        super::positive(spacing, "volume spacing")?;
        if !initial.is_finite() || dimensions.iter().any(|d| *d < 2) {
            return Err(Error(
                "volume needs finite samples and at least two lattice points per axis".into(),
            ));
        }
        let limit = max_samples.min(MAX_SAMPLES);
        let count = dimensions
            .iter()
            .try_fold(1usize, |n, d| n.checked_mul(*d as usize))
            .filter(|n| *n <= limit)
            .ok_or_else(|| Error(format!("volume exceeds {limit} retained samples")))?;
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
            materials: None,
            dirty: None,
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
        self.values[range.clone()].copy_from_slice(samples);
        self.descriptor.revision = revision;
        self.mark_range(range);
        Ok(())
    }

    /// Material indices of a sample range; zero where none was written.
    pub fn read_materials(&self, start: usize, count: usize) -> Result<Vec<u16>, Error> {
        let range = self.range(start, count)?;
        Ok(match &self.materials {
            Some(materials) => materials[range].to_vec(),
            None => vec![0; range.len()],
        })
    }

    pub fn write_materials(&mut self, start: usize, materials: &[u16]) -> Result<(), Error> {
        let range = self.range(start, materials.len())?;
        let revision = self.next_revision()?;
        let count = self.values.len();
        self.materials.get_or_insert_with(|| vec![0; count])[range.clone()]
            .copy_from_slice(materials);
        self.descriptor.revision = revision;
        self.mark_range(range);
        Ok(())
    }

    fn next_revision(&self) -> Result<u64, Error> {
        self.descriptor
            .revision
            .checked_add(1)
            .ok_or_else(|| Error("volume revision exhausted".into()))
    }

    fn coordinate(&self, index: usize) -> [usize; 3] {
        let [nx, ny, _] = self.descriptor.dimensions.map(|v| v as usize);
        [index % nx, (index / nx) % ny, index / (nx * ny)]
    }

    /// Record a changed x-fastest range as the smallest box holding it.
    fn mark_range(&mut self, range: std::ops::Range<usize>) {
        if range.is_empty() {
            return;
        }
        let first = self.coordinate(range.start);
        let last = self.coordinate(range.end - 1);
        let [nx, ny, _] = self.descriptor.dimensions.map(|v| v as usize);
        let changed = if first[2] == last[2] && first[1] == last[1] {
            (first, last)
        } else if first[2] == last[2] {
            ([0, first[1], first[2]], [nx - 1, last[1], last[2]])
        } else {
            ([0, 0, first[2]], [nx - 1, ny - 1, last[2]])
        };
        self.mark(changed);
    }

    fn mark(&mut self, changed: SampleBox) {
        let Some(boxes) = &mut self.dirty else {
            return;
        };
        boxes.push(changed);
        if boxes.len() > MAX_DIRTY_BOXES {
            let merged = boxes.iter().fold(changed, |(low, high), (min, max)| {
                (
                    std::array::from_fn(|axis| low[axis].min(min[axis])),
                    std::array::from_fn(|axis| high[axis].max(max[axis])),
                )
            });
            *boxes = vec![merged];
        }
    }

    fn mark_all(&mut self) {
        self.dirty = None;
    }

    /// The blocks of `block_samples` owned samples per axis whose surface
    /// changed since the last call, in z, y, x order, and forget them. A new
    /// volume, a rasterized one, and a volume first read with a block layout
    /// report every block.
    pub fn take_dirty_blocks(&mut self, block_samples: u32) -> Result<Vec<[u32; 3]>, Error> {
        if block_samples == 0 {
            return Err(Error("blocks need at least one sample per axis".into()));
        }
        let block = block_samples as usize;
        let dims = self.descriptor.dimensions.map(|v| v as usize);
        let blocks = dims.map(|d| d.div_ceil(block));
        let mut dirty = BTreeSet::new();
        match self.dirty.take() {
            None => {
                for z in 0..blocks[2] {
                    for y in 0..blocks[1] {
                        for x in 0..blocks[0] {
                            dirty.insert([z as u32, y as u32, x as u32]);
                        }
                    }
                }
            }
            Some(boxes) => {
                for (low, high) in boxes {
                    let first: [usize; 3] =
                        std::array::from_fn(|a| low[a].saturating_sub(BLOCK_DIRTY_MARGIN) / block);
                    let last: [usize; 3] = std::array::from_fn(|a| {
                        ((high[a] + BLOCK_DIRTY_MARGIN).min(dims[a] - 1)) / block
                    });
                    for z in first[2]..=last[2] {
                        for y in first[1]..=last[1] {
                            for x in first[0]..=last[0] {
                                dirty.insert([z as u32, y as u32, x as u32]);
                            }
                        }
                    }
                }
            }
        }
        self.dirty = Some(Vec::new());
        Ok(dirty.into_iter().map(|[z, y, x]| [x, y, z]).collect())
    }

    /// The owned samples of one block, or an error outside the volume.
    pub fn block_region(&self, block_samples: u32, block: [u32; 3]) -> Result<ScalarRegion, Error> {
        if block_samples == 0 {
            return Err(Error("blocks need at least one sample per axis".into()));
        }
        let dims = self.descriptor.dimensions.map(|v| v as usize);
        let size = block_samples as usize;
        let min: [usize; 3] = std::array::from_fn(|axis| block[axis] as usize * size);
        if (0..3).any(|axis| min[axis] >= dims[axis]) {
            return Err(Error("block lies outside the volume".into()));
        }
        Ok(ScalarRegion {
            min,
            max: std::array::from_fn(|axis| (min[axis] + size).min(dims[axis])),
        })
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

    /// Evaluate an expression at every lattice point with one prepared
    /// evaluator, in batches.
    fn evaluate(&self, field: &Field, source: Node) -> Result<Vec<f32>, Error> {
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
        Ok(values)
    }

    /// Evaluates an expression onto this lattice with one prepared evaluator.
    /// Commits only after every sample has been produced and checked.
    pub fn rasterize(&mut self, field: &Field, source: Node) -> Result<(), Error> {
        let revision = self.next_revision()?;
        self.values = self.evaluate(field, source)?;
        self.descriptor.revision = revision;
        self.mark_all();
        Ok(())
    }

    /// Give every sample where `source` is at or below zero the material
    /// `index`. Densities are unchanged.
    pub fn paint(&mut self, field: &Field, source: Node, index: u16) -> Result<(), Error> {
        let revision = self.next_revision()?;
        let inside = self.evaluate(field, source)?;
        let count = self.values.len();
        let mut changed: Option<SampleBox> = None;
        let materials = self.materials.get_or_insert_with(|| vec![0; count]);
        for (sample, value) in inside.iter().enumerate() {
            if *value <= 0.0 && materials[sample] != index {
                materials[sample] = index;
                let [nx, ny, _] = self.descriptor.dimensions.map(|v| v as usize);
                let at = [sample % nx, (sample / nx) % ny, sample / (nx * ny)];
                changed = Some(match changed {
                    None => (at, at),
                    Some((low, high)) => (
                        std::array::from_fn(|axis| low[axis].min(at[axis])),
                        std::array::from_fn(|axis| high[axis].max(at[axis])),
                    ),
                });
            }
        }
        self.descriptor.revision = revision;
        if let Some(changed) = changed {
            self.mark(changed);
        }
        Ok(())
    }

    /// Dual-contour the whole volume or one region of it. Each triangle's
    /// slot is the material index of its edge's inside sample.
    pub fn generate(&self, options: VolumeGenerateOptions<'_>) -> Result<Geometry, Error> {
        let started = Instant::now();
        let d = self.descriptor;
        let surface = svc_mesh::mesh_scalar_surface(
            ScalarVolume {
                origin: d.origin.map(f64::from),
                spacing: d.spacing as f64,
                dimensions: d.dimensions.map(|v| v as usize),
                samples: &self.values,
                materials: self.materials.as_deref(),
                isovalue: options.isovalue,
            },
            options.materials,
            options.region,
            options.limits,
        )
        .map_err(|e| Error(format!("sampled volume extraction failed: {e:?}")))?;
        let has_halo = surface.halo.iter().any(|halo| *halo);
        Ok(Geometry {
            positions: surface.positions,
            triangles: surface.triangles,
            slots: surface.slots.into_iter().map(u32::from).collect(),
            halo: if has_halo { surface.halo } else { Vec::new() },
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

    fn whole<'a>(materials: &'a SurfaceMaterials, max: u32) -> VolumeGenerateOptions<'a> {
        VolumeGenerateOptions {
            isovalue: 0.0,
            materials,
            region: None,
            limits: SurfaceMeshLimits {
                max_vertices: max,
                max_indices: max * 6,
                ..SurfaceMeshLimits::default()
            },
        }
    }

    #[test]
    fn lattice_reads_updates_and_interpolation_preserve_scalar_magnitudes() {
        let mut v =
            SampledVolume::new([10., 20., 30.], 2., [2, 2, 2], 0., DEFAULT_MAX_SAMPLES).unwrap();
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
        let mut v = SampledVolume::new([-1.; 3], 0.25, [9; 3], 1., DEFAULT_MAX_SAMPLES).unwrap();
        v.rasterize(&f, s).unwrap();
        drop(f);
        assert!((v.sample([0.; 3]).unwrap() + 0.7).abs() < 1e-5);
        let materials = SurfaceMaterials::default();
        let mesh = v.generate(whole(&materials, 20000)).unwrap();
        assert!(!mesh.triangles.is_empty());
        assert!(mesh.slots.iter().all(|slot| *slot == 0));
        assert_eq!(mesh.topology(), super::super::TopologyReadout::default());
        let previous = v.read(0, 1).unwrap()[0];
        v.write(0, &[7.]).unwrap();
        assert_eq!(v.read(0, 1).unwrap()[0], 7.);
        assert_ne!(previous, 7.);
    }
    #[test]
    fn invalid_allocation_and_coordinates_fail_before_allocating() {
        assert!(SampledVolume::new([0.; 3], 1., [u32::MAX; 3], 0., DEFAULT_MAX_SAMPLES).is_err());
        assert!(SampledVolume::new([0.; 3], 1., [1, 2, 2], 0., DEFAULT_MAX_SAMPLES).is_err());
        assert!(SampledVolume::new([1e20; 3], 0.1, [2; 3], 0., DEFAULT_MAX_SAMPLES).is_err());
        assert!(SampledVolume::new([0.; 3], 1., [8, 8, 8], 0., 500).is_err());
        assert!(SampledVolume::new([0.; 3], 1., [8, 8, 8], 0., 512).is_ok());
    }

    #[test]
    fn writes_report_the_blocks_their_surface_reaches_once() {
        let mut v = SampledVolume::new([0.; 3], 1., [64, 64, 64], 1., DEFAULT_MAX_SAMPLES).unwrap();
        assert_eq!(v.take_dirty_blocks(16).unwrap().len(), 64);
        assert!(v.take_dirty_blocks(16).unwrap().is_empty());
        // One sample in the middle of block (1, 1, 1).
        let index = (24 * 64 + 24) * 64 + 24;
        v.write(index, &[-1.]).unwrap();
        assert_eq!(v.take_dirty_blocks(16).unwrap(), vec![[1, 1, 1]]);
        // A sample at a block corner reaches the blocks around it.
        let corner = (32 * 64 + 32) * 64 + 32;
        v.write_materials(corner, &[3]).unwrap();
        assert_eq!(v.take_dirty_blocks(16).unwrap().len(), 8);
        assert_eq!(v.read_materials(corner, 1).unwrap(), vec![3]);
    }

    #[test]
    fn a_block_region_is_its_owned_samples_clipped_to_the_volume() {
        let v = SampledVolume::new([0.; 3], 1., [40, 20, 10], 1., DEFAULT_MAX_SAMPLES).unwrap();
        let region = v.block_region(16, [2, 1, 0]).unwrap();
        assert_eq!(region.min, [32, 16, 0]);
        assert_eq!(region.max, [40, 20, 10]);
        assert!(v.block_region(16, [3, 0, 0]).is_err());
    }
}
