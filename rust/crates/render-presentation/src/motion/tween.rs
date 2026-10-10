//! Presentation tweens: a flat timeline of eased segments that samples to an
//! offset layered over an object's published transform and tint.
//!
//! Each channel has a base track and additive segments. The base track
//! shows the latest base segment that has started (before the first one
//! starts, that segment's start), holding its end value afterwards.
//! An additive segment contributes nothing before it starts and holds its
//! end value afterwards. Translations add, rotations, scales and tints
//! multiply.

use render_model::Transform;

use super::easing::Easing;

const TAU: f32 = std::f32::consts::TAU;
const IDENTITY_ROTATION: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const ONE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TweenChannel {
    /// Parent-space translation added to the published translation (`xyz`).
    Translation,
    /// Rotation quaternion (`xyzw`) applied in the object's local frame.
    Rotation,
    /// Local scale multiplying the published scale (`xyz`).
    Scale,
    /// RGBA multiplying the appearance's own colour.
    Tint,
}

impl TweenChannel {
    const ALL: [Self; 4] = [Self::Translation, Self::Rotation, Self::Scale, Self::Tint];

    fn identity(self) -> [f32; 4] {
        match self {
            Self::Translation => [0.0; 4],
            Self::Rotation => IDENTITY_ROTATION,
            Self::Scale | Self::Tint => ONE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenLayer {
    Base,
    Additive,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TweenShape {
    /// From `from` to `to` by the easing. On translation, `arc` is added at
    /// `4p(1 - p)` of linear progress `p`: a parabolic hop peaking mid-way.
    Tween { arc: [f32; 3] },
    /// Swings from `from` toward `to` and past it `frequency` times, the
    /// swing shrinking to nothing by the easing.
    Punch { frequency: f32 },
    /// Smooth seeded noise around `from`, each component reaching up to
    /// `to - from`, at `frequency` changes per second, fading out by the
    /// easing.
    Shake { frequency: f32, seed: u32 },
    /// A Catmull-Rom span from `from` to `to`, shaped by the points before
    /// and after it. Rotation segments interpolate as a tween.
    Spline { before: [f32; 4], after: [f32; 4] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TweenSegment {
    pub start_seconds: f32,
    pub duration_seconds: f32,
    pub channel: TweenChannel,
    pub layer: TweenLayer,
    pub easing: Easing,
    pub shape: TweenShape,
    pub from: [f32; 4],
    pub to: [f32; 4],
}

impl TweenSegment {
    fn end_seconds(&self) -> f32 {
        self.start_seconds + self.duration_seconds
    }

    fn progress(&self, local_seconds: f32) -> f32 {
        if self.duration_seconds <= 0.0 {
            return if local_seconds >= self.start_seconds {
                1.0
            } else {
                0.0
            };
        }
        ((local_seconds - self.start_seconds) / self.duration_seconds).clamp(0.0, 1.0)
    }

    fn sample(&self, local_seconds: f32) -> [f32; 4] {
        let p = self.progress(local_seconds);
        let eased = self.easing.sample(p);
        let rotation = self.channel == TweenChannel::Rotation;
        match self.shape {
            TweenShape::Tween { arc } => {
                if rotation {
                    return rotate_toward(self.from, self.to, eased);
                }
                let mut value = lerp(self.from, self.to, eased);
                if self.channel == TweenChannel::Translation {
                    let height = 4.0 * p * (1.0 - p);
                    for axis in 0..3 {
                        value[axis] += arc[axis] * height;
                    }
                }
                value
            }
            TweenShape::Punch { frequency } => {
                let swing = (TAU * frequency * p).sin() * (1.0 - eased);
                if rotation {
                    rotate_toward(self.from, self.to, swing)
                } else {
                    lerp(self.from, self.to, swing)
                }
            }
            TweenShape::Shake { frequency, seed } => {
                let fade = 1.0 - eased;
                let at = (local_seconds - self.start_seconds).max(0.0) * frequency;
                if rotation {
                    return rotate_toward(self.from, self.to, noise(seed, 0, at) * fade);
                }
                std::array::from_fn(|axis| {
                    self.from[axis]
                        + (self.to[axis] - self.from[axis]) * noise(seed, axis as u32, at) * fade
                })
            }
            TweenShape::Spline { before, after } => {
                if rotation {
                    return rotate_toward(self.from, self.to, eased);
                }
                catmull_rom(before, self.from, self.to, after, eased)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TweenMarker {
    pub marker_id: u64,
    /// Seconds into each iteration.
    pub time_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenRepeat {
    /// Plays this many iterations in all; zero plays one.
    Count(u32),
    Forever,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenDefinitionError {
    /// A start, duration or marker time is negative or not finite.
    InvalidTime { segment: usize },
    /// A value is not finite, or a rotation has no length.
    InvalidValue { segment: usize },
    /// A marker time is negative or not finite.
    InvalidMarker { marker: usize },
}

/// One validated tween timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct TweenDefinition {
    segments: Vec<TweenSegment>,
    markers: Vec<TweenMarker>,
    repeat: TweenRepeat,
    yoyo: bool,
    iteration_seconds: f32,
}

impl TweenDefinition {
    /// Rotations are normalized. Markers after the iteration end never fire.
    pub fn new(
        mut segments: Vec<TweenSegment>,
        markers: Vec<TweenMarker>,
        repeat: TweenRepeat,
        yoyo: bool,
    ) -> Result<Self, TweenDefinitionError> {
        for (index, segment) in segments.iter_mut().enumerate() {
            let time_valid = |value: f32| value.is_finite() && value >= 0.0;
            if !time_valid(segment.start_seconds) || !time_valid(segment.duration_seconds) {
                return Err(TweenDefinitionError::InvalidTime { segment: index });
            }
            let mut values = vec![segment.from, segment.to];
            match segment.shape {
                TweenShape::Tween { arc } => values.push([arc[0], arc[1], arc[2], 0.0]),
                TweenShape::Punch { frequency } | TweenShape::Shake { frequency, .. } => {
                    values.push([frequency; 4])
                }
                TweenShape::Spline { before, after } => values.extend([before, after]),
            }
            if let Easing::CubicBezier(points) = segment.easing {
                values.push(points);
            }
            if let Easing::Spring { stiffness, damping } = segment.easing {
                values.push([stiffness, damping, 0.0, 0.0]);
            }
            if values.iter().flatten().any(|value| !value.is_finite()) {
                return Err(TweenDefinitionError::InvalidValue { segment: index });
            }
            if segment.channel == TweenChannel::Rotation {
                segment.from = normalize(segment.from)
                    .ok_or(TweenDefinitionError::InvalidValue { segment: index })?;
                segment.to = normalize(segment.to)
                    .ok_or(TweenDefinitionError::InvalidValue { segment: index })?;
            }
        }
        if let Some(index) = markers
            .iter()
            .position(|marker| !(marker.time_seconds.is_finite() && marker.time_seconds >= 0.0))
        {
            return Err(TweenDefinitionError::InvalidMarker { marker: index });
        }
        // Base tracks are read in start order; equal starts keep request order.
        segments.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
        let iteration_seconds = segments
            .iter()
            .map(TweenSegment::end_seconds)
            .fold(0.0, f32::max);
        Ok(Self {
            segments,
            markers,
            repeat,
            yoyo,
            iteration_seconds,
        })
    }

    pub fn iteration_seconds(&self) -> f32 {
        self.iteration_seconds
    }

    fn iterations(&self) -> Option<u32> {
        match self.repeat {
            // A timeline with no length ends as soon as it starts.
            _ if self.iteration_seconds <= 0.0 => Some(1),
            TweenRepeat::Count(count) => Some(count.max(1)),
            TweenRepeat::Forever => None,
        }
    }

    /// Seconds from start to end; `None` plays forever.
    pub fn total_seconds(&self) -> Option<f64> {
        self.iterations()
            .map(|count| f64::from(self.iteration_seconds) * f64::from(count))
    }

    pub fn animates(&self, channel: TweenChannel) -> bool {
        self.segments
            .iter()
            .any(|segment| segment.channel == channel)
    }

    /// Starts each channel's base track from `carry`: the first base tween
    /// or spline segment of every channel takes it as its `from`.
    pub fn start_from(&mut self, carry: &TweenOffset) {
        for channel in TweenChannel::ALL {
            if let Some(segment) = self.segments.iter_mut().find(|segment| {
                segment.channel == channel
                    && segment.layer == TweenLayer::Base
                    && matches!(
                        segment.shape,
                        TweenShape::Tween { .. } | TweenShape::Spline { .. }
                    )
            }) {
                segment.from = carry.channel(channel);
            }
        }
    }

    /// The iteration and local time shown `elapsed` seconds after the start.
    fn position(&self, elapsed: f64) -> (u32, f32) {
        let length = f64::from(self.iteration_seconds);
        if length <= 0.0 {
            return (0, 0.0);
        }
        let (iteration, local) = match self.iterations() {
            Some(count) if elapsed >= length * f64::from(count) => (count - 1, length),
            _ => {
                let iteration = (elapsed.max(0.0) / length).floor();
                (iteration as u32, elapsed - iteration * length)
            }
        };
        let local = local as f32;
        if self.yoyo && iteration % 2 == 1 {
            (iteration, self.iteration_seconds - local)
        } else {
            (iteration, local)
        }
    }

    pub fn sample(&self, elapsed: f64) -> TweenOffset {
        let (_, local) = self.position(elapsed);
        let mut offset = TweenOffset::IDENTITY;
        for channel in TweenChannel::ALL {
            let base = self
                .segments
                .iter()
                .filter(|segment| segment.channel == channel && segment.layer == TweenLayer::Base)
                .take_while(|segment| segment.start_seconds <= local)
                .last()
                .or_else(|| {
                    self.segments.iter().find(|segment| {
                        segment.channel == channel && segment.layer == TweenLayer::Base
                    })
                })
                .map_or(channel.identity(), |segment| segment.sample(local));
            offset.set_channel(channel, base);
            for segment in self.segments.iter().filter(|segment| {
                segment.channel == channel
                    && segment.layer == TweenLayer::Additive
                    && segment.start_seconds <= local
            }) {
                let mut layer = TweenOffset::IDENTITY;
                layer.set_channel(channel, segment.sample(local));
                offset = offset.then(&layer);
            }
        }
        offset
    }

    /// Markers crossed moving from `from` to `to` seconds after the start,
    /// as `(marker_id, iteration)` in time order. A marker at 0 is crossed
    /// by the first advance. A yoyo's reversed iterations cross their markers
    /// in reverse, and a marker where playback turns is crossed once, by the
    /// iteration that reaches it.
    pub fn crossings(&self, from: f64, to: f64, crossed: &mut Vec<(u64, u32)>) {
        let length = f64::from(self.iteration_seconds);
        if self.markers.is_empty() || to <= from || length <= 0.0 {
            return;
        }
        let last = self.iterations().map_or(u32::MAX, |count| count - 1);
        let first_iteration = (from / length).floor() as u32;
        let last_iteration = ((to / length).floor() as u32).min(last);
        let mut found = Vec::new();
        for iteration in first_iteration..=last_iteration {
            for marker in &self.markers {
                let time = f64::from(marker.time_seconds);
                if time > length {
                    continue;
                }
                let reversed = self.yoyo && iteration % 2 == 1;
                let into = if reversed { length - time } else { time };
                // The previous iteration ended on this point.
                if self.yoyo && iteration > 0 && into == 0.0 {
                    continue;
                }
                let at = f64::from(iteration) * length + into;
                if (at > from || (at == 0.0 && from == 0.0)) && at <= to {
                    found.push((at, marker.marker_id, iteration));
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        crossed.extend(found.into_iter().map(|(_, id, iteration)| (id, iteration)));
    }
}

/// A presentation offset: translation added in parent space, rotation and
/// scale applied in the object's local frame, tint multiplying its colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TweenOffset {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub tint: [f32; 4],
}

impl TweenOffset {
    pub const IDENTITY: Self = Self {
        translation: [0.0; 3],
        rotation: IDENTITY_ROTATION,
        scale: [1.0; 3],
        tint: ONE,
    };

    /// This offset followed by `next`.
    pub fn then(&self, next: &Self) -> Self {
        Self {
            translation: std::array::from_fn(|axis| {
                self.translation[axis] + next.translation[axis]
            }),
            rotation: quat_mul(self.rotation, next.rotation),
            scale: std::array::from_fn(|axis| self.scale[axis] * next.scale[axis]),
            tint: std::array::from_fn(|axis| self.tint[axis] * next.tint[axis]),
        }
    }

    pub fn apply(&self, published: &Transform) -> Transform {
        Transform {
            translation: std::array::from_fn(|axis| {
                published.translation[axis] + self.translation[axis]
            }),
            rotation: normalize(quat_mul(published.rotation, self.rotation))
                .unwrap_or(published.rotation),
            scale: std::array::from_fn(|axis| published.scale[axis] * self.scale[axis]),
        }
    }

    pub fn tint(&self, color: [f32; 4]) -> [f32; 4] {
        std::array::from_fn(|channel| color[channel] * self.tint[channel])
    }

    /// The offset that shows `presented` over `published`. A zero published
    /// scale component carries no scale on that axis.
    pub fn between(presented: &Transform, published: &Transform, tint: [f32; 4]) -> Self {
        Self {
            translation: std::array::from_fn(|axis| {
                presented.translation[axis] - published.translation[axis]
            }),
            rotation: normalize(quat_mul(
                quat_conjugate(published.rotation),
                presented.rotation,
            ))
            .unwrap_or(IDENTITY_ROTATION),
            scale: std::array::from_fn(|axis| {
                if published.scale[axis] == 0.0 {
                    1.0
                } else {
                    presented.scale[axis] / published.scale[axis]
                }
            }),
            tint,
        }
    }

    fn channel(&self, channel: TweenChannel) -> [f32; 4] {
        match channel {
            TweenChannel::Translation => [
                self.translation[0],
                self.translation[1],
                self.translation[2],
                0.0,
            ],
            TweenChannel::Rotation => self.rotation,
            TweenChannel::Scale => [self.scale[0], self.scale[1], self.scale[2], 1.0],
            TweenChannel::Tint => self.tint,
        }
    }

    fn set_channel(&mut self, channel: TweenChannel, value: [f32; 4]) {
        let [x, y, z, _] = value;
        match channel {
            TweenChannel::Translation => self.translation = [x, y, z],
            TweenChannel::Rotation => self.rotation = normalize(value).unwrap_or(IDENTITY_ROTATION),
            TweenChannel::Scale => self.scale = [x, y, z],
            TweenChannel::Tint => self.tint = value,
        }
    }
}

fn lerp(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|index| from[index] + (to[index] - from[index]) * t)
}

fn catmull_rom(p0: [f32; 4], p1: [f32; 4], p2: [f32; 4], p3: [f32; 4], t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    std::array::from_fn(|index| {
        0.5 * (2.0 * p1[index]
            + (p2[index] - p0[index]) * t
            + (2.0 * p0[index] - 5.0 * p1[index] + 4.0 * p2[index] - p3[index]) * t2
            + (3.0 * p1[index] - p0[index] - 3.0 * p2[index] + p3[index]) * t3)
    })
}

fn normalize(q: [f32; 4]) -> Option<[f32; 4]> {
    let length = q.iter().map(|value| value * value).sum::<f32>().sqrt();
    (length > f32::EPSILON && length.is_finite()).then(|| q.map(|value| value / length))
}

fn quat_mul([ax, ay, az, aw]: [f32; 4], [bx, by, bz, bw]: [f32; 4]) -> [f32; 4] {
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn quat_conjugate([x, y, z, w]: [f32; 4]) -> [f32; 4] {
    [-x, -y, -z, w]
}

/// `from` turned `t` of the shortest way to `to`; `t` outside `[0, 1]`
/// undershoots or overshoots along the same axis.
fn rotate_toward(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    let mut delta = quat_mul(quat_conjugate(from), to);
    if delta[3] < 0.0 {
        delta = delta.map(|value| -value);
    }
    let sine = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if sine <= f32::EPSILON {
        return from;
    }
    let angle = 2.0 * sine.atan2(delta[3]) * t;
    let (half_sine, half_cosine) = (0.5 * angle).sin_cos();
    let scale = half_sine / sine;
    let step = [
        delta[0] * scale,
        delta[1] * scale,
        delta[2] * scale,
        half_cosine,
    ];
    normalize(quat_mul(from, step)).unwrap_or(from)
}

/// Smooth value noise in `[-1, 1]` along `at`, one stream per seed and axis.
fn noise(seed: u32, axis: u32, at: f32) -> f32 {
    let lattice = at.floor();
    let fraction = at - lattice;
    let smooth = fraction * fraction * (3.0 - 2.0 * fraction);
    let value = |index: i64| {
        let mut hash = (u64::from(seed) << 32 | u64::from(axis))
            ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        hash ^= hash >> 30;
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash ^= hash >> 27;
        hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
        hash ^= hash >> 31;
        (hash >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    let index = lattice as i64;
    let a = value(index);
    a + (value(index + 1) - a) * smooth
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenClock {
    /// World presentation time: holds and slows with gameplay time.
    World,
    /// Unscaled host time: keeps moving while the world is held.
    Realtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenState {
    Playing,
    Paused,
    Completed,
}

/// One playing tween: its timeline, clock and position.
#[derive(Debug, Clone, PartialEq)]
pub struct TweenPlayback {
    definition: TweenDefinition,
    clock: TweenClock,
    elapsed_seconds: f64,
    state: TweenState,
}

impl TweenPlayback {
    pub fn new(definition: TweenDefinition, clock: TweenClock) -> Self {
        Self {
            definition,
            clock,
            elapsed_seconds: 0.0,
            state: TweenState::Playing,
        }
    }

    pub fn definition(&self) -> &TweenDefinition {
        &self.definition
    }

    pub fn clock(&self) -> TweenClock {
        self.clock
    }

    pub fn state(&self) -> TweenState {
        self.state
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    /// The iteration the playback is in.
    pub fn iteration(&self) -> u32 {
        self.definition.position(self.elapsed_seconds).0
    }

    /// Advances a playing tween by its own clock's seconds, collecting the
    /// markers crossed. Returns whether it completed.
    pub fn advance(
        &mut self,
        world_seconds: f64,
        realtime_seconds: f64,
        crossed: &mut Vec<(u64, u32)>,
    ) -> bool {
        if self.state != TweenState::Playing {
            return false;
        }
        let seconds = match self.clock {
            TweenClock::World => world_seconds,
            TweenClock::Realtime => realtime_seconds,
        };
        let total = self.definition.total_seconds();
        let mut next = self.elapsed_seconds + seconds.max(0.0);
        if let Some(total) = total {
            next = next.min(total);
        }
        self.definition
            .crossings(self.elapsed_seconds, next, crossed);
        self.elapsed_seconds = next;
        if total.is_some_and(|total| next >= total) {
            self.state = TweenState::Completed;
            return true;
        }
        false
    }

    pub fn pause(&mut self) {
        if self.state == TweenState::Playing {
            self.state = TweenState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == TweenState::Paused {
            self.state = TweenState::Playing;
        }
    }

    /// Moves to `elapsed` seconds after the start, clamped to the timeline,
    /// keeping whether it plays or is paused. Markers between are not
    /// crossed: the pose jumps there. A tween moved to its end completes at
    /// its next advance. A completed tween stays where it ended.
    pub fn seek(&mut self, elapsed: f64) {
        if self.state == TweenState::Completed {
            return;
        }
        let total = self.definition.total_seconds().unwrap_or(f64::INFINITY);
        self.elapsed_seconds = elapsed.clamp(0.0, total);
    }

    /// Jumps to the end. A tween that plays forever stops where it is.
    pub fn complete(&mut self) {
        if let Some(total) = self.definition.total_seconds() {
            self.elapsed_seconds = total;
        }
        self.state = TweenState::Completed;
    }

    pub fn offset(&self) -> TweenOffset {
        self.definition.sample(self.elapsed_seconds)
    }
}
