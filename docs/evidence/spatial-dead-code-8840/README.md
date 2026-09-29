# Spatial dead code left by #8840

The #8840 review listed code that no longer had a consumer. This change removes it.

## Removed

- **Trigger revision.** `TriggerVolumeSystem` no longer counts changes to its
  active and overlap sets. The revision was kept for overlap-page
  continuations. #8840 made overlap reads a single borrowed result, so nothing
  read the revision any more. These fields are removed:
  - the revision on `TriggerOverlapReadout`, `TriggerReconcileReceipt`,
    `TriggerLifecycleReceipt` and `TriggerRestoreReceipt`, and on their ABI
    mirrors `NativeSpatialTriggerReconcileResult`,
    `NativeSpatialTriggerLifecycleResult`,
    `NativeSpatialTriggerRestoreReceipt` and
    `NativeSpatialTriggerReadResult`;
  - the `RevisionOverflow` code. `reconcile` could only fail with
    `RevisionOverflow`, so it is now infallible.
- **Trigger snapshot codec.** `trigger_codec.rs`, `snapshot`/`from_snapshot`,
  the schema version, the snapshot diagnostic codes and the serde derives on
  the trigger definitions are removed. No host saved or restored trigger state
  through them. Products restore through `RestoreTriggers`.
- **World-origin snapshot codec and conversions.** These are removed:
  - `encode_/decode_world_origin_state`, `WorldOriginSnapshotV1`, the schema
    version and the three snapshot errors;
  - `global_from_local`/`local_from_global`.

  Only tests used any of them. `serde_path_to_error` leaves engine-spatial,
  and `serde_json` moves to dev-dependencies.
- **`fixtures/csharp-lease-release`.** It had not compiled since #8817, and no
  script built it.
- A dynamics bridge test read the world origin and then ignored the readout.

## Evidence

- `cargo test -p engine-spatial -p csharp-engine-services -p csharp-engine-abi`
  passes.
- `cargo test --workspace --exclude renderer-webview-host --no-run` compiles.
- Clippy on those crates with `--all-targets` is clean.
- The bindings regenerated.
- `Rusty.Engine.Entities.Example` runs clean.
- These checks pass:
  - `scripts/test-csharp-sdk-package.sh --coreclr-smoke`;
  - `scripts/test-runtime-pack.sh`;
  - a local `scripts/build-csharp-release-pair.sh` with
    `scripts/test-csharp-release-pair.sh`.

## Migration

- C#: `SpatialTriggerReconcileResult` and `SpatialTriggerReadResult` lose
  `Revision`. `SpatialTriggerLifecycleResult` and
  `SpatialTriggerRestoreReceipt` lose `RevisionBefore`/`RevisionAfter`.
  Remove those arguments from positional constructors in test doubles.
- Rust: `TriggerVolumeSystem::reconcile` returns the receipt directly, and
  `revision()` is gone.
