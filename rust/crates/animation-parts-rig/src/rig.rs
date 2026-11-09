//! A rig read from `skeleton.model.json`: names resolved to indices, keys
//! read as rig-c's core reads them, every drawable attachment placed on its
//! page.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::document::{self, Document};
use crate::timeline::{
    f32r, DeformKey, DeformKeys, DrawOrderKey, EventKey, Keys, SequenceKey, SequenceMode,
    StatedCurve,
};
use crate::world::{Inherit, Local, RAD};

/// The one model document version this crate reads.
pub const MODEL_SPEC: &str = "rigc-compiled/3";

/// Why a model document cannot be posed.
#[derive(Debug, Clone, PartialEq)]
pub enum RigError {
    /// The text is not a model document.
    Json(String),
    /// The document is another version of rig-c's model, or none.
    Spec(Option<String>),
    /// The rig declares constraints, as `(kind, name)`; the core evaluates
    /// none yet.
    Constraints(Vec<(String, String)>),
    /// A clipping attachment, as `(skin, slot, placeholder)`; clipping is not
    /// evaluated yet.
    Clipping(String, String, String),
    /// A name the document uses and does not declare.
    Unresolved(String),
    /// A drawable attachment whose region no page holds, or whose region
    /// record states no rectangle.
    Region(String),
}

impl fmt::Display for RigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RigError::Json(why) => write!(f, "not a rig-c model document: {why}"),
            RigError::Spec(spec) => write!(
                f,
                "model document spec is {}, and this core reads only \"{MODEL_SPEC}\"",
                spec.as_deref().map(|s| format!("\"{s}\"")).unwrap_or_else(|| "absent".to_owned())
            ),
            RigError::Constraints(list) => {
                write!(f, "the rig declares constraints the parts-rig core does not evaluate yet:")?;
                for (i, (kind, name)) in list.iter().enumerate() {
                    write!(f, "{} {kind} \"{name}\"", if i == 0 { "" } else { "," })?;
                }
                Ok(())
            }
            RigError::Clipping(skin, slot, placeholder) => write!(
                f,
                "skin \"{skin}\" slot \"{slot}\" attachment \"{placeholder}\" is a clipping attachment, which the parts-rig core does not evaluate yet"
            ),
            RigError::Unresolved(what) => write!(f, "{what}"),
            RigError::Region(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for RigError {}

/// How a slot's colour combines with what is drawn under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Additive,
    Multiply,
    Screen,
}

/// One texture page the rig's regions sit on.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The page image, as the document names it: a path relative to the
    /// model document.
    pub name: String,
    pub width: f64,
    pub height: f64,
    /// Whether the page's texels are premultiplied by alpha.
    pub pma: bool,
}

/// What one attachment frame draws with: its page, the page UVs of its
/// vertices and its triangles. Static for the rig's life.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawable {
    pub page: usize,
    /// One page UV per vertex, `u` right and `v` down the page.
    pub uvs: Vec<[f32; 2]>,
    /// Vertex indices, three per triangle.
    pub triangles: Vec<u32>,
}

pub(crate) struct BoneData {
    pub name: String,
    pub parent: Option<usize>,
    pub setup: Local,
    pub skin_required: bool,
}

pub(crate) struct SlotData {
    pub name: String,
    pub bone: usize,
    pub setup: Option<u32>,
    pub light: [f64; 4],
    pub dark: Option<[f64; 3]>,
    pub blend: BlendMode,
}

pub(crate) struct SkinData {
    pub name: String,
    pub bones: Vec<usize>,
    /// `(slot, placeholder)` to attachment.
    pub table: HashMap<(usize, u32), usize>,
}

pub(crate) enum VertexData {
    /// Positions in the slot bone's space, through float32.
    Unweighted(Vec<f64>),
    /// Per vertex, its count of influences; then every influence as
    /// `(bone, x, y, weight)` through float32, in order.
    Weighted {
        counts: Vec<u32>,
        bindings: Vec<(usize, f64, f64, f64)>,
    },
}

impl VertexData {
    pub fn weighted(&self) -> bool {
        matches!(self, VertexData::Weighted { .. })
    }

    /// rig-c's `setupArray`: the deform array a key starts from.
    fn setup_array(&self) -> Vec<f64> {
        match self {
            VertexData::Unweighted(xy) => xy.clone(),
            VertexData::Weighted { bindings, .. } => vec![0.0; 2 * bindings.len()],
        }
    }
}

pub(crate) enum Geometry {
    /// Corner offsets in the slot bone's space, one set per frame.
    Region(Vec<[f64; 8]>),
    Mesh(MeshData),
    /// A mesh drawing another's vertices.
    Linked {
        source: usize,
    },
    /// Bounding boxes and paths: posed by nothing here, drawn by nothing.
    Inert,
}

pub(crate) struct MeshData {
    pub vertices: VertexData,
    /// The deform array of the setup geometry.
    pub setup: Vec<f64>,
}

pub(crate) struct AttachmentData {
    pub name: String,
    pub color: [f64; 4],
    pub geometry: Geometry,
    /// The attachment whose deform timelines move this one: itself, or a
    /// linked mesh's source when the link plays its source's timelines.
    pub deform_identity: usize,
    /// The first drawable; one per sequence frame.
    pub drawables: usize,
    pub frames: usize,
    pub setup_frame: usize,
}

impl AttachmentData {
    pub fn drawn(&self) -> bool {
        !matches!(self.geometry, Geometry::Inert)
    }
}

/// A bone's local property, as an animation keys it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoneTimeline {
    Rotate,
    Translate,
    TranslateX,
    TranslateY,
    Scale,
    ScaleX,
    ScaleY,
    Shear,
    ShearX,
    ShearY,
}

/// The slot colour timelines.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColourTimeline {
    Rgba,
    Rgb,
    Alpha,
    Rgba2,
    Rgb2,
}

pub(crate) enum TimelineKind {
    Bone {
        bone: usize,
        kind: BoneTimeline,
        keys: Keys,
    },
    Inherit {
        bone: usize,
        keys: Vec<(f64, Inherit)>,
    },
    Colour {
        slot: usize,
        kind: ColourTimeline,
        keys: Keys,
    },
    Attachment {
        slot: usize,
        keys: Vec<(f64, Option<u32>)>,
    },
    Deform {
        identity: usize,
        keys: DeformKeys,
    },
    Events(Vec<EventKey>),
}

/// One property a timeline poses, for deciding how clips mix.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Property {
    Rotate(usize),
    X(usize),
    Y(usize),
    ScaleX(usize),
    ScaleY(usize),
    ShearX(usize),
    ShearY(usize),
    Inherit(usize),
    Rgb(usize),
    Alpha(usize),
    Rgb2(usize),
    Attachment(usize),
    Deform(usize, usize),
    Event,
}

pub(crate) struct Timeline {
    pub kind: TimelineKind,
    pub properties: Vec<Property>,
    /// Attachment, inherit and event timelines change at a key and do not
    /// fade.
    pub instant: bool,
}

pub(crate) struct AnimationData {
    pub name: String,
    pub duration: f64,
    /// In the order rig-c's mixer applies them: slots, bones, deforms,
    /// events.
    pub timelines: Vec<Timeline>,
    pub properties: HashSet<Property>,
    pub draw_order: Vec<DrawOrderKey>,
    /// `(attachment, keys)`.
    pub sequences: Vec<(usize, Vec<SequenceKey>)>,
}

pub(crate) struct EventData {
    pub name: String,
}

/// A parts rig read from rig-c's `skeleton.model.json`: immutable, shared by
/// every [`crate::Player`] that plays it.
pub struct Rig {
    pub(crate) bones: Vec<BoneData>,
    pub(crate) slots: Vec<SlotData>,
    pub(crate) skins: Vec<SkinData>,
    pub(crate) default_skin: Option<usize>,
    pub(crate) attachments: Vec<AttachmentData>,
    pub(crate) drawables: Vec<Drawable>,
    pub(crate) animations: Vec<AnimationData>,
    pub(crate) events: Vec<EventData>,
    pub(crate) pages: Vec<Page>,
}

const REGION_UV_TRIANGLES: [u32; 6] = [0, 1, 2, 2, 3, 0];

fn colour(hex: &str, what: &str) -> Result<[f64; 4], RigError> {
    let channel = |i: usize| -> Result<f64, RigError> {
        hex.get(2 * i..2 * i + 2)
            .and_then(|c| u8::from_str_radix(c, 16).ok())
            .map(|c| f64::from(c) / 255.0)
            .ok_or_else(|| RigError::Unresolved(format!("{what}: \"{hex}\" is not a hex colour")))
    };
    Ok([
        channel(0)?,
        channel(1)?,
        channel(2)?,
        if hex.len() == 8 { channel(3)? } else { 1.0 },
    ])
}

fn fold(stated: &str, lower_first: bool) -> String {
    let mut chars = stated.chars();
    match chars.next() {
        Some(first) if lower_first => first.to_lowercase().chain(chars).collect(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn inherit(stated: &str, what: &str) -> Result<Inherit, RigError> {
    Ok(match fold(stated, true).as_str() {
        "normal" => Inherit::Normal,
        "onlyTranslation" => Inherit::OnlyTranslation,
        "noRotationOrReflection" => Inherit::NoRotationOrReflection,
        "noScale" => Inherit::NoScale,
        "noScaleOrReflection" => Inherit::NoScaleOrReflection,
        _ => {
            return Err(RigError::Unresolved(format!(
                "{what}: inherit mode \"{stated}\" is no mode"
            )))
        }
    })
}

fn blend(stated: &str, what: &str) -> Result<BlendMode, RigError> {
    Ok(match fold(stated, false).as_str() {
        "Normal" => BlendMode::Normal,
        "Additive" => BlendMode::Additive,
        "Multiply" => BlendMode::Multiply,
        "Screen" => BlendMode::Screen,
        _ => {
            return Err(RigError::Unresolved(format!(
                "{what}: blend \"{stated}\" is no mode"
            )))
        }
    })
}

/// A key's `curve` as stated: absent, `"stepped"`, or the control numbers
/// `numbers` holds.
fn stated_curve<'a>(
    value: Option<&serde_json::Value>,
    numbers: &'a Option<Vec<f64>>,
) -> StatedCurve<'a> {
    match (value, numbers) {
        (_, Some(c)) => StatedCurve::Bezier(c),
        (Some(serde_json::Value::String(s)), None) if s == "stepped" => StatedCurve::Stepped,
        _ => StatedCurve::Linear,
    }
}

/// The control numbers of a Bézier `curve`.
fn curve_numbers(value: Option<&serde_json::Value>) -> Option<Vec<f64>> {
    match value {
        Some(serde_json::Value::Array(items)) => {
            Some(items.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect())
        }
        _ => None,
    }
}

type KeyMap = serde_json::Map<String, serde_json::Value>;

fn number(key: &KeyMap, field: &str) -> f64 {
    key.get(field)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

/// Builds number-timeline keys from document keys, reading `channels` of
/// each with `read`.
fn number_keys(
    keys: &[KeyMap],
    read: impl Fn(&KeyMap) -> Result<Vec<f64>, RigError>,
) -> Result<Keys, RigError> {
    let curves: Vec<Option<Vec<f64>>> =
        keys.iter().map(|k| curve_numbers(k.get("curve"))).collect();
    let mut stated = Vec::with_capacity(keys.len());
    for (k, numbers) in keys.iter().zip(&curves) {
        stated.push((
            number(k, "time"),
            read(k)?,
            stated_curve(k.get("curve"), numbers),
        ));
    }
    Ok(Keys::new(stated))
}

struct Names<'a> {
    bones: HashMap<&'a str, usize>,
    slots: HashMap<&'a str, usize>,
    skins: HashMap<&'a str, usize>,
    placeholders: HashMap<String, u32>,
}

impl Names<'_> {
    fn bone(&self, name: &str, what: &str) -> Result<usize, RigError> {
        self.bones
            .get(name)
            .copied()
            .ok_or_else(|| RigError::Unresolved(format!("{what}: no bone \"{name}\"")))
    }

    fn slot(&self, name: &str, what: &str) -> Result<usize, RigError> {
        self.slots
            .get(name)
            .copied()
            .ok_or_else(|| RigError::Unresolved(format!("{what}: no slot \"{name}\"")))
    }

    fn placeholder(&mut self, name: &str) -> u32 {
        let next = self.placeholders.len() as u32;
        *self.placeholders.entry(name.to_owned()).or_insert(next)
    }
}

impl Rig {
    /// Reads a `rigc-compiled/3` model document.
    ///
    /// A document of another version, a rig with constraints and a rig with
    /// clipping attachments are refused by name.
    pub fn from_json(text: &str) -> Result<Rig, RigError> {
        let doc: Document =
            serde_json::from_str(text).map_err(|e| RigError::Json(e.to_string()))?;
        if doc.spec.as_deref() != Some(MODEL_SPEC) {
            return Err(RigError::Spec(doc.spec));
        }
        if !doc.constraints.is_empty() {
            return Err(RigError::Constraints(
                doc.constraints
                    .iter()
                    .map(|c| (c.kind.clone(), c.name.clone()))
                    .collect(),
            ));
        }
        Self::read(&doc)
    }

    fn read(doc: &Document) -> Result<Rig, RigError> {
        let mut names = Names {
            bones: doc
                .bones
                .iter()
                .enumerate()
                .map(|(i, b)| (b.name.as_str(), i))
                .collect(),
            slots: doc
                .slots
                .iter()
                .enumerate()
                .map(|(i, s)| (s.name.as_str(), i))
                .collect(),
            skins: doc
                .skins
                .iter()
                .enumerate()
                .map(|(i, s)| (s.name.as_str(), i))
                .collect(),
            placeholders: HashMap::new(),
        };

        let mut bones = Vec::with_capacity(doc.bones.len());
        for b in &doc.bones {
            let what = format!("bone \"{}\"", b.name);
            let parent = match &b.parent {
                Some(p) => Some(names.bone(p, &what)?),
                None => None,
            };
            bones.push(BoneData {
                name: b.name.clone(),
                parent,
                setup: Local {
                    rotation: b.rotation,
                    x: b.x,
                    y: b.y,
                    scale_x: b.scale_x,
                    scale_y: b.scale_y,
                    shear_x: b.shear_x,
                    shear_y: b.shear_y,
                    inherit: match &b.inherit_mode {
                        Some(mode) => inherit(mode, &what)?,
                        None => Inherit::Normal,
                    },
                },
                skin_required: b.skin_required,
            });
        }

        let mut slots = Vec::with_capacity(doc.slots.len());
        for s in &doc.slots {
            let what = format!("slot \"{}\"", s.name);
            slots.push(SlotData {
                name: s.name.clone(),
                bone: names.bone(&s.bone, &what)?,
                setup: s.setup.as_deref().map(|p| names.placeholder(p)),
                light: match &s.color {
                    Some(c) => colour(c, &what)?,
                    None => [1.0; 4],
                },
                dark: match &s.dark {
                    Some(c) => {
                        let [r, g, b, _] = colour(c, &what)?;
                        Some([r, g, b])
                    }
                    None => None,
                },
                blend: match &s.blend {
                    Some(m) => blend(m, &what)?,
                    None => BlendMode::Normal,
                },
            });
        }

        let pages: Vec<Page> = doc
            .pages
            .iter()
            .map(|p| Page {
                name: p.name.clone(),
                width: p.width,
                height: p.height,
                pma: p.pma,
            })
            .collect();
        let mut regions: HashMap<&str, (usize, &document::PageRegion)> = HashMap::new();
        for (i, page) in doc.pages.iter().enumerate() {
            for region in &page.regions {
                regions.entry(region.name.as_str()).or_insert((i, region));
            }
        }

        // Attachments, in two passes so a linked mesh finds its source.
        struct Raw<'a> {
            skin: usize,
            slot: usize,
            placeholder: &'a str,
            record: document::Attachment,
        }
        let mut raws: Vec<Raw<'_>> = Vec::new();
        let mut skins = Vec::with_capacity(doc.skins.len());
        for (k, skin) in doc.skins.iter().enumerate() {
            let what = format!("skin \"{}\"", skin.name);
            let mut table = HashMap::new();
            for (slot_name, entries) in &skin.attachments {
                let slot = names.slot(slot_name, &what)?;
                let Some(entries) = entries.as_object() else {
                    continue;
                };
                for (placeholder, value) in entries {
                    let record: document::Attachment = serde_json::from_value(value.clone())
                        .map_err(|e| {
                            RigError::Json(format!(
                                "{what} slot \"{slot_name}\" attachment \"{placeholder}\": {e}"
                            ))
                        })?;
                    if record.kind == "clipping" {
                        return Err(RigError::Clipping(
                            skin.name.clone(),
                            slot_name.clone(),
                            placeholder.clone(),
                        ));
                    }
                    let id = names.placeholder(placeholder);
                    table.insert((slot, id), raws.len());
                    raws.push(Raw {
                        skin: k,
                        slot,
                        placeholder,
                        record,
                    });
                }
            }
            let mut members = Vec::with_capacity(skin.bones.len());
            for b in &skin.bones {
                members.push(names.bone(b, &what)?);
            }
            skins.push(SkinData {
                name: skin.name.clone(),
                bones: members,
                table,
            });
        }

        let mut attachments: Vec<AttachmentData> = Vec::with_capacity(raws.len());
        let mut drawables: Vec<Drawable> = Vec::new();
        // Mesh vertices and art UVs by attachment, for links.
        let mut mesh_uvs: Vec<Option<(Vec<f64>, Vec<u32>)>> = Vec::with_capacity(raws.len());
        for raw in &raws {
            let r = &raw.record;
            if r.kind == "mesh" && r.source.is_none() {
                mesh_uvs.push(Some((r.uvs.clone(), r.triangles.clone())));
            } else {
                mesh_uvs.push(None);
            }
        }
        for (index, raw) in raws.iter().enumerate() {
            let r = &raw.record;
            let skin_name = &doc.skins[raw.skin].name;
            let slot_name = &doc.slots[raw.slot].name;
            let what = format!(
                "skin \"{skin_name}\" slot \"{slot_name}\" attachment \"{}\"",
                raw.placeholder
            );
            let name = r.name.clone().unwrap_or_else(|| raw.placeholder.to_owned());
            let color = match &r.color {
                Some(c) => colour(c, &what)?,
                None => [1.0; 4],
            };
            let frames = r.sequence.as_ref().map_or(1, |s| s.count.max(1));
            let setup_frame = r.sequence.as_ref().map_or(0, |s| s.setup);
            let path = r.path.clone().unwrap_or_else(|| name.clone());
            let region_name = |frame: usize| -> String {
                match &r.sequence {
                    Some(s) => format!("{path}{:0width$}", s.start + frame, width = s.digits),
                    None => path.clone(),
                }
            };
            let first = drawables.len();
            let mut deform_identity = index;
            let geometry = match r.kind.as_str() {
                "region" => {
                    let rects: Vec<document::AtlasRect> = match (&r.sequence, r.atlas) {
                        (Some(s), _) => s.atlas.clone(),
                        (None, Some(rect)) => vec![rect],
                        (None, None) => {
                            return Err(RigError::Region(format!(
                                "{what}: the region states no atlas rectangle"
                            )))
                        }
                    };
                    let mut corners = Vec::with_capacity(frames);
                    for frame in 0..frames {
                        let rect = rects.get(frame).ok_or_else(|| {
                            RigError::Region(format!(
                                "{what}: the sequence states no rectangle for frame {frame}"
                            ))
                        })?;
                        corners.push(region_corners(r, rect));
                        let (page, region) = place(&regions, &region_name(frame), &what)?;
                        drawables.push(Drawable {
                            page,
                            uvs: region_uvs(region, &pages[page]),
                            triangles: REGION_UV_TRIANGLES.to_vec(),
                        });
                    }
                    Geometry::Region(corners)
                }
                "mesh" | "linkedmesh" => {
                    let (art, triangles, geometry) = match &r.source {
                        Some(source) => {
                            let source_skin = match &r.skin {
                                Some(s) => *names.skins.get(s.as_str()).ok_or_else(|| {
                                    RigError::Unresolved(format!("{what}: the linked mesh's skin \"{s}\" is no skin"))
                                })?,
                                None => *names.skins.get("default").ok_or_else(|| {
                                    RigError::Unresolved(format!("{what}: the linked mesh names no skin and there is no default skin"))
                                })?,
                            };
                            let source_slot = match &r.slot {
                                Some(s) => names.slot(s, &what)?,
                                None => raw.slot,
                            };
                            let source_id = names
                                .placeholders
                                .get(source.as_str())
                                .and_then(|p| skins[source_skin].table.get(&(source_slot, *p)))
                                .copied()
                                .filter(|&i| mesh_uvs[i].is_some())
                                .ok_or_else(|| RigError::Unresolved(format!("{what}: the linked mesh's source mesh \"{source}\" is not found")))?;
                            if r.timelines.unwrap_or(true) {
                                deform_identity = source_id;
                            }
                            let (uvs, triangles) = mesh_uvs[source_id].clone().unwrap_or_default();
                            (uvs, triangles, Geometry::Linked { source: source_id })
                        }
                        None => {
                            let vertices = vertex_data(r.vertices.as_ref(), &names, &what)?;
                            let setup = vertices.setup_array();
                            (
                                r.uvs.clone(),
                                r.triangles.clone(),
                                Geometry::Mesh(MeshData { vertices, setup }),
                            )
                        }
                    };
                    for frame in 0..frames {
                        let (page, region) = place(&regions, &region_name(frame), &what)?;
                        drawables.push(Drawable {
                            page,
                            uvs: mesh_page_uvs(region, &pages[page], &art),
                            triangles: triangles.clone(),
                        });
                    }
                    geometry
                }
                _ => Geometry::Inert,
            };
            attachments.push(AttachmentData {
                name,
                color,
                geometry,
                deform_identity,
                drawables: first,
                frames: if drawables.len() > first { frames } else { 0 },
                setup_frame,
            });
        }

        let events: Vec<EventData> = doc
            .events
            .iter()
            .map(|e| EventData {
                name: e.name.clone(),
            })
            .collect();
        let event_index: HashMap<&str, usize> = doc
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| (e.name.as_str(), i))
            .collect();

        let default_skin = names.skins.get("default").copied();
        let mut animations = Vec::with_capacity(doc.animations.len());
        for a in &doc.animations {
            animations.push(read_animation(
                a,
                doc,
                &mut names,
                &skins,
                &attachments,
                &event_index,
            )?);
        }

        Ok(Rig {
            bones,
            slots,
            skins,
            default_skin,
            attachments,
            drawables,
            animations,
            events,
            pages,
        })
    }
}

fn place<'a>(
    regions: &HashMap<&str, (usize, &'a document::PageRegion)>,
    name: &str,
    what: &str,
) -> Result<(usize, &'a document::PageRegion), RigError> {
    regions
        .get(name)
        .copied()
        .ok_or_else(|| RigError::Region(format!("{what}: no page holds region \"{name}\"")))
}

fn vertex_data(
    vertices: Option<&document::Vertices>,
    names: &Names<'_>,
    what: &str,
) -> Result<VertexData, RigError> {
    let Some(v) = vertices else {
        return Err(RigError::Unresolved(format!(
            "{what}: the mesh states no vertices"
        )));
    };
    if !v.weighted {
        return Ok(VertexData::Unweighted(
            v.xy.iter().map(|&x| f32r(x)).collect(),
        ));
    }
    let mut counts = Vec::with_capacity(v.bindings.len());
    let mut bindings = Vec::new();
    for influences in &v.bindings {
        counts.push(influences.len() as u32);
        for b in influences {
            bindings.push((
                names.bone(&b.bone, what)?,
                f32r(b.x),
                f32r(b.y),
                f32r(b.weight),
            ));
        }
    }
    Ok(VertexData::Weighted { counts, bindings })
}

/// rig-c's `regionCorners`, before the bone: the four corners in the slot
/// bone's space, in the order `(x1, y1) (x1, y2) (x2, y2) (x2, y1)`.
fn region_corners(r: &document::Attachment, rect: &document::AtlasRect) -> [f64; 8] {
    let (w, h) = (r.width, r.height);
    let (sx, sy) = (r.scale_x, r.scale_y);
    let kx = (w / rect.original_width) * sx;
    let ky = (h / rect.original_height) * sy;
    let x1 = (-w / 2.0) * sx + rect.offset_x * kx;
    let y1 = (-h / 2.0) * sy + rect.offset_y * ky;
    let x2 = x1 + rect.width * kx;
    let y2 = y1 + rect.height * ky;
    let angle = r.rotation * RAD;
    let (sin, cos) = (angle.sin(), angle.cos());
    let mut out = [0.0; 8];
    for (i, (px, py)) in [(x1, y1), (x1, y2), (x2, y2), (x2, y1)]
        .into_iter()
        .enumerate()
    {
        out[2 * i] = px * cos + r.x - py * sin;
        out[2 * i + 1] = py * cos + r.y + px * sin;
    }
    out
}

/// rig-c's `regionPageUvs`, in the corner order of [`region_corners`].
fn region_uvs(region: &document::PageRegion, page: &Page) -> Vec<[f32; 2]> {
    let turned = region.degrees == 90.0;
    let u = region.x / page.width;
    let v = region.y / page.height;
    let u2 = (region.x + if turned { region.height } else { region.width }) / page.width;
    let v2 = (region.y + if turned { region.width } else { region.height }) / page.height;
    let corners = if turned {
        [[u2, v2], [u, v2], [u, v], [u2, v]]
    } else {
        [[u, v2], [u, v], [u2, v], [u2, v2]]
    };
    corners
        .iter()
        .map(|[a, b]| [*a as f32, *b as f32])
        .collect()
}

/// rig-c's `computeUvs`: a mesh's art UVs placed on its page region.
fn mesh_page_uvs(region: &document::PageRegion, page: &Page, art: &[f64]) -> Vec<[f32; 2]> {
    let (ox, oy, ow, oh) = (
        region.offset_x,
        region.offset_y,
        region.original_width,
        region.original_height,
    );
    let (w_page, h_page) = (page.width, page.height);
    let mut u = region.x / w_page;
    let mut v = region.y / h_page;
    let pairs = art.as_chunks::<2>().0.iter();
    let out: Vec<[f64; 2]> = match region.degrees as i64 {
        90 => {
            u -= (oh - oy - region.height) / w_page;
            v -= (ow - ox - region.width) / h_page;
            let (w, h) = (oh / w_page, ow / h_page);
            pairs
                .map(|s| [u + s[1] * w, v + (1.0 - s[0]) * h])
                .collect()
        }
        180 => {
            u -= (ow - ox - region.width) / w_page;
            v -= oy / h_page;
            let (w, h) = (ow / w_page, oh / h_page);
            pairs
                .map(|s| [u + (1.0 - s[0]) * w, v + (1.0 - s[1]) * h])
                .collect()
        }
        270 => {
            u -= oy / w_page;
            v -= ox / h_page;
            let (w, h) = (oh / w_page, ow / h_page);
            pairs
                .map(|s| [u + (1.0 - s[1]) * w, v + s[0] * h])
                .collect()
        }
        _ => {
            u -= ox / w_page;
            v -= (oh - oy - region.height) / h_page;
            let (w, h) = (ow / w_page, oh / h_page);
            pairs.map(|s| [u + s[0] * w, v + s[1] * h]).collect()
        }
    };
    out.iter().map(|[a, b]| [*a as f32, *b as f32]).collect()
}

fn hex_values(
    key: &KeyMap,
    field: &str,
    channels: usize,
    what: &str,
) -> Result<Vec<f64>, RigError> {
    let hex = key
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    Ok(colour(hex, what)?[..channels].to_vec())
}

fn read_animation(
    a: &document::Animation,
    doc: &Document,
    names: &mut Names<'_>,
    skins: &[SkinData],
    attachments: &[AttachmentData],
    event_index: &HashMap<&str, usize>,
) -> Result<AnimationData, RigError> {
    let what = format!("animation \"{}\"", a.name);
    let mut timelines = Vec::new();
    let mut times: Vec<f64> = Vec::new();

    for target in &a.slots {
        let slot = names.slot(&target.name, &what)?;
        for tl in &target.timelines {
            let at = format!("{what} slot \"{}\" {}", target.name, tl.name);
            let (kind, properties, instant) = match tl.name.as_str() {
                "attachment" => {
                    let mut keys = Vec::with_capacity(tl.keys.len());
                    for k in &tl.keys {
                        let name = k
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .map(|n| names.placeholder(n));
                        keys.push((f32r(number(k, "time")), name));
                    }
                    (
                        TimelineKind::Attachment { slot, keys },
                        vec![Property::Attachment(slot)],
                        true,
                    )
                }
                colour_name => {
                    let (kind, properties): (ColourTimeline, Vec<Property>) = match colour_name {
                        "rgba" => (
                            ColourTimeline::Rgba,
                            vec![Property::Rgb(slot), Property::Alpha(slot)],
                        ),
                        "rgb" => (ColourTimeline::Rgb, vec![Property::Rgb(slot)]),
                        "alpha" => (ColourTimeline::Alpha, vec![Property::Alpha(slot)]),
                        "rgba2" => (
                            ColourTimeline::Rgba2,
                            vec![
                                Property::Rgb(slot),
                                Property::Alpha(slot),
                                Property::Rgb2(slot),
                            ],
                        ),
                        "rgb2" => (
                            ColourTimeline::Rgb2,
                            vec![Property::Rgb(slot), Property::Rgb2(slot)],
                        ),
                        other => {
                            return Err(RigError::Unresolved(format!(
                                "{what} slot \"{}\": no slot timeline \"{other}\"",
                                target.name
                            )))
                        }
                    };
                    let keys = number_keys(&tl.keys, |k| match kind {
                        ColourTimeline::Rgba => hex_values(k, "color", 4, &at),
                        ColourTimeline::Rgb => hex_values(k, "color", 3, &at),
                        ColourTimeline::Alpha => Ok(vec![number(k, "value")]),
                        ColourTimeline::Rgba2 => {
                            let mut v = hex_values(k, "light", 4, &at)?;
                            v.extend(hex_values(k, "dark", 3, &at)?);
                            Ok(v)
                        }
                        ColourTimeline::Rgb2 => {
                            let mut v = hex_values(k, "light", 3, &at)?;
                            v.extend(hex_values(k, "dark", 3, &at)?);
                            Ok(v)
                        }
                    })?;
                    (TimelineKind::Colour { slot, kind, keys }, properties, false)
                }
            };
            times.extend(timeline_times(&kind));
            timelines.push(Timeline {
                kind,
                properties,
                instant,
            });
        }
    }

    for target in &a.bones {
        let bone = names.bone(&target.name, &what)?;
        for tl in &target.timelines {
            use BoneTimeline::*;
            let (kind, channels, properties): (BoneTimeline, &[&str], Vec<Property>) =
                match tl.name.as_str() {
                    "inherit" => {
                        let mut keys = Vec::with_capacity(tl.keys.len());
                        for k in &tl.keys {
                            let mode = k
                                .get("inherit")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("normal");
                            keys.push((f32r(number(k, "time")), inherit(mode, &what)?));
                        }
                        let kind = TimelineKind::Inherit { bone, keys };
                        times.extend(timeline_times(&kind));
                        timelines.push(Timeline {
                            kind,
                            properties: vec![Property::Inherit(bone)],
                            instant: true,
                        });
                        continue;
                    }
                    "rotate" => (Rotate, &["value"], vec![Property::Rotate(bone)]),
                    "translate" => (
                        Translate,
                        &["x", "y"],
                        vec![Property::X(bone), Property::Y(bone)],
                    ),
                    "translatex" => (TranslateX, &["value"], vec![Property::X(bone)]),
                    "translatey" => (TranslateY, &["value"], vec![Property::Y(bone)]),
                    "scale" => (
                        Scale,
                        &["x", "y"],
                        vec![Property::ScaleX(bone), Property::ScaleY(bone)],
                    ),
                    "scalex" => (ScaleX, &["value"], vec![Property::ScaleX(bone)]),
                    "scaley" => (ScaleY, &["value"], vec![Property::ScaleY(bone)]),
                    "shear" => (
                        Shear,
                        &["x", "y"],
                        vec![Property::ShearX(bone), Property::ShearY(bone)],
                    ),
                    "shearx" => (ShearX, &["value"], vec![Property::ShearX(bone)]),
                    "sheary" => (ShearY, &["value"], vec![Property::ShearY(bone)]),
                    other => {
                        return Err(RigError::Unresolved(format!(
                            "{what} bone \"{}\": no bone timeline \"{other}\"",
                            target.name
                        )))
                    }
                };
            let keys = number_keys(&tl.keys, |k| {
                Ok(channels.iter().map(|c| number(k, c)).collect())
            })?;
            let kind = TimelineKind::Bone { bone, kind, keys };
            times.extend(timeline_times(&kind));
            timelines.push(Timeline {
                kind,
                properties,
                instant: false,
            });
        }
    }

    let mut sequences = Vec::new();
    for skin_entry in &a.attachments {
        let skin = *names.skins.get(skin_entry.name.as_str()).ok_or_else(|| {
            RigError::Unresolved(format!("{what}: no skin \"{}\"", skin_entry.name))
        })?;
        for slot_entry in &skin_entry.slots {
            let slot = names.slot(&slot_entry.name, &what)?;
            for entry in &slot_entry.attachments {
                let placeholder = names.placeholder(&entry.name);
                let identity = *skins[skin].table.get(&(slot, placeholder)).ok_or_else(|| {
                    RigError::Unresolved(format!(
                        "{what}: skin \"{}\" files no attachment \"{}\" under slot \"{}\"",
                        skin_entry.name, entry.name, slot_entry.name
                    ))
                })?;
                if let Some(keys) = &entry.deform {
                    if let Geometry::Mesh(_) = attachments[identity].geometry {
                        let curves: Vec<Option<Vec<f64>>> = keys
                            .iter()
                            .map(|k| curve_numbers(k.curve.as_ref()))
                            .collect();
                        let stated: Vec<StatedCurve<'_>> = keys
                            .iter()
                            .zip(&curves)
                            .map(|(k, n)| stated_curve(k.curve.as_ref(), n))
                            .collect();
                        let deform_keys = keys
                            .iter()
                            .map(|k| DeformKey {
                                time: f32r(k.time),
                                stated_time: k.time,
                                curve: crate::timeline::Curve::Linear,
                                offset: k.offset,
                                vertices: k.vertices.clone(),
                            })
                            .collect();
                        let kind = TimelineKind::Deform {
                            identity,
                            keys: DeformKeys::new(deform_keys, stated),
                        };
                        times.extend(timeline_times(&kind));
                        timelines.push(Timeline {
                            kind,
                            properties: vec![Property::Deform(slot, identity)],
                            instant: false,
                        });
                    }
                }
                if let Some(keys) = &entry.sequence {
                    let mut out = Vec::with_capacity(keys.len());
                    let mut delay = 0.0;
                    for k in keys {
                        if let Some(d) = k.delay {
                            delay = f32r(d);
                        }
                        let mode = match k.mode.as_deref().unwrap_or("hold") {
                            "once" => SequenceMode::Once,
                            "loop" => SequenceMode::Loop,
                            "pingpong" => SequenceMode::Pingpong,
                            "onceReverse" => SequenceMode::OnceReverse,
                            "loopReverse" => SequenceMode::LoopReverse,
                            "pingpongReverse" => SequenceMode::PingpongReverse,
                            _ => SequenceMode::Hold,
                        };
                        times.push(f32r(k.time));
                        out.push(SequenceKey {
                            time: f32r(k.time),
                            mode,
                            index: k.index,
                            delay,
                        });
                    }
                    sequences.push((identity, out));
                }
            }
        }
    }

    if !a.events.is_empty() {
        let mut keys = Vec::with_capacity(a.events.len());
        for k in &a.events {
            let event = *event_index.get(k.name.as_str()).ok_or_else(|| {
                RigError::Unresolved(format!("{what}: event \"{}\" is not declared", k.name))
            })?;
            let def = &doc.events[event];
            let audio = def.audio.is_some();
            keys.push(EventKey {
                time: f32r(k.time),
                event,
                int: k.int.unwrap_or(def.int),
                float: k.float.unwrap_or(def.float),
                string: k.string.clone().unwrap_or_else(|| def.string.clone()),
                volume: if audio {
                    k.volume.unwrap_or(def.volume)
                } else {
                    0.0
                },
                balance: if audio {
                    k.balance.unwrap_or(def.balance)
                } else {
                    0.0
                },
            });
        }
        let kind = TimelineKind::Events(keys);
        times.extend(timeline_times(&kind));
        timelines.push(Timeline {
            kind,
            properties: vec![Property::Event],
            instant: true,
        });
    }

    let mut draw_order = Vec::with_capacity(a.draw_order.len());
    for k in &a.draw_order {
        let mut moves = Vec::with_capacity(k.offsets.len());
        for o in &k.offsets {
            moves.push((names.slot(&o.slot, &what)?, o.offset));
        }
        times.push(f32r(k.time));
        draw_order.push(DrawOrderKey {
            time: f32r(k.time),
            moves,
        });
    }

    let properties = timelines
        .iter()
        .flat_map(|t| t.properties.iter().copied())
        .collect();
    Ok(AnimationData {
        name: a.name.clone(),
        duration: times.into_iter().fold(0.0, f64::max),
        timelines,
        properties,
        draw_order,
        sequences,
    })
}

fn timeline_times(kind: &TimelineKind) -> Vec<f64> {
    match kind {
        TimelineKind::Bone { keys, .. } | TimelineKind::Colour { keys, .. } => {
            keys.keys.iter().map(|k| k.time).collect()
        }
        TimelineKind::Inherit { keys, .. } => keys.iter().map(|k| k.0).collect(),
        TimelineKind::Attachment { keys, .. } => keys.iter().map(|k| k.0).collect(),
        TimelineKind::Deform { keys, .. } => keys.keys.iter().map(|k| k.time).collect(),
        TimelineKind::Events(keys) => keys.iter().map(|k| k.time).collect(),
    }
}

/// Read access for consumers: names, pages and what attachments draw with.
impl Rig {
    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    pub fn bone_name(&self, bone: usize) -> &str {
        &self.bones[bone].name
    }

    pub fn find_bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn slot_name(&self, slot: usize) -> &str {
        &self.slots[slot].name
    }

    pub fn find_slot(&self, name: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.name == name)
    }

    pub fn skin_count(&self) -> usize {
        self.skins.len()
    }

    pub fn skin_name(&self, skin: usize) -> &str {
        &self.skins[skin].name
    }

    pub fn find_skin(&self, name: &str) -> Option<usize> {
        self.skins.iter().position(|s| s.name == name)
    }

    pub fn animation_count(&self) -> usize {
        self.animations.len()
    }

    pub fn animation_name(&self, animation: usize) -> &str {
        &self.animations[animation].name
    }

    /// The animation's duration in seconds: its last key's time.
    pub fn animation_duration(&self, animation: usize) -> f64 {
        self.animations[animation].duration
    }

    pub fn find_animation(&self, name: &str) -> Option<usize> {
        self.animations.iter().position(|a| a.name == name)
    }

    pub fn event_name(&self, event: usize) -> &str {
        &self.events[event].name
    }

    /// The name an attachment answers to: its own `name`, else its
    /// placeholder.
    pub fn attachment_name(&self, attachment: usize) -> &str {
        &self.attachments[attachment].name
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn drawable(&self, drawable: usize) -> &Drawable {
        &self.drawables[drawable]
    }

    pub fn drawables(&self) -> &[Drawable] {
        &self.drawables
    }
}
