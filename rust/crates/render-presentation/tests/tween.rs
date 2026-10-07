use render_model::Transform;
use render_presentation::*;

const EPSILON: f32 = 1.0e-4;

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < EPSILON
}

fn assert_vec(actual: &[f32], expected: &[f32]) {
    assert!(
        actual.iter().zip(expected).all(|(a, e)| close(*a, *e)),
        "{actual:?} != {expected:?}"
    );
}

fn segment(
    channel: TweenChannel,
    start_seconds: f32,
    duration_seconds: f32,
    from: [f32; 4],
    to: [f32; 4],
) -> TweenSegment {
    TweenSegment {
        start_seconds,
        duration_seconds,
        channel,
        layer: TweenLayer::Base,
        easing: Easing::Linear,
        shape: TweenShape::Tween { arc: [0.0; 3] },
        from,
        to,
    }
}

fn definition(segments: Vec<TweenSegment>) -> TweenDefinition {
    TweenDefinition::new(segments, Vec::new(), TweenRepeat::Count(1), false).unwrap()
}

fn hop() -> TweenSegment {
    TweenSegment {
        shape: TweenShape::Tween {
            arc: [0.0, 0.5, 0.0],
        },
        ..segment(
            TweenChannel::Translation,
            0.0,
            0.4,
            [-1.0, 0.0, 0.0, 0.0],
            [0.0; 4],
        )
    }
}

#[test]
fn a_hop_travels_to_identity_along_a_parabolic_arc() {
    let tween = definition(vec![hop()]);
    assert_eq!(tween.total_seconds(), Some(f64::from(0.4_f32)));
    assert_vec(&tween.sample(0.0).translation, &[-1.0, 0.0, 0.0]);
    assert_vec(&tween.sample(0.1).translation, &[-0.75, 0.375, 0.0]);
    assert_vec(&tween.sample(0.2).translation, &[-0.5, 0.5, 0.0]);
    assert_vec(&tween.sample(0.4).translation, &[0.0, 0.0, 0.0]);
    // Past the end it holds the end.
    assert_eq!(tween.sample(9.0), TweenOffset::IDENTITY);
    // Other channels stay identity.
    let middle = tween.sample(0.2);
    assert_eq!(
        (middle.rotation, middle.scale, middle.tint),
        (
            TweenOffset::IDENTITY.rotation,
            TweenOffset::IDENTITY.scale,
            TweenOffset::IDENTITY.tint
        )
    );
}

#[test]
fn a_base_track_shows_its_latest_started_segment_and_holds_between_them() {
    // Lift, then (after a gap) land with a squash.
    let lift = segment(
        TweenChannel::Scale,
        0.0,
        0.1,
        [1.0; 4],
        [0.9, 1.1, 1.0, 1.0],
    );
    let squash = segment(
        TweenChannel::Scale,
        0.3,
        0.2,
        [1.3, 0.7, 1.0, 1.0],
        [1.0; 4],
    );
    let delayed = segment(TweenChannel::Tint, 0.2, 0.1, [1.0, 1.0, 1.0, 0.0], [1.0; 4]);
    let tween = definition(vec![squash, delayed, lift]);
    assert_eq!(tween.iteration_seconds(), 0.5);
    assert_vec(&tween.sample(0.05).scale, &[0.95, 1.05, 1.0]);
    // Held at the lift's end until the squash starts.
    assert_vec(&tween.sample(0.2).scale, &[0.9, 1.1, 1.0]);
    assert_vec(&tween.sample(0.3).scale, &[1.3, 0.7, 1.0]);
    assert_vec(&tween.sample(0.4).scale, &[1.15, 0.85, 1.0]);
    // Before its first segment starts a track shows that segment's start.
    assert_vec(&tween.sample(0.0).tint, &[1.0, 1.0, 1.0, 0.0]);
    assert_vec(&tween.sample(0.25).tint, &[1.0, 1.0, 1.0, 0.5]);
}

#[test]
fn additive_segments_layer_over_the_base_from_their_start() {
    let punch = TweenSegment {
        layer: TweenLayer::Additive,
        shape: TweenShape::Punch { frequency: 1.0 },
        ..segment(
            TweenChannel::Scale,
            0.4,
            0.2,
            [1.0; 4],
            [1.2, 1.2, 1.2, 1.0],
        )
    };
    let nudge = TweenSegment {
        layer: TweenLayer::Additive,
        ..segment(
            TweenChannel::Translation,
            0.1,
            0.1,
            [0.0; 4],
            [0.0, 0.0, 2.0, 0.0],
        )
    };
    let tween = definition(vec![hop(), punch, nudge]);
    // Before the punch starts it contributes nothing.
    assert_vec(&tween.sample(0.3).scale, &[1.0, 1.0, 1.0]);
    // A quarter in: sin(π/2) of the swing, 3/4 left of a linear fade.
    assert_vec(&tween.sample(0.45).scale, &[1.15, 1.15, 1.15]);
    // And back to identity at its end.
    assert_vec(&tween.sample(0.6).scale, &[1.0, 1.0, 1.0]);
    // The additive translation adds to the hop and holds its end.
    let hop_only = definition(vec![hop()]);
    let both = tween.sample(0.15).translation;
    let alone = hop_only.sample(0.15).translation;
    assert_vec(&both, &[alone[0], alone[1], alone[2] + 1.0]);
    assert_vec(&tween.sample(0.6).translation, &[0.0, 0.0, 2.0]);
}

#[test]
fn rotation_turns_the_short_way_and_overshoots_with_the_easing() {
    let quarter = [
        0.0,
        (std::f32::consts::FRAC_PI_4).sin(),
        0.0,
        (std::f32::consts::FRAC_PI_4).cos(),
    ];
    let tween = definition(vec![segment(
        TweenChannel::Rotation,
        0.0,
        1.0,
        [0.0, 0.0, 0.0, 1.0],
        quarter,
    )]);
    let eighth = (std::f32::consts::PI / 8.0).sin_cos();
    assert_vec(&tween.sample(0.5).rotation, &[0.0, eighth.0, 0.0, eighth.1]);
    let overshoot = definition(vec![TweenSegment {
        easing: Easing::Ease(EaseFamily::Back, EaseMode::Out),
        ..segment(
            TweenChannel::Rotation,
            0.0,
            1.0,
            [0.0, 0.0, 0.0, 1.0],
            quarter,
        )
    }]);
    let peak = (0..=100)
        .map(|step| overshoot.sample(f64::from(step) / 100.0).rotation[1])
        .fold(f32::MIN, f32::max);
    assert!(
        peak > quarter[1] + 0.01,
        "back-out turns past the target: {peak}"
    );
    // Unnormalized input is normalized, a zero quaternion refused.
    let scaled = definition(vec![segment(
        TweenChannel::Rotation,
        0.0,
        1.0,
        [0.0, 0.0, 0.0, 3.0],
        quarter,
    )]);
    assert_vec(&scaled.sample(0.0).rotation, &[0.0, 0.0, 0.0, 1.0]);
    let error = TweenDefinition::new(
        vec![segment(TweenChannel::Rotation, 0.0, 1.0, [0.0; 4], quarter)],
        Vec::new(),
        TweenRepeat::Count(1),
        false,
    );
    assert_eq!(
        error,
        Err(TweenDefinitionError::InvalidValue { segment: 0 })
    );
}

#[test]
fn shakes_are_seeded_bounded_and_end_where_they_started() {
    let shake = |seed| {
        definition(vec![TweenSegment {
            shape: TweenShape::Shake {
                frequency: 20.0,
                seed,
            },
            ..segment(
                TweenChannel::Translation,
                0.0,
                0.5,
                [0.0; 4],
                [0.1, 0.2, 0.0, 0.0],
            )
        }])
    };
    let (a, b, c) = (shake(7), shake(7), shake(8));
    let mut differs = false;
    for step in 0..50 {
        let at = f64::from(step) / 100.0;
        let value = a.sample(at).translation;
        assert_eq!(value, b.sample(at).translation, "same seed, same shake");
        differs |= value != c.sample(at).translation;
        assert!(value[0].abs() <= 0.1 + EPSILON && value[1].abs() <= 0.2 + EPSILON);
        assert_eq!(value[2], 0.0, "no amplitude, no motion");
    }
    assert!(differs, "another seed shakes differently");
    assert_vec(&a.sample(0.5).translation, &[0.0, 0.0, 0.0]);
}

#[test]
fn splines_pass_through_their_points_with_continuous_tangents() {
    let points = [
        [0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0, 0.0],
        [3.0, 1.0, 0.0, 0.0],
    ];
    let span = |index: usize, start: f32| TweenSegment {
        shape: TweenShape::Spline {
            before: points[index.saturating_sub(1)],
            after: points[(index + 2).min(points.len() - 1)],
        },
        ..segment(
            TweenChannel::Translation,
            start,
            1.0,
            points[index],
            points[index + 1],
        )
    };
    let tween = definition(vec![span(0, 0.0), span(1, 1.0), span(2, 2.0)]);
    for (index, point) in points.iter().enumerate() {
        assert_vec(&tween.sample(index as f64).translation, &point[..3]);
    }
    let slope = |at: f64| {
        let delta = 1.0e-3;
        let (a, b) = (
            tween.sample(at - delta).translation,
            tween.sample(at + delta).translation,
        );
        (b[1] - a[1]) / (2.0 * delta as f32)
    };
    // At the joint between spans 0 and 1 the curve is smooth.
    assert!((slope(0.999) - slope(1.001)).abs() < 0.05);
}

#[test]
fn repeats_and_yoyo_map_elapsed_time_to_iterations() {
    let fade = segment(TweenChannel::Tint, 0.0, 1.0, [1.0, 1.0, 1.0, 0.0], [1.0; 4]);
    let twice = TweenDefinition::new(vec![fade], Vec::new(), TweenRepeat::Count(2), false).unwrap();
    assert_eq!(twice.total_seconds(), Some(2.0));
    assert!(close(twice.sample(1.25).tint[3], 0.25));
    let yoyo = TweenDefinition::new(vec![fade], Vec::new(), TweenRepeat::Count(2), true).unwrap();
    assert!(
        close(yoyo.sample(1.25).tint[3], 0.75),
        "the second iteration plays back"
    );
    assert!(
        close(yoyo.sample(2.0).tint[3], 0.0),
        "and ends where the first began"
    );
    let breathe = TweenDefinition::new(vec![fade], Vec::new(), TweenRepeat::Forever, true).unwrap();
    assert_eq!(breathe.total_seconds(), None);
    assert!(close(breathe.sample(101.25).tint[3], 0.75));
    // Zero counts as one; a timeline with no length ends at once.
    let once = TweenDefinition::new(vec![fade], Vec::new(), TweenRepeat::Count(0), false).unwrap();
    assert_eq!(once.total_seconds(), Some(1.0));
    let instant = TweenDefinition::new(
        vec![segment(TweenChannel::Tint, 0.0, 0.0, [0.5; 4], [1.0; 4])],
        Vec::new(),
        TweenRepeat::Forever,
        false,
    )
    .unwrap();
    assert_eq!(instant.total_seconds(), Some(0.0));
}

#[test]
fn a_yoyo_crosses_its_markers_where_its_pose_passes_them() {
    let markers = [(25, 0.25), (75, 0.75)]
        .map(|(marker_id, time_seconds)| TweenMarker {
            marker_id,
            time_seconds,
        })
        .to_vec();
    let tween = TweenDefinition::new(
        vec![segment(
            TweenChannel::Translation,
            0.0,
            1.0,
            [0.0; 4],
            [1.0, 0.0, 0.0, 0.0],
        )],
        markers,
        TweenRepeat::Count(2),
        true,
    )
    .unwrap();
    let crossings = |from: f64, to: f64| {
        let mut crossed = Vec::new();
        tween.crossings(from, to, &mut crossed);
        crossed
    };
    // Reversed, 1.25 s shows 0.75: it has just crossed marker 75.
    assert_vec(&tween.sample(1.25).translation, &[0.75, 0.0, 0.0]);
    assert_eq!(crossings(1.0, 1.3), vec![(75, 1)]);
    assert_eq!(crossings(1.3, 2.0), vec![(25, 1)]);
    // Across the turn: forward 25 and 75, then back over 75.
    assert_eq!(crossings(0.0, 1.5), vec![(25, 0), (75, 0), (75, 1)]);
    // Every pose the markers sit at agrees with the marker crossed there.
    for (from, to) in [(0.2, 0.3), (0.7, 0.8), (1.2, 1.3), (1.7, 1.8)] {
        let crossed = crossings(from, to);
        let (a, b) = (
            tween.sample(from).translation[0],
            tween.sample(to).translation[0],
        );
        let expected: Vec<u64> = [25u64, 75]
            .into_iter()
            .filter(|id| {
                let at = *id as f32 / 100.0;
                a.min(b) < at && at <= a.max(b)
            })
            .collect();
        assert_eq!(
            crossed.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            expected,
            "{from}..{to}"
        );
    }
}

#[test]
fn markers_are_crossed_once_per_iteration_in_time_order() {
    let markers = vec![
        TweenMarker {
            marker_id: 2,
            time_seconds: 0.5,
        },
        TweenMarker {
            marker_id: 1,
            time_seconds: 0.0,
        },
        TweenMarker {
            marker_id: 9,
            time_seconds: 3.0,
        },
    ];
    let tween = TweenDefinition::new(
        vec![segment(TweenChannel::Scale, 0.0, 1.0, [1.0; 4], [2.0; 4])],
        markers,
        TweenRepeat::Count(3),
        true,
    )
    .unwrap();
    let mut crossed = Vec::new();
    tween.crossings(0.0, 0.5, &mut crossed);
    assert_eq!(
        crossed,
        vec![(1, 0), (2, 0)],
        "a marker at 0 fires on the first advance"
    );
    crossed.clear();
    tween.crossings(0.5, 0.5, &mut crossed);
    tween.crossings(0.5, 2.6, &mut crossed);
    // The yoyo's second iteration plays back from 1 to 0: it crosses 0.5,
    // then turns on 0, which the third iteration does not cross again.
    assert_eq!(crossed, vec![(2, 1), (1, 1), (2, 2)]);
    crossed.clear();
    // Past the last iteration nothing more fires, and the marker after the
    // iteration end never does.
    tween.crossings(2.6, 50.0, &mut crossed);
    assert!(crossed.is_empty());
    let error = TweenDefinition::new(
        Vec::new(),
        vec![TweenMarker {
            marker_id: 1,
            time_seconds: f32::NAN,
        }],
        TweenRepeat::Count(1),
        false,
    );
    assert_eq!(
        error,
        Err(TweenDefinitionError::InvalidMarker { marker: 0 })
    );
}

#[test]
fn playback_follows_its_own_clock_and_reports_completion() {
    let mut world = TweenPlayback::new(definition(vec![hop()]), TweenClock::World);
    let mut realtime = TweenPlayback::new(definition(vec![hop()]), TweenClock::Realtime);
    let mut crossed = Vec::new();
    // A held world: no world seconds, host time moves.
    assert!(!world.advance(0.0, 0.1, &mut crossed));
    assert!(!realtime.advance(0.0, 0.1, &mut crossed));
    assert_eq!(
        (world.elapsed_seconds(), realtime.elapsed_seconds()),
        (0.0, 0.1)
    );
    world.pause();
    assert!(!world.advance(1.0, 1.0, &mut crossed));
    assert_eq!(world.state(), TweenState::Paused);
    world.resume();
    assert!(
        world.advance(1.0, 0.0, &mut crossed),
        "completes on the advance past its end"
    );
    assert_eq!(world.state(), TweenState::Completed);
    assert_eq!(world.elapsed_seconds(), f64::from(0.4_f32));
    assert!(!world.advance(1.0, 1.0, &mut crossed), "completes once");
    assert_eq!(world.offset(), TweenOffset::IDENTITY);
    realtime.complete();
    assert_eq!(realtime.state(), TweenState::Completed);
    assert_eq!(realtime.offset(), TweenOffset::IDENTITY);
}

#[test]
fn starting_from_a_carried_offset_replaces_each_base_tracks_start() {
    let squash = segment(
        TweenChannel::Scale,
        0.1,
        0.2,
        [1.3, 0.7, 1.0, 1.0],
        [1.0; 4],
    );
    let mut tween = definition(vec![hop(), squash]);
    let carry = TweenOffset {
        translation: [-0.4, 0.3, 0.0],
        scale: [1.1, 0.9, 1.0],
        ..TweenOffset::IDENTITY
    };
    tween.start_from(&carry);
    let start = tween.sample(0.0);
    assert_vec(&start.translation, &carry.translation);
    assert_vec(&start.scale, &carry.scale);
    assert_vec(&tween.sample(0.4).translation, &[0.0, 0.0, 0.0]);
}

#[test]
fn offsets_apply_over_a_published_transform_and_can_be_recovered() {
    let published = Transform {
        translation: [3.0, 0.0, 1.0],
        rotation: [0.0, (0.3_f32).sin(), 0.0, (0.3_f32).cos()],
        scale: [2.0, 2.0, 0.0],
    };
    let offset = TweenOffset {
        translation: [0.5, 1.0, 0.0],
        rotation: [(0.2_f32).sin(), 0.0, 0.0, (0.2_f32).cos()],
        scale: [1.5, 0.5, 3.0],
        tint: [1.0, 0.5, 0.5, 0.25],
    };
    let presented = offset.apply(&published);
    assert_vec(&presented.translation, &[3.5, 1.0, 1.0]);
    assert_vec(&presented.scale, &[3.0, 1.0, 0.0]);
    let recovered = TweenOffset::between(&presented, &published, offset.tint);
    assert_vec(&recovered.translation, &offset.translation);
    assert_vec(&recovered.rotation, &offset.rotation);
    // A zero published scale carries none on that axis.
    assert_vec(&recovered.scale, &[1.5, 0.5, 1.0]);
    assert_eq!(offset.tint([0.8, 0.8, 0.8, 1.0]), [0.8, 0.4, 0.4, 0.25]);
    assert_eq!(TweenOffset::IDENTITY.apply(&published), published);
    // Layering composes translation by sum and the rest by product.
    let twice = offset.then(&offset);
    assert_vec(&twice.translation, &[1.0, 2.0, 0.0]);
    assert_vec(&twice.scale, &[2.25, 0.25, 9.0]);
}

#[test]
fn invalid_times_and_values_are_refused() {
    let base = segment(TweenChannel::Scale, 0.0, 1.0, [1.0; 4], [2.0; 4]);
    for (bad, error) in [
        (
            TweenSegment {
                start_seconds: -1.0,
                ..base
            },
            TweenDefinitionError::InvalidTime { segment: 0 },
        ),
        (
            TweenSegment {
                duration_seconds: f32::INFINITY,
                ..base
            },
            TweenDefinitionError::InvalidTime { segment: 0 },
        ),
        (
            TweenSegment {
                to: [f32::NAN; 4],
                ..base
            },
            TweenDefinitionError::InvalidValue { segment: 0 },
        ),
        (
            TweenSegment {
                easing: Easing::CubicBezier([0.0, f32::NAN, 1.0, 1.0]),
                ..base
            },
            TweenDefinitionError::InvalidValue { segment: 0 },
        ),
    ] {
        assert_eq!(
            TweenDefinition::new(vec![bad], Vec::new(), TweenRepeat::Count(1), false),
            Err(error)
        );
    }
}
