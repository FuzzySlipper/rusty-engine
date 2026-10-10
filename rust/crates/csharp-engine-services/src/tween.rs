//! Engine-played presentation tweens over published appearance objects.
//!
//! Tweens live in the appearance state, so a refused call drops their
//! changes with the rest. Each update advances them by their clock before
//! the product runs; the end of every call writes what each tweened object
//! shows (its published transform and colour under the tweens' offset) as
//! ordinary render updates after the call's own publishes.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_void;

use csharp_engine_abi::*;
use render_model::{Material, RenderDiff, RenderFrameDiff, RenderHandle, Transform};
use render_presentation::{
    EaseFamily, EaseMode, Easing, TweenChannel, TweenClock, TweenDefinition, TweenLayer,
    TweenMarker, TweenOffset, TweenPlayback, TweenRepeat, TweenSegment, TweenShape, TweenState,
};
use render_projection::Appearance;

use crate::appearance::{RuntimeAppearanceBridge, RuntimeAppearanceCall, RuntimeAppearanceData};
use crate::composition::{borrowed_slice, CsharpEngineServicesError, ABI_OK};

#[derive(Clone, Default)]
pub(crate) struct RuntimeTweens {
    /// By handle, which is also start order.
    tweens: BTreeMap<u64, RuntimeTween>,
    /// Each object's tweens, in start order.
    by_object: BTreeMap<u64, Vec<u64>>,
    next_tween: u64,
    /// Objects whose shown values may have changed since the last write.
    dirty: BTreeSet<u64>,
    /// What each tweened object last showed.
    shown: BTreeMap<u64, ShownObject>,
    /// The latest update's events.
    events: Vec<NativeTweenEvent>,
    /// Raised since the latest update; reported by the next one.
    pending_events: Vec<NativeTweenEvent>,
}

#[derive(Clone)]
struct RuntimeTween {
    object_id: u64,
    playback: TweenPlayback,
}

#[derive(Clone, Copy)]
struct ShownObject {
    /// The published transform the offset was composed over.
    published: Transform,
    transform: Transform,
    /// The colour shown, when a tween changed it.
    color: Option<[f32; 4]>,
}

impl RuntimeTweens {
    fn object_tweens(&self, object_id: u64) -> impl Iterator<Item = (&u64, &RuntimeTween)> {
        self.by_object
            .get(&object_id)
            .into_iter()
            .flatten()
            .map(|handle| (handle, &self.tweens[handle]))
    }

    fn insert(&mut self, handle: u64, tween: RuntimeTween) {
        self.by_object
            .entry(tween.object_id)
            .or_default()
            .push(handle);
        self.dirty.insert(tween.object_id);
        self.tweens.insert(handle, tween);
    }

    fn remove(&mut self, handle: u64) {
        let Some(tween) = self.tweens.remove(&handle) else {
            return;
        };
        self.dirty.insert(tween.object_id);
        if let Some(handles) = self.by_object.get_mut(&tween.object_id) {
            handles.retain(|other| *other != handle);
            if handles.is_empty() {
                self.by_object.remove(&tween.object_id);
            }
        }
    }

    fn object_offset(&self, object_id: u64) -> TweenOffset {
        self.object_tweens(object_id)
            .fold(TweenOffset::IDENTITY, |offset, (_, tween)| {
                offset.then(&tween.playback.offset())
            })
    }

    fn end_object_tweens(&mut self, object_id: u64) {
        for handle in self.by_object.remove(&object_id).unwrap_or_default() {
            self.tweens.remove(&handle);
        }
        self.dirty.insert(object_id);
    }
}

/// What tweens advance by beyond an update's admitted steps, so they move at
/// the presentation cadence: between steps the world time owed toward the
/// next one, and the host time since they last advanced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TweenTime {
    /// World seconds owed toward the next fixed step (the lifecycle's
    /// remainder at the gameplay rate); `None` keeps the last.
    pub owed_world_seconds: Option<f64>,
    pub host_seconds: f64,
}

impl TweenTime {
    /// Steps only: the update's host seconds and no owed world time.
    pub fn of_update(facts: &NativeProductUpdateFacts) -> Self {
        Self {
            owed_world_seconds: None,
            host_seconds: facts.host_elapsed_seconds,
        }
    }
}

/// Advances every playing tween by `admitted_seconds` of steps plus the
/// clock's change in owed world time, and its host seconds. An update
/// reports the events raised since the last; a call without one keeps its
/// events for the next.
pub(crate) fn advance(
    state: &mut crate::appearance::RuntimeAppearanceState,
    owed: &mut f64,
    admitted_seconds: f64,
    clock: TweenTime,
    update: bool,
) {
    // The owed time is followed whether or not tweens play, so one started
    // between steps counts from where it started.
    let owed_before = *owed;
    if let Some(now) = clock.owed_world_seconds {
        *owed = now;
    }
    // Read first: a call with no tweens does not write the shared state.
    let idle = &state.tweens;
    if idle.tweens.is_empty() && idle.pending_events.is_empty() && idle.events.is_empty() {
        return;
    }
    let tweens = &mut state.tweens;
    if update {
        tweens.events = std::mem::take(&mut tweens.pending_events);
    }
    // A reset owed time (a new baseline) gives no time back.
    let world_seconds = (admitted_seconds + *owed - owed_before).max(0.0);
    let realtime_seconds = clock.host_seconds;
    let mut crossed = Vec::new();
    let mut raised = Vec::new();
    for (handle, tween) in &mut tweens.tweens {
        let before = tween.playback.elapsed_seconds();
        crossed.clear();
        let completed = tween
            .playback
            .advance(world_seconds, realtime_seconds, &mut crossed);
        for &(marker_id, iteration) in &crossed {
            raised.push(NativeTweenEvent {
                tween: NativeTweenHandle { value: *handle },
                object_id: tween.object_id,
                kind: NativeTweenEventKind::Marker,
                marker_id,
                iteration,
            });
        }
        if completed {
            raised.push(completed_event(*handle, tween));
        }
        if completed || tween.playback.elapsed_seconds() != before {
            tweens.dirty.insert(tween.object_id);
        }
    }
    if update {
        tweens.events.extend(raised);
    } else {
        tweens.pending_events.extend(raised);
    }
}

/// Whether any tween plays, so a host observation without an update still
/// has tweens to advance.
pub(crate) fn playing(state: &crate::appearance::RuntimeAppearanceState) -> bool {
    !state.tweens.tweens.is_empty()
}

fn completed_event(handle: u64, tween: &RuntimeTween) -> NativeTweenEvent {
    NativeTweenEvent {
        tween: NativeTweenHandle { value: handle },
        object_id: tween.object_id,
        kind: NativeTweenEventKind::Completed,
        marker_id: 0,
        iteration: tween.playback.iteration(),
    }
}

/// Ends completed tweens and tweens of removed objects, then writes what
/// each changed or republished tweened object shows.
pub(crate) fn settle(staged: &mut RuntimeAppearanceCall) -> Result<(), CsharpEngineServicesError> {
    let state = &mut staged.state;
    if state.tweens.dirty.is_empty() && state.tweens.shown.is_empty() {
        return Ok(());
    }
    let RuntimeAppearanceData {
        projector, tweens, ..
    } = &mut **state;
    let ended: Vec<u64> = tweens
        .tweens
        .iter()
        .filter(|(_, tween)| {
            tween.playback.state() == TweenState::Completed
                || projector.object_handle(tween.object_id).is_none()
        })
        .map(|(handle, _)| *handle)
        .collect();
    for handle in ended {
        tweens.remove(handle);
    }
    // A publish in this call may have written an object's own values over
    // what its tweens show.
    let handles: BTreeMap<RenderHandle, u64> = tweens
        .shown
        .keys()
        .chain(tweens.dirty.iter())
        .filter_map(|object| Some((projector.object_handle(*object)?, *object)))
        .collect();
    let mut rewritten = BTreeSet::new();
    for output in &staged.outputs {
        let crate::appearance::RuntimeAppearanceCallOutput::Frame(frame) = output else {
            continue;
        };
        for op in &frame.ops {
            let handle = match op {
                RenderDiff::Create { handle, .. } | RenderDiff::CreateSprite { handle, .. } => {
                    *handle
                }
                RenderDiff::Update {
                    handle,
                    transform,
                    material,
                    ..
                } if transform.is_some() || material.is_some() => *handle,
                RenderDiff::UpdateSprite {
                    handle,
                    tint: Some(_),
                    ..
                } => *handle,
                _ => continue,
            };
            if let Some(object) = handles.get(&handle) {
                rewritten.insert(*object);
            }
        }
    }
    let mut ops = Vec::new();
    let candidates: BTreeSet<u64> = std::mem::take(&mut tweens.dirty)
        .into_iter()
        .chain(rewritten.iter().copied())
        .collect();
    for object in candidates {
        let Some(handle) = projector.object_handle(object) else {
            tweens.shown.remove(&object);
            continue;
        };
        let (published, appearance) = projector
            .object_projection(object)
            .expect("an object with a handle is retained");
        let live = tweens.object_tweens(object).next().is_some();
        let previous = tweens.shown.get(&object).copied();
        if !live && previous.is_none() {
            continue;
        }
        let offset = tweens.object_offset(object);
        let transform = offset.apply(&published);
        let tinted = live
            && tweens
                .object_tweens(object)
                .any(|(_, tween)| tween.playback.definition().animates(TweenChannel::Tint));
        let base_color = appearance_color(appearance);
        let color = base_color
            .filter(|_| tinted)
            .map(|color| tinted_color(&offset, color));
        let republished = rewritten.contains(&object);
        let transform_changed =
            republished || previous.is_none_or(|shown| shown.transform != transform);
        let color_changed = (republished && color.is_some())
            || previous.map_or(color.is_some(), |shown| shown.color != color);
        // Ending a tint shows the appearance's own colour again.
        let color = if color_changed {
            color.or(base_color)
        } else {
            None
        };
        match appearance {
            Appearance::Sprite { .. } => {
                if transform_changed {
                    ops.push(transform_update(handle, transform, None));
                }
                if let Some(tint) = color {
                    ops.push(RenderDiff::UpdateSprite {
                        handle,
                        frame: None,
                        tint: Some(tint),
                        render_order: None,
                        visible: None,
                    });
                }
            }
            Appearance::Primitive { material, .. } => {
                let material = color.map(|color| Material { color, ..*material });
                if transform_changed || material.is_some() {
                    ops.push(transform_update(handle, transform, material));
                }
            }
            _ => {
                if transform_changed {
                    ops.push(transform_update(handle, transform, None));
                }
            }
        }
        if live {
            tweens.shown.insert(
                object,
                ShownObject {
                    published,
                    transform,
                    color: base_color
                        .filter(|_| tinted)
                        .map(|color| tinted_color(&offset, color)),
                },
            );
        } else {
            tweens.shown.remove(&object);
        }
    }
    if ops.is_empty() {
        return Ok(());
    }
    let frame = RenderFrameDiff::try_from_ops(ops).map_err(|error| {
        CsharpEngineServicesError::new(
            "CSHARP_TWEEN_FRAME",
            format!("tween frame is invalid: {error:?}"),
        )
    })?;
    crate::appearance::push_extra_frame(staged, frame);
    Ok(())
}

fn transform_update(
    handle: RenderHandle,
    transform: Transform,
    material: Option<Material>,
) -> RenderDiff {
    RenderDiff::Update {
        handle,
        transform: Some(transform),
        material,
        visible: None,
        metadata: None,
    }
}

/// `color` under the offset's tint, within the [0, 1] colours render: a
/// flash brightens up to white.
fn tinted_color(offset: &TweenOffset, color: [f32; 4]) -> [f32; 4] {
    offset.tint(color).map(|channel| channel.clamp(0.0, 1.0))
}

/// The colour a tint multiplies: a sprite's tint or a primitive's colour.
fn appearance_color(appearance: &Appearance) -> Option<[f32; 4]> {
    match appearance {
        Appearance::Sprite { sprite } => Some(sprite.tint),
        Appearance::Primitive { material, .. } => Some(material.color),
        _ => None,
    }
}

impl RuntimeAppearanceBridge {
    pub(crate) fn tween_start(
        &mut self,
        request: &NativeTweenStartRequest,
    ) -> Result<NativeTweenReadout, CsharpEngineServicesError> {
        let native_segments =
            unsafe { borrowed_slice(request.segments, request.segments_len, "tween segments") }?;
        let native_markers =
            unsafe { borrowed_slice(request.markers, request.markers_len, "tween markers") }?;
        let segments = native_segments.iter().map(segment).collect::<Vec<_>>();
        let markers = native_markers
            .iter()
            .map(|marker| TweenMarker {
                marker_id: marker.marker_id,
                time_seconds: marker.time_seconds,
            })
            .collect();
        let repeat = if request.forever {
            TweenRepeat::Forever
        } else {
            TweenRepeat::Count(request.iterations)
        };
        let mut definition = TweenDefinition::new(segments, markers, repeat, request.yoyo)
            .map_err(|error| {
                tween_error(
                    "CSHARP_TWEEN_DEFINITION",
                    format!("tween is invalid: {error:?}"),
                )
            })?;
        let elapsed = elapsed_seconds(request.elapsed_seconds)?;
        let staged = self.staged_mut()?;
        let RuntimeAppearanceData {
            projector, tweens, ..
        } = &mut *staged.state;
        let object_id = request.object_id;
        let (published, appearance) = projector.object_projection(object_id).ok_or_else(|| {
            tween_error(
                "CSHARP_TWEEN_OBJECT",
                format!("object {object_id} is not in the published scene"),
            )
        })?;
        if definition.animates(TweenChannel::Tint) && appearance_color(appearance).is_none() {
            return Err(tween_error(
                "CSHARP_TWEEN_TINT",
                format!("object {object_id} shows no colour a tint can change: tint sprites and primitives"),
            ));
        }
        match request.start {
            NativeTweenStart::Layer => {}
            NativeTweenStart::Replace => tweens.end_object_tweens(object_id),
            NativeTweenStart::FromPresented => {
                if let Some(shown) = tweens.shown.get(&object_id) {
                    let offset = tweens.object_offset(object_id);
                    let presented = offset.apply(&shown.published);
                    let carry = TweenOffset::between(&presented, &published, offset.tint);
                    definition.start_from(&carry);
                }
                tweens.end_object_tweens(object_id);
            }
        }
        tweens.next_tween += 1;
        let handle = tweens.next_tween;
        let clock = match request.clock {
            NativeTweenClock::World => TweenClock::World,
            NativeTweenClock::Realtime => TweenClock::Realtime,
        };
        let mut playback = TweenPlayback::new(definition, clock);
        playback.seek(elapsed);
        tweens.insert(
            handle,
            RuntimeTween {
                object_id,
                playback,
            },
        );
        Ok(readout(handle, &tweens.tweens[&handle]))
    }

    pub(crate) fn tween_control(
        &mut self,
        request: NativeTweenControlRequest,
    ) -> Result<NativeTweenReadout, CsharpEngineServicesError> {
        let tweens = &mut self.staged_mut()?.state.tweens;
        let handle = issued(tweens, request.tween)?;
        let Some(tween) = tweens.tweens.get_mut(&handle) else {
            return Ok(ended(handle));
        };
        let object_id = tween.object_id;
        match request.control {
            NativeTweenControl::Pause => tween.playback.pause(),
            NativeTweenControl::Resume => tween.playback.resume(),
            NativeTweenControl::Complete => {
                if tween.playback.state() != TweenState::Completed {
                    tween.playback.complete();
                    let event = completed_event(handle, tween);
                    tweens.pending_events.push(event);
                }
            }
            NativeTweenControl::Cancel => {
                tweens.remove(handle);
                return Ok(ended(handle));
            }
            NativeTweenControl::Seek => {
                tween
                    .playback
                    .seek(elapsed_seconds(request.elapsed_seconds)?);
            }
        }
        tweens.dirty.insert(object_id);
        Ok(readout(handle, &tweens.tweens[&handle]))
    }

    pub(crate) fn tween_read(
        &self,
        tween: NativeTweenHandle,
    ) -> Result<NativeTweenReadout, CsharpEngineServicesError> {
        let tweens = &self.staged_ref()?.state.tweens;
        let handle = issued(tweens, tween)?;
        Ok(tweens
            .tweens
            .get(&handle)
            .map_or_else(|| ended(handle), |tween| readout(handle, tween)))
    }

    pub(crate) fn tween_read_events(
        &mut self,
    ) -> Result<NativeTweenEventResult, CsharpEngineServicesError> {
        let events: Box<[NativeTweenEvent]> = self.staged_ref()?.state.tweens.events.clone().into();
        let result = NativeTweenEventResult {
            events: events.as_ptr(),
            events_len: events.len(),
        };
        self.borrowed.hold(events);
        Ok(result)
    }
}

impl RuntimeAppearanceBridge {
    pub(crate) fn tween_sample(
        &self,
        request: &NativeTweenSampleRequest,
    ) -> Result<NativeTweenSample, CsharpEngineServicesError> {
        let native_segments =
            unsafe { borrowed_slice(request.segments, request.segments_len, "tween segments") }?;
        let repeat = if request.forever {
            TweenRepeat::Forever
        } else {
            TweenRepeat::Count(request.iterations)
        };
        let definition = TweenDefinition::new(
            native_segments.iter().map(segment).collect(),
            Vec::new(),
            repeat,
            request.yoyo,
        )
        .map_err(|error| {
            tween_error(
                "CSHARP_TWEEN_DEFINITION",
                format!("tween is invalid: {error:?}"),
            )
        })?;
        // Where a tween playing the timeline would be.
        let mut playback = TweenPlayback::new(definition, TweenClock::World);
        playback.seek(elapsed_seconds(request.elapsed_seconds)?);
        let offset = playback.offset();
        let [x, y, z] = offset.translation;
        let [rx, ry, rz, rw] = offset.rotation;
        let [sx, sy, sz] = offset.scale;
        let [r, g, b, a] = offset.tint;
        Ok(NativeTweenSample {
            translation: NativeVec3 { x, y, z },
            rotation: NativeQuat {
                x: rx,
                y: ry,
                z: rz,
                w: rw,
            },
            scale: NativeVec3 {
                x: sx,
                y: sy,
                z: sz,
            },
            tint: NativeVec4 {
                x: r,
                y: g,
                z: b,
                w: a,
            },
        })
    }
}

/// The curve a tween segment with this easing plays.
pub(crate) fn evaluate_easing(request: NativeTweenEasingSampleRequest) -> NativeTweenEasingSample {
    NativeTweenEasingSample {
        value: easing(&request.easing).sample(request.progress),
    }
}

fn elapsed_seconds(seconds: f64) -> Result<f64, CsharpEngineServicesError> {
    if seconds.is_finite() && seconds >= 0.0 {
        Ok(seconds)
    } else {
        Err(tween_error(
            "CSHARP_TWEEN_DEFINITION",
            format!("elapsed seconds must be finite and not negative, not {seconds}"),
        ))
    }
}

fn issued(
    tweens: &RuntimeTweens,
    tween: NativeTweenHandle,
) -> Result<u64, CsharpEngineServicesError> {
    if tween.value == 0 || tween.value > tweens.next_tween {
        return Err(tween_error(
            "CSHARP_TWEEN_HANDLE",
            format!("tween {} was never started", tween.value),
        ));
    }
    Ok(tween.value)
}

fn readout(handle: u64, tween: &RuntimeTween) -> NativeTweenReadout {
    let definition = tween.playback.definition();
    NativeTweenReadout {
        tween: NativeTweenHandle { value: handle },
        object_id: tween.object_id,
        state: match tween.playback.state() {
            TweenState::Playing => NativeTweenState::Playing,
            TweenState::Paused => NativeTweenState::Paused,
            TweenState::Completed => NativeTweenState::Completed,
        },
        elapsed_seconds: tween.playback.elapsed_seconds(),
        iteration_seconds: f64::from(definition.iteration_seconds()),
        total_seconds: definition.total_seconds().unwrap_or(f64::INFINITY),
        iteration: tween.playback.iteration(),
    }
}

fn ended(handle: u64) -> NativeTweenReadout {
    NativeTweenReadout {
        tween: NativeTweenHandle { value: handle },
        object_id: 0,
        state: NativeTweenState::Ended,
        elapsed_seconds: 0.0,
        iteration_seconds: 0.0,
        total_seconds: 0.0,
        iteration: 0,
    }
}

fn segment(native: &NativeTweenSegment) -> TweenSegment {
    let vec4 = |value: NativeVec4| [value.x, value.y, value.z, value.w];
    TweenSegment {
        start_seconds: native.start_seconds,
        duration_seconds: native.duration_seconds,
        channel: match native.channel {
            NativeTweenChannel::Translation => TweenChannel::Translation,
            NativeTweenChannel::Rotation => TweenChannel::Rotation,
            NativeTweenChannel::Scale => TweenChannel::Scale,
            NativeTweenChannel::Tint => TweenChannel::Tint,
        },
        layer: match native.layer {
            NativeTweenLayer::Base => TweenLayer::Base,
            NativeTweenLayer::Additive => TweenLayer::Additive,
        },
        easing: easing(&native.easing),
        shape: match native.shape {
            NativeTweenShape::Tween => TweenShape::Tween {
                arc: [native.arc.x, native.arc.y, native.arc.z],
            },
            NativeTweenShape::Punch => TweenShape::Punch {
                frequency: native.frequency,
            },
            NativeTweenShape::Shake => TweenShape::Shake {
                frequency: native.frequency,
                seed: native.seed,
            },
            NativeTweenShape::Spline => TweenShape::Spline {
                before: vec4(native.before),
                after: vec4(native.after),
            },
        },
        from: vec4(native.from),
        to: vec4(native.to),
    }
}

fn easing(native: &NativeTweenEasing) -> Easing {
    use NativeTweenEasingKind as Kind;
    let family = match native.kind {
        Kind::Linear => return Easing::Linear,
        Kind::CubicBezier => {
            return Easing::CubicBezier([
                native.parameter_0,
                native.parameter_1,
                native.parameter_2,
                native.parameter_3,
            ]);
        }
        // Saturating: a negative or huge count holds one level or many.
        Kind::Steps => return Easing::Steps(native.parameter_0 as u32),
        Kind::Spring => {
            return Easing::Spring {
                stiffness: native.parameter_0,
                damping: native.parameter_1,
            };
        }
        Kind::QuadIn | Kind::QuadOut | Kind::QuadInOut => EaseFamily::Quad,
        Kind::CubicIn | Kind::CubicOut | Kind::CubicInOut => EaseFamily::Cubic,
        Kind::QuartIn | Kind::QuartOut | Kind::QuartInOut => EaseFamily::Quart,
        Kind::QuintIn | Kind::QuintOut | Kind::QuintInOut => EaseFamily::Quint,
        Kind::SineIn | Kind::SineOut | Kind::SineInOut => EaseFamily::Sine,
        Kind::ExpoIn | Kind::ExpoOut | Kind::ExpoInOut => EaseFamily::Expo,
        Kind::CircIn | Kind::CircOut | Kind::CircInOut => EaseFamily::Circ,
        Kind::BackIn | Kind::BackOut | Kind::BackInOut => EaseFamily::Back,
        Kind::ElasticIn | Kind::ElasticOut | Kind::ElasticInOut => EaseFamily::Elastic,
        Kind::BounceIn | Kind::BounceOut | Kind::BounceInOut => EaseFamily::Bounce,
    };
    // Each family's kinds run In, Out, InOut from QuadIn = 1.
    let mode = match (native.kind as u32 - 1) % 3 {
        0 => EaseMode::In,
        1 => EaseMode::Out,
        _ => EaseMode::InOut,
    };
    Easing::Ease(family, mode)
}

fn tween_error(code: &'static str, message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, message)
}

unsafe fn bridge<'call>(context: *mut c_void) -> Option<&'call mut RuntimeAppearanceBridge> {
    if context.is_null() {
        None
    } else {
        Some(unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() })
    }
}

fn respond<T>(
    context: *mut c_void,
    result: *mut T,
    call: impl FnOnce(&mut RuntimeAppearanceBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    let Some(bridge) = (unsafe { bridge(context) }) else {
        return 0;
    };
    match call(bridge) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_operation_error(error);
            0
        }
    }
}

unsafe extern "C" fn start(
    context: *mut c_void,
    request: *const NativeTweenStartRequest,
    result: *mut NativeTweenReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    crate::appearance::appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        respond(context, result, |bridge| {
            bridge.tween_start(unsafe { &*request })
        })
    })
}

unsafe extern "C" fn control(
    context: *mut c_void,
    request: NativeTweenControlRequest,
    result: *mut NativeTweenReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    crate::appearance::appearance_operation(context, operation_error, || {
        respond(context, result, |bridge| bridge.tween_control(request))
    })
}

unsafe extern "C" fn read(
    context: *mut c_void,
    tween: NativeTweenHandle,
    result: *mut NativeTweenReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    crate::appearance::appearance_operation(context, operation_error, || {
        respond(context, result, |bridge| bridge.tween_read(tween))
    })
}

unsafe extern "C" fn read_events(
    context: *mut c_void,
    result: *mut NativeTweenEventResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    crate::appearance::appearance_operation(context, operation_error, || {
        respond(context, result, RuntimeAppearanceBridge::tween_read_events)
    })
}

unsafe extern "C" fn evaluate_easing_operation(
    context: *mut c_void,
    request: NativeTweenEasingSampleRequest,
    result: *mut NativeTweenEasingSample,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    crate::appearance::appearance_operation(context, operation_error, || {
        respond(context, result, |_| Ok(evaluate_easing(request)))
    })
}

unsafe extern "C" fn sample(
    context: *mut c_void,
    request: *const NativeTweenSampleRequest,
    result: *mut NativeTweenSample,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    crate::appearance::appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        respond(context, result, |bridge| {
            bridge.tween_sample(unsafe { &*request })
        })
    })
}

pub(crate) fn api(bridge: &mut RuntimeAppearanceBridge) -> NativeTweenApi {
    NativeTweenApi {
        context: (bridge as *mut RuntimeAppearanceBridge).cast(),
        start,
        control,
        read,
        read_events,
        evaluate_easing: evaluate_easing_operation,
        sample,
    }
}
