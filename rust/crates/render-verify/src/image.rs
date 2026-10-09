//! Eight-bit RGBA images read from and written to PNG, and the measures the
//! lane compares two renders by.

use std::{fs::File, io::BufWriter, path::Path};

/// An 8-bit RGBA image, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, fill: [u8; 4]) -> Self {
        let rgba = fill
            .iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect();
        Self {
            width,
            height,
            rgba,
        }
    }

    /// Reads an 8-bit greyscale, RGB or RGBA PNG.
    pub fn read_png(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder
            .read_info()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let size = reader
            .output_buffer_size()
            .ok_or_else(|| format!("{}: image too large", path.display()))?;
        let mut buffer = vec![0; size];
        let info = reader
            .next_frame(&mut buffer)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        buffer.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => buffer,
            png::ColorType::Rgb => buffer
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect(),
            png::ColorType::Grayscale => buffer.iter().flat_map(|&v| [v, v, v, 255]).collect(),
            png::ColorType::GrayscaleAlpha => buffer
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                .collect(),
            png::ColorType::Indexed => {
                return Err(format!(
                    "{}: indexed colour after expansion",
                    path.display()
                ))
            }
        };
        Ok(Self {
            width: info.width,
            height: info.height,
            rgba,
        })
    }

    pub fn write_png(&self, path: &Path) -> Result<(), String> {
        let file = File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, value: [u8; 4]) {
        let at = (y as usize * self.width as usize + x as usize) * 4;
        self.rgba[at..at + 4].copy_from_slice(&value);
    }

    /// Box-filtered to `width` (height in proportion), for sheets.
    pub fn shrunk_to_width(&self, width: u32) -> Self {
        if width >= self.width {
            return self.clone();
        }
        let height = ((self.height as u64 * width as u64) / self.width as u64).max(1) as u32;
        let mut out = Self::new(width, height, [0, 0, 0, 255]);
        for y in 0..height {
            let y0 = y as u64 * self.height as u64 / height as u64;
            let y1 = ((y as u64 + 1) * self.height as u64 / height as u64).max(y0 + 1);
            for x in 0..width {
                let x0 = x as u64 * self.width as u64 / width as u64;
                let x1 = ((x as u64 + 1) * self.width as u64 / width as u64).max(x0 + 1);
                let mut sum = [0u64; 4];
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        let pixel = self.pixel(sx as u32, sy as u32);
                        for (total, value) in sum.iter_mut().zip(pixel) {
                            *total += value as u64;
                        }
                    }
                }
                let count = (y1 - y0) * (x1 - x0);
                out.set_pixel(x, y, sum.map(|total| (total / count) as u8));
            }
        }
        out
    }

    /// Copies `other` in with its top-left corner at (x, y), clipped.
    pub fn blit(&mut self, other: &Image, x: u32, y: u32) {
        for sy in 0..other.height.min(self.height.saturating_sub(y)) {
            for sx in 0..other.width.min(self.width.saturating_sub(x)) {
                self.set_pixel(x + sx, y + sy, other.pixel(sx, sy));
            }
        }
    }
}

/// How two renders of one scene differ.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDiff {
    /// The sizes differ; nothing else was measured.
    pub size_mismatch: bool,
    /// The largest difference in any colour channel, 0 to 255.
    pub max_channel: u8,
    /// The mean absolute difference over the colour channels, in levels.
    pub mean_abs: f64,
    /// The share of pixels with some colour channel more than the threshold
    /// apart (0 to 1).
    pub changed_share: f64,
    /// The threshold `changed_share` counts above, in levels.
    pub changed_above: u8,
    /// Mean structural similarity of luminance over 8×8 windows (1 is
    /// identical).
    pub ssim: f64,
}

impl ImageDiff {
    pub fn identical(&self) -> bool {
        !self.size_mismatch && self.max_channel == 0
    }
}

/// Compares `candidate` with `baseline`. Alpha is ignored: the renders are
/// opaque.
pub fn compare(baseline: &Image, candidate: &Image, changed_above: u8) -> ImageDiff {
    if baseline.width != candidate.width || baseline.height != candidate.height {
        return ImageDiff {
            size_mismatch: true,
            max_channel: 255,
            mean_abs: 255.0,
            changed_share: 1.0,
            changed_above,
            ssim: 0.0,
        };
    }
    let mut max_channel = 0u8;
    let mut total = 0u64;
    let mut changed = 0u64;
    for (a, b) in baseline
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(candidate.rgba.as_chunks::<4>().0)
    {
        let mut pixel_max = 0u8;
        for channel in 0..3 {
            let difference = a[channel].abs_diff(b[channel]);
            pixel_max = pixel_max.max(difference);
            total += difference as u64;
        }
        max_channel = max_channel.max(pixel_max);
        if pixel_max > changed_above {
            changed += 1;
        }
    }
    let pixels = (baseline.width as u64 * baseline.height as u64).max(1);
    ImageDiff {
        size_mismatch: false,
        max_channel,
        mean_abs: total as f64 / (pixels * 3) as f64,
        changed_share: changed as f64 / pixels as f64,
        changed_above,
        ssim: ssim(baseline, candidate),
    }
}

fn luminance(image: &Image) -> Vec<f64> {
    image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| 0.2126 * pixel[0] as f64 + 0.7152 * pixel[1] as f64 + 0.0722 * pixel[2] as f64)
        .collect()
}

/// Mean SSIM over 8×8 windows stepped by 4, on luminance in levels.
fn ssim(baseline: &Image, candidate: &Image) -> f64 {
    const WINDOW: usize = 8;
    const STEP: usize = 4;
    const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
    const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);
    let (width, height) = (baseline.width as usize, baseline.height as usize);
    if width < WINDOW || height < WINDOW {
        return if baseline == candidate { 1.0 } else { 0.0 };
    }
    let (a, b) = (luminance(baseline), luminance(candidate));
    let mut total = 0.0;
    let mut windows = 0u64;
    let count = (WINDOW * WINDOW) as f64;
    for y in (0..=height - WINDOW).step_by(STEP) {
        for x in (0..=width - WINDOW).step_by(STEP) {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for row in y..y + WINDOW {
                for column in x..x + WINDOW {
                    let (va, vb) = (a[row * width + column], b[row * width + column]);
                    sa += va;
                    sb += vb;
                    saa += va * va;
                    sbb += vb * vb;
                    sab += va * vb;
                }
            }
            let (ma, mb) = (sa / count, sb / count);
            let (va, vb) = (saa / count - ma * ma, sbb / count - mb * mb);
            let cov = sab / count - ma * mb;
            total += ((2.0 * ma * mb + C1) * (2.0 * cov + C2))
                / ((ma * ma + mb * mb + C1) * (va + vb + C2));
            windows += 1;
        }
    }
    total / windows as f64
}

/// Where two renders differ: the baseline's luminance as a dim grey, with
/// differences drawn over it from dark red (a level or two) through yellow to
/// white (32 levels or more).
pub fn heat_map(baseline: &Image, candidate: &Image) -> Image {
    let width = baseline.width.min(candidate.width);
    let height = baseline.height.min(candidate.height);
    let mut out = Image::new(width, height, [0, 0, 0, 255]);
    for y in 0..height {
        for x in 0..width {
            let (a, b) = (baseline.pixel(x, y), candidate.pixel(x, y));
            let difference = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0);
            let value = if difference == 0 {
                let grey = ((a[0] as u32 * 54 + a[1] as u32 * 183 + a[2] as u32 * 19) >> 8) / 4;
                [grey as u8, grey as u8, grey as u8, 255]
            } else {
                let t = (difference as f32 / 32.0).min(1.0);
                let red = 96.0 + 159.0 * (t * 2.0).min(1.0);
                let green = 255.0 * ((t - 0.25) / 0.5).clamp(0.0, 1.0);
                let blue = 255.0 * ((t - 0.75) / 0.25).clamp(0.0, 1.0);
                [red as u8, green as u8, blue as u8, 255]
            };
            out.set_pixel(x, y, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: u32, height: u32) -> Image {
        let mut image = Image::new(width, height, [0, 0, 0, 255]);
        for y in 0..height {
            for x in 0..width {
                image.set_pixel(
                    x,
                    y,
                    [(x * 255 / width) as u8, (y * 255 / height) as u8, 90, 255],
                );
            }
        }
        image
    }

    #[test]
    fn identical_renders_compare_identical() {
        let image = gradient(64, 48);
        let diff = compare(&image, &image.clone(), 2);
        assert!(diff.identical());
        assert_eq!(diff.changed_share, 0.0);
        assert!((diff.ssim - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_changed_patch_is_counted_and_lowers_similarity() {
        let baseline = gradient(64, 48);
        let mut candidate = baseline.clone();
        for y in 0..12 {
            for x in 0..16 {
                candidate.set_pixel(x, y, [255, 255, 255, 255]);
            }
        }
        let diff = compare(&baseline, &candidate, 2);
        assert!(!diff.identical());
        assert!((diff.changed_share - (16.0 * 12.0) / (64.0 * 48.0)).abs() < 1e-3);
        assert!(diff.ssim < 0.95);
        let heat = heat_map(&baseline, &candidate);
        assert_eq!(heat.pixel(0, 0), [255, 255, 255, 255]);
        assert_ne!(heat.pixel(40, 40)[0], 255);
    }

    #[test]
    fn a_one_level_change_stays_under_the_threshold() {
        let baseline = gradient(32, 32);
        let mut candidate = baseline.clone();
        let pixel = candidate.pixel(5, 5);
        candidate.set_pixel(5, 5, [pixel[0] + 1, pixel[1], pixel[2], 255]);
        let diff = compare(&baseline, &candidate, 2);
        assert_eq!(diff.max_channel, 1);
        assert_eq!(diff.changed_share, 0.0);
    }

    #[test]
    fn sizes_that_differ_are_reported() {
        let diff = compare(&gradient(32, 32), &gradient(16, 32), 2);
        assert!(diff.size_mismatch);
    }

    #[test]
    fn png_round_trips() {
        let image = gradient(20, 10);
        let path = std::env::temp_dir().join(format!("render-verify-{}.png", std::process::id()));
        image.write_png(&path).unwrap();
        let read = Image::read_png(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(read, image);
        assert_eq!(image.shrunk_to_width(10).width, 10);
    }
}
