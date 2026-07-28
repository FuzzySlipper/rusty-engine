# Studio external-project adapter protocol

Status: protocol 9 Engine surface implemented; downstream adapters adopt versions deliberately

Rusty Engine Studio talks to one project-owned Rust adapter at a time through a bounded JSON-lines
process. The adapter is a downstream composition root: it understands that project's layout,
content schema, compatibility policy, and named domain operations, while it delegates reusable
admission, mutation, persistence planning, inspection, and renderer projection to Rusty Engine
owners.

The first implementation is the `rusty-engine-demo` Loading Bay adapter. It proves the boundary
against a real external checkout without turning that checkout into an ordinary Engine dependency.

## Closed protocol

Every request carries `protocolVersion: 9` and a caller-selected `requestId`. Version 9 contains
only these tagged request families:

| Request | Purpose | Canonical authority |
| --- | --- | --- |
| `describe` | Identify adapter, project kind, schema, and the closed operation set. | Project adapter |
| `openProject` | Open an explicit absolute root and safe relative project file; return canonical readouts and initial projection. | Project adapter plus Engine owners |
| `createProject`, `saveProjectAs` | Create a complete admitted project or atomically publish a renamed copy under explicit path and hash guards. | Project adapter, `content-store`, Engine admission owners |
| `readProject` | Reread the open source and produce current readouts and one complete replaceable projection. | Project adapter plus Engine owners |
| `createScene`, `renameScene`, `deleteScene`, `setEntryScene` | Manage the finite stored scene set and active entry scene through project-owned policy. | project adapter, `authored-scene`, `content-store` |
| `createSceneObject`, `deleteSceneObject`, `renameSceneObject`, `reparentSceneObject` | Mutate canonical hierarchy and lifecycle with expected project and scene identity. | `authored-scene`, `entity-state`, downstream admission, `content-store` |
| `setSceneObjectTransform`, `setEntityTranslation` | Apply a full or legacy translation-only authored transform with expected project hash and scene revision. | `authored-scene`, downstream admission, `content-store` |
| `setSceneObjectAppearance` | Replace empty, static-mesh, or typed light appearance and rerun resource/projection admission. | `authored-scene`, `asset-catalog`, render projection, downstream admission |
| `setEntityCollision`, `setEntityKinematic` | Attach, replace, or remove named entity components atomically. | `entity-state`, downstream spatial admission, `content-store` |
| `upsertMaterial` | Create or replace one stored material definition. | `asset-catalog`, downstream admission, `content-store` |
| `prepareAssetImport`, `prepareAssetReimport`, `applyAssetImport`, `discardAssetImport` | Read bounded project/host mesh sources into a private deterministic plan, expose diagnostics/dependencies/generated locks, then install the exact candidate atomically or discard it. | `asset-import`, `asset-catalog`, project adapter, `content-store` |
| `initializeVoxelAsset`, `duplicateVoxelAsset`, `replaceVoxelPalette` | Create or change canonical project-embedded voxel assets under exact asset guards. | `voxel-asset`, `engine-spatial`, project adapter |
| `attachVoxelInstance`, `setVoxelInstanceTransform`, `removeVoxelInstance` | Manage transformed scene instances without giving Studio scene authority. | downstream scene schema plus `authored-scene`/projection admission |
| `validateVoxelPick` | Re-cast an untrusted shared-renderer ray against the named transformed instance and compare the claimed cell/face. | `engine-spatial` picking and collision authority |
| `applyVoxelBrush` | Expand one bounded cube brush into a validated atomic edit transaction. | `engine-spatial` edit/history plus `voxel-asset` |
| `applyVoxelPrimitive`, `initializeVoxelTemplate` | Generate bounded block/box/shell/edge/line edits or one deterministic house asset without moving semantic generation into TypeScript. | `engine-spatial` primitive/template services plus `voxel-asset` |
| `importVoxelAssetFile`, `exportVoxelAssetFile` | Open or publish a canonical voxel asset through explicit trusted host paths and exact replacement identity. | `voxel-asset`, downstream host-file adapter |
| `materializeEnvironment` | Materialize one deterministic preset/seed into a managed asset, scene instance, and named project markers. | `environment-authoring`, downstream scene admission |
| `undoVoxelEdit`, `redoVoxelEdit`, `revertVoxelHistory` | Move durable committed history under project and asset hash guards. | `engine-spatial` history codec/service |
| `queryVoxelHistory`, `prepareVoxelHistoryRevert`, `applyVoxelHistoryRevert`, `discardVoxelHistoryRevert` | Return bounded entries/diffs/samples and retain a private non-mutating revert candidate until explicit apply or discard. | `engine-spatial` history codec/service |
| `createVoxelAnnotationLayer`, `editVoxelAnnotation` | Create or transactionally edit typed semantic regions. | `voxel-annotation` plus target voxel identity |
| `queryVoxelAnnotation`, `exportVoxelAnnotation`, `queryVoxelModel` | Return bounded owner readouts without sending canonical meaning to TypeScript. | `voxel-annotation`, `voxel-convert` query owners |
| `prepareVoxelConversion`, `applyVoxelConversion`, `discardVoxelConversion` | Prepare a private bounded project/host GLB plan with primitive, affine, default-material, and typed texture policy; atomically install its exact output or discard it. | `voxel-convert`, `voxel-asset`, project adapter |
| `inspectVoxelObjectSource` | Import a bounded static or animated GLB snapshot and expose Rust-derived hierarchy, groups, materials, UV sets, clips, channel targets, and classified diagnostics. | `voxel-convert`, project adapter |
| `prepareVoxelObjectConversion`, `previewVoxelObjectConversion`, `applyVoxelObjectConversion`, `discardVoxelObjectConversion` | Retain one exact static-object or animated-flipbook candidate, select a stored frame for a complete shared-renderer projection, atomically install it, or explicitly discard it. | `voxel-convert`, `voxel-asset`, `voxel-object-runtime`, render projection, project adapter |
| `attachVoxelObjectInstance` | Attach a transformed canonical object with one explicit default or clip-frame posture and material overrides. | downstream scene schema plus object admission/render projection |
| `previewVoxelObjectInstance` | Scrub, play, pause, sample, or stop one applied instance through explicit caller time while returning its saved pose, disposable playback posture, and a complete renderer-neutral projection. | `voxel-object-runtime`, render projection, downstream project adapter |
| `closeProject` | Release open-project and retained-projection state. | Project adapter host lifecycle |

Responses are likewise a closed tagged union: `described`, `projectOpened`, `projectRead`,
`entityTranslationApplied`, `projectMutationApplied`, `voxelPickValidated`, `voxelRead`,
`voxelConversionPrepared`, `voxelConversionDiscarded`, `voxelObjectSourceInspected`,
`voxelObjectConversionPrepared`, `voxelObjectConversionPreviewed`,
`voxelObjectConversionDiscarded`, `voxelObjectInstancePreviewed`, `voxelHistoryRevertPrepared`,
`voxelHistoryRevertDiscarded`, `voxelAssetFileExported`, `assetImportPrepared`,
`assetImportDiscarded`, `projectClosed`, or `rejected`. There is no
generic method string, command registry, arbitrary payload, provider lookup, RuntimeSession, or
cross-capability gameplay envelope.

The TypeScript owner is [`../studio/libs/adapter-client`](../studio/libs/adapter-client). It performs
strict structural decoding, request correlation, and named client methods. It deliberately does not
parse the canonical owner JSON strings or reproduce project, scene, entity, voxel, persistence, or
game semantics. Shared render frames are decoded by `@rusty-engine/render-contracts`.
The isolated Studio workspace includes the same-repository renderer packages explicitly. Its
`viewport` library mounts `renderer-host`, which composes render-projection and renderer-three;
Studio does not import Three, retain a private scene graph, translate materials/resources, own a
raycaster, or duplicate renderer disposal.

## Loading Bay owner composition

Opening `content/projects/loading-bay.project.json` exercises the shipped Engine capabilities:

- `content-store` admits the bounded project source and identity-bearing manifest;
- `asset-catalog` owns the derived catalog and validation;
- `authored-scene` owns the canonical entry-scene view, edit service, and admission plan;
- `entity-state` owns admitted generic entity invariants and the durable snapshot;
- `engine-inspector` owns catalog, scene, entity, persistence, and voxel readouts;
- Loading Bay owns its project schema and complete game-specific semantic admission; and
- `render-projection` and `render-model` own the renderer-neutral retained frame.

The adapter returns the canonical project, catalog, scene, entity-state, and content-manifest codec
results alongside inspection DTOs, Loading Bay's explicitly named domain summary, voxel inspection,
an authored-scene hierarchy readout, and the shared render frame. Hierarchy order, node identity,
parentage, kind, and local/world transforms are produced in Rust. Every response carries a complete
frame, including resource definitions, so Studio can atomically replace the shared renderer channel.
These are readouts rebuilt from admitted Rust state on every read, not a second content model.

Protocol 9 also admits one optional, independently versioned `meshResources` readout for adapters
that opt into content-addressed retained mesh sources. The frame carries only renderer-neutral
identity, hash, length, encoding, and stream offsets; the readout maps those identities to
downstream-owned project-relative paths. Studio resolves the bounded bytes through its existing
resource host before applying the frame. Inline adapters remain valid, and the authoritative voxel
object remains unchanged. The format and migration contract are documented in
[the voxel mesh data-plane decision](topics/voxel/voxel-mesh-data-plane.md).

The Converted Wall artifact additionally composes canonical `voxel-asset` payloads, catalog material
definitions, transformed scene instances, `engine-spatial` collision/edit/history state,
`voxel-annotation` layers, bounded `voxel-convert` model/conversion readouts, and voxel chunk
projection. The shared frame tags voxel assets and instances for renderer hint routing; Rust still
revalidates the ray, transformed instance, local cell, and face before an edit can use the result.

Protocol 7 added a required `voxelObjectAuthoring` readout beside the unchanged voxel-volume
readout. It exposes canonical object grid/pivot, frame identities and timing, clips, palette and
source-material bindings, provenance, and transformed scene instances after every open, mutation,
and reread. It is an inspection DTO over project-owned Rust content, not a TypeScript object format.

Prepared object previews return a complete renderer-neutral frame composed by the downstream Rust
adapter from the canonical project plus one private candidate instance. The frame contains the
actual `defineVoxelObject` resource and object-instance operations produced through the shared
runtime/projection path. Angular may select a clip/frame and run a disposable play timer, but every
scrub or timer tick names a Rust-stored frame and receives another complete owner-produced frame.
It never meshes sample voxels, deforms animation, computes timing, or manufactures hashes.

Protocol 8 keeps applied-instance playback separate from candidate inspection. Studio sends a
closed playback command and an explicit monotonic microsecond timestamp. The downstream adapter
retains one disposable `VoxelObjectPlayer`, while Rust resolves admitted clip durations, playback
posture, runtime frame identity, and the complete shared-renderer projection. The response reports
the durable default/clip-frame selection beside the transient sampled frame and exact project/object
hashes. Scrub, play, pause, and timer samples do not publish the player posture, revise the project,
or rewrite the voxel-object artifact; `stop` presents the durable pose again. TypeScript schedules
only the next sampling request and never advances frame indices or interprets clip durations. A
pause or stop chosen during an in-flight sample is queued as the next closed command; the latest
user control wins, so stop may supersede a queued pause without racing adapter requests. That queue
is scoped to the current project and object-operation generations and is discarded by every
canonical project lifecycle transition or accepted replacement.

Protocol 9 makes the owning entity explicit for every applied voxel-object instance readout. The
owner identity is supplied by the downstream Rust project schema, is repeated in hierarchy,
entity-state inspection, and renderer metadata, and lets Studio locate the typed Voxel Object
capability without matching labels or assets. Applied playback therefore lives in the selected
Entity inspector; the conversion panel remains responsible for source inspection, candidates,
canonical asset publication, and instance attachment. This is one explicit built-in capability,
not a universal component-description or arbitrary command protocol.

The proposed successor for other downstream components is documented in
[`studio-downstream-entity-inspector-extensions.md`](studio-downstream-entity-inspector-extensions.md).
It is not part of protocol 9. The proposal keeps the core protocol limited to bounded owner,
component, and inspector-contract identity; downstream values and mutations remain in separately
closed product-owned contracts composed statically by the downstream Studio host.

## Safety and atomicity

The process bounds request and response bytes. The selected root must be absolute and the project
path must be safe and relative. Explicit host selections must be absolute and lexically normalized.
The downstream adapter rejects symlinks throughout existing path chains, path escapes, non-files,
oversized sources, malformed protocol input, and unsupported versions. Host replacement requires
the exact prior SHA-256 and uses a synced same-directory candidate with a final target recheck.

Every durable mutation is staged before publication:

1. compare exact source hash and derived scene revision;
2. invoke the one named scene/material/voxel/annotation/history/conversion owner on a candidate;
3. rerun complete Loading Bay admission;
4. build and authorize the `content-store` write candidate;
5. build canonical readouts and renderer projection;
6. atomically replace the file through the existing project store; and
7. reread canonical bytes and confirm publication.

Rejected, invalid, stale, and malformed operations leave the original project bytes unchanged.
Prepared volume conversions, voxel-object conversions, and history reverts are private
adapter-process values containing exact source/settings/project/asset identity. Visible fields are
informative; apply succeeds only for the retained candidate and current optimistic guards. Object
apply additionally pins the exact candidate output hash. Stale project/source/plan/output identity
and an oversized renderer projection fail before publication. Object discard returns a newly
composed canonical complete frame rather than asking Studio to restore browser-owned scene state.
Applied voxel-object playback is private adapter-process state as well. Open, reread, close, or any
durable project mutation clears it, so a reopened project begins from canonical bytes and may start
a fresh transient preview without reconstructing hidden browser state.
The adapter retains at most one prepared candidate of each kind; a successful replacement prepare
evicts the older candidate, whose identity then rejects without mutation. Voxel history is encoded beside the
embedded asset and reconstructed by a fresh process before query, undo, redo, or revert.

Asset-import plans use the same pattern. The visible plan contains settings, source hash, generated
artifact readouts, diagnostics, and a classification, but apply accepts only the retained private
candidate with its exact plan and project hashes. Reimport replaces only the previously generated
asset IDs, rejects unrelated collisions, reruns complete project admission, and canonically rereads
the result. Source drift is observational until a named reimport is prepared and applied.

Host-user settings are intentionally not part of this Rust protocol because they are browser/webview
host preferences, not gameplay or project semantics. The isolated Node host exposes one bounded
GET/PUT endpoint backed by the shared versioned `studio-user-settings` artifact. Files are keyed by
canonical project root outside project content, protected by SHA-256 compare-and-swap, symlink and
size checks, same-directory atomic replacement, and future-version preservation. Renderer-host,
not the Angular shell, implements the resulting camera movement, boost, pan, orbit, and input cleanup.

## Gates

- `./scripts/verify-studio.sh` checks and tests the TypeScript boundary without any demo checkout.
- The demo's Rust gate tests protocol decoding, owner delegation, path safety, bounds, downstream
  semantic rejection, optimistic replacement, atomicity, and canonical reread.
- `./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo` is the explicit
  cross-repository proof. It builds the project-owned adapter, opens Loading Bay, then mutates a
  temporary Converted Wall copy through brush/primitive/history-preview/template/host-file/
  annotation/model-query/conversion/environment operations.
  It closes and starts a fresh adapter process to verify reconstruction and byte-preserving stale
  rejection. Real Chromium workflows then cover canonical hierarchy selection, observable
  shared-renderer selection/full-transform preview/cancel, project/scene/entity/light/component
  authoring, general asset import/dependency/lock/source-drift/reimport, restart-stable host-user
  camera/input preferences, transformed voxel picking,
  shared-renderer brush/conversion preview restoration, brush undo/redo, annotations, private-plan
  conversion, reload persistence, and stale non-mutation.
- `.github/workflows/studio-demo-integration.yml` checks out the public demo at the exact revision
  declared by `studio/demo-consumer-source.json` and runs that proof as an explicit integration
  gate. The pin makes downstream drift a conscious update instead of an ambient sibling checkout.
- `./scripts/verify-studio-voxel-integration.sh /absolute/path/to/rusty-engine-voxels` separately
  accepts only the exact public revision declared by `studio/voxel-consumer-source.json`. It checks
  the consumer's exact Engine pin and runtime/quality reports, then drives saved-pose, named-clip,
  repeat, pause/resume, once, restore, and reopen behavior through current Studio and the shared
  renderer in Chromium. `.github/workflows/studio-voxel-integration.yml` reproduces the same clean
  checkout; ordinary Studio and provider gates do not inspect a sibling voxel repository.

Ordinary `./scripts/verify.sh` remains Rust/shell-only and does not inspect, build, or require a
sibling demo checkout.
