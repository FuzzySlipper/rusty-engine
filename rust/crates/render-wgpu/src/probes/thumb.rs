//! A texture's colour reduced to a small grid, for the probe bake's albedo.

use std::collections::HashMap;

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
    thumbs: &HashMap<String, Thumb>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(thumb.mean([0, 0], [32, 32]), [1.0, 1.0, 0.0]);
        assert_eq!(thumb.mean([32, 32], [32, 32]), [0.0, 0.0, 0.0]);
        let all = thumb.mean_all();
        assert!((all[0] - 0.5).abs() < 1e-5 && (all[1] - 0.5).abs() < 1e-5);
    }
}
