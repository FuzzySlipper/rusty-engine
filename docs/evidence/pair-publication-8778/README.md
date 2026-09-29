# CI publication of matched SDK/runtime pairs (#8778)

## What changed

- **New `pair` workflow** (`.github/workflows/pair.yml`). On each `main` push
  that changes Rust, C#, the browser shell, fixtures or the pair scripts, it:
  1. builds the pair with the existing `build-csharp-release-pair.sh`;
  2. runs the existing packaged-consumer check, `test-csharp-release-pair.sh`,
     on that archive;
  3. publishes the same archive with `publish-csharp-release-pair.sh`.

  Markdown-only changes do not trigger it. It does not wait on the other
  workflows.
- **Publisher.** `publish-csharp-release-pair.sh` still verifies the archive and
  refuses to replace a published release. It now also:
  - uploads the assets to a draft, then publishes the draft and moves Latest in
    one `gh release edit`;
  - skips a revision that is already published, and replaces a leftover draft;
  - moves Latest only forward: a late publication of an older revision stays
    available by tag;
  - adds a stable-named `pair-release.json` asset for discovery;
  - publishes pairs as ordinary releases, not prereleases, because GitHub's
    Latest pointer ignores prereleases.
- **Builder.** `build-csharp-release-pair.sh --print-version` prints the version
  for `HEAD`, so the workflow's skip check uses the builder's own naming.
- **Docs.** `docs/csharp-distribution.md` now says CI owns publication and
  gives the discovery URLs. The instructions for publishing by hand are gone.

## Discovery

```text
https://github.com/FuzzySlipper/rusty-engine/releases/latest/download/pair-release.json
https://github.com/FuzzySlipper/rusty-engine/releases/download/csharp-sdk-v<version>/pair-release.json
```

`pair-release.json` holds `version`, `sourceRevision`, `tag`, the archive's
`name`, `sha256`, `bytes` and `url`, the package identity, the ABI, and
`publishedBy` (the Actions run URL). Readers ignore unknown fields; #8780 adds
release information.

## Evidence

**First CI publication.** Pushing `6fd6ee77` ran
[run 36519632423](https://github.com/FuzzySlipper/rusty-engine/actions/runs/36519632423):
build, consumer check and publish in about 9 minutes. It produced
`csharp-sdk-v0.1.0-dev.6fd6ee77ae05`, with archive SHA-256
`7028476a6ce4bbc1290bd99a6ef20e2d543e7d93a234a7a4c2e0874bf0bd92e8` and
66,709,759 bytes. The release is non-draft and non-prerelease, targets
`6fd6ee77ae055fa48127e37d08ebefa0660d2e82`, and has three assets.

**Installed from discovery and run.** The following worked from a directory
outside the checkout:
- fetched `releases/latest/download/pair-release.json`;
- downloaded `archive.url` and its `.sha256`;
- `sha256sum -c` passed, and the digest matched `pair-release.json`.

`test-csharp-release-pair.sh` then passed on those downloaded bytes. It
restores `Rusty.Engine` only from the pair's feed into a disposable consumer,
stages it for CoreCLR, and runs it on the extracted runtime-pack host. It
covers inventory, input, persistence, residency, capture and export, plus a
tamper check.

**Latest moves forward.** The next main push (`59fd6376`, from another lane)
published through
[run 36519650246](https://github.com/FuzzySlipper/rusty-engine/actions/runs/36519650246)
and became Latest. By 04:59Z, CI had published pairs for `6fd6ee77`,
`59fd6376`, `1ba57617`, `76f180c5`, `ff6bda2b` and `7eca06d2`. The pending runs
for other pushes in between were superseded.

**Duplicate publication.**
- **By script:** running the publisher again for a published tag prints
  `already published; leaving it and Latest unchanged` and exits 0.
- **By workflow:** once the queue was empty, I dispatched the workflow at
  `7eca06d2`, which its push run had already published.
  [Run 36524089685](https://github.com/FuzzySlipper/rusty-engine/actions/runs/36524089685)
  passed the skip step in 10 seconds and skipped every build and publish step.
  The release's publish time and asset timestamps (04:59:04 to 04:59:07Z) are
  all earlier than the dispatch (04:59:44Z), and Latest was unchanged.
- **Superseded rerun:** an earlier rerun of run 36519632423 was superseded
  before it started, as described under Limits.

**Failed publication leaves discovery unchanged.** I ran the real publisher
against GitHub with a locally built pair (`174b9112`, pushed temporarily to
`release-probe/8778`). A `gh` shim failed the final `release edit`. After each
of two attempts:
- Latest was still `csharp-sdk-v0.1.0-dev.6fd6ee77ae05`;
- the only release for the tag was a draft;
- no tag existed;
- the public `pair-release.json` URL returned 404.

The second attempt replaced the first draft; the release ID changed, and only
one draft remained. I then deleted the draft and the branch. A failed build or
consumer check stops the job before the publish step runs, so nothing reaches
GitHub.

## Limits

- **Superseded pending runs.** GitHub keeps one pending run per concurrency
  group. When pushes arrive faster than a publication (about 9 minutes),
  intermediate revisions are superseded and get no pair. The newest revision
  always publishes. My first manual dispatch was superseded this way.
- **Local hostfxr lookup.** On this machine, .NET lives in `~/.dotnet` with no
  `DOTNET_ROOT`, so the local consumer check needs `DOTNET_ROOT=$HOME/.dotnet`.
  `actions/setup-dotnet` sets `DOTNET_ROOT` in CI.
- **Studio dependency.** `build-runtime-pack.sh` still builds the live-debug
  panel from `studio/`. The workflow installs Studio's dependencies only while
  `studio/pnpm-lock.yaml` exists. #8794 drops that line when it moves the
  panel; that lane has been told.
- **Tracked feed.** `artifacts/csharp-sdk-feed/*.nupkg` is not a publication
  input. #8752 decides its fate.
- **Old draft.** A draft release, `csharp-sdk-v0.1.0-dev.889a46c90f3e` from
  2026-09-13, predates this work and is untouched.

## Migration

- **Latest is a CI pair.** C# SDK/runtime pairs are now ordinary releases, not
  prereleases, and GitHub's Latest release is the newest pair CI published.
  Tooling that listed prereleases to find pairs should read
  `releases/latest/download/pair-release.json`, or a tag's
  `pair-release.json`, instead.
- **No automatic updates.** Publication does not move product pins. Update a
  product's pinned version explicitly.
- **Contributors do not publish by hand.** To build and exercise a pair
  locally, run `build-csharp-release-pair.sh` and
  `test-csharp-release-pair.sh`.
