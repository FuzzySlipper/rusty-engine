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

pub(crate) struct DecodedImage {
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

/// Vertex streams ready for upload. Colours (RGBA) are drawn only for static
/// meshes; the other families clear them before upload.
pub(crate) struct MeshStreams {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Option<Vec<f32>>,
    pub colors: Option<Vec<f32>>,
    pub indices: Vec<u32>,
}

pub(crate) fn mesh_streams(
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
