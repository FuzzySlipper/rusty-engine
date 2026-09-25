//! Campaign 6993: exercise the actual pinned solver with caches discarded every tick.
use rapier3d_f64::prelude::*;

const DT: f64 = 1.0 / 60.0;
const GRAVITY: f64 = 9.81;
const SUBSTEPS: usize = 4;
const ITERATIONS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
struct State {
    position: Vector,
    velocity: Vector,
}

fn step(states: &mut [State], length: f64, attached: bool) {
    let mut world = PhysicsWorld {
        gravity: Vector::new(0.0, -GRAVITY, 0.0),
        integration_parameters: IntegrationParameters {
            dt: DT / SUBSTEPS as f64,
            num_solver_iterations: ITERATIONS,
            ..Default::default()
        },
        ..Default::default()
    };
    let fixed = world.insert_body(RigidBodyBuilder::fixed());
    let mut previous = fixed;
    let mut handles = Vec::new();
    for state in states.iter() {
        let (body, _) = world.insert(
            RigidBodyBuilder::dynamic()
                .translation(state.position)
                .linvel(state.velocity)
                .can_sleep(false)
                .ccd_enabled(true),
            ColliderBuilder::ball(0.05).mass(1.0),
        );
        if attached {
            world.insert_impulse_joint(
                previous,
                body,
                RopeJointBuilder::new(length).contacts_enabled(false),
            );
        }
        previous = body;
        handles.push(body);
    }
    for _ in 0..SUBSTEPS {
        world.step();
    }
    for (state, handle) in states.iter_mut().zip(handles) {
        state.position = world.bodies[handle].translation();
        state.velocity = world.bodies[handle].linvel();
        assert!(state.position.is_finite() && state.velocity.is_finite());
    }
}

fn run(initial: State, mut length: impl FnMut(usize) -> f64) -> Vec<State> {
    let mut states = [initial];
    let mut history = Vec::new();
    for tick in 0..600 {
        step(&mut states, length(tick), true);
        history.push(states[0]);
    }
    history
}

#[test]
fn hanging_mass_rebuilds_without_accumulating_extension() {
    let history = run(
        State {
            position: Vector::new(0.0, -3.0, 0.0),
            velocity: Vector::ZERO,
        },
        |_| 3.0,
    );
    let extension = history
        .iter()
        .map(|s| s.position.length() - 3.0)
        .fold(0.0_f64, f64::max);
    println!("hanging maximum extension: {extension}");
    assert!(extension < 0.02);
    assert!(history.last().unwrap().velocity.length() < 0.1);
}

#[test]
fn slack_catch_does_not_create_energy() {
    let initial = State {
        position: Vector::new(0.0, -1.0, 0.0),
        velocity: Vector::new(8.0, -15.0, 0.0),
    };
    let history = run(initial, |_| 3.0);
    let energy = |s: State| 0.5 * s.velocity.length_squared() + GRAVITY * s.position.y;
    let maximum = history
        .iter()
        .map(|s| energy(*s))
        .fold(f64::NEG_INFINITY, f64::max);
    println!("catch initial/max energy: {} / {maximum}", energy(initial));
    assert!(maximum <= energy(initial) + 0.1);
    assert!(history.iter().any(|s| s.position.length() > 2.99));
    assert!(history.iter().all(|s| s.position.length() < 3.1));
}

#[test]
fn pendulum_is_repeatable_and_release_preserves_horizontal_momentum() {
    let initial = State {
        position: Vector::new(2.4, -1.8, 0.0),
        velocity: Vector::ZERO,
    };
    let history = run(initial, |_| 3.0);
    assert_eq!(history, run(initial, |_| 3.0));
    assert!(history.iter().any(|s| s.position.x < -1.0));
    let mut released = [history[30]];
    let before = released[0].velocity.x;
    step(&mut released, 3.0, false);
    assert!((released[0].velocity.x - before).abs() < 1e-10);
}

#[test]
fn gradual_loaded_length_change_is_bounded() {
    let history = run(
        State {
            position: Vector::new(0.0, -3.0, 0.0),
            velocity: Vector::ZERO,
        },
        |tick| {
            if tick < 240 {
                3.0 - tick as f64 * DT * 0.25
            } else {
                2.0 + (tick - 240).min(240) as f64 * DT * 0.25
            }
        },
    );
    let speed = history
        .iter()
        .map(|s| s.velocity.length())
        .fold(0.0_f64, f64::max);
    println!("loaded reeling maximum speed: {speed}");
    assert!(speed < 2.0);
    assert!((history[239].position.length() - 2.0).abs() < 0.03);
    assert!((history[599].position.length() - 3.0).abs() < 0.03);
}

#[test]
fn eight_link_chain_remains_bounded_without_warm_start() {
    let mut states: Vec<_> = (1..=8)
        .map(|i| State {
            position: Vector::new(0.0, -0.5 * f64::from(i), 0.0),
            velocity: Vector::new(1.0, 0.0, 0.0),
        })
        .collect();
    let mut extension = 0.0_f64;
    for _ in 0..600 {
        step(&mut states, 0.5, true);
        let mut previous = Vector::ZERO;
        for state in &states {
            extension = extension.max((state.position - previous).length() - 0.5);
            previous = state.position;
        }
    }
    println!("eight-link maximum segment extension: {extension}");
    assert!(extension < 0.05);
}
