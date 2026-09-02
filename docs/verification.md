# Verification notes

## Playtest warning deltas

`scripts/capture-playtest-warning-delta.mjs` is a small report writer for one
named Playwright exercise. It independently listens for Playwright console
warnings/errors and `pageerror` events, and, when an Engine host origin is
provided, reads only structured Engine Warning/Error diagnostics through the
bounded local diagnostics route. It retains normalized, 512-character messages
and stable fingerprints with occurrence counts; it never forwards console
arguments, request/response bodies, or stacks.

Run a named navigation-only exercise:

```sh
node scripts/capture-playtest-warning-delta.mjs \
  --url http://127.0.0.1:4173/ \
  --exercise-id renderer-smoke \
  --engine-origin http://127.0.0.1:9348 \
  --output warning-delta.json
```

For a multi-step required path, pass `--exercise path/to/exercise.mjs`. That
module must export `async function exercise({ page, url })`; it owns the
explicit navigation and interactions. The capture helper owns only listeners
and report construction.

The Engine read drains a private checkpoint to its current `throughSequence`
just before the exercise, then drains that same private cursor after it.
`--engine-cursor` can make the initial checkpoint detect an already-lagged
cursor, but does not share or advance another reader's cursor. Engine capture
is optional to configure, but an omitted, failed, lagged, dropped, or
incomplete Engine capture prevents the report from saying a clean claim is
eligible.

Compare against an explicitly named prior report:

```sh
node scripts/capture-playtest-warning-delta.mjs \
  --url http://127.0.0.1:4173/ \
  --exercise-id renderer-smoke \
  --engine-origin http://127.0.0.1:9348 \
  --baseline previous-warning-delta.json \
  --output warning-delta.json
```

Only matching schema/protocol, exercise ID, page origin, Engine origin, and
complete baseline capture are compatible. The JSON report explicitly says
`unavailable` when no baseline was supplied, `incompatible` when it cannot be
compared, or lists new/resolved/unchanged fingerprints. The helper is
report-only: warning findings and deltas do not themselves choose an exit code;
a failed browser run still exits nonzero. `cleanClaimEligible` is false for
incomplete capture, incompatible comparison, and terminal or unknown Engine
diagnostics; explicit Error findings also require disposition before a clean
claim.
