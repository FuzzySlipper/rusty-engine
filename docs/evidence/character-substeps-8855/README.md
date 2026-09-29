# #8855: long character steps are sub-stepped, not refused

## Decision

The character controller refused any step longer than 1/15 s, while the
runtime admits fixed steps down to 1 Hz.
- **The bound is real.** At the 55 m/s terminal fall and the 10 m
  displacement envelope, and for jump arcs and friction, the solver needs
  short intervals.
- **Where it belongs.** The bound is the solver's own, so it should be the
  controller's concern, not the product's.

`engine-spatial` (`7a791b431`) now takes any step from 1 ms to 1 s:
- **Sub-steps.** A step longer than 1/15 s is solved as equal sub-steps of
  at most 1/15 s. Steps up to 1/15 s solve exactly as before.
- **The receipt covers the whole command.**
  - Transform, motion, ground, stance and movement come from the last
    sub-step.
  - Contacts (capped at `maximumContacts`), blocks, dynamic impulses, cast
    counts, query statistics and recovery accumulate.
  - An accepted climb stays reported.
  - Tether events and impulses combine.
- **Once per command.** A jump press and an external impulse go to the first
  sub-step only. Platform carry happens once, on the first sub-step, over
  the whole command's duration, so the support's point velocity is not
  overstated.
- **The range.** 1 s is the runtime's slowest fixed step (1 Hz), fifteen
  sub-steps. A longer step is still `InvalidCommand`, so an accidental huge
  delta faults instead of hanging.

The runtime-side alternative, refusing fixed steps below 15 Hz, would tie
the lifecycle to one Engine service. It was not taken.

## Tests

`engine-spatial/tests/character_controller.rs`:
- **`fixed_step_partitions_are_repeatable_and_near_equivalent`.** One second
  of walking at 30, 10, 4 and 1 Hz ends within 0.2 m (horizontal) and 0.05 m
  (vertical) of 60 Hz.
- **`a_long_step_matches_the_same_interval_in_solver_steps`.** A 0.2 s
  command matches three 1/15 s commands to 1e-4. Its receipt runs from
  before the first sub-step to after the last, and sums their cast counts.
- **`one_hertz_steps_fall_land_and_jump_within_the_solver_envelope`.**
  - A 58 m drop in 1 s commands lands in the third one. An unsplit second
    would ask for 20 m, beyond the 10 m envelope.
  - A jump over a half-second command follows its arc: 0.8–1.6 m up and
    past the peak, where an unsplit step would rise 3.5 m.
- **`steps_are_admitted_from_a_millisecond_to_a_second`.** 0.5 ms and 1.01 s
  are refused; 1 ms, just over 1/15 s, and 1 s are admitted.

`csharp-engine-services`: the validation test now admits 0.5 s and refuses
1.5 s. `cargo test -p engine-spatial -p csharp-engine-services -p
csharp-product-runtime`, `cargo fmt --check` and stable clippy `-D warnings`
pass.

## Downstream

- **Underworld** (`76c5f45`, pair `0.1.0-dev.7a791b43134e`) drops its mirror
  of the controller's private bounds (`MaximumStepSeconds`,
  `MinimumStepSeconds`, `MaxSubsteps`), its eight-proposal loop, and the test
  that guarded them. Each fixed step is one proposal.
- **Dagger** already proposed once per admitted fixed step, so a hitch is
  several 1/60 s steps. It moved to the same pair (`df9658a`) with no source
  change.
- **Both products** build, pass their suites, and run streamed on the pair
  under `rusty dev`.

**Keyboard walking in Underworld.** The #8835 evidence reported that
physical keys did not move the avatar in a headless run. That was a product
bug, not a headless limit:
- `UuLocomotionPolicy` read the frame's movement from `FpsInput` but never
  put it in the step's `PlanarIntent`, so the controller always saw zero
  intent.
- Underworld `dcdbddf` passes it through. On this pair, W, A, S and D then
  moved the avatar 3.32, 2.84, 3.32 and 2.84 m in 0.8 s each, back to the
  start (`abyss.where`), through the single-proposal path.
