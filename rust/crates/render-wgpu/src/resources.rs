//! Content-addressed bytes and their decoding. The runtime already admitted
//! and hashed every resource; this module only reads the bytes it is given.

use std::borrow::Cow;

use render_model::{
    MeshPayloadDescriptor, MeshPayloadSource, TextureDescriptor, TexturePayloadSource,
};

/// Where the renderer reads resource bytes named by retained descriptors
/// (`texture-resource/…`, mesh resources). The runtime implements it over its
/// admitted render-output resources; tests implement it over files.
pub trait ResourceSource {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>>;
}

/// A source with no resources, for scenes that carry everything inline.
pub struct NoResources;

impl ResourceSource for NoResources {
    fn bytes(&self, _identity: &str) -> Option<Cow<'_, [u8]>> {
        None
    }
}

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub(crate) fn texture_image(
    texture: &TextureDescriptor,
    resources: &dyn ResourceSource,
) -> Result<Option<DecodedImage>, String> {
    let Some(payload) = &texture.payload else {
        // Metadata-only legacy descriptor: realized as the colour fallback.
        return Ok(None);
    };
    let bytes = match &payload.source {
        TexturePayloadSource::Inline { encoded_bytes } => Cow::Borrowed(encoded_bytes.as_slice()),
        TexturePayloadSource::Resource { resource } => resources
            .bytes(resource)
            .ok_or_else(|| format!("resource {resource} is not available"))?,
    };
    decode_png(&bytes).map(Some)
}

pub(crate) fn decode_png(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let size = reader.output_buffer_size().ok_or("png is too large")?;
    let mut pixels = vec![0; size];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|error| error.to_string())?;
    pixels.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => expand(&pixels, 3, |p| [p[0], p[1], p[2], 255]),
        png::ColorType::GrayscaleAlpha => expand(&pixels, 2, |p| [p[0], p[0], p[0], p[1]]),
        png::ColorType::Grayscale => expand(&pixels, 1, |p| [p[0], p[0], p[0], 255]),
        png::ColorType::Indexed => return Err("indexed png was not expanded".to_owned()),
    };
    Ok(DecodedImage {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn expand(pixels: &[u8], stride: usize, to_rgba: impl Fn(&[u8]) -> [u8; 4]) -> Vec<u8> {
    pixels.chunks_exact(stride).flat_map(to_rgba).collect()
}

pub(crate) fn decode_jpeg(bytes: &[u8]) -> Result<DecodedImage, String> {
    use jpeg_decoder::PixelFormat;
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().map_err(|error| error.to_string())?;
    let info = decoder.info().ok_or("jpeg has no image information")?;
    let rgba = match info.pixel_format {
        PixelFormat::RGB24 => expand(&pixels, 3, |p| [p[0], p[1], p[2], 255]),
        PixelFormat::L8 => expand(&pixels, 1, |p| [p[0], p[0], p[0], 255]),
        PixelFormat::L16 => expand(&pixels, 2, |p| {
            let luminance = (u16::from_ne_bytes([p[0], p[1]]) >> 8) as u8;
            [luminance, luminance, luminance, 255]
        }),
        PixelFormat::CMYK32 => expand(&pixels, 4, |p| {
            // The decoder returns inverted CMYK components, including K.
            let channel = |c: u8| (u16::from(c) * u16::from(p[3]) / 255) as u8;
            [channel(p[0]), channel(p[1]), channel(p[2]), 255]
        }),
    };
    Ok(DecodedImage {
        width: u32::from(info.width),
        height: u32::from(info.height),
        rgba,
    })
}

/// Every mip level of an RGBA8 image after the first, smallest last: each a
/// 2×2 box filter of the one above (odd edges repeat their last texel), in
/// linear light for sRGB images. Returns the level count and the pixels of
/// all levels, base first, as one texture upload expects them.
pub(crate) fn mip_chain(width: u32, height: u32, rgba: &[u8], srgb: bool) -> (u32, Vec<u8>) {
    let tables = srgb.then(srgb_tables);
    let levels = 32 - width.max(height).max(1).leading_zeros();
    let mut all = rgba.to_vec();
    let (mut source, mut w, mut h) = (0usize, width as usize, height as usize);
    for _ in 1..levels {
        let (next_w, next_h) = ((w / 2).max(1), (h / 2).max(1));
        let start = all.len();
        all.resize(start + next_w * next_h * 4, 0);
        let (above, below) = all.split_at_mut(start);
        let above = &above[source..];
        let row = |y: usize| &above[y.min(h - 1) * w * 4..][..w * 4];
        for (y, out) in below.chunks_exact_mut(next_w * 4).enumerate() {
            let (top, bottom) = (row(2 * y), row(2 * y + 1));
            for (x, texel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (left, right) = (2 * x * 4, (2 * x + 1).min(w - 1) * 4);
                let corners = [
                    &top[left..left + 4],
                    &top[right..right + 4],
                    &bottom[left..left + 4],
                    &bottom[right..right + 4],
                ];
                // Colour averages in linear light; alpha as stored.
                let channels = match tables {
                    Some(_) => 3,
                    None => 0,
                };
                if let Some((decode, encode)) = tables {
                    for channel in 0..3 {
                        let linear: u32 = corners
                            .iter()
                            .map(|corner| u32::from(decode[usize::from(corner[channel])]))
                            .sum();
                        texel[channel] = encode[((linear + 2) / 4) as usize];
                    }
                }
                for channel in channels..4 {
                    let sum: u32 = corners
                        .iter()
                        .map(|corner| u32::from(corner[channel]))
                        .sum();
                    texel[channel] = ((sum + 2) / 4) as u8;
                }
            }
        }
        (source, w, h) = (start, next_w, next_h);
    }
    (levels, all)
}

/// sRGB bytes to linear light in 65536 steps, and those steps back to the
/// nearest sRGB byte: built once, so a mip level costs integer lookups.
fn srgb_tables() -> &'static ([u16; 256], Box<[u8]>) {
    static TABLES: std::sync::OnceLock<([u16; 256], Box<[u8]>)> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let decode = std::array::from_fn(|value| {
            let c = value as f32 / 255.0;
            let linear = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            (linear * 65535.0 + 0.5) as u16
        });
        let encode = (0..=u16::MAX)
            .map(|step| {
                let linear = f32::from(step) / 65535.0;
                let c = if linear <= 0.0031308 {
                    linear * 12.92
                } else {
                    1.055 * linear.powf(1.0 / 2.4) - 0.055
                };
                (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            })
            .collect();
        (decode, encode)
    })
}

/// A lossy or lossless WebP image (GLB `EXT_texture_webp`), as RGBA.
pub(crate) fn decode_webp(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|error| error.to_string())?;
    let (width, height) = decoder.dimensions();
    let size = decoder.output_buffer_size().ok_or("webp is too large")?;
    let mut pixels = vec![0; size];
    decoder
        .read_image(&mut pixels)
        .map_err(|error| error.to_string())?;
    let rgba = if decoder.has_alpha() {
        pixels
    } else {
        expand(&pixels, 3, |p| [p[0], p[1], p[2], 255])
    };
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

/// Vertex streams ready for upload. Colours (RGBA) are drawn only for static
/// meshes; the other families clear them before upload.
pub struct MeshStreams {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Option<Vec<f32>>,
    pub colors: Option<Vec<f32>>,
    pub indices: Vec<u32>,
}

pub fn mesh_streams(
    payload: &MeshPayloadDescriptor,
    resources: &dyn ResourceSource,
) -> Result<MeshStreams, String> {
    match &payload.source {
        MeshPayloadSource::Inline {
            positions,
            normals,
            uvs,
            colors,
            indices,
        } => Ok(MeshStreams {
            positions: positions.clone(),
            normals: normals.clone(),
            uvs: uvs.clone(),
            colors: colors.clone(),
            indices: indices.clone(),
        }),
        MeshPayloadSource::Resource {
            resource,
            positions_byte_offset,
            normals_byte_offset,
            uvs_byte_offset,
            colors_byte_offset,
            indices_byte_offset,
            ..
        } => {
            let bytes = resources
                .bytes(resource)
                .ok_or_else(|| format!("resource {resource} is not available"))?;
            let vertices = payload.layout.vertex_count as usize;
            Ok(MeshStreams {
                positions: f32_stream(&bytes, *positions_byte_offset, vertices * 3)?,
                normals: f32_stream(&bytes, *normals_byte_offset, vertices * 3)?,
                uvs: uvs_byte_offset
                    .map(|offset| f32_stream(&bytes, offset, vertices * 2))
                    .transpose()?,
                colors: colors_byte_offset
                    .map(|offset| f32_stream(&bytes, offset, vertices * 4))
                    .transpose()?,
                indices: u32_stream(
                    &bytes,
                    *indices_byte_offset,
                    payload.layout.index_count as usize,
                )?,
            })
        }
    }
}

fn stream(bytes: &[u8], offset: u32, count: usize) -> Result<&[u8], String> {
    let start = offset as usize;
    bytes
        .get(start..start + count * 4)
        .ok_or_else(|| format!("stream at {offset} ({count} values) is outside the resource"))
}

fn f32_stream(bytes: &[u8], offset: u32, count: usize) -> Result<Vec<f32>, String> {
    Ok(stream(bytes, offset, count)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|value| f32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        .collect())
}

fn u32_stream(bytes: &[u8], offset: u32, count: usize) -> Result<Vec<u32>, String> {
    Ok(stream(bytes, offset, count)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        .collect())
}

/// Encode tightly packed sRGB RGBA8 rows (as
/// [`crate::OffscreenTarget::read_rgba`] returns them) as a PNG marked sRGB,
/// as output captures and screenshots are.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(rgba)
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

/// Decode a PNG to tightly packed RGBA8 rows: `(width, height, rgba)`.
pub fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    decode_png(bytes).map(|image| (image.width, image.height, image.rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_levels_average_in_linear_light_down_to_one_texel() {
        // A 4×2 sRGB black/white checker: levels 4×2, 2×1, 1×1.
        let checker: Vec<u8> = (0..8)
            .flat_map(|texel| {
                if (texel % 4 + texel / 4) % 2 == 0 {
                    [255; 4]
                } else {
                    [0, 0, 0, 255]
                }
            })
            .collect();
        let (levels, pixels) = mip_chain(4, 2, &checker, true);
        assert_eq!(levels, 3);
        assert_eq!(pixels.len(), (8 + 2 + 1) * 4);
        // Half white in linear light is sRGB 188, not 128.
        assert!(pixels[32..]
            .chunks(4)
            .all(|texel| texel == [188, 188, 188, 255]));
        let (_, linear) = mip_chain(4, 2, &checker, false);
        assert!(linear[32..]
            .chunks(4)
            .all(|texel| texel == [128, 128, 128, 255]));
        // An odd edge repeats its last texel.
        let (levels, pixels) = mip_chain(
            3,
            1,
            &[10, 10, 10, 255, 20, 20, 20, 255, 40, 40, 40, 255],
            false,
        );
        assert_eq!(levels, 2);
        assert_eq!(&pixels[12..], [15, 15, 15, 255]);
    }
}
