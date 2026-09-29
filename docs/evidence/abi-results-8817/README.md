# Every transient result is borrowed; the lease path is gone (#8817)

#8744 introduced borrowed results for Dynamics. This task moves every
remaining lease to that shape and deletes the generator's lease path.

A borrowed result points into storage owned by the service bridge. It stays
valid until the next call on the same service context. The generated wrapper
copies it before returning, so nothing is destroyed explicitly.

## Landed in two commits

1. **`394c7b5f0`: operation refusals.**
   - **Receipt.** `NativeOperationErrorReceipt` is now just
     `diagnostics`/`diagnostics_len`, pointing at the bridge's latest refusal.
   - **Removed from every family:**
     - the `NativeEngineDiagnosticLease` and its handle;
     - fourteen `destroy_operation_diagnostic_lease` table slots, one per
       family table, and their eleven typedefs;
     - the per-family diagnostic lease maps. They collapse into the shared
       `OperationDiagnostics`.
   - **Also removed: the receipt's `service`, `operation` and `status`
     fields.** Every bridge wrote `status: 0` and the same service and operation
     names that the generated caller already passes to `EngineCallException`.
   - **Generator.** `NativeCall.Require` no longer takes a destroy pointer.
2. **This commit: results.**
   - **ABI.** Every `Native*Lease` becomes a `Native*Result` without a handle.
     Its `NativeDestroy*Lease` typedef and table slot are removed.
   - **Bridges.** Each bridge keeps its latest backing in one `BorrowedResult`
     slot, a `Box<dyn Any>` in `operation_diagnostics.rs`.
   - **Generator.** The lease path is deleted: `IsLeaseResult`,
     `DestroyLeaseFor`, the lease half of `ForeignNativeTableOwners`,
     `LeaseHandleTypes`, and the try/finally destroy emission. What remains
     recognises a `Native*Result` out receipt that has a pointer field.

## Counts (`csharp-engine-abi`, before #8817 → after)

| | Before | After |
|---|---|---|
| `Native*Lease` structs | 27 (26 results + `NativeEngineDiagnosticLease`) | 0 |
| `Native*LeaseHandle` structs | 27 | 0 |
| `NativeDestroy*Lease` typedefs | 39 (28 result, 11 diagnostic) | 0 |
| `destroy_*lease` function-table slots | 43 (29 result, 14 diagnostic) | 0 |

- **Diff across both commits:** 67 files, +1,444 / −4,020 lines (docs
  excluded).
- **By area:**
  - `csharp-engine-services`: +795 / −2,883.
  - The generator: +81 / −126.
- **Generated C#.** It still has 95 `destroy_*` calls, and every one releases
  a genuinely retained resource: a body, material, session, clip, reference
  and so on. None is a result or diagnostic lease.

## Transient versus retained

Every lease was transient. The generator only ever called a lease's destroy in
the `finally` right after copying it, and no hand-written C# called a lease
destroy. Retained resources keep their disposable handles unchanged.

Two results now borrow retained storage directly, with no holder and no copy:

- **Persistence `ReadBlobBytes`** used to clone the blob payload into a new
  `Arc`. It now points into the blob, which stays retained until a later call
  destroys it.
- **Content `ReadBytes`** used to clone the reference's `Arc` into a lease map.
  It now points into the retained reference's bytes.

## Short buffer and growth

As in #8744, the caller supplies no capacity, so a short buffer cannot happen
and no operation is re-run. That includes the mutating ones: annotation edits,
voxel asset publish, kinematic motion, and sprite advance.

## Evidence

- **Rust tests.**
  - Command: `MALLOC_PERTURB_=165 cargo test -p csharp-engine-abi -p csharp-engine-services -p csharp-product-runtime`.
    It passes with 185, 45 and 27 tests.
  - glibc's `MALLOC_PERTURB_` overwrites freed memory, so a test that read a
    result after a later call replaced it would fail.
  - The check itself was validated. With the kinematic test's `.to_vec()`
    removed, the test fails.
  - Two tests kept several results or refusals and read them later: kinematic
    facts, the Spatial map cells, and the 32 retained voxel-scene refusals. They
    now copy or read immediately, as the generated caller does.
- **Clippy.** `cargo clippy … --all-targets -- -D warnings` passes for the three
  crates.
- **Generator fixture.**
  - It is renamed from `fixtures/csharp-binding-generator-lease` to
    `fixtures/csharp-binding-generator-results`, with its script and CI path.
  - It now checks borrowed collections, metadata, nested UTF-8 and bytes, owned
    handles, borrowed request spans, and copied refusal diagnostics.
  - Its native side poisons and frees each result on the next call. The
    fixture reads a result only after that next call, so a wrapper that had not
    copied would fail.
  - Its two rejection headers still fail generation as expected.
  - Removed:
    - the lease release-count checks;
    - the success-with-diagnostic-lease case;
    - the metadata-only summary case, since no production result has that
      shape.
  - Command: `scripts/test-csharp-binding-generator-results-fixture.sh`
    prints `BINDING_GENERATOR_RESULT_FIXTURE_PASSED`.
- **SDK smoke.** `scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot`
  passes under CoreCLR and NativeAOT.
  - The consumer covers implicit audit reports, content bundles and bytes,
    spatial artifacts, navigation, caught refusals, hardware exceptions and the
    Dynamics results.
- **Runtime pack.** `scripts/test-runtime-pack.sh` passes. It launches the
  CoreCLR and NativeAOT trial bundles, which cover the Spatial map, kinematic
  motion, perception, authored content, voxel content and animation clip info.
- **Other builds.**
  - `csharp/Rusty.Engine.{Implicit,Content,Entities}.Example` build.
  - `fixtures/csharp-architectural-room` builds against a freshly packed SDK.

## Migration

- **`XLeaseReceipt` → `XResult`.** The members are unchanged. Examples:
  - `PerceptionReadoutLeaseReceipt` → `PerceptionReadoutResult`;
  - `SpritePlaybackAdvanceLeaseReceipt` → `SpritePlaybackAdvanceResult`;
  - `SpatialMapLeaseReceipt` → `SpatialMapResult`;
  - `KinematicMotionLeaseReceipt` → `KinematicMotionResult`.

  Dagger, Doom and CraftSurvive use these names.
- **`EngineCallException`.** `Service` and `Operation` are always the generated
  names, and `Status` is the call status. They were already equal in practice:
  every bridge wrote the same names and `status: 0`.
- **Generator fixture.** Anything that ran
  `scripts/test-csharp-binding-generator-lease-fixture.sh` should run
  `scripts/test-csharp-binding-generator-results-fixture.sh` instead.

## Not changed

- **`MagicaVoxelAdmissionStatus.PaletteLeaseExhausted` (13)** was removed
  afterwards in #8829.
  - It had also been the fallback for an unknown object handle, which now
    reports `InvalidRequest`.
  - The palette's source hash is converted once at admission. A digest that
    fails to convert rejects the admission with `CanonicalObject`, so the read
    can no longer fail on it.
  - No product, fixture or doc referenced the value.
  - Evidence: the new
    `magica_palette_read_reports_its_rejections_by_status` test (success, a
    non-MagicaVoxel object, an unknown handle), `test-runtime-pack.sh`, and the
    CoreCLR smoke.
- **The remaining `Read*At` index calls in Spatial and World Origin** are
  #8818.
