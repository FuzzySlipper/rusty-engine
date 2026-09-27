# Direct packaged CoreCLR hosting (#8686)

## Reproduction and decision

The accepted pair `6edeecefb13c8c7eac2ceb5ae290db9b40d58846` reproduced both
reported failures: the voxel fixture exited -2 on SIGINT, and CraftSurvive
exited -11 before its batch probe. The included process results and system
core metadata record these failures; neither is a passing shutdown.
The CoreCLR SIGSEGV itself has not been root-caused, and no common cause with
the signal-handler issue is asserted.

Ordinary packaged CoreCLR launches now reuse the existing foreground shell
and managed worker. This removes the reproduced in-process ordinary lane,
keeps signal ownership outside CoreCLR, and reports worker failure without
silently replaying gameplay. Explicit finite probes and contributor raw
artifacts retain their internal in-process path. NativeAOT is unchanged.
Final product-owner disposal is now relayed before worker exit as well as
lifecycle shutdown; a unit regression covers that distinct final event.

## Contributor candidate proof

These artifacts use the accepted packaged SDK/products with the locally built
candidate host substituted into a copied runtime pack. They are source proof,
not proof of a published new immutable pair.

- `candidate-signals.json`: direct packaged fixture SIGINT and SIGTERM each
  exit 0 in about 0.12 seconds, with exactly one `DISPOSED` diagnostic.
  Deliberately killing its worker yields exit 1 and named worker EOF/exit
  diagnostics; no disposal is claimed for that forced crash.
- `candidate-supervised-disposal.ndjson`: the existing supervised stdin-close
  path still exits 0 and emits disposal once.
- `candidate-native-smoke.json`: NativeAOT listener startup and SIGINT exit 0.
  This is a lifecycle smoke check, not new NativeAOT gameplay proof.
- `candidate-craft-proof.txt` and result: direct CraftSurvive without debugger
  completes all fourteen water/stone transactions, reaches update 200 and
  swimming immersion 0.876, then exits 0 on SIGINT.
- `candidate-browser-observation.json` and three original PNGs: an independent
  browser observer saw terrain/water after initial loading on the preceding
  worker-shell candidate. Its W input attempt did not establish visible motion;
  no movement assertion is made. No page errors; Chromium ReadPixels warnings
  occurred during captures. Warning comparison has no compatible baseline and
  no clean warning-delta claim is made.

Validation: 42 runtime-library, 39 host and 64 product-dev-host tests passed;
focused clippy with warnings denied and documentation verification passed.
Exact immutable pair publication, repeated packaged proof and review follow
this source commit and are recorded in the Den task handoff/review packet.

## Review finding R8686-1: foreground process-group signals

The original PID-only signal tests missed terminal-style delivery. On the
published `4087584f5dab14b52fedbd0e9ce241a5d99414a9` pair, launching the fixture
in a fresh session and signalling the shell's whole process group reproduced
exit 1 with zero disposal events (direct SIGINT/SIGTERM and supervised SIGINT).
See `group-signal-baseline.json`; the earlier PID-targeted proof does not
establish Ctrl+C behavior.

The worker now starts in its own Unix process group. The foreground shell
receives terminal/group signals and stops the worker through its existing
protocol, preserving disposal. `group-signal-proof.py` is the runnable Linux
packaged-fixture regression: it launches fresh sessions, confirms distinct
shell/worker groups, calls `killpg`, checks exit 0 and exactly one `DISPOSED`,
and confirms the worker was reaped. Both direct and supervised SIGINT/SIGTERM
pass with the candidate host (`group-signal-candidate.json`). Host tests
(39) and clippy with warnings denied pass. This candidate evidence precedes the
corrected immutable pair; exact-pair repetition is recorded in Den.
