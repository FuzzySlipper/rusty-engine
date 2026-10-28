//! Air moving past a body pushes it: the same quadratic drag soft-body
//! surfaces feel, on the face a body's shape shows the wind. A thin box is a
//! plate (a cape bone, a sail, a leaf) pushed along its thinnest axis; a
//! capsule is a cylinder pushed across its axis; a ball by the whole flow.

use rapier3d_f64::prelude::Vector;

use crate::dynamics::DynamicsShape;

/// Air moving at `wind` (m/s) past a body, pushing it by
/// `drag × area × v|v|`, `v` the air's speed through its face. `drag` is half
/// the air's density times a drag coefficient (0.6 for a flat plate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsWind {
    pub wind: [f64; 3],
    pub drag: f64,
}

impl DynamicsWind {
    pub(crate) fn is_valid(&self) -> bool {
        self.wind.iter().copied().all(f64::is_finite) && self.drag.is_finite() && self.drag >= 0.0
    }
}

/// The face a body shows the wind, in its own frame.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Face {
    /// Pushed along `normal` only.
    Plate { normal: Vector, area: f64 },
    /// Pushed across `axis` only.
    Cylinder { axis: Vector, area: f64 },
    /// Pushed along the flow.
    Ball { area: f64 },
}

impl Face {
    pub(crate) fn of(shape: DynamicsShape) -> Self {
        match shape {
            DynamicsShape::Cuboid { half_extents } => {
                let thinnest = (0..3)
                    .min_by(|a, b| half_extents[*a].total_cmp(&half_extents[*b]))
                    .unwrap_or(2);
                let mut normal = Vector::ZERO;
                normal[thinnest] = 1.0;
                let area = (0..3)
                    .filter(|axis| *axis != thinnest)
                    .map(|axis| 2.0 * half_extents[axis])
                    .product();
                Face::Plate { normal, area }
            }
            DynamicsShape::CapsuleY {
                half_height,
                radius,
            } => Face::Cylinder {
                axis: Vector::Y,
                area: 2.0 * radius * 2.0 * (half_height + radius),
            },
            DynamicsShape::Sphere { radius } => Face::Ball {
                area: std::f64::consts::PI * radius * radius,
            },
        }
    }

    /// The force of air moving at `air` relative to the body, turned by
    /// `rotation` into the world.
    pub(crate) fn force(
        self,
        rotation: rapier3d_f64::prelude::Rotation,
        air: Vector,
        drag: f64,
    ) -> Vector {
        match self {
            Face::Plate { normal, area } => {
                let normal = rotation * normal;
                let through = air.dot(normal);
                normal * (drag * area * through * through.abs())
            }
            Face::Cylinder { axis, area } => {
                let axis = rotation * axis;
                let across = air - axis * air.dot(axis);
                across * (drag * area * across.length())
            }
            Face::Ball { area } => air * (drag * area * air.length()),
        }
    }
}

#[cfg(test)]
mod tests {
    use rapier3d_f64::prelude::Rotation;

    use super::*;

    #[test]
    fn a_plate_feels_only_the_air_through_it() {
        let plate = Face::of(DynamicsShape::Cuboid {
            half_extents: [0.5, 0.5, 0.01],
        });
        // Face-on: 0.6 × 1 m² × 10² along its normal.
        let face_on = plate.force(Rotation::IDENTITY, Vector::new(0.0, 0.0, 10.0), 0.6);
        assert!(
            (face_on - Vector::new(0.0, 0.0, 60.0)).length() < 1e-9,
            "{face_on:?}"
        );
        // Edge-on, nothing.
        let edge_on = plate.force(Rotation::IDENTITY, Vector::new(10.0, 0.0, 0.0), 0.6);
        assert!(edge_on.length() < 1e-9);
        // Turned a quarter about Y, the wind along X is face-on.
        let turned = plate.force(
            Rotation::from_rotation_y(std::f64::consts::FRAC_PI_2),
            Vector::new(10.0, 0.0, 0.0),
            0.6,
        );
        assert!(
            (turned - Vector::new(60.0, 0.0, 0.0)).length() < 1e-9,
            "{turned:?}"
        );
    }

    #[test]
    fn a_capsule_feels_the_air_across_it() {
        let capsule = Face::of(DynamicsShape::CapsuleY {
            half_height: 0.4,
            radius: 0.1,
        });
        let along = capsule.force(Rotation::IDENTITY, Vector::new(0.0, 5.0, 0.0), 0.6);
        assert!(along.length() < 1e-9);
        let across = capsule.force(Rotation::IDENTITY, Vector::new(5.0, 0.0, 0.0), 0.6);
        // 0.2 m wide, 1 m long.
        assert!((across.x - 0.6 * 0.2 * 25.0).abs() < 1e-9, "{across:?}");
    }
}
