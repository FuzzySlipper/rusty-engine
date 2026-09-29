# Packaged world-streaming proof

Build with `-p:RustyEngineFixtureSdkVersion=VERSION` and that pair's SDK feed.
Launch using the same pair's `rusty dev --project ... --live-debug`.

Start runs bounded generated-service exercises for swim hover, climb, flight and
release back to gravity. It then admits two cells with `ApplyResidency` and
verifies a state-only edit and its product-owned undo (the inverse edit). The
ordinary VoxelScenePresentation renders default and variant-specific faces.

`streaming.inspect` must report `committed`, `modeProof:true`, `stateProof:true`.
`movement.start swim`, `movement.start climb` and `movement.start fly` each
animate 120 admitted updates through the Engine solver. Inspect reports accepted
position and movement facts; the yellow character uses that same accepted pose.
The debug commands explicitly select/reset fixture scenarios, not ordinary-input
playability proof. Disposal releases the retained presentation/session owners. Rust tests cover collision ceilings,
water surface equilibrium, invalid requests, rebase/stale commits and teardown.
