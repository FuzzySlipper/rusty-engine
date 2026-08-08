# Rusty Engine agent code atlas

This atlas routes an unfamiliar agent to the owner of a mechanism without
requiring campaign history or Den access. Current planning may live elsewhere;
this document describes committed repository surfaces.

The optional Rust `gameplay-rules` package support and isolated TypeScript
authoring workspace are implemented. The workspace remains a separately gated
optional surface and is never an ordinary provider dependency.

The completed three-composition mechanics/rules evidence, exact consumer pins,
measured limits, and deliberate stopping point are indexed in the
[gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md).

The completed runtime voxel texture/atlas owner chain, exact public Studio
consumer, measured costs, and deliberate stopping point are indexed in the
[textured voxel campaign closeout](textured-voxel-campaign-closeout.md).

## How to use this atlas

1. Read [design.md](design.md) for the provider boundary.
2. Choose the coarse owner map below before opening implementation files.
3. Follow its primary paths and public surfaces rather than searching by a
   historical Asha concept name.
4. Run the focused acceptance gate named by the map, then the appropriate
   repository gate.
5. If a map disagrees with source, manifests, or tests, update the map. Do not
   force current code to match stale navigation prose.

## Ownership flow

```text
downstream game policy and orchestration
        |
        +--> entity-state / state-machine
        +--> gameplay-mechanics
        +--> gameplay-rules <--> rules/ (optional build-time authoring)
        +--> environment-authoring --> authored-scene
        +--> content-store / asset-catalog / asset-import
        +--> engine-spatial --> core-* / svc-*
        +--> voxel-asset / voxel-annotation / voxel-convert
        |                         \--> voxel-object-runtime
        +--> render-model --> render-projection --> render-presentation
                                      |
                                      +--> render-host-contracts
                                      +--> renderer-webview-host --> private render/ artifact
                                      +--> studio/ viewport and tools

rusty-engine is the complete downstream facade over every public Rust library.

engine-inspector reads these surfaces; runtime libraries do not depend on it.
```

## Code maps

| Area | Map |
|---|---|
| Entity components, atomic mutation, relationships, snapshots, state machines | [Entity state and state machines](code-map/entity-state-and-state-machines.md) |
| Stats, tracks, sources, effects, items, equipment, damage, restoration | [Gameplay mechanics](code-map/gameplay-mechanics.md) |
| Optional opaque rules packages, TypeScript authoring, exact dependencies, provenance, canonicalization, and diagnostics | [Gameplay rules](code-map/gameplay-rules.md) |
| Canonical voxel space, collision, navigation, mesh, motion, triggers, edits | [Spatial mechanisms](code-map/spatial-mechanisms.md) |
| Content manifests, catalogs, imports, prefabs, and authored scenes | [Content, assets, and scenes](code-map/content-assets-and-scenes.md) |
| Stored voxel artifacts, annotations, GLB conversion, and object playback | [Voxel assets and conversion](code-map/voxel-assets-and-conversion.md) |
| Recipe planning and materialization into authored scenes | [Environment authoring](code-map/environment-authoring.md) |
| Versioned Rust render frames, host contracts, projection, and complete facade | [Rust render model and projection](code-map/rust-render-model-and-projection.md) |
| Engine-private TypeScript retained projection, compiled artifact, Three backend, and webview host | [Renderer workspace and hosts](code-map/renderer-workspace-and-hosts.md) |
| First-party Angular/Nx authoring product and adapter boundary | [Studio](code-map/studio.md) |
| Read-only structured inspection and diagnostics | [Inspection and diagnostics](code-map/inspection-and-diagnostics.md) |
| IDs, math, time, coordinates, voxel values, and focused algorithms | [Core and service foundations](code-map/core-and-service-foundations.md) |

## Stable orientation sources

- [Repository README](../README.md)
- [Agent guidance](../AGENTS.md)
- [Canonical design](design.md)
- [Gameplay mechanics campaign closeout](gameplay-mechanics-campaign-closeout.md)
- [Textured voxel campaign closeout](textured-voxel-campaign-closeout.md)
- [Rust source organization](topics/development/rust-style.md)
- [Downstream Engine revision contract](topics/development/downstream-engine-revisions.md)
- [Cargo workspace](../Cargo.toml)
- [Provider gate](../scripts/verify.sh)
- [Gameplay rules gate](../scripts/verify-rules.sh)
- [Renderer gate](../scripts/verify-render.sh)
- [Studio gate](../scripts/verify-studio.sh)
- [Exact demo/Studio integration gate](../scripts/verify-studio-demo-integration.sh)

## Freshness and validation

This atlas intentionally does not port Asha's generated inventory. Rusty Engine
has a smaller, coarser owner set, and a generated symbol/file dump would obscure
the public mechanisms these maps are meant to explain.

Validate the hard dependency direction, curated ownership assignments, and
ordinary links directly:

```bash
python3 scripts/dependency_boundary_check.py
python3 scripts/code_map_freshness.py
./scripts/check-doc-links.sh
```

The dependency checker follows resolved normal/build workspace edges from
`cargo metadata --locked --all-features` and hard-fails only the explicit
architecture inversions in [design.md](design.md). The code-map checker compares
Cargo members with links in each `Primary paths` section. It reports missing
assignments, stale crate paths, and unresolved primary paths, emits GitHub
annotations and a step summary in Actions, and deliberately returns success so
curated grouping remains reviewer-owned.

## Non-claims

- This is not a live planning or task-status mirror.
- This is not a universal architecture layer or assignment-cell system.
- This does not make internal paths public to downstream consumers.
- This does not transfer product policy, browser behavior, or Studio ownership
  into ordinary Rust provider crates.
