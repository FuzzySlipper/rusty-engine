//! Bone world transforms from local values, with the five inherit modes
//! (rig-c's `src/core/world.ts`).

/// π as the runtime spells it, and degrees to radians and back by it. The
/// double nearest `3.1415927` is neither `f64::consts::PI` nor
/// `f32::consts::PI`, and poses read it to the bit.
#[allow(clippy::approx_constant)]
pub(crate) const RUNTIME_PI: f64 = 3.1415927;
pub(crate) const RAD: f64 = RUNTIME_PI / 180.0;
pub(crate) const DEG: f64 = 180.0 / RUNTIME_PI;
const COLLAPSED_X_AXIS_SQ: f64 = 0.00001 * 0.00001;

/// How much of its parent's world transform a bone takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inherit {
    Normal,
    OnlyTranslation,
    NoRotationOrReflection,
    NoScale,
    NoScaleOrReflection,
}

/// A bone's local values: degrees counter-clockwise, y up, relative to the
/// parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Local {
    pub rotation: f64,
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub shear_x: f64,
    pub shear_y: f64,
    pub inherit: Inherit,
}

/// A bone's world transform: the matrix `[a b; c d]` and the origin, in rig
/// units, y up. An inactive bone (a skin-required bone its skin does not
/// name) is all zeros.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoneWorld {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub x: f64,
    pub y: f64,
}

pub(crate) type M2 = [f64; 4];

fn frame(rotation: f64, shear_x: f64, shear_y: f64, scale_x: f64, scale_y: f64) -> M2 {
    let x_angle = (rotation + shear_x) * RAD;
    let y_angle = (rotation + 90.0 + shear_y) * RAD;
    [
        x_angle.cos() * scale_x,
        y_angle.cos() * scale_y,
        x_angle.sin() * scale_x,
        y_angle.sin() * scale_y,
    ]
}

fn times(p: M2, q: M2) -> M2 {
    [
        p[0] * q[0] + p[1] * q[2],
        p[0] * q[1] + p[1] * q[3],
        p[2] * q[0] + p[3] * q[2],
        p[2] * q[1] + p[3] * q[3],
    ]
}

/// rig-c's `noScaleDirection`: the parent's matrix turned by `rotation`
/// degrees and its x column normalised.
pub(crate) fn no_scale_direction(p: M2, rotation: f64) -> (f64, f64) {
    let r = rotation * RAD;
    let (cos, sin) = (r.cos(), r.sin());
    let ux = p[0] * cos + p[1] * sin;
    let uy = p[2] * cos + p[3] * sin;
    let inverse = 1.0 / (ux * ux + uy * uy).sqrt();
    (ux * inverse, uy * inverse)
}

fn mode_matrix(parent: &BoneWorld, bone: &Local) -> M2 {
    let p: M2 = [parent.a, parent.b, parent.c, parent.d];
    let own = || {
        frame(
            bone.rotation,
            bone.shear_x,
            bone.shear_y,
            bone.scale_x,
            bone.scale_y,
        )
    };
    match bone.inherit {
        Inherit::Normal => times(p, own()),
        Inherit::OnlyTranslation => own(),
        Inherit::NoRotationOrReflection => {
            let x_length_sq = p[0] * p[0] + p[2] * p[2];
            let (conformal, r) = if x_length_sq > COLLAPSED_X_AXIS_SQ {
                let k = (p[0] * p[3] - p[1] * p[2]).abs() / x_length_sq;
                (
                    [p[0], -p[2] * k, p[2], p[0] * k],
                    bone.rotation - p[2].atan2(p[0]) * DEG,
                )
            } else {
                (
                    [0.0, -p[1], 0.0, p[3]],
                    bone.rotation - 90.0 + p[3].atan2(p[1]) * DEG,
                )
            };
            let x_angle = (r + bone.shear_x) * RAD;
            let y_angle = (r + bone.shear_y + 90.0) * RAD;
            times(
                conformal,
                [
                    x_angle.cos() * bone.scale_x,
                    y_angle.cos() * bone.scale_y,
                    x_angle.sin() * bone.scale_x,
                    y_angle.sin() * bone.scale_y,
                ],
            )
        }
        Inherit::NoScale | Inherit::NoScaleOrReflection => {
            let (ux, uy) = no_scale_direction(p, bone.rotation);
            let flip = if bone.inherit == Inherit::NoScale && p[0] * p[3] - p[1] * p[2] < 0.0 {
                -1.0
            } else {
                1.0
            };
            let turned: M2 = [ux, -uy * flip, uy, ux * flip];
            times(
                turned,
                frame(0.0, bone.shear_x, bone.shear_y, bone.scale_x, bone.scale_y),
            )
        }
    }
}

/// One bone's world transform from its local values, under its parent's.
pub(crate) fn pose_bone(bone: &Local, parent: Option<&BoneWorld>) -> BoneWorld {
    match parent {
        None => {
            let [a, b, c, d] = frame(
                bone.rotation,
                bone.shear_x,
                bone.shear_y,
                bone.scale_x,
                bone.scale_y,
            );
            BoneWorld {
                a,
                b,
                c,
                d,
                x: bone.x,
                y: bone.y,
            }
        }
        Some(p) => {
            let [a, b, c, d] = mode_matrix(p, bone);
            BoneWorld {
                a,
                b,
                c,
                d,
                x: p.a * bone.x + p.b * bone.y + p.x,
                y: p.c * bone.x + p.d * bone.y + p.y,
            }
        }
    }
}

/// Every bone's world transform, parents before children. `active` says
/// which bones the applied skin keeps.
pub(crate) fn world_transforms(
    locals: &[Local],
    parents: &[Option<usize>],
    active: &[bool],
    out: &mut Vec<BoneWorld>,
) {
    out.clear();
    for (i, bone) in locals.iter().enumerate() {
        let world = if active[i] {
            pose_bone(bone, parents[i].map(|p| &out[p]))
        } else {
            BoneWorld::default()
        };
        out.push(world);
    }
}
