# Lane: cli

**Tasks, in order:** #8779, #8781.
**Start:** after #8775 (build lane) is on main. #8775 rewrites `rusty-cli`
`stage_product` and `Rusty.Engine.targets`, the same files #8779 extends.
Watch #8775 or #8779 in Den for the landing note.
Campaign #8777; read the campaign task and its coordination section.
Shared protocol: [README.md](README.md).

## Tasks

- **#8779: SDK/runtime install, update and discovery in the `rusty` CLI.**
  - Give a product one explicit pair pin.
  - Install into a shared cache that works offline.
  - Updating must be explicit and edit the pin; installing or launching never
    advances it.
  - Add a status/inspection command, with help text that includes ordinary
    product examples.
  - Provide one Engine-owned bootstrap route.
  - Replace the duplicated installer scripts and per-product feed copies in the
    template, in Dagger, and in one other consumer.
- **#8781: move the product template and bootstrap guidance onto the CLI
  workflow.**
  - Split `docs/csharp-sdk.md` into a short run/update/troubleshoot entry page
    plus linked capability references.
  - Update the stale Den downstream briefing.
  - Automate template adoption of newly published pairs, with a visible
    failure when an update breaks.

## Coordination

- **release lane (#8778, #8780)** provides the published pairs, discovery
  metadata and migration notes. Use existing published pairs until theirs
  land.
- **content lane (#8743)** may add a reload route that `rusty dev` calls. Read
  what they recorded on #8743 before changing runtime resolution.
- **hygiene lane (#8752)** owns obsolete audit docs and tracked outputs.
- **#8781's split of `docs/csharp-sdk.md`** is a hot-file edit: many lanes add
  small sections there. Do it in one commit, rebase just before landing, and
  keep every section other lanes added.

## Files

- **Owns:** the `rusty-cli` install, update, status and bootstrap commands;
  `rusty-template` (at `/home/dev/rusty-template`); the downstream pin and
  installer scripts in the representative consumers; the new SDK guide
  structure.

## Evidence

- From a fresh environment, using only the bootstrap route and CLI help:
  install a pinned pair, build and run a product, inspect it, update it.
- Repeat with the cache offline and with a missing prerequisite.
- **#8781:** a fresh product from the template; one automated template update
  and one failed update.
