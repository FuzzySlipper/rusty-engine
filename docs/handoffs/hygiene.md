# Lane: hygiene

**Tasks, in order:** #8794, #8752. **Start:** now.
Campaign #8723; read Den doc `rusty-engine/architecture-reset-2026-09`.
Shared protocol: [README.md](README.md).

## Tasks

- **#8794: delete `studio/` and move the live-debug panel out of it.**
  - The runtime pack's only Studio dependency is the live-debug panel.
    `scripts/build-runtime-pack.sh` builds it from `studio/libs/live-debug-panel`
    and installs it under `share/browser/engine/live-debug-panel` and
    `share/live-debug-panel`.
  - Rewrite it as plain DOM in the render workspace, for example
    `render/packages/live-debug-panel`. Keep the `browser-mount.ts` mount
    contract and the install paths.
  - Delete `studio/`, `.github/workflows/studio.yml`, the Studio scripts, the
    `verify:studio` root script, and the Studio entries in
    `scripts/check-ci-routing.py` and its test.
  - **Tell the runtime lane on #8745 when this lands.** It then classifies
    `engine-inspector`, whose only referrer is `studio.yml`.
- **#8752: stop tracking build outputs; retire obsolete audit guidance.**
  - `render/artifacts` and `renderer-webview-host/artifacts` are in scope.
    Their lanes are scheduled for deletion under #8792, so a simple
    ignore-and-untrack is enough.
  - First confirm that `scripts/build-runtime-pack.sh` (and anything else that
    ships them) builds those bundles itself rather than reading the tracked
    copy.
  - Other lanes rebuild `render/artifacts` whenever they change TypeScript.
    Once you untrack it, post a short Den note on #8798 and in your
    review-request message so they stop committing the bundles.
  - `artifacts/csharp-sdk-feed/*.nupkg`: keep anything a current package
    reference needs, and give it a small explicit route. The release lane
    (#8778) is moving pair publication into CI; don't delete what they
    consume.
  - Docs: `docs/validation-inventory.md`, `docs/validation-operation-audit.md`
    and the `docs/audit-78xx*.md` files become clearly historical. Fix
    `docs/README.md` so campaign #8723 direction is unambiguous.

## Files

- **Owns:** `studio/`; `.github/workflows/studio.yml`; the Studio scripts;
  `scripts/check-ci-routing.py`; `docs/README.md`; the historical audit docs;
  tracked build outputs; `.gitignore`.
- **Leave alone:** `docs/csharp-sdk.md`. #8781 (cli lane) splits it later.

## Evidence

- Root and render verification pass.
- `scripts/build-runtime-pack.sh` builds with the panel in its new place.
- A `rusty dev` session shows the live-debug panel and answers a command.
- The CI routing check passes.
