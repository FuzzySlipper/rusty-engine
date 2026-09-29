# Legacy runtime surfaces removed (#8745)

About 5,100 lines were deleted in two commits. Each surface below was deleted
once a workspace build showed it had no production consumer. The tests whose
purpose went with it were removed or rewritten.

## Deleted

**`runtime-ui` lane** (`ff6bda2b`):

- `channel.rs`: `RuntimeUiProjection` and `PreparedRuntimeUiProjection`.
- The free `encode_runtime_ui_projection_json` and
  `decode_runtime_ui_projection_json`, and `RuntimeUiProjectionEnvelope::decode_json`.
- `RuntimeUiProjectionReadout`, `MAX_RUNTIME_UI_PROJECTION_STREAMS` and the
  lane-only error variants.

**Test-only `render-projection` modules** (`ff6bda2b`):

- `entity.rs`, `debug.rs`, `model_preview.rs` and `RetainedNodeProjector`;
- their tests, and the dependencies only they used (`core-ids`,
  `entity-state`).
- `PresentationProjectorSet` was already gone.

**`engine-inspector` (`rusty-inspect`)** (`ff6bda2b`). After Studio was deleted
(#8794), no product, CI job, runtime pack or doc used it. Its leaf rule in
`scripts/dependency_boundary_check.py` went too.

**`runtime-session` recovery vocabulary** (second commit):

- `RuntimeMutationCertainty`, `RuntimeInvalidatedScope`, `RuntimeNextAction`
  and `RuntimeRecovery`;
- their `ProductDevRuntimeRecovery` aliases in `product-dev-host`;
- the optional `recovery` object in every host result on the wire, and its
  TypeScript type and decoder.

The browser never read that object. Its behaviour depends on `disposition` and
the `X-Rusty-Commit-Disposition` header, and both were derived from the
recovery triple. `ProductDevRuntimeError` now carries the disposition itself:

| Constructor | Disposition | Commit header |
|---|---|---|
| `new` | `terminal` | `unknown` |
| `new_not_applied` | `rejected-recoverable` | `not-applied` |

The operation resync receipt (`ProductDevOperationResult::resync_required`)
still sets `resync-required`. Two constructors had no callers and were
deleted: `ProductDevRuntimeError::new_output_rebaseline` and
`ProductDevTimelineCompletionResult::resync_required_with_current`.

**Attach and reset paths.** `begin_attach_call`, `reset_renderer_projection` and
`rebase_ghost_plates` were already deleted by #8736. Nothing to do.

## Kept, with the consumer that needs it

- **`RuntimeUiProjectionEnvelope` and `RuntimeUiRuntimeBinding`.**
  `csharp-engine-services` (UI streams), `runtime-publication` and
  `product-dev-host` build and serialize them.
- **The envelope's `Deserialize`.** `product-dev-host`'s
  `ProductDevRuntimeOutput` derives `Deserialize` through it. Only that crate's
  own round-trip tests decode outputs.
- **`StableHandleRegistry`,** used by `runtime_appearance.rs` and `voxel.rs`.
- **`RuntimeSession` and `RuntimeReceipt`.** `product-dev-host` serializes
  runtime calls through `RuntimeSession`, including `with_locked_timed`.
  `RuntimeReceipt` carries each operation's result and outputs.

## Classified elsewhere

- **Content-store:** #8763. Deleting `rusty-inspect` removes one of its
  consumers.
- **Three.js lane and `renderer-webview-host`:** deleted under #8792.
- **In-place renderer reset:** dropped by the task (streaming mode, #8786).

## Exercises

- **Supervised fault exercise.** This build and its browser bundle, run with
  `docs/evidence/managed-hardware-exceptions-8753/scripts/fault-exercise.mjs`
  (`results/supervised-fault-exercise.json`). An uncaught
  `NullReferenceException` gives `running → faulted` on the same instance and
  logs the stack trace. The page stays `ready` and resume is accepted
  (241 → 420). There were no page errors, no 5xx responses and no restarts.
- **Rejected response on the new wire** (`results/rejected-resume.http`). A
  resume naming the wrong binding returns `disposition: rejected-recoverable`
  with `X-Rusty-Commit-Disposition: not-applied`, and no `recovery` object.

## Validation

- `cargo check --workspace --exclude renderer-webview-host --all-targets`
  passes.
- Tests pass for `runtime-session`, `runtime-ui`, `render-projection`,
  `runtime-publication`, `product-dev-host` (lib and loopback) and
  `csharp-product-runtime`.
- `pnpm test` in `product-browser-host` passes: 105 tests.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes.
- `python3 -m unittest test_architecture_checks` (7 tests) and
  `scripts/dependency_boundary_check.py` pass.
- `cargo clippy --no-deps -D warnings` is clean on `runtime-session`,
  `runtime-ui`, `render-projection` and `csharp-product-runtime`. On
  `product-dev-host` it reports one lint that is already on main, not from
  this change: `collapsible_match` in `model.rs`, near the video feedback
  status match.

## Migration

- **Rust API.**
  - `ProductDevRuntimeError::recovery()` is now `disposition()`.
  - `with_recovery` and `new_output_rebaseline` are removed.
  - The `ProductDev{MutationCertainty,InvalidatedScope,NextAction,RuntimeRecovery}`
    re-exports are gone.
- **Wire.** Host results no longer include `recovery`. The matching
  `product-browser-host` decoder no longer accepts it. The browser shell and
  host ship as a matched pair.
- **Products.** Nothing to change: C# products never saw these types.
