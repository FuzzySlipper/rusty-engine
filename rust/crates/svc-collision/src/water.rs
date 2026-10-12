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
//! Submerged volumes: a sphere's cap and a box's clipped tetrahedra are exact.
//! A capsule is its cylinder and two hemispheres: exact when upright or level,
//! otherwise each integrated by piecewise Gauss-Legendre in slices cut exactly
//! by the surface, split where the surface is tangent to a slice, to within
//! 1e-6 of the submerged volume.

use rapier3d_f64::prelude::{Rotation, Vector};

use crate::DynamicsShape;

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
    /// The water's torque about the centre of mass: the buoyant force's
    /// moment and the angular drag.
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

/// Gauss-Legendre nodes and weights on [-1, 1], the positive half of 16.
const GAUSS: [(f64, f64); 8] = [
    (0.095_012_509_837_637_44, 0.189_450_610_455_068_5),
    (0.281_603_550_779_258_9, 0.182_603_415_044_923_6),
    (0.458_016_777_657_227_4, 0.169_156_519_395_002_5),
    (0.617_876_244_402_643_7, 0.149_595_988_816_576_7),
    (0.755_404_408_355_003, 0.124_628_971_255_533_9),
    (0.865_631_202_387_831_7, 0.095_158_511_682_492_78),
    (0.944_575_023_073_232_6, 0.062_253_523_938_647_89),
    (0.989_400_934_991_649_9, 0.027_152_459_411_754_09),
];

/// Composite Gauss-Legendre of a volume density and its first moment over
/// `[a, b]`, split at `breaks` (where the integrand's derivative is not
/// smooth) and each piece into [`GAUSS_PIECES`].
fn integrate(
    a: f64,
    b: f64,
    breaks: &[f64],
    density: impl Fn(f64) -> (f64, Vector),
) -> (f64, Vector) {
    let mut cuts: Vec<f64> = std::iter::once(a)
        .chain(breaks.iter().copied().filter(|&x| x > a && x < b))
        .chain(std::iter::once(b))
        .collect();
    cuts.sort_by(f64::total_cmp);
    let (mut volume, mut moment) = (0.0, Vector::ZERO);
    for pair in cuts.windows(2) {
        let step = (pair[1] - pair[0]) / GAUSS_PIECES as f64;
        for piece in 0..GAUSS_PIECES {
            let low = pair[0] + piece as f64 * step;
            let (middle, half) = (low + 0.5 * step, 0.5 * step);
            for (node, weight) in GAUSS {
                for x in [middle - half * node, middle + half * node] {
                    let (v, m) = density(x);
                    volume += weight * half * v;
                    moment += m * (weight * half);
                }
            }
        }
    }
    (volume, moment)
}

/// Pieces each smooth stretch is split into.
const GAUSS_PIECES: usize = 2;

/// Below this, an axis counts as upright or level and its exact closed form
/// is used.
const LEVEL: f64 = 1.0e-9;

/// The part of a disk of radius `r` on one side of a chord at signed
/// distance `t` from its centre: its area, and the first moment of that
/// part along the chord's normal (pointing to the side kept).
fn beyond_chord(r: f64, t: f64) -> (f64, f64) {
    if t >= r {
        return (0.0, 0.0);
    }
    if t <= -r {
        return (std::f64::consts::PI * r * r, 0.0);
    }
    let root = (r * r - t * t).sqrt();
    (r * r * (t / r).acos() - t * root, 2.0 / 3.0 * root.powi(3))
}

/// A capsule: a cylinder along its axis and a hemisphere at each end, each
/// integrated exactly where a closed form exists (upright or level) and by
/// piecewise Gauss-Legendre elsewhere, split where the surface is tangent
/// to a slice.
fn capsule(
    centre: Vector,
    rotation: Rotation,
    half_height: f64,
    radius: f64,
    surface: f64,
) -> Submerged {
    let axis = rotation * Vector::Y;
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
    let (mut volume, mut moment) = cylinder(centre, axis, half_height, radius, surface);
    for sign in [1.0, -1.0] {
        let (v, m) = hemisphere(
            centre + axis * (sign * half_height),
            axis * sign,
            radius,
            surface,
        );
        volume += v;
        moment += m;
    }
    finish(volume, moment)
}

/// The cylinder between `centre ± axis·half_height` below the surface.
fn cylinder(
    centre: Vector,
    axis: Vector,
    half_height: f64,
    radius: f64,
    surface: f64,
) -> (f64, Vector) {
    let area = std::f64::consts::PI * radius * radius;
    let tilt = Vector::Y - axis * axis.y;
    let rise = tilt.length();
    if rise < LEVEL {
        // Upright: whole disks, below the surface along part of the axis.
        let below = (surface - centre.y) / axis.y;
        let (low, high) = if axis.y > 0.0 {
            (-half_height, below.min(half_height))
        } else {
            (below.max(-half_height), half_height)
        };
        if high <= low {
            return (0.0, Vector::ZERO);
        }
        let volume = area * (high - low);
        return (volume, (centre + axis * (0.5 * (low + high))) * volume);
    }
    // Each slice's disk, cut by the chord where it meets the surface, at
    // distance t along the in-disk direction that rises fastest.
    let up = tilt / rise;
    let chord = |s: f64| (surface - centre.y - s * axis.y) / rise;
    let density = |s: f64| {
        // The part below the surface lies beyond the chord at -t looking
        // down the disk.
        let (part, first) = beyond_chord(radius, -chord(s));
        (part, (centre + axis * s) * part - up * first)
    };
    let mut breaks = Vec::new();
    if axis.y.abs() > 0.0 {
        for t in [radius, -radius] {
            breaks.push((surface - centre.y - t * rise) / axis.y);
        }
    }
    integrate(-half_height, half_height, &breaks, density)
}

/// The hemisphere of radius `radius` about `end`, on the side `outward`
/// points to, below the surface: integrated in horizontal slices, each a
/// disk cut by the hemisphere's flat face.
fn hemisphere(end: Vector, outward: Vector, radius: f64, surface: f64) -> (f64, Vector) {
    use std::f64::consts::PI;
    let top = (surface - end.y).min(radius);
    if top <= -radius {
        return (0.0, Vector::ZERO);
    }
    let flat = outward - Vector::Y * outward.y;
    let sideways = flat.length();
    if sideways < LEVEL {
        // Upright: the slices are whole disks on the outward side of the
        // centre's height.
        let (low, high) = if outward.y > 0.0 {
            (0.0, top)
        } else {
            (-radius, top.min(0.0))
        };
        if high <= low {
            return (0.0, Vector::ZERO);
        }
        let r2 = radius * radius;
        let volume = PI * (r2 * (high - low) - (high.powi(3) - low.powi(3)) / 3.0);
        let height =
            PI * (r2 * (high * high - low * low) / 2.0 - (high.powi(4) - low.powi(4)) / 4.0);
        return (volume, end * volume + Vector::Y * height);
    }
    let across = flat / sideways;
    let density = |z: f64| {
        let r = (radius * radius - z * z).max(0.0).sqrt();
        // Within the slice, the flat face keeps w·across >= -z·outward.y / sideways.
        let (part, first) = beyond_chord(r, -z * outward.y / sideways);
        (part, (end + Vector::Y * z) * part + across * first)
    };
    let tangent = radius * sideways;
    integrate(-radius, top, &[-tangent, tangent], density)
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

    /// R9883-2: an upright capsule dipped 1 cm is a spherical cap, volume
    /// and centroid exact; lying down its cylinder is a segment prism and its
    /// ends a sphere's cap, exact too.
    #[test]
    fn capsule_ends_and_lying_capsules_match_their_closed_forms() {
        let (hh, r) = (0.6, 0.4);
        let shape = DynamicsShape::CapsuleY {
            half_height: hh,
            radius: r,
        };
        for (rotation, bottom) in [
            (Rotation::IDENTITY, Vector::new(0.0, -hh, 0.0)),
            (Rotation::from_rotation_z(PI), Vector::new(0.0, -hh, 0.0)),
        ] {
            let h = 0.01;
            let surface = bottom.y - r + h;
            let part = submerged(shape, Vector::ZERO, rotation, surface);
            let exact = PI * h * h * (3.0 * r - h) / 3.0;
            assert!(
                close(part.volume, exact, 1.0e-9),
                "{} against {exact}",
                part.volume
            );
            let below = 3.0 * (2.0 * r - h).powi(2) / (4.0 * (3.0 * r - h));
            assert!(close(part.centroid.y, bottom.y - below, 1.0e-9), "{part:?}");
            assert!(part.centroid.y < surface);
        }
        let lying = Rotation::from_rotation_z(FRAC_PI_2);
        for d in [0.01, 0.2, 0.5, 0.79] {
            let part = submerged(shape, Vector::ZERO, lying, d - r);
            let t = d - r;
            let segment = r * r * (PI - (t / r).acos()) + t * (r * r - t * t).sqrt();
            let exact = 2.0 * hh * segment + PI * d * d * (3.0 * r - d) / 3.0;
            assert!(
                close(part.volume, exact, 1.0e-9),
                "depth {d}: {} against {exact}",
                part.volume
            );
        }
    }

    /// A tilted capsule dipped by its lowest tip, against a fine sampling of
    /// the region around that tip: the buoyancy and its centre are right at
    /// first contact, not only on average.
    #[test]
    fn a_tilted_capsule_tip_matches_a_fine_sampling() {
        let (hh, r) = (0.6, 0.4);
        let shape = DynamicsShape::CapsuleY {
            half_height: hh,
            radius: r,
        };
        let rotation = Rotation::from_rotation_z(0.5);
        let lowest = -(hh * 0.5f64.cos() + r);
        for dip in [0.02, 0.1] {
            let surface = lowest + dip;
            let part = submerged(shape, Vector::ZERO, rotation, surface);
            let (volume, centroid) = sampled_region(hh, r, rotation, surface, lowest);
            assert!(
                close(part.volume, volume, 0.01),
                "dip {dip}: {} against {volume}",
                part.volume
            );
            assert!(
                (part.centroid - centroid).length() < 0.01 * r,
                "{part:?} against {centroid:?}"
            );
            assert!(part.centroid.y < surface);
        }
    }

    /// Across tilts and depths, within a fine sampling's own resolution.
    #[test]
    fn capsule_volumes_match_a_fine_sampling_at_any_tilt() {
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
                    (part.volume - reference).abs() < 0.003 * whole,
                    "tilt {angle}, surface {depth}: {} against {reference}",
                    part.volume
                );
            }
        }
    }

    /// Volume and centroid of the capsule below `surface`, by a fine grid
    /// over the slab between its lowest point and the surface.
    fn sampled_region(
        half_height: f64,
        radius: f64,
        rotation: Rotation,
        surface: f64,
        lowest: f64,
    ) -> (f64, Vector) {
        const N: i32 = 400;
        let reach = half_height + radius;
        let (dx, dy) = (
            2.0 * reach / f64::from(N),
            (surface - lowest) / f64::from(N),
        );
        let inverse = rotation.inverse();
        let (mut volume, mut moment) = (0.0, Vector::ZERO);
        for i in 0..N {
            for j in 0..N {
                for k in 0..N {
                    let p = Vector::new(
                        -reach + (f64::from(i) + 0.5) * dx,
                        lowest + (f64::from(j) + 0.5) * dy,
                        -reach + (f64::from(k) + 0.5) * dx,
                    );
                    let local = inverse * p;
                    let along = local.y.clamp(-half_height, half_height);
                    let d2 = local.x * local.x + (local.y - along).powi(2) + local.z * local.z;
                    if d2 <= radius * radius {
                        let cell = dx * dy * dx;
                        volume += cell;
                        moment += p * cell;
                    }
                }
            }
        }
        (volume, moment / volume)
    }

    /// The submerged volume by a fine grid of points.
    fn sampled(half_height: f64, radius: f64, rotation: Rotation, surface: f64) -> f64 {
        const N: i32 = 240;
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
