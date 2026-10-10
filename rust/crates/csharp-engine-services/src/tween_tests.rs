use super::tests::primitive_request;
use super::*;

const OBJECT: u64 = 7;
const HOP_SECONDS: f32 = 0.5;
const STEP_SECONDS: f64 = 0.25;

fn vec3(x: f32, y: f32, z: f32) -> NativeVec3 {
    NativeVec3 { x, y, z }
}

fn vec4(x: f32, y: f32, z: f32, w: f32) -> NativeVec4 {
    NativeVec4 { x, y, z, w }
}

fn fact(appearance: NativeAppearanceHandle, x: f32) -> NativeAppearanceFact {
    NativeAppearanceFact {
        object_id: OBJECT,
        has_parent_object: false,
        parent_object_id: 0,
        transform: NativeTransform {
            translation: vec3(x, 0.0, 0.0),
            rotation: NativeQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: vec3(1.0, 1.0, 1.0),
        },
        appearance,
        visible: true,
        layer: NativeRenderLayer::Scene,
        shadow_casting: Default::default(),
    }
}

fn update(steps: u32, host_seconds: f64) -> NativeProductUpdateFacts {
    NativeProductUpdateFacts {
        admitted_step_count: steps,
        fixed_delta_seconds: STEP_SECONDS,
        host_elapsed_seconds: host_seconds,
        ..realtime_sprite_update(1, STEP_SECONDS)
    }
}

fn segment(channel: NativeTweenChannel, from: NativeVec4, to: NativeVec4) -> NativeTweenSegment {
    NativeTweenSegment {
        start_seconds: 0.0,
        duration_seconds: HOP_SECONDS,
        channel,
        layer: NativeTweenLayer::Base,
        shape: NativeTweenShape::Tween,
        easing: NativeTweenEasing {
            kind: NativeTweenEasingKind::Linear,
            parameter_0: 0.0,
            parameter_1: 0.0,
            parameter_2: 0.0,
            parameter_3: 0.0,
        },
        from,
        to,
        arc: vec3(0.0, 0.0, 0.0),
        frequency: 0.0,
        seed: 0,
        before: vec4(0.0, 0.0, 0.0, 0.0),
        after: vec4(0.0, 0.0, 0.0, 0.0),
    }
}

/// A hop from one unit behind the published position.
fn hop() -> NativeTweenSegment {
    NativeTweenSegment {
        arc: vec3(0.0, 1.0, 0.0),
        ..segment(
            NativeTweenChannel::Translation,
            vec4(-1.0, 0.0, 0.0, 0.0),
            vec4(0.0, 0.0, 0.0, 0.0),
        )
    }
}

fn request(
    segments: &[NativeTweenSegment],
    markers: &[NativeTweenMarker],
    clock: NativeTweenClock,
    start: NativeTweenStart,
) -> NativeTweenStartRequest {
    NativeTweenStartRequest {
        object_id: OBJECT,
        segments: segments.as_ptr(),
        segments_len: segments.len(),
        markers: markers.as_ptr(),
        markers_len: markers.len(),
        iterations: 1,
        forever: false,
        yoyo: false,
        clock,
        start,
        elapsed_seconds: 0.0,
    }
}

/// A bridge with a white cube published as `OBJECT` at x = 2.
fn published_cube() -> (RuntimeAppearanceBridge, NativeAppearanceHandle) {
    let mut bridge =
        RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
    bridge.begin_call();
    let cube = bridge.create_primitive(primitive_request()).unwrap();
    unsafe { bridge.stage_snapshot(&fact(cube, 2.0), 1) }.unwrap();
    bridge.end_call();
    (bridge, cube)
}

/// One render update's translation and colour, when it wrote them.
type Write = (Option<[f32; 3]>, Option<[f32; 4]>);

/// Ends the call; the transforms and materials it wrote, in order.
fn end(bridge: &mut RuntimeAppearanceBridge) -> Vec<Write> {
    let call = bridge.take_staged_call();
    assert!(call.release_error.is_none());
    let writes = call
        .render_ops()
        .into_iter()
        .filter_map(|op| match op {
            RenderDiff::Update {
                transform,
                material,
                ..
            } => Some((
                transform.map(|transform| transform.translation),
                material.map(|material| material.color),
            )),
            _ => None,
        })
        .collect();
    bridge.commit(call);
    writes
}

fn shown(bridge: &mut RuntimeAppearanceBridge) -> Option<[f32; 3]> {
    end(bridge)
        .into_iter()
        .rev()
        .find_map(|(translation, _)| translation)
}

fn events(bridge: &mut RuntimeAppearanceBridge) -> Vec<(NativeTweenEventKind, u64, u32)> {
    let result = bridge.tween_read_events().unwrap();
    unsafe { std::slice::from_raw_parts(result.events, result.events_len) }
        .iter()
        .map(|event| (event.kind, event.marker_id, event.iteration))
        .collect()
}

fn assert_near(actual: Option<[f32; 3]>, expected: [f32; 3]) {
    let actual = actual.expect("the call wrote a transform");
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() < 1.0e-5),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn a_hop_plays_from_its_start_without_republishing_and_reports_completion() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    let started = bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    assert_eq!(started.state, NativeTweenState::Playing);
    assert_eq!(started.total_seconds, f64::from(HOP_SECONDS));
    // It shows its start at the end of the call that starts it.
    assert_near(shown(&mut bridge), [1.0, 0.0, 0.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert!(events(&mut bridge).is_empty());
    assert_near(shown(&mut bridge), [1.5, 1.0, 0.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Completed, 0, 0)]
    );
    assert_eq!(
        bridge.tween_read(started.tween).unwrap().state,
        NativeTweenState::Completed
    );
    assert_near(shown(&mut bridge), [2.0, 0.0, 0.0]);
    // Ended: nothing more to write, and the events were that update's.
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert!(events(&mut bridge).is_empty());
    assert_eq!(
        bridge.tween_read(started.tween).unwrap().state,
        NativeTweenState::Ended
    );
    assert!(end(&mut bridge).is_empty());
    bridge.begin_call();
    assert_eq!(
        bridge
            .tween_read(NativeTweenHandle { value: 99 })
            .unwrap_err()
            .code(),
        "CSHARP_TWEEN_HANDLE"
    );
    bridge.end_call();
}

#[test]
fn a_held_world_holds_world_tweens_while_realtime_tweens_move() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    let world = bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    end(&mut bridge);
    // Held: no admitted steps, host time moves.
    bridge.begin_update_call(update(0, 0.1));
    assert!(end(&mut bridge).is_empty(), "nothing moved");
    bridge.begin_call();
    assert_eq!(bridge.tween_read(world.tween).unwrap().elapsed_seconds, 0.0);
    let realtime = bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::Realtime,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    assert_eq!(
        bridge.tween_read(world.tween).unwrap().state,
        NativeTweenState::Ended
    );
    end(&mut bridge);
    bridge.begin_update_call(update(0, 0.25));
    assert_near(shown(&mut bridge), [1.5, 1.0, 0.0]);
    bridge.begin_call();
    assert_eq!(
        bridge.tween_read(realtime.tween).unwrap().elapsed_seconds,
        0.25
    );
    bridge.end_call();
}

#[test]
fn a_republish_moves_what_the_tween_shows_over_and_a_new_start_continues_from_the_shown_pose() {
    let (mut bridge, cube) = published_cube();
    bridge.begin_call();
    bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    end(&mut bridge);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    // The product moves the object; the offset stays over the new position.
    unsafe { bridge.stage_snapshot(&fact(cube, 5.0), 1) }.unwrap();
    let writes = end(&mut bridge);
    assert_eq!(
        writes.first().unwrap().0,
        Some([5.0, 0.0, 0.0]),
        "the product's own write"
    );
    assert_near(writes.last().unwrap().0, [4.5, 1.0, 0.0]);
    // Mid-flight, the product moves it again and starts a new hop from
    // where it shows: the start replaces the hop's own `from`.
    bridge.begin_call();
    unsafe { bridge.stage_snapshot(&fact(cube, 8.0), 1) }.unwrap();
    bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::FromPresented,
        ))
        .unwrap();
    assert_near(shown(&mut bridge), [4.5, 1.0, 0.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    // Half way from (4.5, 1) to 8, plus the arc's peak.
    assert_near(shown(&mut bridge), [6.25, 1.5, 0.0]);
}

#[test]
fn layered_tweens_compose_and_controls_pause_complete_and_cancel() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    let hop = bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    let settle = segment(
        NativeTweenChannel::Translation,
        vec4(0.0, 0.0, 4.0, 0.0),
        vec4(0.0, 0.0, 0.0, 0.0),
    );
    let layered = bridge
        .tween_start(&request(
            &[settle],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Layer,
        ))
        .unwrap();
    assert_near(shown(&mut bridge), [1.0, 0.0, 4.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_near(shown(&mut bridge), [1.5, 1.0, 2.0]);
    bridge.begin_call();
    let paused = bridge
        .tween_control(NativeTweenControlRequest {
            tween: hop.tween,
            control: NativeTweenControl::Pause,
            elapsed_seconds: 0.0,
        })
        .unwrap();
    assert_eq!(paused.state, NativeTweenState::Paused);
    end(&mut bridge);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    // The hop holds half way; the layered tween completes and ends.
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Completed, 0, 0)]
    );
    assert_near(shown(&mut bridge), [1.5, 1.0, 0.0]);
    bridge.begin_call();
    // Skipping to the end ends it with this call and reports it next update.
    let skipped = bridge
        .tween_control(NativeTweenControlRequest {
            tween: hop.tween,
            control: NativeTweenControl::Complete,
            elapsed_seconds: 0.0,
        })
        .unwrap();
    assert_eq!(skipped.state, NativeTweenState::Completed);
    assert_near(shown(&mut bridge), [2.0, 0.0, 0.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Completed, 0, 0)]
    );
    assert_eq!(
        bridge.tween_read(layered.tween).unwrap().state,
        NativeTweenState::Ended
    );
    end(&mut bridge);
    // Cancel shows the published values at once and reports nothing.
    bridge.begin_call();
    let again = bridge
        .tween_start(&request(
            &[self::hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    end(&mut bridge);
    bridge.begin_call();
    let cancelled = bridge
        .tween_control(NativeTweenControlRequest {
            tween: again.tween,
            control: NativeTweenControl::Cancel,
            elapsed_seconds: 0.0,
        })
        .unwrap();
    assert_eq!(cancelled.state, NativeTweenState::Ended);
    assert_near(shown(&mut bridge), [2.0, 0.0, 0.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert!(events(&mut bridge).is_empty());
    bridge.end_call();
}

#[test]
fn tint_multiplies_a_primitive_colour_and_ending_restores_it() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    let fade = segment(
        NativeTweenChannel::Tint,
        vec4(1.0, 0.5, 0.5, 0.0),
        vec4(1.0, 1.0, 1.0, 1.0),
    );
    let markers = [NativeTweenMarker {
        marker_id: 3,
        time_seconds: 0.25,
    }];
    let started = bridge
        .tween_start(&NativeTweenStartRequest {
            iterations: 2,
            yoyo: true,
            ..request(
                &[fade],
                &markers,
                NativeTweenClock::World,
                NativeTweenStart::Replace,
            )
        })
        .unwrap();
    assert_eq!(started.total_seconds, 1.0);
    let colors = |writes: Vec<Write>| writes.into_iter().rev().find_map(|(_, color)| color);
    assert_eq!(colors(end(&mut bridge)), Some([1.0, 0.5, 0.5, 0.0]));
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Marker, 3, 0)]
    );
    assert_eq!(colors(end(&mut bridge)), Some([1.0, 0.75, 0.75, 0.5]));
    bridge.begin_update_call(update(1, STEP_SECONDS));
    // The first iteration's end; the second plays backwards from there.
    assert!(events(&mut bridge).is_empty());
    assert_eq!(colors(end(&mut bridge)), Some([1.0, 1.0, 1.0, 1.0]));
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Marker, 3, 1)]
    );
    assert_eq!(colors(end(&mut bridge)), Some([1.0, 0.75, 0.75, 0.5]));
    bridge.begin_update_call(update(4, STEP_SECONDS));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Completed, 0, 1)]
    );
    // Ended: the cube's own white again.
    assert_eq!(colors(end(&mut bridge)), Some([1.0, 1.0, 1.0, 1.0]));
    // A flash brightens up to the colours that render.
    bridge.begin_call();
    let flash = segment(
        NativeTweenChannel::Tint,
        vec4(2.5, 0.5, 2.5, 1.0),
        vec4(1.0, 1.0, 1.0, 1.0),
    );
    bridge
        .tween_start(&request(
            &[flash],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Layer,
        ))
        .unwrap();
    assert_eq!(colors(end(&mut bridge)), Some([1.0, 0.5, 1.0, 1.0]));
}

#[test]
fn a_tween_needs_a_published_object_and_ends_when_it_is_removed() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    let elsewhere = NativeTweenStartRequest {
        object_id: 99,
        ..request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        )
    };
    assert_eq!(
        bridge.tween_start(&elsewhere).unwrap_err().code(),
        "CSHARP_TWEEN_OBJECT"
    );
    let invalid = NativeTweenSegment {
        duration_seconds: -1.0,
        ..hop()
    };
    assert_eq!(
        bridge
            .tween_start(&request(
                &[invalid],
                &[],
                NativeTweenClock::World,
                NativeTweenStart::Replace
            ))
            .unwrap_err()
            .code(),
        "CSHARP_TWEEN_DEFINITION"
    );
    let started = bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    end(&mut bridge);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
    end(&mut bridge);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert!(events(&mut bridge).is_empty(), "removal reports nothing");
    assert_eq!(
        bridge.tween_read(started.tween).unwrap().state,
        NativeTweenState::Ended
    );
    assert!(end(&mut bridge).is_empty());
}

/// The time an observation between steps gives tweens.
fn between_steps(owed_world_seconds: f64, host_seconds: f64) -> crate::tween::TweenTime {
    crate::tween::TweenTime {
        owed_world_seconds: Some(owed_world_seconds),
        host_seconds,
    }
}

#[test]
fn world_tweens_move_between_steps_by_the_world_time_owed() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    bridge
        .tween_start(&request(
            &[hop()],
            &[NativeTweenMarker {
                marker_id: 5,
                time_seconds: 0.1,
            }],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    end(&mut bridge);
    // Observations between steps: no update, the Engine's own tween call.
    // A tenth of a second is owed toward the 0.25 s step.
    bridge.begin_call_with_update(None, Some(between_steps(0.1, 0.1)));
    let first = shown(&mut bridge).expect("the tween moved between steps");
    assert!((first[0] - 1.2).abs() < 1.0e-5, "{first:?}");
    // Held: the owed time stays, so nothing moves, however much host time.
    bridge.begin_call_with_update(None, Some(between_steps(0.1, 0.5)));
    assert!(end(&mut bridge).is_empty(), "a held world holds the tween");
    // The step arrives: 0.25 s of steps, nothing owed after it. The marker
    // crossed in the Engine's call is reported by this update.
    let step = update(1, 0.15);
    bridge.begin_call_with_update(Some(step), Some(between_steps(0.0, 0.15)));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Marker, 5, 0)]
    );
    assert_near(shown(&mut bridge), [1.5, 1.0, 0.0]);
    // The next step completes it.
    bridge.begin_call_with_update(Some(step), Some(between_steps(0.0, 0.25)));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Completed, 0, 0)]
    );
    assert_near(shown(&mut bridge), [2.0, 0.0, 0.0]);
}

fn control(
    bridge: &mut RuntimeAppearanceBridge,
    tween: NativeTweenHandle,
    control: NativeTweenControl,
    elapsed_seconds: f64,
) -> Result<NativeTweenReadout, CsharpEngineServicesError> {
    bridge.tween_control(NativeTweenControlRequest {
        tween,
        control,
        elapsed_seconds,
    })
}

#[test]
fn a_tween_started_part_way_shows_what_one_advanced_that_far_shows() {
    let markers = [
        NativeTweenMarker {
            marker_id: 1,
            time_seconds: 0.1,
        },
        NativeTweenMarker {
            marker_id: 2,
            time_seconds: 0.4,
        },
    ];
    let (mut from_zero, _) = published_cube();
    from_zero.begin_call();
    from_zero
        .tween_start(&request(
            &[hop()],
            &markers,
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap();
    end(&mut from_zero);
    from_zero.begin_update_call(update(1, STEP_SECONDS));
    let advanced = shown(&mut from_zero);

    let (mut part_way, _) = published_cube();
    part_way.begin_call();
    let started = part_way
        .tween_start(&NativeTweenStartRequest {
            elapsed_seconds: STEP_SECONDS,
            ..request(
                &[hop()],
                &markers,
                NativeTweenClock::World,
                NativeTweenStart::Replace,
            )
        })
        .unwrap();
    assert_eq!(started.elapsed_seconds, STEP_SECONDS);
    assert_eq!(shown(&mut part_way), advanced);
    assert_near(advanced, [1.5, 1.0, 0.0]);
    // Markers before the start are not reported; later ones are.
    part_way.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(
        events(&mut part_way),
        vec![
            (NativeTweenEventKind::Marker, 2, 0),
            (NativeTweenEventKind::Completed, 0, 0)
        ]
    );
    part_way.end_call();

    part_way.begin_call();
    for refused in [-1.0, f64::NAN, f64::INFINITY] {
        let error = part_way
            .tween_start(&NativeTweenStartRequest {
                elapsed_seconds: refused,
                ..request(
                    &[hop()],
                    &[],
                    NativeTweenClock::World,
                    NativeTweenStart::Replace,
                )
            })
            .unwrap_err();
        assert_eq!(error.code(), "CSHARP_TWEEN_DEFINITION");
    }
    part_way.end_call();
}

#[test]
fn a_paused_tween_sought_shows_that_pose_and_stays_paused() {
    let (mut bridge, _) = published_cube();
    bridge.begin_call();
    let hop = bridge
        .tween_start(&request(
            &[hop()],
            &[],
            NativeTweenClock::World,
            NativeTweenStart::Replace,
        ))
        .unwrap()
        .tween;
    control(&mut bridge, hop, NativeTweenControl::Pause, 0.0).unwrap();
    let sought = control(&mut bridge, hop, NativeTweenControl::Seek, STEP_SECONDS).unwrap();
    assert_eq!(
        (sought.state, sought.elapsed_seconds),
        (NativeTweenState::Paused, STEP_SECONDS)
    );
    // The call that seeks shows the pose.
    assert_near(shown(&mut bridge), [1.5, 1.0, 0.0]);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(shown(&mut bridge), None, "paused: nothing moves");
    // Jog back and step forward while paused.
    bridge.begin_call();
    control(&mut bridge, hop, NativeTweenControl::Seek, 0.0).unwrap();
    assert_near(shown(&mut bridge), [1.0, 0.0, 0.0]);
    bridge.begin_call();
    let past_end = control(&mut bridge, hop, NativeTweenControl::Seek, 9.0).unwrap();
    assert_eq!(past_end.elapsed_seconds, f64::from(HOP_SECONDS));
    assert_eq!(past_end.state, NativeTweenState::Paused);
    assert_near(shown(&mut bridge), [2.0, 0.0, 0.0]);
    bridge.begin_call();
    assert_eq!(
        control(&mut bridge, hop, NativeTweenControl::Seek, -0.5)
            .unwrap_err()
            .code(),
        "CSHARP_TWEEN_DEFINITION"
    );
    // Resumed at its end, it completes at the next update.
    control(&mut bridge, hop, NativeTweenControl::Resume, 0.0).unwrap();
    end(&mut bridge);
    bridge.begin_update_call(update(1, STEP_SECONDS));
    assert_eq!(
        events(&mut bridge),
        vec![(NativeTweenEventKind::Completed, 0, 0)]
    );
    bridge.end_call();
}

fn easing_of(kind: NativeTweenEasingKind) -> NativeTweenEasing {
    let [parameter_0, parameter_1, parameter_2, parameter_3] = match kind {
        NativeTweenEasingKind::CubicBezier => [0.3, -0.4, 0.6, 1.5],
        NativeTweenEasingKind::Steps => [4.0, 0.0, 0.0, 0.0],
        NativeTweenEasingKind::Spring => [120.0, 6.0, 0.0, 0.0],
        _ => [0.0; 4],
    };
    NativeTweenEasing {
        kind,
        parameter_0,
        parameter_1,
        parameter_2,
        parameter_3,
    }
}

#[test]
fn evaluated_easings_and_sampled_timelines_match_what_tweens_show() {
    use NativeTweenEasingKind as Kind;
    let kinds = [
        Kind::Linear,
        Kind::QuadIn,
        Kind::QuadOut,
        Kind::QuadInOut,
        Kind::CubicIn,
        Kind::CubicOut,
        Kind::CubicInOut,
        Kind::QuartIn,
        Kind::QuartOut,
        Kind::QuartInOut,
        Kind::QuintIn,
        Kind::QuintOut,
        Kind::QuintInOut,
        Kind::SineIn,
        Kind::SineOut,
        Kind::SineInOut,
        Kind::ExpoIn,
        Kind::ExpoOut,
        Kind::ExpoInOut,
        Kind::CircIn,
        Kind::CircOut,
        Kind::CircInOut,
        Kind::BackIn,
        Kind::BackOut,
        Kind::BackInOut,
        Kind::ElasticIn,
        Kind::ElasticOut,
        Kind::ElasticInOut,
        Kind::BounceIn,
        Kind::BounceOut,
        Kind::BounceInOut,
        Kind::CubicBezier,
        Kind::Steps,
        Kind::Spring,
    ];
    let (mut bridge, _) = published_cube();
    // A call writes only a changed pose.
    let mut last = [2.0, 0.0, 0.0];
    for kind in kinds {
        // A unit move over one second, so the eased progress is the offset.
        let segments = [NativeTweenSegment {
            duration_seconds: 1.0,
            easing: easing_of(kind),
            ..segment(
                NativeTweenChannel::Translation,
                vec4(0.0, 0.0, 0.0, 0.0),
                vec4(1.0, 0.0, 0.0, 0.0),
            )
        }];
        for progress in [0.0_f32, 0.1, 0.25, 0.5, 0.7, 0.9, 1.0] {
            bridge.begin_call();
            bridge
                .tween_start(&NativeTweenStartRequest {
                    elapsed_seconds: f64::from(progress),
                    ..request(
                        &segments,
                        &[],
                        NativeTweenClock::World,
                        NativeTweenStart::Replace,
                    )
                })
                .unwrap();
            let evaluated = crate::tween::evaluate_easing(NativeTweenEasingSampleRequest {
                easing: easing_of(kind),
                progress,
            })
            .value;
            let sampled = bridge
                .tween_sample(&NativeTweenSampleRequest {
                    segments: segments.as_ptr(),
                    segments_len: segments.len(),
                    iterations: 1,
                    forever: false,
                    yoyo: false,
                    elapsed_seconds: f64::from(progress),
                })
                .unwrap();
            let shown = shown(&mut bridge).unwrap_or(last);
            last = shown;
            // The shown pose is the sample over the published x = 2.
            assert!(
                (shown[0] - 2.0 - sampled.translation.x).abs() < 1.0e-5,
                "{kind:?} at {progress}"
            );
            assert!(
                (evaluated - sampled.translation.x).abs() < 1.0e-6,
                "{kind:?} at {progress}: evaluated {evaluated}, shown {}",
                sampled.translation.x
            );
        }
    }
    // A sample holds every channel: unit scale and white where none plays.
    bridge.begin_call();
    let sampled = bridge
        .tween_sample(&NativeTweenSampleRequest {
            segments: [hop()].as_ptr(),
            segments_len: 1,
            iterations: 1,
            forever: false,
            yoyo: false,
            elapsed_seconds: STEP_SECONDS,
        })
        .unwrap();
    assert_eq!(
        [
            sampled.translation.x,
            sampled.translation.y,
            sampled.scale.y,
            sampled.tint.w,
            sampled.rotation.w
        ],
        [-0.5, 1.0, 1.0, 1.0, 1.0]
    );
    bridge.end_call();
}
