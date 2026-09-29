# Lane: spatial

**Tasks, in order:** #8805, #8807, #8806. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md). Round 1 of this lane landed #8754
(with #8755); these are its follow-ups.

## Tasks

- **#8805: remove the throwaway `EntityState` from perception occluders.**
  - `csharp-engine-services/src/perception.rs` still calls
    `entity_state(occluders)`. Read the occluder rows directly, as #8754 did for
    triggers (`engine-spatial/src/trigger.rs`).
  - Then delete `spatial.rs` `fn entity_state`, if nothing else uses it.
  - The ABI already passes occluder rows (`csharp-engine-abi/src/perception.rs`),
    so no ABI change is expected.
  - Evidence: bridge tests still show visibility and occlusion results;
    `engine-spatial/tests/perception.rs`.
- **#8807: drop product-supplied expected revisions from world-origin prepare.**
  - Remove the expected-revision fields:
    - `NativeWorldOriginPrepareRequest` (`csharp-engine-abi/src/world_origin.rs`);
    - `WorldOriginRebaseRequest` (`engine-spatial/src/world_origin.rs`).
  - Commit fences on the base revisions recorded at prepare time.
  - Callers to update:
    - `csharp-engine-services/src/world_origin.rs`;
    - `dynamics.rs`, the one `expected_origin_revision` site; keep that edit
      to the field, because the abi lane may be working in that file;
    - `EntityOriginRebaser.cs`;
    - `csharp/Rusty.Engine.Entities.Example/Program.cs`;
    - `docs/csharp-sdk.md`;
    - the voxel soak test in `render-projection/src/voxel.rs`, which builds a
      rebase request.
  - Evidence: `--coreclr-smoke` plus the stale-candidate bridge test.
- **#8806 (exploration): how product-held character motion follows a
  world-origin rebase.**
  - Run a far rebase with a supported, tethered and falling character through
    the C# bridge.
  - Decide between three answers:
    - nothing is needed;
    - the product applies the delta;
    - the Engine returns the delta in the commit receipt.
  - Build a remedy only if a concrete failure shows up. A receipt delta
    edits the same world-origin ABI as #8807, which is why this task comes
    after it.

## Files

- **Owns:** see the README table.
- **Leave alone:** the lease and result plumbing (abi lane, #8744).

## Evidence

Focused bridge tests, plus `scripts/test-csharp-sdk-package.sh --coreclr-smoke`
for #8807 and for #8806 if it changes the ABI. Dagger reads world-origin
receipts. Check that it still builds against the new pair, and write
migration notes.
