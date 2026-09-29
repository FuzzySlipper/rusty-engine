# Source-root UI and content reload without runtime replacement (#8743)

## What changed

**Trigger route.** `rusty dev` already writes framed commands to the
supervisor's stdin, and the supervisor already writes `serve` to the runtime's
stdin. A second command, `reload-assets`, reuses both pipes. This was picked
over a runtime HTTP endpoint because the endpoint would need port discovery in
`rusty dev` and a new route that the browser could also reach.

**Change routing (`rusty-cli`).** `rusty dev` diffs its watch snapshots.
- If every changed file is under an asset root, it runs only the SDK target
  `StageRustyEngineProductAssets` and sends `reload-assets`. The asset roots
  are `RustyEngineProductUiSourceRoot`, `RustyEngineProductUiRoot`, and each
  `RustyEngineContentBundle` root.
- Anything else takes the existing path: a full restage and `replace-runtime`.
- Loose content is not an asset root. The product receives it as a
  create-time snapshot copied into managed memory (`ProductContent.Files`), so
  a loose edit needs a new product.

**Runtime.**
- The supervisor forwards `reload-assets` to the serving runtime.
- The runtime's stdin reader, which only waited for EOF before, now also
  accepts `reload-assets` lines. For each one it:
  - rebuilds the browser bundle from the staged `ui/` and swaps it into the
    dev host (the host already kept it behind a lock);
  - calls `ProductDevRuntime::reload_content`. `CsharpProductRuntime`
    re-admits the staged bundle inventory (`.rusty-bundles.json`) through the
    existing `bind_content_bundles`, under the same session lock as every
    product call.
- On failure the old bundle or inventory stays in place, and the runtime
  prints `RUSTY_HOST DEV_HOST_ASSET_RELOAD: …`.

**Identities.**
- **UI** is the explicitly mutable route. Product UI entries are served
  `Cache-Control: no-store`, which was already true.
- **Bundle content** is immutable per identity. A new open gets the file's
  manifest SHA-256 identity, so changed bytes get a new identity (and a new
  content-addressed renderer URL if opened as a resource). Nothing re-verifies
  at runtime beyond the existing open-time check.
- **Old references.** Bundles and references opened before the edit hold
  their own `Arc` bytes, so they keep the old content.

**SDK build isolation (`Rusty.Engine.targets`).**
- **`BuildRustyEngineProductUi`** runs a product's
  `RustyEngineProductUiBuildCommand` with MSBuild `Inputs`/`Outputs`:
  - inputs: files under the UI source root except the output, the project
    file, and `RustyEngineProductUiInput` extras;
  - outputs: a stamp plus the UI entry. The stamp lists the files the last
    UI build produced, and the build reruns when any of them is missing (see
    the review fix below).

  So an unchanged or C#-only build skips the UI compiler, and a missing output
  reruns it.
- **`StageRustyEngineProductAssets`** covers the UI build, the UI/content sync
  and the bundle inventory. It is split out of `StageRustyEngineCoreClrProduct`,
  which now depends on it and keeps its `product.json`-last order.
- **UI-root check.** The UI-root existence check moved after the UI build, so
  a clean clone no longer needs its UI output to exist before composition.
- **`RustyEngineWatchPaths` default** moved to the targets file. There it sees
  the product's own UI and content roots; in props it saw only the defaults.

## Evidence

The exercise product is in [`product/`](product/). It has:
- a TS UI compiled by `tsc`, with each invocation logged to
  `tsc-invocations.log`;
- a `notes` content bundle and a loose file;
- live-debug commands:
  - `exercise.identity` (pid plus a per-construction GUID);
  - `exercise.read` (opens the bundle now);
  - `exercise.hold` / `exercise.held` (a bundle opened earlier and kept);
  - `exercise.loose`.

It ran against a runtime pack and SDK package built from this change (SDK
`0.1.0-reload8743.1`), with `DOTNET_ROOT` set.

### Compiler invocations (MSBuild directly)

"product dll" is `obj/Debug/net10.0/Exercise.dll`, the `CoreCompile` output.

| Step | Target | tsc runs | product dll rebuilt | wall |
|---|---|---|---|---|
| clean | `StageRustyEngineCoreClrProduct` | 1 | yes | 13.6 s |
| unchanged | `StageRustyEngineCoreClrProduct` | 0 | no | 5.2 s |
| C#-only edit | `StageRustyEngineCoreClrProduct` | 0 | yes | 15.6 s |
| UI edit | `StageRustyEngineProductAssets` | 1 | no | 4.4 s |
| unchanged | `StageRustyEngineProductAssets` | 0 | no | 1.7 s |
| bundle content edit | `StageRustyEngineProductAssets` | 0 | no | 1.3 s |
| UI output deleted | `StageRustyEngineProductAssets` | 1 | no | 2.4 s |
| `ui/tsconfig.json` edited | `StageRustyEngineProductAssets` | 1 | no | 3.3 s |

### Live `rusty dev` session

All of these ran in one session, starting with `pid=209537 instance=2c4f0bb5…`:

| Edit | `rusty dev` path | Result | Same pid/instance |
|---|---|---|---|
| `ui/main.ts` v1→v2, v2→v3 | assets | `/product-ui/main.js` serves the new label, `no-store` | yes |
| `content/notes/message.txt` v1→v2 | assets | new open: `note v2 sha=4e880fb6c573`; bundle held from before: `note v1 sha=d5cd8c4150cc` | yes |
| delete `message.txt`, add `other.txt` | assets | new open: `missing`; `other.txt` opens; held bundle still `note v1` | yes |
| `ui/main.ts` type error | assets, stage fails (`tsc` exit 1) | `restage-failed phase=stage-assets`; UI stays v3 | yes |
| UI stage copies `data.bin` (no admitted media type) | assets, runtime reload fails | `DEV_HOST_ASSET_RELOAD: … data.bin has no admitted content type`; UI stays v3 | yes |
| remove `data.bin`, UI v5 | assets | UI v5 served | yes |
| `content/loose.txt` v1→v2 | full restage, `runtime-replaced` | `pid=226909`, new instance, `loose v2` | no (intended) |

`rusty dev` logs `assets-restaged` once it has sent the command. The runtime
logs its own `assets-reloaded` or `DEV_HOST_ASSET_RELOAD` result.

### Checks

- `cargo test` passes on `rusty-cli` (18 + 1), `product-dev-host` (43 + 29)
  and `csharp-product-runtime` (43 + 27). New tests cover:
  - the snapshot diff;
  - asset-root derivation;
  - the `reload-assets` frame in both encoders.

  One existing test fails when `TMPDIR` is a long path (a Unix socket path
  over `SUN_LEN`) and passes with the default `/tmp`.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes. It needs
  `DOTNET_ROOT`, and a `TMPDIR` whose path does not start with the checkout's
  path; otherwise its source-path leak check reports a false positive.
- Clippy adds nothing in the touched files. The #8757 baseline lints, plus an
  existing `product-dev-host/src/model.rs:609` warning, are unchanged.

## Not covered

- **Page reload.** An open page still needs a manual refresh to load new UI,
  as it did before. Nothing pushes a reload to the page (#8802).
- **Loose content** still replaces the runtime.
- **Dagger and CraftSurvive** pin older pairs (`58f6316dad11`,
  `1aecde636cd3`) that predate other breaking campaign changes. Their `tsc`
  hooks are migrated to `RustyEngineProductUiBuildCommand` on local
  `engine-8743-ui-build` branches:
  - Dagger `57d0a6f`;
  - CraftSurvive `9b1f7f1`.

  Merge them when each product moves to a pair containing this change (#8800).
  Their new SDK properties do nothing on the old pins.
- **Staging window.** Since #8798 (`70a7745d`), a bundle open takes its
  identity from the staged manifest instead of re-hashing. The reload
  re-admits that manifest after every asset restage, so a changed file gets
  its new identity. For the moment while MSBuild is copying files, before the
  reload arrives, an open of a same-length edited file would get the new bytes
  under the old identity; a different length is refused. This was not
  observed. It is dev-only and closes at the reload, so nothing was added for
  it.
- **First debug command.** The first live-debug command after a runtime start,
  with no browser attached, returned `DEV_HOST_OUTPUT_BASELINE: incremental
  output arrived before a complete binding baseline`; later commands succeed.
  This change does not touch output publication (#8801).

## Migration

- **Compiled UI.** Declare `RustyEngineProductUiBuildCommand` and
  `RustyEngineProductUiSourceRoot`, plus `RustyEngineProductUiInput` for
  dependencies outside the source root. Remove targets that ran the compiler
  `BeforeTargets` the SDK's composition or staging targets.
- **Custom targets.** A product target hooked to `StageRustyEngineCoreClrProduct`
  to change UI or content should hook `StageRustyEngineProductAssets` instead,
  so asset-only restages run it too.
- **`ProductDevRuntime` implementers** get a defaulted `reload_content`; no
  change is needed.

## Review fixes

**A missing secondary UI output did not rebuild.** `Outputs` named only the
stamp and the entry. With a multi-module UI, deleting an imported module left
both in place, so the build skipped and staging then removed the module from
the staged UI.
- The fix: the stamp now lists every file under the UI root after a
  successful build (`WriteLinesToFile`, which also creates the stamp's
  directory; `Touch` did not). `_CheckRustyEngineProductUiOutputs` runs first
  and deletes the stamp when a listed file is gone.
- Why not list every output in `Outputs`: MSBuild would then compare
  timestamps against the oldest output. A compiler that leaves unchanged
  modules untouched (`tsc --incremental`, most bundlers) would rerun on every
  C#-only build. The check here is existence only.
- Evidence: the reviewer's MSBuild probe (`main.js` imports `dep.js`) run
  against both versions of the targets file.

  | Step | Old targets | New targets |
  |---|---|---|
  | first build | 1 build, stages `dep.js main.js` | 1 build, stages `dep.js main.js` |
  | unchanged | — | skipped |
  | delete `out/dep.js` | skipped; staged UI is `main.js` only | rebuilt; stages `dep.js main.js` |
  | unchanged | — | skipped |
  | source edited | — | rebuilt |

**A failed content reload still published the new UI.**
`ProductDevAssetReload::reload` swapped the served bundle and then ran the
fallible content reload. It now runs the content reload first and swaps the
UI only when that succeeds, so a reported failure leaves both old.
`a_failed_content_reload_keeps_the_served_ui` covers both orders of outcome.

**Checks.**
- `product-dev-host` passes (73 tests). Its clippy run still stops at the
  #8757 `model.rs` lint only.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke` passes with the
  changed targets.
