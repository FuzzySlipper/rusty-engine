//! Overlay sets: inspection drawing on the Debug layer. A product publishes
//! named sets of lines, shapes and labels, each placed in a stated space;
//! the renderer resolves and draws them only while inspection is on. A set
//! may also hold landmark pairs, which the renderer measures as drawn.

use serde::{Deserialize, Serialize};

/// One named overlay set's items and landmark pairs, replaced whole.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlaySetDescriptor {
    pub items: Vec<OverlayItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<LandmarkPair>,
}

impl OverlaySetDescriptor {
    /// A set with nothing in it, which removes the set it replaces.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.pairs.is_empty()
    }
}

/// One overlay item: a shape in a space, its colour, and whether the scene
/// hides it where it is behind something (`on_top` false) or not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlayItem {
    pub shape: OverlayShape,
    pub space: OverlaySpace,
    /// Linear RGBA.
    pub color: [f32; 4],
    #[serde(default)]
    pub on_top: bool,
}

/// The space an item's values are in. An entity names the scene node its
/// product published for it (`RenderMetadata::source_entity`); an item
/// whose entity or joint draws nothing is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OverlaySpace {
    World,
    /// The entity node's own space.
    Model {
        entity: u64,
    },
    /// A rig joint of the entity's animated mesh, as drawn: an index into
    /// its rig's joints (`ReadJoints`). The joint's position and
    /// orientation place the item; its scale does not.
    Joint {
        entity: u64,
        joint: u32,
    },
    /// The primary camera's space: x right, y up, looking down -z. A
    /// viewmodel's parts are placed in it.
    Camera,
}

/// What an item draws, in its space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OverlayShape {
    Segment {
        from: [f32; 3],
        to: [f32; 3],
    },
    /// Three short crossed lines `size` long, centred on `at`.
    Marker {
        at: [f32; 3],
        size: f32,
    },
    /// The x, y and z axes of the frame at `at` turned by `rotation`
    /// (`[x, y, z, w]`), `length` long, in red, green and blue: the item's
    /// colour is not used.
    Axes {
        at: [f32; 3],
        rotation: [f32; 4],
        length: f32,
    },
    WireBox {
        center: [f32; 3],
        half_extents: [f32; 3],
        rotation: [f32; 4],
    },
    /// Three great circles.
    WireSphere {
        center: [f32; 3],
        radius: f32,
    },
    /// A perspective camera's frustum from `at`, looking down its -z turned
    /// by `rotation`.
    Frustum {
        at: [f32; 3],
        rotation: [f32; 4],
        fov_y_degrees: f32,
        aspect: f32,
        near: f32,
        far: f32,
    },
    /// Text drawn facing the viewer, anchored at `at`.
    Label {
        at: [f32; 3],
        text: String,
    },
}

/// Two landmarks the renderer measures as drawn: how far apart they are,
/// judged against `tolerance` (metres; 0 leaves the distance unjudged), and
/// with an `aim` the angle between the `from` landmark's axis and what it
/// aims at, judged against `angle_tolerance` (degrees).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LandmarkPair {
    /// One word, as commands and labels name it.
    pub name: String,
    pub from: Landmark,
    pub to: Landmark,
    #[serde(default)]
    pub tolerance: f32,
    #[serde(default)]
    pub aim: LandmarkAim,
    #[serde(default)]
    pub angle_tolerance: f32,
}

/// What a pair's `from` axis is measured against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LandmarkAim {
    /// No angle.
    #[default]
    None,
    /// The `to` landmark's axis.
    Axis,
    /// The way from the `from` landmark to the `to` landmark.
    Point,
    /// The way to the nearest point of the line through the `to` landmark
    /// along its axis: a palm facing a handle.
    Line,
}

/// A point the renderer finds as drawn, with an axis and a frame. A
/// landmark on joints names rig joints of the entity's animated mesh by
/// index (`ReadJoints`), placed as drawn without the rig's joint scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Landmark {
    /// A point in an overlay space. `axis` (zero for none) and the frame
    /// the pair's offset is read along are the space's frame turned by
    /// `rotation`.
    At {
        space: OverlaySpace,
        point: [f32; 3],
        rotation: [f32; 4],
        axis: [f32; 3],
    },
    /// `amount` of the way from joint `from` to joint `to`: the web between
    /// thumb and index. Its axis runs from `from` to `to`; its frame is
    /// `from`'s.
    Between {
        entity: u64,
        from: u32,
        to: u32,
        amount: f32,
    },
    /// Past joint `last` along its own bone, `amount` of the length from
    /// joint `previous` to it: a fingertip's pad. Its axis runs along that
    /// bone; its frame is `last`'s.
    Beyond {
        entity: u64,
        previous: u32,
        last: u32,
        amount: f32,
    },
    /// A palm's centre, `along` of the way from the `wrist` joint to the
    /// `middle` finger's base and `depth` metres out of the palm, with the
    /// palm's normal as its axis: across the `index` and `pinky` bases,
    /// turned for a `left` hand. Its frame is the wrist's.
    Palm {
        entity: u64,
        wrist: u32,
        index: u32,
        middle: u32,
        pinky: u32,
        along: f32,
        depth: f32,
        left: bool,
    },
}
