use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::EntityState;
use render_model::{
    Geometry, Material, RenderFrameDiff, RenderHandle, RenderLayer, RenderMetadata, RenderNode,
    Transform,
};

use crate::{RenderHandleNamespace, RetainedNodeProjector, RetainedProjectionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DebugOverlayId(u64);

impl DebugOverlayId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DebugOverlayPrimitive {
    Point { position: [f32; 3] },
    Line { a: [f32; 3], b: [f32; 3] },
    Label { position: [f32; 3], text: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugOverlayDescriptor {
    pub id: DebugOverlayId,
    pub primitive: DebugOverlayPrimitive,
    pub color: [f32; 4],
    pub wireframe: bool,
    pub visible: bool,
    pub source: Option<EntityId>,
    pub tags: Vec<String>,
    pub label: Option<String>,
}

impl DebugOverlayDescriptor {
    pub fn point(id: DebugOverlayId, position: [f32; 3]) -> Self {
        Self {
            id,
            primitive: DebugOverlayPrimitive::Point { position },
            color: [1.0, 1.0, 0.0, 1.0],
            wireframe: true,
            visible: true,
            source: None,
            tags: Vec::new(),
            label: None,
        }
    }

    pub fn line(id: DebugOverlayId, a: [f32; 3], b: [f32; 3]) -> Self {
        Self {
            id,
            primitive: DebugOverlayPrimitive::Line { a, b },
            color: [0.0, 1.0, 0.0, 1.0],
            wireframe: false,
            visible: true,
            source: None,
            tags: Vec::new(),
            label: None,
        }
    }

    pub fn label(id: DebugOverlayId, position: [f32; 3], text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            id,
            primitive: DebugOverlayPrimitive::Label {
                position,
                text: text.clone(),
            },
            color: [1.0; 4],
            wireframe: false,
            visible: true,
            source: None,
            tags: Vec::new(),
            label: Some(text),
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_wireframe(mut self, wireframe: bool) -> Self {
        self.wireframe = wireframe;
        self
    }

    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn with_source(mut self, source: EntityId) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = String>) -> Self {
        self.tags = tags.into_iter().collect();
        self.tags.sort();
        self.tags.dedup();
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn validate(&self) -> Result<(), DebugProjectionError> {
        if !self
            .color
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(DebugProjectionError::InvalidColor { id: self.id });
        }
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        match &self.primitive {
            DebugOverlayPrimitive::Point { position } => {
                if !finite(position) {
                    return Err(DebugProjectionError::InvalidPosition { id: self.id });
                }
            }
            DebugOverlayPrimitive::Line { a, b } => {
                if !finite(a) || !finite(b) {
                    return Err(DebugProjectionError::InvalidPosition { id: self.id });
                }
            }
            DebugOverlayPrimitive::Label { position, text } => {
                if !finite(position) {
                    return Err(DebugProjectionError::InvalidPosition { id: self.id });
                }
                if text.trim().is_empty() {
                    return Err(DebugProjectionError::EmptyText { id: self.id });
                }
            }
        }
        if self
            .label
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(DebugProjectionError::EmptyText { id: self.id });
        }
        if self.tags.iter().any(|value| value.trim().is_empty())
            || self.tags.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(DebugProjectionError::NonCanonicalTags { id: self.id });
        }
        Ok(())
    }

    fn to_node(&self) -> RenderNode {
        let (geometry, translation, label) = match &self.primitive {
            DebugOverlayPrimitive::Point { position } => {
                (Geometry::Point, *position, self.label.clone())
            }
            DebugOverlayPrimitive::Line { a, b } => (
                Geometry::Line { a: *a, b: *b },
                [0.0; 3],
                self.label.clone(),
            ),
            DebugOverlayPrimitive::Label { position, text } => (
                Geometry::Point,
                *position,
                self.label.clone().or_else(|| Some(text.clone())),
            ),
        };
        RenderNode {
            geometry,
            material: Material {
                color: self.color,
                wireframe: self.wireframe,
            },
            transform: Transform {
                translation,
                ..Transform::IDENTITY
            },
            visible: self.visible,
            layer: RenderLayer::Debug,
            metadata: RenderMetadata {
                source_entity: self.source.map(EntityId::raw),
                source_scene_node: None,
                tags: self.tags.clone(),
                label,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebugOverlayProjector {
    retained: RetainedNodeProjector<DebugOverlayId>,
}

impl Default for DebugOverlayProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugOverlayProjector {
    pub fn new() -> Self {
        Self {
            retained: RetainedNodeProjector::new(RenderHandleNamespace::DEBUG),
        }
    }

    pub fn project(
        &mut self,
        descriptors: &[DebugOverlayDescriptor],
    ) -> Result<DebugProjectionResult, DebugProjectionError> {
        let mut ids = BTreeSet::new();
        let mut nodes = BTreeMap::new();
        for descriptor in descriptors {
            if !ids.insert(descriptor.id) {
                return Err(DebugProjectionError::DuplicateId { id: descriptor.id });
            }
            descriptor.validate()?;
            nodes.insert(descriptor.id, descriptor.to_node());
        }
        let frame = self
            .retained
            .project(nodes)
            .map_err(DebugProjectionError::Retained)?;
        Ok(DebugProjectionResult {
            frame,
            readout: DebugProjectionReadout {
                retained_overlays: self.retained.retained_len(),
            },
        })
    }

    pub fn handle_of(&self, id: DebugOverlayId) -> Option<RenderHandle> {
        self.retained.handle_of(&id)
    }
}

/// Conventional entity-label overlay helper. It is an explicit read of
/// object-centric state, not a component update callback.
pub fn entity_debug_labels(state: &EntityState) -> Vec<DebugOverlayDescriptor> {
    state
        .projection()
        .into_iter()
        .map(|node| {
            let translation = node.translation.unwrap_or_default();
            DebugOverlayDescriptor::label(
                DebugOverlayId::new(node.entity.raw()),
                [translation.x, translation.y + 1.0, translation.z],
                format!("{} #{}", node.name, node.entity.raw()),
            )
            .with_source(node.entity)
            .with_tags(["entity-label".to_string()])
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugProjectionResult {
    pub frame: RenderFrameDiff,
    pub readout: DebugProjectionReadout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugProjectionReadout {
    pub retained_overlays: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DebugProjectionError {
    DuplicateId { id: DebugOverlayId },
    InvalidColor { id: DebugOverlayId },
    InvalidPosition { id: DebugOverlayId },
    EmptyText { id: DebugOverlayId },
    NonCanonicalTags { id: DebugOverlayId },
    Retained(RetainedProjectionError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_line_and_label_lifecycle_is_retained() {
        let mut projector = DebugOverlayProjector::new();
        let point = DebugOverlayDescriptor::point(DebugOverlayId::new(1), [1.0, 2.0, 3.0]);
        let line =
            DebugOverlayDescriptor::line(DebugOverlayId::new(2), [0.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
        let label =
            DebugOverlayDescriptor::label(DebugOverlayId::new(3), [2.0, 2.0, 0.0], "label-a");
        let first = projector.project(&[point.clone(), line, label]).unwrap();
        assert_eq!(first.frame.len(), 3);
        let point_handle = projector.handle_of(point.id).unwrap();
        let second = projector
            .project(&[point.with_color([1.0, 0.0, 0.0, 1.0])])
            .unwrap();
        assert_eq!(second.frame.len(), 3);
        assert_eq!(
            projector.handle_of(DebugOverlayId::new(1)),
            Some(point_handle)
        );
    }

    #[test]
    fn duplicate_id_fails_before_retained_state_changes() {
        let mut projector = DebugOverlayProjector::new();
        let id = DebugOverlayId::new(1);
        let point = DebugOverlayDescriptor::point(id, [0.0; 3]);
        assert!(matches!(
            projector.project(&[point.clone(), point]),
            Err(DebugProjectionError::DuplicateId { .. })
        ));
        assert_eq!(projector.handle_of(id), None);
    }
}
