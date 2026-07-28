# Typed downstream Entity inspector extensions

Status: **accepted architecture; protocol 10 identity envelope and static Engine outlet implemented**

Decision task: Den `rusty-engine#6300`

This decision defines the smallest boundary by which a downstream Rust project can put one of its
own entity components in Rusty Engine Studio's Entity inspector. Protocol 10 now implements the
bounded identity envelope and removes the fixed Loading Bay summary. The inspector outlet, host
mutation lease, and built-in Voxel Object migration are implemented by Engine tasks `#6302` and
`#6303`. The Loading Bay panel and final two-consumer reconciliation remain ordered follow-ups until
their owning tasks land and pass review.

## Decision

Studio will support downstream Entity inspector panels through **static host composition**, not a
runtime plugin system and not a universal component-description format.

The next deliberate Studio adapter revision will carry only bounded component identity and owner
metadata in its core project readout. A downstream UI package will be compiled into a downstream
Studio application explicitly and will bind to a separate, closed, product-owned request/response
contract. The core protocol will never carry the downstream component value, field schema, mutation
payload, operation name, UI package path, or executable code.

In one sentence:

> The core protocol tells the host which entity owns which stable component and which typed
> inspector contract can interpret it; the statically composed downstream package supplies that
> contract, while downstream Rust remains the only semantic and mutation authority.

The existing Voxel Object panel is the first component-shaped inspector surface. It will move behind
the same static outlet as a built-in contribution. The first independent consumer that earns the
shared seam will be a Loading Bay Weapon authoring panel owned by `rusty-engine-demo`, after the
demo's gameplay-mechanics migration in Den task `rusty-engine-demo#6290` establishes its current
Rust component and project schema.

## Why the current shape needs a boundary

Protocol 9 and the pre-extension shell proved that entity-owned component controls are much easier
to find than controls buried in a domain workflow. They also expose two kinds of central coupling:

- `StudioProjectReadout` contained a fixed `loadingBay` domain summary even though Loading Bay is an
  external product. Protocol 10 removed it.
- The Entity inspector template names Voxel Object, Renderer Appearance, Collision, and Kinematic
  directly. Adding a downstream Weapon, Door, Encounter, or other component would require another
  Engine template and protocol edit.

Expanding those unions for every game would make Engine the vocabulary owner. Replacing them with a
generic field/value tree would merely hide the same ownership error behind an AST. Loading arbitrary
UI modules named by an adapter would instead create a plugin lifecycle, trust, versioning, and
deployment system before there is evidence that one is needed.

The useful repeated concept is narrower: a selected entity has a stable component identity, and a
known host package may have a typed panel for that exact identity and contract version.

## Boundary at a glance

```text
downstream Rust project adapter
  core Studio protocol
    -> project identity / hierarchy / projection
    -> bounded entity-component references only

  downstream closed authoring protocol
    -> named read operation
    -> named mutation operation
    -> typed readout / candidate / receipt
    -> canonical project hash after mutation
             |
             v
downstream Studio application composition root
  -> Engine Studio shell + built-in contributions
  -> explicitly imported downstream contribution
       -> downstream Angular panel
       -> downstream typed client and decoder
             |
             v
Engine Entity inspector outlet
  -> matches exact owner/component/contract identity
  -> provides selection and mutation-settlement context
  -> never sees the downstream component value or command
```

There remains one project adapter process and one project authority. A product may implement a
closed superset of the core adapter's wire tags in that process, but every non-core tag belongs to a
named downstream protocol and decoder. Engine does not add an `extension`, `invoke`, `method`, or
`payload` request that can tunnel arbitrary messages.

## What belongs in the core protocol

The core protocol owns discoverability and attribution only. The proposed shapes are illustrative
TypeScript names; implementation may refine naming while preserving their exact information
boundary.

```ts
interface StudioEntityInspectorContractIdentity {
  readonly contractId: string;
  readonly contractVersion: number;
}

interface StudioEntityComponentReference {
  readonly ownerEntityId: number;
  readonly componentTypeId: string;
  readonly inspectorContract: StudioEntityInspectorContractIdentity | null;
}

interface AdapterDescription {
  // Existing closed fields remain.
  readonly entityInspectorContracts: readonly StudioEntityInspectorContractIdentity[];
}

interface StudioProjectReadout {
  // Existing core readouts remain, subject to the deliberate Loading Bay extraction below.
  readonly entityComponents: readonly StudioEntityComponentReference[];
}
```

The protocol 10 core decoder validates and bounds this envelope. It does not interpret either
identity string.
The Rust project adapter must additionally prove that:

- every `ownerEntityId` exists in the canonical entity inspection and hierarchy vocabulary;
- each `(ownerEntityId, componentTypeId)` pair is unique;
- every non-null inspector contract appears in the adapter description with the same positive
  version;
- identities use the established bounded stable-ID syntax rather than Rust type names, labels, or
  asset names; and
- reference and per-entity counts remain below explicit protocol limits.

The core reference deliberately excludes:

- component fields or serialized values;
- a generic schema for numbers, references, conditions, effects, lists, or nested objects;
- display labels, icons, panel ordering, and other host presentation;
- operation names, writable flags, permissions, or arbitrary endpoint addresses;
- a component revision, candidate, or mutation receipt; and
- a JavaScript module name, URL, package tarball, callback, or executable handle.

Exact read and write guards stay in the downstream typed readout and request. Some components have
an `entity-state::ComponentRevision`; other project-owned capabilities, including the current Voxel
Object association, have a different exact identity. Pretending those guards are one universal core
revision would weaken rather than strengthen the boundary.

An unknown component or unsupported contract version remains visible as an identity-only row in the
Entity inspector. Studio does not silently hide it, guess fields, or permit editing. Unknown editor
support therefore degrades read-only without making project opening or generic hierarchy work
impossible.

### Loading Bay extraction

Protocol 10 removed the fixed `loadingBay` field from the core `StudioProjectReadout`. Its summary
and future domain readouts belong to a Loading Bay-owned closed contract and client. The migration
is a deliberate version cut: adapters adopt the new core version and downstream contract together,
with no long-lived compatibility alias in Engine.

Voxel Object authoring remains a core protocol family because conversion, canonical object assets,
runtime admission, playback, and projection are reusable Engine mechanisms. Moving its panel behind
the static inspector outlet changes presentation composition, not semantic ownership.

## Downstream typed authoring contracts

Each downstream component family owns a small closed protocol beside its Rust semantic owner. For
the selected second consumer, the rough request vocabulary is:

```ts
type LoadingBayWeaponAuthoringRequest =
  | ReadLoadingBayWeaponRequest
  | ReplaceLoadingBayWeaponRequest;

interface ReadLoadingBayWeaponRequest {
  readonly type: 'readLoadingBayWeapon';
  readonly contractVersion: 1;
  readonly requestId: string;
  readonly expectedProjectHash: string;
  readonly ownerEntityId: number;
}

interface ReplaceLoadingBayWeaponRequest {
  readonly type: 'replaceLoadingBayWeapon';
  readonly contractVersion: 1;
  readonly requestId: string;
  readonly expectedProjectHash: string;
  readonly expectedComponentRevision: number;
  readonly ownerEntityId: number;
  readonly candidate: LoadingBayWeaponDraft;
}
```

`LoadingBayWeaponDraft`, its readout, its exact revision, and its receipt are closed types in the
downstream package. The implementation task must derive their fields from the post-`#6290` Rust
component and project schema rather than inventing a parallel editor model in this proposal.

The operation path is explicit:

1. The panel calls `readLoadingBayWeapon` through its concrete downstream client.
2. Downstream Rust resolves the exact owner and component and returns a bounded typed readout.
3. The panel edits a disposable local form.
4. The panel calls `replaceLoadingBayWeapon` with the current project hash, exact component
   revision, owner, and complete typed candidate.
5. A named downstream Rust authoring service validates the candidate and all game-specific
   references, stages the project mutation, reruns complete project admission, and atomically
   publishes it.
6. The typed receipt returns the before/after project hashes and the new component revision.
7. The host rereads the canonical core project and discards any late response from an older
   project, selection, or contract generation.

There is no generic `readComponent`, `setComponent`, JSON Patch, reflection-based Rust field walk,
or browser-side semantic validation. A new Loading Bay component adds a new downstream named
contract or deliberately extends an existing bounded downstream union. It does not add an Engine
operation.

The same long-lived Rust executable may accept both the core closed protocol and the product's
closed authoring protocol. Sharing a process and physical byte transport does not merge their type
ownership. Core clients decode only core responses; the downstream client decodes only its own
response union. The product host must route the fixed protocol set explicitly and retain the same
request/response byte, process-lifecycle, and failure bounds as the current adapter host.

## Static Studio host composition

The Engine Studio shell exposes one host-level outlet and a small contribution contract from its
existing `editor-shell` public package. A new package is not justified solely to hold these few
types.

The contribution is supplied directly to `StudioShellComponent` by its application root as an
immutable list. It is not discovered through an Angular multi-provider, global registry, filesystem
scan, adapter response, URL, or package manager at runtime.

```ts
interface StudioEntityInspectorContribution {
  readonly componentTypeId: string;
  readonly contract: StudioEntityInspectorContractIdentity;
  readonly title: string;
  readonly order: number;
  readonly panel: Type<StudioEntityInspectorPanel>;
}
```

The implemented panel interface exposes only common host context:

- the selected `ownerEntityId` and stable `componentTypeId`;
- current project identity/hash and a project/selection generation;
- read-only busy and connection posture;
- a bounded host mutation lease used only to serialize UI operations; and
- mutation settlement that accepts the downstream receipt's before/after project hashes, invokes
  canonical `readProject`, verifies the resulting hash, and releases the lease.

The panel does not receive `StudioWorkspaceStore`, raw transport, the complete canonical JSON
strings, renderer handles, or a generic callback that executes a semantic command. Its concrete
client is imported and constructed by the downstream application. A mutation lease carries no
component payload and cannot choose a Rust service; it only prevents a downstream edit from racing a
core Studio edit and makes the canonical reread visible to the shared shell.

The stock Engine Studio application composes built-in contributions explicitly. A downstream
product that wants game-specific panels owns a small Studio application composition root which
imports the Engine shell, the pinned Engine built-ins, and its own panel package:

```ts
const contributions = [
  ...rustyEngineInspectorContributions,
  loadingBayWeaponInspectorContribution,
] as const;
```

Engine never imports that downstream package. The dependency remains one-way.

The public composition packages use the same exact-Git subdirectory shape as the shared renderer:
`github:FuzzySlipper/rusty-engine#<sha>&path:studio/libs/<package>`. A product pins
`adapter-client`, `editor-shell`, `user-settings`, `viewport`, and `voxel-editor` plus the four
renderer packages to one reviewed public revision. Each package prepares and exposes only its
`dist` entry points; its installed closure contains versioned peers rather than Engine workspace,
link, or sibling-checkout paths. Angular-bearing packages publish partial-compiled Angular metadata
in those entry points. `verify-studio-package-consumer.sh` proves that closure from a clean
temporary consumer by building an external Angular application that imports the shell, viewport,
Voxel editor, and playback components before downstream adoption.

`StudioWorkspaceStore.entityInspectorMutationPort` admits at most one active lease. Acquisition
requires the exact selected owner/component/contract, accepted project hash, adapter identity, and
project/selection/contract generations. Core mutations and selection/remount requests remain
serialized while it is active, including the interval after a downstream request has started but
before its receipt is available. Settlement accepts only the downstream receipt's before/after
project hashes, performs the closed core `readProject`, and publishes that reread only when its hash
matches. Project replacement or a newer accepted contract generation makes the settlement stale;
late success or failure cannot replace the newer project or operation state. Panels receive the
narrow port, never the store or a generic command callback.

## Admission, isolation, and versioning

### Build-time admission

The downstream application is the admission point. Its explicit contribution list must reject
duplicate `(componentTypeId, contractId, contractVersion)` keys before bootstrap. The package lock
and exact Engine pin determine the installed code; adapters cannot request new code at runtime.

### Connection-time compatibility

After `describe`, the shell compares adapter-advertised contracts with the statically installed
contributions. A panel mounts only for an exact contract ID/version match and a matching component
reference. Unsupported versions show the identity-only fallback. No semver range negotiation or
best-effort decoder is needed at the wire boundary; a new wire shape receives a new positive
contract version.

### Package isolation

A downstream panel package may depend on Angular, the public Engine `editor-shell` contribution
types, its own protocol/client package, and deliberately selected presentation helpers. It must not
import Engine app internals, `StudioWorkspaceStore`, renderer backend internals, Node host scripts,
or another downstream extension's private state.

The downstream Rust protocol and authoring service remain usable in focused headless tests without
Angular, Node, Chromium, or the Engine Studio application.

### Bounds and failures

The implementation must establish finite limits for advertised contracts, component references,
components per entity, identity lengths, and downstream request/readout collections. A duplicate,
orphan owner, contract mismatch, stale project hash, stale component revision, malformed readout,
oversized response, rejected candidate, or failed canonical reread leaves durable project bytes
unchanged and the inspector visibly rejected or stale.

Late reads and receipts are scoped to the project, selection, and contract generation that issued
them. Project open, close, reread, or replacement invalidates those generations. An extension panel
cannot retain an authoritative parallel component model across those transitions.

## Validation ownership

Evidence follows the owner:

1. **Core protocol:** strict identity-envelope decoding, bounds, uniqueness, orphan-owner rejection,
   contract-version mismatch, and unknown-component read-only fallback.
2. **Engine host outlet:** static contribution admission, exact matching, deterministic ordering,
   selection/remount behavior, mutation-lease serialization, canonical reread, and stale-response
   disposal in headless Angular tests.
3. **Built-in Voxel Object contribution:** existing inspector controls and playback behavior through
   the new outlet, with no semantic or protocol regression.
4. **Downstream Rust contract:** focused Loading Bay read/replace tests, semantic rejection,
   optimistic conflict handling, atomic persistence, and fresh-process reconstruction.
5. **Downstream panel:** component tests using the real downstream decoder/client contract.
6. **Product integration:** exact-pinned served Chromium proof that selects a real weapon-owning
   entity, reads its Rust-owned configuration, commits one change, observes the canonical reread,
   and preserves the change after a fresh adapter process.

Ordinary Engine Rust verification remains free of Node, Angular, browser, renderer, and sibling
checkout dependencies. Studio's isolated gate proves the generic host seam and built-in panel. The
explicit pinned cross-repository gate proves Loading Bay adoption.

## Concrete second consumer: Loading Bay Weapon authoring

Loading Bay Weapon authoring is the promotion consumer, not a placeholder `ExampleComponent`.

It is a strong test because:

- Loading Bay already reports weapon presence, and its FPS campaign defines weapons as real
  Rust-owned inventory items while keeping weapon vocabulary, attack modes, ammunition policy,
  pickups, and product persistence downstream;
- Den task `rusty-engine-demo#6290` is explicitly migrating its state to reviewed typed
  gameplay-mechanics components and named services while keeping weapon policy downstream;
- a weapon is entity-owned configuration that does not belong in Renderer Appearance;
- its editor needs downstream definition/reference validation and one real atomic project mutation,
  so an identity-only mock panel cannot falsely prove the seam; and
- neither Engine nor the Voxel Object protocol should learn the game's firing or ammunition
  semantics.

The first panel is intentionally narrow. It will expose the exact durable weapon identity,
definition/configuration, and initial inventory/equipment bindings already present after `#6290`,
then support one complete replace operation through a named downstream Rust authoring service. The
contract-freezing task must remove any field from that sentence which is not actually durable
project authoring data.

The panel will not edit live ammunition, current firing cooldown, attack resolution, enemy health,
fixed-tick state, or runtime save/checkpoint state. Runtime inspection is a separate product need and
does not enter this project-authoring seam accidentally.

Promotion succeeds only if Voxel Object and Loading Bay Weapon use the same identity matching,
static outlet, compatibility admission, operation serialization, and canonical-reread mechanics
while retaining completely different typed readouts and Rust operations.

## Rejected alternatives

### Keep extending Engine unions and templates

Rejected because every downstream component would require editing the core protocol, client, store,
and shell. Engine would become the game vocabulary owner and downstream work would remain hard to
locate or extract.

### Generic component value or field schema

Rejected because `fields`, `type`, `value`, validation rules, reference pickers, collection editing,
and generic mutation rapidly become a universal component/authoring AST. It also invites TypeScript
to reinterpret Rust semantics and encourages an arbitrary `setComponent` command.

Repeated neutral field widgets may be extracted later as ordinary presentation helpers after two
real panels use the same interaction. They do not become a wire schema.

### Adapter-supplied runtime plugins

Rejected because module URLs, package discovery, code loading, trust policy, dependency resolution,
hot reload, lifecycle, and compatibility negotiation are a plugin platform. The trusted-LAN posture
does not make that complexity useful. Static application composition is simpler and auditable.

### A generic extension request envelope

Rejected even if its payload is called typed. `extensionId + operation + JSON` is still an arbitrary
command tunnel to the core client and makes structural decoding meaningless. Each downstream
protocol keeps named tags and a closed decoder owned by the product.

### Give panels the workspace store or a service locator

Rejected because panels could reach unrelated mutation paths and would become coupled to shell
internals. The common host port is limited to selection context, operation serialization, canonical
reread settlement, and error presentation.

### Put downstream UI packages in Engine

Rejected because it reverses the provider dependency and makes ordinary Engine work depend on game
policy. Only the built-in Voxel Object contribution remains in this repository.

## Implementation sequence

Implementation is deliberately ordered so the generic seam follows the real second contract:

1. **Freeze the downstream contract after GM6.** Audit the actual Loading Bay weapon component and
   durable project schema, implement or specify its two named Rust operations and closed readout,
   and provide fixtures without changing Engine.
2. **Revise the core identity envelope.** Protocol 10 adds bounded adapter contract advertisement
   and entity component references, migrates the fixed Loading Bay summary out of the core readout,
   and preserves unknown components as identity-only rows. Implemented by `rusty-engine#6302`.
3. **Add static host composition.** Introduce the explicit contribution input/outlet and mutation
   settlement port, then move Voxel Object behind it without behavior loss. Implemented by
   `rusty-engine#6303`.
4. **Adopt from Loading Bay.** Build the downstream application composition root and Weapon panel,
   pin the reviewed Engine revision, and prove one real mutation through canonical reopen.
5. **Close the promotion.** Run the isolated and exact-pinned integration gates, reconcile this
   proposal with implemented names and limits, and update canonical design/protocol documents.

The Den scheduling chain is:

| Order | Task | Owner | Depends on |
| --- | --- | --- | --- |
| 1 | `rusty-engine-demo#6301` — Freeze the Loading Bay Weapon authoring contract for Studio | Demo | `rusty-engine-demo#6290`, this decision (`rusty-engine#6300`) |
| 2 | `rusty-engine#6302` — Add identity-only downstream Entity component references to the Studio protocol | Engine | `rusty-engine-demo#6301` |
| 3 | `rusty-engine#6303` — Add static Entity inspector composition and migrate Voxel Object behind it | Engine Studio | `rusty-engine#6302` |
| 4 | `rusty-engine-demo#6304` — Compose the Loading Bay Weapon panel into Studio | Demo product/host | `rusty-engine-demo#6301`, `rusty-engine#6303` |
| 5 | `rusty-engine#6305` — Close the two-consumer Entity inspector extension boundary | Engine integration/docs | `rusty-engine-demo#6304` |

These are top-level follow-ups rather than subtasks because `#6300` is a design deliverable and may
close after review without implying that the implementation campaign has shipped. Cross-project
dependencies preserve the one-way code dependency while making the evidence order explicit.

## Scope accounting

Task `rusty-engine#6300` is complete when this proposal is reviewable and its ordered Den tasks
exist. It intentionally implements no protocol, panel outlet, downstream operation, or browser UI;
the parent task explicitly calls for design and decomposition rather than a speculative framework.

The follow-ups are implementation phases, not hidden acceptance gaps in `#6300`. None may claim the
extension seam is implemented until the real Loading Bay mutation and fresh-process product proof
pass. If the post-GM6 weapon schema does not contain suitable durable authoring data, the first
follow-up must return to this decision instead of fabricating a parallel component merely to satisfy
the planned example.
