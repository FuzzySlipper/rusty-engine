//! Buoyancy and drag (#9883): bodies a product names float in its water.
//!
//! The water is a list of planes, each a horizontal surface over a
//! rectangle of x and z with a horizontal current, a fluid density and drag
//! rates. A body takes the first plane whose rectangle holds its centre. Every
//! solver substep, the part of its shape below the surface pushes up at its
//! centroid with the weight of the water it displaces, and drag pulls its
//! velocity toward the current and slows its spin, both in proportion to the
//! submerged fraction. The water does not feel the body.
//!
//! Submerged volumes: a sphere's cap and a box's clipped tetrahedra are exact;
//! a capsule is integrated in [`CAPSULE_SLICES`] slices along its axis, each
//! slice's disk cut exactly by the surface.

use rapier3d_f64::prelude::{Rotation, Vector};

use crate::DynamicsShape;

/// Slices a capsule is integrated in along its axis.
pub const CAPSULE_SLICES: usize = 48;

/// A flat stretch of water.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsWaterPlane {
    /// The rectangle of x and z the plane covers.
    pub min: [f64; 2],
    pub max: [f64; 2],
    /// The surface's height.
    pub surface: f64,
    /// Horizontal flow, x and z.
    pub current: [f64; 2],
    /// Mass per unit volume (1000 for fresh water in kilograms and metres).
    pub density: f64,
    /// Rates, per second at full immersion, at which a body's velocity
    /// relative to the current and its spin decay.
    pub linear_drag: f64,
    pub angular_drag: f64,
}

impl DynamicsWaterPlane {
    pub(crate) fn holds(&self, point: Vector) -> bool {
        point.x >= self.min[0]
            && point.x <= self.max[0]
            && point.z >= self.min[1]
            && point.z <= self.max[1]
    }

    pub(crate) fn is_valid(&self) -> bool {
        let finite = self
            .min
            .into_iter()
            .chain(self.max)
            .chain(self.current)
            .chain([
                self.surface,
                self.density,
                self.linear_drag,
                self.angular_drag,
            ])
            .all(f64::is_finite);
        finite
            && self.min[0] <= self.max[0]
            && self.min[1] <= self.max[1]
            && self.density >= 0.0
            && self.linear_drag >= 0.0
            && self.angular_drag >= 0.0
    }
}

/// What the water did to one body over the latest step, averaged over its
/// substeps.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DynamicsWaterReadout {
    /// The part of the body under the surface at the end of the step, 0 to 1.
    pub submerged_fraction: f64,
    /// The buoyant force, and the drag force toward the current.
    pub buoyancy: [f64; 3],
    pub drag: [f64; 3],
    /// The buoyant force's torque about the centre of mass.
    pub torque: [f64; 3],
}

/// The volume of a shape under a surface and that part's centroid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Submerged {
    pub volume: f64,
    pub centroid: Vector,
}

impl Submerged {
    const NONE: Self = Self {
        volume: 0.0,
        centroid: Vector::ZERO,
    };
}

/// A shape's whole volume.
pub fn shape_volume(shape: DynamicsShape) -> f64 {
    use std::f64::consts::PI;
    match shape {
        DynamicsShape::Sphere { radius } => 4.0 / 3.0 * PI * radius.powi(3),
        DynamicsShape::Cuboid { half_extents } => {
            8.0 * half_extents[0] * half_extents[1] * half_extents[2]
        }
        DynamicsShape::CapsuleY {
            half_height,
            radius,
        } => PI * radius * radius * (2.0 * half_height + 4.0 / 3.0 * radius),
    }
}

/// The part of `shape`, posed at `translation` and `rotation`, below the
/// horizontal plane `y = surface`.
pub fn submerged(
    shape: DynamicsShape,
    translation: Vector,
    rotation: Rotation,
    surface: f64,
) -> Submerged {
    match shape {
        DynamicsShape::Sphere { radius } => sphere(translation, radius, surface),
        DynamicsShape::Cuboid { half_extents } => {
            cuboid(translation, rotation, half_extents, surface)
        }
        DynamicsShape::CapsuleY {
            half_height,
            radius,
        } => capsule(translation, rotation, half_height, radius, surface),
    }
}

fn sphere(centre: Vector, radius: f64, surface: f64) -> Submerged {
    use std::f64::consts::PI;
    let depth = surface - centre.y;
    if depth <= -radius {
        return Submerged::NONE;
    }
    if depth >= radius {
        return Submerged {
            volume: 4.0 / 3.0 * PI * radius.powi(3),
            centroid: centre,
        };
    }
    // A cap of height h below the surface; its centroid lies
    // 3(2r - h)^2 / (4(3r - h)) below the sphere's centre.
    let h = radius + depth;
    let volume = PI * h * h * (3.0 * radius - h) / 3.0;
    let below = 3.0 * (2.0 * radius - h).powi(2) / (4.0 * (3.0 * radius - h));
    Submerged {
        volume,
        centroid: centre - Vector::Y * below,
    }
}

/// The box's corners, bit 0 for +x, bit 1 for +y, bit 2 for +z.
fn corners(translation: Vector, rotation: Rotation, half: [f64; 3]) -> [Vector; 8] {
    std::array::from_fn(|i| {
        let sign = |bit: usize| if i & (1 << bit) != 0 { 1.0 } else { -1.0 };
        translation
            + rotation * Vector::new(sign(0) * half[0], sign(1) * half[1], sign(2) * half[2])
    })
}

/// Six tetrahedra about the diagonal from corner 0 to corner 7.
const BOX_TETRAHEDRA: [[usize; 4]; 6] = [
    [0, 7, 1, 3],
    [0, 7, 3, 2],
    [0, 7, 2, 6],
    [0, 7, 6, 4],
    [0, 7, 4, 5],
    [0, 7, 5, 1],
];

fn cuboid(translation: Vector, rotation: Rotation, half: [f64; 3], surface: f64) -> Submerged {
    let corners = corners(translation, rotation, half);
    let (mut volume, mut moment) = (0.0, Vector::ZERO);
    for tetrahedron in BOX_TETRAHEDRA {
        let (v, m) = clip_tetrahedron(tetrahedron.map(|i| corners[i]), surface);
        volume += v;
        moment += m;
    }
    finish(volume, moment)
}

fn finish(volume: f64, moment: Vector) -> Submerged {
    if volume <= 0.0 {
        return Submerged::NONE;
    }
    Submerged {
        volume,
        centroid: moment / volume,
    }
}

/// Volume and first moment (volume times centroid) of a tetrahedron.
fn tetrahedron(a: Vector, b: Vector, c: Vector, d: Vector) -> (f64, Vector) {
    let volume = (b - a).cross(c - a).dot(d - a).abs() / 6.0;
    (volume, (a + b + c + d) * (volume / 4.0))
}

/// The part of a tetrahedron with `y <= surface`, as volume and first moment.
fn clip_tetrahedron(points: [Vector; 4], surface: f64) -> (f64, Vector) {
    let (mut below, mut above) = ([Vector::ZERO; 4], [Vector::ZERO; 4]);
    let (mut b, mut a) = (0, 0);
    for point in points {
        if point.y <= surface {
            below[b] = point;
            b += 1;
        } else {
            above[a] = point;
            a += 1;
        }
    }
    let cut =
        |from: Vector, to: Vector| from + (to - from) * ((surface - from.y) / (to.y - from.y));
    match b {
        0 => (0.0, Vector::ZERO),
        4 => tetrahedron(points[0], points[1], points[2], points[3]),
        1 => {
            let p = below[0];
            tetrahedron(p, cut(p, above[0]), cut(p, above[1]), cut(p, above[2]))
        }
        3 => {
            let (whole, whole_moment) = tetrahedron(points[0], points[1], points[2], points[3]);
            let d = above[0];
            let (tip, tip_moment) =
                tetrahedron(d, cut(d, below[0]), cut(d, below[1]), cut(d, below[2]));
            (whole - tip, whole_moment - tip_moment)
        }
        _ => {
            // A prism: triangle (a, ac, ad) at one end, (b, bc, bd) at the other.
            let (p, q, c, d) = (below[0], below[1], above[0], above[1]);
            let (p1, p2) = (cut(p, c), cut(p, d));
            let (q1, q2) = (cut(q, c), cut(q, d));
            let parts = [
                tetrahedron(p, p1, p2, q2),
                tetrahedron(p, p1, q1, q2),
                tetrahedron(p, q, q1, q2),
            ];
            parts
                .into_iter()
                .fold((0.0, Vector::ZERO), |(v, m), (pv, pm)| (v + pv, m + pm))
        }
    }
}

fn capsule(
    centre: Vector,
    rotation: Rotation,
    half_height: f64,
    radius: f64,
    surface: f64,
) -> Submerged {
    let axis = rotation * Vector::Y;
    let reach = half_height + radius;
    // Wholly above or below: no slicing.
    let extent = half_height * axis.y.abs() + radius;
    if centre.y - extent >= surface {
        return Submerged::NONE;
    }
    if centre.y + extent <= surface {
        return Submerged {
            volume: shape_volume(DynamicsShape::CapsuleY {
                half_height,
                radius,
            }),
            centroid: centre,
        };
    }
    // The in-disk direction that rises fastest, and how fast.
    let tilt = Vector::Y - axis * axis.y;
    let rise = tilt.length();
    let up = if rise > 0.0 {
        tilt / rise
    } else {
        Vector::ZERO
    };
    let thickness = 2.0 * reach / CAPSULE_SLICES as f64;
    let (mut volume, mut moment) = (0.0, Vector::ZERO);
    for slice in 0..CAPSULE_SLICES {
        let s = -reach + (slice as f64 + 0.5) * thickness;
        let beyond = s.abs() - half_height;
        let r2 = if beyond > 0.0 {
            radius * radius - beyond * beyond
        } else {
            radius * radius
        };
        if r2 <= 0.0 {
            continue;
        }
        let r = r2.sqrt();
        let at = centre + axis * s;
        // A slice also spans its thickness along the axis; nearly level
        // slices are cut by how much of that span lies under the surface.
        let span = thickness * axis.y.abs();
        let (area, offset) = if r * rise < 0.5 * span {
            let low = at.y - 0.5 * span;
            let fraction = if span > 0.0 {
                ((surface - low) / span).clamp(0.0, 1.0)
            } else if at.y <= surface {
                1.0
            } else {
                0.0
            };
            (std::f64::consts::PI * r2 * fraction, Vector::ZERO)
        } else {
            // The disk below a chord at distance t from its centre along `up`.
            let t = ((surface - at.y) / rise).clamp(-r, r);
            let root = (r2 - t * t).max(0.0).sqrt();
            let area = r2 * (std::f64::consts::PI - (t / r).acos()) + t * root;
            let first = -2.0 / 3.0 * (r2 - t * t).max(0.0).powf(1.5);
            (area, up * first)
        };
        volume += area * thickness;
        moment += (at * area + offset) * thickness;
    }
    finish(volume, moment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance * b.abs().max(1.0e-12)
    }

    #[test]
    fn a_level_box_submerges_by_its_draft() {
        let half = [1.0, 0.5, 2.0];
        let shape = DynamicsShape::Cuboid { half_extents: half };
        let part = submerged(shape, Vector::new(3.0, 0.0, -1.0), Rotation::IDENTITY, 0.25);
        // 0.75 of 1.0 height under water.
        assert!(close(part.volume, 2.0 * 4.0 * 0.75, 1.0e-12), "{part:?}");
        assert!(close(part.centroid.y, 0.25 - 0.375, 1.0e-12));
        assert!(close(part.centroid.x, 3.0, 1.0e-12) && close(part.centroid.z, -1.0, 1.0e-12));
    }

    #[test]
    fn a_tilted_box_halved_by_a_plane_through_its_centre_keeps_half_its_volume() {
        let shape = DynamicsShape::Cuboid {
            half_extents: [1.0, 0.3, 0.7],
        };
        let rotation = Rotation::from_axis_angle(Vector::new(1.0, 2.0, 0.5).normalize(), 0.9);
        let part = submerged(shape, Vector::new(0.0, 2.0, 0.0), rotation, 2.0);
        assert!(close(part.volume, shape_volume(shape) / 2.0, 1.0e-12));
        assert!(part.centroid.y < 2.0);
        // Rolled 45 degrees about z, the lower half's centroid shifts toward the low side.
        let rolled = Rotation::from_rotation_z(FRAC_PI_4);
        let part = submerged(shape, Vector::ZERO, rolled, 0.0);
        assert!(close(part.volume, shape_volume(shape) / 2.0, 1.0e-12));
    }

    #[test]
    fn sphere_caps_match_their_closed_form() {
        let r = 0.8;
        let shape = DynamicsShape::Sphere { radius: r };
        for depth in [-0.8, -0.3, 0.0, 0.5, 0.8] {
            let part = submerged(shape, Vector::ZERO, Rotation::IDENTITY, depth);
            let h = r + depth;
            assert!(close(
                part.volume,
                PI * h * h * (3.0 * r - h) / 3.0,
                1.0e-12
            ));
        }
        let half = submerged(shape, Vector::ZERO, Rotation::IDENTITY, 0.0);
        assert!(close(half.centroid.y, -3.0 * r / 8.0, 1.0e-12));
    }

    /// The capsule's slices against a fine integration, upright, lying and
    /// tilted, at several depths.
    #[test]
    fn capsule_slices_stay_within_half_a_percent_of_its_volume() {
        let (half_height, radius) = (0.6, 0.4);
        let shape = DynamicsShape::CapsuleY {
            half_height,
            radius,
        };
        let whole = shape_volume(shape);
        for angle in [0.0, 0.2, FRAC_PI_4, 1.3, FRAC_PI_2] {
            let rotation = Rotation::from_rotation_z(angle);
            for depth in [-0.7, -0.35, 0.0, 0.2, 0.55] {
                let part = submerged(shape, Vector::ZERO, rotation, depth);
                let reference = sampled(half_height, radius, rotation, depth);
                assert!(
                    (part.volume - reference).abs() < 0.005 * whole,
                    "tilt {angle}, surface {depth}: {} against {reference}",
                    part.volume
                );
            }
        }
    }

    /// The submerged volume by a fine grid of points.
    fn sampled(half_height: f64, radius: f64, rotation: Rotation, surface: f64) -> f64 {
        const N: i32 = 160;
        let reach = half_height + radius;
        let step = 2.0 * reach / f64::from(N);
        let inverse = rotation.inverse();
        let mut count = 0usize;
        for i in 0..N {
            for j in 0..N {
                for k in 0..N {
                    let p = Vector::new(
                        -reach + (f64::from(i) + 0.5) * step,
                        -reach + (f64::from(j) + 0.5) * step,
                        -reach + (f64::from(k) + 0.5) * step,
                    );
                    if p.y > surface {
                        continue;
                    }
                    let local = inverse * p;
                    let along = local.y.clamp(-half_height, half_height);
                    let d2 = local.x * local.x + (local.y - along).powi(2) + local.z * local.z;
                    if d2 <= radius * radius {
                        count += 1;
                    }
                }
            }
        }
        count as f64 * step.powi(3)
    }
}
