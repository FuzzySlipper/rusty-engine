# Tracked build outputs and historical audit ledgers (#8752)

## Untracked and ignored

| Output | What built or read it | After |
| --- | --- | --- |
| `render/artifacts/{application-host,product-browser-host}` (16 files) | `pnpm --dir render run build`. `scripts/build-runtime-pack.sh` already ran the build and bundle steps before installing `product-browser-host.js`. `check-boundaries.mjs` and `product-browser-bundle.browser.spec.ts` read the built files. | Ignored. `scripts/verify-render.sh` and render's `verify` script now build before the boundary check. |
| `rust/crates/renderer-webview-host/artifacts/renderer-webview.js` | `pnpm --dir render run build:webview-artifact`. The crate embeds it with `include_str!`. The workspace test and clippy gates exclude the crate, and no crate depends on it. | Ignored. `scripts/verify-renderer-webview-host.sh` always builds it first. Its only caller-facing option, `--artifacts-ready`, had no users and is gone. |
| `artifacts/csharp-sdk-feed/*.nupkg` (4 packages) | Default output directory of `scripts/pack-csharp-sdk.sh`. No script, workflow, NuGet.Config or fixture reads it. Release pairs pass their own feed directory and publish to GitHub releases (#8778). Doom's pinned `0.1.0-dev.playtest-20260928c` resolves from its own `.runtime/.../sdk-feed`; Dagger uses `.runtime/sdk-feed`. | Ignored. The packages remain in Git history (parent of this commit) and in the local NuGet cache. |
| `scripts/__pycache__/*.pyc` | Python bytecode cache. | Ignored everywhere (`__pycache__/`). |

`.gitattributes` held only a whitespace rule for the webview bundle, so it is
deleted. Per the task note, `studio/artifacts` belonged to #8794, which deleted
it with Studio.

The render lanes these bundles serve are scheduled for deletion under #8792, so
there is no new bootstrap route beyond the existing build scripts.

## Removed because its purpose disappeared

- `scripts/verify-render-artifacts.sh` compared the tracked bundles with a
  rebuild. With nothing tracked, there is nothing to compare. Its path entries
  in `render.yml` and `check-ci-routing.py`, and the three routing cases for the
  bundle paths, went with it.

## Documentation

- `docs/validation-inventory.md`, `docs/validation-operation-audit.md` and
  `docs/audit-{7874,7875,7876-conversion,7877,7882}.md` moved to `docs/history/`.
  Each now opens with a banner stating that it is superseded by campaign #8723
  and that its retain/preserve language is not policy. `docs/history/README.md`
  indexes them. `scripts/inventory-validation.py` stays; tool removal belongs to
  #8745.
- `docs/README.md` now leads with the campaign #8723 direction, no longer lists
  the validation inventory as current guidance, and points to `history/` for
  provenance.
- `render/README.md`, `docs/verification.md` and the shared-generated-files note
  in `docs/handoffs/README.md` now say the bundles are ignored build output.

## Evidence

- In a fresh clone of this commit, with no ignored outputs present:
  `pnpm run build` produces `application-host`, `product-browser-host` and
  `live-debug-panel`. `pnpm run build:webview-artifact` writes
  `rust/crates/renderer-webview-host/artifacts/renderer-webview.js` at the
  `include_str!` path. `pnpm run boundary`, `typecheck:browser` and `test:compiled`
  pass. `product-browser-bundle.browser.spec.ts` and
  `live-debug-panel.browser.spec.ts` pass (8 tests).
- `scripts/verify-docs.sh`: doc links, CI owner routing and the negative probes
  pass.

## Migration

Lanes that change TypeScript no longer commit `render/artifacts`. After pulling
this commit, a checkout that had those files tracked loses them from its working
tree. Run `pnpm --dir render run build`, or `build:webview-artifact` for the
webview crate, to recreate them.
