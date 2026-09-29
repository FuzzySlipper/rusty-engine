//! Captured Doom room-study baseline shared by every #8796 candidate.
//!
//! `capture-fixture.py` records one fresh-attachment baseline from a running
//! room study: the presentation-world frame, the camera composition and the
//! content-addressed texture resources. This crate replays that frame into a
//! `PresentationWorld` and hands candidates the same baseline delta, camera,
//! decoded textures and neutral lighting the Three lane applies by default.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use render_model::{RenderDiff, RenderFrameDiff, TextureDescriptor, TexturePayloadSource};
use render_presentation::PresentationWorld;
use serde::Deserialize;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 720;

/// The Three lane's `createNeutralLights([5, 8, 6])` world rig.
pub mod neutral_light {
    pub const HEMISPHERE_SKY: [f32; 3] = [1.0, 1.0, 1.0];
    /// 0x263238 in linear RGB.
    pub const HEMISPHERE_GROUND: [f32; 3] = [0.0185, 0.0343, 0.0409];
    pub const HEMISPHERE_INTENSITY: f32 = 2.4;
    pub const KEY_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
    pub const KEY_INTENSITY: f32 = 2.2;
    /// Key light position; it shines toward the origin.
    pub const KEY_POSITION: [f32; 3] = [5.0, 8.0, 6.0];
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: [f32; 3],
    /// Engine yaw: zero faces -Z, positive yaw turns toward +X.
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub fov_y_degrees: f32,
    pub near: f32,
    pub far: f32,
}

pub struct Rgba8Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub struct Fixture {
    pub world: PresentationWorld,
    pub camera: Camera,
    resources: PathBuf,
}

impl Fixture {
    /// Default location written by `capture-fixture.py`.
    pub fn default_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("captured")
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let frame: RenderFrameDiff = read_json(&dir.join("world-frame.json"))?;
        let mut world = PresentationWorld::default();
        world
            .apply(frame)
            .map_err(|error| format!("captured frame rejected: {error}"))?;
        let view: ViewComposition = read_json(&dir.join("view.json"))?;
        let camera = view
            .cameras
            .into_iter()
            .next()
            .ok_or("view composition has no camera")?;
        Ok(Self {
            world,
            camera: Camera {
                position: camera.pose.position,
                yaw_degrees: camera.pose.yaw_degrees,
                pitch_degrees: camera.pose.pitch_degrees,
                fov_y_degrees: camera.projection.fov_y_degrees,
                near: camera.projection.near,
                far: camera.projection.far,
            },
            resources: dir.join("resources"),
        })
    }

    /// The baseline delta a fresh backend receives from `PresentationWorld`.
    pub fn baseline(&self) -> Vec<RenderDiff> {
        self.world.snapshot().frame.ops
    }

    /// Decode a retained texture's PNG payload to RGBA8.
    pub fn texture_pixels(&self, texture: &TextureDescriptor) -> Result<Rgba8Image, String> {
        let payload = texture
            .payload
            .as_ref()
            .ok_or_else(|| format!("{} has no payload", texture.id))?;
        let bytes = match &payload.source {
            TexturePayloadSource::Inline { encoded_bytes } => encoded_bytes.clone(),
            TexturePayloadSource::Resource { resource } => {
                let path = self.resources.join(resource.replace('/', "__"));
                fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?
            }
        };
        decode_png(&bytes)
    }
}

/// Engine camera pose to a right-handed world-to-view basis: returns the
/// forward and up vectors for a look-to matrix.
pub fn camera_forward_up(camera: &Camera) -> ([f32; 3], [f32; 3]) {
    let (yaw, pitch) = (
        camera.yaw_degrees.to_radians(),
        camera.pitch_degrees.to_radians(),
    );
    let forward = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let up = [
        -yaw.sin() * pitch.sin(),
        pitch.cos(),
        yaw.cos() * pitch.sin(),
    ];
    (forward, up)
}

pub fn decode_png(bytes: &[u8]) -> Result<Rgba8Image, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|error| error.to_string())?;
    pixels.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        other => return Err(format!("unsupported png color type {other:?}")),
    };
    Ok(Rgba8Image {
        width: info.width,
        height: info.height,
        pixels,
    })
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(rgba)
        .map_err(|error| error.to_string())
}

/// Counts of each baseline op kind, for the mirror-size record.
pub fn op_census(ops: &[RenderDiff]) -> BTreeMap<&'static str, usize> {
    let mut census = BTreeMap::new();
    for op in ops {
        let kind = match op {
            RenderDiff::DefineTexture { .. } => "defineTexture",
            RenderDiff::DefineMaterial { .. } => "defineMaterial",
            RenderDiff::DefineStaticMesh { .. } => "defineStaticMesh",
            RenderDiff::CreateStaticMeshInstance { .. } => "createStaticMeshInstance",
            RenderDiff::SetSkyBackground { .. } => "setSkyBackground",
            RenderDiff::DefineSpriteAtlas { .. } => "defineSpriteAtlas",
            RenderDiff::CreateSprite { .. } => "createSprite",
            _ => "other",
        };
        *census.entry(kind).or_default() += 1;
    }
    census
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

#[derive(Deserialize)]
struct ViewComposition {
    cameras: Vec<ViewCamera>,
}

#[derive(Deserialize)]
struct ViewCamera {
    pose: ViewPose,
    projection: ViewProjection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewPose {
    position: [f32; 3],
    pitch_degrees: f32,
    yaw_degrees: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewProjection {
    fov_y_degrees: f32,
    near: f32,
    far: f32,
}
