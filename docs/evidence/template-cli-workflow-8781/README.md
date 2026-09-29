# Template and guidance on the rusty CLI workflow (#8781)

## What changed

- **Template** ([rusty-template](https://github.com/FuzzySlipper/rusty-template)):
  - `d7630ec` (#8779) deleted the install, build and run scripts and
    `NuGet.Config`, and reduced the pin to one element.
  - `c84ca82` added the `engine-pair` workflow.
  - `15e9e5e` rewrote the architecture, AGENTS and README text that still
    described the composition project, product feed and scripts. AGENTS now
    points at the published SDK guide instead of `../rusty-engine`.
- **`engine-pair` workflow.** Runs every six hours or by dispatch
  (`version` input optional):
  - bootstraps `rusty`, runs `rusty update`, then `rusty build`;
  - runs `rusty dev` and requires `product-bootstrap.json` to name
    `rusty-template`;
  - only then commits the pin (with the notes chain in the message), pushes,
    and closes any open `engine-pair-update` issue.
  - On any failure it pushes nothing. The run is red, and it opens (or
    comments on) an `engine-pair-update` issue with the compiler/host output
    and the release notes to read.
- **SDK guide split.** `docs/csharp-sdk.md` (2,131 lines) is now a 132-line
  entry page: start, run, update, troubleshoot, capability references, the
  contributor override, product architecture and the missing-capability
  workflow. The capability sections moved unchanged into
  `csharp-product-project.md`, `csharp-lifecycle.md`, `csharp-helpers.md`,
  `csharp-offline-images.md` and `csharp-implicit-surfaces.md`.
  - Inbound anchors in docs, fixtures and `PACKAGE_README.md` were rewritten
    to the new files. A check confirmed that every `csharp-*.md#anchor` link
    in tracked docs resolves.
  - The product-project reference now shows the pin and `rusty dev`/`build`
    instead of a pack path and hand-configured feed.
- **Den.**
  - `rusty-engine/downstream-csharp-sdk-runbook` is rewritten around the CLI
    (pin, commands, update flow, troubleshooting, stop rule).
  - `rusty-engine/downstream-csharp-agent-brief` no longer teaches the
    `Product.NativeProduct` composition project. It keeps the ownership model
    and points at the runbook.

## Evidence

- **Failed update.** On a probe branch at the template's old pin
  `360a1ce508a9`, a file used `Rusty.Engine.AuthoredContentStorePublishReceipt`
  (present in that pair, removed with the content store in #8763). Dispatching
  the workflow on the branch
  ([run 36544435902](https://github.com/FuzzySlipper/rusty-template/actions/runs/36544435902))
  gave:
  - Propose succeeded; Build failed with `CS0234`; Serve and Advance were
    skipped; Report succeeded.
  - [Issue #1](https://github.com/FuzzySlipper/rusty-template/issues/1) was
    opened with the compiler error and the nine-pair notes chain.
  - The branch pin stayed `360a1ce508a9`. The probe branch was then deleted.
- **Successful update.** Dispatching on `main`
  ([run 36544569630](https://github.com/FuzzySlipper/rusty-template/actions/runs/36544569630))
  gave:
  - build and serve passed;
  - `ed4fd7c Adopt Engine pair 0.1.0-dev.8f08ab04275f` was pushed by the
    workflow;
  - issue #1 was closed with "The template adopted Engine pair
    0.1.0-dev.8f08ab04275f in ed4fd7c."
- **Fresh product** ([`fresh-product.txt`](fresh-product.txt)). Starting from
  an empty HOME, the published bootstrap, a clone of the template from GitHub,
  and only its README commands:
  - `rusty status` reported not installed (exit 1);
  - `rusty install`, `rusty build` and `rusty dev` each worked, and the
    product served `rusty-template`.
- **Ordinary instructions** (template, SDK entry page, Den runbook) contain no
  Engine checkout, Cargo, handwritten interop, product feed or composition
  project, except as prohibitions and the explicit contributor
  `--engine-source` section.

## Migration

- Links to `docs/csharp-sdk.md#<section>` for capability sections now live in
  the reference file named on the entry page. The anchors are unchanged.
- Products created from the template before `d7630ec` can follow the #8779
  migration notes.
