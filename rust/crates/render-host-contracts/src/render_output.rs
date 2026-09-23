use crate::RendererCompositionCamera;
use render_model::{RenderFrameDiff, RenderHandle};
use serde::{Deserialize, Serialize};

/// Immutable Engine renderer job. Resource bytes remain owned by the runtime
/// until this job is terminal; the frame describes current authored facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOutputJob {
    pub id: u64,
    pub source: RenderHandle,
    pub frame: RenderFrameDiff,
    pub operation: RenderOutputOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RenderOutputOperation {
    #[serde(rename_all = "camelCase")]
    Image {
        camera: Box<RendererCompositionCamera>,
        width: u32,
        height: u32,
        background: [f32; 4],
        use_camera_background: bool,
        exposure: f32,
        aces_filmic: bool,
        samples: u32,
        pose: Option<RenderOutputPose>,
    },
    #[serde(rename_all = "camelCase")]
    Glb { include_animations: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOutputPose {
    pub handle: RenderHandle,
    pub clip: String,
    pub normalized_time: f64,
}

/// Named bounded transfer of an output. Offsets allow retries without repeated
/// allocation or corrupting a previously accepted prefix. A final empty chunk
/// is legal; an error ends the job without exposing partial bytes as output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOutputChunk {
    pub id: u64,
    pub offset: usize,
    pub bytes: Vec<u8>,
    pub complete: bool,
    pub error: Option<String>,
}
