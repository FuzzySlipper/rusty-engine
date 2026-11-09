//! Keys, curves and their evaluation, as rig-c's core reads them
//! (`src/core/animation.ts`, `deform.ts`, `draw_order.ts`, `events.ts`).
//!
//! A key's time and values are read through float32 (`f32r`); a Bézier
//! segment is drawn as the core's ten-piece polyline from the key's stated
//! (unrounded) time and values, each point read through float32.

/// JavaScript's `Math.fround`.
pub(crate) fn f32r(v: f64) -> f64 {
    v as f32 as f64
}

const BEZIER_SIXTH: f64 = 0.16666667;
const BEZIER_PIECES: usize = 10;
/// The inner points of a Bézier segment's polyline, as `t, v` pairs.
const INNER: usize = 2 * (BEZIER_PIECES - 1);
const DEFORM_CURVE_END: f64 = 0.99999999;

/// rig-c's `bezierPolyline`: the nine inner points of one channel's segment.
fn bezier_polyline(t0: f64, v0: f64, c: &[f64], t3: f64, v3: f64) -> [f64; INNER] {
    fn walk(p0: f64, p1: f64, p2: f64, p3: f64, out: &mut [f64; INNER], at: usize) {
        let second = (p0 - 2.0 * p1 + p2) * 0.03;
        let third = ((p1 - p2) * 3.0 - p0 + p3) * 0.006;
        let mut first = (p1 - p0) * 0.3 + second + third * BEZIER_SIXTH;
        let mut d2 = second * 2.0 + third;
        let mut value = p0;
        for i in 0..BEZIER_PIECES - 1 {
            value += first;
            first += d2;
            d2 += third;
            out[2 * i + at] = f32r(value);
        }
    }
    let mut out = [0.0; INNER];
    walk(t0, c[0], c[2], t3, &mut out, 0);
    walk(v0, c[1], c[3], v3, &mut out, 1);
    out
}

/// Where in a polyline `[x0, y0, inner…, x1, y1]` the time `t` falls.
fn polyline_at(
    start: (f64, f64),
    inner: &[f64; INNER],
    end: (f64, f64),
    t: f64,
) -> (usize, [f64; 4]) {
    let point = |i: usize| -> (f64, f64) {
        match i {
            0 => start,
            i if i == BEZIER_PIECES => end,
            i => (inner[2 * (i - 1)], inner[2 * (i - 1) + 1]),
        }
    };
    let mut i = 1;
    while i < BEZIER_PIECES && point(i).0 < t {
        i += 1;
    }
    let (x0, y0) = point(i - 1);
    let (x1, y1) = point(i);
    (i, [x0, y0, x1, y1])
}

#[derive(Clone)]
pub(crate) enum Curve {
    Linear,
    Stepped,
    /// One polyline per channel.
    Bezier(Vec<[f64; INNER]>),
}

/// The `curve` field of a key, as stated: absent, `"stepped"` or four
/// control numbers per channel.
pub(crate) enum StatedCurve<'a> {
    Linear,
    Stepped,
    Bezier(&'a [f64]),
}

/// One key of a number timeline: bone and slot-colour channels.
pub(crate) struct Key {
    pub time: f64,
    pub values: Vec<f64>,
    pub curve: Curve,
}

/// A number timeline's keys, the curves of each segment drawn once.
pub(crate) struct Keys {
    pub keys: Vec<Key>,
}

impl Keys {
    /// Builds the keys from `(time, values, curve)` as the document states
    /// them, each segment's polyline drawn from the stated numbers.
    pub fn new(stated: Vec<(f64, Vec<f64>, StatedCurve<'_>)>) -> Keys {
        let mut keys: Vec<Key> = Vec::with_capacity(stated.len());
        for (i, (time, values, curve)) in stated.iter().enumerate() {
            let curve = match curve {
                StatedCurve::Linear => Curve::Linear,
                StatedCurve::Stepped => Curve::Stepped,
                StatedCurve::Bezier(c) => match stated.get(i + 1) {
                    Some((next_time, next_values, _)) => Curve::Bezier(
                        (0..values.len())
                            .map(|ch| {
                                bezier_polyline(
                                    *time,
                                    values[ch],
                                    &c[ch * 4..ch * 4 + 4],
                                    *next_time,
                                    next_values[ch],
                                )
                            })
                            .collect(),
                    ),
                    None => Curve::Linear,
                },
            };
            keys.push(Key {
                time: f32r(*time),
                values: values.iter().map(|&v| f32r(v)).collect(),
                curve,
            });
        }
        Keys { keys }
    }

    /// The index of the last key at or before `t`.
    pub fn index_at(&self, t: f64) -> Option<usize> {
        key_index_at(self.keys.iter().map(|k| k.time), t)
    }

    /// rig-c's `channelAt`.
    pub fn channel(&self, index: usize, channel: usize, t: f64) -> f64 {
        let a = &self.keys[index];
        let Some(b) = self.keys.get(index + 1) else {
            return a.values[channel];
        };
        match &a.curve {
            Curve::Stepped => a.values[channel],
            Curve::Linear => {
                a.values[channel]
                    + ((t - a.time) / (b.time - a.time)) * (b.values[channel] - a.values[channel])
            }
            Curve::Bezier(polylines) => {
                let (_, [x0, y0, x1, y1]) = polyline_at(
                    (a.time, a.values[channel]),
                    &polylines[channel],
                    (b.time, b.values[channel]),
                    t,
                );
                y0 + ((t - x0) / (x1 - x0)) * (y1 - y0)
            }
        }
    }

    /// Every channel at `t`, or `None` before the first key.
    pub fn values_at(&self, t: f64, out: &mut [f64]) -> bool {
        let Some(index) = self.index_at(t) else {
            return false;
        };
        for (channel, value) in out
            .iter_mut()
            .enumerate()
            .take(self.keys[index].values.len())
        {
            *value = self.channel(index, channel, t);
        }
        true
    }
}

/// The index of the last time at or before `t` in increasing `times`.
pub(crate) fn key_index_at(times: impl Iterator<Item = f64>, t: f64) -> Option<usize> {
    let mut found = None;
    for (i, time) in times.enumerate() {
        if time <= t {
            found = Some(i);
        } else {
            break;
        }
    }
    found
}

/// One key of a deform timeline: a sparse run of offsets.
pub(crate) struct DeformKey {
    pub time: f64,
    pub stated_time: f64,
    pub curve: Curve,
    pub offset: usize,
    pub vertices: Vec<f64>,
}

/// A deform timeline's keys (`deform.ts`).
pub(crate) struct DeformKeys {
    pub keys: Vec<DeformKey>,
}

impl DeformKeys {
    pub fn new(mut keys: Vec<DeformKey>, stated_curves: Vec<StatedCurve<'_>>) -> DeformKeys {
        for i in 0..keys.len() {
            let curve = match &stated_curves[i] {
                StatedCurve::Linear => Curve::Linear,
                StatedCurve::Stepped => Curve::Stepped,
                StatedCurve::Bezier(c) => match keys.get(i + 1) {
                    Some(next) => Curve::Bezier(vec![bezier_polyline(
                        keys[i].stated_time,
                        0.0,
                        c,
                        next.stated_time,
                        DEFORM_CURVE_END,
                    )]),
                    None => Curve::Linear,
                },
            };
            keys[i].curve = curve;
        }
        DeformKeys { keys }
    }

    /// rig-c's `heldArray`: a key's whole deform array.
    fn held(&self, setup: &[f64], weighted: bool, key: &DeformKey, out: &mut Vec<f64>) {
        out.clear();
        if weighted {
            out.resize(setup.len(), 0.0);
        } else {
            out.extend_from_slice(setup);
        }
        for (i, &v) in key.vertices.iter().enumerate() {
            let at = key.offset + i;
            out[at] = if weighted {
                f32r(v)
            } else {
                f32r(out[at] + f32r(v))
            };
        }
    }

    /// rig-c's `deformPercent`.
    fn percent(a: &DeformKey, b: &DeformKey, t: f64) -> f64 {
        match &a.curve {
            Curve::Stepped => 0.0,
            Curve::Linear => (t - a.time) / (b.time - a.time),
            Curve::Bezier(polylines) => {
                let (i, [x0, y0, x1, y1]) =
                    polyline_at((a.time, 0.0), &polylines[0], (b.time, 1.0), t);
                if i == 1 {
                    (y1 * (t - x0)) / (x1 - x0)
                } else if i == BEZIER_PIECES {
                    y0 + ((1.0 - y0) * (t - x0)) / (x1 - x0)
                } else {
                    y0 + ((t - x0) / (x1 - x0)) * (y1 - y0)
                }
            }
        }
    }

    /// rig-c's `deformAt`: the deform array at `t`, or `false` before the
    /// first key. `setup` is the attachment's setup array (`setupArray`).
    pub fn at(
        &self,
        setup: &[f64],
        weighted: bool,
        t: f64,
        out: &mut Vec<f64>,
        scratch: &mut Vec<f64>,
    ) -> bool {
        let Some(i) = key_index_at(self.keys.iter().map(|k| k.time), t) else {
            return false;
        };
        let a = &self.keys[i];
        self.held(setup, weighted, a, out);
        let Some(b) = self.keys.get(i + 1) else {
            return true;
        };
        if matches!(a.curve, Curve::Stepped) {
            return true;
        }
        self.held(setup, weighted, b, scratch);
        let p = Self::percent(a, b, t);
        for (v, next) in out.iter_mut().zip(scratch.iter()) {
            *v += (next - *v) * p;
        }
        true
    }
}

/// The ways a sequence key steps through its series.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMode {
    Hold,
    Once,
    Loop,
    Pingpong,
    OnceReverse,
    LoopReverse,
    PingpongReverse,
}

pub(crate) struct SequenceKey {
    pub time: f64,
    pub mode: SequenceMode,
    pub index: usize,
    pub delay: f64,
}

const SEQUENCE_EPSILON: f64 = 0.00001;

/// rig-c's `sequenceFrameAt`.
pub(crate) fn sequence_frame_at(keys: &[SequenceKey], count: usize, t: f64) -> Option<usize> {
    let i = key_index_at(keys.iter().map(|k| k.time), t)?;
    let k = &keys[i];
    let mut n = k.index as i64;
    if k.mode != SequenceMode::Hold && k.delay > 0.0 {
        n += ((t - k.time) / k.delay + SEQUENCE_EPSILON).floor() as i64;
    }
    let count = count as i64;
    use SequenceMode::*;
    let reverse = matches!(k.mode, OnceReverse | LoopReverse | PingpongReverse);
    let f = match k.mode {
        Hold | Once | OnceReverse => n.min(count - 1),
        Loop | LoopReverse => n % count,
        Pingpong | PingpongReverse => {
            let period = 2 * (count - 1);
            let mut f = if period == 0 { 0 } else { n % period };
            if f >= count {
                f = period - f;
            }
            f
        }
    };
    Some((if reverse { count - 1 - f } else { f }) as usize)
}

/// One draw-order key: the slots it moves, as `(slot, offset)` from setup.
pub(crate) struct DrawOrderKey {
    pub time: f64,
    pub moves: Vec<(usize, i64)>,
}

/// rig-c's `orderOf`: the slot permutation one key states.
pub(crate) fn order_of(count: usize, moves: &[(usize, i64)], out: &mut Vec<usize>) {
    out.clear();
    out.resize(count, usize::MAX);
    for &(slot, offset) in moves {
        out[(slot as i64 + offset) as usize] = slot;
    }
    let mut rest = (0..count).filter(|i| !moves.iter().any(|m| m.0 == *i));
    for entry in out.iter_mut() {
        if *entry == usize::MAX {
            *entry = rest.next().unwrap_or(0);
        }
    }
}

/// One event key, its payload resolved against the declaration.
pub(crate) struct EventKey {
    pub time: f64,
    pub event: usize,
    pub int: i64,
    pub float: f64,
    pub string: String,
    pub volume: f64,
    pub balance: f64,
}
