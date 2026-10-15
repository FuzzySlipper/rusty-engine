//! Overlay sets: inspection drawing on the Debug layer. A product publishes
//! named sets of lines, shapes and labels, each placed in a stated space;
//! the renderer resolves and draws them only while inspection is on.

use serde::{Deserialize, Serialize};

/// One named overlay set's items, replaced whole.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlaySetDescriptor {
    pub items: Vec<OverlayItem>,
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
