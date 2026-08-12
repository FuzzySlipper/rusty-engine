# FPS character controller survey

## Status and scope

This is the research deliverable for Den task 6847. It records the source
snapshots inspected on 2026-08-11 and separates reusable mechanism from product
policy. The companion
[controller design](fps-character-controller-proposal.md) has since been
approved and adopted; this survey remains the provenance and rationale rather
than the current API reference.

The source checkouts are retained under `/home/research/` so later design and
implementation reviews can reproduce the findings. No donor source has been
copied into Rusty Engine.

## Research corpus and provenance

| Source | Exact revision | License | Primary value |
|---|---|---|---|
| [Daggerfall Unity](https://github.com/Interkarma/daggerfall-unity) | `81e89e90c27bc3c1a7a61871e545fad129174dec` | MIT | Controller orchestration, stance clearance, platforms, mode handoff, origin shifts |
| [Doom.ts](https://gitlab.com/tchandelle/doom.ts) | `0d88ba912f7b084a05b776a19801d45f383cef20` | GPLv3 | Fixed-tick momentum and bounded slide; 2.5D exclusions |
| [Quake III Arena](https://github.com/id-Software/Quake-III-Arena) | `dbe4ddb10315479fc00086f08e25d968b4b43c49` | GPLv2 or later | Wish velocity, acceleration, friction, multi-plane clipping, step comparison |
| [Source SDK 2013](https://github.com/ValveSoftware/source-sdk-2013) | `22288b919617be6c8ca3cefd7cca979cbb39a88c` | Restrictive Source SDK license | Mature FPS feel, slide/step/crouch/contact behavior |
| [Godot](https://github.com/godotengine/godot) | `7216a6290065f79d2826d7bd35812add1f513eb8` | MIT | Floor classification, snap, slide facts, platform carry/departure |
| [Rapier](https://github.com/dimforge/rapier) | `3e12c2679cb1940a876bde93af9cec0cf2f57944` | Apache-2.0 | Host-neutral casts, recovery, slopes, stairs, snap, hit reporting |
| [OpenKCC](https://github.com/nicholas-maltbie/OpenKCC) | `a1a30ed7f7722ea82a1df6bd01849e0bfde6abf4` | MIT | Projection pipeline, overlap recovery, state seams, moving-ground policy |
| [bevy_fps_controller](https://github.com/qhdwight/bevy_fps_controller) | `572b8eb786295f61bb272f9a223ff6b88a0fc115` | MIT or Apache-2.0 | Source-like movement policy in Rust |

Quake III, Doom.ts, and Source SDK are behavioral references only. The design
is an independent implementation even where a permissive source would allow
reuse. Source-specific constants and quirks are not Engine defaults merely
because a mature game used them.

## Pre-implementation Rusty Engine baseline

Before task 6847 implementation, Rusty Engine had the right authority split but
not the required controller:

- `svc-collision` owns canonical voxel and admitted static-mesh collision
  projections. Existing public motion queries are conservative Boolean swept
  AABB tests; the static-mesh internals already use richer shape casts.
- `engine-spatial::KinematicMotionSystem` performs fixed X/Y/Z AABB motion and
  reports axis blocking. It has no contact normals, time of impact, capsule,
  slope, step, floor snap, stance, platform, or corner solver.
- `engine-spatial::FirstPersonMotionService` is a free-fly pose integrator with
  optional entity AABB checks. It is useful for noclip and camera-relative free
  motion and must retain its existing semantics.
- `engine-spatial::RigidBodyService` demonstrates the desired off-side
  prepare/atomic-commit model and exact entity/environment revision guards.
- `entity-state::KinematicComponent` stores only half-extents and velocity.
  `ControllerComponent` means process/subject control and is not available as a
  movement-controller name.

The adopted implementation therefore added richer collision queries and a
separate named service. It did not expand either existing motion service.

## Cross-source capability matrix

`Strong` means the source contains a substantial mechanism. `Policy` means it
mainly supplies feel or product choices. `Partial` means it helps but cannot be
used as the complete reference.

| Capability | Strong references | Partial or policy references | Adopted transfer |
|---|---|---|---|
| Explicit fixed-step command | Quake III, Source | Doom.ts | Typed command plus caller-supplied fixed `dt`; no scheduler |
| Yaw-relative wish velocity | Quake III, Source | Daggerfall, bevy | Camera-independent yaw plus normalized planar intent |
| Ground acceleration, friction, braking | Quake III, Source, bevy | Doom.ts | Neutral configurable movement policy, separate from collision |
| Air acceleration and speed cap | Quake III, Source, bevy | Daggerfall | Explicit parameters; bunny-hop behavior remains configured policy |
| Capsule cast and recovery | Rapier, OpenKCC | Godot | Typed cast/overlap port with bounded depenetration |
| Multi-plane wall/corner slide | Quake III, Source, Rapier | Doom.ts, Godot | Remaining-time solver, repeated-plane handling, crease motion, bounded stop |
| Ground/wall/ceiling facts | Godot, Rapier, OpenKCC | Quake III, Source | Normal-based typed classification, not Boolean flags |
| Slope climb/slip | Rapier, Godot | OpenKCC, bevy | Configurable standable angle and steep-slope result |
| Ascending/descending steps | Rapier, Quake III, Source | OpenKCC, Doom.ts | Compare direct and raised candidates; validate landing slope |
| Floor adhesion/snap | Rapier, Godot, OpenKCC | Daggerfall | Bounded snap disabled during intentional upward motion |
| Crouch and stand clearance | Daggerfall, Quake III, Source | bevy | Foot-preserving capsule profile; stand only after clearance succeeds |
| Jump buffer and coyote time | — | Mature feel conventions; Daggerfall grounded grace | Small deterministic timers with downstream-selected durations |
| Moving platforms | Godot, Daggerfall, OpenKCC | Rapier, Source | Stable support identity, point velocity, carry and one departure policy |
| Dynamic-body interaction | Rapier | OpenKCC, Godot | Read-only obstacle motion plus typed impulse proposals; no dynamic authority |
| External velocity/impulse | Source base velocity | Daggerfall | Explicit additive contribution that preserves steerable player velocity |
| Typed contact diagnostics | Godot, Rapier, OpenKCC | Source, Quake III | Bounded stable facts retained after the solve |
| Origin/world revision change | Daggerfall | Current Engine revision model | Reject stale prepared results; explicit state-rebase operation |
| Swimming/climbing/noclip | Daggerfall | Quake III, Source, bevy | Downstream modes using neutral query/motion seams; not core controller policy |

## Important source findings

### Quake III and Source

Both separate command input, wish direction/speed, acceleration, and actual
velocity. Quake's `PM_Accelerate` projects current velocity onto the wish
direction before adding only the missing speed (`code/game/bg_pmove.c:240-267`).
Its slide solver consumes remaining time, accumulates clip planes, moves along a
two-plane crease, and stops at a triple-plane constraint
(`code/game/bg_slidemove.c:44-235`). `PM_StepSlideMove` compares a direct slide
against a raised candidate rather than always forcing a step
(`code/game/bg_slidemove.c:240-315`).

Source retains the same broad vocabulary but adds mature stuck handling,
endpoint validation, crouch transitions, impact collection, and platform base
velocity (`src/game/shared/gamemovement.cpp`). These are valuable behavioral
checks, but its license makes independent reimplementation mandatory.

Transfer: the decomposition, bounded solver shape, and test cases. Keep
friction, air cap, jump behavior, crouch speed, and compatibility quirks as
configuration rather than universal constants.

### Rapier, Godot, and OpenKCC

Rapier is the strongest geometric reference. Its kinematic controller combines
shape casts, contact offset, recovery, slope classification, autostep,
floor-snap, moving-parent velocity, and rich collision callbacks
(`src/control/character_controller.rs`). Its dynamic-body impulse transfer is
explicitly approximate, which supports keeping that concern outside the
canonical geometry solve.

Godot provides the strongest public fact vocabulary: travel and remainder,
floor/wall/ceiling normals, collider identity and velocity, floor snap, and
platform departure policy (`scene/3d/physics/character_body_3d.cpp`). It does
not provide FPS acceleration, crouch, or stairs in this surface.

OpenKCC makes casts, overlap push-out, bounce history, grounded state, stair
tests, and moving-ground attachment visible as separate responsibilities. Its
Unity lifecycle, transform parenting, input, animator, and layer-mask concerns
do not transfer.

Transfer: Rapier-like query and stair mechanisms, Godot-like typed outputs, and
OpenKCC-like responsibility seams. Do not expose Rapier, Godot, Unity, or ECS
types through the Engine API.

### Daggerfall Unity and Doom.ts

Daggerfall's `PlayerMotor.FixedUpdate` is an instructive orchestration pipeline:
select locomotion mode, compute ground or air movement, apply gravity, move with
platform carry, then update contacts. `PlayerHeightChanger` preserves the feet
while changing capsule height and blocks standing with an upward clearance
cast. `PlayerGroundMotor` carries a local platform anchor and rotation.

The actual collision solver is Unity's `CharacterController`, so Daggerfall is
not evidence for implementing casts or corner resolution. Its climbing,
swimming, levitation, camera, skills, damage, and content logic are downstream
policy. Its floating-origin code does establish that cached support, fall, and
contact state must be explicitly rebased or invalidated.

Doom.ts contributes fixed-tick momentum, post-move floor friction, splitting of
large moves, and bounded slide retries. Its collision world is 2.5D: XY circles
and map lines plus scalar sector floors and ceilings. Its 24-unit step rule,
separate XY/Z clipping, and sector openings do not transfer to arbitrary 3D
capsules and slopes.

### bevy_fps_controller

This is a useful Rust example of Source-inspired wish velocity, friction,
ground/air acceleration, air speed caps, crouch, sprint, and noclip. It delegates
general collision resolution to a dynamic physics body and documents weak
capsule stair behavior. It is therefore a feel reference, not the Engine
controller architecture.

## Chosen semantics

The survey supports these adopted choices:

1. Use a local +Y capsule and true swept-shape queries, not an AABB or dynamic
   rigid body.
2. Separate movement policy from geometric constraint solving while exposing
   one convenient named service that composes them.
3. Keep requested planar intent, controlled velocity, external velocity, and
   accepted displacement distinct.
4. Classify ground, wall, ceiling, steep slope, step, and platform from typed
   contacts. Do not infer them from axis-blocked Booleans.
5. Use bounded multi-plane slide, explicit depenetration, direct-versus-step
   candidate comparison, and conditional floor snap.
6. Treat crouch as a capsule profile transition with a stand-clearance
   transaction and foot-preserving pose adjustment.
7. Treat moving support as identified world state with velocity at the contact
   point. Apply carry before player motion and a configured departure
   contribution once.
8. Observe dynamic obstacles but return impulse proposals rather than mutating
   rigid bodies from the controller.
9. Make jump buffer and coyote time deterministic configuration. Engine owns
   the timer mechanism; downstream owns duration and whether either is enabled.
10. Preserve fixed-step and stable input ordering, and bind every prepared
    result to the exact collision-world and entity revisions it observed.

## Explicit exclusions

- No DOM, key codes, mouse events, renderer camera, audio, animation, damage,
  stamina, footsteps, or material meaning.
- No universal scheduler, ambient input/event bus, ECS lifecycle, service
  locator, or hidden global controller registry.
- No embedded swimming, climbing, ladder, mantling, sprint, noclip, vehicle, or
  ability rules. Downstream may replace or constrain ordinary locomotion.
- No dynamic rigid-body authority. The controller cannot secretly become a
  second physics solver.
- No transform parenting as platform state and no persistence of collision
  backend handles.
- No exact Source/Quake/Doom constants as unexplained Engine defaults.
- No promise of invariance across arbitrary variable-step partitions. The
  supported contract is deterministic execution for the same bounded fixed-step
  command stream and world revisions; partition sensitivity is measured and
  bounded in tests.

## Survey conclusion

No single donor is a suitable union-and-port target. The coherent design is a
Rapier-informed capsule query and constraint solver, Quake/Source-informed
movement policy, Godot/OpenKCC-informed facts and platform seams, and
Daggerfall-informed stance/mode/origin handling. That combination is
implemented as a direct, host-neutral service while gameplay meaning and
scheduling remain downstream.
