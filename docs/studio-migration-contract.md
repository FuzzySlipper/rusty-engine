# Studio migration contract

Status: M11F successor parity implemented; fresh exact-SHA closeout acceptance remains

Rusty Engine Studio is a first-party authoring product in this repository. It is not a narrow
vertical slice and it is not a compatibility shell for Asha. The migration target is full useful
feature parity with the pinned Asha Studio donor: every substantive tracked workflow is preserved,
adapted, consolidated, or explicitly rejected because it exists only for removed Asha topology.
Later M11 slices may change terminology and module boundaries, but they may not silently drop a
workflow recorded here.

## Pinned donor

| Field | Value |
| --- | --- |
| Repository | `git@github.com:FuzzySlipper/asha-studio.git` |
| Public source | `https://github.com/FuzzySlipper/asha-studio` |
| Commit | `709e1be780796ca1b802df764f0ec064bd271bc4` |
| Tree | `beb5e34e97ef73c9bda7a8d12e7e28a97175a6cd` |
| Commit time | `2026-07-22T05:02:28-07:00` |
| Tracked files | 147 |
| Raw `git ls-tree -r` SHA-256 | `5211cde5134894ed7e2a47d9b7d91d34a194f36669d6e58d86d45cd623e6da44` |
| Tracked worktree state | Clean and equal to `origin/main` at audit time |
| Excluded local-only paths | untracked `assets/` and `untitled.scene.json` |
| License status | No tracked `LICENSE`, `COPYING`, or `NOTICE`; GitHub reports no SPDX license |

The exact committed tree is frozen in [`../studio/donor-inventory.tsv`](../studio/donor-inventory.tsv).
[`../studio/donor-surface-disposition.tsv`](../studio/donor-surface-disposition.tsv) assigns every
path to exactly one substantial surface and one of these decisions:

- `preserve`: useful product material can move substantially unchanged;
- `adapt`: preserve the behavior while replacing Asha ownership or integration;
- `consolidate`: retain the behavior inside a coarser successor module;
- `historical-only`: retain the file as donor evidence, while current docs/tests replace it; or
- `exclude`: the surface exists only for obsolete topology or development ceremony.

The donor has no declared license. This repository records the exact owner-controlled provenance
and makes no new third-party licensing claim. Untracked files are not donor inputs.

## Parity baseline

These are user-visible or behaviorally important Studio capabilities, not promises to preserve the
donor's internal package names. The named M11 slice is the first required product proof; M11F must
reconcile every row before the old Studio can be considered retired.

| Workflow family | Required successor behavior | Primary slice |
| --- | --- | --- |
| Product shell | Angular application shell, task-oriented File/Edit/Scene/View/Project/Runtime/Voxel/Preferences menus, hierarchy, viewport, inspector, bottom workspace, popouts, status, theme, and accessible focus behavior | M11C |
| Scene files | New, Open, Save, Save As, dirty protection, explicit discard, stale conflict reload/overwrite/cancel, canonical reread, and failure non-mutation | M11B/M11D |
| Trusted host files | Arbitrary explicit host paths, directory navigation, file filtering, bounded reads, safe staging, compare-and-swap promotion, rollback, and focus restoration | M11B/M11C |
| External projects | Open and create external projects, startup deep links, content-root discovery, project switching, atomic A-to-B open, and project-owned compatibility diagnostics | M11B/M11C |
| Project content | Typed content browser, manifest closure, scene/asset navigation, prefab references, write authorization, source conflicts, and canonical whole-project saves | M11B/M11D |
| Settings | Versioned project spatial settings, host-user settings outside browser storage, per-project identity, scene-view colors/grid, configurable movement keys, speed/boost, and look/pan inversion | M11C/M11D |
| Hierarchy | Deterministic parent traversal, filtering with ancestor context, per-node expansion, visibility, selection, and refresh-stable UI expansion | M11C/M11D |
| Inspector | Rust-described typed fields, references, diagnostics, entity definitions/capabilities, transforms, appearance, material and presentation values, and explicit unsupported-field states | M11B/M11D |
| Scene authoring | Create/update/delete scene nodes, canonical parent/child order, entity instances, lights, appearance bindings, local/world transform composition, and optimistic mutation | M11B/M11D |
| Viewport | Shared authored/runtime/overlay channels, shared resource realization, ordinary lights/materials, renderer-owned grid, resize/lifecycle, and deterministic cleanup | M11D |
| Camera | Orbit, pan, zoom, frame selection, camera-relative WASD, world-relative QE, focus/key cleanup, speed boost, stored presentation settings, and disposable preview state | M11C/M11D |
| Picking and selection | Shared renderer hints routed to authored or runtime selection, followed by Rust/project revalidation before meaningful operations | M11D/M11E |
| Transform tools | Translate/rotate/scale gizmos, world/local orientation, parented objects, anisotropic grid snapping, fine/snap modifiers, per-frame preview, one explicit settlement, cancel, and stale restoration | M11D |
| Asset catalog | Browse canonical assets and locks, dependency/navigation readouts, import/reimport metadata, material preview, eligible target options, and owner diagnostics | M11B/M11D |
| Entity appearance | Author admitted static/animated appearance and clips, project resolved resources, visible projection, and explicit missing/unsupported binding diagnostics | M11D |
| Environment authoring | Choose deterministic provider/preset/seed, create or replace generated environment artifacts and markers, preserve provenance, and reject ambiguous managed targets | M11E |
| Voxel files and instances | New/open/save/save-as canonical voxel assets, initialize blank authoring volumes, attach multiple transformed scene instances, preserve bindings on reopen, and show unresolved assets honestly | M11E |
| Direct voxel tools | Object/Edit modes, local-space cell overlay, paint/erase, bounded cube brush, deterministic command ordering, transient stroke preview, cancellation, and atomic multi-edit commit | M11E |
| Voxel picking | Renderer hint plus authoritative transformed-instance ray re-cast, typed place/remove anchors, mismatch rejection, and no mutation on invalid picks | M11E |
| Voxel history | Bounded committed history, diffs/samples, cursor and redo-tail behavior, preview/apply revert, undo, redo, quotas, stale rejection, and durable reconstruction | M11E |
| Voxel annotations | Layers/regions, semantic kinds, bounds/cell/region queries, parentage, provenance, every typed edit family, target identity, validation, and canonical export | M11E |
| Voxel materials | Stored palettes, catalog bindings, compact edit material selection, conversion material maps, texture-sampling readouts, material counts, optimistic replacement, and reopen preservation | M11E |
| Mesh import and conversion | Bounded GLB import, groups/material slots, source metadata, fit/origin/affine settings, plan, preview, apply/install, surface and closed-solid modes, texture sampling, provenance, model/window readouts, and failure non-mutation | M11E |
| Projection delivery | Incremental retained frames, coalescing, bounded recovery state, expensive-path metrics, shared renderer contracts, and no Studio-owned scene graph or resource cache | M11D/M11E |
| Live inspection | Read-only project/runtime identity, entity/scene/voxel/render diagnostics, attachment and disconnect states, and refresh through explicit project-owned adapter operations rather than RuntimeSession | M11B/M11D |
| Domain panels | Generated-level metadata, encounter tuning, playable-loop inspection, and project-specific actions remain available for the reference demo through its typed adapter; they are not generalized into an Engine behavior AST | M11B/M11C |
| Typed actions | Accepted/rejected operation results and a visible action timeline remain; the universal command registry and arbitrary command envelope do not | M11B/M11C |
| Product acceptance | Real visible controls prove scene, entity, lighting, voxel, material, conversion, save/reopen, and external-demo behavior; proof catalogs and hidden browser mutation hooks do not return | M11D/M11E/M11F |

## M11E foundation and M11F parity reconciliation

M11E established protocol 3 against the Converted Wall demo artifact. It includes canonical
voxel inspection/initialization/duplication, catalog material upsert and palette replacement,
multiple transformed instances, shared-renderer projection and transformed picking, bounded cube
paint/erase with disposable preview, durable undo/redo/cursor revert, annotation layer creation,
label edit, query/export, bounded model windows, and private-plan GLB conversion/apply. Rust owns
hashes, semantic validation, history, plan identity, project admission, and atomic publication.

Protocol 4 implements the previously explicit M11F voxel work:

- canonical voxel host-file open/export/save-as with explicit absolute paths, bounded reads,
  symlink rejection, exact target SHA replacement, and atomic promotion;
- deterministic house-template creation and bounded block, filled/shell/edge box, and line edits;
- bounded history entry/diff/sample inspection plus private preview/apply/discard revert candidates;
- disposable brush and conversion sample presentations through the shared renderer, with canonical
  frame restoration on cancel/discard and no Studio-owned Three scene;
- controls for every typed annotation edit family and cell/bounds/region/summary query mode;
- project or host GLB/license selection, primitive-group selection, complete affine input, explicit
  default material, and closed texture sample/binding policy; and
- deterministic preset/seed environment materialization into a managed voxel asset, instance, and
  named downstream player/exit entities.

The actual protocol, editor controls, owner tests, fresh-process integration, and exact demo pin are
the authority for this completion; no donor TypeScript semantic generator was copied.

Protocol 5 and 6 close the non-voxel parity set without adding a generic editor command:

- project create and save-as plus scene create, rename, delete, and entry-scene selection;
- entity create, rename, reparent, delete, full translation/rotation/scale settlement, static-mesh,
  animated-mesh-with-clip, or typed light appearance, and collision/kinematic capability mutation;
- a complete catalog browser with dependency, dependent, generated-lock, import provenance, source
  drift, and private prepare/apply/discard import or reimport candidates;
- real imported static-mesh payloads and materials consumed by the same Rust projection and shared
  renderer used by Demo and Studio; and
- versioned per-canonical-project host-user preferences, outside browser storage and project bytes,
  for theme, snapping/grid presentation, movement keys, six-axis speed/boost, and look/pan inversion.

The settings host derives identity from the canonical project root, preserves malformed or future
artifacts instead of overwriting them, rejects symlink targets and stale hashes, and publishes one
bounded same-directory candidate atomically. The donor's separate committed spatial-settings file
is consolidated: current successor projects fix right-handed Y-up/meters, while the adjustable grid
spacing, colors, visibility, and snapping behavior remain per-project host-user presentation. A
future project-owned spatial convention requires a named Rust adapter operation rather than a
browser-owned project codec.

The review closure retains the remaining interaction-level behavior rather than treating the
protocol rows alone as parity. A trusted host browser provides bounded directory navigation,
extension filters, symbolic-link exclusion, and dialog focus restoration. Separate
translate/rotate/scale gizmos provide world/local operation, parent-aware conversion, anisotropic
snapping, fine and snap-toggle modifiers, disposable preview, one settlement, and cancellation.
Animated GLB resources and named clips are Rust-admitted, hash-verified by the trusted host, and
realized by the same shared renderer used for static meshes; Studio owns only selection controls and
never creates a private Three scene.

## Cohesive successor modules

The donor's small Nx libraries are evidence about change reasons, not a package-count target. The
successor begins with these ownership areas and splits them only when their dependencies and change
reasons diverge:

```text
studio/
  apps/studio/             Angular product shell and composition
  libs/adapter-client/     one closed protocol decoder/client; structural checks only
  libs/user-settings/      versioned host-user preference artifact and bounded HTTP client
  libs/application/        project-open lifecycle, owner readouts, selections, dirty/conflict UI
  libs/editor-shell/       menus, regions, dialogs, settings UI, focus, theme
  libs/scene-editor/       hierarchy, inspector view models, transform interaction
  libs/voxel-editor/       voxel/material/annotation/conversion UI and transient previews
  libs/viewport/           composition over shared Rusty renderer packages
  scripts/                 isolated checks and supported launch/host operations
  test/                    behavior regressions and browser product acceptance
```

`adapter-client` is the sole cross-language DTO owner in TypeScript. Feature libraries may derive
view models, but they do not duplicate codecs, semantic validation, hashes, CAS logic, committed
history, picking authority, projection, renderer resources, or lifecycle.

## External-project adapter

Studio selects a trusted external project root. A project-owned Rust adapter understands only that
project's layout, domain schema, compatibility policy, and available game-specific operations. It
composes Rusty Engine owners for reusable behavior and returns their canonical readouts and shared
render frames.

The boundary is a closed, versioned request/response protocol. Its request families are deliberately
named and finite:

- describe adapter and project compatibility;
- open/refresh an explicit project root;
- read canonical project, scene, entity, asset, voxel, diagnostic, and projection views;
- propose one typed owner operation with an optimistic revision/hash; and
- close the project or adapter connection.

There is no `methodName + json`, provider registry, universal command payload, callback
subscription, RuntimeSession facade, or Studio-selected native module. The trusted host chooses and
starts the adapter. The protocol may carry owner-defined versioned values; TypeScript may reject a
malformed closed message but cannot reinterpret its semantics.

Host file access is explicit and bounded. Paths are resolved by the adapter/host, checked against
the selected operation's policy, protected against escapes and relevant symlink surprises, and
published through `content-store` write sets or the narrower owning persistence API. Browser fetch,
downloads, local storage, and HTTP routes are not canonical persistence.

## Engine owner adoption

[`../studio/owner-adoption.tsv`](../studio/owner-adoption.tsv) classifies every current Rust workspace
crate, every shared renderer package, and the external project adapter as `direct`, `indirect`,
`downstream-only`, or `non-studio`. It names the workflow, boundary, Studio-only state, and first
proof slice. This is a decision map, not a second Asha behavior ledger.

Runtime-only mechanics such as physics, triggers, navigation, generic state machines, time, and RNG
do not acquire editor authority merely to demonstrate package use. A project adapter may expose
their typed readouts where the donor already had a useful inspection workflow. Studio never
reimplements them.

## Deliberate exclusions

- Asha `RuntimeSession`, runtime/native/browser bridge topology, global providers, and lifecycle
  compatibility negotiation;
- universal command registry/dispatch, arbitrary method names, broad operation envelopes, and a
  universal gameplay or authoring AST;
- `ProjectBundle`, game-workspace publication/control-plane fields, and `asha.game.toml` meanings
  that do not exist in the selected external project;
- generated contract mirrors and global code-generation ceremony;
- replay, certification, proof hashes, evidence catalogs, committed proof dumps, source-token
  delivery checks, and hidden browser globals;
- operational sibling-checkout links or hard-coded `/home/dev/asha-*` paths;
- donor caches, build output, artifacts, `.den-serve.json`, untracked assets, and the untracked
  untitled scene; and
- tiny library/generator topology that adds navigation overhead without a distinct owner.

These exclusions remove structure, not working editor behavior. A useful donor panel or interaction
must be expressed through the new owners even when its old transport is excluded.

## Verification and CI domains

| Domain | Local entry point | Trigger/ownership rule |
| --- | --- | --- |
| Engine | `./scripts/verify.sh` | Rust and shell only. It may audit isolation but never installs or executes Studio, Node, Angular, Nx, or Playwright. Studio-only paths do not schedule this job. |
| Renderer | `./scripts/verify-render.sh` | Shared renderer packages, fixtures, and Rust render owners. Studio consumes the result and does not fork it. |
| Studio | `./scripts/verify-studio.sh` or `pnpm run verify:studio` | Installs only from `studio/pnpm-lock.yaml`; owns migration/boundary checks now and later lint, typecheck, unit, build, and browser checks. It runs for Studio and deliberately named public owner seams. |
| All local domains | `./scripts/verify-all.sh` or `pnpm run verify:all` | Explicit opt-in aggregate; never the ordinary Engine default. |
| Engine-Studio product integration | M11B onward | Manual/scheduled/release and selected public-contract changes; opens an explicit external demo checkout and proves real authoring-to-product behavior. |

M11A establishes the isolated workspace and static gates only. M11B adds the real adapter before the
Angular shell is imported. M11C through M11E establish the retained product workflows. M11F adds the
remaining voxel and non-voxel workflows and closes only after exact-SHA review reconciles this
contract, the donor inventory, every owner-adoption row, and the actual product evidence.
