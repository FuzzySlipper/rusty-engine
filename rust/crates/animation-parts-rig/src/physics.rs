//! Physics constraints (rig-c's `src/core/constraints_physics.ts`): a bone
//! springs back toward where its animation puts it, lagging behind its own
//! motion and the rig's, and answers the wind and gravity it is given.
//!
//! Each constraint keeps its state from step to step on its player. A step
//! moves the player's clock by its `dt`, and every constraint integrates
//! the time since it last read the clock in fixed steps of `1 / fps`,
//! carrying what remains to the next step, so a pose depends only on the
//! steps taken. The step is applied in the constraints' document order, on
//! the bone's world transform, after which the bone's local values are read
//! back and the bones below it posed again, as a world-space transform
//! leaves them. Every constant and every order of operations here is
//! rig-c's.

use crate::constraints::IkScaleY;
use crate::world::{BoneWorld, RUNTIME_PI};

/// Indices of the seven values a physics constraint's timelines key, in
/// [`crate::constraints::ConstraintValues::values`]. The pose holds the
/// inverse of the mass.
pub(crate) const INERTIA: usize = 0;
pub(crate) const STRENGTH: usize = 1;
pub(crate) const DAMPING: usize = 2;
pub(crate) const MASS_INVERSE: usize = 3;
pub(crate) const WIND: usize = 4;
pub(crate) const GRAVITY: usize = 5;
pub(crate) const MIX: usize = 6;

/// The timeline names, by value index.
pub(crate) const VALUE_NAMES: [&str; 7] = [
    "inertia", "strength", "damping", "mass", "wind", "gravity", "mix",
];

const PI2: f64 = RUNTIME_PI * 2.0;
const INV_PI2: f64 = 1.0 / PI2;

pub(crate) struct PhysicsData {
    pub bone: usize,
    pub x: f64,
    pub y: f64,
    pub rotate: f64,
    pub scale_x: f64,
    pub shear_x: f64,
    pub limit: f64,
    /// `1 / fps`, the integrator's fixed step in seconds.
    pub step: f64,
    /// The document's `referenceScale`, which wind and gravity act over.
    pub reference_scale: f64,
    /// Which values the timeline naming no constraint (`*`) writes.
    pub global: [bool; 7],
    pub scale_y: IkScaleY,
}

/// One constraint's state between two steps.
#[derive(Clone)]
pub(crate) struct State {
    /// The next step takes the bone where it is and integrates nothing.
    pending: bool,
    last_time: f64,
    remaining: f64,
    ux: f64,
    uy: f64,
    cx: f64,
    cy: f64,
    tx: f64,
    ty: f64,
    x_offset: f64,
    x_lag: f64,
    x_velocity: f64,
    y_offset: f64,
    y_lag: f64,
    y_velocity: f64,
    rotate_offset: f64,
    rotate_lag: f64,
    rotate_velocity: f64,
    scale_offset: f64,
    scale_lag: f64,
    scale_velocity: f64,
}

impl Default for State {
    fn default() -> State {
        State {
            pending: true,
            last_time: 0.0,
            remaining: 0.0,
            ux: 0.0,
            uy: 0.0,
            cx: 0.0,
            cy: 0.0,
            tx: 0.0,
            ty: 0.0,
            x_offset: 0.0,
            x_lag: 0.0,
            x_velocity: 0.0,
            y_offset: 0.0,
            y_lag: 0.0,
            y_velocity: 0.0,
            rotate_offset: 0.0,
            rotate_lag: 0.0,
            rotate_velocity: 0.0,
            scale_offset: 0.0,
            scale_lag: 0.0,
            scale_velocity: 0.0,
        }
    }
}

impl State {
    /// Offsets, velocities and lags cleared, nothing remaining, the clock
    /// read at `time`, pending.
    pub fn reset(&mut self, time: f64) {
        *self = State {
            last_time: time,
            ..State::default()
        };
    }
}

/// What every physics constraint of one player steps with.
#[derive(Clone)]
pub(crate) struct Physics {
    /// Each constraint's state, by constraint index.
    pub states: Vec<State>,
    /// The sum of every step's `dt`.
    pub time: f64,
    /// The next pass resets every constraint it steps: a player's first.
    pub resetting: bool,
    /// The directions the constraints' own wind and gravity act along, in
    /// rig-c's convention (x right, y down).
    pub wind: [f64; 2],
    pub gravity: [f64; 2],
}

impl Physics {
    pub fn new(constraints: usize) -> Physics {
        Physics {
            states: vec![State::default(); constraints],
            time: 0.0,
            resetting: true,
            wind: [1.0, 0.0],
            gravity: [0.0, 1.0],
        }
    }

    /// rig-c's `translatePhysics`: every constraint's next step sees its
    /// bone moved by `(x, y)`, rig units, y up.
    pub fn translate(&mut self, x: f64, y: f64) {
        for s in &mut self.states {
            s.ux -= x;
            s.uy -= y;
            s.cx -= x;
            s.cy -= y;
        }
    }
}

/// `u` held to `±q`, as rig-c compares: NaN passes, and a negative `q`
/// gives `q` for any `u` above it.
fn held(u: f64, q: f64) -> f64 {
    if u > q {
        q
    } else if u < -q {
        -q
    } else {
        u
    }
}

/// rig-c's `stepPhysics`: one constraint updated on its bone's world
/// transform `w`, `length` the bone's length, with the values `v` its
/// timelines posed. False when its mix is 0: it then does nothing at all,
/// its clock included, and the bone is not posed again.
pub(crate) fn step(
    p: &PhysicsData,
    v: &[f64; 7],
    s: &mut State,
    w: &mut BoneWorld,
    length: f64,
    run: &Physics,
) -> bool {
    let mix = v[MIX];
    if mix == 0.0 {
        return false;
    }
    let (x, y) = (p.x > 0.0, p.y > 0.0);
    let rotate_or_shear_x = p.rotate > 0.0 || p.shear_x > 0.0;
    let scale_x = p.scale_x > 0.0;
    let l = length;
    if run.resetting {
        s.reset(run.time);
    }
    let delta = (run.time - s.last_time).max(0.0);
    // The previous step's interpolation weight, before this one's time is
    // added: the rotation and the scale-only inertia read the pose as shown.
    let shown = (1.0 - s.remaining / p.step).max(0.0);
    s.remaining += delta;
    s.last_time = run.time;
    let (bx, by) = (w.x, w.y);
    let [wind_x, wind_y] = run.wind;
    let [gravity_x, gravity_y] = run.gravity;
    let mut z = 0.0;
    if s.pending {
        s.pending = false;
        s.ux = bx;
        s.uy = by;
    } else {
        let mut a = s.remaining;
        let i = v[INERTIA];
        let t = p.step;
        let f = p.reference_scale;
        let mut d = -1.0;
        let qx = p.limit * delta;
        let qy = qx;
        if x || y {
            if x {
                let u = (s.ux - bx) * i;
                s.x_offset += held(u, qx);
                s.ux = bx;
            }
            if y {
                let u = (s.uy - by) * i;
                s.y_offset += held(u, qy);
                s.uy = by;
            }
            if a >= t {
                d = v[DAMPING].powf(60.0 * t);
                let m = v[MASS_INVERSE] * t;
                let e = v[STRENGTH];
                let wind = v[WIND] * f;
                let g = v[GRAVITY] * f;
                let ax = wind * wind_x + g * gravity_x;
                let ay = wind * wind_y + g * gravity_y;
                let (xs, ys) = (s.x_offset, s.y_offset);
                loop {
                    if x {
                        s.x_velocity += (ax - s.x_offset * e) * m;
                        s.x_offset += s.x_velocity * t;
                        s.x_velocity *= d;
                    }
                    if y {
                        s.y_velocity -= (ay + s.y_offset * e) * m;
                        s.y_offset += s.y_velocity * t;
                        s.y_velocity *= d;
                    }
                    a -= t;
                    if a < t {
                        break;
                    }
                }
                s.x_lag = s.x_offset - xs;
                s.y_lag = s.y_offset - ys;
            }
            z = (1.0 - a / t).max(0.0);
            if x {
                w.x += (s.x_offset - s.x_lag * z) * mix * p.x;
            }
            if y {
                w.y += (s.y_offset - s.y_lag * z) * mix * p.y;
            }
        }
        if rotate_or_shear_x || scale_x {
            let ca = w.c.atan2(w.a);
            let mut c;
            let mut sn;
            let mut mr = 0.0;
            let dx = held(s.cx - w.x, qx);
            let dy = held(s.cy - w.y, qy);
            if rotate_or_shear_x {
                mr = (p.rotate + p.shear_x) * mix;
                let lagged = s.rotate_lag * shown;
                let mut rr = (dy + s.ty).atan2(dx + s.tx) - ca - (s.rotate_offset - lagged) * mr;
                s.rotate_offset += (rr - (rr * INV_PI2 - 0.5).ceil() * PI2) * i;
                rr = (s.rotate_offset - lagged) * mr + ca;
                c = rr.cos();
                sn = rr.sin();
                if scale_x {
                    let r = l * (w.a * w.a + w.c * w.c).sqrt();
                    if r > 0.0 {
                        s.scale_offset += ((dx * c + dy * sn) * i) / r;
                    }
                }
            } else {
                c = ca.cos();
                sn = ca.sin();
                let r = l * (w.a * w.a + w.c * w.c).sqrt() - s.scale_lag * shown;
                if r > 0.0 {
                    s.scale_offset += ((dx * c + dy * sn) * i) / r;
                }
            }
            a = s.remaining;
            if a >= t {
                if d == -1.0 {
                    d = v[DAMPING].powf(60.0 * t);
                }
                let m = v[MASS_INVERSE] * t;
                let e = v[STRENGTH];
                let ax = v[WIND] * wind_x + v[GRAVITY] * gravity_x;
                let ay = v[WIND] * wind_y + v[GRAVITY] * gravity_y;
                let h = l / f;
                let (rs, ss) = (s.rotate_offset, s.scale_offset);
                loop {
                    a -= t;
                    if scale_x {
                        s.scale_velocity += (ax * c - ay * sn - s.scale_offset * e) * m;
                        s.scale_offset += s.scale_velocity * t;
                        s.scale_velocity *= d;
                    }
                    if rotate_or_shear_x {
                        s.rotate_velocity -= ((ax * sn + ay * c) * h + s.rotate_offset * e) * m;
                        s.rotate_offset += s.rotate_velocity * t;
                        s.rotate_velocity *= d;
                        if a < t {
                            break;
                        }
                        let rr = s.rotate_offset * mr + ca;
                        c = rr.cos();
                        sn = rr.sin();
                    } else if a < t {
                        break;
                    }
                }
                s.rotate_lag = s.rotate_offset - rs;
                s.scale_lag = s.scale_offset - ss;
            }
            z = (1.0 - a / t).max(0.0);
        }
        s.remaining = a;
    }
    s.cx = w.x;
    s.cy = w.y;
    if rotate_or_shear_x {
        let o = (s.rotate_offset - s.rotate_lag * z) * mix;
        if p.shear_x > 0.0 {
            let mut r = 0.0;
            if p.rotate > 0.0 {
                // The rotation's share turns the y column here, and the x
                // column with the shear below.
                r = o * p.rotate;
                let (sn, c) = r.sin_cos();
                let b = w.b;
                w.b = c * b - sn * w.d;
                w.d = sn * b + c * w.d;
            }
            r += o * p.shear_x;
            let (sn, c) = r.sin_cos();
            let a = w.a;
            w.a = c * a - sn * w.c;
            w.c = sn * a + c * w.c;
        } else {
            let r = o * p.rotate;
            let (sn, c) = r.sin_cos();
            let a = w.a;
            w.a = c * a - sn * w.c;
            w.c = sn * a + c * w.c;
            let b = w.b;
            w.b = c * b - sn * w.d;
            w.d = sn * b + c * w.d;
        }
    }
    if scale_x {
        let k = 1.0 + (s.scale_offset - s.scale_lag * z) * mix * p.scale_x;
        w.a *= k;
        w.c *= k;
        match p.scale_y {
            IkScaleY::None => {}
            IkScaleY::Uniform => {
                w.b *= k;
                w.d *= k;
            }
            IkScaleY::Volume => {
                let magnitude = k.abs();
                let transverse = if magnitude >= 0.7 {
                    1.0 / magnitude
                } else {
                    4.0 - 3.67347 * magnitude
                };
                w.b *= transverse;
                w.d *= transverse;
            }
        }
    }
    s.tx = l * w.a;
    s.ty = l * w.c;
    true
}
