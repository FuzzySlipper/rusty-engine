use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpatialGridCoordinateSystem {
    RightHandedYUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpatialGridSpec {
    pub coordinate_system: SpatialGridCoordinateSystem,
    pub origin: [f64; 3],
    pub spacing: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorGridPlane {
    Xz,
    Xy,
    Yz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpatialGridSnapAnchor {
    Boundary,
    CellCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorGridStyle {
    pub minor_color: [f32; 4],
    pub major_color: [f32; 4],
    pub x_axis_color: [f32; 4],
    pub y_axis_color: [f32; 4],
    pub z_axis_color: [f32; 4],
    pub major_line_every: u32,
    pub opacity: f32,
    pub fade_start: f64,
    pub fade_end: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorGridDescriptor {
    pub visible: bool,
    pub grid: SpatialGridSpec,
    pub plane: EditorGridPlane,
    pub snap_anchor: SpatialGridSnapAnchor,
    pub style: EditorGridStyle,
}

impl EditorGridDescriptor {
    pub fn validate(&self) -> Result<(), EditorGridDescriptorError> {
        if !self.grid.origin.iter().all(|value| value.is_finite()) {
            return Err(EditorGridDescriptorError::InvalidOrigin);
        }
        if !self
            .grid
            .spacing
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        {
            return Err(EditorGridDescriptorError::InvalidSpacing);
        }
        if ![
            self.style.minor_color,
            self.style.major_color,
            self.style.x_axis_color,
            self.style.y_axis_color,
            self.style.z_axis_color,
        ]
        .iter()
        .flatten()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(EditorGridDescriptorError::InvalidColor);
        }
        if self.style.major_line_every == 0 {
            return Err(EditorGridDescriptorError::InvalidMajorLineCadence);
        }
        if !self.style.opacity.is_finite() || !(0.0..=1.0).contains(&self.style.opacity) {
            return Err(EditorGridDescriptorError::InvalidOpacity);
        }
        if !self.style.fade_start.is_finite()
            || !self.style.fade_end.is_finite()
            || self.style.fade_start < 0.0
            || self.style.fade_end <= self.style.fade_start
        {
            return Err(EditorGridDescriptorError::InvalidFadeRange);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGridDescriptorError {
    InvalidOrigin,
    InvalidSpacing,
    InvalidColor,
    InvalidMajorLineCadence,
    InvalidOpacity,
    InvalidFadeRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorGridBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorGridProjectionReadout {
    pub descriptor: EditorGridDescriptor,
    pub bounds: Option<EditorGridBounds>,
    pub minor_line_step: u32,
    pub rendered_line_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_validation_carries_the_y_up_grid_contract() {
        let descriptor = EditorGridDescriptor {
            visible: true,
            grid: SpatialGridSpec {
                coordinate_system: SpatialGridCoordinateSystem::RightHandedYUp,
                origin: [0.0; 3],
                spacing: [1.0; 3],
            },
            plane: EditorGridPlane::Xz,
            snap_anchor: SpatialGridSnapAnchor::Boundary,
            style: EditorGridStyle {
                minor_color: [0.1, 0.1, 0.1, 0.4],
                major_color: [0.2, 0.2, 0.2, 0.8],
                x_axis_color: [1.0, 0.0, 0.0, 1.0],
                y_axis_color: [0.0, 1.0, 0.0, 1.0],
                z_axis_color: [0.0, 0.0, 1.0, 1.0],
                major_line_every: 4,
                opacity: 1.0,
                fade_start: 12.0,
                fade_end: 48.0,
            },
        };
        assert_eq!(descriptor.validate(), Ok(()));
    }
}
