# Portable assets and joint attachment proof

These original browser captures exercise ordinary packaged C# fixtures against
matching local contributor SDK/runtime builds. They are source implementation
proof; exact immutable release-pair verification is recorded in the Den review
handoff after publication. `index.json` retains original capture paths and IDs.

- `sprite-nativeaot-blue.png` and `sprite-nativeaot-red.png`: centered viewport
  atlas changes under Engine playback. `portable.inspect` confirmed equivalent
  loose/bundle facts, ordered 0.2/0.4/0.2 timing, missing right direction, three
  glTF clips and three independently reopened model references (local SDK .1).
- `attachment-coreclr-pose0.png` and `attachment-coreclr-pose05.png`: body and
  separate yellow weapon loaded through the descriptor. The weapon changes
  with the hand between two sampled run poses (local SDK .3).
- `attachment-coreclr-reloaded.png`: same pose after two release/reload cycles.
  Mesh admission is asynchronous: an immediate capture may precede readiness;
  the scene returned after one second. MissingHand8647 was rejected with the
  actual target object 8654 and the valid scene remained usable.

The independent observer opened original images and stopped all sessions.
There were no page errors in successful runs. Chromium reported GPU ReadPixels
stall warnings during capture. No compatible warning baseline exists, so this
is report-only evidence, not a clean warning-delta claim. An earlier blank
attachment run was traced to a stale application-host build artifact; rebuilding
the full browser closure resolved its unsupported-operation error.

Validation: full Rust verification, packaged C# verification, renderer compiled
suite and all 54 browser tests, documentation/routing checks, and reproducible
browser artifact freshness passed. Focused regressions cover native diagnostic
rollback, retained baseline restoration and actual animated bone parenting.

NativeAOT SDK .3 repeated the same attachment mission: pose 0 and 0.5,
MissingHand8647 rejection, two reloads and a usable pose after reload.
The corresponding `attachment-nativeaot-*` originals are indexed alongside
CoreCLR. After reload the first one-second capture was blank; the scene was
visible after one additional second. Both sessions were stopped successfully.
