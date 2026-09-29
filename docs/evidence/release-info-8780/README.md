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

**Semantic-only change, in a real CI publication.** Pushing `1f2d755b` ran
[run 36524229020](https://github.com/FuzzySlipper/rusty-engine/actions/runs/36524229020).
It built the pair, ran the consumer check, and built release information
against the Latest pair at the time, `0.1.0-dev.7eca06d2e972`. Then it
published `csharp-sdk-v0.1.0-dev.1f2d755b43a7`. All of the following came from
the Latest discovery URL:

- `pair-release.json` → `releaseInfo.previous` names `0.1.0-dev.7eca06d2e972` /
  `7eca06d2…0472`; `notes` and `apiSurface` URLs return 200; `apiDiff` is
  `null`.
- `release-notes.md` (also the release description) names both pair
  identities. It says "No public signature changes since
  `0.1.0-dev.7eca06d2e972`" and includes two authored notes:
  - `pair-publication-8778`, which landed in a docs-only commit that published
    no pair, so it was carried into this one;
  - `release-info-8780`.

**Public signature change.** I produced this with the same script from two real
published pairs:
`--pair` the CI pair `0.1.0-dev.6fd6ee77ae05`, and `--previous`
`0.1.0-dev.58f6316dad11`. The result was 43 lines added and 456 removed; the
83 KB diff was too large to inline, so it was linked. The #8737 change shows
as:

```diff
@@ ... @@ public interface IGraphicsService
         Rusty.Engine.MeshPartition PartitionMesh(Rusty.Engine.MeshPartitionRequest arg0);
-        void PublishAttachedSnapshot(Rusty.Engine.AttachedAppearanceSnapshotRequest arg0);
+        void PublishChanges(Rusty.Engine.AppearanceChangesRequest arg0);
         void PublishSnapshot(System.ReadOnlySpan<Rusty.Engine.AppearanceFact> values);
```

The same notes gathered the changed `## Migration` sections of #8737, #8738,
#8739, #8741 and #8742. Evidence READMEs without such a section were omitted.

**No previous pair.** Running without `--previous` wrote `previous: null` and
`apiDiff: null`. The notes say there is no baseline pair, and the full surface
is in `api-surface.txt`.

**CLI and discovery links.** Discovery works through `pair-release.json`
as shown above. No `rusty` update command exists yet; #8779 owns it and has the
contract.

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
