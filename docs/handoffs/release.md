# Lane: release

**Tasks, in order:** #8778, #8780. **Start:** now.
Campaign #8777 (developer experience); read the campaign task and its
coordination section. Shared protocol: [README.md](README.md).

## Tasks

- **#8778: publish matched SDK/runtime pairs from CI automatically.**
  - Wire the existing pair builder and publisher into CI:
    `scripts/pack-csharp-sdk.sh` and `scripts/build-runtime-pack.sh`, plus
    whatever publishes today; find it before writing anything.
  - Run the packaged-consumer check on one artifact and publish those same
    bytes.
  - Provide one discoverable "latest eligible" pair and keep explicit revision
    selection. A failed build must leave current discovery unchanged.
- **#8780: publish public API diffs and migration notes with each pair.**
  - Generate a readable diff of the public `Rusty.Engine` surface against the
    previous published pair, and handle the no-baseline case.
  - Add a small authored-note path for semantic changes. The evidence READMEs'
    "Migration" sections (for example
    `docs/evidence/retained-graphics-8737/README.md` and
    `docs/evidence/policy-caps-8742/README.md`) show what such notes look
    like.

## Coordination

- **cli lane (#8779)** consumes your discovery metadata. Publish its shape in
  Den on #8778 early.
- **hygiene lane (#8752)** decides what happens to the tracked
  `artifacts/csharp-sdk-feed/*.nupkg`. Agree on it before either of you
  deletes or relies on those files.
- **build lane (#8775, #8776, #8764)** changes how products build and stage,
  not how the pair is packed. If the pack scripts change under you, rebase
  rather than duplicating them.

## Files

- **Owns:** pair build/publish orchestration; the CI workflow that publishes
  pairs; the release metadata; API diff and migration note generation.

## Evidence

Follow each task's acceptance:
- **#8778:** one real CI publication, installed and run with an ordinary
  packaged consumer; a duplicate or retried publication; a failed build
  leaving discovery unchanged.
- **#8780:** a public signature change and a semantic-only change, each shown
  in the produced release information.
