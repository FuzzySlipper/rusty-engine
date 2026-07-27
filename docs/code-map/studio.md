# Studio

## Purpose

Route the separately isolated first-party authoring product, its renderer
viewport, and its closed external-project adapter protocol.

## Owns

- Angular/Nx application composition and editor shell.
- Adapter client protocol, viewport integration, voxel editor, user settings,
  and Studio-owned host services.
- Studio migration accounting and explicit owner-adoption evidence.
- Browser and explicit external-demo integration gates.

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
- [Studio migration contract](../studio-migration-contract.md)
- [Studio adapter protocol](../studio-adapter-protocol.md)
- [Proposed downstream Entity inspector extensions](../studio-downstream-entity-inspector-extensions.md)

## Public downstream surfaces

- The closed adapter protocol defines operations a downstream product may
  implement without granting Studio direct access to engine or game internals.
- The external consumer is always selected explicitly by an integration
  command; ordinary Engine work never scans a sibling checkout.
- Renderer packages are consumed through their package roots.
- The proposed downstream Entity inspector seam is static host composition plus
  identity-only core metadata. It is not implemented yet; use its ordered Den
  tasks rather than adding another hard-coded game component to the shell.

## Private or forbidden paths

- Do not import or mutate a sibling `rusty-engine-demo` checkout during ordinary
  Studio or provider work.
- Do not move adapter/game policy into Studio UI state.
- Do not make local browser storage or DOM state the source of project truth.
- Do not add Studio packages to the ordinary Cargo or root pnpm gate.

## Acceptance gates and fixtures

```bash
./scripts/verify-studio.sh
./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo
```

The first command proves isolated Studio behavior. The second explicitly proves
the current cross-repository adapter and browser workflow.

## Common agent mistakes

- Treating a mocked adapter receipt as proof of a real downstream mutation.
- Adding product policy to the shared adapter-client or viewport.
- Reading sibling files directly instead of using the explicit adapter.
- Requiring Studio dependencies for unrelated Rust changes.

## Follow-up routing

- Durable scene/content mechanisms:
  [Content, assets, and scenes](content-assets-and-scenes.md).
- Shared renderer behavior:
  [Renderer workspace and hosts](renderer-workspace-and-hosts.md).
- Loading-bay product behavior belongs in the external `rusty-engine-demo`
  repository.
