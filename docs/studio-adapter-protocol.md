# Studio external-project adapter protocol

Status: M11D scene workflow implemented

Rusty Engine Studio talks to one project-owned Rust adapter at a time through a bounded JSON-lines
process. The adapter is a downstream composition root: it understands that project's layout,
content schema, compatibility policy, and named domain operations, while it delegates reusable
admission, mutation, persistence planning, inspection, and renderer projection to Rusty Engine
owners.

The first implementation is the `rusty-engine-demo` Loading Bay adapter. It proves the boundary
against a real external checkout without turning that checkout into an ordinary Engine dependency.

## Closed protocol

Every request carries `protocolVersion: 2` and a caller-selected `requestId`. Version 2 contains
only these tagged request families:

| Request | Purpose | Canonical authority |
| --- | --- | --- |
| `describe` | Identify adapter, project kind, schema, and the closed operation set. | Project adapter |
| `openProject` | Open an explicit absolute root and safe relative project file; return canonical readouts and initial projection. | Project adapter plus Engine owners |
| `readProject` | Reread the open source and produce current readouts and one complete replaceable projection. | Project adapter plus Engine owners |
| `setEntityTranslation` | Apply one typed authored transform with expected project hash and scene revision. | `authored-scene`, downstream admission, `content-store` |
| `closeProject` | Release open-project and retained-projection state. | Project adapter host lifecycle |

Responses are likewise a closed tagged union: `described`, `projectOpened`, `projectRead`,
`entityTranslationApplied`, `projectClosed`, or `rejected`. There is no generic method string,
command registry, arbitrary payload, provider lookup, RuntimeSession, or cross-capability gameplay
envelope.

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

## Safety and atomicity

The process bounds request and response bytes. The selected root must be absolute and the project
path must be safe and relative. The downstream adapter rejects symlinks throughout the writable
path, path escapes, non-files, oversized sources, malformed protocol input, and unsupported
versions.

Transform mutation is staged before publication:

1. compare exact source hash and derived scene revision;
2. apply `SceneEditService::SetTransform` to a candidate;
3. rerun complete Loading Bay admission;
4. build and authorize the `content-store` write candidate;
5. build canonical readouts and renderer projection;
6. atomically replace the file through the existing project store; and
7. reread canonical bytes and confirm publication.

Rejected, invalid, stale, and malformed operations leave the original project bytes unchanged.

## Gates

- `./scripts/verify-studio.sh` checks and tests the TypeScript boundary without any demo checkout.
- The demo's Rust gate tests protocol decoding, owner delegation, path safety, bounds, downstream
  semantic rejection, optimistic replacement, atomicity, and canonical reread.
- `./scripts/verify-studio-demo-integration.sh /absolute/path/to/rusty-engine-demo` is the explicit
  cross-repository proof. It builds the project-owned adapter, opens the real Loading Bay source,
  validates voxel and owner readouts, and then runs a real Chromium workflow against a temporary
  project copy. The browser proof covers canonical hierarchy selection, shared-renderer picking,
  translation preview/commit, refresh/reopen persistence, and invalid-operation non-mutation.
- `.github/workflows/studio-demo-integration.yml` checks out the public demo at the exact revision
  declared by `studio/demo-consumer-source.json` and runs that proof as an explicit integration
  gate. The pin makes downstream drift a conscious update instead of an ambient sibling checkout.

Ordinary `./scripts/verify.sh` remains Rust/shell-only and does not inspect, build, or require a
sibling demo checkout.
