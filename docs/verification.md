# Verification notes

## CI lanes

CI follows the current C# product, Rust Engine, and browser renderer ownership.
Changes route to their owning lanes; superseded runs are cancelled.

| Lane | Default evidence |
| --- | --- |
| Rust | Formatting, Cargo dependency boundaries, mechanism tests, Clippy with warnings as errors |
| C# | Binding generation and a disposable packaged SDK consumer staged and exercised through the Rust CoreCLR host |
| Render | TypeScript package boundaries, current browser artifact freshness, compiled unit tests and Chromium behavior |
| Studio | Studio boundaries, lint, types, tests and build |
| Docs | Local links and CI owner routing |

Run the corresponding `scripts/verify.sh`, `scripts/verify-csharp.sh`,
`scripts/verify-render.sh`, `scripts/verify-studio.sh`, or
`scripts/verify-docs.sh` locally. Renderer and Studio checks require their pnpm
dependencies; C# requires .NET, Clang/libclang and the pinned binding tools.

NativeAOT is a separate fidelity path: `scripts/verify-csharp.sh --aot`, the
C# workflow dispatch option, and SDK release verification. The optional native
webview remains available through `scripts/verify-renderer-webview-host.sh`
with its platform dependencies and Playwright Chromium; ordinary browser CI
does not install GTK or
WebKit or certify that host.

Boundary checks inspect current dependencies and artifacts. Historical symbol
blacklists, obsolete TypeScript product-authoring restrictions, public TypeScript
package declaration isolation, and exact build
command text are not architecture contracts. Browser artifacts are Engine build
inputs; ordinary products do not consume their TypeScript declarations. Deliberate ABI adapter signatures
may carry a local, reasoned Clippy exception; warnings-as-errors remains active.

## Playtest warning deltas

`scripts/capture-playtest-warning-delta.mjs` is a small report writer for one
named Playwright exercise. It independently listens for Playwright console
warnings/errors and `pageerror` events, and, when an Engine host origin is
provided, reads structured Engine diagnostics through the bounded local route.
Warnings/errors remain findings; later confirmed baseline observations can
resolve an exact attachment's settled response-delivery warning. It retains normalized, 512-character messages
and stable fingerprints with occurrence counts; it never forwards console
arguments, request/response bodies, or stacks.

Attachment recovery preserves the original warning and records its matching
baseline sequence. A queued input acknowledgement does not prove C# execution;
queued-input or unknown delivery certainty remains unresolved. New tabs, missing
correlation, incomplete capture, and terminal errors cannot clear that warning.
Failed browser resources carry their observed URL and status rather than an
assumed favicon diagnosis.

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
