# Tasks 8675, 8684, 8685 — local implementation evidence

These are **precommit local packaged** checks, using SDK
`0.1.0-task868x-local2` and its matching ABI/runtime. Exact immutable pair
publication, downstream adoption, CI, and acceptance are recorded in Den.

- CoreCLR and NativeAOT: the retained water voxels remain present; the ray hits
  floor voxel (0,0,0), and the character rests near Y=1.92. Six 3/4/9/27/64-cell
  stone/water transactions are accepted with later update counts increasing.
- A separate 64-cell solid enclosure returns
  `unresolved-character-controller-penetration` with depth 0.35. The managed
  fixture handles that diagnostic and continues ordinary updates. It does not
  pretend an embedded character can always escape bounded recovery.
- The supervised fixture's SIGINT result is exit 0. Its log contains one
  `VOXEL_PROOF disposed` marker and the file diagnostic contains one
  `voxel-proof/DISPOSED` row. The parent mock-host regression separately tests
  both SIGINT and SIGTERM.
- CraftSurvive's isolated `CRAFTSURVIVE_PROOF=voxel-edits` run accepts 1,2,3,4,9,
  27,64-cell water and stone transactions and continues through update 200.
  Its lake ray hits (0,-1,-26), below water level 2, and the player reports
  Swimming, immersion 0.876. Ordinary supervised shutdown exits 0.
- A local Dagger entry/world browser run reaches music playback and produces
  exactly one `daggerfall.music/cue.retired` and no
  `CSHARP_AUDIO_CLIP_IN_USE` on SIGINT. Its first terminal wrapper returned
  143 despite successful stop markers, so this is disposal evidence only;
  final exact-pair proof must capture the actual supervisor exit code.

The original CraftSurvive stall was reproduced by placing a solid block around
the actor: ApplyEdits succeeded, then ProposeCharacterStep failed. That bridge
discarded the native error. The new retained operation receipt preserves the
native code and depth. Editing beside the actor and passable water work without
a two-cell limit; product placement/crush policy remains downstream.

Independent Crew browser observations saw the retained purple voxel structure,
and CraftSurvive's water and grassy terrain, with no page errors in successful
runs. Captures/reports reside under `/tmp/868x-playtest-state`,
`/tmp/868x-craft3-playtest.json`, and `/tmp/8675-dagger-shutdown-proof.json`.
Screenshots do not establish hidden character contact; native receipts do.

Warning reports are report-only: no compatible baseline was supplied. Chromium
reported WebGL ReadPixels performance stalls during capture; Engine capture
completed without task-breaking diagnostics in these final fixture exercises.
No clean warning-delta claim is made. Earlier failed proof attempts are not
acceptance: the broad Craft substrate harness deliberately attempted a rejected
partial graphics snapshot; the isolated probe avoids it. A separate direct
CoreCLR native crash and direct-host signal behavior are retained in Engine
follow-up **8686**, outside the ordinary supervised-dev proof.
