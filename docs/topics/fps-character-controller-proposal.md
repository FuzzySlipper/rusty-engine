# FPS character controller design

## Decision status

**Approved and adopted for implementation.** This document records the design
decision for Den task 6847, the public Rust surface currently present in the
worktree, and the remaining verification/consumer work. The companion
[survey](fps-character-controller-survey.md) records the research basis, and
the [canonical design](../design.md) owns the durable authority boundary.

## Adopted decision

Use a dedicated `CharacterControllerService` in `engine-spatial`, supported by
host-neutral capsule query primitives in `svc-collision`. The service composes
configurable FPS movement policy with bounded swept-capsule constraint solving
and returns an exact-revision prepared result containing accepted Transform,
motion continuation state, and typed contact facts.

Use a separate, smaller `FirstPersonLookService` for canonical pivot-look math.
It shares heading conventions with the movement service but is not part of the
collision controller. A downstream may use both, use only one, or replace
either without reimplementing the other.

This is a new path. It does not change:

- `FirstPersonMotionService`, which remains free-fly/noclip-style pose motion;
- `EntityMotionService`, which remains its existing bounded entity motion path;
- `KinematicMotionSystem`, which remains the simple axis-separated AABB path;
- `RigidBodyService`, which remains the dynamic-body authority.

## Ownership boundary

| Concern | Owner |
|---|---|
| Capsule casts, overlap recovery, stable hit geometry | `svc-collision` |
| Movement policy, slide/step/snap solve, support tracking, prepared result | `engine-spatial::CharacterControllerService` |
| Canonical entity transform and inert character-motion facts | `entity-state` |
| Host-neutral yaw/pitch pivot-look integration | `engine-spatial::FirstPersonLookService` |
| Fixed-step scheduling, command construction, locomotion-mode selection | Downstream Rust |
| Gameplay tuning, abilities, damage, stamina, material/audio consequences | Downstream game |
| Camera, device bindings, browser/native host, presentation smoothing | Downstream host |
| Dynamic-body integration and accepted impulses | `RigidBodyService` plus downstream orchestration |

The service consumes semantic movement intent, not keyboard or camera objects.
Yaw follows the established Engine convention: yaw zero faces -Z and positive
yaw turns toward +X. Pitch is not used for ordinary planar locomotion.

## Public data model

The names below are the currently implemented and facade-exported Rust API.

### Configuration

`CharacterControllerConfig` contains bounded values grouped into these public
nested structs:

- `CharacterShapeConfig`: standing/crouched height, radius, contact skin, and
  clearance padding;
- `CharacterGroundConfig`: directional speed limits, acceleration, braking,
  friction, stop speed, and direction-change multiplier;
- `CharacterAirConfig`: maximum/wish speed, acceleration, braking, lateral
  control, and drag;
- `CharacterVerticalConfig`: gravity, terminal rise/fall speed, jump speed,
  and grounded downward bias;
- `CharacterJumpConfig`: buffer, coyote, landing-lockout, and held-input
  retrigger policy;
- `CharacterSurfaceConfig`: slope/hysteresis and steep-slide policy, step
  dimensions, floor snap, and ledge-support fraction;
- `CharacterRecoveryConfig`: bounded depenetration distance/speed, normal
  nudge, and unresolved tolerance;
- `CharacterPlatformConfig`: translation/rotation carry, departure inheritance,
  support-loss grace, and crush tolerance;
- `CharacterExternalMotionConfig`: impulse/decay/speed bounds, authored mass,
  and bounded dynamic-impulse proposal policy; and
- `CharacterSolverConfig`: slide planes, cast/recovery/contact/step/query limits,
  and maximum displacement.

Every dimension and rate has a documented unit. Angles are radians. Invalid,
non-finite, negative, contradictory, or unsafe configurations are rejected
before any query or mutation.

The configuration is intentionally broad and composed from public nested
policy structs. The top-level and every nested struct are serde-defaulted and
`#[non_exhaustive]`. External Rust callers start from `Default` or
`responsive_fps()` and mutate selected public fields; sparse serialized
documents receive the same defaults. This permits compatible defaulted fields
to be added without requiring exhaustive downstream literals. Validation runs
before collision queries or mutation and reports the rejected field where the
error is field-specific. The adopted `responsive_fps` baseline is in Engine
world units:

| Setting | Default |
|---|---:|
| Standing height / crouched height / radius | `1.8 / 1.1 / 0.35` |
| Contact skin / clearance padding | `0.02 / 0.01` |
| Forward / backward / strafe speed | `5.0 / 4.5 / 5.0` units/s |
| Ground acceleration / braking / friction | `35 / 45 / 8` units/s² |
| Air acceleration / wish-speed cap | `12 / 5` units/s², units/s |
| Gravity / terminal fall speed / jump speed | `20 / 55 / 7` units/s², units/s, units/s |
| Standable slope / step height / floor snap | `50° / 0.4 / 0.25` |
| Jump buffer / coyote time | `0.12 / 0.10` seconds |
| Slide planes / cast iterations / recovery passes | `5 / 8 / 4` |

These are coherent out-of-box values, not compatibility law. CraftSurvive
calibration and benchmarks may tune its downstream instance. Additional named
presets should be added only when a real consumer needs a coherent alternative;
the full config remains available so a new feel choice does not require an
upstream API change.

### Command

`CharacterControllerCommand` is one immutable fixed-step input:

- `planar_intent`: bounded local right/forward axes;
- `heading_yaw_radians`: camera-independent heading;
- `jump_pressed` and `jump_held`;
- `crouch_requested`;
- `external_velocity` and one-shot `external_impulse`;
- `step_seconds` and a caller command sequence number.

The service normalizes planar intent so diagonal input cannot exceed unit
magnitude. Device bindings, camera pitch, sprint, swim, climb, noclip, and
ability checks never enter this type.

### Engine-owned runtime state

An inert `CharacterMotionComponent` is the canonical per-entity runtime state:

- controlled and external velocity components;
- standing/crouched stance and grounded state;
- jump-buffer, coyote, and landing-lockout timers;
- moving-platform identity, local contact anchor, prior pose and point velocity;
- fall origin/peak if a neutral travel fact is retained;
- last accepted command sequence and collision-world identity.

The component is data only: no callback, scheduler, input binding, renderer
reference, backend handle, or gameplay consequence. The named service reads it
by entity identity and exact slot revision, prepares the next transform and
motion facts together, and commits both atomically. A caller supplies the
entity, config and command; it does not retain or feed back hidden continuation
state. The receipt includes complete before/after motion readouts for easy
inspection, prediction and tests, but those readouts are not a second mutation
path.

The component has a stable schema-1 JSON codec and exact validation bounds;
downstream complete-save admission and persistence policy remain downstream.
On a changed collision-world identity the service clears stale support before
solving. The existing `ControllerComponent` name is not reused.

### Result facts

`CharacterControllerReceipt` returns generation and entity revisions, command
sequence, complete before/after Transform and `CharacterMotionComponent`
values, wish velocity, and accepted displacement. Its typed observations are:

- optional `CharacterGroundFact` with source, point, normal, and snap distance;
- bounded `CharacterContactFact` values with source, point, normal, TOI,
  ground/steep-slope/wall/ceiling classification, and start-solid state;
- `CharacterBlockKind` values for wall, ceiling, steep slope, start-solid, and
  solver-budget outcomes;
- requested/accepted/blocked `CharacterStanceFact`;
- optional attempted/accepted/rise `CharacterStepFact`;
- optional `CharacterPlatformFact` with support entity, carried displacement,
  point velocity, and departure state;
- bounded `DynamicImpulseProposal` values for downstream/rigid-body handling;
  and
- cast/recovery-pass counts and total recovery distance.

`CharacterControllerService::readout()` exposes the last accepted generation,
entity, command sequence, grounded/contact summary, and collision-world hash.
`PreparedCharacterControllerStep` is intentionally opaque: callers may stage
and later commit it, but cannot rewrite the candidate around its captured exact
slot/environment guards. Typed config, command, collision, publication, stale
environment, displacement, penetration/crush, and output errors reject before
partial publication.

These are observations and proposals, not gameplay events. Engine does not play
sounds, apply damage, select materials, or directly mutate dynamic bodies.

## Companion first-person look service

`FirstPersonLookService` standardizes the small but repeatedly error-prone
pivot-look operation without making the character controller own a camera.

Its command contains abstract horizontal and vertical look deltas. The host
converts mouse motion, stick motion, touch drag, accessibility input, or replay
data into those deltas; no pixel, DOM, key-code, or device type enters Engine.
`FirstPersonLookConfig` exposes horizontal/vertical radians per input unit,
independent axis inversion, pitch minimum/maximum, optional yaw wrapping, and
maximum accepted per-axis delta. Defaults use direct input, positive horizontal input turns
toward +X, positive vertical input looks up, pitch clamps just short of ±90°,
and position is never changed.

The service consumes `FirstPersonLookState` yaw/pitch and returns yaw, pitch,
orientation, and forward/right/up basis facts using the same convention as
`CharacterControllerCommand`. It rotates around the view pivot in place. It
does not orbit a target, change camera distance, apply collision, alter entity
translation, smooth presentation position, or own a renderer camera.

This service is optional. Custom recoil, head bob, third-person orbit, camera
collision, cinematic constraints, VR head pose, or product-specific smoothing
compose downstream. The canonical service exists so ordinary consumers do not
accidentally reverse axes, rotate translation around an origin, mix degrees and
radians, or disagree with movement heading.

## Collision query extension

`svc-collision` exposes Engine-owned types for:

1. local +Y capsule overlap with penetration normal/depth;
2. capsule cast over a bounded displacement;
3. stable source identity across voxel chunks, admitted static meshes, and
   explicit active-entity obstacles.

A cast hit includes TOI, contact point, normal, start-solid/convergence state,
and stable source identity. An overlap includes point, separation normal, and
penetration depth. The query surface uses `f64` world values and does not expose
Parry or Rapier handles.

Voxel/static-mesh casts and active-entity obstacle casts are combined behind
the service boundary. Entity obstacle identity, bounds, and available
rigid-body/character velocity contribute to the captured environment hash and
dynamic impulse proposals; backend handles never enter public or durable state.

## Solve pipeline

For each command, the service prepares the complete candidate off-side:

1. Validate configuration, command, pose, state, `dt`, and supported motion
   envelope.
2. Capture exact Transform and character-motion slot revisions plus a
   collision-world identity covering voxel state, static-mesh projection, and
   active entity obstacle geometry/motion.
3. Validate or invalidate cached support after origin/world revision changes.
4. Apply moving-platform carry from the identified support's change in pose and
   velocity at the contact point.
5. Resolve crouch/stand request. Crouching preserves the feet; standing occurs
   only if an expanded-capsule clearance query succeeds.
6. Advance jump-buffer/coyote timers and consume at most one jump transition.
7. Convert heading-relative planar intent into wish velocity; apply ground or
   air acceleration, friction, braking, gravity, terminal velocity, and external
   contributions as separate values.
8. Recover a bounded initial overlap. Return a typed unresolved result rather
   than searching indefinitely or teleporting through arbitrary offsets.
9. Sweep and slide over remaining time. Accumulate unique contact planes,
   ignore numerically repeated planes, move along a two-plane crease, and stop
   when three independent planes constrain motion or the iteration budget ends.
10. On a low forward obstruction, compare the direct candidate with a
    step-up/forward/down candidate. Require upward clearance, forward width,
    standable landing, and no larger penetration; choose the candidate with
    greater intended planar progress, using a stable tie rule.
11. Apply floor snap only when not jumping, not intentionally moving upward,
    and within the configured adhesion envelope. Never snap to an over-limit
    slope.
12. Classify final ground, wall, ceiling, step, platform, and block facts;
    validate finite output and supported penetration tolerance.
13. Return a prepared candidate. Commit rechecks the environment and exact
    component slots before atomically publishing Transform and
    `CharacterMotionComponent`.

Any stale entity or collision-world revision rejects the whole candidate with
no partial mutation. Callers may prepare again against the new world, which is
how edits beneath or around the player remain coherent.

## Specific behavior decisions

### Ground and slopes

A ground contact has a normal whose dot product with +Y meets the configured
standable threshold. A shallower contact is a steep slope, not a wall merely
because it is unstandable. Controlled planar velocity is projected onto the
standable plane. A steep slope may remove inward velocity and contribute
downhill motion, but may not report grounded support.

Ground friction and braking operate only on controlled planar velocity.
External velocity decays only according to explicit external-contribution
policy, preventing ground friction from silently erasing knockback.

### Corners and walls

The solver retains contact planes for the current command. One plane removes
inward velocity; two independent planes allow only crease motion; three stop
the constrained component. Iterations, epsilon/nudge, and total correction are
bounded and reported. Stable source/plane ordering makes equal-TOI results
independent of backend enumeration order.

### Steps and ledges

Autostep is attempted only for grounded or recently supported locomotion, a
predominantly lateral obstruction, and a non-upward intentional move. The
raised candidate must fit the complete capsule and land on a standable surface.
Descending steps use floor snap; open ledges become airborne rather than
manufacturing support.

### Jump, ceiling, and stance

Jump input is buffered for a configured duration and may consume coyote support
for a configured duration. Both durations may be zero. A jump clears floor snap
for that command and separates from the current platform. A ceiling hit removes
only upward velocity; it never reverses velocity as an unexplained bounce.

Crouch is a collision shape and state fact, not camera animation. Downstream
presentation may smooth eye height independently. Releasing crouch beneath a
ceiling keeps the crouched capsule and reports the blocked requested/accepted
stance transition.

### Moving platforms

Support is tracked by stable identity and local contact anchor, never transform
parenting. Before ordinary motion, the controller applies the configured
support translation/rotation carry at that anchor. On departure, configuration
selects whether to inherit point velocity and the bounded authored factor
applied to it. The returned platform fact identifies a departure and the last
observed point velocity.

A disappeared or revision-changed platform invalidates support safely. Platform
motion that would crush the capsule participates in ordinary contacts and may
end with a typed unresolved/crush candidate; damage remains downstream.

### Dynamic bodies and external motion

Dynamic bodies are read-only obstacles during the geometric solve. The service
may compute bounded impulse proposals from contact normal, relative velocity,
and authored controller mass, but another named owner decides whether and when
to apply them. The next controller command observes the resulting dynamic
readout revision.

Controlled velocity remains steerable while external velocity represents
knockback, conveyor, explosion, or other imposed motion. Both are constrained
by collision, but they remain separately observable so downstream policy can
decay or replace the external contribution deliberately.

### Other locomotion modes

Grounded, airborne, and crouched are neutral controller states; controlled and
external velocity remain separate contributions. Swimming, climbing, ladders,
mantling, noclip, flying, vehicles, and game abilities remain downstream modes.
They can construct a different command, use the collision query surface
directly, or select the existing free-fly service. The controller does not
acquire a universal mode graph.

## Determinism and bounds

- Downstream calls the service on a fixed-step schedule. Engine does not own
  catch-up or wall-clock policy.
- Identical canonical input, command sequence, configuration, Engine build,
  target, and collision-world revisions produce identical ordered facts within
  the documented numeric contract.
- Cast count, overlap passes, contact planes, contacts, correction distance,
  step attempts, and displacement per command are hard bounded.
- Variable partition tests compare supported tolerance and reveal partition
  sensitivity; they do not claim arbitrary partition equivalence.
- Non-finite inputs or outputs, unsupported displacement, stale revisions, and
  exhausted recovery return typed errors without mutation.

## Verification and remaining acceptance

### Current focused mechanism routes

The current focused commands are:

```bash
cargo test -p svc-collision character_capsule --locked
cargo test -p entity-state character_motion --locked
cargo test -p engine-spatial --lib character_controller::tests --locked
cargo test -p engine-spatial --test character_controller --locked
cargo run -p rusty-engine --example character_controller --locked
./scripts/measure-character-controller.sh
./scripts/verify-rust-sdk-consumer.sh
./scripts/verify-character-controller-consumer.sh /home/dev/rusty-craftsurvive
```

The collision tests cover capsule cast TOI/normal/source identity and overlap
separation depth. Entity-state tests cover schema round-trip/validation and
exact Transform+motion publication with stale rejection and no partial change.
The controller library tests cover sparse/defaulted config compatibility,
field-specific validation, the shared look heading convention, pitch clamping,
and look-delta bounds. The `engine-spatial` integration test currently covers
diagonal-normalized level motion, tangent-preserving wall slide/contact
reporting, foot-preserving crouch with blocked stand, buffered/coyote jump
continuation, stale-environment fail-atomic commit, bounded autostep,
moving-platform carry, controlled/external velocity separation, ceiling
response, unresolved-overlap failure atomicity, below/above-limit ramps, and a
generated bounded command stream. It also covers the exact standable slope,
live edits beneath the character, and repeatable/near-equivalent 60 Hz versus
30 Hz partitions. The release-only measurement route runs 5,000 steps each over
representative voxel, admitted static-mesh, and mixed 128-active-collider scenes
and enforces the current less-than-1 ms per-step budget. The facade consumer gate
checks that the public exports remain
consumable from a clean temporary Rust crate. The facade example constructs an
entity with Transform plus `CharacterMotionComponent`, executes a movement
step, applies the separate look service, and prints accepted observations.
The selected-consumer script first verifies that CraftSurvive uses the adjacent
complete facade path and then invokes the consumer-owned `./scripts/verify.sh`;
it does not fetch, reset, or otherwise manage that checkout.

These are focused implementation routes, not the complete task acceptance
suite. In particular, they do not claim browser behavior, CraftSurvive product
integration, a recorded performance result, broad fuzz/property coverage, or
Luna usability.

### Remaining deterministic mechanism fixtures

Additional fixtures required by task acceptance include:

- wall slide, acute/obtuse inside corners, outside corners and narrow gaps;
- ascending and descending steps, blocked step headroom, ledges and floor snap;
- held-jump policy and additional jump-buffer/coyote boundary cases;
- acceleration, braking, friction, and floor-snap edge conditions;
- translating and rotating platforms, departure, disappearance and crush;
- dynamic impulse proposals;
- initial overlap, bounded recovery, all-solid and solver-budget exhaustion;
- live voxel/static/entity/dynamic revision changes between prepare and commit;
- fixed-step repetitions and supported partition comparisons;
- first-person look axis signs, inversion, pitch clamping, yaw wrapping,
  degree/radian misuse rejection, basis consistency, and unchanged position.

### Properties and fuzzing

Generated bounded scenes and commands remain to be added to assert:

- finite facts and no unbounded loops;
- no penetration growth beyond tolerance after accepted movement;
- no supported-envelope tunneling;
- stable output under shuffled equal-priority obstacle enumeration;
- no partial publication on any stale revision or invalid result;
- contact/diagnostic arrays never exceed declared bounds.

### Performance

`./scripts/measure-character-controller.sh` runs the ignored release-mode
representative scene probe and enforces the current 1 ms fixed-step budget for
one controller. On the 2026-08-11 development machine it measured 19,833 ns per
voxel step, 14,431 ns per admitted static-mesh step, and 129,985 ns per step in
the mixed 128-active-collider scene (5,000 commands each). Broader measurements
still need stairs, platforms, edits, multiple controllers, p50/p95 preparation
time, allocation count, and worst accepted solver iterations.

### Consumer proof

The Engine-owned explicit
`./scripts/verify-character-controller-consumer.sh /home/dev/rusty-craftsurvive`
integration route is now present. It admits only the selected adjacent facade
path and delegates proof to the consumer's verification gate. CraftSurvive
remains the explicit proving consumer and will
exercise real keyboard/mouse movement, terrain edits beneath and beside the
capsule, stairs/slopes/crouch/platform fixtures, and typed diagnostics in a real
browser. A later Luna/max pass supplies usability judgment; deterministic
Engine fixtures remain the mechanism proof. Neither the browser route nor the
Luna pass has run yet, and this document does not claim either. The sibling
checkout never becomes an ordinary Engine dependency.

## Implementation status and sequence

1. Implemented: typed capsule cast/overlap facts in `svc-collision`, preserving
   the existing AABB APIs.
2. Implemented: bounded movement policy and capsule slide/step/snap/recovery in
   `CharacterControllerService` with typed receipts.
3. Implemented: durable inert `CharacterMotionComponent`, exact
   prepare/commit guards, and atomic Transform+motion publication.
4. Implemented: separate `FirstPersonLookService` and public facade exports.
5. Current documentation work: adopt the authority boundary and route the
   implemented API and focused tests/examples.
6. Remaining acceptance: broaden deterministic/property coverage, measure the
   representative budget, run the explicit CraftSurvive integration, and then
   obtain browser and Luna/max evidence.

## Adopted decisions

The approved design decisions are:

1. New `engine-spatial` service plus `svc-collision` query extensions, not a new
   crate and not an expansion of `KinematicMotionSystem`.
2. Engine-owned inert `CharacterMotionComponent`, atomically updated with the
   transform. Receipts expose complete before/after readouts, but callers do not
   manually round-trip continuation state.
3. Capsule-only version-one controller, local +Y, with parented or non-unit-scale
   entities rejected rather than ambiguously transformed.
4. Dynamic impulse proposals only; dynamic-body mutation remains with the
   rigid-body owner and downstream orchestration.
5. Core grounded/airborne/crouched/external-motion states only; swimming,
   climbing, ladders and noclip remain composable downstream modes.
6. Preserve all existing motion APIs and semantics while consumers opt into the
   new service explicitly.
7. Add an optional companion `FirstPersonLookService` with shared yaw/pitch
   conventions and pivot-only orientation math; keep renderer camera, orbit,
   recoil, bob and product smoothing downstream.

These decisions are now part of the provider architecture. Remaining acceptance
evidence may refine implementation details and defaults without moving device,
camera, scheduling, gameplay, or consumer ownership into Engine.
