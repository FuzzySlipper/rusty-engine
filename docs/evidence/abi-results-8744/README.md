# Borrowed Dynamics results instead of leases (#8744)

## What changed

Dynamics `StepAndRead` used to return a `NativeDynamicsStepAndReadLease`. It
worked like this:

1. Rust allocated a lease handle and boxed the result.
2. Rust stored the box in a per-bridge `BTreeMap`.
3. The generated C# copied the result.
4. The generated C# made a second FFI call, `destroy_step_and_read_lease`, to
   remove it.

Enumerating a world took one FFI call per element through `ReadBodyAt(index)`
and `ReadContactAt(index)`. `ReadBodyAt` walked the body map with
`keys().nth(index)` on each call, so a full loop was quadratic.

Both paths now return a **borrowed result**:

- **`NativeDynamicsStepAndReadResult`** holds `bodies`/`bodies_len` plus
  `generation`, `body_count` and `contact_count`.
- **`NativeDynamicsWorldResult`** holds `bodies`/`bodies_len`,
  `contacts`/`contacts_len` and `generation`. `ReadWorld` now returns every
  body readout and contact in one call.
- **Where the memory lives.** The pointers refer to two `Vec`s owned by the
  Dynamics bridge (`body_facts` and `contacts`). Each call clears and refills
  them.
- **Lifetime.** A result stays valid until the next call on the same Dynamics
  context. The generated wrapper copies it into managed memory before
  returning, so no product code ever sees the pointer.
- **Generator.** An out receipt named `Native*Result` with a pointer field is a
  borrowed result (`BindingModel.IsBorrowedResult`). The copy code is the same
  as for leases, but no handle, destroy call or `finally` is emitted. Lease
  results still work, so the other families are unchanged.

### Removed

- **ABI types:**
  - `NativeDynamicsStepAndReadLease`, `NativeDynamicsStepAndReadLeaseHandle` and
    `NativeDestroyDynamicsStepAndReadLease`;
  - `NativeDynamicsBodyAtRequest`/`Receipt`,
    `NativeDynamicsContactAtRequest`/`Receipt` and
    `NativeDynamicsWorldReadout`;
  - the `read_body_at` and `read_contact_at` typedefs.
- **Function-table entries:** `destroy_step_and_read_lease`, `read_body_at`
  and `read_contact_at`.
- **Bridge state:** the lease `BTreeMap`, its handle counter,
  `DynamicsStepAndReadLeaseBacking`, and one `Box` allocation per step.
- **FFI calls:**
  - one destroy crossing per `StepAndRead`;
  - `bodies + contacts` indexed crossings per world enumeration, which are now
    one call.
- **Totals.** `csharp-engine-abi` goes from 28 to 27 lease structs and from 40
  to 39 destroy-lease typedefs. `csharp-engine-services/src/dynamics.rs` loses 77
  lines net.

### Kept

- **`NativeDynamicsBodyFact` (renamed from `StepAndReadBody`)** is still copied
  element by element. The safe `DynamicsBodyFact` is a managed record, so the
  wrapper converts each native row, as it did for the lease.
- **The Dynamics operation-diagnostic lease** is shared by every family's
  `NativeOperationErrorReceipt`. It moves with the other leases in #8817.

## Short buffer and growth

There is no caller-provided capacity, so a short buffer cannot happen. Rust
sizes its own storage inside the call, and a mutating step never has to run
again to fit its result.

Caller-provided storage was the alternative, and it was rejected for three
reasons:

- **It cannot cover every family.** It would need the result size before the
  mutation. Several remaining leases come from mutating operations whose size
  is known only afterwards: voxel annotation edits, voxel asset publish,
  kinematic motion, sprite playback advance, and diagnostics after a failed
  mutation. For those, a short buffer would force the Engine to keep the
  overflow, which is a lease again.
- **It saves no copies.** Rust writes native rows once in either design, and
  C# converts each one to a safe record once.
- **It allocates more.** C# would allocate a native staging array per call. The
  bridge `Vec` allocates only when it grows.

Growth is exercised directly. The CoreCLR check steps with 2 bodies, then 0,
then 64 (repeated handles). The generation rises by exactly one per call, and
the 64-row result is complete and in order. The earlier 2-row managed copy is
unchanged after the native buffer is reused and grown.

## Evidence

- **Layout.** [`layout-probe.sh`](layout-probe.sh) compiles a C probe against
  the generated `rusty_engine.h` and a Rust probe against `csharp-engine-abi`,
  then diffs them. The generated C# structs come from the same clang AST, and
  the ABI fingerprint hashes these offsets.

  ```text
  NativeDynamicsStepAndReadResult size=32 align=8
    bodies=0 bodies_len=8 generation=16 body_count=24 contact_count=28
  NativeDynamicsWorldResult size=40 align=8
    bodies=0 bodies_len=8 contacts=16 contacts_len=24 generation=32
  NativeDynamicsBodyFact size=152 align=8   body=0 readout=8
  NativeDynamicsContact size=40 align=8     environment=0 first=8 second=16 impulse=24 impulse_magnitude=36
  C and Rust layouts match
  NativeStepAndReadDynamics { internal delegate* unmanaged[Cdecl]<void*, NativeDynamicsStepAndReadRequest*, NativeDynamicsStepAndReadResult*, NativeOperationErrorReceipt*, int>
  NativeReadDynamicsWorld { internal delegate* unmanaged[Cdecl]<void*, NativeDynamicsWorldReadRequest, NativeDynamicsWorldResult*, NativeOperationErrorReceipt*, int>
  ```

- **Generated wrapper.** `StepAndRead` is now `status → Require → return
  NativeConversions.CopyLeaseReceipt(rawResult)`, with no `try`/`finally` and
  no destroy call.
- **CoreCLR and NativeAOT.** `scripts/test-csharp-sdk-package.sh
  --coreclr-smoke --aot` passes. Its consumer runs
  `scripts/fixtures/DynamicsResultChecks.cs`, which covers the ordinary,
  empty, growth and copy-survives-reuse cases plus `ReadWorld`. It printed
  `DYNAMICS_RESULT_CHECKS_PASSED` under both loaders.
- **NativeAOT.** No NativeAOT-specific issue was found. The borrowed result is
  a plain `repr(C)` struct read through `delegate* unmanaged[Cdecl]`, exactly
  as the lease was.
- **Rust tests.**
  - The rewritten `step_and_read_returns_ordered_borrowed_result_that_the_next_call_replaces`
    covers order, empty, and growth without a repeated step.
  - The world-contact, body-enumeration and world-origin rebase tests now read
    `ReadWorld`.
  - `cargo test -p csharp-engine-abi -p csharp-engine-services -p csharp-product-runtime`
    passes.
  - `cargo clippy -p csharp-engine-abi -p csharp-engine-services --all-targets -- -D warnings` passes.
- **Not run.** `scripts/test-runtime-pack.sh` still stops at the NativeAOT
  fixture until the tooling lane's #8808 lands. The trial fixture's Dynamics
  section (`fixtures/csharp-nativeaot-trial/Product.cs`) is updated, but that
  script was not run here.

## Migration

| Before | After |
|---|---|
| `DynamicsStepAndReadLeaseReceipt` | `DynamicsStepAndReadResult` (same members) |
| `DynamicsStepAndReadBody` | `DynamicsBodyFact` (same members) |
| `DynamicsWorldReadout` (`Generation`, `BodyCount`, `ContactCount`) | `DynamicsWorldResult` (`Bodies`, `Contacts`, `Generation`); counts are `Bodies.Length` and `Contacts.Length` |
| `ReadBodyAt(new(world, i))` → `Present`, `Body`, `Readout` | `ReadWorld(new(world)).Bodies.Span[i]` → `Body`, `Readout` |
| `ReadContactAt(new(world, i))` → `Present`, … | `ReadWorld(new(world)).Contacts.Span[i]` (`DynamicsContact`, no `Present`) |

Downstream renames, needed when each product moves to this pair:

- **Rusty Doom.** `LoadingBayEngineServices.cs` and
  `LoadingBayRecipeGameplay.cs` name the first two types.
- **CraftSurvive.** `RopePlayground.Rebase` reads `DynamicsWorldReadout` for
  `Generation` only, so it needs the type rename.
- **Dagger and the template** do not use these APIs.

`ReadWorld` now copies every body and contact even when a caller needs only
`Generation`. The only such caller is a world-origin rebase, which is rare, so
the extra copy is accepted rather than kept as a second operation.

## Decision for the other families

The shape works end to end. The remaining 27 lease structs and 39 destroy
entrypoints, including the shared operation-diagnostic lease, move in #8817.
The abi lane takes it after #8799. Its final step deletes the generator's lease
path.

Spatial `ReadCharacterContactAt` and `ReadWorldOriginAffectedAt` are the last
indexed ReadAt calls. They live in spatial-lane files and are filed as #8818.
