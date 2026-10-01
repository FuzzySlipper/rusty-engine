# Generated spatial artifact fixture

`valid.json` is a three-cell floor in the Engine's spatial content
format. It contains a two-triangle collision mesh and precomputed navigation.
`bad-bounds.json` differs only in bounds whose minimum exceeds their maximum,
producing `CSHARP_SPATIAL_CONTENT_BOUNDS`.
`valid.rspatial` holds the same facts in the binary encoding
(`docs/csharp-lifecycle.md#generated-level-artifact-admission`); the check
admits it and compares its counts, navigation projection hash and queries
with the JSON admission.

Run `scripts/test-csharp-sdk-package.sh --coreclr-smoke` from the Engine root.
The packaged consumer runs `fixtures/csharp-spatial-artifact/SpatialArtifactChecks.cs` through
CoreCLR. It seeds resident voxel content, admits the floor, exercises collision
and navigation, catches the generated named refusal, compares retained state,
and admits a valid artifact again.

Generation rules, required connectivity, portal/socket semantics and Procgen
provenance remain with the generator/importer or product. This fixture proves
the Engine spatial admission boundary; it does not consume the Procgen floor
schema. See `docs/csharp-lifecycle.md#generated-level-artifact-admission`.
