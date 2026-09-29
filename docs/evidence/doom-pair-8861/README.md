# Doom on the current pair (#8861)

rusty-doom moved from pair `f1d747afc01c` to `b13d4a897b41` in commit
`3ab47cf` on rusty-doom main. That pair is past #8840 (borrowed results, no
trigger revisions) and includes #8841, #8849 and #8857.

The product-side notes are in rusty-doom
`docs/agent-review/engine-pair-b13d4a89-20260929.md`. The same commit also
closes rusty-doom #8845.

## Migration

The #8840 notes ([indexed reads](../indexed-reads-8840/README.md)) apply as
written:

| Before | After |
|---|---|
| `SpatialTriggerFactAtReceipt` with `Present` | `SpatialTriggerFact` over `Facts.Span` (canonical pickups, world triggers, recipe gameplay) |
| `ReadTriggerFactAt(i)` for `i < FactCount` | `receipt.Facts.Span` |
| `ReadTrigger` + `ReadTriggerOverlapAt(i)` with a revision recheck | `ReadTrigger(...).Subjects` |
| `SpatialTriggerLifecycleReceipt.RevisionBefore/After` | `SpatialTriggerLifecycleResult` (no revisions) |
| `SpatialTriggerRestoreReceipt.RevisionBefore/After` | removed |
| `NavigationStepReceipt.PathLen` | `NavigationStepResult.Path.Length` |
| `AnimationRealizationReadout.RetainedFactCount` | `AnimationRealizationResult.Facts.Length` |
| `PresentationFactsReadout` | `PresentationFactsResult` |

The pair notes for `2d8af40ada74` and `b13d4a897b41` call for no product
change.

## Checks

On `b13d4a897b41`, Doom's own checks all pass:

- `scripts/verify-csharp-spine.sh`: build, lifecycle and recipe animation
  exercises, NativeAOT publish;
- `scripts/audit-boundary.mjs`;
- `scripts/audit-active-guidance.mjs`;
- `scripts/check-canonical-projects.mjs`.

## Stream mode (`RUSTY_RENDER_OUTPUT=stream`)

This was a crew-services `rusty-doom` session with an unchanged profile. The
responses in `responses/` are trimmed to the presentation facts.

**Observer and drawn cameras (the #8841 follow-up).**

| Step | Call | `views.cameras[0]` | `views.sourceCameras[0]` |
|---|---|---|---|
| 1 | `capture {engine_presentation:true}` | product pose `(-7, 1.62, 3)`, yaw 0, pitch 0, `observer: false` | same pose |
| 2 | `camera {position:[-7,14,14], yawDegrees 0, pitchDegrees -45}` → `capture` | `(-7, 14, 14)`, pitch −45, `observer: true`, forward `(0, −0.707, −0.707)` | unchanged product pose |
| 3 | `camera null` → `capture` | back to the product pose, `observer: false` | unchanged |

- The world was held in `action-driven` time, so the product camera did not
  move between captures.
- `views.cameras` reports what the frame drew from. `sourceCameras` stays the
  product's authored camera throughout (`responses/01`, `03`, `05`;
  `captures/01`, `02`).

**Trigger facts, room-study recipe gameplay (default scene).**
- Ordinary `act forward` in realtime walked spawn → western shotgun.
- The migrated `Facts.Span` loop collected shells on the way, then the
  shotgun: `collected` 0 → 2, weapon Shotgun, shells 16, HUD `ITEMS 2/16`
  (`captures/03`, `responses/06`).

**Trigger facts, canonical path (`LOADING_BAY_SCENE=legacy-voxel`).**
- That scene registers no `playtest.*` modules, so raw held keys (XTest-style
  virtual keys through `playtest input`) were used.
- They walked onto health bonus 78. Health went 100 → 101 through
  `SpatialTriggerFact` → collect → `SetTriggerActive` (`captures/04`).
- The nukage hazard is in the courtyard, outside the starting room, and was
  not reached. The `Subjects` hazard read is compile-checked only.

## Window mode (`RUSTY_RENDER_OUTPUT=window`)

- The pack was the `--desktop` runtime pack built for #8790/#8791 (source
  `fbc78ee4c`). Its ABI fingerprint `6297d14a…` is identical to
  `b13d4a897b41`'s. No published pair ships a desktop pack; #8860 owns that.
- It ran in the private headless KWin from
  [desktop-shell-8790](../desktop-shell-8790/scripts/headless-compositor.sh),
  on Wayland.
- The world, sprites, viewmodel and the whole TypeScript HUD draw in the window
  (`captures/05`), and `engine.renderer.status` reports
  `render-wgpu (desktop window)`.

## Found, not changed here

- **#8865.** `navigation.route` returns `NoPath` from spawn for every
  room-study target.
  - An A/B run of Doom's previous commit on `f1d747afc01c` gives the same
    result, so the move did not cause it.
  - #8399 had fixed spawn → west shotgun, so this is a regression.
  - `responses/07` is from the same session on the intermediate pin
    `4ad84170df9a`.
- **#8843.** In `action-driven` time, `act forward` is accepted and the player
  does not move. Reproduced here unchanged.
