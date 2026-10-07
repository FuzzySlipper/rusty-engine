//! Easing curves: maps normalized segment time `t` in `[0, 1]` to progress.
//! Every curve returns exactly 0 at 0 and 1 at 1; back, elastic and spring
//! overshoot in between.

use std::f32::consts::{FRAC_PI_2, PI};

const BACK_OVERSHOOT: f32 = 1.701_58;
const BACK_IN_OUT_OVERSHOOT: f32 = BACK_OVERSHOOT * 1.525;
const ELASTIC_PERIOD: f32 = 0.3;
const BEZIER_NEWTON_STEPS: usize = 8;
const BEZIER_BISECTION_STEPS: usize = 24;
const BEZIER_EPSILON: f32 = 1.0e-6;
/// A spring is settled once its envelope falls below this fraction of the
/// distance it travels.
const SPRING_SETTLE_RESIDUAL: f32 = 1.0e-3;

/// The shape families with in, out and in-out forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseFamily {
    Quad,
    Cubic,
    Quart,
    Quint,
    Sine,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseMode {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    Ease(EaseFamily, EaseMode),
    /// CSS `cubic-bezier(x1, y1, x2, y2)`; `x1` and `x2` lie in `[0, 1]`.
    CubicBezier([f32; 4]),
    /// `steps(n, jump-end)`: holds each of `n` levels, reaching 1 at the end.
    Steps(u32),
    /// A unit-mass spring released from 0 toward 1, its whole settle time
    /// ([`spring_settle_seconds`]) stretched over the segment.
    Spring {
        stiffness: f32,
        damping: f32,
    },
}

impl Easing {
    pub fn sample(self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        match self {
            Self::Linear => t,
            Self::Ease(family, EaseMode::In) => ease_in(family, t),
            Self::Ease(family, EaseMode::Out) => 1.0 - ease_in(family, 1.0 - t),
            Self::Ease(EaseFamily::Back, EaseMode::InOut) => back_in_out(t),
            Self::Ease(family, EaseMode::InOut) => {
                if t < 0.5 {
                    0.5 * ease_in(family, 2.0 * t)
                } else {
                    1.0 - 0.5 * ease_in(family, 2.0 - 2.0 * t)
                }
            }
            Self::CubicBezier(points) => cubic_bezier(points, t),
            Self::Steps(count) => {
                let count = count.max(1) as f32;
                (t * count).floor() / count
            }
            Self::Spring { stiffness, damping } => spring(
                stiffness,
                damping,
                t * spring_settle_seconds(stiffness, damping),
            ),
        }
    }
}

fn ease_in(family: EaseFamily, t: f32) -> f32 {
    match family {
        EaseFamily::Quad => t * t,
        EaseFamily::Cubic => t * t * t,
        EaseFamily::Quart => t * t * t * t,
        EaseFamily::Quint => t * t * t * t * t,
        EaseFamily::Sine => 1.0 - (t * FRAC_PI_2).cos(),
        EaseFamily::Expo => 2.0_f32.powf(10.0 * t - 10.0),
        EaseFamily::Circ => 1.0 - (1.0 - t * t).max(0.0).sqrt(),
        EaseFamily::Back => t * t * ((BACK_OVERSHOOT + 1.0) * t - BACK_OVERSHOOT),
        EaseFamily::Elastic => {
            let phase = (t - 1.0 - ELASTIC_PERIOD / 4.0) * 2.0 * PI / ELASTIC_PERIOD;
            -(2.0_f32.powf(10.0 * t - 10.0)) * phase.sin()
        }
        EaseFamily::Bounce => 1.0 - bounce_out(1.0 - t),
    }
}

fn back_in_out(t: f32) -> f32 {
    let c = BACK_IN_OUT_OVERSHOOT;
    if t < 0.5 {
        let u = 2.0 * t;
        0.5 * u * u * ((c + 1.0) * u - c)
    } else {
        let u = 2.0 * t - 2.0;
        0.5 * (u * u * ((c + 1.0) * u + c) + 2.0)
    }
}

fn bounce_out(t: f32) -> f32 {
    const N: f32 = 7.5625;
    const D: f32 = 2.75;
    if t < 1.0 / D {
        N * t * t
    } else if t < 2.0 / D {
        let u = t - 1.5 / D;
        N * u * u + 0.75
    } else if t < 2.5 / D {
        let u = t - 2.25 / D;
        N * u * u + 0.9375
    } else {
        let u = t - 2.625 / D;
        N * u * u + 0.984_375
    }
}

fn cubic_bezier([x1, y1, x2, y2]: [f32; 4], t: f32) -> f32 {
    let x1 = x1.clamp(0.0, 1.0);
    let x2 = x2.clamp(0.0, 1.0);
    let coordinate = |a: f32, b: f32, s: f32| {
        let inverse = 1.0 - s;
        3.0 * inverse * inverse * s * a + 3.0 * inverse * s * s * b + s * s * s
    };
    let slope = |a: f32, b: f32, s: f32| {
        let inverse = 1.0 - s;
        3.0 * inverse * inverse * a + 6.0 * inverse * s * (b - a) + 3.0 * s * s * (1.0 - b)
    };
    // x(s) is monotonic for x1, x2 in [0, 1]: Newton first, bisection when
    // the slope is flat.
    let mut s = t;
    for _ in 0..BEZIER_NEWTON_STEPS {
        let error = coordinate(x1, x2, s) - t;
        if error.abs() < BEZIER_EPSILON {
            return coordinate(y1, y2, s);
        }
        let derivative = slope(x1, x2, s);
        if derivative.abs() < BEZIER_EPSILON {
            break;
        }
        s = (s - error / derivative).clamp(0.0, 1.0);
    }
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    s = t;
    for _ in 0..BEZIER_BISECTION_STEPS {
        let x = coordinate(x1, x2, s);
        if (x - t).abs() < BEZIER_EPSILON {
            break;
        }
        if x < t {
            low = s;
        } else {
            high = s;
        }
        s = 0.5 * (low + high);
    }
    coordinate(y1, y2, s)
}

/// Seconds a unit-mass spring with this stiffness and damping takes to come
/// within 0.1% of its target. Non-positive values fall back to a stiff,
/// critically damped spring.
pub fn spring_settle_seconds(stiffness: f32, damping: f32) -> f32 {
    const BISECTION_STEPS: usize = 40;
    let (omega, zeta) = spring_parameters(stiffness, damping);
    // Up to critical damping |1 - x(t)| <= e^(-ζωt)(1 + ζωt), which is exact
    // at ζ = 1; an overdamped spring's own distance decreases monotonically.
    let remaining = |t: f32| {
        if zeta <= 1.0 {
            let decay = zeta * omega * t;
            (-decay).exp() * (1.0 + decay)
        } else {
            1.0 - spring(stiffness, damping, t)
        }
    };
    let mut high = 1.0 / omega;
    while remaining(high) > SPRING_SETTLE_RESIDUAL {
        high *= 2.0;
    }
    let mut low = 0.0;
    for _ in 0..BISECTION_STEPS {
        let middle = 0.5 * (low + high);
        if remaining(middle) > SPRING_SETTLE_RESIDUAL {
            low = middle;
        } else {
            high = middle;
        }
    }
    high
}

fn spring_parameters(stiffness: f32, damping: f32) -> (f32, f32) {
    const FALLBACK_STIFFNESS: f32 = 200.0;
    let stiffness = if stiffness > 0.0 {
        stiffness
    } else {
        FALLBACK_STIFFNESS
    };
    let omega = stiffness.sqrt();
    let damping = if damping > 0.0 { damping } else { 2.0 * omega };
    (omega, damping / (2.0 * omega))
}

/// Position at `seconds` of a unit-mass spring released at rest from 0
/// toward 1.
fn spring(stiffness: f32, damping: f32, seconds: f32) -> f32 {
    let (omega, zeta) = spring_parameters(stiffness, damping);
    let t = seconds;
    let offset = if zeta < 1.0 {
        let damped = omega * (1.0 - zeta * zeta).sqrt();
        (-zeta * omega * t).exp()
            * ((damped * t).cos() + zeta * omega / damped * (damped * t).sin())
    } else if (zeta - 1.0).abs() < 1.0e-4 {
        (-omega * t).exp() * (1.0 + omega * t)
    } else {
        let root = (zeta * zeta - 1.0).sqrt();
        let fast = -omega * (zeta + root);
        let slow = -omega * (zeta - root);
        (fast * (slow * t).exp() - slow * (fast * t).exp()) / (fast - slow)
    };
    1.0 - offset
}
