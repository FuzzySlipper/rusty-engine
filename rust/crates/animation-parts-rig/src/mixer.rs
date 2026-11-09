//! One clip at a time, with a crossfade from the clips before it: rig-c's
//! live-tracks mixer (`src/core/tracks.ts`) on its first track, at full
//! alpha, with no queue.
//!
//! Each step starts from the setup pose and applies the playing clip and
//! every clip still fading out under it, oldest first, the way the mixer's
//! modes decide: a property the older clips key fades from where they left
//! it, a property only the fading clip keys fades back to setup, and a
//! property the incoming clip keys is held under it until it takes over.

use crate::constraints::ConstraintValues;
use crate::rig::{
    AnimationData, BoneTimeline, ColourTimeline, Geometry, Property, Rig, Timeline, TimelineKind,
};
use crate::timeline::key_index_at;
use crate::world::Local;

const CURRENT: u8 = 0;
const SETUP: u8 = 1;
const FIRST: u8 = 2;
const MODE: u8 = 3;
const HOLD: u8 = 4;

const ATTACH_NONE: u8 = 0;
const ATTACH_SETUP: u8 = 1;
const ATTACH_RETAIN: u8 = 2;

/// One clip on the track, playing or fading out.
#[derive(Clone)]
pub(crate) struct Entry {
    /// `None` is the empty clip a stop fades to.
    pub animation: Option<usize>,
    pub looping: bool,
    pub track_time: f64,
    track_last: f64,
    next_track_last: f64,
    track_end: f64,
    animation_last: f64,
    next_animation_last: f64,
    mix_time: f64,
    mix_duration: f64,
    total_alpha: f64,
    keep_hold: bool,
    timeline_mode: Vec<u8>,
    /// The newer entry (an index into the chain) whose mix ends a hold.
    timeline_hold_mix: Vec<Option<usize>>,
    timelines_rotation: Vec<f64>,
}

impl Entry {
    fn new(animation: Option<usize>, looping: bool) -> Entry {
        Entry {
            animation,
            looping,
            track_time: 0.0,
            track_last: -1.0,
            next_track_last: -1.0,
            track_end: f64::MAX,
            animation_last: -1.0,
            next_animation_last: -1.0,
            mix_time: 0.0,
            mix_duration: 0.0,
            total_alpha: 0.0,
            keep_hold: false,
            timeline_mode: Vec::new(),
            timeline_hold_mix: Vec::new(),
            timelines_rotation: Vec::new(),
        }
    }

    fn duration(&self, rig: &Rig) -> f64 {
        self.animation.map_or(0.0, |a| rig.animations[a].duration)
    }

    /// The clip time this entry poses at.
    pub fn animation_time(&self, rig: &Rig) -> f64 {
        let d = self.duration(rig);
        if !self.looping {
            return (self.track_time + 0.0).min(d);
        }
        if d == 0.0 {
            return 0.0;
        }
        (self.track_time % d) + 0.0
    }

    fn mix(&self) -> f64 {
        if self.mix_duration == 0.0 {
            1.0
        } else {
            (self.mix_time / self.mix_duration).min(1.0)
        }
    }
}

fn timelines<'r>(rig: &'r Rig, entry: &Entry) -> &'r [Timeline] {
    entry
        .animation
        .map_or(&[], |a| &rig.animations[a].timelines)
}

fn has_any(animation: Option<&AnimationData>, properties: &[Property]) -> bool {
    animation.is_some_and(|a| properties.iter().any(|p| a.properties.contains(p)))
}

/// What a slot shows and how, while the clips apply.
#[derive(Clone)]
pub(crate) struct SlotState {
    pub placeholder: Option<u32>,
    pub light: [f64; 4],
    pub dark: Option<[f64; 3]>,
    pub deform: Option<Vec<f64>>,
    attachment_state: u8,
}

/// The pose the clips write into: bones' local values and slots' states,
/// reset to setup each step.
#[derive(Clone)]
pub(crate) struct Working {
    pub locals: Vec<Local>,
    pub slots: Vec<SlotState>,
    pub active: Vec<bool>,
    /// `(event key index, animation)` fired this step, in order.
    pub events: Vec<(usize, usize)>,
    /// The skins a placeholder resolves through: the chosen one, then the
    /// default.
    pub lookup: Vec<usize>,
    /// Each constraint's keyed values.
    pub constraints: Vec<ConstraintValues>,
    target: Vec<f64>,
    scratch: Vec<f64>,
}

impl Working {
    pub fn new() -> Working {
        Working {
            locals: Vec::new(),
            slots: Vec::new(),
            active: Vec::new(),
            events: Vec::new(),
            lookup: Vec::new(),
            constraints: Vec::new(),
            target: Vec::new(),
            scratch: Vec::new(),
        }
    }

    pub fn reset(&mut self, rig: &Rig) {
        self.locals.clear();
        self.locals.extend(rig.bones.iter().map(|b| b.setup));
        self.slots.clear();
        self.slots.extend(rig.slots.iter().map(|s| SlotState {
            placeholder: s.setup,
            light: s.light,
            dark: s.dark,
            deform: None,
            attachment_state: ATTACH_NONE,
        }));
        self.events.clear();
        self.constraints.clear();
        self.constraints
            .extend(rig.constraints.iter().map(|c| c.setup));
    }

    /// The attachment a slot shows under a placeholder.
    pub fn resolve(&self, rig: &Rig, slot: usize, placeholder: Option<u32>) -> Option<usize> {
        let placeholder = placeholder?;
        self.lookup
            .iter()
            .find_map(|&skin| rig.skins[skin].table.get(&(slot, placeholder)).copied())
    }

    fn slot_live(&self, rig: &Rig, slot: usize) -> bool {
        self.active[rig.slots[slot].bone]
    }

    /// rig-c's `Pose.setAttachment`: a change of attachment drops the deform
    /// unless both draw the same vertices under the same timelines.
    fn set_attachment(&mut self, rig: &Rig, slot: usize, placeholder: Option<u32>) {
        let was = self.resolve(rig, slot, self.slots[slot].placeholder);
        let now = self.resolve(rig, slot, placeholder);
        self.slots[slot].placeholder = placeholder;
        if was == now {
            return;
        }
        let vertex = |a: Option<usize>| {
            a.filter(|&a| {
                matches!(
                    rig.attachments[a].geometry,
                    Geometry::Mesh(_) | Geometry::Linked { .. }
                )
            })
        };
        match (vertex(was), vertex(now)) {
            (Some(w), Some(n))
                if rig.attachments[w].deform_identity == rig.attachments[n].deform_identity => {}
            _ => self.slots[slot].deform = None,
        }
    }
}

/// The track: the playing entry first, then each entry it fades from.
#[derive(Clone, Default)]
pub(crate) struct Mixer {
    pub chain: Vec<Entry>,
    changed: bool,
}

impl Mixer {
    /// rig-c's `setAnimation` on track 0, then its `mixDuration`.
    pub fn set(&mut self, animation: Option<usize>, looping: bool, mix_duration: f64) {
        if let Some(current) = self.chain.first() {
            if current.next_track_last == -1.0 && current.animation == animation {
                self.chain.remove(0);
                self.changed = true;
            }
        }
        let mut entry = Entry::new(animation, looping);
        entry.mix_duration = mix_duration;
        if animation.is_none() {
            entry.track_end = mix_duration;
        }
        if let Some(from) = self.chain.first_mut() {
            from.timelines_rotation.clear();
        }
        self.chain.insert(0, entry);
        self.changed = true;
    }

    pub fn update(&mut self, delta: f64) {
        let Some(current) = self.chain.first_mut() else {
            return;
        };
        current.animation_last = current.next_animation_last;
        current.track_last = current.next_track_last;
        if current.track_last >= current.track_end && self.chain.len() == 1 {
            self.chain.clear();
            self.changed = true;
            return;
        }
        if self.chain.len() > 1 && self.update_mixing_from(0, delta) {
            self.chain.truncate(1);
            self.changed = true;
        }
        self.chain[0].track_time += delta;
    }

    fn update_mixing_from(&mut self, to: usize, delta: f64) -> bool {
        let from = to + 1;
        if from >= self.chain.len() {
            return true;
        }
        let finished = self.update_mixing_from(from, delta);
        let f = &mut self.chain[from];
        f.animation_last = f.next_animation_last;
        f.track_last = f.next_track_last;
        let (to_next_track_last, to_mix_time, to_mix_duration) = (
            self.chain[to].next_track_last,
            self.chain[to].mix_time,
            self.chain[to].mix_duration,
        );
        if to_next_track_last != -1.0 && to_mix_time >= to_mix_duration {
            if self.chain[from].total_alpha == 0.0 || to_mix_duration == 0.0 {
                let hold = self.chain[from].total_alpha == 0.0;
                self.chain.remove(from);
                if hold {
                    for entry in self.chain[1..=to].iter_mut() {
                        entry.keep_hold = true;
                    }
                }
                self.changed = true;
            }
            return finished;
        }
        self.chain[from].track_time += delta;
        self.chain[to].mix_time += delta;
        false
    }

    /// Applies the track to `pose`; returns the playing entry's clip time.
    pub fn apply(&mut self, rig: &Rig, pose: &mut Working) -> f64 {
        if self.chain.is_empty() {
            return 0.0;
        }
        if self.changed {
            self.compute_modes(rig);
        }
        let mut alpha = 1.0;
        if self.chain.len() > 1 {
            alpha *= self.apply_mixing_from(rig, 0, pose);
        } else if self.chain[0].track_time >= self.chain[0].track_end {
            alpha = 0.0;
        }
        let current = &self.chain[0];
        let animation_last = current.animation_last;
        let animation_time = current.animation_time(rig);
        let tls = timelines(rig, current);
        if alpha == 1.0 {
            for tl in tls {
                if let TimelineKind::Attachment { .. } = tl.kind {
                    apply_attachment(rig, pose, tl, animation_time, SETUP, true);
                } else {
                    apply_timeline(
                        rig,
                        pose,
                        tl,
                        current.animation,
                        animation_last,
                        animation_time,
                        true,
                        1.0,
                        SETUP,
                        false,
                    );
                }
            }
        } else {
            let count = tls.len();
            let first_frame = current.timelines_rotation.len() != count * 2;
            let animation = current.animation;
            let modes = std::mem::take(&mut self.chain[0].timeline_mode);
            let mut rotations = std::mem::take(&mut self.chain[0].timelines_rotation);
            if first_frame {
                rotations.resize(count * 2, f64::NAN);
            }
            for (i, tl) in tls.iter().enumerate() {
                let from = modes[i] & MODE;
                match tl.kind {
                    TimelineKind::Bone {
                        kind: BoneTimeline::Rotate,
                        ..
                    } => apply_rotate(
                        rig,
                        pose,
                        tl,
                        animation_time,
                        alpha,
                        from,
                        &mut rotations,
                        i * 2,
                        first_frame,
                    ),
                    TimelineKind::Attachment { .. } => {
                        apply_attachment(rig, pose, tl, animation_time, from, alpha >= 0.0)
                    }
                    _ => apply_timeline(
                        rig,
                        pose,
                        tl,
                        animation,
                        animation_last,
                        animation_time,
                        true,
                        alpha,
                        from,
                        false,
                    ),
                }
            }
            self.chain[0].timelines_rotation = rotations;
            self.chain[0].timeline_mode = modes;
        }
        let current = &mut self.chain[0];
        current.next_animation_last = animation_time;
        current.next_track_last = current.track_time;
        for slot in 0..pose.slots.len() {
            if pose.slots[slot].attachment_state == ATTACH_SETUP {
                pose.set_attachment(rig, slot, rig.slots[slot].setup);
            }
        }
        animation_time
    }

    fn apply_mixing_from(&mut self, rig: &Rig, to: usize, pose: &mut Working) -> f64 {
        let from = to + 1;
        let from_mix = if from + 1 < self.chain.len() {
            self.apply_mixing_from(rig, from, pose)
        } else {
            1.0
        };
        let mix = self.chain[to].mix();
        let a = from_mix;
        let keep = 1.0 - mix;
        let alpha_mix = a * (1.0 - mix);
        let alpha_hold = if keep > 0.0 { alpha_mix / keep } else { a };
        let hold_mixes: Vec<f64> = self.chain[from]
            .timeline_hold_mix
            .iter()
            .map(|h| h.map_or(f64::NAN, |h| self.chain[h].mix()))
            .collect();
        let entry = &mut self.chain[from];
        let tls = timelines(rig, entry);
        let count = tls.len();
        let first_frame = entry.timelines_rotation.len() != count * 2;
        if first_frame {
            entry.timelines_rotation.resize(count * 2, f64::NAN);
        }
        let animation_last = entry.animation_last;
        let animation_time = entry.animation_time(rig);
        entry.total_alpha = 0.0;
        for (i, tl) in tls.iter().enumerate() {
            let mode = entry.timeline_mode[i];
            let mix_from = mode & MODE;
            let alpha = if mode & HOLD != 0 {
                match hold_mixes.get(i) {
                    Some(h) if !h.is_nan() => alpha_hold * (1.0 - h),
                    _ => alpha_hold,
                }
            } else {
                alpha_mix
            };
            entry.total_alpha += alpha;
            match tl.kind {
                TimelineKind::Bone {
                    kind: BoneTimeline::Rotate,
                    ..
                } => apply_rotate(
                    rig,
                    pose,
                    tl,
                    animation_time,
                    alpha,
                    mix_from,
                    &mut entry.timelines_rotation,
                    i * 2,
                    first_frame,
                ),
                TimelineKind::Attachment { .. } => {
                    apply_attachment(rig, pose, tl, animation_time, mix_from, false)
                }
                _ => apply_timeline(
                    rig,
                    pose,
                    tl,
                    entry.animation,
                    animation_last,
                    animation_time,
                    false,
                    alpha,
                    mix_from,
                    true,
                ),
            }
        }
        entry.next_animation_last = animation_time;
        entry.next_track_last = entry.track_time;
        mix
    }

    /// rig-c's `computeModes` for one track: oldest entry first.
    fn compute_modes(&mut self, rig: &Rig) {
        self.changed = false;
        let mut seen: std::collections::HashSet<Property> = std::collections::HashSet::new();
        for index in (0..self.chain.len()).rev() {
            let entry = &self.chain[index];
            let tls = timelines(rig, entry);
            let to = index.checked_sub(1);
            let mut modes = std::mem::take(&mut self.chain[index].timeline_mode);
            modes.resize(tls.len(), 0);
            let mut hold_mix = vec![None; tls.len()];
            for (i, tl) in tls.iter().enumerate() {
                let mut from = SETUP;
                for p in &tl.properties {
                    if !seen.insert(*p) {
                        from = FIRST;
                    }
                }
                let to_animation = to
                    .and_then(|t| self.chain[t].animation)
                    .map(|a| &rig.animations[a]);
                let mut mode =
                    if to.is_none() || tl.instant || !has_any(to_animation, &tl.properties) {
                        from
                    } else {
                        let mut next = to.and_then(|t| t.checked_sub(1));
                        while let Some(n) = next {
                            let n_animation = self.chain[n].animation.map(|a| &rig.animations[a]);
                            if !has_any(n_animation, &tl.properties) {
                                if self.chain[n].mix_duration > 0.0 {
                                    hold_mix[i] = Some(n);
                                }
                                break;
                            }
                            next = n.checked_sub(1);
                        }
                        from | HOLD
                    };
                if self.chain[index].keep_hold {
                    mode = (mode & !HOLD) | (modes[i] & HOLD);
                }
                modes[i] = mode;
            }
            let entry = &mut self.chain[index];
            entry.timeline_mode = modes;
            entry.timeline_hold_mix = hold_mix;
        }
    }
}

fn before_first_key(from: u8, alpha: f64, current: f64, setup: f64) -> f64 {
    match from {
        SETUP => setup,
        FIRST => current + (setup - current) * alpha,
        _ => current,
    }
}

fn scale_value(value: f64, alpha: f64, from: u8, out: bool, current: f64, setup: f64) -> f64 {
    if alpha == 1.0 {
        return value;
    }
    let mut base = if from == SETUP { setup } else { current };
    if out {
        return base + (value.abs() * sign(base) - base) * alpha;
    }
    base = base.abs() * sign(value);
    base + (value - base) * alpha
}

/// JavaScript's `Math.sign`.
fn sign(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        v
    }
}

fn signum(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_timeline(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    animation: Option<usize>,
    last: f64,
    time: f64,
    fire: bool,
    alpha: f64,
    from: u8,
    out: bool,
) {
    match &tl.kind {
        TimelineKind::Bone { .. } | TimelineKind::Inherit { .. } => {
            apply_bone(rig, pose, tl, time, alpha, from, out)
        }
        TimelineKind::Colour { .. } => apply_colour(rig, pose, tl, time, alpha, from),
        TimelineKind::Deform { .. } => apply_deform(rig, pose, tl, time, alpha, from),
        TimelineKind::Constraint { .. } => apply_constraint(rig, pose, tl, time, alpha, from, out),
        TimelineKind::Events(keys) => {
            if fire {
                let Some(animation) = animation else { return };
                let wrapped = time < last;
                if wrapped {
                    pose.events.extend(
                        keys.iter()
                            .enumerate()
                            .filter(|(_, k)| k.time > last)
                            .map(|(i, _)| (i, animation)),
                    );
                    pose.events.extend(
                        keys.iter()
                            .enumerate()
                            .filter(|(_, k)| k.time <= time)
                            .map(|(i, _)| (i, animation)),
                    );
                } else {
                    pose.events.extend(
                        keys.iter()
                            .enumerate()
                            .filter(|(_, k)| k.time > last && k.time <= time)
                            .map(|(i, _)| (i, animation)),
                    );
                }
            }
        }
        TimelineKind::Attachment { .. } => {}
    }
}

fn apply_bone(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    out: bool,
) {
    let (bone, keys, kind) = match &tl.kind {
        TimelineKind::Inherit { bone, keys } => {
            if !pose.active[*bone] {
                return;
            }
            let setup = rig.bones[*bone].setup.inherit;
            let local = &mut pose.locals[*bone];
            if out {
                if from != CURRENT {
                    local.inherit = setup;
                }
                return;
            }
            match key_index_at(keys.iter().map(|k| k.0), time) {
                None => {
                    if from != CURRENT {
                        local.inherit = setup;
                    }
                }
                Some(i) => local.inherit = keys[i].1,
            }
            return;
        }
        TimelineKind::Bone { bone, kind, keys } => (*bone, keys, *kind),
        _ => return,
    };
    if !pose.active[bone] {
        return;
    }
    let setup = rig.bones[bone].setup;
    let mut v = [0.0; 2];
    let keyed = keys.values_at(time, &mut v);
    let local = &mut pose.locals[bone];
    let relative = |current: f64, s: f64, value: f64| -> f64 {
        if !keyed {
            before_first_key(from, alpha, current, s)
        } else if from == SETUP {
            s + value * alpha
        } else {
            current + (value + s - current) * alpha
        }
    };
    let pair = |cx: f64, cy: f64, sx: f64, sy: f64| -> (f64, f64) {
        if !keyed {
            return match from {
                SETUP => (sx, sy),
                FIRST => (cx + (sx - cx) * alpha, cy + (sy - cy) * alpha),
                _ => (cx, cy),
            };
        }
        if from == SETUP {
            (sx + v[0] * alpha, sy + v[1] * alpha)
        } else {
            (cx + (sx + v[0] - cx) * alpha, cy + (sy + v[1] - cy) * alpha)
        }
    };
    let scale = |current: f64, s: f64, value: f64| -> f64 {
        if !keyed {
            before_first_key(from, alpha, current, s)
        } else {
            scale_value(value * s, alpha, from, out, current, s)
        }
    };
    use BoneTimeline::*;
    match kind {
        Rotate => local.rotation = relative(local.rotation, setup.rotation, v[0]),
        Translate => (local.x, local.y) = pair(local.x, local.y, setup.x, setup.y),
        TranslateX => local.x = relative(local.x, setup.x, v[0]),
        TranslateY => local.y = relative(local.y, setup.y, v[0]),
        Shear => {
            (local.shear_x, local.shear_y) =
                pair(local.shear_x, local.shear_y, setup.shear_x, setup.shear_y)
        }
        ShearX => local.shear_x = relative(local.shear_x, setup.shear_x, v[0]),
        ShearY => local.shear_y = relative(local.shear_y, setup.shear_y, v[0]),
        Scale => {
            if !keyed {
                match from {
                    SETUP => (local.scale_x, local.scale_y) = (setup.scale_x, setup.scale_y),
                    FIRST => {
                        local.scale_x += (setup.scale_x - local.scale_x) * alpha;
                        local.scale_y += (setup.scale_y - local.scale_y) * alpha;
                    }
                    _ => {}
                }
            } else {
                local.scale_x = scale_value(
                    v[0] * setup.scale_x,
                    alpha,
                    from,
                    out,
                    local.scale_x,
                    setup.scale_x,
                );
                local.scale_y = scale_value(
                    v[1] * setup.scale_y,
                    alpha,
                    from,
                    out,
                    local.scale_y,
                    setup.scale_y,
                );
            }
        }
        ScaleX => local.scale_x = scale(local.scale_x, setup.scale_x, v[0]),
        ScaleY => local.scale_y = scale(local.scale_y, setup.scale_y, v[0]),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_rotate(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    rotations: &mut [f64],
    i: usize,
    first_frame: bool,
) {
    if first_frame {
        rotations[i] = 0.0;
    }
    if alpha == 1.0 {
        apply_bone(rig, pose, tl, time, 1.0, from, false);
        return;
    }
    let TimelineKind::Bone { bone, keys, .. } = &tl.kind else {
        return;
    };
    if !pose.active[*bone] {
        return;
    }
    let setup = rig.bones[*bone].setup.rotation;
    let local = &mut pose.locals[*bone];
    let mut v = [0.0; 1];
    let (r1, r2) = if !keys.values_at(time, &mut v) {
        match from {
            SETUP => {
                local.rotation = setup;
                return;
            }
            CURRENT => return,
            _ => (local.rotation, setup),
        }
    } else {
        (
            if from == SETUP { setup } else { local.rotation },
            setup + v[0],
        )
    };
    let mut diff = r2 - r1;
    diff -= (diff / 360.0 - 0.5).ceil() * 360.0;
    let total = if diff == 0.0 {
        rotations[i]
    } else {
        let (last_total, last_diff) = if first_frame {
            (0.0, diff)
        } else {
            (rotations[i], rotations[i + 1])
        };
        let loops = last_total - (last_total % 360.0);
        let mut t = diff + loops;
        let current = diff >= 0.0;
        let mut dir = last_total >= 0.0;
        if last_diff.abs() <= 90.0 && signum(last_diff) != signum(diff) {
            if (last_total - loops).abs() > 180.0 {
                t += 360.0 * signum(last_total);
                dir = current;
            } else if loops != 0.0 {
                t -= 360.0 * signum(last_total);
            } else {
                dir = current;
            }
        }
        if dir != current {
            t += 360.0 * signum(last_total);
        }
        rotations[i] = t;
        t
    };
    rotations[i + 1] = diff;
    local.rotation = r1 + total * alpha;
}

fn apply_attachment(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    from: u8,
    retain: bool,
) {
    let TimelineKind::Attachment { slot, keys } = &tl.kind else {
        return;
    };
    let slot = *slot;
    if !pose.slot_live(rig, slot) {
        return;
    }
    if !retain && pose.slots[slot].attachment_state == ATTACH_RETAIN {
        return;
    }
    let index = key_index_at(keys.iter().map(|k| k.0), time);
    let mut setup = index.is_none();
    let mut name = None;
    if let Some(i) = index {
        name = keys[i].1;
        setup = !retain && name.is_none();
    }
    if setup {
        if from == CURRENT {
            return;
        }
        name = rig.slots[slot].setup;
    }
    pose.set_attachment(rig, slot, name);
    if retain {
        pose.slots[slot].attachment_state = ATTACH_RETAIN;
    } else if !setup {
        pose.slots[slot].attachment_state = ATTACH_SETUP;
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn apply_colour(rig: &Rig, pose: &mut Working, tl: &Timeline, time: f64, alpha: f64, from: u8) {
    let TimelineKind::Colour { slot, kind, keys } = &tl.kind else {
        return;
    };
    let slot = *slot;
    if !pose.slot_live(rig, slot) {
        return;
    }
    let sl = rig.slots[slot].light;
    let sd = rig.slots[slot].dark.unwrap_or([0.0; 3]);
    let mut v = [0.0; 7];
    let keyed = keys.values_at(time, &mut v);
    let s = &mut pose.slots[slot];
    let l = &mut s.light;
    let clamp_light = |l: &mut [f64; 4]| l.iter_mut().for_each(|c| *c = clamp01(*c));
    match kind {
        ColourTimeline::Rgba => {
            if !keyed {
                if from == SETUP {
                    *l = sl;
                } else if from == FIRST {
                    for k in 0..4 {
                        l[k] += (sl[k] - l[k]) * alpha;
                    }
                    clamp_light(l);
                }
                return;
            }
            for k in 0..4 {
                l[k] = if alpha == 1.0 {
                    v[k]
                } else if from == SETUP {
                    sl[k] + (v[k] - sl[k]) * alpha
                } else {
                    l[k] + (v[k] - l[k]) * alpha
                };
            }
            clamp_light(l);
        }
        ColourTimeline::Rgb => {
            if !keyed {
                if from == SETUP {
                    l[..3].copy_from_slice(&sl[..3]);
                } else if from == FIRST {
                    for k in 0..3 {
                        l[k] += (sl[k] - l[k]) * alpha;
                    }
                }
                return;
            }
            for k in 0..3 {
                let c = if alpha == 1.0 {
                    v[k]
                } else if from == SETUP {
                    sl[k] + (v[k] - sl[k]) * alpha
                } else {
                    l[k] + (v[k] - l[k]) * alpha
                };
                l[k] = clamp01(c);
            }
        }
        ColourTimeline::Alpha => {
            if !keyed {
                if from == SETUP {
                    l[3] = sl[3];
                } else if from == FIRST {
                    l[3] += (sl[3] - l[3]) * alpha;
                }
                return;
            }
            let a = if alpha == 1.0 {
                v[0]
            } else if from == SETUP {
                sl[3] + (v[0] - sl[3]) * alpha
            } else {
                l[3] + (v[0] - l[3]) * alpha
            };
            l[3] = clamp01(a);
        }
        ColourTimeline::Rgba2 | ColourTimeline::Rgb2 => {
            let Some(d) = s.dark.as_mut() else { return };
            let with_alpha = *kind == ColourTimeline::Rgba2;
            let light_n = if with_alpha { 4 } else { 3 };
            if !keyed {
                if from == SETUP {
                    l[..light_n].copy_from_slice(&sl[..light_n]);
                    *d = sd;
                } else if from == FIRST {
                    for k in 0..light_n {
                        l[k] += (sl[k] - l[k]) * alpha;
                    }
                    if with_alpha {
                        clamp_light(l);
                    }
                    for k in 0..3 {
                        d[k] += (sd[k] - d[k]) * alpha;
                    }
                }
                return;
            }
            let mut light = [v[0], v[1], v[2], v[3]];
            let mut dark = [v[light_n], v[light_n + 1], v[light_n + 2]];
            if with_alpha {
                if alpha == 1.0 {
                    *l = light;
                } else if from == SETUP {
                    for k in 0..4 {
                        l[k] = sl[k] + (light[k] - sl[k]) * alpha;
                    }
                    for k in 0..3 {
                        dark[k] = sd[k] + (dark[k] - sd[k]) * alpha;
                    }
                } else {
                    for k in 0..4 {
                        l[k] += (light[k] - l[k]) * alpha;
                    }
                    for k in 0..3 {
                        dark[k] = d[k] + (dark[k] - d[k]) * alpha;
                    }
                }
                clamp_light(l);
            } else {
                if alpha != 1.0 {
                    for k in 0..3 {
                        light[k] = if from == SETUP {
                            sl[k] + (light[k] - sl[k]) * alpha
                        } else {
                            l[k] + (light[k] - l[k]) * alpha
                        };
                        dark[k] = if from == SETUP {
                            sd[k] + (dark[k] - sd[k]) * alpha
                        } else {
                            d[k] + (dark[k] - d[k]) * alpha
                        };
                    }
                }
                for k in 0..3 {
                    l[k] = clamp01(light[k]);
                }
            }
            for k in 0..3 {
                d[k] = clamp01(dark[k]);
            }
        }
    }
}

fn apply_deform(rig: &Rig, pose: &mut Working, tl: &Timeline, time: f64, alpha: f64, from: u8) {
    let TimelineKind::Deform { identity, keys } = &tl.kind else {
        return;
    };
    let Geometry::Mesh(mesh) = &rig.attachments[*identity].geometry else {
        return;
    };
    let weighted = mesh.vertices.weighted();
    let mut target = std::mem::take(&mut pose.target);
    let mut scratch = std::mem::take(&mut pose.scratch);
    for slot in 0..pose.slots.len() {
        if !pose.slot_live(rig, slot) {
            continue;
        }
        let Some(shown) = pose.resolve(rig, slot, pose.slots[slot].placeholder) else {
            continue;
        };
        let shown = &rig.attachments[shown];
        if !matches!(shown.geometry, Geometry::Mesh(_) | Geometry::Linked { .. })
            || shown.deform_identity != *identity
        {
            continue;
        }
        let keyed = keys.at(&mesh.setup, weighted, time, &mut target, &mut scratch);
        let s = &mut pose.slots[slot];
        let mix_from = if s.deform.is_none() { SETUP } else { from };
        if !keyed {
            if mix_from == SETUP {
                s.deform = None;
            } else if mix_from == FIRST {
                if alpha == 1.0 {
                    s.deform = None;
                    continue;
                }
                if let Some(current) = s.deform.as_mut() {
                    if weighted {
                        let keep = 1.0 - alpha;
                        current.iter_mut().for_each(|c| *c *= keep);
                    } else {
                        for (c, setup) in current.iter_mut().zip(&mesh.setup) {
                            *c += (setup - *c) * alpha;
                        }
                    }
                }
            }
            continue;
        }
        if alpha == 1.0 {
            s.deform = Some(target.clone());
        } else if mix_from == SETUP {
            s.deform = Some(if weighted {
                target.iter().map(|v| v * alpha).collect()
            } else {
                target
                    .iter()
                    .zip(&mesh.setup)
                    .map(|(v, setup)| setup + (v - setup) * alpha)
                    .collect()
            });
        } else if let Some(current) = s.deform.as_mut() {
            for (c, v) in current.iter_mut().zip(&target) {
                *c += (v - *c) * alpha;
            }
        }
    }
    pose.target = target;
    pose.scratch = scratch;
}

/// A constraint timeline: its values from setup (or from what the clips
/// under it left) toward the keyed ones by `alpha`. At full alpha from setup
/// this is rig-c's `setup + (value - setup) * 1`; the stepped ik flags are
/// the key's, and a clip fading out leaves them.
fn apply_constraint(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    out: bool,
) {
    let TimelineKind::Constraint {
        constraint,
        keys,
        flags,
    } = &tl.kind
    else {
        return;
    };
    let setup = rig.constraints[*constraint].setup;
    let current = &mut pose.constraints[*constraint];
    let mut v = [0.0; 6];
    let Some(index) = keys.index_at(time) else {
        match from {
            SETUP => *current = setup,
            FIRST => {
                for (c, s) in current.values.iter_mut().zip(setup.values) {
                    *c += (s - *c) * alpha;
                }
                current.flags = setup.flags;
            }
            _ => {}
        }
        return;
    };
    keys.values_at(time, &mut v);
    let channels = keys.keys[index].values.len();
    for (k, value) in v.iter().enumerate().take(channels) {
        let base = if from == SETUP {
            setup.values[k]
        } else {
            current.values[k]
        };
        current.values[k] = base + (value - base) * alpha;
    }
    if let Some(f) = flags.get(index) {
        if !out {
            current.flags = *f;
        } else if from == SETUP {
            current.flags = setup.flags;
        }
    }
}
