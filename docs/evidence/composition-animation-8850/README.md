# Animated poses refresh cached composition targets (#8850)

**Finding (confirmed on current source).**
- An offscreen composition target is redrawn when `scene_generation` differs
  from the generation it was drawn at. Three things bump that generation:
  scene deltas (`apply`), presentation ops, and particle advances.
- Animation posing on Engine time (`set_animation_time` →
  `advance_animations` → `pose_animated_instance`) did not bump it.
- So a repeating clip rewrote two part rows, but drew no offscreen view. The
  target stayed `Current` and kept showing the old pose.

**Fix** (`render-wgpu/src/animated.rs`).
- `pose_animated_instance` compares the newly evaluated joint matrices with the
  previous pose. If they differ, it bumps `scene_generation`, which is the
  existing invalidation signal.
- A held, sampled or finished pose evaluates to the same matrices, so its
  targets stay cached.
- No new clock, guard or signal is added; Engine time stays authoritative.
- `composition.rs`'s module comment now lists changed poses among the reasons
  a target is stale.

**Regression test.**
`tests/animated.rs` `an_advancing_pose_redraws_a_cached_composition_target_and_a_held_pose_reuses_it`
presents the joint-attachment character through an offscreen target:
- first frame: one offscreen view;
- sampled pose with Engine time advanced to 1.0: zero offscreen views and
  identical pixels;
- repeating `run` playback with Engine time advanced to 1.2, with no delta
  and no republished composition: one offscreen view, changed pixels, and the
  target `Current`.

Without the fix, the last step fails with the review's symptom:
`parts_uploaded: 2, offscreen_views: 0`.

**Checks.** `cargo test -p render-wgpu` passes all 68 tests, including the
screenshot suites. Clippy `-D warnings` is clean.
