# Held-time input reaches the steps `engine.time.advance` runs (#8843)

## Cause

A realtime product's input POST goes into the host's input mailbox. The host
never takes the runtime lock at the input edge, so a slow product update cannot
hold up browser input. The realtime scheduler drains the mailbox at each tick,
before it advances.

In held playtest time (`manual` or `action-driven`), the runtime reports its
schedule as paused, so the scheduler does not tick. `engine.time.advance` ran
its manual steps without draining the mailbox. The harness's key-down therefore
waited in the queue through the advance, and down and up both reached the
product only once realtime resumed.

It was neither the renderer (Three and streaming alike) nor Doom's migration.

## Fix

`product-dev-host` now hands the queued input to the runtime before every
live-debug command, inside the same runtime lock:
- `scheduler::deliver_queued_input` is the drain that the realtime tick already
  performed, now shared by the tick and the debug route;
- input receipts publish on the SSE output family as for a tick;
- in realtime the input simply joins the next tick's snapshot, as it would
  have.

`docs/playtest-inspection.md` states the rule.

## Evidence

Doom `3ab47cf` on a runtime pack built from this change, stream mode.

**Without the browser.** `held_probe.py` posts a physical `key-w` press to
`/__rusty/product/runtime/input`, advances time (`engine.time.advance 500` in a
held mode, 0.5 s wall time in realtime), then posts the release:

| Mode | Pair `b13d4a897b41` (before) | This change |
|---|---|---|
| realtime | z 3.00 → −0.91 | z 3.00 → −1.01 |
| action-driven | 30 steps, no movement | z −1.01 → −3.99 (3.0 units) |
| manual | 30 steps, no movement | z −3.99 → −6.99 (3.0 units) |

3.0 units is Doom's 6 units/s for 0.5 s.

**Harness (`playtest assist … {"op":"act","id":"forward","ms":500}`).** Before
the fix: accepted, `advancedMs: 500`, `distanceMoved: 0`, handback
`no-observed-movement` (#8843's report, reproduced in #8861). With the fix:
`distanceMoved` 2.98 in `action-driven` and 3.00 in `manual`.

**Test.** `product-dev-host`
`a_debug_command_takes_the_input_queued_before_it`: a held realtime runtime
receives the queued batch before the `engine.time.advance` command, and the
mailbox is empty afterwards.
