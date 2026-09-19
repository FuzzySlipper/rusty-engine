# Lane: Error and boundary paths

**Optional.** Use when the change adds parsing, input handling, or failure paths.

## One question

Does the change handle the boundary and failure paths its inputs actually reach?

## Basis required for an actionable finding

Name all three:

1. the input, state, or boundary condition;
2. the code path it takes, with file and line;
3. the observed wrong result — a crash, a silent default, a wrong value, a
   swallowed error, or a partial mutation that is not rolled back.

Prefer a case you can execute. A reproducing command is the strongest evidence
this lane can carry, and `bash` is available for it.

## What to probe

- Empty, zero, single, and maximum inputs, and the transition between them.
- Malformed source data from an import or decoding boundary, and the version or
  format cases the parser claims to accept.
- Failure of an underlying Rust service or ABI call, and whether caller-visible
  state stays coherent when it fails partway. Preserve actual ABI/lifetime
  invariants; do not ask for repeated downstream validation merely because an
  upstream operation can reject something.
- Cancellation or interruption between two mutations that were meant to be one
  operation.
- The absent case: a required resource, pack entry, or catalog row that is
  missing rather than wrong.

## Not a finding

- A defensive check for a state the type system or the caller already excludes.
- A request for broad input validation unrelated to what this change reads.
- An underlying service failure the caller correctly surfaces and stops on.
