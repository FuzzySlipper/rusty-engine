# Studio

## Purpose

Route the separately isolated first-party authoring product, its renderer
viewport, and its closed external-project adapter protocol.

## Owns

- Angular/Nx application composition and editor shell.
- Adapter client protocol, viewport integration, voxel editor, user settings,
  and Studio-owned host services.
- Studio migration accounting and explicit owner-adoption evidence.
- Browser and explicit external-demo and voxel-consumer integration gates.

## Does not own

- Ordinary Rust provider state or game-specific adapter implementations.
- The loading-bay product, its content, or its current task queue.
- A general Engine scheduler, gameplay runtime, or browser dependency for Rust.

## Primary paths

- [`studio/apps/studio-app`](../../studio/apps/studio-app)
- [`studio/libs/adapter-client`](../../studio/libs/adapter-client)
- [`studio/libs/editor-shell`](../../studio/libs/editor-shell)
- [`studio/libs/viewport`](../../studio/libs/viewport)
- [`studio/libs/voxel-editor`](../../studio/libs/voxel-editor)
- [`studio/libs/user-settings`](../../studio/libs/user-settings)
- [`studio/libs/editor-shell/src/entity-inspector.ts`](../../studio/libs/editor-shell/src/entity-inspector.ts)
- [`studio/scripts/check-entity-inspector-boundary.mjs`](../../studio/scripts/check-entity-inspector-boundary.mjs)
- [`studio/scripts/studio-adapter-host.ts`](../../studio/scripts/studio-adapter-host.ts)
- [`studio/scripts/studio-adapter-process.ts`](../../studio/scripts/studio-adapter-process.ts)
- [`studio/scripts/studio-service.ts`](../../studio/scripts/studio-service.ts)
- [`studio/ops/rusty-studio.service`](../../studio/ops/rusty-studio.service)
- [`scripts/verify-studio-generic-browser-integration.sh`](../../scripts/verify-studio-generic-browser-integration.sh)
- [`studio/test/generic-browser`](../../studio/test/generic-browser)
- [`studio/test/entity-inspector-consumer-browser`](../../studio/test/entity-inspector-consumer-browser)
- [Studio migration contract](../studio-migration-contract.md)
- [Studio adapter protocol](../studio-adapter-protocol.md)
- [Persistent generic Studio service](../topics/studio-service.md)
- [Persistent Studio concurrency exploration](../reviews/2026-08-studio-service-concurrency-exploration.md)
- [Downstream renderer and Studio boundary](../topics/development/downstream-renderer-and-studio.md)
- [Downstream Entity inspector extensions](../studio-downstream-entity-inspector-extensions.md)
- [Voxel mesh data plane](../topics/voxel/voxel-mesh-data-plane.md)
- [Runtime voxel surface textures](../topics/voxel/voxel-surface-textures.md)
- [Reconstructed voxel surfaces](../topics/voxel/reconstructed-surfaces.md)
- [Textured voxel campaign closeout](../textured-voxel-campaign-closeout.md)

## Public downstream surfaces

- The closed adapter protocol defines operations a downstream product may
  implement without granting Studio direct access to engine or game internals.
- The asset-source chooser truthfully includes `.gltf`; the request still
  carries only the selected project/host path. A downstream trusted adapter may
  use `asset-import`'s resource-discovery API to load the bounded closure and
  must never delegate URI resolution to Angular or the browser. The published
  runtime resource remains one hash-verified GLB.
- The external consumer is always selected explicitly by an integration
  command; ordinary Engine work never scans a sibling checkout.
- Managed `serve-den` admits the exact configured consumer before listening and exposes one frozen
  host identity at `/health`, `/api/studio-status`, and the title bar. Manifest drift terminates the
  complete process group with `studioRestartRequired`; it never silently keeps a stale adapter.
- Generic `host` starts adapterless and uses one explicit `/api/studio-session/open` transaction to
  read a selected root's `.rusty-studio.json`, start its command, handshake the adapter, and open
  the project. The bootstrap is a bounded trusted development input; it does not transfer project
  schema or semantic authority to Studio. Status reports the active root, adapter, and protocol.
- Renderer packages are consumed through their package roots.
- Protocol 14 retains the promoted downstream Entity inspector seam.
  `studio/libs/editor-shell/src/entity-inspector.ts` owns static contribution
  admission, exact matching, remount generations, and the narrow
  mutation-settlement contract. The shell owns the single outlet and the stock
  Engine-hosted app explicitly composes only the built-in Voxel Object
  contribution. Ordinary downstream repositories provide project data and a
  Rust adapter; they do not compose Studio packages or add a game component to
  the shell.
- Protocol 11 added only the bounded `prepareVoxelObjectPlacement` presentation
  read needed to preview an unused canonical object. Its resource-only frame is
  merged by `viewport`; authoritative attachment remains downstream.
- Protocol 12 adds one create-only `attachVoxelObjectInstances` request for
  1–32 ordered placements. Downstream Rust stages owner allocation, complete
  admission, projection, and publication; Studio accepts one canonical readout
  and never implements a placement loop or parallel history authority.
- Protocol 13 adds `setSceneObjectRenderableTransform` plus a strict
  `renderableTransform` hierarchy readout. Studio presents entity/world and
  visual-local transforms separately; admitted mesh bounds, origin triad,
  contact plane, clearance, and lower-bound alignment remain disposable UI
  observations over the named Rust mutation.
- Protocol 14 adds strict Rust-authored voxel texture/atlas/material readouts,
  named upsert/removal requests, a keyboard-accessible Studio surfaces panel,
  and exact content-addressed PNG resolver wiring into the one viewport. The
  downstream adapter remains the catalog, persistence, and assignment owner.
- Protocol 15 adds a built-in Voxel Object Entity-inspector dropdown for the
  three Engine surface modes. The shell sends one named mutation and replaces
  its readout only after the downstream Rust adapter stages admission,
  projection, and atomic persistence; Studio never reaches into the renderer
  or treats the selection as local UI authority.
- `studio/libs/viewport` emits immutable `frameSubmitted` observations only
  after an accepted complete, incremental, or presentation-only frame has been
  submitted through its private shared inspection surface. The event pairs the
  Studio generation with the renderer-owned timing/resource sample; it exposes
  neither the surface nor Three/WebGL state and does not create another
  telemetry loop.
- `studio/libs/editor-shell` forwards that exact `frameSubmitted` event through
  a public shell output while retaining the generation-only `frameApplied`
  workspace acknowledgement.
- Tools > Animation Inspection operates on the selected authored animated-mesh
  handle through that same shared inspection surface. Human users can choose a
  canonical clip, scrub an exact normalized time, play or pause with an
  explicit cross-fade, and open or download a bounded five-frame labeled
  contact sheet. These controls mutate only disposable renderer playback; they
  never write authored project state.
- `RendererInspectionSurface.sampleAnimatedMesh` and
  `setAnimatedMeshPlayback` are the corresponding generic agent/tool seam.
  They remain channel-checked, handle-checked, renderer-owned, and lifecycle
  bounded; Studio exposes no Three scene, mixer, loader, or WebGL object.
- `studio/test/voxel-surface-comparison` is an explicit evidence harness over
  the public Studio viewport-submission helper and shared inspection surface.
  It is not a project setting or a second mesh owner; the caller supplies
  Rust-projected frames and content-addressed resources.

## Private or forbidden paths

- Do not import or mutate a sibling `rusty-engine-demo` checkout during ordinary
  Studio or provider work.
- Do not move adapter/game policy into Studio UI state.
- Do not make local browser storage or DOM state the source of project truth.
- Do not add Studio packages to the ordinary Cargo or root pnpm gate.

## Acceptance gates and fixtures

```bash
./scripts/verify-studio.sh
./scripts/verify-studio-generic-browser-integration.sh \
  /absolute/path/to/rusty-engine-demo \
  /absolute/path/to/rusty-engine-voxels
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels
```

The first command proves the isolated Studio workspace. The generic browser
command explicitly selects adjacent downstream checkouts, starts one
Engine-hosted Studio, and proves root-local adapter discovery and transactional
project switching. The demo and voxel commands are focused opt-in integration
proofs over the selected adjacent checkout: they build its project-owned Rust
adapter, run Engine's Studio host, and exercise the applicable protocol,
mutation, persistence, resource, and browser behavior.

These integration commands consume each sibling checkout exactly as it stands.
They do not fetch, pin, checkout, or mutate the Engine or downstream source
repository, and they are not automatic requirements for unrelated Engine
changes. Exact commits belong in Den task/review evidence. Ordinary Engine CI
uses `verify-studio.sh`; run the narrowest explicit downstream proof only when
the changed surface or review acceptance requires it.

Browser rejection proof uses the same bounded project-mutation readiness budget as successful
preparation, then requires a visible error diagnostic, a disabled apply action, and an unchanged
project hash. A short presentation timeout is not an authority or atomicity boundary.

## Common agent mistakes

- Treating a mocked adapter receipt as proof of a real downstream mutation.
- Treating an identity-only fixture as proof that a downstream panel can mutate and persist.
- Adding product policy to the shared adapter-client or viewport.
- Adding dynamic module discovery, a generic extension payload, or store access to make a third panel
  easier to wire.
- Reading sibling files directly instead of using the explicit adapter.
- Requiring Studio dependencies for unrelated Rust changes.

## Follow-up routing

- Durable scene/content mechanisms:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Shared renderer behavior:
  [Renderer workspace and hosts](renderer-workspace-and-hosts.md).
- Loading-bay product behavior belongs in the external `rusty-engine-demo`
  repository.
- Voxel experiment content, quality reports, and adapter policy belong in the external
  `rusty-engine-voxels` repository.
