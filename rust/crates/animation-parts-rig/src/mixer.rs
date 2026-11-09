//! Clips on layered tracks: rig-c's live-tracks mixer (`src/core/tracks.ts`),
//! with no queue.
//!
//! Each step starts from the setup pose and applies every track, lowest
//! first: its playing clip and every clip still fading out under it, oldest
//! first, the way the mixer's modes decide. On one track a property the
//! older clips key fades from where they left it, a property only the fading
//! clip keys fades back to setup, and a property the incoming clip keys is
//! held under it until it takes over. A track above mixes each property a
//! lower track keyed from the value it left, by its clip's alpha; an
//! additive clip adds its keyed values to that value instead (rig-c's
//! `TrackEntry.alpha` and `additive`).

use std::collections::{BTreeMap, HashMap};

use crate::constraints::{ConstraintKind, ConstraintValues};
use crate::physics::{GRAVITY, MASS_INVERSE, WIND};
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
    /// From 0 to 1, multiplied into every timeline's weight.
    pub alpha: f64,
    /// Adds its keyed values to what the tracks below posed.
    pub additive: bool,
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
            alpha: 1.0,
            additive: false,
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
    /// The physics constraints a `reset` key this step crossed resets:
    /// one, or (`None`) every one.
    pub resets: Vec<Option<usize>>,
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
            resets: Vec::new(),
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
        self.resets.clear();
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
/// The tracks, by index: each the playing entry first, then each entry it
/// fades from. A track plays over the tracks below it.
#[derive(Clone, Default)]
pub(crate) struct Mixer {
    pub tracks: BTreeMap<usize, Vec<Entry>>,
    changed: bool,
}

/// Whether an additive entry adds this timeline's keyed values, rather than
/// replacing what the tracks below posed (the runtime's `Timeline.additive`,
/// rig-c's `addable` and `ADDITIVE_MODE`): every bone value, a deform, a
/// transform's mixes and a physics constraint's wind and gravity.
fn addable(rig: &Rig, tl: &Timeline) -> bool {
    match &tl.kind {
        TimelineKind::Bone { .. } | TimelineKind::Deform { .. } => true,
        TimelineKind::Constraint { constraint, .. } => {
            matches!(
                rig.constraints[*constraint].kind,
                ConstraintKind::Transform(_)
            )
        }
        TimelineKind::Physics { value, .. } => *value == WIND || *value == GRAVITY,
        _ => false,
    }
}

/// rig-c's `propertyFrom`: `SETUP` for a property no timeline keyed yet,
/// `FIRST` for one an older entry of this track keyed, `CURRENT` for one a
/// lower track keyed.
fn property_from(
    owners: &mut HashMap<Property, usize>,
    track: usize,
    properties: &[Property],
) -> u8 {
    let mut from = SETUP;
    for (i, p) in properties.iter().enumerate() {
        match owners.get(p) {
            None => {
                owners.insert(*p, track);
            }
            Some(&owner) if owner != track => {
                for q in &properties[i + 1..] {
                    owners.entry(*q).or_insert(track);
                }
                return CURRENT;
            }
            Some(_) => from = FIRST,
        }
    }
    from
}

impl Mixer {
    /// rig-c's `setAnimation` on `track`, then its `mixDuration`, `alpha`
    /// and `additive`.
    pub fn set(
        &mut self,
        track: usize,
        animation: Option<usize>,
        looping: bool,
        mix_duration: f64,
        alpha: f64,
        additive: bool,
    ) {
        let chain = self.tracks.entry(track).or_default();
        if let Some(current) = chain.first() {
            if current.next_track_last == -1.0 && current.animation == animation {
                chain.remove(0);
            }
        }
        let mut entry = Entry::new(animation, looping);
        entry.mix_duration = mix_duration;
        entry.alpha = alpha;
        entry.additive = additive;
        if animation.is_none() {
            entry.track_end = mix_duration;
        }
        if let Some(from) = chain.first_mut() {
            from.timelines_rotation.clear();
        }
        chain.insert(0, entry);
        self.changed = true;
    }

    /// The playing entry of `track`, if it plays anything.
    pub fn current(&self, track: usize) -> Option<&Entry> {
        self.tracks.get(&track).and_then(|chain| chain.first())
    }

    /// rig-c's `setAlpha`: the playing entry's alpha from the next pose on;
    /// false when `track` plays nothing.
    pub fn set_alpha(&mut self, track: usize, alpha: f64) -> bool {
        match self
            .tracks
            .get_mut(&track)
            .and_then(|chain| chain.first_mut())
        {
            Some(entry) => {
                entry.alpha = alpha;
                true
            }
            None => false,
        }
    }

    pub fn update(&mut self, delta: f64) {
        for chain in self.tracks.values_mut() {
            let Some(current) = chain.first_mut() else {
                continue;
            };
            current.animation_last = current.next_animation_last;
            current.track_last = current.next_track_last;
            if current.track_last >= current.track_end && chain.len() == 1 {
                chain.clear();
                self.changed = true;
                continue;
            }
            if chain.len() > 1 && update_mixing_from(chain, 0, delta, &mut self.changed) {
                chain.truncate(1);
                self.changed = true;
            }
            chain[0].track_time += delta;
        }
    }

    /// Applies every track to `pose`, lowest first; returns track 0's
    /// playing entry's clip time (0 when it plays nothing).
    pub fn apply(&mut self, rig: &Rig, pose: &mut Working) -> f64 {
        if self.changed {
            self.compute_modes(rig);
        }
        let mut base = 0.0;
        for (&track, chain) in self.tracks.iter_mut() {
            if chain.is_empty() {
                continue;
            }
            let mut alpha = chain[0].alpha;
            if chain.len() > 1 {
                alpha *= apply_mixing_from(rig, chain, 0, pose);
            } else if chain[0].track_time >= chain[0].track_end {
                alpha = 0.0;
            }
            let current = &mut chain[0];
            let animation_last = current.animation_last;
            let animation_time = current.animation_time(rig);
            let animation = current.animation;
            let tls = timelines(rig, current);
            if track == 0 && alpha == 1.0 {
                // Track 0 at full weight poses from the setup pose and adds
                // nothing, additive or not.
                for tl in tls {
                    if let TimelineKind::Attachment { .. } = tl.kind {
                        apply_attachment(rig, pose, tl, animation_time, SETUP, true);
                    } else {
                        apply_timeline(
                            rig,
                            pose,
                            tl,
                            animation,
                            animation_last,
                            animation_time,
                            true,
                            1.0,
                            SETUP,
                            false,
                            false,
                        );
                    }
                }
            } else {
                // An additive entry's rotations add through the timeline
                // itself, never the remembered direction.
                let add = current.additive;
                let count = tls.len();
                let first_frame = !add && current.timelines_rotation.len() != count * 2;
                if first_frame {
                    current.timelines_rotation.resize(count * 2, f64::NAN);
                }
                for (i, tl) in tls.iter().enumerate() {
                    let from = current.timeline_mode[i] & MODE;
                    match tl.kind {
                        TimelineKind::Bone {
                            kind: BoneTimeline::Rotate,
                            ..
                        } if !add => apply_rotate(
                            rig,
                            pose,
                            tl,
                            animation_time,
                            alpha,
                            from,
                            &mut current.timelines_rotation,
                            i * 2,
                            first_frame,
                        ),
                        // An entry keeps its attachments at any alpha.
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
                            add,
                        ),
                    }
                }
            }
            if track == 0 {
                base = animation_time;
            }
            current.next_animation_last = animation_time;
            current.next_track_last = current.track_time;
        }
        for slot in 0..pose.slots.len() {
            if pose.slots[slot].attachment_state == ATTACH_SETUP {
                pose.set_attachment(rig, slot, rig.slots[slot].setup);
            }
        }
        base
    }

    /// rig-c's `computeModes`: every track lowest first, each track's
    /// entries oldest first.
    fn compute_modes(&mut self, rig: &Rig) {
        self.changed = false;
        let mut owners: HashMap<Property, usize> = HashMap::new();
        for (&track, chain) in self.tracks.iter_mut() {
            for index in (0..chain.len()).rev() {
                compute_hold(rig, chain, index, track, &mut owners);
            }
        }
    }
}

/// rig-c's `updateMixingFrom` for the entry at `to` in a track's chain.
fn update_mixing_from(chain: &mut Vec<Entry>, to: usize, delta: f64, changed: &mut bool) -> bool {
    let from = to + 1;
    if from >= chain.len() {
        return true;
    }
    let finished = update_mixing_from(chain, from, delta, changed);
    let f = &mut chain[from];
    f.animation_last = f.next_animation_last;
    f.track_last = f.next_track_last;
    let (to_next_track_last, to_mix_time, to_mix_duration) = (
        chain[to].next_track_last,
        chain[to].mix_time,
        chain[to].mix_duration,
    );
    if to_next_track_last != -1.0 && to_mix_time >= to_mix_duration {
        if chain[from].total_alpha == 0.0 || to_mix_duration == 0.0 {
            let hold = chain[from].total_alpha == 0.0;
            chain.remove(from);
            if hold {
                for entry in chain[1..=to].iter_mut() {
                    entry.keep_hold = true;
                }
            }
            *changed = true;
        }
        return finished;
    }
    chain[from].track_time += delta;
    chain[to].mix_time += delta;
    false
}

/// rig-c's `applyMixingFrom`: the entry the one at `to` fades from, after
/// the entries it fades from in turn; returns `to`'s mix.
fn apply_mixing_from(rig: &Rig, chain: &mut [Entry], to: usize, pose: &mut Working) -> f64 {
    let from = to + 1;
    let from_mix = if from + 1 < chain.len() {
        apply_mixing_from(rig, chain, from, pose)
    } else {
        1.0
    };
    let mix = chain[to].mix();
    let a = chain[from].alpha * from_mix;
    let keep = 1.0 - mix * chain[to].alpha;
    let alpha_mix = a * (1.0 - mix);
    let alpha_hold = if keep > 0.0 { alpha_mix / keep } else { a };
    let hold_mixes: Vec<f64> = chain[from]
        .timeline_hold_mix
        .iter()
        .map(|h| h.map_or(f64::NAN, |h| chain[h].mix()))
        .collect();
    let entry = &mut chain[from];
    let tls = timelines(rig, entry);
    let count = tls.len();
    let add = entry.additive;
    let first_frame = !add && entry.timelines_rotation.len() != count * 2;
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
            } if !add => apply_rotate(
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
            // An entry fading out keeps no attachment, whatever its alpha.
            TimelineKind::Attachment { .. } => {
                apply_attachment(rig, pose, tl, animation_time, mix_from, false)
            }
            // An entry fading out fires no event; an additive one adds at
            // its faded alpha.
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
                add,
            ),
        }
    }
    entry.next_animation_last = animation_time;
    entry.next_track_last = entry.track_time;
    mix
}

/// rig-c's `computeHold` for the entry at `index` of track `track`'s chain.
/// An additive entry's additive timelines are never held, and a timeline
/// under an additive entry fading in is not held for it.
fn compute_hold(
    rig: &Rig,
    chain: &mut [Entry],
    index: usize,
    track: usize,
    owners: &mut HashMap<Property, usize>,
) {
    let entry = &chain[index];
    let tls = timelines(rig, entry);
    let to = index.checked_sub(1);
    let add = entry.additive;
    let to_adds = to.is_some_and(|t| chain[t].additive);
    let keep_hold = entry.keep_hold;
    let mut modes = std::mem::take(&mut chain[index].timeline_mode);
    modes.resize(tls.len(), 0);
    let mut hold_mix = vec![None; tls.len()];
    let animation_of = |e: &Entry| e.animation.map(|a| &rig.animations[a]);
    for (i, tl) in tls.iter().enumerate() {
        let from = property_from(owners, track, &tl.properties);
        let adds = addable(rig, tl);
        if add && adds {
            modes[i] = from;
            continue;
        }
        let mut mode = match to {
            Some(t)
                if !tl.instant
                    && !(to_adds && adds)
                    && has_any(animation_of(&chain[t]), &tl.properties) =>
            {
                // Find the next entry that doesn't overwrite this property:
                // its mix fades out the hold.
                for n in (0..t).rev() {
                    if (chain[n].additive && adds)
                        || !has_any(animation_of(&chain[n]), &tl.properties)
                    {
                        if chain[n].mix_duration > 0.0 {
                            hold_mix[i] = Some(n);
                        }
                        break;
                    }
                }
                from | HOLD
            }
            _ => from,
        };
        if keep_hold {
            mode = (mode & !HOLD) | (modes[i] & HOLD);
        }
        modes[i] = mode;
    }
    let entry = &mut chain[index];
    entry.timeline_mode = modes;
    entry.timeline_hold_mix = hold_mix;
}

fn before_first_key(from: u8, alpha: f64, current: f64, setup: f64) -> f64 {
    match from {
        SETUP => setup,
        FIRST => current + (setup - current) * alpha,
        _ => current,
    }
}

/// rig-c's `scaleValue`: past the first key, `value` the curve times the
/// setup scale; added, its offset from the setup scale is added to the base.
fn scale_value(
    value: f64,
    alpha: f64,
    from: u8,
    out: bool,
    add: bool,
    current: f64,
    setup: f64,
) -> f64 {
    if alpha == 1.0 && !add {
        return value;
    }
    let mut base = if from == SETUP { setup } else { current };
    if add {
        return base + (value - setup) * alpha;
    }
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
    add: bool,
) {
    // Only the timelines the runtime adds read an additive entry's `add`.
    let add = add && addable(rig, tl);
    match &tl.kind {
        TimelineKind::Bone { .. } | TimelineKind::Inherit { .. } => {
            apply_bone(rig, pose, tl, time, alpha, from, out, add)
        }
        TimelineKind::Colour { .. } => apply_colour(rig, pose, tl, time, alpha, from),
        TimelineKind::Deform { .. } => apply_deform(rig, pose, tl, time, alpha, from, add),
        TimelineKind::Constraint { .. } => {
            apply_constraint(rig, pose, tl, time, alpha, from, out, add)
        }
        TimelineKind::Physics { .. } => apply_physics(rig, pose, tl, time, alpha, from, add),
        // A reset key is crossed as an event is fired: by the playing clip.
        TimelineKind::PhysicsReset { constraint, times } => {
            if fire && crossed(times.iter().copied(), last, time) {
                pose.resets.push(*constraint);
            }
        }
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

#[allow(clippy::too_many_arguments)]
fn apply_bone(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    out: bool,
    add: bool,
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
        } else if add {
            current + value * alpha
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
        } else if add {
            (cx + v[0] * alpha, cy + v[1] * alpha)
        } else {
            (cx + (sx + v[0] - cx) * alpha, cy + (sy + v[1] - cy) * alpha)
        }
    };
    let scale = |current: f64, s: f64, value: f64| -> f64 {
        if !keyed {
            before_first_key(from, alpha, current, s)
        } else {
            scale_value(value * s, alpha, from, out, add, current, s)
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
                    add,
                    local.scale_x,
                    setup.scale_x,
                );
                local.scale_y = scale_value(
                    v[1] * setup.scale_y,
                    alpha,
                    from,
                    out,
                    add,
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
        apply_bone(rig, pose, tl, time, 1.0, from, false, false);
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

fn apply_deform(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    add: bool,
) {
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
        // Added only over a deform already there: over none it is the setup
        // pose's branch, additive or not.
        let from_setup = mix_from == SETUP;
        if alpha == 1.0 {
            match s.deform.as_mut() {
                Some(current) if add && !from_setup => {
                    if weighted {
                        current.iter_mut().zip(&target).for_each(|(c, v)| *c += v);
                    } else {
                        for ((c, v), setup) in current.iter_mut().zip(&target).zip(&mesh.setup) {
                            *c += v - setup;
                        }
                    }
                }
                _ => s.deform = Some(target.clone()),
            }
        } else if from_setup {
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
            if !add {
                for (c, v) in current.iter_mut().zip(&target) {
                    *c += (v - *c) * alpha;
                }
            } else if weighted {
                for (c, v) in current.iter_mut().zip(&target) {
                    *c += v * alpha;
                }
            } else {
                for ((c, v), setup) in current.iter_mut().zip(&target).zip(&mesh.setup) {
                    *c += (v - setup) * alpha;
                }
            }
        }
    }
    pose.target = target;
    pose.scratch = scratch;
}

/// Whether a step from clip time `last` to `time` crosses a key: one in
/// `(last, time]`, or across a wrap (`time` below `last`) one after `last`
/// or at or before `time`.
fn crossed(mut keys: impl Iterator<Item = f64>, last: f64, time: f64) -> bool {
    if time < last {
        keys.any(|k| k > last || k <= time)
    } else {
        keys.any(|k| k > last && k <= time)
    }
}

/// A physics value timeline, on each constraint it writes: from setup (or
/// from what the clips under it left) toward the keyed value by `alpha`. At
/// full alpha from setup this is rig-c's `setup + (value - setup)`; a mass
/// blends as a mass and is held as its inverse.
/// Under an additive clip, wind and gravity add the keyed value (times
/// `alpha`) to what the clips under it left, as rig-c's `ADDITIVE_MODE`
/// says the runtime does.
fn apply_physics(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    add: bool,
) {
    let TimelineKind::Physics {
        targets,
        value,
        keys,
    } = &tl.kind
    else {
        return;
    };
    let index = keys.index_at(time);
    let keyed = index.map(|i| keys.channel(i, 0, time));
    let mass = *value == MASS_INVERSE;
    for &c in targets {
        let setup = rig.constraints[c].setup.values[*value];
        let current = &mut pose.constraints[c].values[*value];
        let Some(keyed) = keyed else {
            *current = before_first_key(from, alpha, *current, setup);
            continue;
        };
        let base = if from == SETUP { setup } else { *current };
        *current = if add && from != SETUP {
            base + keyed * alpha
        } else if mass {
            let base = 1.0 / base;
            1.0 / (base + (keyed - base) * alpha)
        } else {
            base + (keyed - base) * alpha
        };
    }
}

/// A constraint timeline: its values from setup (or from what the clips
/// under it left) toward the keyed ones by `alpha`. At full alpha from setup
/// this is rig-c's `setup + (value - setup) * 1`; the stepped ik flags are
/// the key's, and a clip fading out leaves them. Under an additive clip a
/// transform's mixes add the keyed values (times `alpha`) to what the clips
/// under it left, as rig-c's `ADDITIVE_MODE` says the runtime does.
#[allow(clippy::too_many_arguments)]
fn apply_constraint(
    rig: &Rig,
    pose: &mut Working,
    tl: &Timeline,
    time: f64,
    alpha: f64,
    from: u8,
    out: bool,
    add: bool,
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
    let mut v = [0.0; 7];
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
        current.values[k] = if add && from != SETUP {
            base + value * alpha
        } else {
            base + (value - base) * alpha
        };
    }
    if let Some(f) = flags.get(index) {
        if !out {
            current.flags = *f;
        } else if from == SETUP {
            current.flags = setup.flags;
        }
    }
}
