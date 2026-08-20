# Studio external-project adapter protocol

Status: protocol 15 is an Engine-owned, downstream-neutral boundary.

Studio talks to one selected project-owned Rust adapter through a bounded
JSON-lines process. The adapter owns its project layout, schema, admission,
persistence policy, and domain operations. It delegates reusable validation,
mutation planning, inspection, and retained projection to Rusty Engine owners.
Studio never reads project files or evaluates project semantics directly.

## Generic host discovery

`pnpm run host` starts adapterless. A trusted selected root may provide a
bounded `.rusty-studio.json` with an explicit adapter command and working
directory. The host validates that bootstrap, starts one candidate process,
performs `describe` and `openProject`, then atomically publishes the session.
Failed bootstrap, startup, handshake, identity, or admission leaves a prior
admitted session unchanged.

The bootstrap is a development launch description, not a registry, plugin
system, schema loader, or authority transfer. Downstream certification, when
needed, is selected and owned by that downstream product; it is not an ordinary
Studio or Engine gate.

## Protocol posture

Every request carries `protocolVersion: 15` and a bounded caller-selected
`requestId`. The closed request/response schemas in
[`studio/libs/adapter-client`](../studio/libs/adapter-client) are authoritative
for exact operation names and validation limits. Version 15 contains only these
tagged request families:

| Request | Purpose | Canonical authority |
| --- | --- | --- |
| `describe` | Identify an adapter, project kind, schema, and its closed operation set. | Project adapter |
| `openProject`, `readProject`, `closeProject` | Admit, reread, or release one explicit project and retained projection. | Project adapter plus Engine owners |
| `createProject`, `saveProjectAs` | Create or atomically publish a complete admitted project under exact path and hash guards. | Project adapter, `content-store`, admission owners |
| `createScene`, `renameScene`, `deleteScene`, `setEntryScene` | Manage the finite stored scene set through project policy. | Project adapter, `authored-scene`, `content-store` |
| `createSceneObject`, `deleteSceneObject`, `renameSceneObject`, `reparentSceneObject` | Mutate canonical hierarchy and lifecycle under project/scene guards. | `authored-scene`, `entity-state`, admission, `content-store` |
| `setSceneObjectTransform`, `setEntityTranslation`, `setSceneObjectRenderableTransform` | Apply authored or presentation-local transforms without transferring authority to Studio. | `authored-scene`, `entity-state`, render projection, admission |
| `setSceneObjectAppearance`, `setEntityCollision`, `setEntityKinematic` | Replace named entity appearance or components atomically. | `authored-scene`, `entity-state`, `asset-catalog`, admission |
| `upsertMaterial`, `upsertVoxelSurfaceMaterial`, `removeVoxelSurfaceMaterial` | Admit materials and bounded texture/atlas resources under exact project guards. | `asset-catalog`, `render-model`, render projection, adapter |
| `prepareAssetImport`, `prepareAssetReimport`, `applyAssetImport`, `discardAssetImport` | Create, retain, apply, or discard a private deterministic import plan. | `asset-import`, `asset-catalog`, adapter, `content-store` |
| `initializeVoxelAsset`, `duplicateVoxelAsset`, `replaceVoxelPalette` | Change canonical embedded voxel assets. | `voxel-asset`, `engine-spatial`, adapter |
| `attachVoxelInstance`, `setVoxelInstanceTransform`, `removeVoxelInstance` | Manage transformed scene instances without giving Studio scene authority. | Project schema, `authored-scene`, projection admission |
| `validateVoxelPick`, `applyVoxelBrush`, `applyVoxelPrimitive`, `initializeVoxelTemplate` | Revalidate untrusted picks and stage bounded atomic voxel edits. | `engine-spatial`, `voxel-asset` |
| `importVoxelAssetFile`, `exportVoxelAssetFile` | Read or publish canonical voxel assets through explicit trusted host paths. | `voxel-asset`, project host-file adapter |
| `materializeEnvironment` | Materialize one deterministic environment preset into managed content. | `environment-authoring`, project admission |
| `undoVoxelEdit`, `redoVoxelEdit`, `revertVoxelHistory` | Move durable history under exact asset/project guards. | `engine-spatial` history codec/service |
| `queryVoxelHistory`, `prepareVoxelHistoryRevert`, `applyVoxelHistoryRevert`, `discardVoxelHistoryRevert` | Inspect or retain one private non-mutating history candidate. | `engine-spatial` history codec/service |
| `createVoxelAnnotationLayer`, `editVoxelAnnotation`, `queryVoxelAnnotation`, `exportVoxelAnnotation` | Maintain bounded semantic annotations and owner readouts. | `voxel-annotation` |
| `queryVoxelModel`, `prepareVoxelConversion`, `applyVoxelConversion`, `discardVoxelConversion` | Query models or stage an exact private GLB conversion candidate. | `voxel-convert`, `voxel-asset`, adapter |
| `inspectVoxelObjectSource`, `prepareVoxelObjectConversion`, `previewVoxelObjectConversion`, `applyVoxelObjectConversion`, `discardVoxelObjectConversion` | Inspect a bounded object source or retain, preview, publish, or discard an exact candidate. | `voxel-convert`, `voxel-asset`, `voxel-object-runtime`, render projection, adapter |
| `prepareVoxelObjectPlacement` | Resolve one admitted object's resource-only placement preview; it creates no entity, instance, project bytes, or gameplay state. | Project object admission, render projection, resource host |
| `attachVoxelObjectInstance`, `attachVoxelObjectInstances` | Create one or 1–32 ordered object instances in a fail-atomic project mutation. | Project schema, owner allocation, complete admission, persistence, object projection |
| `setVoxelObjectInstanceSurfaceMode`, `previewVoxelObjectInstance` | Persist one derived surface choice or sample disposable playback through explicit caller time. | Project schema, `voxel-object-runtime`, projection, atomic persistence |

Responses are a closed tagged union: `described`, `projectOpened`,
`projectRead`, `entityTranslationApplied`, `projectMutationApplied`,
`voxelPickValidated`, `voxelRead`, `voxelConversionPrepared`,
`voxelConversionDiscarded`, `voxelObjectSourceInspected`,
`voxelObjectConversionPrepared`, `voxelObjectConversionPreviewed`,
`voxelObjectConversionDiscarded`, `voxelObjectPlacementPrepared`,
`voxelObjectInstancePreviewed`, `voxelHistoryRevertPrepared`,
`voxelHistoryRevertDiscarded`, `voxelAssetFileExported`,
`assetImportPrepared`, `assetImportDiscarded`, `projectClosed`, or `rejected`.
There is no generic method string, command registry, arbitrary payload,
provider lookup, runtime session, or cross-capability gameplay envelope.

The adapter remains the canonical project and persistence authority. TypeScript
only presents typed readouts and submits named requests; it never acquires a
project store, generic extension payload, dynamic module loader, renderer
internals, or gameplay policy.

## Safety, private plans, and identity

The host bounds request and response bytes. Selected roots are absolute;
project paths are safe and relative. Adapters reject symlinks in existing path
chains, escapes, non-files, oversized sources, malformed protocol input, and
unsupported versions. Host replacement uses an exact prior SHA-256 and a
same-directory candidate with a final target recheck.

Every durable mutation compares its exact project hash and relevant scene or
asset revision, stages the named owner operation on a candidate, reruns complete
project admission, builds canonical readouts and projection, atomically
publishes through `content-store`, then rereads canonical bytes. Rejected,
invalid, stale, and malformed operations leave original project bytes unchanged.

Prepared imports, conversions, object conversions, and history reverts are
private adapter-process candidates with exact source, settings, project, asset,
plan, and output identities. Their visible fields are informative; apply accepts
only the retained candidate under its current optimistic guards. A replacement
prepare evicts the older candidate. Disposable voxel-object playback is likewise
private state and clears on open, reread, close, and every durable mutation.

The adapter returns complete renderer-neutral frames rebuilt from admitted Rust
state. Studio may select a clip/frame and schedule another named sample, but it
does not mesh voxels, advance animation, manufacture hashes, perform raycasts,
or restore browser-owned renderer state. The built-in Voxel Object inspector is
an explicit static contribution with exact identity matching; it does not create
a plugin registry, dynamic import, component payload, or service locator.

Managed host identity exposes a source/build identity, adapter executable
SHA-256, and negotiated protocol only as operational evidence. It is neither
project authority nor a dependency pin. The generic root-local path does not
require a revision command, network update, or sibling-checkout mutation.

## Verification

Run `./scripts/verify-studio.sh` for the isolated Studio workspace. The
optional `./scripts/verify-studio-generic-browser-integration.sh` accepts one
explicit supporting consumer checkout for browser-host discovery evidence.
Neither check launches or certifies a retired Demo product.
