//! Concrete Rust-owned adapter for the existing browser/webview renderer host.
//!
//! Downstream owns the application window and event loop. This leaf owns one
//! child webview, one embedded Engine-built renderer artifact, one private
//! fixed binding, and the lifecycle of exactly one retained renderer surface.
//! It exposes no JavaScript evaluation, module loading, or generic invocation.

#![forbid(unsafe_code)]

use std::{
    borrow::Cow,
    fmt,
    sync::mpsc::{self, Receiver},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use render_host_contracts::{
    RendererCameraBasis, RendererCameraPose, RendererHostDiagnostic, RendererPhysicalInputReadout,
    RendererPickReceipt, RendererPickRequest, RendererViewComposition,
};
use render_model::RenderFrameDiff;
use render_presentation::PresentationFrameDiff;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use wry::{
    dpi::{LogicalPosition, LogicalSize},
    raw_window_handle::HasWindowHandle,
    PageLoadEvent, Rect, WebView, WebViewBuilder,
};

const BRIDGE_VERSION: &str = "rusty_renderer_webview_bridge.v1";
const RENDERER_ARTIFACT: &str = include_str!("../artifacts/renderer-webview.js");
const MAX_RESOURCE_COUNT: usize = 1_536;
const MAX_RESOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOTAL_RESOURCE_BYTES: usize = 384 * 1024 * 1024;

const RENDERER_DOCUMENT_PREFIX: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    html,body,#rusty-renderer-root { margin:0; width:100%; height:100%; overflow:hidden; background:#000; }
    #rusty-renderer-root { position:relative; }
    #rusty-renderer-canvas { display:block; width:100%; height:100%; }
    #rusty-renderer-overlays { position:absolute; inset:0; overflow:hidden; pointer-events:none; }
  </style>
</head>
<body>
  <div id="rusty-renderer-root">
    <canvas id="rusty-renderer-canvas"></canvas>
    <div id="rusty-renderer-overlays"></div>
  </div>
  <script>"#;

const RENDERER_DOCUMENT_SUFFIX: &str = r#"</script>
</body>
</html>"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererWebviewBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for RendererWebviewBounds {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 800,
            height: 450,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererResource {
    pub identity: String,
    pub content_hash: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererAnimatedMeshResource {
    pub asset: String,
    pub content_hash: String,
    pub clip_ids: Vec<String>,
}

impl RendererResource {
    pub fn validate(&self) -> Result<(), RendererWebviewError> {
        if self.identity.is_empty()
            || self.identity.len() > 256
            || self.identity.chars().any(char::is_control)
        {
            return Err(RendererWebviewError::InvalidResource(
                "resource identity is empty, oversized, or contains control characters",
            ));
        }
        if self.media_type.is_empty()
            || self.media_type.len() > 128
            || self.media_type.chars().any(char::is_control)
        {
            return Err(RendererWebviewError::InvalidResource(
                "resource media type is empty, oversized, or contains control characters",
            ));
        }
        if self.bytes.is_empty() || self.bytes.len() > MAX_RESOURCE_BYTES {
            return Err(RendererWebviewError::InvalidResource(
                "resource byte length is zero or exceeds the per-resource bound",
            ));
        }
        let actual = format!("sha256:{:x}", Sha256::digest(&self.bytes));
        if actual != self.content_hash {
            return Err(RendererWebviewError::InvalidResource(
                "resource bytes do not match the declared SHA-256 identity",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendererWebviewOptions {
    pub auto_start: bool,
    pub bounds: RendererWebviewBounds,
    pub clear_color: Option<u32>,
    pub pixel_ratio: f64,
    pub resources: Vec<RendererResource>,
}

impl Default for RendererWebviewOptions {
    fn default() -> Self {
        Self {
            auto_start: true,
            bounds: RendererWebviewBounds::default(),
            clear_color: None,
            pixel_ratio: 1.0,
            resources: Vec::new(),
        }
    }
}

impl RendererWebviewOptions {
    pub fn validate(&self) -> Result<(), RendererWebviewError> {
        validate_bounds(self.bounds)?;
        if !self.pixel_ratio.is_finite() || self.pixel_ratio <= 0.0 || self.pixel_ratio > 4.0 {
            return Err(RendererWebviewError::InvalidOptions(
                "pixel ratio must be finite and in (0, 4]",
            ));
        }
        if self.clear_color.is_some_and(|color| color > 0x00ff_ffff) {
            return Err(RendererWebviewError::InvalidOptions(
                "clear color must be a 24-bit RGB integer",
            ));
        }
        if self.resources.len() > MAX_RESOURCE_COUNT {
            return Err(RendererWebviewError::InvalidOptions(
                "resource count exceeds the renderer host bound",
            ));
        }
        let mut identities = std::collections::BTreeSet::new();
        let mut total = 0_usize;
        for resource in &self.resources {
            resource.validate()?;
            if !identities.insert(resource.identity.as_str()) {
                return Err(RendererWebviewError::InvalidResource(
                    "resource identities must be unique",
                ));
            }
            total = total.checked_add(resource.bytes.len()).ok_or(
                RendererWebviewError::InvalidOptions("aggregate resource byte length overflowed"),
            )?;
        }
        if total > MAX_TOTAL_RESOURCE_BYTES {
            return Err(RendererWebviewError::InvalidOptions(
                "aggregate resource byte length exceeds the renderer host bound",
            ));
        }
        Ok(())
    }
}

pub struct RendererWebviewAdapter {
    webview: WebView,
    observations: Receiver<String>,
    next_request_id: u64,
    disposed: bool,
}

impl RendererWebviewAdapter {
    /// Mount the current concrete host as one child webview of a downstream-owned window.
    pub fn mount<W: HasWindowHandle>(
        window: &W,
        options: RendererWebviewOptions,
    ) -> Result<Self, RendererWebviewError> {
        Self::mount_with_animated_meshes(window, options, Vec::new())
    }

    /// Mount with explicit animated-mesh descriptors whose content hashes
    /// resolve through `options.resources`. This keeps GLB asset/clip identity
    /// in the Engine host instead of exposing its private bridge downstream.
    pub fn mount_with_animated_meshes<W: HasWindowHandle>(
        window: &W,
        options: RendererWebviewOptions,
        animated_meshes: Vec<RendererAnimatedMeshResource>,
    ) -> Result<Self, RendererWebviewError> {
        options.validate()?;
        validate_animated_mesh_resources(&options.resources, &animated_meshes)?;
        let bounds = options.bounds;
        let configuration = RendererWireConfiguration::new(options, animated_meshes);
        let configuration_json = escape_inline_script_json(serde_json::to_string(&configuration)?);
        let renderer_document = format!(
            r#"{RENDERER_DOCUMENT_PREFIX}
globalThis.__rustyEngineRendererConfiguration={configuration_json};
try {{
{RENDERER_ARTIFACT}
}} catch (cause) {{
  window.ipc.postMessage(JSON.stringify({{
    bridgeVersion: "{BRIDGE_VERSION}",
    kind: "mountFailed",
    message: cause instanceof Error ? cause.message : String(cause),
  }}));
}}
{RENDERER_DOCUMENT_SUFFIX}"#
        );
        let (sender, observations) = mpsc::channel();
        let page_sender = sender.clone();
        let webview = WebViewBuilder::new()
            .with_bounds(wry_bounds(bounds))
            .with_ipc_handler(move |request| {
                let _ = sender.send(request.body().clone());
            })
            .with_custom_protocol("rustyrenderer".to_owned(), move |_id, request| {
                if request.method() == wry::http::Method::GET
                    && matches!(request.uri().path(), "" | "/")
                {
                    return wry::http::Response::builder()
                        .status(200)
                        .header("Content-Type", "text/html; charset=utf-8")
                        .body(Cow::Owned(renderer_document.as_bytes().to_vec()))
                        .expect("embedded renderer document response is valid");
                }
                wry::http::Response::builder()
                    .status(404)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Cow::<[u8]>::Borrowed(&[]))
                    .expect("fixed renderer observation response is valid")
            })
            .with_url("rustyrenderer://localhost")
            .with_on_page_load_handler(move |event, _url| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = page_sender.send(format!(
                        r#"{{"bridgeVersion":"{BRIDGE_VERSION}","kind":"documentLoaded"}}"#
                    ));
                }
            })
            .build_as_child(window)?;
        Ok(Self {
            webview,
            observations,
            next_request_id: 1,
            disposed: false,
        })
    }

    pub fn submit_frame(&mut self, frame: &RenderFrameDiff) -> Result<u64, RendererWebviewError> {
        frame
            .validate()
            .map_err(|_| RendererWebviewError::InvalidContract("render frame is invalid"))?;
        self.invoke(PrivateMethod::SubmitFrame, &(frame,))
    }

    pub fn submit_presentation(
        &mut self,
        frame: &PresentationFrameDiff,
    ) -> Result<u64, RendererWebviewError> {
        frame
            .validate()
            .map_err(|_| RendererWebviewError::InvalidContract("presentation frame is invalid"))?;
        self.invoke(PrivateMethod::SubmitPresentation, &(frame,))
    }

    pub fn configure_views(
        &mut self,
        composition: &RendererViewComposition,
    ) -> Result<u64, RendererWebviewError> {
        composition
            .validate()
            .map_err(|_| RendererWebviewError::InvalidContract("view composition is invalid"))?;
        self.invoke(PrivateMethod::ConfigureViews, &(composition,))
    }

    pub fn set_camera_pose(
        &mut self,
        pose: RendererCameraPose,
        basis: Option<RendererCameraBasis>,
    ) -> Result<u64, RendererWebviewError> {
        if !pose.position.into_iter().all(f64::is_finite)
            || !pose.pitch_degrees.is_finite()
            || !pose.yaw_degrees.is_finite()
        {
            return Err(RendererWebviewError::InvalidContract(
                "camera pose must contain finite values",
            ));
        }
        if basis.is_some_and(|basis| {
            basis
                .forward
                .into_iter()
                .chain(basis.right)
                .chain(basis.up)
                .any(|value| !value.is_finite())
        }) {
            return Err(RendererWebviewError::InvalidContract(
                "camera basis must contain finite values",
            ));
        }
        match basis {
            Some(basis) => self.invoke(PrivateMethod::SetCameraPose, &(pose, basis)),
            None => self.invoke(PrivateMethod::SetCameraPose, &(pose,)),
        }
    }

    pub fn pick(&mut self, request: &RendererPickRequest) -> Result<u64, RendererWebviewError> {
        self.invoke(PrivateMethod::Pick, &(request,))
    }

    pub fn read_state(&mut self) -> Result<u64, RendererWebviewError> {
        self.invoke(PrivateMethod::ReadState, &[] as &[u8])
    }

    pub fn read_physical_input(&mut self) -> Result<u64, RendererWebviewError> {
        self.invoke(PrivateMethod::ReadInput, &[] as &[u8])
    }

    pub fn render_once(&mut self, time_ms: Option<f64>) -> Result<u64, RendererWebviewError> {
        if time_ms.is_some_and(|time| !time.is_finite() || time < 0.0) {
            return Err(RendererWebviewError::InvalidContract(
                "render time must be finite and non-negative",
            ));
        }
        match time_ms {
            Some(time_ms) => self.invoke(PrivateMethod::RenderOnce, &(time_ms,)),
            None => self.invoke(PrivateMethod::RenderOnce, &[] as &[u8]),
        }
    }

    pub fn resume_audio(&mut self) -> Result<u64, RendererWebviewError> {
        self.invoke(PrivateMethod::ResumeAudio, &[] as &[u8])
    }

    pub fn start(&mut self) -> Result<u64, RendererWebviewError> {
        self.invoke(PrivateMethod::Start, &[] as &[u8])
    }

    pub fn stop(&mut self) -> Result<u64, RendererWebviewError> {
        self.invoke(PrivateMethod::Stop, &[] as &[u8])
    }

    pub fn resize(
        &mut self,
        bounds: RendererWebviewBounds,
        pixel_ratio: f64,
    ) -> Result<u64, RendererWebviewError> {
        validate_bounds(bounds)?;
        if !pixel_ratio.is_finite() || pixel_ratio <= 0.0 || pixel_ratio > 4.0 {
            return Err(RendererWebviewError::InvalidOptions(
                "pixel ratio must be finite and in (0, 4]",
            ));
        }
        self.ensure_live()?;
        self.webview.set_bounds(wry_bounds(bounds))?;
        self.invoke(
            PrivateMethod::Resize,
            &(bounds.width, bounds.height, pixel_ratio),
        )
    }

    pub fn dispose(&mut self) -> Result<u64, RendererWebviewError> {
        self.ensure_live()?;
        let request_id = self.invoke(PrivateMethod::Dispose, &[] as &[u8])?;
        self.disposed = true;
        Ok(request_id)
    }

    /// Drain immutable renderer observations without polling renderer state or starting another loop.
    pub fn drain_observations(
        &mut self,
    ) -> Vec<Result<RendererWebviewObservation, RendererWebviewError>> {
        let mut drained = Vec::new();
        while let Ok(message) = self.observations.try_recv() {
            drained.push(decode_observation(&message));
        }
        drained
    }

    fn invoke<A: Serialize + ?Sized>(
        &mut self,
        method: PrivateMethod,
        arguments: &A,
    ) -> Result<u64, RendererWebviewError> {
        self.ensure_live()?;
        if self.next_request_id > render_model::JSON_SAFE_U64_MAX {
            return Err(RendererWebviewError::RequestIdExhausted);
        }
        let request_id = self.next_request_id;
        let script = invocation_script(method, request_id, arguments)?;
        self.webview.evaluate_script(&script)?;
        self.next_request_id += 1;
        Ok(request_id)
    }

    fn ensure_live(&self) -> Result<(), RendererWebviewError> {
        if self.disposed {
            Err(RendererWebviewError::Disposed)
        } else {
            Ok(())
        }
    }
}

fn invocation_script<A: Serialize + ?Sized>(
    method: PrivateMethod,
    request_id: u64,
    arguments: &A,
) -> Result<String, serde_json::Error> {
    let arguments = serde_json::to_string(arguments)?;
    Ok(format!(
        "globalThis.__rustyEnginePrivateRenderer.{}({},...{});",
        method.javascript_name(),
        request_id,
        arguments
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub enum RendererWebviewObservation {
    DocumentLoaded,
    Ready(RendererSurfaceStateReadout),
    FrameApplied {
        request_id: u64,
        receipt: RendererFrameReceipt,
    },
    PresentationApplied {
        request_id: u64,
        receipt: RendererPresentationReceipt,
    },
    ViewsConfigured {
        request_id: u64,
        receipt: RendererViewConfigurationReceipt,
    },
    CameraUpdated {
        request_id: u64,
        pose: RendererCameraPose,
    },
    PickCompleted {
        request_id: u64,
        receipt: RendererPickReceipt,
    },
    StateRead {
        request_id: u64,
        readout: RendererSurfaceStateReadout,
    },
    PhysicalInputRead {
        request_id: u64,
        readout: RendererPhysicalInputReadout,
    },
    FrameRendered {
        request_id: u64,
        readout: RendererSubmissionReadout,
    },
    AudioResumed {
        request_id: u64,
        diagnostics: Vec<RendererHostDiagnostic>,
    },
    Started {
        request_id: u64,
        readout: RendererSurfaceStateReadout,
    },
    Stopped {
        request_id: u64,
        readout: RendererSurfaceStateReadout,
    },
    Resized {
        request_id: u64,
        readout: RendererSubmissionReadout,
    },
    Disposed {
        request_id: u64,
    },
    OperationFailed {
        request_id: u64,
        operation: RendererHostOperation,
        message: String,
    },
    MountFailed {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RendererHostOperation {
    SubmitFrame,
    SubmitPresentation,
    ConfigureViews,
    SetCameraPose,
    Pick,
    ReadState,
    ReadInput,
    RenderOnce,
    ResumeAudio,
    Start,
    Stop,
    Resize,
    Dispose,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererFrameReceipt {
    pub applied: bool,
    pub diagnostics: Vec<RendererHostDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererPresentationReceipt {
    pub schema_version: u32,
    pub applied: u64,
    pub diagnostics: Vec<RendererHostDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererViewConfigurationReceipt {
    pub applied: bool,
    pub revision: u64,
    pub diagnostics: Vec<RendererHostDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSurfaceStateReadout {
    pub kind: String,
    pub backend: RendererBackendReadout,
    pub camera: RendererCameraPose,
    pub pointer_locked: bool,
    pub submission: RendererSubmissionReadout,
    pub timing: RendererTimingReadout,
    pub views: RendererViewCompositionReadout,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererBackendReadout {
    pub family: String,
    pub implementation: String,
    pub public_contract: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererTimingReadout {
    pub schema_version: u32,
    pub render_sequence: u64,
    pub source: String,
    pub source_time_ms: f64,
    pub frame_interval_ms: Option<f64>,
    pub backend_submission_duration_ms: Option<f64>,
}

pub type RendererSubmissionReadout = RendererTimingReadout;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererViewCompositionReadout {
    pub schema_version: u32,
    pub revision: u64,
    pub cameras: Vec<render_host_contracts::RendererCompositionCamera>,
    pub targets: Vec<RendererTargetReadout>,
    pub views: Vec<render_host_contracts::RendererCompositionView>,
    pub presentations: Vec<render_host_contracts::RendererCompositionPresentation>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererTargetReadout {
    #[serde(flatten)]
    pub target: render_host_contracts::RendererCompositionTarget,
    pub last_refreshed_submission: Option<u64>,
    pub status: String,
}

#[derive(Debug)]
pub enum RendererWebviewError {
    Wry(wry::Error),
    Json(serde_json::Error),
    InvalidOptions(&'static str),
    InvalidResource(&'static str),
    InvalidContract(&'static str),
    InvalidProtocol(String),
    RequestIdExhausted,
    Disposed,
}

impl fmt::Display for RendererWebviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wry(error) => write!(formatter, "webview operation failed: {error}"),
            Self::Json(error) => write!(formatter, "renderer boundary JSON failed: {error}"),
            Self::InvalidOptions(message) => {
                write!(formatter, "invalid renderer options: {message}")
            }
            Self::InvalidResource(message) => {
                write!(formatter, "invalid renderer resource: {message}")
            }
            Self::InvalidContract(message) => {
                write!(formatter, "invalid renderer contract: {message}")
            }
            Self::InvalidProtocol(message) => {
                write!(formatter, "invalid renderer observation: {message}")
            }
            Self::RequestIdExhausted => formatter.write_str("renderer request IDs are exhausted"),
            Self::Disposed => formatter.write_str("renderer webview adapter is disposed"),
        }
    }
}

impl std::error::Error for RendererWebviewError {}

impl From<wry::Error> for RendererWebviewError {
    fn from(error: wry::Error) -> Self {
        Self::Wry(error)
    }
}

impl From<serde_json::Error> for RendererWebviewError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererWireConfiguration {
    auto_start: bool,
    animated_meshes: Vec<RendererAnimatedMeshResource>,
    clear_color: Option<u32>,
    pixel_ratio: f64,
    resources: Vec<RendererWireResource>,
}

impl RendererWireConfiguration {
    fn new(
        options: RendererWebviewOptions,
        animated_meshes: Vec<RendererAnimatedMeshResource>,
    ) -> Self {
        Self {
            auto_start: options.auto_start,
            animated_meshes,
            clear_color: options.clear_color,
            pixel_ratio: options.pixel_ratio,
            resources: options
                .resources
                .into_iter()
                .map(|resource| RendererWireResource {
                    identity: resource.identity,
                    content_hash: resource.content_hash,
                    media_type: resource.media_type,
                    bytes_base64: BASE64.encode(resource.bytes),
                })
                .collect(),
        }
    }
}

fn validate_animated_mesh_resources(
    resources: &[RendererResource],
    animated_meshes: &[RendererAnimatedMeshResource],
) -> Result<(), RendererWebviewError> {
    let resources_by_hash = resources
        .iter()
        .map(|resource| (resource.content_hash.as_str(), resource))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut assets = std::collections::BTreeSet::new();
    for animated in animated_meshes {
        if animated.asset.is_empty()
            || animated.asset.len() > 256
            || animated.asset.chars().any(char::is_control)
            || !assets.insert(animated.asset.as_str())
        {
            return Err(RendererWebviewError::InvalidResource(
                "animated mesh asset identity is invalid or duplicated",
            ));
        }
        if animated.clip_ids.is_empty()
            || animated.clip_ids.len() > 256
            || animated.clip_ids.iter().any(|clip| {
                clip.is_empty() || clip.len() > 256 || clip.chars().any(char::is_control)
            })
        {
            return Err(RendererWebviewError::InvalidResource(
                "animated mesh clips are empty, oversized, or invalid",
            ));
        }
        let Some(resource) = resources_by_hash.get(animated.content_hash.as_str()) else {
            return Err(RendererWebviewError::InvalidResource(
                "animated mesh content hash has no admitted resource",
            ));
        };
        if resource.media_type != "application/octet-stream" {
            return Err(RendererWebviewError::InvalidResource(
                "animated mesh resource must use application/octet-stream",
            ));
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererWireResource {
    identity: String,
    content_hash: String,
    media_type: String,
    bytes_base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum RendererWireObservation {
    DocumentLoaded {
        #[serde(rename = "bridgeVersion")]
        bridge_version: String,
    },
    Ready {
        #[serde(rename = "bridgeVersion")]
        bridge_version: String,
        value: Box<RendererSurfaceStateReadout>,
    },
    OperationSucceeded {
        #[serde(rename = "bridgeVersion")]
        bridge_version: String,
        operation: RendererHostOperation,
        #[serde(rename = "requestId")]
        request_id: u64,
        value: Value,
    },
    OperationFailed {
        #[serde(rename = "bridgeVersion")]
        bridge_version: String,
        operation: RendererHostOperation,
        #[serde(rename = "requestId")]
        request_id: u64,
        message: String,
    },
    MountFailed {
        #[serde(rename = "bridgeVersion")]
        bridge_version: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy)]
enum PrivateMethod {
    SubmitFrame,
    SubmitPresentation,
    ConfigureViews,
    SetCameraPose,
    Pick,
    ReadState,
    ReadInput,
    RenderOnce,
    ResumeAudio,
    Start,
    Stop,
    Resize,
    Dispose,
}

impl PrivateMethod {
    const fn javascript_name(self) -> &'static str {
        match self {
            Self::SubmitFrame => "submitFrame",
            Self::SubmitPresentation => "submitPresentation",
            Self::ConfigureViews => "configureViews",
            Self::SetCameraPose => "setCameraPose",
            Self::Pick => "pick",
            Self::ReadState => "readState",
            Self::ReadInput => "readInput",
            Self::RenderOnce => "renderOnce",
            Self::ResumeAudio => "resumeAudio",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Resize => "resize",
            Self::Dispose => "dispose",
        }
    }
}

fn decode_observation(message: &str) -> Result<RendererWebviewObservation, RendererWebviewError> {
    let observation: RendererWireObservation = serde_json::from_str(message)?;
    let version = match &observation {
        RendererWireObservation::DocumentLoaded { bridge_version }
        | RendererWireObservation::Ready { bridge_version, .. }
        | RendererWireObservation::OperationSucceeded { bridge_version, .. }
        | RendererWireObservation::OperationFailed { bridge_version, .. }
        | RendererWireObservation::MountFailed { bridge_version, .. } => bridge_version,
    };
    if version != BRIDGE_VERSION {
        return Err(RendererWebviewError::InvalidProtocol(format!(
            "bridge version {version:?} is unsupported"
        )));
    }
    match observation {
        RendererWireObservation::DocumentLoaded { .. } => {
            Ok(RendererWebviewObservation::DocumentLoaded)
        }
        RendererWireObservation::Ready { value, .. } => {
            Ok(RendererWebviewObservation::Ready(*value))
        }
        RendererWireObservation::OperationFailed {
            operation,
            request_id,
            message,
            ..
        } => Ok(RendererWebviewObservation::OperationFailed {
            request_id,
            operation,
            message,
        }),
        RendererWireObservation::MountFailed { message, .. } => {
            Ok(RendererWebviewObservation::MountFailed { message })
        }
        RendererWireObservation::OperationSucceeded {
            operation,
            request_id,
            value,
            ..
        } => decode_success(operation, request_id, value),
    }
}

fn decode_success(
    operation: RendererHostOperation,
    request_id: u64,
    value: Value,
) -> Result<RendererWebviewObservation, RendererWebviewError> {
    match operation {
        RendererHostOperation::SubmitFrame => Ok(RendererWebviewObservation::FrameApplied {
            request_id,
            receipt: decode_value(value)?,
        }),
        RendererHostOperation::SubmitPresentation => {
            Ok(RendererWebviewObservation::PresentationApplied {
                request_id,
                receipt: decode_value(value)?,
            })
        }
        RendererHostOperation::ConfigureViews => Ok(RendererWebviewObservation::ViewsConfigured {
            request_id,
            receipt: decode_value(value)?,
        }),
        RendererHostOperation::SetCameraPose => Ok(RendererWebviewObservation::CameraUpdated {
            request_id,
            pose: decode_value(value)?,
        }),
        RendererHostOperation::Pick => Ok(RendererWebviewObservation::PickCompleted {
            request_id,
            receipt: decode_value(value)?,
        }),
        RendererHostOperation::ReadState => Ok(RendererWebviewObservation::StateRead {
            request_id,
            readout: decode_value(value)?,
        }),
        RendererHostOperation::ReadInput => Ok(RendererWebviewObservation::PhysicalInputRead {
            request_id,
            readout: decode_value(value)?,
        }),
        RendererHostOperation::RenderOnce => Ok(RendererWebviewObservation::FrameRendered {
            request_id,
            readout: decode_value(value)?,
        }),
        RendererHostOperation::ResumeAudio => Ok(RendererWebviewObservation::AudioResumed {
            request_id,
            diagnostics: decode_value(value)?,
        }),
        RendererHostOperation::Start => Ok(RendererWebviewObservation::Started {
            request_id,
            readout: decode_value(value)?,
        }),
        RendererHostOperation::Stop => Ok(RendererWebviewObservation::Stopped {
            request_id,
            readout: decode_value(value)?,
        }),
        RendererHostOperation::Resize => Ok(RendererWebviewObservation::Resized {
            request_id,
            readout: decode_value(value)?,
        }),
        RendererHostOperation::Dispose => {
            if value.get("disposed").and_then(Value::as_bool) != Some(true) {
                return Err(RendererWebviewError::InvalidProtocol(
                    "dispose receipt did not confirm disposal".to_owned(),
                ));
            }
            Ok(RendererWebviewObservation::Disposed { request_id })
        }
    }
}

fn decode_value<T: DeserializeOwned>(value: Value) -> Result<T, RendererWebviewError> {
    serde_json::from_value(value).map_err(RendererWebviewError::Json)
}

fn validate_bounds(bounds: RendererWebviewBounds) -> Result<(), RendererWebviewError> {
    if bounds.width == 0 || bounds.height == 0 || bounds.width > 16_384 || bounds.height > 16_384 {
        Err(RendererWebviewError::InvalidOptions(
            "webview width and height must be in [1, 16384]",
        ))
    } else {
        Ok(())
    }
}

fn wry_bounds(bounds: RendererWebviewBounds) -> Rect {
    Rect {
        position: LogicalPosition::new(bounds.x, bounds.y).into(),
        size: LogicalSize::new(bounds.width, bounds.height).into(),
    }
}

fn escape_inline_script_json(json: String) -> String {
    json.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{RenderDiff, SpriteAtlasDescriptor, SpriteFrameRect};

    #[test]
    fn artifact_is_closed_and_contains_only_the_fixed_bridge() {
        assert!(RENDERER_ARTIFACT.len() > 100_000);
        assert!(!RENDERER_ARTIFACT.contains("import("));
        assert!(!RENDERER_ARTIFACT.contains(" from \"@rusty-engine/"));
        assert!(!RENDERER_ARTIFACT.contains(" from '@rusty-engine/"));
        for method in [
            PrivateMethod::SubmitFrame,
            PrivateMethod::SubmitPresentation,
            PrivateMethod::ConfigureViews,
            PrivateMethod::SetCameraPose,
            PrivateMethod::Pick,
            PrivateMethod::ReadState,
            PrivateMethod::ReadInput,
            PrivateMethod::RenderOnce,
            PrivateMethod::ResumeAudio,
            PrivateMethod::Start,
            PrivateMethod::Stop,
            PrivateMethod::Resize,
            PrivateMethod::Dispose,
        ] {
            assert!(RENDERER_ARTIFACT.contains(method.javascript_name()));
        }
        assert!(!RENDERER_ARTIFACT.contains("eval("));
    }

    #[test]
    fn submit_frame_invocation_carries_optional_sprite_frame_size() {
        let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/test".to_owned(),
                texture: "texture/test".to_owned(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0, 0.0],
                    uv_max: [1.0, 1.0],
                    size: Some([2.0, 3.0]),
                }],
            },
        }])
        .expect("valid public render frame");

        let script = invocation_script(PrivateMethod::SubmitFrame, 7, &(frame,))
            .expect("serialize renderer invocation");
        assert!(script.contains(r#""size":[2.0,3.0]"#));
        assert!(script.contains(".submitFrame(7,"));
    }

    #[test]
    fn malformed_and_wrong_version_observations_are_rejected() {
        assert!(decode_observation("not JSON").is_err());
        let error =
            decode_observation(r#"{"bridgeVersion":"future","kind":"mountFailed","message":"no"}"#)
                .unwrap_err();
        assert!(matches!(error, RendererWebviewError::InvalidProtocol(_)));
    }

    #[test]
    fn operation_failure_is_typed() {
        let observation = decode_observation(
            r#"{"bridgeVersion":"rusty_renderer_webview_bridge.v1","kind":"operationFailed","operation":"pick","requestId":7,"message":"bad ray"}"#,
        )
        .unwrap();
        assert_eq!(
            observation,
            RendererWebviewObservation::OperationFailed {
                request_id: 7,
                operation: RendererHostOperation::Pick,
                message: "bad ray".to_owned(),
            }
        );
    }

    #[test]
    fn resource_bytes_are_content_hash_bound_before_mount() {
        let bytes = b"renderer resource".to_vec();
        let resource = RendererResource {
            identity: "audio/test".to_owned(),
            content_hash: format!("sha256:{:x}", Sha256::digest(&bytes)),
            media_type: "audio/wav".to_owned(),
            bytes,
        };
        assert!(resource.validate().is_ok());
        let mut invalid = resource;
        invalid.bytes.push(0);
        assert!(matches!(
            invalid.validate(),
            Err(RendererWebviewError::InvalidResource(_))
        ));
    }

    #[test]
    fn animated_mesh_descriptors_resolve_only_admitted_glb_resources() {
        let bytes = b"animated mesh glb bytes".to_vec();
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let resources = vec![RendererResource {
            identity: format!("mesh-resource/{}", &content_hash["sha256:".len()..]),
            content_hash: content_hash.clone(),
            media_type: "application/octet-stream".to_owned(),
            bytes,
        }];
        let animated = vec![RendererAnimatedMeshResource {
            asset: "mesh-animation/test-actor".to_owned(),
            content_hash,
            clip_ids: vec!["idle".to_owned(), "run".to_owned()],
        }];
        assert!(validate_animated_mesh_resources(&resources, &animated).is_ok());

        let mut missing = animated;
        missing[0].content_hash = format!("sha256:{}", "0".repeat(64));
        assert!(matches!(
            validate_animated_mesh_resources(&resources, &missing),
            Err(RendererWebviewError::InvalidResource(_))
        ));
    }
}
