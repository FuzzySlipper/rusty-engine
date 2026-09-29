# First live-debug command without a browser (#8801)

## Cause

Before a browser attaches, the host's output bus has no active binding. Only
an SSE connection, or an operation that publishes a complete baseline, sets
one.

The scheduled input and realtime paths each checked for this and dropped their
outputs. The request paths (debug execute, input, lifecycle) did not. They
passed their outputs to `encode_output_batches`, which rejected any incremental
output with no binding: `DEV_HOST_OUTPUT_BASELINE: incremental output arrived
before a complete binding baseline`. By then the debug command had already run,
yet the caller got a 503.

Only the first command failed because the product runtime attaches its readout
to a successful debug result. `changed_readout` marked that readout as
published even though publishing failed. Later commands then had nothing new
to publish, so they returned early and succeeded. A later command whose
readout changed would have failed too.

## Change

`encode_output_batches` now drops incremental outputs while no binding is
active. There is no subscriber state for them to apply to, and the next
connection starts from its own complete baseline. That is what the scheduled
paths already did, so their two separate pre-checks are removed. Every other
publication failure still fails and fences: a baseline that never completes, a
completion that does not match its binding, and encoding errors. A binding
marker still opens a baseline whether or not a binding is active.

The same rule now also covers the state after a fence: incrementals are
dropped until a fresh baseline arrives. Before, request paths returned 503 in
that state and scheduled paths already dropped.

## Evidence

- New test `a_debug_command_can_be_the_first_operation_without_a_subscriber`
  (`tests/loopback_host.rs`). Two readout-carrying debug commands, with no
  subscriber, both return 200, and a later SSE connection still gets a
  complete baseline. Without the `host.rs` change, the test fails with the
  reported `DEV_HOST_OUTPUT_BASELINE` error.
- `output_publication_failure_returns_the_consumed_receipt_for_resync_without_replay`
  used an unbound incremental to force a publication failure. It now uses a
  baseline that never completes, so the resync-required path keeps its
  coverage.
- `a_rejected_publication_sends_nothing_and_fences_the_binding` now asserts
  that an incremental after a fence is dropped, not rejected.
- `cargo test -p product-dev-host`: 74 passed. Clippy is clean with
  `--no-deps --tests -- -D warnings`.
