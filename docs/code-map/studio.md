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
- [`studio/scripts/check-demo-consumer-revision.mjs`](../../studio/scripts/check-demo-consumer-revision.mjs)
- [`studio/test/entity-inspector-consumer-browser`](../../studio/test/entity-inspector-consumer-browser)
- [Studio migration contract](../studio-migration-contract.md)
- [Studio adapter protocol](../studio-adapter-protocol.md)
- [Downstream Entity inspector extensions](../studio-downstream-entity-inspector-extensions.md)
- [Downstream Engine revision contract](../topics/development/downstream-engine-revisions.md)
- [Voxel mesh data plane](../topics/voxel/voxel-mesh-data-plane.md)

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
- Renderer packages are consumed through their package roots.
- Protocol 13 retains the promoted downstream Entity inspector
  seam. `studio/libs/editor-shell/src/entity-inspector.ts` owns static
  contribution admission, exact matching, remount generations, and the narrow
  mutation-settlement contract. The shell owns the single outlet; the stock app
  explicitly composes the built-in Voxel Object contribution. The exact-pinned
  Loading Bay application composes its own Weapon contribution without adding
  its value or operations to Engine. Do not add another hard-coded game
  component to the shell.
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
- `studio/libs/viewport` emits immutable `frameSubmitted` observations only
  after an accepted complete, incremental, or presentation-only frame has been
  submitted through its private shared inspection surface. The event pairs the
  Studio generation with the renderer-owned timing/resource sample; it exposes
  neither the surface nor Three/WebGL state and does not create another
  telemetry loop.
- `studio/libs/editor-shell` forwards that exact `frameSubmitted` event through
  a public shell output while retaining the generation-only `frameApplied`
  workspace acknowledgement.

## Private or forbidden paths

- Do not import or mutate a sibling `rusty-engine-demo` checkout during ordinary
  Studio or provider work.
- Do not move adapter/game policy into Studio UI state.
- Do not make local browser storage or DOM state the source of project truth.
- Do not add Studio packages to the ordinary Cargo or root pnpm gate.

## Acceptance gates and fixtures

```bash
./scripts/verify-studio.sh
./scripts/verify-studio-package-consumer.sh <40-character-public-sha>
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels
```

The first command proves isolated Studio behavior. The package-consumer command proves the public
static-composition packages install from one exact Git revision without workspace or sibling paths.
The demo command admits only `studio/demo-consumer-source.json`, proves that the selected public
consumer's `engine-source.json` agrees with the reverse pin, and runs the consumer-owned revision
checker before any build. That checker owns the Cargo, renderer-package, Studio-package, build-policy,
and lock agreement. Its browser workflow also imports a project-local `.gltf` closure with external
buffer and texture resources, observes external-only drift, reapplies the import, and proves after
reload that authored `.gltf` provenance still names a content-addressed GLB runtime resource. The gate
then invokes the focused two-consumer proof in
`scripts/verify-studio-entity-inspector-integration.sh`. That proof serves the downstream-built
application in Chromium, covers Voxel Object, unknown read-only fallback, a real Weapon replacement
and canonical reread, then repeats the read in a fresh adapter process. The final command proves the
pinned animated-voxel runtime/quality workflow.

The local demo command defaults to both proofs and builds Studio plus the project-owned adapter once.
CI selects its `browser` and `entity-inspector` modes in parallel, then retains
`verify-studio-demo-integration` as the aggregate exact-revision gate. Documentation-only changes do
not start this long consumer proof. Provider changes become eligible only by deliberately advancing
`studio/demo-consumer-source.json`; directly triggering on an arbitrary Rust crate would merely rerun
the old consumer pin and could not certify the changed provider implementation.

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
