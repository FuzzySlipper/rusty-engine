# Studio removal and live-debug panel relocation (#8794)

## Removed

- `studio/`, about 29k lines of TypeScript plus its Angular/Nx workspace, lockfile,
  tests and the Angular build of the live-debug panel.
- `.github/workflows/studio.yml`, `scripts/verify-studio.sh`,
  `scripts/audit-studio-isolation.sh`, `scripts/verify-studio-voxel-integration.sh`.
- `scripts/verify-voxel-surface-comparison.sh`. Its browser stage ran a Playwright
  suite under `studio/test/voxel-surface-comparison` that submitted frames through
  `@rusty-engine/studio-viewport`, so it could not outlive Studio.
- `.den-serve.json`, which only served Studio for Den.
- The `serve:studio` and `verify:studio` root scripts, the Studio lane in
  `scripts/check-ci-routing.py` and its negative probes, the Studio owner buckets
  in `scripts/inventory-validation.py`, and Studio mentions in
  `docs/verification.md` and the agent-review lane guides.

## Kept, and why

The runtime pack's only Studio dependency was the live-debug panel. Doom's
Loading Bay mounts it through the `@rusty-engine/live-debug` import map entry,
so the panel moves rather than disappearing.

- `render/packages/live-debug-panel` is a plain-DOM rewrite of the roughly 460-line
  Angular component. It keeps the `browser-mount.ts` contract
  (`mountLiveDebugPanel`, `mountRendererMetricsWidget`,
  `createLiveDebugHttpTransport` and their option types), the CSS class names,
  and the transcript, history, completion, diagnostics and telemetry behavior.
  `renderer-metrics-widget.ts` and `live-debug-panel-model.ts` moved unchanged
  apart from one index-signature access.
- `render/vite.live-debug-panel.config.ts` and
  `render/scripts/publish-live-debug-panel-artifact.mjs` write the import-closed
  artifact to `render/artifacts/live-debug-panel`, which is git-ignored. The
  artifact went from 573 kB (Angular runtime included) to 30 kB.
- `scripts/build-runtime-pack.sh` builds that artifact and installs it at the
  same two pack paths: `share/browser/engine/live-debug-panel/index.js` and
  `share/live-debug-panel/`.
- `studio/libs/live-debug-panel/test/browser-artifact.test.mjs` became
  `render/browser/live-debug-panel.browser.spec.ts` in the ordinary render
  Playwright suite, with the same assertions.

Out of scope: `rust/crates/engine-inspector` (#8745 classifies it) and the
`render-projection/src/model_preview.rs` comment that still names Studio
(runtime lane file).

## Evidence

- `scripts/verify-render.sh`: boundaries, artifact freshness, browser typecheck
  and compiled tests pass. In Playwright, 55 passed and 2 failed:
  - `product-browser-host-cadence` (100 calls against a limit of `< 100`) passed
    when rerun at load average 30.
  - `renderer.browser.spec.ts` (a green-pixel threshold, 4 against `> 4`) fails
    the same way on unmodified `origin/main`, so it predates this change.
    Follow-up: #8803.
  - All three `live-debug-panel.browser.spec.ts` tests pass.
- `scripts/verify-docs.sh`: doc links, CI owner routing and the routing
  checker's negative probes pass.
- `scripts/build-runtime-pack.sh` builds, and the pack contains the new panel at
  both install paths. `scripts/test-runtime-pack.sh` then fails at its NativeAOT
  fixture step: `dotnet publish` emits managed assemblies but no
  `CsharpNativeAotTrial.so`. This diff touches no C#, fixture or ABI files.
  Follow-up: #8804. The Doom run below covers that script's panel check: the
  host serves `/engine/live-debug-panel/index.js` and the page mounts it.
- Doom Loading Bay under `rusty dev` (CoreCLR, `--live-debug`), using this
  checkout's runtime pack through the explicit `--engine-source` contributor
  override, because Doom's pinned SDK has a different ABI fingerprint. Driven
  headlessly: F3, then "Open live debug". The panel reports Connected, Tab
  completes `interaction.he` to `interaction.help`, and running it returns the
  interaction help text. Diagnostic events with their live `event-age-ms` and the
  full product/runtime telemetry lane appear. The page logged no console errors.
- Version skew seen on the way: mounting the new panel on Doom's older pinned pack
  (source `4cce6e9d`) shows "telemetry snapshot contains unknown fields". That
  pack's host still sends `workerUpdate` and `outputQueueFloor`, which #8767
  removed from main's client. The Angular panel on main would behave the same
  way. A matched pack does not show it.

## Migration

None for products. The import name, mount API, class names and runtime-pack
paths are unchanged. Contributors who ran `pnpm --dir studio ...` now use
`pnpm --dir render run bundle:live-debug-panel-artifact`.
