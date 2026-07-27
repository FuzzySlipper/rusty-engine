# Rusty Engine agent code atlas

This atlas routes an unfamiliar agent to the owner of a mechanism without
requiring campaign history or Den access. Current planning may live elsewhere;
this document describes committed repository surfaces.

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
        +--> environment-authoring --> authored-scene
        +--> content-store / asset-catalog / asset-import
        +--> engine-spatial --> core-* / svc-*
        +--> voxel-asset / voxel-annotation / voxel-convert
        |                         \--> voxel-object-runtime
        +--> render-model --> render-projection --> render-presentation
                                      |
                                      +--> render/ packages and hosts
                                      +--> studio/ viewport and tools

engine-inspector reads these surfaces; runtime libraries do not depend on it.
```

## Code maps

| Area | Map |
|---|---|
| Entity components, atomic mutation, relationships, snapshots, state machines | [Entity state and state machines](code-map/entity-state-and-state-machines.md) |
| Stats, tracks, sources, effects, items, equipment, damage, restoration | [Gameplay mechanics](code-map/gameplay-mechanics.md) |
| Canonical voxel space, collision, navigation, mesh, motion, triggers, edits | [Spatial mechanisms](code-map/spatial-mechanisms.md) |
| Content manifests, catalogs, imports, prefabs, and authored scenes | [Content, assets, and scenes](code-map/content-assets-and-scenes.md) |
| Stored voxel artifacts, annotations, GLB conversion, and object playback | [Voxel assets and conversion](code-map/voxel-assets-and-conversion.md) |
| Recipe planning and materialization into authored scenes | [Environment authoring](code-map/environment-authoring.md) |
| Versioned Rust render frames and renderer-neutral projection | [Rust render model and projection](code-map/rust-render-model-and-projection.md) |
| TypeScript retained projection, Three backend, browser and headless hosts | [Renderer workspace and hosts](code-map/renderer-workspace-and-hosts.md) |
| First-party Angular/Nx authoring product and adapter boundary | [Studio](code-map/studio.md) |
| Read-only structured inspection and diagnostics | [Inspection and diagnostics](code-map/inspection-and-diagnostics.md) |
| IDs, math, time, coordinates, voxel values, and focused algorithms | [Core and service foundations](code-map/core-and-service-foundations.md) |

## Stable orientation sources

- [Repository README](../README.md)
- [Agent guidance](../AGENTS.md)
- [Canonical design](design.md)
- [Rust source organization](topics/development/rust-style.md)
- [Cargo workspace](../Cargo.toml)
- [Provider gate](../scripts/verify.sh)
- [Renderer gate](../scripts/verify-render.sh)
- [Studio gate](../scripts/verify-studio.sh)

## Freshness and validation

This atlas intentionally does not port Asha's generated inventory. Rusty Engine
has a smaller, coarser owner set, and a generated symbol/file dump would obscure
the public mechanisms these maps are meant to explain.

For now, validate links and the actual workspace directly:

```bash
./scripts/check-doc-links.sh
cargo metadata --format-version 1 --locked --no-deps
```

A future advisory checker may compare Cargo members and isolated package
manifests against the owner pages. It should report drift without turning these
curated explanations into a generated governance system.

## Non-claims

- This is not a live planning or task-status mirror.
- This is not a universal architecture layer or assignment-cell system.
- This does not make internal paths public to downstream consumers.
- This does not transfer product policy, browser behavior, or Studio ownership
  into ordinary Rust provider crates.
