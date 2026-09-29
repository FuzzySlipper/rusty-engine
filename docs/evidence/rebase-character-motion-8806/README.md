# Character motion across a world-origin rebase (#8806)

## Question

C# products hold `CharacterMotion` themselves. After a world-origin rebase,
are its local-frame values wrong on the next `ProposeCharacterStep`? #8754
deleted the Rust `EntityState` path that used to rewrite them, which no host
used.

## Experiment

The bridge test `spatial::tests::rebased_character_motion_needs_the_origin_delta`
runs through the C# bridge. It commits far rebases, shifts the product's own
positions by the commit receipt's cell delta (as rusty-dagger does), then
steps once with the motion unchanged and once with its frame-dependent values
shifted.

| Case | Motion left as it was | Motion shifted by the delta |
|---|---|---|
| Standing on a moving platform, rebase by −4096 in X, platform moves 0.2 | carry −4095.8: the character snaps back by the whole delta | carry 0.2 |
| Fixed tether reeled to 2.5, rebase by −4096 in X | re-attaches at full length: 2.9 after the step | keeps reeling: 2.4 |
| Falling from 12, rebase by −8 in Y | peak stays 12 while y ≈ 4: an 8-unit overstated landing | peak 4 |

The collision world hash does **not** change on a rebase, so the controller
does not clear support. Nothing hides the carry failure.

Why each case fails:
- **Support carry:** the controller carries by the support's current transform
  minus `SupportPreviousTranslation`.
- **Tether:** a fixed tether continues only while `TetherAnchorPoint` equals
  the request's anchor.
- **Fall:** `PeakY` and `FallOriginY` are reported heights that products turn
  into landing distances (rusty-dagger's `DaggerfallLanding`).

## Decision

The product applies the delta. The Engine already returns it: the commit
receipt carries the origin cell before and after. The frame-dependent fields
are Engine knowledge about an Engine value, so the SDK now carries them:

- `WorldOriginCommitReceipt.LocalDelta`: before cell minus after cell.
- `CharacterMotion.Rebased(localDelta)` shifts `SupportPreviousTranslation`,
  `TetherAnchorPoint`, `FallOriginY` and `PeakY`.
- `EntityOriginRebaser.Commit` applies `Rebased` to every stored
  `CharacterMotion`. Before, the SDK's own composition (with
  `EntityCharacterController`) showed the carry failure after every rebase.

Not chosen:
- **An Engine-side motion rewrite**, for example motion rows in the rebase
  request. It would move product-held state into the ABI for arithmetic that
  the delta already enables.
- **Doing nothing.** Three concrete failures, including a 4 km teleport.

Products that hold motion outside `EntityStore` call
`motion.Rebased(receipt.LocalDelta)`. Values they pass in each step, such as a
fixed tether anchor and support/obstacle transforms, move by the same delta,
like any other position. `docs/csharp-helpers.md` says so.

## Evidence

- The bridge test above; `cargo test -p csharp-engine-services` passes.
- `Rusty.Engine.Entities.Example` checks that `EntityOriginRebaser.Commit`
  rebases a stored `CharacterMotion` (its support anchor moves from X=100 to
  X=0). With the shift removed, the example fails with "world-origin commit
  left stored character motion in the old frame".
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes. No ABI change.

## Downstream

rusty-dagger's `ApplyExteriorOriginCommit` shifted player and actor positions
but not `PlayerControl.Motion`. That is the unshifted column above: the first
step after a live exterior rebase on a support would snap the player back.
Dagger now rebases the motion too.
