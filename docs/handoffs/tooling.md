# Lane: tooling

**Tasks, in order:** #8808 (closes #8804 too), #8757, #8803 (closes #8774
too), #8801, #8809, #8816, #8802. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

This lane makes the repo's own checks trustworthy again, then fixes two small
dev-host issues. **#8808 is first because other lanes need
`scripts/test-runtime-pack.sh` as evidence.** Post a Den note on #8799 (abi
lane) when it passes again.

## Tasks

- **#8808: the NativeAOT trial publish emits a CoreCLR app, not a `.so`.**
  #8804 is the same failure; close both with one fix.
  - Reproduce: after a clean `obj`/`bin`,
    `dotnet publish fixtures/csharp-nativeaot-trial/CsharpNativeAotTrial.csproj -c Release -r linux-x64`.
    The restored `project.assets.json` has no ILCompiler reference and
    `IlcCompile` never runs, although `PublishAot=true`.
  - Decide environment versus repo. #8776 (`7eca06d2`, fixed in `8f08ab042`)
    changed the NativeAOT restore path in `Rusty.Engine.targets`, and the
    fixture may be caught in it. `scripts/test-csharp-sdk-package.sh --aot`
    passes, so compare what that consumer does differently.
  - Done when `scripts/test-runtime-pack.sh` passes end to end.
- **#8757: pre-existing clippy failures.** Apply the simplifications and do
  not add allows. Current sites:
  - `product-dev-host/src/model.rs` (`collapsible_match`; it moved from line
    609 to around 577);
  - `render-presentation/src/frame.rs:419` and `:463`;
  - `csharp-engine-services/src/spatial.rs` and `voxel.rs` (`nonminimal_bool`).

  Clippy must be clean for those crates with `--no-deps --tests -- -D warnings`.
  Afterwards, `scripts/verify-renderer-webview-host.sh` should pass too.
- **#8803: failing green-pixel assertion in `renderer.browser.spec.ts`.**
  #8774 is the same failure; close both.
  - Three is scheduled for deletion (#8792), so bound the work. If it's a real
    composition regression in a current product, fix it. If it's the sample
    point or a fixture expectation, fix the assertion and say why. Don't turn
    the assertion into a no-op.
- **#8801: the first live-debug command after a runtime start fails with
  `DEV_HOST_OUTPUT_BASELINE` when no browser is attached.**
  - Code: `product-dev-host/src/host.rs` (`invoke_debug_execute`),
    `session.rs` (`finish_call`), `model.rs` (`into_wire_parts`).
  - Repro product: `docs/evidence/source-root-reload-8743/product`.
  - Evidence: a focused test.
- **#8809: `fixtures/csharp-json-persistence`.** Delete it unless it proves
  something nothing else covers. Its only references are its own README and
  `docs/evidence/content-store-removal-8763/README.md`.
- **#8816: `audit-standalone.sh` fails** on absolute worktree paths in the
  `before/Cargo.toml` manifests under
  `docs/evidence/retained-graphics-8737/scripts/projector-probe/` and
  `docs/evidence/retained-dynamics-8738/scripts/step-bench/`. Use git-rev
  dependencies or drop the manifests.
- **#8802: decide whether `rusty dev` reloads open pages after a UI asset
  reload.** Decide from actual use. If yes, the smallest form is one SSE event
  plus `location.reload()`, with no hot-module framework. The browser side is
  frozen until #8792, so keep any TS change minimal.

## Files

- **Owns:** see the README table.
- **Leave alone:** `Rusty.Engine.targets` beyond what #8808 needs; the cli
  lane is changing `rusty-cli`.
