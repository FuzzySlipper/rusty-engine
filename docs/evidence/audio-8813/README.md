# Listener pose and entity emitter positions on the device path (#8813)

Campaign #8782, audio lane, 2026-09-29.

## Change

- **`PresentationWorld::entity_world_position(entity)`** (`render-presentation/src/world.rs`)
  returns the world origin of the committed graphics node whose
  `RenderMetadata::source_entity` is the entity. `EntityGraphicsProjection` and
  `Graphics.PublishSnapshot` publish each row with its entity id there. The
  position is composed through the node's parents. A joint attachment uses the
  parent node's transform, not the joint's; an entity with no node gives `None`.
- **`EngineServiceSet::entity_world_position` and `view_composition`**:
  read-only accessors.
- **`AudioRealizer::set_listener_pose(position, forward, up)`** replaces the
  quaternion-only `set_listener`. It builds kira's listener orientation (+X
  right, +Y up, −Z forward) from a camera basis.
- **`csharp-product-runtime/src/audio_output.rs`**:
  - After each committed call, the listener follows the camera of the
    lowest-ordered primary view in that call's `ViewComposition` publication.
    It uses the explicit basis when present; otherwise yaw 0 faces −Z, positive
    yaw turns right and positive pitch looks up, the same convention as the TS
    `renderer-listener-pose.ts`.
  - A baseline reads the committed composition once.
  - A composition with no primary view leaves the listener where it was.
  - Entity-attached voices are placed and followed through the world query, on
    every call and before every update.
- **Browser `RendererAudioHost`:** not wired. #8792 deletes it.

No product API changed.

## Evidence

- **`render-presentation`:**
  `entity_world_position_composes_parent_transforms_and_follows_updates`. A
  child at (1, 0.5, 0) under a parent at (10, 0, 0), rotated 90° about +Y and
  scaled ×2, lands at (10, 1, −2). It follows a parent `Update`. An unknown
  entity gives `None` (the test caught an early version returning the origin).
- **`render-audio` capture tests**, which render through kira's real mixer:
  - `the_listener_pose_pans_and_attenuates_world_emitters`: an emitter at +Z
    is louder on the right while facing +X and on the left while facing −X, and
    quieter from 15 m than from 3 m;
  - `an_entity_attached_voice_follows_its_entity`: panning flips when the entity
    crosses.
  - render-audio: 16 passed.
- **`csharp-product-runtime`:** `the_listener_is_the_lowest_ordered_primary_view_camera`
  covers view choice over an offscreen view and a later primary view, yaw 90 →
  forward +X, and pitch −90 → forward −Y with up −Z.
- **Device run.** The Engine fixture `csharp-audio-containers` gained
  `audio.proof.face`, `audio.proof.world` and `audio.proof.entity`, which use
  only `CameraView`, `Graphics` and `Audio`. It ran with
  `RUSTY_AUDIO_OUTPUT=device` into a null sink (`spatial-live-debug.txt`,
  `spatial-recording.txt`). Per-channel RMS per 0.25 s window:

  | Step | Left | Right |
  |---|---|---|
  | World emitter at (3, 0, 0), camera yaw 0 (facing −Z) | 0.0008 | 0.0197 |
  | Camera yaw 180 | 0.0197 | 0.0008 |
  | Camera yaw 90 (emitter straight ahead) | 0.0139 | 0.0139 |
  | Entity 900 at (3, 0, 0), yaw 0 | 0.0008 | 0.0197 |
  | Entity moved to (−3, 0, 0) | 0.0197 | 0.0008 |
  | Entity moved to (0, 0, −15), 15 m ahead | 0.0002 | 0.0002 |

  Silence follows each `audio.proof.stop`.

## Differences that remain

- Falloff is kira's linear 1..`attenuation`, as in #8789.
- Joint-attached nodes use their parent node's position.
- Position updates land once per committed call, not per rendered frame; the
  Engine publishes positions at that cadence anyway.
