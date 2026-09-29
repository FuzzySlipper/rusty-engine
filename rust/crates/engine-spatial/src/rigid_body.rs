use entity_state::{Quat, RigidBodyComponent, RigidBodyShape};

use core_math::Vec3;

/// Purpose-neutral mass facts for an admitted dynamic primitive. The native C#
/// bridge reports these values so product control code can use Engine's shape
/// and mass policy without copying an inertia formula downstream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyMassProperties {
    pub mass: f32,
    pub center_of_mass: Vec3,
    pub principal_inertia: Vec3,
    pub principal_inertia_local_frame: Quat,
}

pub fn rigid_body_mass_properties(
    shape: RigidBodyShape,
    mass: f32,
) -> Option<RigidBodyMassProperties> {
    let body = RigidBodyComponent::dynamic(shape, mass);
    entity_state::validate_rigid_body(&body).ok()?;
    let principal_inertia = match shape {
        // Ixx = m / 12 * ((2hy)^2 + (2hz)^2), and cyclic permutations.
        RigidBodyShape::Cuboid { half_extents } => {
            let scale = mass / 3.0;
            Vec3::new(
                scale * (half_extents.y * half_extents.y + half_extents.z * half_extents.z),
                scale * (half_extents.x * half_extents.x + half_extents.z * half_extents.z),
                scale * (half_extents.x * half_extents.x + half_extents.y * half_extents.y),
            )
        }
        RigidBodyShape::Sphere { radius } => {
            let inertia = 0.4 * mass * radius * radius;
            Vec3::new(inertia, inertia, inertia)
        }
        // Capsule admission is intentionally not part of the generated C#
        // Dynamics family yet, so it has no readout policy in this slice.
        RigidBodyShape::CapsuleY { .. } => return None,
    };
    Some(RigidBodyMassProperties {
        mass,
        center_of_mass: Vec3::ZERO,
        principal_inertia,
        principal_inertia_local_frame: Quat::IDENTITY,
    })
}

/// Return the exact mass-property tuple selected by an authored body.
///
/// The shape/mass helper above remains the compatibility entry point for
/// callers that want Engine's derived defaults. This body-oriented readout
/// preserves an explicit policy without asking a downstream caller to repeat
/// the policy selection.
pub fn rigid_body_component_mass_properties(
    body: RigidBodyComponent,
) -> Option<RigidBodyMassProperties> {
    entity_state::validate_rigid_body(&body).ok()?;
    match body.inertia {
        entity_state::RigidBodyInertiaPolicy::DeriveFromShapeAndMass => {
            rigid_body_mass_properties(body.shape, body.mass)
        }
        entity_state::RigidBodyInertiaPolicy::Explicit {
            center_of_mass,
            principal_inertia,
            principal_inertia_local_frame,
        } => Some(RigidBodyMassProperties {
            mass: body.mass,
            center_of_mass,
            principal_inertia,
            principal_inertia_local_frame,
        }),
    }
}

/// Compatibility helper for existing cuboid callers.
pub fn cuboid_mass_properties(half_extents: Vec3, mass: f32) -> Option<RigidBodyMassProperties> {
    rigid_body_mass_properties(RigidBodyShape::Cuboid { half_extents }, mass)
}
