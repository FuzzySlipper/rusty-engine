# Lane: abi

**Tasks, in order:** #8744, #8799. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

## Tasks

- **#8744: replace transient result leases with direct result buffers.**
  - **Today's pattern.** There are no `ReadAt` loops left (#8739 removed the
    last). A service returns a `Native*Lease` (handle plus pointer and
    length). Generated C# bulk-copies it (`CopyLease`) and then calls a
    `NativeDestroy*Lease`. `csharp-engine-abi` has 28 lease structs and 40
    destroy entrypoints.
  - **Where the leases are.** Most are in `authored_content`,
    `voxel_content`, `implicit_surfaces`, `spatial`, `content`, `appearance`
    and `dynamics`. The C# side is the lease validation in
    `csharp/Rusty.Engine.BindingGenerator/Program.cs`, plus a few uses in
    `EntityDynamicsAdapter.cs`, `EntityKinematicMotion.cs`,
    `SpatialMapSnapshot.cs` and `PortableAssetContent.cs`.
  - **Work.** Caller-provided storage or one bulk result instead of a lease
    plus a destroy call. Handle short buffers and growth without re-running a
    mutating operation.
  - **Approach.** Prove the shape on one family end to end; Dynamics is the
    task's likely choice. Then decide in the task whether the rest move in
    this task or in follow-ups.
  - **Evidence.** An ABI layout check, and a CoreCLR exercise covering
    ordinary, empty, short-buffer and growth cases.
- **#8799: make product ABI callbacks required under exact ABI identity.**
  - **Code.** `csharp-product-runtime/src/lib.rs` (`from_bound_product`,
    `optional_callback_pair`, the `create_with_error` fallback, fixtures),
    `csharp-engine-abi/src/product.rs` and
    `csharp/Rusty.Engine/NativeProduct/ProductBridge.cs`.
  - **Work.** Delete the optional callback pairs and the plain `create`.
    #8744 may already touch the optional-callback branches. If so, land
    #8744 first and start #8799 from its result.
  - **Evidence.** `scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot`
    and `scripts/test-runtime-pack.sh`. The runtime-pack script fails until the
    tooling lane's #8808 lands; watch #8799 in Den for its note.

## Files

- **Owns:** see the README table.
- **Coordinate:**
  - **spatial lane.** It changes `world_origin` and `perception` ABI rows
    (#8807, #8806, #8805). Leave those files to it. If Dynamics is your
    first family, #8807 also edits one field in
    `csharp-engine-services/src/dynamics.rs`.
  - **generated identity.** Regenerate `generated_abi_identity.rs` on every
    rebase that conflicts.

## Evidence

Each change breaks the product ABI, so write migration notes. Downstream
products pick the change up with their next pair.
