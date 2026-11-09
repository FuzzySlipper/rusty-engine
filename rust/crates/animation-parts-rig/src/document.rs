//! The parts of rig-c's `skeleton.model.json` the pose reads, as written.
//!
//! Sections and fields the pose does not read (the carried compile reports,
//! `editorOrder`, `stage`, `spine`, editor affordances) are left to serde's
//! default of ignoring them. Every number is the double the document spells;
//! [`crate::rig`] applies rig-c's float32 reads where its core applies them.

use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Document {
    pub spec: Option<String>,
    pub bones: Vec<Bone>,
    pub slots: Vec<Slot>,
    pub skins: Vec<Skin>,
    pub constraints: Vec<Constraint>,
    pub events: Vec<EventDef>,
    pub animations: Vec<Animation>,
    pub pages: Vec<Page>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Bone {
    pub name: String,
    pub parent: Option<String>,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
    #[serde(default = "one")]
    pub scale_x: f64,
    #[serde(default = "one")]
    pub scale_y: f64,
    pub shear_x: f64,
    pub shear_y: f64,
    pub inherit_mode: Option<String>,
    pub skin_required: bool,
}

fn one() -> f64 {
    1.0
}

#[derive(Deserialize)]
pub(crate) struct Slot {
    pub name: String,
    pub bone: String,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub dark: Option<String>,
    #[serde(default)]
    pub blend: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Skin {
    pub name: String,
    #[serde(default)]
    pub bones: Vec<String>,
    /// Slot name, then placeholder, then the attachment record, in the
    /// document's order.
    #[serde(default)]
    pub attachments: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct Constraint {
    pub kind: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Attachment {
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default = "one")]
    pub scale_x: f64,
    #[serde(default = "one")]
    pub scale_y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub atlas: Option<AtlasRect>,
    #[serde(default)]
    pub sequence: Option<Sequence>,
    #[serde(default)]
    pub uvs: Vec<f64>,
    #[serde(default)]
    pub triangles: Vec<u32>,
    #[serde(default)]
    pub vertices: Option<Vertices>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub skin: Option<String>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub timelines: Option<bool>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AtlasRect {
    pub width: f64,
    pub height: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub original_width: f64,
    pub original_height: f64,
}

#[derive(Deserialize)]
pub(crate) struct Sequence {
    pub count: usize,
    #[serde(default = "sequence_start")]
    pub start: usize,
    #[serde(default)]
    pub digits: usize,
    #[serde(default)]
    pub setup: usize,
    #[serde(default)]
    pub atlas: Vec<AtlasRect>,
}

fn sequence_start() -> usize {
    1
}

#[derive(Deserialize)]
pub(crate) struct Vertices {
    pub weighted: bool,
    #[serde(default)]
    pub xy: Vec<f64>,
    #[serde(default)]
    pub bindings: Vec<Vec<Binding>>,
}

#[derive(Deserialize)]
pub(crate) struct Binding {
    pub bone: String,
    pub x: f64,
    pub y: f64,
    pub weight: f64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct EventDef {
    pub name: String,
    pub int: i64,
    pub float: f64,
    pub string: String,
    pub audio: Option<String>,
    pub volume: f64,
    pub balance: f64,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Animation {
    pub name: String,
    pub bones: Vec<Target>,
    pub slots: Vec<Target>,
    pub attachments: Vec<SkinTimelines>,
    pub draw_order: Vec<DrawOrderKey>,
    pub events: Vec<EventKey>,
}

#[derive(Deserialize)]
pub(crate) struct Target {
    pub name: String,
    pub timelines: Vec<Timeline>,
}

#[derive(Deserialize)]
pub(crate) struct Timeline {
    pub name: String,
    pub keys: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize)]
pub(crate) struct SkinTimelines {
    pub name: String,
    pub slots: Vec<SlotTimelines>,
}

#[derive(Deserialize)]
pub(crate) struct SlotTimelines {
    pub name: String,
    pub attachments: Vec<AttachmentTimelines>,
}

#[derive(Deserialize)]
pub(crate) struct AttachmentTimelines {
    pub name: String,
    #[serde(default)]
    pub deform: Option<Vec<DeformKey>>,
    #[serde(default)]
    pub sequence: Option<Vec<SequenceKey>>,
}

#[derive(Deserialize)]
pub(crate) struct DeformKey {
    pub time: f64,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub vertices: Vec<f64>,
    #[serde(default)]
    pub curve: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct SequenceKey {
    pub time: f64,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub delay: Option<f64>,
}

#[derive(Deserialize)]
pub(crate) struct DrawOrderKey {
    pub time: f64,
    #[serde(default)]
    pub offsets: Vec<DrawOrderOffset>,
}

#[derive(Deserialize)]
pub(crate) struct DrawOrderOffset {
    pub slot: String,
    pub offset: i64,
}

#[derive(Deserialize)]
pub(crate) struct EventKey {
    pub time: f64,
    pub name: String,
    #[serde(default)]
    pub int: Option<i64>,
    #[serde(default)]
    pub float: Option<f64>,
    #[serde(default)]
    pub string: Option<String>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub balance: Option<f64>,
}

#[derive(Deserialize)]
pub(crate) struct Page {
    pub name: String,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub pma: bool,
    pub regions: Vec<PageRegion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageRegion {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub original_width: f64,
    pub original_height: f64,
    #[serde(default)]
    pub degrees: f64,
}
