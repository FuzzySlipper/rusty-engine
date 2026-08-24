# Voxel vignette visual gate

This is the first user-facing visual gate for Den #6925. It admits four local
experimental inputs from Asset Pipeline run `voxel-vignette-128-20260823-003`:
terrain, shrine-nano, gpt-tree, and door. They are palette-unlit producer
derivatives from `palette-unlit-6925-20260824-001`. The source layout's `vignetteId`
still says `002`; this app intentionally presents run `003` as its identity.

Run `./scripts/stage-voxel-vignette-playtest-assets.sh` from the repository
root before serving. It defaults to
`/home/dev/asset-pipeline/live-evidence/palette-unlit-6925-20260824-001`; set
`VOXEL_VIGNETTE_ASSET_SOURCE_ROOT` only to an explicit equivalent producer
output. If that evidence is missing, the script names both the stable expected
location and the producer command/README path; it never generates or mutates
assets. The checked manifest records every exact SHA-256 and byte length. The
copied GLBs are ignored and must remain uncommitted; their source
envelopes/licenses are not recorded, so they are local experimental visual
inputs only.

The current route is deliberately temporary: palette-unlit static GLBs pass through the
public `defineAnimatedMesh` API with empty clips and `defaultClip: null`.
Runtime voxel/object admission is not wired, the conventional-mesh comparator
is absent negative evidence, and collision is not wired. This is a visual
comparison scene, not a durable static-mesh profile or a production game.
