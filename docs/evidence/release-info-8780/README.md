# Public API diffs and migration notes with each pair (#8780)

## What changed

- **`csharp/Rusty.Engine.ApiSurface`.** A small console tool that prints the
  public C# surface of a `Rusty.Engine.dll` using
  [PublicApiGenerator](https://github.com/PublicApiGenerator/PublicApiGenerator).
  Each DLL loads into its own collectible context. `Rusty.Engine` has no
  package dependencies, so any pair's SDK can be read.
- **`scripts/build-csharp-release-info.sh`.** Takes this pair's archive and,
  optionally, the previous pair's archive, and writes:
  - `api-surface.txt`: this pair's full surface;
  - `api-diff.diff`: a unified diff against the previous surface; each hunk
    header names the enclosing public type;
  - `release-notes.md`: both pair identities, the authored migration notes,
    and the API change summary. The diff is inlined when it is under 60 KB;
    GitHub caps release bodies at 125,000 characters;
  - `release-info.json`: the fields the publisher adds to `pair-release.json`.

  The baseline surface comes from the previous pair's own archive, not from a
  separate asset, so every earlier release can serve as a baseline.
- **Authored notes.** These are the existing `## Migration` sections of
  `docs/evidence/*/README.md`. A section is included when it was added or
  changed between the previous pair's revision and this one. Nothing new needs
  writing: agents already put migration notes there, as in #8737 and #8742.
  Notes that land in a docs-only commit, which publishes no pair, appear in the
  next pair.
- **Publication.** The `pair` workflow downloads the current Latest archive,
  builds the release information, and passes it to the publisher. The
  publisher:
  - uploads the notes, surface and diff as release assets;
  - uses the notes as the release description;
  - adds `releaseInfo` to `pair-release.json`:
    `{previous: {version, sourceRevision, tag} | null, notes, apiSurface, apiDiff | null}`,
    with absolute download URLs.
- **Skipped releases.** `releaseInfo.previous` links each pair to the one
  before it. An update across several pairs follows that chain and reads each
  release's notes. One adjacent diff never claims to cover the whole jump.

## Evidence

See the results sections below.

## Limits

- **Signatures only.** The diff does not capture behaviour. The notes say so,
  and the authored notes carry the rest.
- **Previous means Latest at publication.** When CI supersedes pending runs,
  the skipped revisions' changes appear in the next published pair's diff and
  notes, because both cover the whole range between the two published
  revisions.
- **Out-of-order publication.** If an older revision publishes late, its
  "previous" is the newer Latest, and its diff runs backwards. It does not take
  Latest, so updates never see it by default.
- **CLI display is #8779's.** `pair-release.json` exposes the notes and
  links. Showing them in `rusty` update and help output belongs to #8779, which
  has the contract.

## Migration

- **`pair-release.json`.** It gains `releaseInfo`. Pair releases now carry
  `release-notes.md`, `api-surface.txt` and, when the surface changed,
  `api-diff.diff`.
- **Updating a product.** Read `releaseInfo.notes` for every pair between the
  current pin and the target. Follow `releaseInfo.previous` back to the pin.
