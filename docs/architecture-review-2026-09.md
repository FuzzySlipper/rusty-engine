# Adversarial architecture and performance review — 2026-09

Scope: the entire repository at `61f9f6eb`, reviewed against the owner's
current model, not against `AGENTS.md`, `docs/`, or comments:

- The C# product is trusted first-party code. It owns game logic.
- Rust is glue plus reusable mechanisms.
- TypeScript realizes presentation and DOM UI.
- There is no untrusted-caller model, no replay authority, and no
  upstream-over-downstream trust hierarchy.

Existing documents were read only to find where old assumptions are still
encoded. They were not treated as requirements.

Method: seven parallel read-only lanes covered the per-frame and per-load
paths:

1. ABI
2. runtime/session
3. host/transport
4. Rust presentation
5. TypeScript renderer
6. spatial/services
7. assets/tooling

The load-bearing claims were then re-checked in source.

- **V** = verified directly in source during synthesis.
- **R** = reported by a lane's code reading and not independently re-traced.

Nothing was benchmarked. Cost figures are structural (what is copied, scanned,
or rebuilt, and how often), not measurements.

---

## 1. Summary

The earlier cleanup removed most *scattered* validation. What remains is more
expensive because it is *architectural*: a handful of mechanisms that
every feature routes through, built for a world that had to distrust and
roll back its caller, replay work exactly once, and reconstruct a remote
observer from any point. Five patterns account for nearly every finding.

| # | Pattern | Where it bites | Frequency |
|---|---|---|---|
| P1 | **Per-callback transaction**: stage a candidate copy of Engine state, commit or discard it, then taint the whole incarnation anyway. | Every C# callback, all staged services. | Every frame |
| P2 | **Stateless recomputation from canonical inputs**: full-world snapshots in, derived worlds rebuilt per call. | Graphics `PublishSnapshot`, Rapier per step, `EntityState` per query/character step, voxel scene per edit. | Every frame / edit |
| P3 | **Reconnect/replay-grade transport**: 3 processes and 3 JSON encodes, a history ring, fragments, cursors, acks, and baselines triggered by input fences. | Every output and input event. | Every frame |
| P4 | **Optimistic concurrency against a single trusted caller**: `expected_revision`, receipts, guards, sequence contiguity, generations. | Spatial, voxel, dynamics, entity-state, C# `EntityStore`/Inventory, TS publication tracker. | Per call |
| P5 | **Re-validation at every layer plus fail-whole-operation policy caps**. | Mesh data scanned about 5× in Rust; presentation frames deep-decoded in TS; physics steps fail on caps. | Per publish / step |

These are not independent. P1 exists because a failed callback was once
assumed recoverable. P2 exists because the Engine was once the authority that
recomputed truth from product-submitted facts. P3 exists to guarantee a remote
observer could be resynchronised deterministically. P4 exists because the
caller was not trusted to be current. With those premises gone, each mechanism
costs O(world) somewhere and forces new features through extra layers.

Removing P1 and P2 is expected to be the largest runtime win. Removing P3 is
the largest latency and complexity win. None of these requires changing the
C# → Rust → TypeScript split.

### Findings that are bugs, not just cost

These deserve fixing regardless of any refactor:

1. **A handled Engine error still kills the session. V**
   - Any graphics, audio, camera, UI or implicit-surface operation failure sets
     `callback_error` (`csharp-engine-services/src/appearance.rs:8550` and
     siblings).
   - `take_staged_call` then fails (`appearance.rs:2079`), `take_call` returns an
     error (`composition.rs:652`), and `taint_after_callback` runs
     (`csharp-product-runtime/src/lib.rs:2045`).
   - A product that catches `EngineCallException` and continues is still torn
     down and respawned, losing all game state.
   - Spatial and Dynamics errors do not do this, so the behaviour is also
     inconsistent.
2. **Pointer deltas are clamped to 32 px per DOM event. V**
   - `runtime-pack-shell/main.js:55` → `input-ingress.ts:319`.
   - Fast mouse flicks lose rotation.
3. **An oversized error message can panic while the session lock is held. V
   (mechanism) / plausible (trigger)**
   - `ProductDevRuntimeError::new` rejects diagnostics over 1,024 bytes
     (`product-dev-host/src/error.rs:115`).
   - Callers `.expect("fixed bounded NativeAOT error")`
     (`csharp-product-runtime/src/lib.rs:2371`, also near 2454 and 611).
   - C# error text may be up to 64 KiB (`ProductGenerator.cs:664-666`).
   - The lane also reports that `ProductDevLogEvent::new(..).expect` rejects
     `\n`, which C# stack traces always contain.
4. **The audio duplicate-signal set grows without bound. V**
   - `render-presentation/src/audio.rs:236,425`: one entry per one-shot is
     inserted and never pruned.
   - The lane reports that this set is also cloned into every call candidate,
     so per-frame cost grows with session length.
   - The same unbounded dedupe pattern exists in TS: `particle-host.ts:148` and
     `audio-host.ts:204` (**R**).
5. **Dynamics errors are discarded. R**
   - `Err(_) => 0` in `csharp-engine-services/src/dynamics.rs:1961` and
     elsewhere.
   - C# receives a zero status with no diagnostic.
6. **Some caps fail a whole physics step. R**
   - `MAX_DYNAMICS_CONTACTS = 4096` → `TooManyContacts`.
   - A non-CCD body moving more than 1 m/step fails the step.
   - A body beyond `MAX_ABS_TRANSLATION` breaks every later step
     (`svc-collision/src/dynamics.rs:417,620-660`).
7. **One revision check can never fail. R**
   - Static-mesh collider replacement passes the current revision back as its
     own `expected_revision` (`csharp-engine-services/src/spatial.rs:870-880`).
   - This check can never fail: pure ceremony.
8. **Voxel or static-mesh edits reset ground support for every character. R,
   intent uncertain**
   - `collision_world_hash` includes voxel and static-mesh identity.

---

## 2. Priority table

Impact is expected effect on frame time, latency, load/iteration time, or
development friction. Effort is rough.

| Rank | Change | Pattern | Impact | Effort |
|---|---|---|---|---|
| 1 | Remove per-callback staging; mutate committed state; C# exception → log + pause, not taint | P1 | Very high (per-frame O(world) copies incl. mesh bytes) | Medium–large |
| 2 | Retained-handle delta graphics API (`SetTransforms(span)` etc.); resources defined once, `Arc` bodies | P2 | Very high for any product with generated meshes | Medium |
| 3 | Persistent Rapier world per `DynamicsWorld`; drop `EntityState` round-trip | P2/P4 | High (physics per frame) | Medium |
| 4 | Incremental voxel edits and residency: `reconcile` dirty chunk colliders, `Arc` mesh chunks, version counters instead of hashes | P2 | High (edits/streaming currently O(world)) | Medium |
| 5 | Worker owns a WebSocket; binary mesh/resource frames; baseline on connect; delete history/fragments/cursors/acks | P3 | High (latency, CPU, about 40% of dev-host code) | Large |
| 6 | Character/kinematic/raycast queries take typed slices or a retained collider set; no throwaway `EntityState` | P2 | High for AI/NPC-heavy products | Small–medium |
| 7 | TS: remove GPU duty governor default; stop double world render with primary `CameraView`; per-frame matrix/instance work only on change | — | High (visible FPS) | Small |
| 8 | Dev loop: source-root content in dev, change-type routing, `WriteOnlyWhenDifferent`, lazy content hashing, cacheable resources | — | High (iteration time) | Small–medium |
| 9 | Input fences stop triggering full output rebaselines; drop gap-free sequencing and per-batch receipts | P3/P4 | Medium (menu transitions, pause) | Small |
| 10 | Remove revision/guard ceremony from C# SDK helpers (`EntityStore`, adapters, Inventory) | P4 | Medium (O(N²) patterns) | Small–medium |
| 11 | ABI: caller-provided output spans instead of leases plus `Read*At` loops; declarative shim macro | P4/P5 | Medium (FFI chatter, boilerplate) | Medium |
| 12 | Delete legacy surfaces (§8) and rewrite the stale architecture text | — | Medium (agent/human confusion) | Small |

---

## 3. P1 — the per-callback transaction

### What happens every frame

Each C# callback runs through the same sequence:

1. `EngineServiceSet::begin_update_call`
   (`csharp-engine-services/src/composition.rs:462-576`) starts a candidate in
   every staged service.
2. **R** Several services are cloned unconditionally, whether or not the
   callback touches them:
   - audio (`audio.rs:275`)
   - video
   - camera
   - render output
   - UI streams, including their latest JSON `Value` trees (`ui.rs:86-92`)
   - voxel content (`voxel_content.rs:744`)
   - voxel scene presentation, a per-chunk map (`voxel_scene_presentation.rs:119`)
   - implicit fields (`implicit_surfaces.rs:162`)
3. Appearance state is `Arc<RuntimeAppearanceData>` with `DerefMut =
   Arc::make_mut` (`appearance.rs:1538-1555`, **R**). The committed side always
   holds a second reference, so the first write in a call deep-copies the whole
   struct. That struct holds about 45–50 BTreeMaps and the projector, including
   the resource catalog. Product-generated meshes live there as inline
   `Vec<f32>`/`Vec<u32>` (`render-model/src/mesh.rs:113-122`, **V**). The
   projector's `last_resources` holds a second copy.
4. Every snapshot publish is a write, so the full copy happens on essentially
   every frame with motion.
5. `take_call` (`composition.rs:650-782`):
   - clones `PresentationWorld`; the first `apply` then `make_mut`s its retained
     maps;
   - rebuilds the full effects baseline whenever appearance changed;
   - rebuilds the audio and video baselines unconditionally (**R**).
6. Commit, then a `complete_call` FFI acknowledgement back to C#.
7. On the C# side, `LeaseReleaseCoordinator` keeps commit and rollback closures
   so that disposal can be "revived" on discard (`ProductGenerator.cs:76-110`).

### What it buys

Nothing, by the runtime's own statement. `taint_after_callback`
(`csharp-product-runtime/src/lib.rs:2143-2148`, **V**) says discarding staged
output "cannot undo immediate Engine mutations or restore managed product
state", and latches the incarnation for replacement.

Spatial, dynamics, voxel, persistence and RNG are deliberately immediate. The
mixed model is what made `recover_voxel_presentation_outputs`
(`composition.rs:901`) and `pending_recovery_outputs` necessary. The only
consumer that rolls back and continues is timeline completion
(`lib.rs:3300-3325`, **R**).

The checked performance probe measures an idle begin/discard
(`appearance.rs:~11143`). That is why this cost has stayed invisible: the
probe never reaches the deep-copy path.

### Recommendation

- Services mutate committed state in place and append outputs to a per-call
  `Vec`.
- Delete the following:
  - `discard_call`
  - staged copies in every bridge
  - `take_staged_call` error latching
  - `prefer_engine_call_error`
  - `recover_voxel_presentation_outputs`
  - `pending_recovery_outputs`
  - `complete_call(committed, terminal)`
  - the C# `LeaseReleaseCoordinator` rollback path
- A failing Engine operation returns its status and diagnostic. That is its
  only consequence.
- An exception escaping `Update`:
  - logs `exception.ToString()`, not just `.Message` (`ProductGenerator.cs:567`);
  - publishes whatever was produced, which matches the actual Engine state;
  - moves the lifecycle to a Faulted/Paused state that the developer can
    resume;
  - optionally triggers `Debugger.Break()` in dev. The existing `ReportFault`
    path (`lib.rs:2060`) is most of this.
- Process respawn is reserved for real crashes and code reload.
- Baselines (effects, audio, video, presentation) are built lazily, only when a
  renderer attaches.
- Replace the idle begin/discard probe with one that publishes a realistic
  snapshot containing a few MB of generated mesh.

---

## 4. P2 — stateless recomputation where retained state belongs

The Engine mostly *holds* retained state, but its APIs are shaped as "here is
everything, recompute". That is authority-era design: the Engine re-derived
truth from submitted facts each time.

### 4.1 Graphics: full snapshot per frame (V/R)

The ABI appearance table has `publish_snapshot` and
`publish_attached_snapshot` (`csharp-engine-abi/src/product.rs:1418-1419`,
**V**), and no per-object transform delta. Rusty Dagger publishes every
update. For each publish:

- `stage_attached_snapshot` (`appearance.rs:7627`) clones the whole
  `appearances` map and one `String` per fact (**R**).
- `RuntimeAppearanceProjector::project_scene` clones the entire resource
  catalog, including inline mesh bodies:
  `resources: self.catalog.resources.clone()`
  (`render-projection/src/runtime_appearance.rs:246`, **V**).
- `validate_resources` finite-scans every vertex and range-scans every index,
  then clones each mesh again (`authored.rs:453-509`, **R**).
- `changed_resource_ids` and `resource_diffs` each deep-`PartialEq` every mesh
  against `last_resources` (`authored.rs:687,754`, **R**).
- Per node, changed or not, the following run (**R**):
  - `Appearance` clone;
  - a throwaway instance descriptor built only to call `.validate()`;
  - a `BTreeSet` in `ensure_acyclic`;
  - a depth sort.
- `release_static_mesh` clones the whole projector and re-projects
  (`runtime_appearance.rs:131-136`, **V**).
- Billboards and particles clone their whole projector per op
  (`billboard.rs:383`). Updating N health bars costs O(N²) (**R**).
- `presentation_assets` and `animation_assets` rebuild a String-keyed map of
  every resource per billboard/particle op and per controller flush
  (`appearance.rs:7883-7928`, **R**).

Combined with P1, a transform-only frame for a product with a few MB of
generated terrain costs roughly:

- 4 deep copies and 3 full scans of all mesh data;
- O(objects) String and map allocation.

All of this happens before any change reaches the renderer.

**Recommendation**

- Define resources once, at `create_mesh_resource`. Emit the `Define*` op then,
  hold bodies as `Arc<[f32]>`, and remove resources from the per-publish
  `AppearanceScene` entirely.
- Add a direct retained API: create/destroy an object handle, then
  `SetTransforms(ReadOnlySpan<(handle, Transform)>)`,
  `SetAppearance(handle, …)` and `SetVisible(span)`. These write to
  `PresentationWorld` and append `RenderDiff::Update`, costing O(changed).
- If the full-snapshot convenience stays, it should diff *facts* only (handles
  plus transforms) and live in the C# SDK or a thin Rust layer on top of the
  delta API. It should not be the canonical path.
- Use `u64` appearance ids instead of Strings across the ABI, and a blittable
  fact struct passed as a span. Today `AppearanceFact` holds a managed
  `Appearance`, so it can never be blittable.

### 4.2 Physics: Rapier world rebuilt every step (V)

`svc-collision/src/dynamics.rs:275` states it directly: "The Rapier world is
rebuilt off-side for every call." Every step:

- builds a new `PhysicsWorld` and inserts every static chunk compound and
  static-mesh instance (`insert_static_environment`, `:678`);
- inserts every body and re-creates every rope joint;
- steps with a cold broadphase and solver, with no warm starting;
- validates, sorts and de-duplicates the input bodies (`:299-330`, **V**).

Around the step, `RigidBodyService::prepare/commit`
(`engine-spatial/src/rigid_body.rs:494-675`, **R**) round-trips through a
Rust `EntityState` per body:

- about 12 component lookups, keyed by `String` type id through BTreeMaps and
  `dyn Any`;
- cloned names and labels;
- exact-revision checks against values captured microseconds earlier in the
  same synchronous call.

`step_and_read` reads each body twice.

Beyond cost, the cold rebuild plausibly degrades stacking and rope stability
(no persistent contact manifolds). That is uncertain, but worth testing.

**Recommendation:** use one persistent Rapier `PhysicsWorld` per
`DynamicsWorld`, keyed by body handle, as the body store. Add or remove static
colliders only when the bound scene changes. Remove `EntityState`,
prepare/commit and revision checks from Dynamics.

### 4.3 Queries and character steps build a throwaway ECS (V)

`EntityState::from_definitions` is called per call in:

- `propose_character` (`spatial.rs:2034`);
- ray and segment casts (`spatial.rs:5578`);
- kinematic motion (`kinematic.rs:330`);
- world origin (`world_origin.rs:218`).

Each call:

- `format!`s a name per entity, only because empty names are rejected;
- validates AABBs twice;
- checks duplicates;
- clones definitions.

For casts, `ignored.contains` is linear, giving O(E·I). A character step also
computes `character_environment` twice, byte-wise FNV-hashing every static-mesh
asset and instance (`svc-collision/src/static_mesh.rs:204`, **R**).

Products call these per character or creature per frame (Dagger, Crawler,
Doom, CraftSurvive, per the lane).

**Recommendation:**

- Queries take borrowed slices directly, or better, the session holds a
  retained entity-collider set updated by deltas.
- Replace environment hashes with revision counters that are bumped on
  mutation.

### 4.4 Voxel edits and residency rebuild the whole scene (V/R)

`csharp-engine-services/src/voxel.rs:209` deep-clones the whole
`VoxelCollisionScene` (**V**) before applying edits. `VoxelEditService::preview`
then does the following (**R**):

1. collects every material voxel into a BTreeMap;
2. rebuilds the `VoxelWorld`;
3. runs `build_from_voxel_world_at_revision`, which:
   - walks every cell;
   - sorts and FNV-hashes all voxels (`authority_hash`);
   - rebuilds every chunk collider as per-voxel cuboid compounds;
   - rebuilds navigation.

Additional costs (**R**):

- Unchanged mesh chunks are deep-cloned.
- Noncollidable-material configuration repeats the rebuild.
- Residency streaming follows the same path and re-hashes every chunk's cells
  (`VoxelChunk::content_hash`, uncached).
- An incremental `CollisionProjection::reconcile(world, changed)` exists and is
  unused (`svc-collision/src/lib.rs:992`).

`MAX_SOLID_VOXELS = 1e6` looks like a policy cap that hides this O(world)
cost.

**Recommendation:**

- Use copy-on-write `Arc` chunks and mutate in place.
- `reconcile` dirty chunk colliders and patch navigation locally.
- Hold mesh chunks as `Arc<VoxelMeshChunk>`.
- Replace `authority_hash`/`content_hash` with per-chunk version counters.
- Drop the duplicate `material_voxels`/`solid_voxels` arrays, which are a third
  copy of voxel truth.

### 4.5 `entity-state` is a transport format, not an authority

C# `EntityStore` is the product's entity authority. Rust `entity-state` now
serves only two purposes:

- a duplicate body store beside Rapier, and through the C# adapters beside C#
  `Transform`/`DynamicsMotion`;
- throwaway argument marshalling (§4.3).

Creating a Dynamics body clones the whole `EntityState` twice
(`dynamics.rs:571`, `authoring.rs:182`), so spawning N bodies costs O(N²)
(**R**). Removing it from hot paths, and probably removing the crate from the
runtime entirely, eliminates a whole layer of revisions and string-keyed
component tables.

---

## 5. P3 — transport and process topology

### Current topology (`rusty dev`, CoreCLR) — R, key hops V

```text
rusty CLI ── stdin (len+JSON) ──▶ rusty-product-host SHELL (HTTP/1.1, thread per connection, OutputBus, SSE)
                                      ▲  loopback TCP, u32-len + serde_json frames
                                      ▼
                                  rusty-product-host --worker (CoreCLR + Engine + product + scheduler)
Browser ◀── SSE /outputs (JSON text, id: cursor) ── SHELL
Browser ── POST /input, /…-feedback, /renderer-diagnostics (new TCP conn each, Connection: close) ──▶ SHELL ──▶ worker
Browser ── GET /resource ──▶ SHELL ──▶ worker (base64 in JSON) ──▶ bytes
```

One render delta goes through these steps:

1. Typed data is converted to a `serde_json::Value` tree, then to bytes, on the
   worker.
2. The shell decodes the bytes into a `Value`, then into typed data
   (`deny_unknown_fields`), and validates it.
3. The shell encodes it again with `to_string`. Above 256 KB it is fragmented
   as JSON-in-JSON with escaped quotes.
4. `format!` makes a copy per subscriber.
5. The browser runs `TextEncoder` over the event just to count bytes, then
   `JSON.parse`. Fragments are re-parsed after reassembly.

The shell never uses the content (**V**: `csharp-product-runtime/src/main.rs:1510-1530`,
`product-dev-host/src/host.rs:3190`).

Byte bodies are affected too (**V**):

- `ProductDevWorkerBundleEntry.bytes: Arc<[u8]>` has no `serde_bytes`
  (`product-dev-host/src/worker.rs:35-39`), so **the browser bundle and every
  preloaded renderer resource cross the worker pipe as a JSON number array**.
  That is about 3.5× inflation, parsed element by element, on every worker
  start and reload.
- Inline mesh floats travel as decimal text through every hop
  (`MeshPayloadSource::Inline`, **V**).
- A reconnect baseline re-sends every inline mesh.

### Reconnect/replay machinery with no local consumer (R)

- the 256-event history ring;
- `Last-Event-ID` resume;
- `rusty-output-lag`;
- fragment transfer ids and aggregate checks, in both Rust and TS;
- private and pending baselines;
- the per-event projection-gate epoch check;
- the `ConnectionBoundary` ack protocol with 5 ms polling loops;
- commit-disposition, delivery-certainty and resync headers;
- retired resource bytes retained for the history window.

The lane estimates that about 40–50% of `product-dev-host`'s roughly 10k
non-test lines is recovery, baseline, resync, cursor or bounded-validation
logic. The browser side (`local-transport.ts`) mirrors it.

### Input latency (R)

- Input shares one serialized browser operation lane with the 750 ms
  diagnostics and feedback POSTs.
- Each POST is a new TCP connection and a new server thread.
- The shell holds its session lock across a synchronous worker round trip.
- The worker takes `publication_gate`, which the scheduler holds for the whole
  update, encode and write.
- Input is drained only at the next tick.

The structural worst case is POST RTT + one update + one tick + SSE.

### Per tick, regardless of content (R)

- 4–5 worker frames, each separately encoded;
- a `RuntimeProgress` SSE event even when idle;
- `finish_call` rebuilding the whole resource-id `BTreeSet<String>` after every
  operation.

### No separate release host (R)

The non-worker path runs the same `ProductDevHost` in-process.
`renderer-webview-host` has no workspace dependents and is not connected to
the C# runtime. A shipped game would currently run on the dev host and bus.

### Recommendation — target transport

- **The worker owns the browser socket.** Use one WebSocket for outputs and
  input. The shell shrinks to a supervisor, or merges into `rusty`. The process
  split has a real reason, CoreCLR signal isolation
  (`docs/evidence/direct-coreclr-8686`), and this keeps that reason while
  removing a hop from every frame and every input.
- **Binary where it is bulk.** Mesh, voxel and resource bodies go as binary
  frames, or as hash-addressed HTTP resources that the browser caches with
  `Cache-Control: immutable`. JSON stays for the small op/handle structure.
  MessagePack or FlatBuffers would add little once the text floats and hops
  are gone.
- **Reconnect = fresh baseline.** Keep one bounded send buffer; on overflow or
  disconnect, send a baseline. Delete:
  - the history ring
  - cursors
  - the lag event
  - fragments
  - transfer ids
  - acks
  - commit/resync headers
  - retirement retention
- **Input** is pushed to a lock-free queue drained at tick start, with no
  receipt round trip. Drop `RuntimeInputResult` unless a consumer is named.
- **Resources and the shell bundle** are served straight from disk in the
  socket-owning process. The worker should not read the browser shell from disk
  only to send it back as JSON (`main.rs:2007-2011`).
- **One tick frame** per tick, with telemetry and progress sampled or opt-in.
  The resource inventory uses a dirty flag.
- **Shipping.** Decide on an explicit production host: the same worker with an
  embedded webview and a custom protocol, or the browser. Build
  `renderer-webview-host` onto *that* transport, or delete it. It currently
  pushes JSON through `evaluate_script` and base64 at mount (`lib.rs:445-475,722`).

---

## 6. P4/P5 — revisions, receipts, guards, and repeated validation

### Revision-checked APIs with a single trusted same-thread caller (R, spot-checked V)

| Area | What is checked |
|---|---|
| Voxel edit | `expected_revision` (**V**, `voxel.rs:203`) |
| Voxel residency | `expected_revision` plus a per-operation `expected_content_hash` |
| Voxel history | cursor hash and revision |
| Annotation edits | six requests take `expected_layer_hash` |
| Triggers | `SetActive`/`Restore`/`OverlapPage` take `expected_revision` |
| Character | continuation `expected_generation`; monotonic command sequence (`DuplicateOrOldCommand`) |
| Dynamics | rebase `expected_entity_revision`/`expected_solver_generation`; the anchor-reaction protocol |
| World origin | three `expected_*` revisions |
| Perception | `expected_projection_identity` |
| Static-mesh collider replace | tautological (see §1) |

Across the tree there are about 240 `expected_revision`-family parameters and
about 2,200 `receipt` mentions. `csharp-engine-services/src/spatial.rs` alone
has 331 receipt mentions.

The **anchor-reaction protocol** is the clearest remnant:

1. observe the anchor, stamping a revision and generation;
2. propose the reaction;
3. `StepWithReactions` de-duplicates on `(source_identity, source_generation)`;
4. it rejects the reaction if anything changed;
5. it re-checks `maximum_impulse`.

This protects against forgery and replay. An ordinary "apply impulse at world
point" action does the same job (**R**, `dynamics/anchor.rs:77-152`).

**Recommendation:** delete expected-revision parameters. Keep a revision only
where it is a cheap change counter that someone reads, such as a render
replacement key or a dirty flag. Receipts should carry results, not proofs.

### C# SDK helpers repeat the pattern and add O(N²) work (R)

- `EntityCharacterController.Step` finds one entity by an O(N) query, twice per
  step, then runs `PrepareBatch` → `ForkForEdit`. That copies every entity
  record and the whole `Transform` and `CharacterMotion` tables
  (`EntityStore.cs:456,733,767`).
- `EntityDynamicsAdapter` works the same way.
- `EntityGraphicsProjection.Publish` builds a dictionary of all transforms
  twice to validate a guard against itself (`EntityGraphicsProjection.cs:55-110`).
- `EntityStore` uses `SortedDictionary` for entities and every table, and
  `Query` allocates a `List` per call.
- Every Inventory operation clones the whole `InventoryStore` and the target
  inventory, and runs `ValidateStore` twice (`Mechanics/Inventory.cs:621,1001,1065,1290`).

The lane found no product using `EntityCharacterController` or
`EntityDynamicsAdapter`. Products call `ProposeCharacterStep` and
`StepAndRead` directly. These helpers are the green path the SDK advertises,
so their cost shapes what new products inherit.

**Recommendation:**

- `Dictionary` everywhere, sorting only where save/debug consumes an order.
- Remove guards and `expectedGuard` parameters.
- Mutate Inventory in place, validating only touched rows.
- Delete or thin the two unused adapters.

### Mesh data is scanned about five times (R)

For a generated mesh:

| Step | Where | Needed? |
|---|---|---|
| Admission bounds and ad-hoc checks | `appearance.rs:4346-4400` | **Yes** |
| `StaticMeshAsset::validate` at admission | `appearance.rs:4497` | No |
| `validate_resources` on every publish | `authored.rs:453-509` | No |
| Two deep `PartialEq` compares on every publish | `authored.rs:687,754` | No |
| `RenderFrameDiff::try_from_ops` | `authored.rs:316` | No |
| O(n²) re-validation of the combined staged frame on every append | `appearance.rs:8186-8207` | No |

The combined staged frame is a test-only mirror, together with `extra_frames`
and `presentation`. The joint-attachment rig check runs in Rust twice and
again in TS.

**Recommendation:** validate once at admission. Everything else becomes
`debug_assert!` at most. Delete the staged-frame mirrors.

### TypeScript validation on the live path (V/R)

- **Render diff ops are not deep-validated live:** `decodeFrame` is a cast.
  The roughly 1,050-line render-diff validator (`render-contracts/src/validation.ts`)
  is reached only through `applyEncodedFrame` and tests.
- **Presentation frames are deep-validated every time** by
  `decodePresentationFrameDiff`. That is about 940 lines, plus 7 `filter`
  passes, plus a second check in each host.
- **View composition and UI projection** are each validated and
  deep-copied/frozen twice. The UI projection also has a strictly-increasing
  BigInt sequence check.
- **Every output** gets `requireKnownFields` and u64-text checks.

**Recommendation:** only finite transforms, existing-handle lookups and a
positive determinant for batching prevent real Three.js failures. Move the
deep decoders to tests.

### Policy caps (R)

Almost every `MAX_*` in services, spatial, codecs and TS hosts is policy. The
lanes list about 50 in services and a similar number in spatial and TS.

| Class | Examples |
|---|---|
| **Keep** (representation limits) | u32 counts; `MAX_VOXEL_MATERIAL_SLOT` 4095; 2^24 exact tile coordinates; MagicaVoxel's 256 dimension; the JS safe-integer limit (enforce in C#); `svc-implicit` sample memory guard (make it a parameter) |
| **Make caller-chosen or remove** | dynamics 1024 bodies/actions per call (forces chunking); 4096 contacts; audio 64 clips / 8 MiB; static meshes 256/4096; triggers 4096; resident chunks 4096; edits per transaction 4096; particles 4096; emitters 64; billboards 500/256; view composition 4 cameras / 8 views; codec byte caps (hostile-input guards); ProductGenerator's 64 KiB debug and error text |

### Loose-content filesystem rules

Symlinks, non-UTF-8 paths and backslashes are rejected
(`csharp-product-runtime/src/lib.rs:4735-4775`, **R**). This is policy that
blocks shared asset directories via symlink.

---

## 7. Dev loop, content, and tooling

### 7.1 `rusty dev` restage (V)

**Every change goes through the same full chain.** Nothing distinguishes C#,
UI and content changes. The chain is:

- `dotnet build`;
- MSBuild staging, which `Exec`s `dotnet restore` and `dotnet build` of a
  generated composition project;
- another MSBuild property query.

That is about five dotnet process launches per change (**R**,
`rusty-cli/src/main.rs:605-629`).

**Composition is forced to rebuild every time.** The composition files are
written with `Overwrite="true"` and no `WriteOnlyWhenDifferent`
(`Rusty.Engine.targets`, **V**). Their timestamps change on every run, which
invalidates the composition build.

**All content is copied twice per change.** Staging does `RemoveDir` and
copies everything; promotion does `RemoveDir` and copies everything again.
Neither uses `SkipUnchangedFiles` or hardlinks (**V**). Rusty Dagger's
`content/` is 623 MB in 2,918 files (**V**), so a one-line C# edit triggers
over a gigabyte of file copying.

**The worker is then replaced cold.** It restarts CoreCLR, and
`RuntimeContentBridge::new` reads and SHA-256s every loose content file up
front, whether or not the product ever asks for an identity
(`csharp-engine-services/src/content.rs:52-58`, **V**). Content bundles hashed
at build time are re-verified with SHA-256 on every open (`content/bundles.rs:108-117`,
**R**).

**Recommendation:**

- **Staging.** In dev, point `product.json` content and UI roots at the source
  directories. Write composition files only when they differ. Skip restore when
  the lock file is unchanged. Use one MSBuild invocation.
- **Change routing.** A content edit reloads content in the running worker, a
  UI edit reloads the browser, and only a C# edit rebuilds.
- **Hashing.** Hash lazily, on the first identity request, or use
  path + size + mtime in dev. Drop bundle re-verification.
- **HTTP.** Stop sending `Cache-Control: no-store` and `Connection: close` for
  hash-addressed resources (`host.rs:4969`; `cache: 'no-store'` in
  `dynamic-renderer-resources.ts:108` and `renderer-preload.ts:32,54`).
- **Hot reload.** Later, evaluate CoreCLR hot reload (`MetadataUpdater`)
  inside the worker. The transport change in §5 makes a C# reload cheaper even
  without it.

### 7.2 Resource admission re-hashing (R)

| Resource | SHA-256 passes | Other work |
|---|---|---|
| PNG | 2 | A bitwise (table-less) CRC32 over every chunk; rejection of anything that is not 8-bit RGBA (`render-model/src/assets.rs:567-606,715-794`) |
| Font, audio, video | 2 | — (`appearance.rs:268,299,333`) |
| Animated GLB | 3 | Two body copies; runtime glTF parse and preflight. `voxel-convert/src/source.rs:207` recomputes the hash it was just handed |

**Recommendation:**

- Thread the bridge's `sha256` through every admit path.
- Drop the CRC.
- Accept any PNG the browser decodes.
- Move GLB import to build time or cache it.

### 7.3 Content-addressed identity pinning (R)

- **Voxel assets** verify their identity by clone, canonicalize, JSON-encode
  and SHA-256 on every decode (`voxel-asset/src/codec.rs:135-165,492-499`).
  Any hand edit fails with `contentHashMismatch`.
- **Scene closure** rejects a referenced asset whose hash differs
  (`authored-scene/src/admission.rs:370-380`).
- **The content store** (`content-store`, `content-store-host`) is a
  generational compare-and-swap store. Each publish re-reads and re-hashes
  every body, validates the manifest twice, and rewrites the whole store with
  an `fsync` per file. The lane found no Dagger usage.

**Recommendation:** compute hashes when writing, and treat a mismatch as a
warning. Replace the content store with temp-file-plus-rename writes, or
delete it until a consumer exists.

### 7.4 ABI binding pipeline friction (R, spot-checked V)

The pipeline is cbindgen → ClangSharp → a custom generator. Its costs for
contributors:

- The generator writes a tracked Rust file back (`generated_abi_identity.rs`).
- `cbindgen.toml` keeps a hand-maintained include list of about 748 names.
- Generation runs as an unconditional `Exec` before every compile in two
  projects.
- One Rust capability touches at least four files and six sites, including
  about 40 lines of `extern "C"` shim per operation. There are 449
  hand-written externs and 655 error constructions.

**Recommendation:**

- A declarative macro over bridge methods that emits the typedef, table field,
  initializer and shim.
- A Rust-side ABI model, either `syn` over the declarations crate plus
  `size_of`/`offset_of!` from a build-time binary, feeding the existing C#
  emitters. That removes cbindgen, clang and the write-back.
- Rust computes the fingerprint as a const.
- Add Inputs/Outputs to the `Exec` hooks so generation is incremental.

Two further simplifications (**R**):

- The optional callback pairs kept "so older products stay loadable"
  (`csharp-product-runtime/src/lib.rs:775-860`) cannot be reached under an
  exact fingerprint match. Make every callback required.
- Replace leases plus `Read*At(index)` loops (18 operations, one FFI crossing
  per element) with caller-provided output spans. C# supplies
  `stackalloc`/`ArrayPool` storage; Rust fills it and returns a count, in one
  crossing, with no destroy call.

### 7.5 Repository hygiene

- **Tracked build outputs:**
  - `render/artifacts/**` (about 2.8 MB, including a 1.4 MB
    `product-browser-host.js`);
  - `rust/crates/renderer-webview-host/artifacts/renderer-webview.js`;
  - `studio/artifacts/**`;
  - `artifacts/csharp-sdk-feed/*.nupkg`;
  - `scripts/__pycache__`.
- **`docs/evidence`:** 70 files.
- **`docs/audit-78xx.md` and `validation-operation-audit.md`:** they describe
  quota and timeline rules as current.
- **Stray `csharp/Rusty.Engine.{Application,Resolution,Persistence,Entities}`
  directories:** they contain only ignored `bin/obj` output (**V**).

---

## 8. Legacy surface to delete or quarantine

Each item below is reported by a lane as having no production consumer. Verify
with a build before deleting.

| Surface | Why |
|---|---|
| `rust/crates/renderer-webview-host` | No workspace dependents; not connected to the C# runtime (**R**) |
| `render/packages/render-projection` TS `RenderProjection` (1,941 lines) | A mirror of `PresentationWorld`. Production `ThreeRenderer` imports only `RenderPublicationTracker`. Used by the editor viewport, demo proofs and tests; also enforces its own caps |
| Rust `render-projection` `entity.rs`, `debug.rs`, `RetainedNodeProjector`, `model_preview.rs`, `PresentationProjectorSet` | Tests/debug only |
| `EngineServiceSet::begin_attach_call` and its dependents (`reset_renderer_projection`, `rebase_ghost_plates`) | No non-test callers |
| `runtime-ui/src/channel.rs` (`RuntimeUiProjection`, `Prepared*`), JSON encode/decode helpers | No external users |
| `render-contracts/src/validation.ts` render-diff validator and `ThreeRenderer.applyEncodedFrame` | Tests only |
| Browser-owned realtime (`advanceRealtime` POST per RAF) | The shell sets `realtimeAdvanceOwner: 'rust-host'` |
| Backend RAF loop in renderer-three | `autoStart: false`; the surface drives frames |
| `animation playback readout` in `userData` per frame | Only the golden `snapshot()` reads it |
| `CollisionProjection::identity` | No non-test caller |
| `studio/` (about 31k lines; own protocol v15, about 227 hand-written types duplicating Rust models; adapter binary outside this repo) | Move out of the repo or freeze. CI routing sends 14 Rust crates' changes to a job that builds none of them |
| `content-store` / `content-store-host` | No consumer found in Dagger |
| `runtime-session` recovery vocabulary (`RuntimeMutationCertainty`, `RuntimeInvalidatedScope`, `RuntimeNextAction`, `PreparedRuntimeReplacement`) and resync receipts | Replay/authority vocabulary; about 87 taint and 42 resync references downstream of it |

---

## 9. TypeScript renderer specifics

These are not validation, but they directly cap frame rate.

- **GPU duty governor (V).** `renderer-three/src/gpu-submission-duty.ts:81-85,405-440`
  holds GPU duty to ≤50%, falling toward 20% as work grows. With a timer query
  reporting 12 ms of GPU work, the minimum interval is 36 ms, about 28 fps,
  even though 60 fps was achievable. A sync-fence ring adds another gate.
  **Make it opt-in, or throttle only above the frame budget.**
- **Double world render (V).** `browser-surface.ts:544` always renders
  world + viewmodel with the fallback camera. `viewComposition.render`
  (`view-composition.ts:256-470`) then clears and re-renders the world for each
  primary view. Any product using a primary C# `CameraView` pays about 2× draw
  calls. Offscreen targets re-render every frame and ignore their `stale` flag
  (**R**). **Skip the fallback pass when a primary view exists.**
- **Per-pass O(scene) CPU work, even with no changes (R):**
  - forced `updateMatrixWorld(true)` 2–3 times per pass;
  - `prepareStaticInstanceBatches` frustum-tests, rewrites every instance
    matrix, re-uploads `instanceMatrix`, and recomputes bounds on every pass;
  - sprites are re-filtered with parent walks and re-sorted;
  - a material/mesh define marks every handle dirty.

  **Update matrices once, unforced; rewrite instances only on membership or
  transform change.**
- **Recovery builds a second renderer (R).** `application-host.ts:878-960`
  creates a new canvas and WebGL context and re-decodes every resource. It is
  triggered by:
  - any `RenderApplyError`;
  - any configured-domain diagnostic on a presentation frame, e.g.
    `budgetExceeded` from the 64-emitter cap.

  **Clear and re-apply in place, keeping the content-addressed caches. Treat
  rejects as bugs.**
- **Per-frame garbage (R):**
  - a discarded `structuredClone` of every delta, where only `.resources` is
    read (`application-content.ts:89,134-138`);
  - particle arrays copied and allocated per particle per frame;
  - billboards forcing synchronous layout through `canvas.clientWidth` reads
    per billboard.
- **Feedback chatter (R):**
  - ghost-plate POSTs even with zero plates;
  - renderer-diagnostics POSTs a full snapshot, with `getParameter(UNMASKED_*)`,
    every 750 ms;
  - audio and animation facts carry `acceptedThroughFactId` echo/eviction
    bookkeeping.
- **Layering (R).** `renderer-host` hard-depends on its only backend,
  `renderer-three`. Each frame's result is translated four times:
  1. a Three throw;
  2. a surface receipt;
  3. an application-host frozen receipt;
  4. product-browser-host recovery.

  **Merge `renderer-host`'s surface into `renderer-three` and make failures
  exceptions.**

---

## 10. Architectural direction for a C#-first Engine

This keeps C# logic, Rust glue and TypeScript presentation. The change is how
those elements meet.

1. **Retained handles with deltas, not submitted worlds.** Every Engine
   service that holds state (graphics objects, physics bodies, colliders,
   triggers, voxel chunks) exposes create/update/destroy by handle, with
   batched span updates. Queries read the retained state. "Publish everything"
   APIs become optional C# SDK conveniences built on top. This single rule
   removes P2 and most of P4, and makes cost proportional to change
   everywhere.

2. **In-place mutation; failures are values; exceptions are bugs.** There is
   no per-callback transaction. An operation that fails returns a status and
   diagnostic. An exception from C# is logged in full and pauses the
   simulation, so the developer can inspect, fix, and hot-reload or resume.
   This matches how every mainstream engine treats first-party script errors,
   and it deletes the candidate/commit/taint/recovery stack across Rust, C#
   and TS.

3. **One process owns the frame.** The CoreCLR worker hosts the Engine *and*
   the browser socket. The supervisor only restarts it. Presentation deltas
   are produced once, in the final wire format, with binary bulk data. A
   renderer attach, whether first load, reload or reconnect, always receives a
   fresh baseline built on demand from `PresentationWorld`. That is the only
   recovery rule.

4. **Bulk data by reference.** Meshes, textures and voxel bodies are defined
   once, held as `Arc` in Rust, and sent once as binary or served as
   immutable, cacheable HTTP resources. Ops refer to them by handle.

5. **The FFI surface is spans in, spans out.** Blittable structs. C# provides
   output buffers, so no lease table or destroy calls are needed for
   transient results. Leases remain only for truly retained native resources.
   A macro generates the Rust shims. A Rust-side ABI model generates C#
   directly.

6. **Validation lives in exactly one place per fact: where it is admitted into
   retained state.** It is limited to what the representation or backend
   requires. Everything else is `debug_assert!` or test code. Policy limits
   become caller-chosen parameters with generous defaults, or disappear.

7. **Revisions are change counters, not preconditions.** A counter exists
   only if someone reads it to skip work: render replacement, dirty chunks,
   UI re-render. No API takes an `expected_*` argument.

8. **The dev loop is incremental by construction.** Serve source content in
   place, rebuild only C#, and reload content or UI in place. The runtime
   already has enough structure to do a hot content swap once §3 and §5 land.

A C# game developer would feel these changes as:

- frame cost that scales with what changed;
- caught errors that stay caught;
- a thrown exception that pauses instead of respawning;
- sub-second content and UI iteration;
- one socket to reason about when debugging.

For engine contributors, each new capability touches:

1. an ABI struct;
2. a service method;
3. a macro line;
4. a TS realization.

Today it touches 9–12 layers. The lanes counted joint attachments at 9 and
video at about 12.

---

## 11. Documentation that currently defends the old model

These passages would steer an agent to *preserve* the mechanisms above. They
should be rewritten as the code changes, not before.

**`docs/architecture.md` treats as invariants:**

- "prohibition on replaying possibly committed work";
- "The canonical Rust call candidate … Failure discards the call candidate";
- the reconnect/history/fragment rules;
- "exact-revision validation before mutation" for tethers.

**`AGENTS.md`**'s ABI text ("copied retained data", leases) and its "fresh
baseline" language presume P1 and P3.

**`docs/validation-inventory.md`, `validation-operation-audit.md` and
`audit-78xx.md`** are campaign ledgers. Several of their "remaining decisions"
are superseded by this review's framing. Move them to history once acted on.

**`docs/performance.md`**: "Resource bytes must be shared, not copied, when a
transactional C# call begins" assumes the transaction exists.

---

## 12. Suggested sequencing

1. **Immediate bug fixes (§1):**
   - the caught-error taint;
   - the pointer clamp;
   - error-size and newline `expect` panics;
   - the audio `seen_signals` leak;
   - the Dynamics `Err(_) => 0` swallow;
   - `serde_bytes` on worker bundle bytes as a stopgap.
2. **Measure first:** extend the crossover probe with a realistic snapshot
   (a few thousand objects plus about 5 MB of generated mesh), a 64-body
   Dynamics step, a 32-character step, and a single voxel edit in a 256³
   world. Capture before numbers so later steps are attributable.
3. **Quick TS wins (§9):**
   - make the duty governor opt-in;
   - fix the double render;
   - stop forced matrix walks and per-pass instance rewrites;
   - make hash-addressed resources cacheable.
4. **Dev-loop quick wins (§7.1):**
   - `WriteOnlyWhenDifferent`;
   - source-root content in dev;
   - lazy hashing;
   - change-type routing.
5. **Remove P1:** in-place mutation, the fault → pause model, lazy baselines.
   This is the prerequisite that makes steps 6–8 straightforward.
6. **Retained graphics delta API** and `Arc` resource bodies (§4.1).
7. **Persistent Rapier, incremental voxel scene, slice-based queries**
   (§4.2–4.4), removing `entity-state` from hot paths.
8. **Transport consolidation** (§5): the worker-owned WebSocket, binary bulk
   data, and deletion of the replay machinery.
9. **ABI ergonomics** (§7.4) and **legacy deletion** (§8), plus the
   documentation rewrite (§11).

---

## 13. Coverage and confidence

**Reviewed:**

- all `rust/crates` on the runtime path;
- `csharp/Rusty.Engine*`, the generators and the MSBuild targets;
- `render/packages/*` source;
- `scripts/` relevant to dev and generation;
- `studio/`, at a high level only;
- rusty-dagger usage, spot-checked.

**Not reviewed in depth:**

- `core-space`, `svc-implicit`, `svc-mesh`, `environment-authoring` and
  `voxel-object-runtime`;
- studio internals;
- NativeAOT publish specifics;
- `asset-import` beyond hashing and copying.

Nothing was executed. Frame-cost magnitudes are inferred from structure:
what is cloned, scanned or rebuilt, and how often. Confirm them with the §12
step 2 probe before prioritising among items of similar rank. Line numbers are
as of `61f9f6eb`.
