use render_presentation::{spring_settle_seconds, EaseFamily, EaseMode, Easing};

const FAMILIES: [EaseFamily; 10] = [
    EaseFamily::Quad,
    EaseFamily::Cubic,
    EaseFamily::Quart,
    EaseFamily::Quint,
    EaseFamily::Sine,
    EaseFamily::Expo,
    EaseFamily::Circ,
    EaseFamily::Back,
    EaseFamily::Elastic,
    EaseFamily::Bounce,
];
const MODES: [EaseMode; 3] = [EaseMode::In, EaseMode::Out, EaseMode::InOut];
const SAMPLES: usize = 400;

fn all_curves() -> Vec<Easing> {
    let mut curves = vec![
        Easing::Linear,
        Easing::CubicBezier([0.25, 0.1, 0.25, 1.0]),
        Easing::CubicBezier([0.3, -0.6, 0.7, 1.6]),
        Easing::Steps(4),
        Easing::Spring {
            stiffness: 180.0,
            damping: 12.0,
        },
        Easing::Spring {
            stiffness: 100.0,
            damping: 20.0,
        },
        Easing::Spring {
            stiffness: 100.0,
            damping: 40.0,
        },
    ];
    for family in FAMILIES {
        for mode in MODES {
            curves.push(Easing::Ease(family, mode));
        }
    }
    curves
}

fn samples(curve: Easing) -> impl Iterator<Item = (f32, f32)> {
    (0..=SAMPLES).map(move |index| {
        let t = index as f32 / SAMPLES as f32;
        (t, curve.sample(t))
    })
}

#[test]
fn every_curve_starts_at_zero_and_ends_at_one_and_clamps_outside() {
    for curve in all_curves() {
        assert_eq!(curve.sample(0.0), 0.0, "{curve:?}");
        assert_eq!(curve.sample(1.0), 1.0, "{curve:?}");
        assert_eq!(curve.sample(-0.5), 0.0, "{curve:?}");
        assert_eq!(curve.sample(1.5), 1.0, "{curve:?}");
        for (t, value) in samples(curve) {
            assert!(value.is_finite(), "{curve:?} at {t}");
        }
    }
}

#[test]
fn every_curve_is_continuous_near_its_ends() {
    for curve in all_curves() {
        if matches!(curve, Easing::Steps(_)) {
            continue;
        }
        assert!(curve.sample(1.0e-5).abs() < 0.01, "{curve:?}");
        assert!((curve.sample(1.0 - 1.0e-5) - 1.0).abs() < 0.01, "{curve:?}");
    }
}

#[test]
fn power_sine_expo_and_circ_curves_never_go_backwards() {
    let monotonic = [
        EaseFamily::Quad,
        EaseFamily::Cubic,
        EaseFamily::Quart,
        EaseFamily::Quint,
        EaseFamily::Sine,
        EaseFamily::Expo,
        EaseFamily::Circ,
    ];
    let mut curves: Vec<Easing> = monotonic
        .iter()
        .flat_map(|family| MODES.map(|mode| Easing::Ease(*family, mode)))
        .collect();
    curves.extend([
        Easing::Linear,
        Easing::Steps(5),
        Easing::CubicBezier([0.42, 0.0, 0.58, 1.0]),
        Easing::Spring {
            stiffness: 100.0,
            damping: 20.0,
        },
        Easing::Spring {
            stiffness: 100.0,
            damping: 40.0,
        },
    ]);
    for curve in curves {
        let mut previous = 0.0;
        for (t, value) in samples(curve) {
            assert!(value >= previous - 1.0e-6, "{curve:?} went back at {t}");
            assert!((0.0..=1.0).contains(&value), "{curve:?} left [0, 1] at {t}");
            previous = value;
        }
    }
}

#[test]
fn in_and_out_forms_mirror_each_other_and_in_out_passes_the_middle() {
    for family in FAMILIES {
        let ease_in = Easing::Ease(family, EaseMode::In);
        let ease_out = Easing::Ease(family, EaseMode::Out);
        for (t, value) in samples(ease_in) {
            let mirrored = 1.0 - ease_out.sample(1.0 - t);
            assert!((value - mirrored).abs() < 1.0e-5, "{family:?} at {t}");
        }
        let middle = Easing::Ease(family, EaseMode::InOut).sample(0.5);
        assert!((middle - 0.5).abs() < 1.0e-5, "{family:?} middle {middle}");
    }
    // Ease-in starts slowly: below the line at a quarter; ease-out above it.
    for family in [
        EaseFamily::Quad,
        EaseFamily::Cubic,
        EaseFamily::Sine,
        EaseFamily::Circ,
    ] {
        assert!(Easing::Ease(family, EaseMode::In).sample(0.25) < 0.25);
        assert!(Easing::Ease(family, EaseMode::Out).sample(0.25) > 0.25);
    }
}

#[test]
fn back_and_elastic_overshoot_and_bounce_stays_inside() {
    let extremes = |curve: Easing| {
        samples(curve).fold((f32::MAX, f32::MIN), |(low, high), (_, value)| {
            (low.min(value), high.max(value))
        })
    };
    let (low, _) = extremes(Easing::Ease(EaseFamily::Back, EaseMode::In));
    assert!(
        low < -0.09 && low > -0.11,
        "back-in dips to about -0.1, got {low}"
    );
    let (_, high) = extremes(Easing::Ease(EaseFamily::Back, EaseMode::Out));
    assert!(
        high > 1.09 && high < 1.11,
        "back-out peaks near 1.1, got {high}"
    );
    let (low, high) = extremes(Easing::Ease(EaseFamily::Back, EaseMode::InOut));
    assert!(
        low < -0.05 && high > 1.05,
        "back in-out goes both ways: {low} {high}"
    );
    let (_, high) = extremes(Easing::Ease(EaseFamily::Elastic, EaseMode::Out));
    assert!(high > 1.2, "elastic-out overshoots, got {high}");
    let (low, high) = extremes(Easing::Ease(EaseFamily::Bounce, EaseMode::Out));
    assert!(
        low >= 0.0 && high <= 1.0 + 1.0e-6,
        "bounce stays in [0, 1]: {low} {high}"
    );
    // Bounce-out touches 1 before the end and leaves it again.
    assert!(Easing::Ease(EaseFamily::Bounce, EaseMode::Out).sample(1.0 / 2.75) > 0.999);
    assert!(Easing::Ease(EaseFamily::Bounce, EaseMode::Out).sample(0.8) < 0.95);
}

#[test]
fn steps_hold_each_level_and_reach_one_only_at_the_end() {
    let curve = Easing::Steps(4);
    let levels: Vec<f32> = [0.1, 0.24, 0.26, 0.6, 0.8, 0.99]
        .map(|t| curve.sample(t))
        .to_vec();
    assert_eq!(levels, vec![0.0, 0.0, 0.25, 0.5, 0.75, 0.75]);
    assert_eq!(
        Easing::Steps(0).sample(0.5),
        0.0,
        "zero steps behaves as one"
    );
}

#[test]
fn cubic_bezier_matches_known_points_and_its_linear_form() {
    let linear = Easing::CubicBezier([0.0, 0.0, 1.0, 1.0]);
    for (t, value) in samples(linear) {
        assert!((value - t).abs() < 1.0e-4, "linear bezier at {t}: {value}");
    }
    // CSS `ease` at its midpoint is about 0.8024.
    let ease = Easing::CubicBezier([0.25, 0.1, 0.25, 1.0]).sample(0.5);
    assert!((ease - 0.8024).abs() < 1.0e-3, "css ease at 0.5: {ease}");
    // Control values above 1 overshoot.
    let overshoot = samples(Easing::CubicBezier([0.3, -0.6, 0.7, 1.6]))
        .map(|(_, value)| value)
        .fold(f32::MIN, f32::max);
    assert!(overshoot > 1.05);
}

#[test]
fn springs_settle_by_their_reported_time_and_underdamped_ones_overshoot() {
    let bouncy = Easing::Spring {
        stiffness: 180.0,
        damping: 12.0,
    };
    let high = samples(bouncy)
        .map(|(_, value)| value)
        .fold(f32::MIN, f32::max);
    assert!(high > 1.1, "underdamped spring overshoots, got {high}");
    // Critically damped: ζ = 1 at damping 2√k.
    let critical = Easing::Spring {
        stiffness: 100.0,
        damping: 20.0,
    };
    let high = samples(critical)
        .map(|(_, value)| value)
        .fold(f32::MIN, f32::max);
    assert!(
        high <= 1.0 + 1.0e-6,
        "critically damped spring never overshoots"
    );
    // Within 0.1% at 99% of the normalized settle time.
    for curve in [
        bouncy,
        critical,
        Easing::Spring {
            stiffness: 100.0,
            damping: 40.0,
        },
    ] {
        assert!((curve.sample(0.99) - 1.0).abs() < 2.0e-3, "{curve:?}");
    }
    // ζω = 6: e^(-6t)(1 + 6t) = 0.001 at t = 1.539 s.
    let settle = spring_settle_seconds(180.0, 12.0);
    assert!((settle - 1.539).abs() < 1.0e-2, "settle {settle}");
    // Stiffer springs settle sooner; non-positive values fall back.
    assert!(spring_settle_seconds(400.0, 40.0) < spring_settle_seconds(100.0, 20.0));
    assert!(spring_settle_seconds(0.0, -1.0).is_finite());
}
