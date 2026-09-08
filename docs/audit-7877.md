# Diagnostic serialization audit

Task **#7877**, 2026-09-07. This records the narrow dispositions for timeline
inspection and retained runtime diagnostics. It does not redesign the host
writer, worker transport, browser route, or ring retention.

## Removed preflights

`runtime-timeline::TimelineCatalog::new` constructs and retains a typed
`RuntimeTimelineInspection`. It no longer serializes that inspection merely to
reject the catalog by an inspection-byte quota. The explicit
`inspection_json_newline()` API remains available for a caller that actually
needs JSON, and serializes only at that call.

`runtime-diagnostics::RuntimeDiagnosticsSink::read_after` now copies retained
events directly into a page. It no longer serializes every candidate event while
holding the sink lock merely to apply a 128 KiB quota, before the host or worker
serializes the returned typed batch for real delivery.

## Retained contracts

The diagnostic ring still retains 256 events by default and accepts at most
1,024. On full retention, publishing one new event evicts the oldest and
increments `droppedCount`. A supplied cursor below the retained floor remains
`lagged`; `floorSequence`, `throughSequence`, and `nextCursor` keep their
existing meanings. Recoverable rejections still coalesce by code before a
sequence is allocated.

A page remains a 64-event cursor page. That is scheduling for the established
readers: the worker has one shared drain cursor, while the local debug client
and warning-delta capture keep private cursors. It is not a byte capacity or a
claim that every 64-event page is inexpensive to serialize.

Actual delivery remains at its owners. `product-dev-host` encodes NDJSON when
the optional file writer writes a line and retains its line/rotation policy. The
host encodes the HTTP diagnostics response, and the worker encodes its framed
message with its existing `u32` length representation. None of those writers or
their policies move into `runtime-diagnostics`.

## Regression evidence

The `runtime-timeline` regression creates a catalog with all 256 allowed steps
and large opaque payloads. Its explicit inspection JSON is more than 1 MiB after
construction, while the catalog retains the same typed inspection. That
measurement proves only that catalog admission no longer uses the removed
inspection-format quota.

The `runtime-diagnostics` regression creates 65 individually valid, dense
events. Its first 64-event page encodes to more than 128 KiB in the test, yet
the retained page advances from cursor 0 through 64 and the next page returns
event 65. The test measures that former threshold only to prove the removed
preflight; it does not establish a new diagnostic payload budget.

The worker-to-shell diagnostic `sync_channel(16)` and its `try_send` handling
are separate delivery behavior. It was not changed here; the former 128 KiB
preflight did not guarantee that channel's capacity because ordinary small
events could already form a 64-event page.

The separate shell queue loss finding is tracked by Engine #7878.
