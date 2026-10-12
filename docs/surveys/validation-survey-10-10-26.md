# Rusty Engine: survey of remaining pointless validation

Date: 2026-10-10. Base commit: 2ee8375 (Reconcile only the renderer resources a call changed).
Method: ten read-only subagents, each assigned a crate group and the same brief
(Appendix 0), with the ResourceList whole-catalog revalidation as the calibration
example. Thirteen of the hottest High items were then re-checked against source
by hand; all thirteen held. Nothing was edited, built or run.

## Totals

| Area | Findings | High | Medium | Low |
|---|---|---|---|---|
| csharp-engine-services: appearance, render resources, composition, camera, audio | 31 | 10 | 15 | 6 |
| csharp-engine-services: spatial, dynamics, character | 27 | 8 | 12 | 7 |
| csharp-engine-services: content, bundles, voxel, authored content, rest | 26 | 2 | 12 | 12 |
| csharp-product-runtime, csharp-engine-abi, test host | 12 | 2 | 3 | 6 |
| product-host, product-container, desktop-shell, rusty-cli, runtime-*, svc-http, svc-session | 36 | 12 | 14 | 10 |
| render-wgpu | 16 | 2 | 5 | 9 |
| render-model, render-projection, render-presentation, other render crates | 31 | 7 | 9 | 15 |
| asset-catalog, asset-import, authored-scene, voxel-*, entity-state, core-* | 34 | 11 | 16 | 7 |
| svc-collision, svc-implicit, svc-mesh, svc-pathfinding, svc-rng, svc-volume, engine-spatial | 38 | 10 | 20 | 8 |
| C# SDK and scripts | 30 | 7 | 12 | 11 |
| **Total** | **281** | **71** | **118** | **91** |

Each area's full report (every finding with file:line, cost, confidence, removal
shape, plus a "judged legitimate" list and coverage notes) is an appendix below.

## Cross-cutting patterns

The same handful of shapes recur in every area. Fixing by pattern will be
cheaper than fixing by finding.

**1. Whole-collection pass per call (the ResourceList shape).** The fix in
2ee8375 covered one layer. The identical shape survives one layer down in
render-presentation (every Release* op scans all nodes, meshes, materials,
atlases and ghost captures; every Destroy finds descendants by a fixpoint loop
over all nodes), one layer up in the services (presentation_assets and
animation_assets rebuild a map of every resource on every billboard, emitter,
particle-emit and controller flush; light logical-id guard scans all lights;
"still in use" dispose scans across six services; texture lookup by linear
registry scan), in svc-collision static-mesh residency (clone every asset,
rebuild the instance map, re-fold topology, hash the catalog per delta), in
render-wgpu (sprite batch liveness sweep per view pass; per-DefineMaterial scan
of all parts), in asset-catalog (project_catalog_material validates the entire
catalog per material, O(N²)), in authored-scene (whole scene validated before
and after every edit with two document clones), and in the C# SDK
(EffectsComponent, InventoryStore and EntityGraphicsProjection re-validate the
whole collection per mutation or per frame).

**2. Re-hashing data whose identity is already known.** SHA-256 recomputed on
the Engine's own output: content bodies on every open in render_resources and
audio; packed mesh resources on every decode in render-model; voxel assets on
every decode/encode/canonicalize/resolve (and again at spatial staging and
conversion apply); voxel objects up to five hashes per frame on one admission;
annotation layers twice per query and about twelve per edit; asset-import
re-reads every staged file and re-hashes it against a manifest built from the
same bytes; voxel-convert re-derives three identities of an in-process plan;
the character continuation checkpoint SHA-256s its own config and verifies it
against itself; the spatial session "fingerprint" SHA-256s three scalars; every
chunk collider rebuild hashes the whole chunk to fill a field nothing reads;
every weighted volumetric path query hashes every resident chunk, twice. The
release-pair scripts hash the same bytes four to five times in one job.

**3. Validate-again at each layer.** Bodies validated at services admission
are re-validated per id by the projector (the services even carry comments
saying "every later projection revalidates"), then by RenderFrameDiff on every
Engine-built frame, then by PresentationFrameDiff, and finite/NaN rechecked in
render-wgpu. Spatial queries re-validate every collider row per raycast; the
character step re-validates Engine-retained mesh transforms and its own 45-field
config per command; engine-spatial re-validates every voxel of every chunk on
every rebuild; entity-state re-validates the controller's own output per step;
the C# Transform/CharacterMotion validators run on values the Engine just
returned. product-host decodes, re-encodes and decodes every input POST and
validates output groups three times per receipt.

**4. Duplicate and "already present" guards where insert would do.** Particle
emitter Create, static-mesh asset/instance, nav traversal cells, character mesh
instance/entity ids, kinematic rows, dynamics chain ids, materials-for-scene
slots, bundle ids, authored content rows, DuplicateTag/DuplicateLabel for lists
the Engine dedups anyway, the generated debug catalog's AlreadyRegistered,
entity-state refusing idempotent ops as errors.

**5. Caps and grammar on first-party data.** 4096 px frame side (refuses real
5K and HiDPI 4K viewers), 512 KB host response cap (can fail a diagnostics
batch), 4 KB log line cap that permanently fails the log writer on one long C#
stack trace, 160-char title, 256 commands / 16 params / 1 KB strings in the
debug catalog, 65,536 nav traversal cells, voxel coordinate and material-slot
caps, RNG scope/key byte quotas, 512-byte trigger diagnostics, bloom/tint/
metalness magnitude refusals, lowercase-only identity grammar per envelope,
>2^53 integer walks over every UI/input JSON payload, 240 Hz / 16-step
lifecycle caps, ~25 u64 handle counters refusing on checked_add overflow.

**6. Path refusals on paths the Engine built.** normalize_bundle_path /
renderer_path (also falsely refuses non-ASCII filenames), persistence
scope/key traversal refusal, product_bundle native artifact symlink/".."
refusal (blocks a symlinked .so in dev), content-root lstat duplicate of
ProductSource::open, loose ProductSource::read canonicalize+symlink check per
file the Engine enumerated, the MSBuild manifest task's "unsafe path" refusals
on the product's own csproj paths.

**7. Rollback, preflight and recovery with no observed failure.** Annotation
edits applied twice on a cloned layer with an unreachable "changed during
commit" refusal; camera ops clone the whole camera state for rollback; ingest_batch
full-state checkpoint per batch; pending-input overflow triggering a full
input-lane resync; character controller StaleEnvironment rollback at commit;
InventoryEdit whole-store clone plus optimistic RevisionConflict.

## Hot list: start here

Ordered by confidence then by how often it runs. Line numbers are at 2ee8375.

| # | Where | What | Runs |
|---|---|---|---|
| 1 | `rust/crates/render-presentation/src/world.rs:1233-1356`, `1062-1085` | Every Release* op scans all retained nodes/meshes/materials/atlases/ghost captures; every Destroy fixpoint-loops over all nodes | per release/destroy op |
| 2 | `rust/crates/csharp-engine-services/src/appearance.rs:7292` (calls 2118, 3054, 3089), `:7273` (call 6824, via 6916-6948) | presentation_assets / animation_assets rebuild a map of every resource per billboard, emitter, particle emit, controller flush and snapshot | per call, per snapshot |
| 3 | `rust/crates/engine-spatial/src/lib.rs:1244, 1272`; `rust/crates/svc-collision/src/lib.rs:1020, 1346` | Whole-chunk content_hash on every collider rebuild for `CollisionProjection::identity`, which has no callers | per chunk rebuild |
| 4 | `rust/crates/svc-pathfinding/src/lib.rs:1856, 2198`; `rust/crates/csharp-engine-services/src/spatial.rs:2314` | Hash every resident chunk per weighted volumetric path query, and again in the bridge | per path query |
| 5 | `rust/crates/csharp-engine-services/src/spatial.rs:5808-5830, 5781-5789, 5915-5917` | Every raycast/segment/AABB/capsule query walks all product colliders checking finite and min<=max | per query |
| 6 | `rust/crates/csharp-engine-services/src/spatial.rs:3449-3496`, `3632-3672`, `3710-3715` | Character step re-validates Engine-retained mesh transforms; builds three BTreeSets to refuse dup/zero ids; refuses the step if an obstacle's unused scale != 1 | per character step |
| 7 | `rust/crates/engine-spatial/src/character_controller.rs:1600, 1675, 2168, 2175` | Controller validates its ~45-field config per command, re-validates/re-hashes obstacle and mesh lists per sub-step and at commit | per command, per sub-step |
| 8 | `rust/crates/runtime-input/src/lane.rs:677-683` | Clones the entire compiled mapping table on every physical input event, mouse moves included | per input event |
| 9 | `rust/crates/product-host/src/host.rs:1741-1758` | Input POST decoded to Value, re-serialized, strictly decoded again; body cap applied three times | per input POST |
| 10 | `rust/crates/product-host/src/model.rs:1719-1746`, `host.rs:2772-2836`, `publication.rs:134-200` | Output groups validated three times per receipt | per 60 Hz tick |
| 11 | `rust/crates/render-projection/src/appearance.rs:159-242` | apply_kind re-runs full mesh (O(V+I)), rig (O(J²)), material/texture/atlas/shader validate on every changed id already validated at admission | per changed resource |
| 12 | `rust/crates/render-model/src/core.rs:1871-1938` | RenderFrameDiff::try_from_ops re-validates every op of Engine-built frames including full mesh bodies; checks op_count==len and revision==base+1 computed one line earlier | per voxel projection |
| 13 | `rust/crates/render-projection/src/material.rs:26-35` (via `asset-catalog`, `AssetCatalog::get` linear) | validate_catalog over the whole catalog per material projection, O(N²) plus cycle DFS | per material projection |
| 14 | `rust/crates/render-model/src/mesh_resource.rs:309-435`; `csharp-engine-services/src/render_resources.rs:177/215/251/285, 1611`; `audio.rs:492/575` | Content bodies re-SHA-256'd on every open though RetainedContent.identity holds the digest; RSTATMSH decoded and re-hashed on import then decoded again by the renderer; audio to_vec + hash before the existing-clip check | per content open |
| 15 | `rust/crates/voxel-asset/src/codec.rs:122-169`; `object_codec.rs:131-182, 673-688`; `voxel-object-runtime/src/admission.rs:85-142`; `voxel-convert/src/object_query.rs:272-285`; `spatial.rs:884`; `voxel-convert/src/planning.rs:254` | Voxel asset hashes recomputed on every validate/decode/encode/canonicalize/resolve, again at staging and apply; voxel objects hashed up to five times per frame per admission | per asset op, per frame |
| 16 | `rust/crates/voxel-annotation/src/{query.rs:71-72, codec.rs:87-118,185-198, edit.rs:109-135}`; `csharp-engine-services/src/voxel_content.rs:518-570, 1461-1470` | Annotation layer hashed twice per query, ~12 times per edit (edit applied on a clone then for real, with an unreachable "changed during commit" refusal), 4 per load | per query, per edit |
| 17 | `rust/crates/svc-collision/src/static_mesh.rs:265-390` | Residency delta clones every asset, rebuilds the instance map, re-folds topology, hashes the catalog; three Duplicate* refusals | per residency delta |
| 18 | `rust/crates/engine-spatial/src/lib.rs:596-607` | Scene rebuild re-validates every voxel of every resident chunk | per rebase / set_mesh_options |
| 19 | `rust/crates/render-wgpu/src/effects.rs:1400-1404`; `particles.rs:178-187` | Sprite-batch buffer liveness sweep per view pass per frame; particle Create refuses an existing handle | per frame |
| 20 | `rust/crates/csharp-engine-services/src/camera_view.rs:1469, 1255-1350, 569-580` | Every camera op (incl. per-frame sample) validates a throwaway composition then rebuilds and validates the whole one; clones camera state for rollback | per camera op, per frame |
| 21 | `rust/crates/csharp-engine-services/src/spatial.rs:3849-3884, 2640-2698` | SHA-256 fingerprint of own config written beside the config and verified on restore; SHA-256 of three scalars as a session fingerprint | per checkpoint/restore |
| 22 | `rust/crates/csharp-engine-services/src/spatial.rs:5312-5446` | Spatial content artifact fully re-validated after parse and re-parsed per placement | per placement |
| 23 | `rust/crates/asset-import/src/publish.rs:291-322` | Re-reads every staged file, re-hashes against a manifest from the same bytes, re-encodes the manifest and byte-compares | per import |
| 24 | `rust/crates/authored-scene/src/edit.rs:173-178, 192, 296-305` | Whole scene validated before and after every edit, two document clones | per edit |
| 25 | `rust/crates/product-host/src/log.rs:22, 226-231, 304-306`; `frames.rs:75, 358-363`; `host.rs` 512 KB response cap | 4 KB log line cap permanently fails the writer; 4096 px frame side refuses 5K/HiDPI; 512 KB cap can 500 a diagnostics batch | concrete harm |
| 26 | `rust/crates/csharp-product-runtime/src/lib.rs:62, 2114-2171, 3220-3235` | Pending input queue bound; overflow triggers full input-lane resync, clears held keys, republishes baseline | per update when two batches land |
| 27 | `csharp/Rusty.Engine/Mechanics/Effects.cs:308-386`; `Equipment.cs:306-373, 413-435`; `Inventory.cs:1118-1174` | Whole-collection re-validate and full map diff per mutation; capacity recomputed twice and store cloned per inventory op | per mutation |
| 28 | `csharp/Rusty.Engine/Entities/EngineComponentTypes.cs:47-111` via `EntityStore.cs:730`; `EntityGraphicsProjection.cs:63-122` | Validators on every component write incl. Engine-returned values; per-frame dup-id/parent-cycle scan that Rust already does | per write, per frame |
| 29 | `scripts/build-runtime-pack.sh:196-204`, `build-csharp-release-pair.sh:141-151`, `verify-csharp-release-pair.sh:83-93,145-150`, `test-csharp-release-pair.sh:20,42`, `publish-csharp-release-pair.sh:39`, `test-runtime-pack.sh:57,103-104` | Same bytes hashed and verified four to five times in one job; pack hashed before and after an unrelated write | per CI job |
| 30 | `rust/crates/csharp-engine-services/src/render_resources.rs:1499-1595` | Traversal/absolute/backslash/ASCII-only/512-byte refusals on a path built by `format!("content/{..}")`; falsely refuses non-ASCII filenames | per resource |

## Notes on scope and judgement

- The ABI edge (pointer/length, UTF-8, catch_unwind, release pairing), the
  single bind-time fingerprint handshake, network-facing bounds in svc-http,
  svc-session and product-host's request reader, the Host/Origin check, the
  one archive checksum per downloaded pair, import-time parsing of third-party
  files, and GPU capability refusals were all judged legitimate and are listed
  in each appendix so they need not be re-surveyed.
- The content bundle path is clean on the specific rule in AGENTS.md: no
  SHA-256 is recomputed on bundle open. What remains there is a byte-length
  recheck after staging, hex-grammar re-parsing of Engine-written digests, and
  an O(N²) duplicate/overlap scan at admission.
- HTTP fetch has no post-receipt re-checks.
- Several findings are "dead but costly" rather than "defensive": the
  chunk-collider hash and the volumetric path source hash feed fields nothing
  reads. Those are the cheapest wins.
- A few Medium items are partially load-bearing and need a replacement rather
  than a delete; each appendix says which (for example render-wgpu
  batch_variants, and the Release* scans that must become per-id counts as in
  2ee8375).
- Every finding is a claim from a read, not a measurement. The hot-list ordering
  is by how often the code runs, not by measured cost.



---

# Appendix 0: BRIEF

## Survey brief: pointless validation in Rusty Engine

Repo: /home/user/rusty-engine (Rust engine + C# SDK). READ-ONLY survey. Do not edit, do not build, do not run tests. Use grep/rg, sed -n, cat.

## Why
For months the owner has been removing validation, hashing and "adversarial" defensive code that an earlier coding model scattered through the engine regardless of whether the use case needed it. They thought it was done; then found another instance (the renderer ResourceList re-validated and diffed the *entire* catalog on every call that touched a resource, O(N) per call; fixed in commit 2ee8375 — `git show 2ee8375 --stat` for the shape). They want a fresh sweep to find any remaining sites.

## Project policy (from AGENTS.md, authoritative)
- Product code (C#) is trusted first-party code. Do NOT protect the Engine from the product: no hostile-input ceremony, permission systems, compatibility negotiation, duplicate guards "just in case".
- Validation, hashing and integrity checks (SHA-256 re-verification, schema re-validation, duplicate guards, defensive bounds) spread until they strangle performance. One earns its place only with a concrete failure it prevents, the smallest remedy, and a cost that fits how often it runs. Existing code, docs or tests alone are not that reason.
- Trust identities the Engine or SDK already computed (e.g. a staged inventory's SHA-256 is NOT recomputed on bundle open).
- The ABI boundary handles ONLY actual ABI/lifetime concerns: layout, pointer/length coherence, valid ownership of retained data, no unwind across ABI, exact release of owned resources. Those checks are LEGITIMATE. Service-level checks beyond that need a concrete behavior or representation reason.
- Keep mechanisms small. "To find out whether a mechanism is needed, start by removing it."

## What to look for (candidates)
- Re-hashing / re-verifying content, SHA-256 or other digests, checksums, size re-checks, "integrity" checks on data the Engine or SDK itself produced or already verified upstream.
- Schema / shape re-validation of data that was already validated at an earlier stage (e.g. validated on import, then validated again on load, then validated again on projection).
- Whole-collection validation or diffing per call/per frame where only a delta changed (the ResourceList pattern).
- Duplicate-id guards, uniqueness scans, "already registered" refusals where last-write-wins or a map insert would do.
- Defensive bounds/range/NaN/finite checks on values the product supplies, where the product is trusted and the value would merely render oddly; magnitude caps, "sanity limits", max-count limits, max-string-length limits, max-nesting limits with no concrete failure behind them.
- Path traversal / ".." / absolute-path / symlink refusals on paths the product or SDK built itself.
- Compatibility/version negotiation, feature-flag handshakes, "protocol version" checks between Engine and product (same build pair, not needed).
- Permission / capability / allowlist systems gating product requests.
- Tamper or "adversarial" language in comments: adversarial, hostile, malicious, untrusted, tamper, forged, spoofed, attacker, injection, sanitize, sanitiz, hardening, defense in depth, "cannot be trusted", "must not trust".
- Double-checking invariants already enforced by the type system or by construction (asserting a Vec built from N items has N items; re-checking a handle a typed map already guarantees).
- Reference-counting / liveness / "still referenced" scans that walk whole collections.
- Retries, recovery, rollback, quarantine or "poison" mechanisms with no observed failure behind them.
- Checks that `debug_assert!` would cover being done as runtime `if ... { return Err }` on hot paths.
- Tests whose sole purpose is to prove one of the above rejections.

## What is NOT a finding (do not report)
- ABI/lifetime checks: null pointer, pointer/length coherence, UTF-8 of a borrowed string at the boundary, catch_unwind at an extern "C" edge, handle-table lookups returning an error for an unknown/released handle.
- Checks on genuinely external untrusted input: bytes fetched over HTTP, files a user drags in, third-party asset files on *import* (glTF, images) at the import step. (Re-checking them *after* import on every load IS a finding.)
- GPU/adapter capability refusals (feature not supported on this adapter) — those are policy.
- Parsing errors for a malformed file format at first parse.
- Obvious cheap single-value checks that produce a clear error at a product-facing API where a bad value would otherwise cause a Rust panic or UB (e.g. index into a slice). Use judgement: a check that prevents a panic is fine; a check that prevents an "odd" render is not.

## Output format
Write your findings to the file named in your assignment, as markdown. For EACH finding:

### <short title>
- **Where:** `path:line` (repo-relative, exact line numbers; multiple if a pattern repeats)
- **What it does:** 1-3 sentences, concrete.
- **Why it looks pointless:** which criterion above; what already guarantees the property; what would actually happen if the check were deleted.
- **Cost:** how often it runs (per call / per frame / per load / once), and whether it scans a whole collection.
- **Confidence:** High / Medium / Low that this should be removed. Be honest; a Low with a reason is useful.
- **Removal shape:** one line: delete, replace with debug_assert, move to import time, replace with per-id delta, etc.

Order findings by (confidence desc, cost desc). Put High-confidence, hot-path items first.

Then a final section `## Looked at and judged legitimate` listing things you considered but rejected (one line each) so the owner doesn't re-survey them. Then `## Coverage` listing the files you actually read vs. skimmed vs. skipped.

Be thorough: read the code, not just grep hits. Grep for the vocabulary above plus: validate, verify, verif, check_, ensure_, sanity, sha256, Sha256, digest, hash, checksum, integrity, duplicate, Duplicate, already_, unique, MAX_, max_, limit, LIMIT, cap, clamp, is_finite, is_nan, traversal, "..", canonical, version, Version, compat, negotiat, allow, permit, reject, refuse, Refus, quarantine, poison, rollback, retry. Many hits will be legitimate; filter per the policy.

Aim for a complete pass over your assigned files. Report the actual count of findings; zero legitimate findings in a region is a fine answer if you actually read it.


---

# Appendix 1: 01-services-appearance

## Survey 01: csharp-engine-services appearance / render_resources / composition / camera_view / audio

Scope: `rust/crates/csharp-engine-services/src/{appearance.rs, appearance/animation_pose.rs, render_resources.rs, composition.rs, camera_view.rs, audio.rs, renderer_settings.rs, render_output.rs}`. No other module file under `src/` starts with image/effect/light/sky/cloud/weather/precipitation/fog (the sky/fog/cloud/precipitation/image-effect services live inside `camera_view.rs`; lights inside `appearance.rs`).

Reference shape: commit 2ee8375 replaced "validate and diff the whole catalog per call" with per-id change tracking and per-id reference counts. Several siblings of that pattern remain in the services layer, listed first.

Counts: **High 10, Medium 15, Low 6** (31 findings). Ordered by confidence, then cost.

---

## High

### 1. `presentation_assets` rebuilds a map of every texture/font resource on every billboard, emitter and particle-emit call
- **Where:** `rust/crates/csharp-engine-services/src/appearance.rs:7292-7320` (builder); call sites `appearance.rs:2118` (`presentation_emit_particles`), `appearance.rs:3054` (`stage_billboard`), `appearance.rs:3089` (`stage_particle`).
- **What it does:** Iterates the whole `RenderResourceRegistry`, filters textures and fonts, clones every asset identity and content hash into a fresh `BTreeMap<String, ResolvedRenderAsset>`, and hands it to the presentation projector so it can check `AssetMissing` / `AssetKindMismatch` / `ContentHashMismatch` for the one op being projected.
- **Why it looks pointless:** Whole-collection validation per call where at most one or two assets matter (the ResourceList pattern). The op's texture/font was already resolved from a live handle by `presentation_texture_ref` / `presentation_font_ref` (lines ~3010-3045), which already checked kind and copied the content hash from the registry; the projector then re-finds the same entry by identity and re-compares the hash the service just copied out of it.
- **Cost:** Per billboard create/update/destroy, per emitter create/update/destroy, per particle emission: O(N resources) with N string clones. A HUD with many billboards updated per frame pays this per billboard per frame.
- **Confidence:** High.
- **Removal shape:** Pass only the op's own resolved asset(s) (or a `&RenderResourceRegistry` lookup closure) to the projector; drop the content-hash re-comparison; keep the projector's asset-kind check only if the projector needs it for its own representation.

### 2. `animation_assets` rebuilds a map of every animated-mesh resource on every controller flush, including unchanged controllers on every snapshot
- **Where:** `appearance.rs:7273-7290` (builder); `appearance.rs:6824` (`flush_animation_controller`), reached from every `set_animation_float/bool`, `fire_animation_trigger`, `tick_animation` (6509-6670) and from `flush_all_animations` (6916-6948), which `stage_changes` (7234) and `publish_resource_releases` (7265) call for **every** controller on every snapshot/release.
- **What it does:** Scans the registry, clones id + content hash of every animated mesh into a `BTreeMap`, then (6855-6860) returns early if the controller's target and revision are unchanged, i.e. the map was built for nothing in the common case.
- **Why it looks pointless:** Same ResourceList pattern. The controller's own asset identity and content hash are already held in `AnimationInstance.{asset, content_hash}` (6845 `AnimationProjectionTarget { content_hash: instance.content_hash }`). The map only exists so the projector can re-check the asset still exists and its hash matches, which the live resource handle already guarantees.
- **Cost:** Per snapshot: O(controllers x animated resources) allocations, even when nothing changed; plus per parameter set/tick.
- **Confidence:** High.
- **Removal shape:** Move the early-return above the map build; pass the single `(asset, content_hash)` of the instance instead of a full map.

### 3. `texture_descriptors_for_material` finds each texture by linear scan of the whole registry
- **Where:** `appearance.rs:10281-10301`; callers `appearance.rs:3433` (`create_material`), `3545-3570` (`create_authored_material`, called three times), `3722` (`update_material`), `3768` (`replace_material`, pre-validation pass before `create_material` repeats it).
- **What it does:** For every texture identity a material names, `resources.iter().find(|r| r.asset_identity() == identity)` then clones the `TextureDescriptor`.
- **Why it looks pointless:** The registry already keeps `identities: BTreeMap<String, u64>` keyed by `asset_identity` (`render_resources.rs:437`), and the material request carried the texture **handles** (`request.texture`, `normal_map`, `shader.texture_a/b`, water textures), so the descriptors were in hand when `material_descriptor` built the identities; the function throws the handles away and searches by string.
- **Cost:** O(textures-in-material x N resources) per material create/update; `replace_material` and `create_authored_material` do it 2-4 times.
- **Confidence:** High.
- **Removal shape:** Resolve descriptors from the request handles once (or use `identities` + `get`), return them alongside the material descriptor.

### 4. `borrowed_renderer_resource` / `audio_clip_bytes` answer the renderer's per-resource byte requests by linear scan of every resource
- **Where:** `composition.rs:931-936` (`audio_clip_bytes`), `composition.rs:940-954` (`borrowed_renderer_resource`, `renderer_resources`). Callers: `csharp-product-runtime/src/frame_output.rs:764` (`ResourceSource::bytes`, one call per resource the renderer loads), `audio_output.rs:237` (`retain_clips(|hash| audio_clip_bytes(hash).is_some())`, one scan per retained clip), `audio_output.rs:259`.
- **What it does:** Chains appearance registry + recently-released + audio clips + video clips and `find`s by `identity()` / `content_hash()` string compare.
- **Why it looks pointless:** The registry has `identities` (map by asset identity) and audio has `clips` keyed by handle; a by-identity map lookup is available or trivially kept. Not a validation per se, but it is a whole-collection scan per call on the renderer's resource path: a rebaseline that loads N resources is O(N^2); audio `retain_clips` is O(clips^2) per report.
- **Cost:** Per renderer resource request, per audio report.
- **Confidence:** High (as a per-call whole-collection pattern; it is a lookup, not a check).
- **Removal shape:** Expose `RenderResourceRegistry::by_identity(&str)` using `identities` (note textures index by texture id, so add the resource identity too), and a `clips_by_hash` map in audio.

### 5. Renderer resource paths the Engine itself built are re-checked for traversal, absolute paths, backslashes, non-ASCII and length
- **Where:** `render_resources.rs:1499-1507` (`renderer_path`), `render_resources.rs:1573-1595` (`normalize_bundle_path`); called from every `admit_*` (lines 68, 140, 170, 206, 245, 274, 299).
- **What it does:** Refuses a path that does not start with `content/`, contains `..`, `.`, empty segments, `\`, a leading `/`, any non-`[A-Za-z0-9 ._-/]` byte, or is longer than 512 bytes.
- **Why it looks pointless:** The path checked is `format!("content/{relative_path}")` built by the caller a few lines earlier (`render_resources.rs:779`, `audio.rs:491`, `appearance.rs` content paths) from a path that `RuntimeContentBridge` already resolved to a real retained file. The `starts_with("content/")` test is tautological. A product that opens a file with a `#` or non-ASCII letter in its name (allowed by the content bundle) is refused here for no reason. Explicit brief criterion: path-traversal refusals on paths the SDK built.
- **Cost:** Per resource open (cheap), but it is a pure "hostile input" gate with a concrete false-refusal.
- **Confidence:** High.
- **Removal shape:** Delete `normalize_bundle_path`; keep only the extension dispatch (already done by `file()`), or a `debug_assert!(path.starts_with("content/"))`.

### 6. Content bodies are SHA-256-hashed again on every open although the Content service already holds their identity
- **Where:** `render_resources.rs:177` (`admit_font`), `:215` (`admit_shader`), `:251` (`admit_audio`), `:285` (`admit_video`), and `render-model/src/assets.rs:1034` via `admit_texture` (`render_resources.rs:62-133`); the only path that uses the retained identity is `.rmesh` at `render_resources.rs:795-803`. Audio additionally hashes before checking for an existing clip: `audio.rs:492-493`, `audio.rs:575-576`.
- **What it does:** Every open of a PNG/WOFF2/WGSL/audio/video body computes `Sha256::digest` over the full body to derive the resource identity. `RetainedContent.identity` (`content.rs:79-91`, a once-per-content `ContentIdentity` that may already be `known` from the bundle manifest) is dropped on the floor in `RenderResourceImports::file`.
- **Why it looks pointless:** "Trust identities the Engine or SDK already computed." For bundle-backed content the digest is already known from the manifest; for snapshot content it is computed once and cached. A product that opens the same clip/texture repeatedly (common: `open_clip` per sound, deduplicated only after hashing via `acquire_existing_clip`, `audio.rs:464-484`) re-hashes a multi-MB body each time. `admit_font`, `admit_audio`, `admit_video` also take `bytes.to_vec()` (`render_resources.rs:812`, `audio.rs:492`, `audio.rs:575`), copying the whole body before dedup.
- **Cost:** Per open, O(body bytes) hash + copy; audio: before the existing-clip short-circuit.
- **Confidence:** High.
- **Removal shape:** Thread `content.identity.of(&bytes)` (the cached digest) into every `admit_*` as `.rmesh` already does; dedup by path/identity before hashing in audio; pass `Arc<[u8]>` instead of `to_vec()`.

### 7. Binary static meshes (`RSTATMSH`) are fully decoded and their packed resource re-hashed on import, then decoded again by the renderer
- **Where:** `render_resources.rs:1611-1631` (`decode_static_mesh`), calling `render-model/src/mesh_resource.rs:309-350` (`decode_mesh_resource_payload`), which re-validates the payload, recomputes `mesh_resource_content_hash(bytes)` and compares it to the descriptor's hash, then decodes every vertex/index stream into an inline payload that `decode_static_mesh` immediately discards (it keeps only `asset.payload.clone()` and the raw bytes).
- **What it does:** A per-import integrity pass (hash + full stream decode) on a file format that the Engine's own tooling writes, purely to refuse a "damaged" file.
- **Why it looks pointless:** SHA re-verification of Engine-produced content on load; the renderer decodes the same resource again when it draws. The doc comment at 1637-1639 says "the resource is checked as the renderer reads it", i.e. the check already exists downstream. Test `render_resources.rs:1870` (`a_damaged_binary_static_mesh_is_refused_with_the_json_forms_codes`) exists only to prove this refusal.
- **Cost:** Per static-mesh content import (cache miss): O(mesh bytes) hash + full decode, result discarded.
- **Confidence:** High.
- **Removal shape:** Parse the header + JSON descriptor only; take `byte_length` from the descriptor; drop the hash and stream decode (and the test).

### 8. Every camera change validates a throwaway composition, then rebuilds and validates the whole composition again
- **Where:** `camera_view.rs:1469-1499` (`validate_descriptor`), `:1501-1525` (`validate_target_descriptor`); called at `:357, 380, 405, 441` (create/update/update_sample/replace camera) and `:481, 502, 529` (targets). Then `stage_composition` `camera_view.rs:1255-1350` rebuilds every camera (clone + `format!` ids), every target and every view from scratch and calls `composition.validate()` (`:1342`) on each of those same calls plus `set_active`, `clear_active`, `destroy`, `destroy_target`, `set_viewport_anchor`, `set_composition`.
- **What it does:** Validates the one changed descriptor by wrapping it in a fake one-camera composition, then validates the complete composition, so the changed camera is validated twice and all unchanged cameras once per call.
- **Why it looks pointless:** Whole-collection rebuild + revalidation per call where one entry changed; the second validation covers the first. `update_camera_sample` is a per-frame call for any product with a moving camera.
- **Cost:** Per camera op, per frame: O(cameras + targets + views) allocation and validation, twice over for the changed descriptor.
- **Confidence:** High.
- **Removal shape:** Delete `validate_descriptor`/`validate_target_descriptor`; keep the composition cached and patch only the changed camera; validate the changed camera once (or let the renderer's own composition validation report).

### 9. Light "logical id already owned" scans every live light on create/update/replace
- **Where:** `appearance.rs:3319-3328` (`create_light`), `:3350-3358` (`update_light`), `:3372-3380` (`replace_light`).
- **What it does:** `lights.values().any(|c| c.light_id == fact.light_id)` to refuse a duplicate product logical id.
- **Why it looks pointless:** Duplicate-id guard over a product-chosen id in trusted code; a second light with the same logical id would simply replace the first in the projector (`apply_lights` keys by `light_id`). Brief criterion: "duplicate guards where last-write-wins or a map insert would do."
- **Cost:** O(lights) per light operation; a scene with many dynamic lights updating per frame pays O(L^2) per frame.
- **Confidence:** High.
- **Removal shape:** Delete the scans (last write wins), or keep a `light_id -> handle` map if the Engine needs the inverse lookup.

### 10. `ingest_joint_poses` scans all animation instances per report and again in a `retain` every frame
- **Where:** `appearance/animation_pose.rs:357-395`; `:364-368` (`animation_instances.values().any(...)` per report), `:388-394` (`joint_poses.retain(|object| instances.values().any(...))`).
- **What it does:** Drops reports for objects that no longer have a live instance, then re-walks all instances for every retained object.
- **Why it looks pointless:** Liveness scan walking whole collections per frame; `destroy_animation_instance` (5901-5955) already knows the object id and could remove its pose entry directly. The renderer only reports for instances the Engine asked to report (`report_joints`), so the filter is defensive.
- **Cost:** Per frame: O(reports x instances + poses x instances).
- **Confidence:** High.
- **Removal shape:** Remove the pose entry in `destroy_animation_instance`; drop both scans.

---

## Medium

### 11. Snapshot-order restriction for projected controllers and ghost plates is enforced three times, with whole-collection pre-scans per snapshot
- **Where:** `appearance.rs:7176-7210` (`stage_changes` builds a `changed` map of all facts and walks every controller and ghost plate before projecting), `appearance.rs:6831-6840` (`flush_animation_controller` refuses the same condition after the fact), `appearance.rs:7580-7601` (`require_projectable_controller`, again on every parameter set/tick), `appearance.rs:6449-6454` (`destroy_animation_controller` refuses once any frame was projected this call). Also `appearance.rs:7162-7168` (every attachment child linearly searched in `owned`) and `appearance.rs:7214-7218` (`present` closure: `owned.iter().any` per parented light, O(lights x facts)).
- **What it does:** Refuses a snapshot that removes/replaces an animated target or ghost source before the product disposes the controller/plate.
- **Why it looks pointless:** Fail-closed ordering restriction with no concrete failure named; `detach_retargeted_controllers` (7668-7703) already handles a recreated target gracefully, so the same code path could treat a removed target as "detached until it returns". AGENTS.md: repeated restriction requests are feedback about the boundary. The pre-scan makes every full snapshot O(facts + controllers + plates) extra even when none are affected.
- **Cost:** Per snapshot (per frame for products that publish each update).
- **Confidence:** Medium.
- **Removal shape:** Keep one site (the flush), make a missing target detach rather than refuse; index `owned` by object id once instead of `any` per child/light.

### 12. Service-level `validate()` on descriptors that render-projection validates again when it applies them
- **Where:** `appearance.rs:4401` (static mesh asset; its comment "every later projection revalidates catalog meshes" describes the pre-2ee8375 world), `:4528` (atlas), `:4696` (sprite material), `:4756` (sprite), `:5481/5496` and `:5538/5542` (legacy sprite + atlas, note 5541 comment), `:3542` (authored texture), `:3585`, `:3690`, `:9990` (materials), `:9720` (`sprite_texture_descriptor` re-validates a texture descriptor the registry already validated at admission `render_resources.rs:109`, which itself re-validates what `admit_png_rgba8_resource` validated at `render-model/src/assets.rs:1055`), `render_resources.rs:1081-1084` (generated mesh, after `generated_mesh_payload` shape checks at 1385-1420). Projector side: `render-projection/src/appearance.rs:166-236` validates each changed id on apply.
- **What it does:** Validates the same immutable descriptor twice (texture: three times) between construction and the renderer.
- **Why it looks pointless:** Schema re-validation of data validated at an earlier stage. The product gets a local error from the service layer, a later one from the projector; only one is needed.
- **Cost:** Per create/update; cheap individually, but it is the pattern the owner is removing.
- **Confidence:** Medium (owner chooses which layer keeps it; the service layer gives the better error locality).
- **Removal shape:** Keep validation in exactly one layer; delete the other and the stale comments.

### 13. Replace paths validate the replacement twice ("refuse before releasing")
- **Where:** `appearance.rs:3756-3770` (`replace_material`: builds descriptor + texture scan, then `create_material` rebuilds both), `:5563-5570` + `:5459-5500` (`replace_sprite` builds throwaway atlas/sprite descriptors named `legacy-validation`, then `create_sprite` builds them again), `:4863-4871` (`replace_sprite_from_atlas` calls `sprite_from_atlas` twice), `:5781-5789` (`replace_animated_mesh_appearance` clones the whole `AnimatedMeshAsset` twice), `:5957-5965` (`replace_animation_instance`).
- **What it does:** Pre-validates to make destroy+create atomic from the product's view.
- **Why it looks pointless:** "Keep mechanisms small ... do not require a particular transaction design." The call-staging model already discards the staged state on a failed call at the runtime level, and the product is trusted first-party code; doubling construction cost on every replace buys atomicity for a product bug.
- **Cost:** 2x per replace.
- **Confidence:** Medium.
- **Removal shape:** Destroy then create; let a failed create surface its own error.

### 14. "Still in use" scans over whole holder collections on every dispose
- **Where:** `appearance.rs:3783-3796` (`destroy_material`: `appearance_materials.values().any(...)` plus `GeneratedMeshes::uses_material` `render_resources.rs:1027-1035` scanning all partitions and meshes), `appearance.rs:3818-3828` (`destroy_appearance`: all `animation_instances`), `:4254-4265` (`destroy_mesh_resource`: all `mesh_appearances`), `:6047-6069` (`destroy_animation_graph`: all controllers, then `animation_transitions.retain`), `:5708-5725` (`associate_animation_clip_pack`: all animated appearances and graphs, to enforce "associate before creating" ordering), `audio.rs:937-970` (`destroy_clip`: all `voice_clips` and `one_shot_clips`).
- **What it does:** Refuses disposal while a dependent object exists, by scanning.
- **Why it looks pointless:** This is exactly the shape 2ee8375 replaced for textures/materials with per-id counts ("refuse a removal that is still referenced without a scan"). Several of these are not even needed as refusals: a material removed under an appearance just falls back; an atlas/clip removed under a voice is a product ordering bug in trusted code.
- **Cost:** O(dependents) per dispose; disposing N appearances with M instances is O(N x M).
- **Confidence:** Medium.
- **Removal shape:** Per-id reference counts (as 2ee8375 did) where the refusal is kept; delete the refusal where last-wins is acceptable.

### 15. One-instance-per-object uniqueness scan on animation instance create
- **Where:** `appearance.rs:5861-5868` (`animation_instance_mesh`).
- **What it does:** Scans all instances to refuse a second instance for the same `object_id`.
- **Why it looks pointless:** Duplicate guard over a product-chosen id; `AnimationControllerService::attach` and the projector key by object id already, so last-write-wins or a `object_id -> instance` map would do.
- **Cost:** O(instances) per instance create/replace.
- **Confidence:** Medium.
- **Removal shape:** Delete, or keep a `BTreeMap<object_id, handle>` if the Engine needs the inverse.

### 16. Clip existence and rig lookups walk the whole registry and clone every clip id
- **Where:** `appearance.rs:7510-7533` (`animation_asset_clips` / `animation_asset_has_clip`), called per `set_animation_playback` (`:6734`), per `read_animation_controller` (`:6996`), per `create_animation_controller` (`:6382`); `appearance/animation_pose.rs:48-58` (`asset_rig`), called per `set_animation_pose` (`:192`) and `ragdoll_source` (`:318`).
- **What it does:** `resources.iter().filter(kind).find(asset == ...)`, then collects every clip id into a `Vec<String>` to test membership of one.
- **Why it looks pointless:** The instance already holds the resource handle (`animated_appearances[appearance]`, used at `:5852`); `registry.get(handle)` is O(log n). The clip check at `:6734` is a "renders oddly" guard: the renderer reports an unknown clip through realization facts anyway.
- **Cost:** Per playback command / per pose set / per readout: O(resources + clips) with allocations.
- **Confidence:** Medium.
- **Removal shape:** Look up by handle; drop the clip membership refusal (renderer already diagnoses it).

### 17. `create_animated_mesh_appearance` scans the catalog list instead of using `ResourceList::contains`, after cloning the whole asset
- **Where:** `appearance.rs:5747` (`animated_mesh_asset` clones the full `AnimatedMeshAsset` including rig and clips), `:5752-5765` (`animated_meshes.iter().any(|c| c.asset == asset.asset)`).
- **What it does:** Linear scan to decide whether to push the asset into the projector catalog.
- **Why it looks pointless:** `ResourceList` has `contains(&str)` (`render-projection/src/resource_list.rs:115`) and `push` on an existing id now replaces (2ee8375). The clone is paid even when the asset is already there.
- **Cost:** O(animated meshes) + full asset clone per animated appearance create (and twice on replace, finding 13).
- **Confidence:** Medium.
- **Removal shape:** `if !list.contains(id) { push(Arc::new(asset.clone())) }` using a borrowed asset.

### 18. Authored material texture content-hash equality check
- **Where:** `appearance.rs:3521-3527` (`create_authored_material`, `CSHARP_AUTHORED_MATERIAL_TEXTURE_HASH`).
- **What it does:** Refuses when the product-selected texture resource's SHA does not equal the hash recorded in the authored material's voxel surface mapping.
- **Why it looks pointless:** Hash re-verification of two Engine-computed identities to protect against the product selecting a different texture than the authoring pipeline expected; worst case it renders with the texture the product chose. The authored projection already resolves the texture by name.
- **Cost:** Per authored material create (cheap); it is a tamper-shaped check.
- **Confidence:** Medium.
- **Removal shape:** Delete.

### 19. `retain_texture_descriptor` compares the whole retained descriptor for equality on every retain
- **Where:** `appearance.rs:10303-10318`.
- **What it does:** If the catalog already has the texture id, compares the full `TextureDescriptor` and refuses with "retained texture identity resolved to different immutable facts".
- **Why it looks pointless:** Integrity check on Engine-produced data: ids are derived from content hash + sampling (`render_resources.rs:85-99`), so two descriptors with one id are the same by construction.
- **Cost:** Per material texture retain (cheap).
- **Confidence:** Medium.
- **Removal shape:** `if !contains { push }` or `debug_assert_eq!`.

### 20. Camera target edits clone the entire camera state to roll back a refusal; `set_composition` clones it as a candidate
- **Where:** `camera_view.rs:569-580` (`with_state_restored_on_refusal`, used by `update_target` `:503` and `replace_target` `:530`), `camera_view.rs:616-645` (`set_composition`).
- **What it does:** `staged.state.clone()` (all cameras, targets, views, presentations) before applying a change so a refused composition can be undone.
- **Why it looks pointless:** Rollback mechanism with no observed failure; the only refusal is the composition validation of finding 8. If that validation moves or the refusal becomes an error without rollback (as every other service does: "there is no rollback", `composition.rs:308`), the clone goes.
- **Cost:** O(cameras + targets + views) clone per target update / composition set.
- **Confidence:** Medium.
- **Removal shape:** Delete with finding 8; apply the change directly.

### 21. Magnitude caps and finite/range refusals on product-supplied look values (environment, camera, render output, material factors)
- **Where:** `camera_view.rs:727-736` (background colour must be 0..1 and alpha exactly 1), `:814` (exposure), `:838` (bloom intensity <= 16), `:857`, `:877` (grading in -1..1), `:905` (sun radius <= 20 deg), `:925` (sun shafts <= 16), `:941` (wind <= 16), `:964` (clouds: drift <= 1000 m/s, altitude <= 100 km ...), `:982`, `:1012`, `:1069` (cloud region radius <= 1000 km), `:1102`, `:1133` (fog volume extents <= 10 km, emission <= 16), `:1167`, `:1202` (precipitation <= 200000 drops, <= 200 m/s ...), `:1227`, `:1244` (sky light <= 16), `:1399` (viewmodel fov), `:1442/1453`, `:2549-2557` (blend amount); `render_output.rs:173-186` (background 0..1, exposure, pose time); `appearance.rs:10249-10279` (`mesh_material_parameters`: tint channels must be 0..1, so an HDR tint is refused), `appearance.rs:9990` via `RenderMaterialDescriptor::validate` (metalness 1.5 refused, test `appearance.rs:12086`). The `valid()` bodies live in render-model.
- **What it does:** Refuses values outside hand-picked "sanity" ranges with long explanatory messages.
- **Why it looks pointless:** Brief criterion: defensive range caps on trusted product values where the value would merely render oddly. A product tuning bloom to 20 or a stylised tint of 1.5 is a look decision. Some entries do bound GPU memory (indirect-light probe count, precipitation drop count) or avoid a divide (fog distance, auto-exposure ordering); those are policy and are listed below as legitimate.
- **Cost:** Per environment call (cheap), but each one is a product-facing refusal and a test.
- **Confidence:** Medium (per item; the pure "look" caps are High, the finite checks on shader uniforms Low).
- **Removal shape:** Keep only caps with a resource reason (probe count, drop count, non-zero divisors); delete look-range caps and their refusal tests (`composition.rs:1690, 1993, 2191, 2454, 2751`, `camera_view.rs:2788`, `appearance.rs:12086` out-of-range half).

### 22. Sprite playback: transition cap redundant with the duration check; stale/lifecycle/finite checks on Engine-produced update facts
- **Where:** `appearance.rs:49` + `:5331-5336` (`MAX_SPRITE_PLAYBACK_TRANSITIONS_PER_ADVANCE = 16_384`), `:5079-5083` (duration must be > 0, which already makes the loop terminate), `:7954-7985` (`validate_sprite_playback_update`: refuses a non-Running lifecycle, non-finite `fixed_delta_seconds`, and facts that compare below the last identity), `:5313-5318` (`remaining` finite check on a product of two Engine-checked values), `:5101-5107` (marker ids nonzero and unique), `:5406` (sample time finite).
- **What it does:** Guards an Engine-owned playback cursor against Engine-produced `NativeProductUpdateFacts`.
- **Why it looks pointless:** The update facts are produced by the Rust lifecycle and handed to C#, which passes them back unchanged; checking them for monotonicity/finiteness is checking Engine data against itself ("stale update" test at `appearance.rs:910-919`). The transition cap is unreachable once durations are positive.
- **Cost:** Per advance per playback per frame (cheap).
- **Confidence:** Medium.
- **Removal shape:** Delete the cap, the lifecycle/finite/stale checks and the marker uniqueness; keep the duration > 0 check (it prevents a spin) and the duplicate-update short-circuit (`:5303`).

### 23. Audio realization feedback bounds and strict-ordering refusal on Engine device facts
- **Where:** `audio.rs:200-209` (`ingest_realized_feedback`: `facts.len() > 128` or any non-increasing `fact_id` returns `CSHARP_AUDIO_REALIZATION_BOUNDS`).
- **What it does:** Rejects a whole feedback batch produced by the Engine's own `render-audio` realizer (`csharp-product-runtime/src/audio_output.rs:238-245`, which already chunks to `MAX_FACTS_PER_REPORT`).
- **Why it looks pointless:** Engine-to-Engine data; the dedup at `:224-229` already ignores old ids. A refused batch loses completion facts for pending one-shots.
- **Cost:** Per audio report (cheap).
- **Confidence:** Medium.
- **Removal shape:** Delete the check (the FIFO bound at `:230` already limits memory).

### 24. Duplicate-slot and slot-coverage guards on material bindings and generated meshes
- **Where:** `appearance.rs:3953-3957`, `:4027-4031`, `:4104-4108`, `:4177-4181` ("must not repeat a slot"), `render_resources.rs:1057-1069` (`bind`: each slot exactly once, every group slot covered), `appearance.rs:10560-10570` (`publish_appearance_changes`: duplicate attachment / empty joint refusal).
- **What it does:** Refuses repeated slots in a product-built list instead of letting the last binding win (the target is a `BTreeMap`/`Vec` keyed by slot).
- **Why it looks pointless:** Duplicate guards where a map insert would do; an uncovered slot draws with the mesh's default material.
- **Cost:** Per update (cheap).
- **Confidence:** Medium.
- **Removal shape:** Last write wins; delete `must cover every group material slot` unless the renderer panics on a missing slot (it falls back to `material_slots`).

### 25. Each settled render-output job clones every render resource
- **Where:** `render_output.rs:354` (`resources: appearance.render_resources.iter().cloned().collect()`).
- **What it does:** Copies the full registry (descriptors, strings, Arc bytes) into each `RenderOutputWork` so the job survives later releases.
- **Why it looks pointless:** Whole-collection copy per call where the job's frozen frame names the handful it reads; `RenderResourceRegistry` already keeps released bodies servable for a call (`released_last_call`).
- **Cost:** O(N resources) per capture/export job (rare path).
- **Confidence:** Medium (cost fits rarity; shape is the pattern).
- **Removal shape:** Collect only the identities the frozen frame references, or keep an `Arc` snapshot of the registry.

---

## Low

### 26. `checked_add(1)` handle/revision overflow refusals on monotonic u64 counters
- **Where:** ~30 sites, e.g. `appearance.rs:2359, 3276, 3330, 3381, 3431, 3483, 3682, 4538, 5122, 5221, 5263, 5348, 5363, 5818, 6013, 6237, 6415, 7997`, `render_resources.rs:548, 589, 1073, 1144, 1184`, `camera_view.rs:360, 416, 450, 484, 515, 538`, `audio.rs:477, 506, 593, 731, 877`, `render_output.rs:147`.
- **What it does:** Returns a product-facing error ("handle overflow") if a u64 counter wraps.
- **Why it looks pointless:** Unreachable in practice; each is an error code, a message and a branch on a hot path. `audio.rs:725` adds a second `JSON_SAFE_U64_MAX` ceiling on top.
- **Cost:** Per create (negligible), but it is ceremony.
- **Confidence:** Low.
- **Removal shape:** Plain `+= 1` (debug overflow checks cover it).

### 27. Redundant zero-handle / zero-version pre-checks before a lookup that would refuse anyway
- **Where:** `appearance.rs:4511-4516` (`create_sprite_atlas` refuses `texture.value == 0` before `self.resource()` would), `:5988-5993` (`create_animation_graph` refuses `version == 0`), `camera_view.rs:586-591` (`destroy_target(0)`), `:698-703` (`set_sky(0)`), `:2549-2550`.
- **Why it looks pointless:** Double-checking what the handle table already enforces, or a meaningless constraint (graph version).
- **Cost:** Negligible.
- **Confidence:** Low.
- **Removal shape:** Delete.

### 28. `copy_inline` re-checks a by-construction invariant
- **Where:** `render_resources.rs:1221-1228`.
- **What it does:** Refuses if the inline position/index vectors are not multiples of 3/3.
- **Why it looks pointless:** Both were built by `flat_map` over `[f32; 3]`/`[u32; 3]` in `generated_mesh_payload` (1437-1455); cannot be violated.
- **Confidence:** Low. **Removal shape:** `debug_assert!`.

### 29. Sequence/length `u32::try_from` refusals on Engine-built op lists
- **Where:** `appearance.rs:7732-7739`, `audio.rs:1122-1129`, `audio.rs:686-694`, `appearance.rs:8838-8850` (`read_presentation` returns failure if counts exceed u32).
- **Confidence:** Low. **Removal shape:** `as u32` with a debug assert.

### 30. `read_animation` walks the registry three times for a readout
- **Where:** `appearance.rs:7046-7100`.
- **Why it looks pointless:** Counts kinds by full scan on every product read; keep per-kind counters or accept it as a diagnostics cost.
- **Confidence:** Low (readout path).

### 31. Sprite batch instance frame scan before `batch.validate()` repeats per-instance checks
- **Where:** `appearance.rs:4813-4821` (every instance checked against the atlas), then `:4852-4855` (`batch.validate()` checks every instance's scale), then the projector validates again (finding 12).
- **Why it looks pointless:** An undefined frame renders as the atlas' first frame or nothing; three passes over 10,000 instances for a "renders oddly" case.
- **Confidence:** Low. **Removal shape:** One pass or none.

---

## Looked at and judged legitimate

- `composition.rs:264-303` `borrowed_slice` / `borrowed_utf8`: null-with-length and UTF-8 at the ABI edge.
- All `*.get(&handle).ok_or_else(... "not live")` handle-table lookups; `RenderResourceRegistry::resource/get/remove` slot checks (`render_resources.rs:496-520, 652-672`).
- `RenderResourceRegistry::hold/unhold/held/take_unowned` (`render_resources.rs:688-726`): per-id counts, the 2ee8375 shape; `HeldResources::holds` (`appearance.rs:9782`) scans only on the refusal path to name the owner.
- Resource-kind checks when projecting a handle into a typed descriptor (texture vs font vs shader vs animated mesh), e.g. `appearance.rs:1486-1510, 3010-3045, 4284, 5465, 5636-5645`: representation requirement, O(1).
- Import-time parse/format checks on product content files: PNG decode in `admit_png_rgba8_resource`, `.rmesh` header (`render_resources.rs:141`), WOFF2 magic (`:171`), WebM EBML (`:276-284`), WAV RIFF parse (`audio.rs:1135-1173`), GLB import diagnostics (`:306-335`), product WGSL `check_product_shader` (`:209`, names file and line).
- `generated_mesh_payload` stream-length coherence (`render_resources.rs:1385-1395`): a normals stream shorter than positions would index out of bounds in the renderer.
- Rig compatibility at clip-pack association (`appearance.rs:5674-5679`, `is_clip_compatible_with`): a mismatched joint set would index skinning data out of range; once per association.
- Pose joint index bounds and IK descent checks (`animation_pose.rs:190-212`): joint indices index the rig arrays; the descent check is the solver's precondition. (Linear-scan lookup of the rig itself is finding 16.)
- `create_animation_controller` `validate_animation_catalog` (`appearance.rs:6392`): the controller service consumes the validated catalog; once per controller.
- `tick_duration_millis == 0` (`appearance.rs:6318`), `renderer_settings.rs:196` `settings.valid()` (render scale 0 means zero-size targets), auto-exposure ordering, fog distance >= 8 m, indirect-light probe cap (262,144 probes bound GPU memory), precipitation drop cap: resource/divide reasons; policy.
- `MAX_PRESENTATION_DIAGNOSTICS`, `MAX_ANIMATION_REALIZATION_FACTS`, audio 128-fact FIFO: bounded readout buffers, not refusals.
- `appearance_operation` receipt reset / `operation_error` take (`appearance.rs:10587-10615`): error transport, no latching.
- `RenderResourceImports` path-keyed caches with `Arc::ptr_eq` (`render_resources.rs:786-803, 843-848, 907-916`): cheap cache validity, not re-validation.
- `update_voice` refusing a clip/bus change (`audio.rs:814-819`): API shape (patch vs replace), not a validation.
- `camera_view.rs:1005-1023` `set_backdrop` session/camera liveness: handle lookups.
- `uses_sky_texture` on resource release (`camera_view.rs:293-307`): O(1).
- Test-only `thread_local` read counters (`appearance.rs:1227-1233`).

## Coverage

- **Read in full (non-test code):** `appearance.rs` lines 1-10617 (all production code; the file's tests at 215-1226 and 10617-15349 were skimmed by grep for rejection-only tests and the 2ee8375 benchmark), `appearance/animation_pose.rs` (all 715 lines), `render_resources.rs` 1-1645, `composition.rs` 1-1092, `camera_view.rs` 1-2577, `audio.rs` 1-1652, `renderer_settings.rs` (all), `render_output.rs` 1-497.
- **Skimmed:** test modules of the above (names and `expect_err` sites only); `render-model/src/mesh_resource.rs:309-420`, `render-model/src/assets.rs:1017-1057, 905-935`, `render-projection/src/resource_list.rs`, `render-projection/src/appearance.rs` validate sites, `content.rs:20-95`, `csharp-product-runtime/src/{audio_output.rs:225-275, frame_output.rs:740-790, lib.rs:2640-2670}` for call frequency.
- **Skipped:** everything else in the crate (other survey assignments).


---

# Appendix 2: 02-services-spatial

## Survey 02: csharp-engine-services spatial / dynamics / kinematic / implicit

All paths are relative to `rust/crates/csharp-engine-services/src/`. Read-only survey; nothing was edited, built or run.

Summary: 8 High, 12 Medium, 7 Low findings (27 total). The hot-path ones are the per-query collider re-validation (H1), the per-character-step re-validation of Engine-retained mesh transforms (H2) and the per-step identity/duplicate guards on mesh instances (H3). The largest single block of pointless code is the spatial-artifact semantic re-validation (H5), which also re-parses and re-validates a whole artifact once per *placement*. The checkpoint fingerprinting (H4) is the only remaining SHA-256 use in these files and verifies Engine-produced data against itself.

---

## High confidence

### H1. Every spatial query re-validates every product collider row
- **Where:** `spatial.rs:5808-5830` (`filtered_entities` -> `validate_entity_collider`), `spatial.rs:5781-5789` (`validate_aabb`), `spatial.rs:5915-5917` (override rows in `cast_ray_parts`). Callers: `cast_ray` 2833, `cast_segment` 2880, `aabb_query` 2937, `capsule_query` 3025, `spatial/inspection.rs:45` (`read_map`).
- **What it does:** Before filtering, walks the whole borrowed `NativeSpatialEntityCollider` span and refuses the call if any row's min/max is non-finite or min > max. The same walk is repeated for the hitbox-override span.
- **Why it looks pointless:** Defensive finite/range check on product-supplied values. The rows are produced by the SDK's own `EntityTriggerProjection.Project` (docs/csharp-lifecycle.md ~1040) from Transform + SpatialCollider components. If deleted, a NaN or inverted box simply fails every `aabb_overlaps` comparison (all comparisons with NaN are false; min > max never overlaps) and the ray/occlusion code sees an empty box; nothing panics.
- **Cost:** Per query (every raycast, segment cast, AABB overlap/sweep, capsule cast/overlap, map read), O(entities) extra pass over the whole collider span before the real O(entities) filter.
- **Confidence:** High.
- **Removal shape:** Delete `validate_entity_collider`, `validate_aabb` and the override loop; `filtered_entities` becomes a plain filter and can stop returning `Result`.

### H2. Character step re-validates Engine-retained static-mesh transforms
- **Where:** `spatial.rs:3449-3496` (`retained_mesh_entity_transform`), called from `spatial.rs:3681` for each admitted mesh instance per `propose_character`.
- **What it does:** Reads the transform the Engine itself retained in `VoxelCollisionScene` for the instance, casts each component to f32, refuses the whole step if any component is non-finite, then refuses again if the quaternion's norm is non-finite or ~0, then normalizes it.
- **Why it looks pointless:** Re-verifying data the Engine stored (criterion: re-checking data the Engine already produced). The transform was admitted through `replace_static_mesh_colliders` / `apply_static_mesh_residency`; if it were bad the collision projection would already be bad. If deleted, the pose for carry anchors would be whatever the scene holds, which is exactly what collision uses.
- **Cost:** Per character step x mesh instances (every frame for every moving platform the character can stand on).
- **Confidence:** High.
- **Removal shape:** Replace with an infallible cast (keep the normalization if the controller needs a unit quaternion; drop the two refusals and the `Result`).

### H3. Character mesh admission: zero-id, duplicate-instance, duplicate-entity and obstacle-collision guards
- **Where:** `spatial.rs:3632-3664` (`character_mesh_instances`), plus `spatial.rs:3665-3672` (finite linear/angular velocity). Tests whose sole purpose is these rejections: `spatial.rs:8261-8333` (the duplicate-instance, duplicate-entity and "mesh and box obstacle cannot share an entity" cases; the unknown-instance case at 8252-8259 is a legitimate handle lookup).
- **What it does:** Per step builds a `BTreeSet` of obstacle entity ids and two more `BTreeSet`s for instance and entity ids, refusing the step on `instance == 0 || entity == 0`, a repeated instance id, a repeated entity id, or an entity id that also appears as a box obstacle. Also refuses non-finite velocities.
- **Why it looks pointless:** Duplicate-id guards and "already registered" refusals on trusted product input. The product builds `ReadOnlyMemory<CharacterMeshInstance>` from its own entity store; a duplicate would at worst add a duplicate pose row to `poses` (a `Vec` searched with `find`, so first wins). Zero ids are ordinary u64 values to the controller. The obstacle/mesh overlap case is the same entity listed in two forms; the controller would just see both colliders.
- **Cost:** Per character step, three `BTreeSet` builds over obstacles + mesh instances (O(n log n)) before any collision work.
- **Confidence:** High for the four identity guards; Medium for the finite-velocity refusal (a NaN velocity would propagate into the controller's support math, but that is a product bug, not an Engine failure).
- **Removal shape:** Delete the guards and the three sets; keep only the `static_mesh_instance` lookup (unknown handle -> error). Delete the three rejection sub-cases of the test at 8261-8333.

### H4. Character continuation checkpoint: SHA-256 fingerprints of Engine-produced data verified against themselves
- **Where:** `spatial.rs:3849-3884` (`character_config_fingerprint` = serde_json + `Sha256`, `spatial_session_fingerprint` = `Sha256` over three scalars), used at `spatial.rs:2643-2645` (capture) and `spatial.rs:2656-2668`, `2681-2686` (restore). `use sha2::{Digest, Sha256}` at `spatial.rs:42` exists only for this. Also `spatial.rs:2656-2661` refuses when `source_session_identity == 0 || source_generation == 0`, and `spatial.rs:2670-2678` re-runs `entity_state::validate_character_motion` on the motion the Engine wrote into the checkpoint. Tests whose sole purpose is these rejections: `spatial.rs:8962-9000` (NaN motion, tampered gravity).
- **What it does:** Capture serializes the controller config to JSON, hashes it with SHA-256 and stores the first 8 bytes *next to the config itself* in the checkpoint. Restore re-serializes, re-hashes and refuses on mismatch. The session fingerprint SHA-256s `collision_voxel_size`, `collision_chunk_size`, `voxel_surface_mode` instead of comparing the three fields. Restore also refuses a checkpoint whose "diagnostic provenance" fields are zero, although docs/csharp-lifecycle.md:1261-1263 says those are copied provenance, "not a native handle".
- **Why it looks pointless:** Integrity check on data the Engine itself produced and handed to trusted product code; the checkpoint is "a plain value" (docs). Nothing can corrupt it short of the product deliberately editing it, which the test literally does (`invalid_config.config.vertical.gravity += 1.0`). If deleted: a product that edits the config gets the edited config, which it could already obtain by passing any config to `ProposeCharacterStep`. The session fingerprint is three equality comparisons dressed as a digest.
- **Cost:** Once per save/load; cheap, but it pulls `serde_json` + `sha2` into this module, adds an ABI field (`config_fingerprint`) and ~60 lines.
- **Confidence:** High for `config_fingerprint`, `spatial_session_fingerprint`'s hashing, the zero-provenance refusal and `validate_character_motion` on restore. (The content-authority-hash and generation lifecycle refusals are listed separately as M8.)
- **Removal shape:** Delete both fingerprint functions and the `config_fingerprint` checkpoint field; compare `session.config` fields directly if session compatibility is still wanted; drop the sha2 import from spatial.rs.

### H5. Spatial content artifact: semantic re-validation after parse, repeated per placement
- **Where:** `spatial.rs:5312-5431` (`parse_spatial_content_artifact` after the parse itself), `spatial.rs:5433-5446` (`validate_spatial_content_id`), `spatial.rs:69/79/86/93/101/112` (`deny_unknown_fields` on every serde struct). Called per `replace_content_artifact` (1137) and once per admitted placement in `apply_content_artifact_residency` (1335). Rejection-only tests: `spatial/artifact_binary.rs:264-272` (bad bounds, bad cell) and fixture `fixtures/csharp-spatial-artifact/bad-bounds.json`.
- **What it does:** After serde/binary decode succeeds, it: refuses `schema_version != 1`; refuses ids that are empty, > 512 bytes, or contain whitespace/control bytes; checks every bound is finite and min <= max; refuses if positions and triangles are not both-empty/both-populated; walks every position checking finite and inside bounds; walks every triangle checking index range **and** that no two indices repeat; checks five config floats finite and in range; sorts all cell identities to refuse duplicates; walks every cell checking support height finite and within bounds, `level == round(height / quantum)`, and the cell's x/z footprint within bounds +- one cell.
- **Why it looks pointless:** (a) The artifact is emitted by the product's own offline generator/importer (docs/csharp-lifecycle.md:1051-1053), i.e. trusted tooling output, re-validated on every load. (b) Triangle index range and finite positions are checked **again** immediately afterwards by `StaticMeshColliderAsset::new` -> `svc-collision::validate_geometry` (`rust/crates/svc-collision/src/static_mesh.rs:485-506`). (c) For the binary encoding the `level` field is *derived* by the decoder as `round(height / quantum)` (`artifact_binary.rs:78`), so the level consistency check at 5405-5414 is a tautology there. (d) Id length/whitespace limits protect nothing; the ids are never used as paths or keys. (e) Duplicate cell ids would just collapse in `NavProjection::from_walkable_cells` / the `BTreeMap` support map (last wins). (f) Positions outside declared bounds or a degenerate triangle render/collide oddly at worst. (g) `deny_unknown_fields` makes forward-compatible fields a refusal.
- **Cost:** Per `ReplaceContentArtifact` and **per placement** in `ApplyContentArtifactResidency` (each placement re-parses and re-validates the entire artifact: a 544,563-cell town is parsed and validated once per site placed, O(cells log cells) for the uniqueness sort). The ignored `measure_artifact_encodings` test reports ~75 ms JSON / ~15 ms binary per parse+validate.
- **Confidence:** High for everything after the decode except `cell_size > 0` / `level_quantum > 0` (divisors; `navigation_grid` already refuses a bad cell size via `VoxelGridSpec::new`).
- **Removal shape:** Keep the parse/decode errors (first-parse of a file format); delete lines 5312-5431 and `validate_spatial_content_id`; let `StaticMeshColliderAsset::new` be the single geometry check; drop `deny_unknown_fields`. Separately, cache the parsed artifact per `ContentReference` so a placement does not re-parse (not validation, but the same per-call pattern as the ResourceList fix).

### H6. `commit_voxel_asset` guards against a session change that cannot happen
- **Where:** `spatial.rs:927-932`, with the supporting `expected_scene` field at `spatial.rs:284` and `Arc::clone(&session.scene)` at 881.
- **What it does:** Keeps an `Arc` of the scene seen at prepare and refuses commit unless `Arc::ptr_eq` still holds.
- **Why it looks pointless:** Prepare and commit are both called inside one synchronous extern "C" call (`voxel_content.rs:1197-1204`), with only `prepare_asset_spatial_publish` (which does not touch Spatial) in between. The check is unreachable; it is "transaction" ceremony around a straight-line sequence.
- **Cost:** Once per voxel-asset publish; negligible CPU, but a dead mechanism with its own error code and doc comment.
- **Confidence:** High.
- **Removal shape:** Delete `expected_scene` and the check; `commit_voxel_asset` becomes an unconditional install (or fold prepare+commit into one function).

### H7. Character obstacle rows must have unit scale or the whole step is refused
- **Where:** `spatial.rs:3710-3715` (`character_obstacles`). Rejection-only test: `spatial.rs:8099-8136`.
- **What it does:** Refuses `propose_character` if any obstacle's `transform.scale != Vec3::ONE`.
- **Why it looks pointless:** The scale is never used: the obstacle box is `translation + (bounds_min+bounds_max)/2` with half extents from the bounds (3720-3723), and the pose pushed for carry anchors is the full transform. A scaled entity would simply be treated as unscaled, which is what the product's own `EntityTriggerProjection` bakes into bounds anyway. The refusal turns one odd entity into a frozen character.
- **Cost:** Per character step, O(obstacles) compare (cheap) but a hard failure path.
- **Confidence:** High.
- **Removal shape:** Delete; document that scale is already folded into bounds. Delete test lines 8099-8136.

### H8. Trigger diagnostic text is truncated to 512 bytes and 64 entries for no buffer
- **Where:** `spatial.rs:63-64` (`MAX_TRIGGER_OPERATION_DIAGNOSTICS`, `MAX_TRIGGER_DIAGNOSTIC_TEXT_BYTES`), `spatial.rs:3260` (`.take(64)`), `spatial.rs:5582-5588` (`bounded_trigger_diagnostic_text`), used at 3263, 3272, 3287, 3292, 4460.
- **What it does:** Truncates each diagnostic message at a char boundary near 512 bytes and keeps at most 64 of them before handing them to `OperationDiagnostics::retain_all`.
- **Why it looks pointless:** `operation_diagnostics.rs:39-75` stores each string as its own `Box<str>` in a `Vec`; there is no fixed-size buffer. Every other bridge passes `error.detail()` through untruncated. The limits exist to "bound" something that is already bounded by the error the Engine itself produced.
- **Cost:** Only on refusal; negligible. Pure ceremony (one function, two constants, five call sites).
- **Confidence:** High.
- **Removal shape:** Delete the two constants and `bounded_trigger_diagnostic_text`; pass `&str` through.

---

## Medium confidence

### M1. Collision residency scans every placed artifact to refuse ids it "owns"
- **Where:** `spatial.rs:1048-1063` (`mutate_collision`), `spatial.rs:165-178` (`SpatialContentComposition::owns_collision`).
- **What it does:** On `apply_collision_residency`, every removed asset id, removed instance id, admitted asset id and admitted instance id is checked against the composition: a map lookup, then a linear scan over all placed artifacts' asset ids, then a hash of the base identity. Any match refuses the whole call with `CSHARP_COLLISION_CONTENT_OWNED`.
- **Why it looks pointless:** Protects Engine bookkeeping from the product changing "its" ids through another API. The product owns both the artifact ids and the collision ids; if it removes a placed artifact's instance via the collision API, last-write-wins would leave `content.placed` stale, which is the same state `replace_collision` already produces when it wipes `session.content` (1080-1082).
- **Cost:** Per residency call, O((removed + admitted) x placed) scan.
- **Confidence:** Medium (the refusal does prevent a stale `placed` map; but the simpler remedy is to drop the id from `placed` when it is removed through collision, which is a map `remove`, not a scan).
- **Removal shape:** Delete `owns_collision` and the check; on removal, `session.content.placed.remove(id)`.

### M2. Content artifact placement: "already resident" refusal and finite-translation refusal
- **Where:** `spatial.rs:1389-1399` (already resident / product-owned instance id), `spatial.rs:1319-1324` (`finite_vec3(translation)`).
- **What it does:** Refuses the whole residency call if a placement id is already placed, appears twice in the same call, or names a static-mesh instance the product admitted through the collision API; refuses a non-finite translation.
- **Why it looks pointless:** Duplicate-id guard where a map insert (replace) would do; `apply_static_mesh_residency` upstream already handles instance replacement by id. The `product_owned` test requires a scene lookup per arrival. A non-finite translation would place one artifact at NaN (renders/collides oddly) rather than warranting refusal of every other placement in the call.
- **Cost:** Per placement call, O(arrivals) with a scene lookup each.
- **Confidence:** Medium.
- **Removal shape:** Delete; treat a repeated id as replacement (remove old instance, insert new).

### M3. Voxel-asset staging demands a "fresh, empty" session (12 conditions) and matching grid
- **Where:** `spatial.rs:851-880` (`prepare_voxel_asset`).
- **What it does:** Refuses unless the session's scene has revision 0, zero voxels, zero chunks, zero static mesh assets/instances, no navigation, navigation revision 0, no character receipt, no trigger definitions, no active overlaps, world origin at ZERO with revision 0; separately refuses if the asset's cell/chunk size differ from the session's.
- **Why it looks pointless:** Unnecessary fail-closed restriction. The candidate scene is built entirely from the asset (887-900), so the session's prior voxel size is irrelevant once replaced. `replace_collision` and the navigation replacements already replace parts of a used session without a lifecycle gate; the only thing this protects is navigation/trigger state derived from the old scene, which the same code could reset (as `replace_collision` resets `content`). Docs at 838-842 justify it as "deliberately an initialization operation", i.e. by policy, not by a failure.
- **Cost:** Once per asset publish; cheap, but 30 lines and two error codes.
- **Confidence:** Medium.
- **Removal shape:** Delete both refusals; reset `navigation`/`content` the way `replace_collision` does, or leave them (same as other scene replacements).

### M4. `read_map` caps the request at 1024 cells
- **Where:** `spatial/inspection.rs:3` (`MAX_MAP_CELLS`), `spatial/inspection.rs:10-14`; rejection-only test at `spatial/inspection.rs:233-234`.
- **What it does:** Refuses a debug map read with more than 1024 cells (or zero).
- **Why it looks pointless:** Max-count limit with no concrete failure behind it. The result is a `Box<[NativeSpatialMapCell]>` the bridge owns until the next call; a product asking for 64x64 cells gets a refusal instead of 4096 cells.
- **Cost:** Per map read; trivial, but a hard product-facing limit on a debug tool.
- **Confidence:** Medium.
- **Removal shape:** Delete the cap (keep `checked_mul` for the allocation size if desired).

### M5. Kinematic motion refuses duplicate entity rows and a selection-shape inconsistency
- **Where:** `kinematic.rs:159-166` (sorted duplicate scan), `kinematic.rs:319-321` (`!selection_present && !selected_ids.is_empty()` -> status 0 with no diagnostic).
- **What it does:** Sorts the product rows (needed for the later `binary_search`), then refuses the call if two adjacent rows share an entity id; separately returns a bare failure if a selection span is supplied without `selection_present`.
- **Why it looks pointless:** Duplicate guard on trusted rows; with duplicates present, `binary_search_by_key` finds one of them and the solver treats the other as a second body, which is what the product asked for. The selection consistency check refuses a request that is unambiguous (the flag says "no selection").
- **Cost:** Per motion call, O(n) extra pass after the sort.
- **Confidence:** Medium.
- **Removal shape:** Delete the `windows(2)` check and the flag/length cross-check (or make `selection_present` mean `!selected_ids.is_empty()`).

### M6. Dynamics chains: duplicate-id refusal and anchor-reach refusal
- **Where:** `dynamics/chain.rs:231-236` (duplicate chain id), `dynamics/chain.rs:251-259` (endpoints "out of reach"). Rejection-only test lines: `dynamics.rs:1724-1729` (bead_count 0, mass -1; the mass case is upstream solver) and `dynamics.rs:1741` (duplicate id).
- **What it does:** Refuses `create_chain` if a chain with that id exists ("remove before recreation"), or if `|end - start| > link_length * bead_count + 0.001`.
- **Why it looks pointless:** Duplicate guard where replace would do (remove the existing chain's beads, then build). An over-stretched initial placement is a physically valid start state; the rope solver pulls the beads in on the first step. Both are product-policy decisions made for the product.
- **Cost:** Once per chain creation; cheap.
- **Confidence:** Medium.
- **Removal shape:** Replace on duplicate; delete the reach check (keep `bead_count == 0` only if `build_chain` would divide by zero: it does at 292, so keep that one or clamp).

### M7. Perception continuation refuses a cursor from a replaced scene
- **Where:** `perception.rs:297-304` (`expected_projection_identity` mismatch -> `CSHARP_PERCEPTION_STALE_CURSOR`). Rejection-only test: `perception.rs:746-862`.
- **What it does:** A paged perception query may carry the projection identity from the previous page; if the session published a new scene since, the page is refused.
- **Why it looks pointless:** Compatibility/version negotiation between consecutive calls of the same trusted caller. If deleted, the next page is evaluated against the current scene, which is at worst a page of slightly inconsistent visibility facts; `projection_identity` is already returned in every readout so a product that cares can compare it itself.
- **Cost:** Per perception page; trivial, but an ABI field + error code + 117-line test.
- **Confidence:** Medium.
- **Removal shape:** Delete the field and refusal (keep `projection_identity` in the readout).

### M8. Character continuation lifecycle refusals (generation, content hash, "newly-created session")
- **Where:** `spatial.rs:2613-2618` (capture: generation must equal latest), `spatial.rs:2634-2639` (capture: content authority hash unchanged since the step), `spatial.rs:2687-2692` (restore: content authority hash must equal target scene's), `spatial.rs:2693-2698` (restore: target session must never have stepped). Rejection tests: `spatial.rs:8955-8960`, `9002-9063`.
- **What it does:** Capture refuses unless the product passes back the exact latest generation and nothing in the scene changed since; restore refuses if the target scene's content hash differs at all from the source's, or the target controller has ever run.
- **Why it looks pointless:** These are documented product-facing rules (docs/csharp-lifecycle.md:1257-1261), but each is a fail-closed restriction with no Engine failure behind it: restoring motion into a session whose world differs by one voxel edit (the test's case) would just make the next step collide differently. Requiring a virgin target session means a product cannot re-seat a character after a scene stream.
- **Cost:** Once per save/load; cheap.
- **Confidence:** Medium (owner-visible behaviour; worth asking whether any of it was requested by a product).
- **Removal shape:** Delete the four refusals; restore becomes "install this motion".

### M9. Voxel navigation refuses `agent_height_voxels == 0` before the projection does
- **Where:** `spatial.rs:1669-1674`.
- **What it does:** Explicit refusal before `build_nav_projection`.
- **Why it looks pointless:** `build_nav_projection` already returns `NavError::InvalidAgentHeight` for the same input and the mapping at 1690-1692 already turns it into `CSHARP_NAVIGATION_CONFIG`. Double check.
- **Cost:** Once per navigation replace; trivial.
- **Confidence:** Medium.
- **Removal shape:** Delete.

### M10. Finite/ordering checks on product-supplied query inputs
- **Where:** `spatial.rs:2792-2797` (`contains_point`), `spatial.rs:2914-2920` (`aabb_query` min/max + translation), `spatial.rs:3003-3008` (`capsule_query`), `spatial.rs:5907-5912` (`cast_ray_parts`: origin/direction finite, `max_distance` > 0 and finite), `spatial.rs:1731-1741` (collision-navigation region finite and strictly ordered), `spatial/inspection.rs:19-42` (map bounds), `spatial.rs:2438-2443` (`evaluate_navigation_step_facts`, returns `NonFinitePosition` outcome rather than an error).
- **What it does:** Refuses the call if the product passed NaN/inf coordinates, an inverted box, or a non-positive ray length.
- **Why it looks pointless:** Defensive NaN/range checks on trusted values. With NaN inputs the underlying overlap/raycast math returns "no hit"; a zero-length ray returns no hit; an inverted AABB overlaps nothing. None of these panics. The collision-navigation region check is the one place a bad value would do real work (world_to_voxel of NaN), but the `i64::MIN+1 / i64::MAX-1` guards right after (1765-1774) already catch the saturated result.
- **Cost:** Per query (the first four are hot); each is a handful of comparisons, so the cost is code and failure modes rather than CPU.
- **Confidence:** Medium (the navigation-step one is already non-fatal and can stay).
- **Removal shape:** Delete the refusals; keep `finite_vec3` only where a NaN would reach an `as i64` cast that must not saturate (collision-navigation region) if the owner wants it.

### M11. Character support context must name the same entity as the continuation
- **Where:** `spatial.rs:3607-3613` (`apply_character_support`).
- **What it does:** Refuses the step if the product's `support.entity` differs from `motion.support_entity` and the support is not an admitted mesh instance.
- **Why it looks pointless:** Consistency check between two fields the same product wrote in the same request. If deleted, the pose pushed is for `motion.support_entity` with the supplied transform, which is what the product evidently meant.
- **Cost:** Per character step, O(mesh instances) `any` plus a compare.
- **Confidence:** Medium.
- **Removal shape:** Delete the refusal.

### M12. Implicit audit refuses a repeated piece id
- **Where:** `implicit_audit.rs:56-58` (`capture_audit_piece`).
- **What it does:** Refuses `capture_audit_piece` if the audit already holds that piece id.
- **Why it looks pointless:** Duplicate guard where `BTreeMap::insert` (replace) would do; a debug-only audit collection.
- **Cost:** Once per captured piece; trivial.
- **Confidence:** Medium.
- **Removal shape:** Delete; let `insert` replace.

---

## Low confidence

### L1. Dynamics finite/range checks on product-supplied values
- **Where:** `dynamics.rs:1082-1100` (`checked_transform`: finite, quaternion norm within 0.001 of 1, `scale == ONE`), `dynamics.rs:236-241` (gravity finite), `dynamics/anchor.rs:34-36` (local anchor finite), `dynamics/chain.rs:174-183` (chain length control finite/positive), `dynamics/chain.rs:237-247` (link length / radius finite+positive), `dynamics/ragdoll.rs:35-41` (blend in 0..=1).
- **What it does:** Refuses body creation/reset/world creation/anchor observation/chain control on NaN, non-unit quaternion, non-unit scale, or out-of-range blend.
- **Why it looks pointless:** Defensive checks on trusted values. However, a NaN body position or velocity does poison a shared solver world (every contact with that body goes NaN), which is a concrete failure that affects other bodies, not just "an odd render". The scale and quaternion-tolerance checks are the weaker ones: scale is unused by the solver and a slightly off quaternion could be normalized.
- **Cost:** Per body create/reset; cheap.
- **Confidence:** Low (NaN guards may be earning their place; the scale/norm/blend ones are not).
- **Removal shape:** Keep finite checks if the owner agrees NaN-poisoning a world is concrete; delete `scale == ONE`, normalize instead of checking the quaternion, clamp blend instead of refusing.

### L2. u64 counter exhaustion turned into runtime errors
- **Where:** `spatial.rs:809-814` (session handles), `spatial.rs:5625-5629` (`next_navigation_revision`, called ~10 times), `spatial.rs:612-616` (cursor identity, `expect`), `world_origin.rs:65-73`, `dynamics.rs:219-229` (`allocate`, wraps to 0 then refuses), `implicit_surfaces.rs:214-219`, `243-248`, `726-729`, `implicit_audit.rs:19-22`.
- **What it does:** `checked_add(1)` on a monotonically increasing u64 and returns an error on overflow.
- **Why it looks pointless:** A u64 counter incremented once per call cannot overflow in the life of a process. `next_navigation_revision` in particular threads a `Result` through ten call sites for this.
- **Cost:** Negligible CPU; ~40 lines and several `?` chains.
- **Confidence:** Low (harmless, cosmetic; worth removing only when touching those functions).
- **Removal shape:** Plain `+= 1`.

### L3. Per-step scan to pick a synthetic character entity id
- **Where:** `spatial.rs:3543-3563` (`call_local_character`).
- **What it does:** Every character step builds a `BTreeSet` of all obstacle, mesh and support entity ids and finds the smallest positive id not in it, so the controller's self-exclusion never matches a product entity.
- **Why it looks pointless:** Whole-collection scan per step to avoid a collision that a fixed reserved id (e.g. `u64::MAX`, which products do not allocate) avoids for free. Not a validation, but the same per-call-scan pattern.
- **Cost:** Per character step, O(n log n) over obstacles + meshes.
- **Confidence:** Low (depends on whether any product can legitimately use `u64::MAX`).
- **Removal shape:** Replace with a constant reserved id.

### L4. Dynamics re-checks body/world membership per action, per read body, per step
- **Where:** `dynamics.rs:721-734` (`world_body`), called per action at `dynamics.rs:509`, per read body at `473-475`, per reaction at `dynamics/anchor.rs:75`, per tether endpoint at `99`, per joint body at `dynamics/ragdoll.rs:88`.
- **What it does:** For each action, looks up the body slot and refuses if it is tombstoned or belongs to another world.
- **Why it looks pointless:** The unknown/tombstoned part is a handle lookup (legitimate). The cross-world check is a consistency check on trusted input; `DynamicsSolver::step` would ignore or error on an id it does not hold. It runs O(actions) map lookups per step before the solver does its own.
- **Cost:** Per step x actions.
- **Confidence:** Low (cheap; the cross-world case is the only questionable part).
- **Removal shape:** Keep the handle lookup; drop the "another world" comparison or move it to `debug_assert!`.

### L5. Collision-navigation budget zero/saturation guards
- **Where:** `spatial.rs:1746-1752` (`maximum_cells == 0` refusal), `spatial.rs:1763-1774` (i64 saturation guards), `spatial.rs:1775-1802` (`checked_sub/checked_add` then budget compare).
- **What it does:** Refuses a zero budget, a region whose cell coordinates sit within 1 of i64 limits, and a region larger than the product's own `maximum_cells`.
- **Why it looks pointless:** The budget comparison honours a product-chosen number and is fine. The zero-budget refusal is a defensive special case of the same comparison (`columns*rows > 0` is always true once columns/rows > 0 is checked). The saturation guards defend against a world box of ~9e18 units.
- **Cost:** Once per publish; trivial.
- **Confidence:** Low.
- **Removal shape:** Delete the `== 0` branch and the saturation guards; keep the budget compare.

### L6. Implicit surfaces: request-shape policing
- **Where:** `implicit_surfaces.rs:455-457` and `556-558` (`regions.len() > 255`), `implicit_surfaces.rs:270-281` (`material_sample_spacing` finite/non-negative and "requires interpolated boundaries"), `implicit_surfaces.rs:907-911` (`block_samples != 0` refused on the whole-volume call).
- **What it does:** Refuses more than 255 material regions, a NaN/negative sample spacing, spacing with the Centroid mode, or a block size on the whole-volume entry point.
- **Why it looks pointless:** Max-count and mode-combination limits with no stated failure (`MaterialRegion.slot` is `u32`; the 255 appears to be a leftover from a u8 slot). The `block_samples` refusal tells the product which function to call instead of ignoring the field.
- **Cost:** Once per generation; trivial.
- **Confidence:** Low (a u8 slot limit may exist further down in svc-mesh; not verified).
- **Removal shape:** Delete the 255 cap if svc-mesh takes u32 slots; ignore `block_samples` in the whole-volume call; treat spacing <= 0 / NaN as "none".

### L7. Collision-navigation policy finite/non-negative checks at publish
- **Where:** `spatial/collision_navigation.rs:72-93` (`CollisionNavigationPolicy::from_native`).
- **What it does:** Refuses a non-finite/non-positive cell size, radius*2 > cell, slope > 89 degrees, non-finite/negative drop or snap distances, zero supports per column, and u8 overflow of two counts.
- **Why it looks pointless:** The u8 conversions are representation checks (keep). Radius*2 <= cell and slope < 90 are algorithm preconditions the dirty-margin reasoning at lines 9-15 depends on (keep). The finite/non-negative drop/snap checks and `supports_per_column == 0` are defensive on trusted config (a zero loop count just yields no supports).
- **Cost:** Once per publish; trivial.
- **Confidence:** Low.
- **Removal shape:** Keep the preconditions; drop the finite/non-negative and zero-count refusals.

---

## Looked at and judged legitimate

- `composition::borrowed_slice` / `borrowed_utf8` pointer-length coherence and `range_slice` (`spatial.rs:3406-3426`) sub-span bounds: ABI pointer/length coherence; a bad range would index out of bounds.
- `checked_u32` / `checked_count` (`spatial.rs:5776`, `perception.rs:447`): ABI field width conversions.
- Null `context`/`request`/`receipt` checks in every extern "C" wrapper; `catch_unwind` in `implicit_surfaces.rs:651-716` and `1101-1121`: no unwind across ABI.
- Session/handle lookups returning errors for unknown or destroyed handles (`session_mut`, `SpatialCollisionSource::scene`, dynamics `active_world`/`active_body`/tombstones, implicit `retained`/`retained_volume`/`node`, `prepared_world_origin`): handle-table lookups.
- `implicit_audit.rs:201-209` `copied_slice` null/size check: the same pointer/length coherence as `borrowed_slice`.
- `artifact_binary.rs` decoder errors (truncation, trailing bytes, count larger than file, non-UTF-8 id): first parse of a file format. (Its `count()` bound at 142-150 is specifically what stops a corrupt count sizing an allocation.)
- `StaticMeshColliderAsset::new` mapping errors (`spatial.rs:1030`, `1208`, `1344`): upstream svc-collision geometry validation at admission, the single legitimate geometry check (which is why H5's copies are redundant).
- `navigation_grid` / `ChunkDims::cubic` / `VoxelGridSpec::new` refusals (`spatial.rs:1588-1606`, `5594-5612`): zero chunk or invalid cell size would divide by zero in grid math.
- `NavTraversalOverlay::from_cells` and the volumetric overlay errors (cell outside projection, zero cost, duplicate): upstream svc-pathfinding, not in scope; zero cost would break Dijkstra, cell-outside-projection is a representation requirement.
- `range` / `u8::try_from(max_step_cells)` and the `u8_field`/`u16_field` conversions in `character_config` (`spatial.rs:3757-3772`): representation width.
- `character_config(...)?.validate()` in `validate_character_controller_config/command` (`spatial.rs:2709-2732`): explicit validation entry points the product calls on purpose.
- `CollisionNavigationPolicy` radius*2 <= cell and slope < 90 degree preconditions (`collision_navigation.rs:72-79`): the incremental dirty-margin argument depends on them.
- `CollisionNavigationCache::rebase` fractional-cell check (`collision_navigation.rs:364-369`): algorithmic (a non-whole-cell shift genuinely invalidates the cache).
- `MAX_DIRTY_REGIONS` (`collision_navigation.rs:17`): a cost cap on the cache's own bookkeeping with a stated reason (beyond it a full derive is cheaper).
- Ragdoll `links` index bounds (`ragdoll.rs:141-148`) and `bones.is_empty()` (138-140): prevent an index panic at 171 and a zero-bone solver insert.
- Ragdoll `joint(index)` out-of-rig refusal (`ragdoll.rs:130-137`): index into the rig's joint array.
- `blend` handled under L1; `NO_END_JOINT` sentinel: representation.
- `world_origin.rs` `native_global_position` / envelope exclusion: upstream `core_space::GlobalPosition::new` and `WorldOriginRebaseService` (not in scope).
- `kinematic.rs` `PhysicsError::NonFiniteInput` / `InvalidShape` / `InvalidStep` statuses: upstream engine_spatial errors merely mapped here.
- `motion.rs` `rejected_motion_does_not_retain_or_publish_call_rows` test: the NaN rejection is upstream `EntityMotionService`, not this crate.
- `content.rs` `ContentIdentity` computing SHA-256 lazily once per admitted body (`content.rs:33`): hashed once, never re-verified; outside my files but checked because spatial uses `content.sha256()`.
- `SceneRevisions::of` hashing noncollidable materials and surface options (`collision_navigation.rs:166-200`): a cache key, computed per publish and per `change_collision`, not a verification; its cost is O(material entries), fine.
- `implicit_surfaces.rs` `u16::try_from(entry.index)` (468, 983, 1027): representation width of the volume's material index.
- `spatial.rs:5551` support "higher wins" and `NavigationState::world_cell`'s fallbacks: behaviour, not validation.
- Not validation but noticed, out of scope: `NavigationState::world_cell` (`spatial.rs:408-439`) scans **every** `cell_centers` entry per call when any placed artifact has a fractional translation (O(cells) per navigation step evaluation), and `character_capsule` / `CharacterCallWorld::world_transform` (`spatial.rs:3578-3583`) does a linear `find` over poses per lookup. Both are per-call whole-collection walks in the ResourceList shape, just not validation.

## Coverage

Read in full (every line, non-test and test code where noted):
- `spatial.rs` lines 1-6204 (all non-test code) in full; test region 6205-10156: read in full 8044-8340, 8515-8620, 8894-9100; the rest listed by test name and grep-scanned for `expect_err` / `is_err` / `assert_ne!` (the only rejection assertions are the ones cited above).
- `spatial/collision_navigation.rs` lines 1-1560 (all non-test code) in full; tests 1561-3549 listed by name and skimmed (behavioural tests, no rejection-only tests found).
- `spatial/artifact_binary.rs` in full.
- `spatial/inspection.rs` in full.
- `dynamics.rs` lines 1-1629 (all non-test code) in full; tests 1630-3125 read 1630-1860, rest grep-scanned.
- `dynamics/anchor.rs`, `dynamics/chain.rs`, `dynamics/errors.rs`, `dynamics/ragdoll.rs` in full (including tests).
- `kinematic.rs`, `motion.rs`, `perception.rs`, `world_origin.rs` in full (including tests).
- `implicit_surfaces.rs`, `implicit_audit.rs` in full.
- `operation_diagnostics.rs` in full (to judge H8).

Skimmed:
- `implicit_surfaces_tests.rs` (test names + grep for reject/refuse/invalid; the "foreign tokens" test is a legitimate handle-lookup test; the sampling/budget rejections it exercises are upstream svc-implicit).
- `content.rs` lines 20-100, 140-200, 360-445 (to confirm SHA-256 is computed once and not re-verified by spatial).
- `rust/crates/svc-collision/src/static_mesh.rs:24-38, 485-506`, `rust/crates/entity-state/src/character_motion.rs:46-76`, `rust/crates/engine-spatial/src/character_controller.rs:161-201`, `rust/crates/svc-pathfinding/src/lib.rs:663-675, 756-768` (upstream checks referenced by findings).
- `voxel_content.rs:1194-1210` (the prepare/commit caller for H6).
- `docs/csharp-lifecycle.md:1040-1125, 1235-1263`.

Skipped (not in assignment): `appearance*.rs`, `audio.rs`, `authored_content.rs`, `camera_view.rs`, `composition.rs`, `content/`, `diagnostics.rs`, `gameplay_time.rs`, `host.rs`, `http.rs`, `input.rs`, `magica_vox.rs`, `persistence.rs`, `presentation.rs`, `render_*.rs`, `renderer_settings.rs`, `rng.rs`, `session.rs`, `tween*.rs`, `ui.rs`, `video.rs`, `voxel*.rs` (except the 17 lines above).


---

# Appendix 3: 03-services-content

## Survey 03: csharp-engine-services (content, persistence, http, voxel, authored content, misc bridges)

Scope: `rust/crates/csharp-engine-services/src/` minus appearance, render_resources, composition, camera_view, audio, spatial, dynamics and the physics/ragdoll/controller/navigation/collision/image-effects/lighting/sky/weather modules. Read-only survey; no edits, builds or tests.

Findings: 26 (High 2, Medium 12, Low 12). Ordered by confidence then cost.

---

### Annotation edits are applied twice on a cloned layer with an unreachable "changed during commit" refusal
- **Where:** `rust/crates/csharp-engine-services/src/voxel_content.rs:518-570` (clone at 533, preflight apply 543-551, second apply 554-561, `debug_assert_eq!` 562-565)
- **What it does:** Every annotation edit (`set_annotation_label`, `set_annotation_bounds`, tags, parent, etc.) clones the whole `VoxelAnnotationLayer`, applies the transaction to the clone to "preflight", converts the receipt to the ABI result, then applies the same transaction again to the retained layer and refuses with `"annotation metadata changed during its atomic commit"` if the second apply fails.
- **Why it looks pointless:** Rollback/transaction mechanism with no observed failure. The bridge is single-threaded per call; nothing can change the layer between the two applies, so the second `map_err` is dead and the comment's "all-or-nothing guarantee" is already the owner's (`VoxelAnnotationEditService::apply`) per its own contract. The only fallible step between the applies is `prepare_edit_result`, which can fail only on `hash()`/`narrow()` of Engine-produced values (see the Low findings). Deleting it: apply once to the retained layer, build the result from that receipt.
- **Cost:** Per annotation edit call: one full layer clone (all regions, tags, cell assignments) plus a double apply. Scales with layer size.
- **Confidence:** High
- **Removal shape:** Delete the clone/preflight; single `apply` on `self.annotation_mut(handle)?.layer`, then `prepare_edit_result`.

### Bundle inventory re-validated at admission, including a byte-length total that is computed and discarded
- **Where:** `rust/crates/csharp-engine-services/src/content/bundles.rs:131-147` (per-file loop), specifically `:134-137` (relative path, name uniqueness, SHA-256 is 64 hex chars) and `:132,144-146` (`total` checked_add "bundle byte length overflow")
- **What it does:** `ProductContentBundles::checked` walks every file of every bundle in `.rusty-bundles.json`, refuses a non-relative path, a repeated path, or a `sha256` string that is not 64 hex digits, and sums `byte_length` with overflow refusal into `total`, which is never read afterwards.
- **Why it looks pointless:** Schema re-validation of data the SDK itself generated (`csharp/Rusty.Engine/buildTransitive/Tasks/RustyEngineContentBundleManifestTask.cs` writes the inventory; `product_container::check` already validates the packed form at `Container::open`). `total` is dead code. The hex-format check is also duplicated: `hex_digest` (`:683-689`) re-parses each digest at `load` (`:174`) and fails the open on a bad one anyway. If deleted, a malformed inventory would surface as a bundle that does not open or a file that does not resolve, which is the same outcome.
- **Cost:** Once per product start, O(total files). Low cost, but pure ceremony.
- **Confidence:** High (the `total` sum and the hex check); see next finding for the rest of `checked`.
- **Removal shape:** Delete lines 131-147 (keep nothing); `hex_digest` at load already covers digest shape.

### Bundle definitions scanned pairwise for duplicate IDs and overlapping roots
- **Where:** `rust/crates/csharp-engine-services/src/content/bundles.rs:111-130`
- **What it does:** For each bundle, checks ID/root are relative, ID has no `/`, root is not the index file name, then compares against every prior bundle for equal ID, equal root, or nested roots (O(N²)).
- **Why it looks pointless:** Duplicate/uniqueness guard on an SDK-generated inventory. `load` (`:166`) uses `find` on ID (first match wins), `owns_path` (`:156-162`) uses `any`; neither needs uniqueness. Nested roots only matter for `owns_path`, which is already an `any`. If deleted, a duplicate ID would resolve to the first definition.
- **Cost:** Once per start, O(N²) in bundle count (small N in practice).
- **Confidence:** Medium
- **Removal shape:** Delete; if a reason appears later, replace with a `debug_assert!` in the SDK task output test, not the Engine.

### Scene material bindings validated against a full scan of every mesh chunk's groups
- **Where:** `rust/crates/csharp-engine-services/src/voxel_scene_presentation.rs:893-923` (expected-set scan 893-896, duplicate refusal 908-913, missing-slot refusal 914-923)
- **What it does:** `materials_for_scene` builds the set of every material slot used by every group of every mesh chunk in the scene, builds the product's binding map, refuses if the bindings contain a duplicate slot, and refuses if any used slot has no binding.
- **Why it looks pointless:** Whole-collection validation per call (the ResourceList pattern). Duplicate slots: `collect::<BTreeMap>` already keeps the last one; the length comparison exists only to refuse. Missing slots: the product supplied the bindings for the scene it is presenting; what the projector does for an unbound slot was not verified in this survey (it may fall back or error at projection time), so this half is Medium rather than High.
- **Cost:** Per `create`/`set_materials` call: O(chunks × groups) scan plus two set builds. Grows with scene size.
- **Confidence:** Medium (duplicate half High; missing-slot half depends on projector behaviour).
- **Removal shape:** Delete the duplicate refusal (last write wins); drop the expected-set scan if the projector handles an unbound slot, else make the projector report it per chunk when it meets it.

### Face material overrides refused for a duplicate (slot, variant, face) key
- **Where:** `rust/crates/csharp-engine-services/src/voxel_scene_presentation.rs:941-964`
- **What it does:** Inserts each override into a `BTreeMap` keyed by (slot, variant, direction) and refuses the whole call with `CSHARP_VOXEL_SCENE_PRESENTATION_DIRECTIONAL` if a key was already present. Also refuses an override whose base slot has no binding (935-940).
- **Why it looks pointless:** Duplicate-id guard where the map insert already resolves it (last write wins). The "requires a base binding" refusal is a shape rule on product rows with no concrete failure named.
- **Cost:** Per materials call, O(overrides).
- **Confidence:** Medium
- **Removal shape:** Delete the `.is_some()` branch; consider deleting 935-940.

### Voxel object presentation demands an exact material binding set and converts slots twice
- **Where:** `rust/crates/csharp-engine-services/src/voxel_content.rs:863-919` (expected set 872-877, first conversion 878-887, set equality 888-893, second conversion 894-906)
- **What it does:** `presentation_materials_for` collects the object's palette slots, converts the product bindings to a `BTreeSet<u16>` (refusing on narrowing), refuses unless `actual.len() == bindings.len() && actual == expected` ("exactly one live material binding for every object palette slot"), then converts the same bindings again into a `BTreeMap`.
- **Why it looks pointless:** Duplicate guard plus exact-set equality on product rows. The palette iteration at 908-917 indexes `by_slot[&palette.material_slot]`, so only "every palette slot has a binding" matters; extra or duplicate bindings are harmless. The first conversion pass exists solely to feed the refusal.
- **Cost:** Per create/update presentation call: three passes over bindings and one over the palette.
- **Confidence:** Medium
- **Removal shape:** One pass into `BTreeMap`; use `by_slot.get(..)` with a per-slot error (or let it panic-free `ok_or`) instead of set equality.

### Cross-presentation "conflicting descriptor" scan on every voxel presentation operation
- **Where:** `rust/crates/csharp-engine-services/src/voxel_content.rs:968-980`
- **What it does:** `project_presentation_state` merges every retained presentation's material map into one map and refuses the operation if two presentations resolved the same material id to different descriptors.
- **Why it looks pointless:** Whole-collection consistency check per call. All descriptors come from `appearance.voxel_material_descriptor` with `id` overwritten by the palette asset id (`:914-915`); the only way they differ is a product binding two Appearance materials to the same palette asset id across presentations, which the product can see and which would merely draw with one of them if `insert` were left to win.
- **Cost:** Per create/update/remove presentation call, O(presentations × materials).
- **Confidence:** Medium
- **Removal shape:** Delete the comparison; keep the `insert` (last write wins), or build the map from the changed presentation only.

### Duplicate payload-row and typed-detail-row refusals in authored catalog / scene admission
- **Where:** `rust/crates/csharp-engine-services/src/authored_content.rs:453-455, 487-489, 501-503, 524-526, 549-551` (material/texture/atlas/region/surface rows), `:2317-2321` (duplicate generator row), `:2434-2438` (duplicate typed detail row), `:2440-2444` (detail row kind mismatch), `:2346-2353` (final pass checking every node has a matching detail row)
- **What it does:** Each row family is tracked in a `BTreeSet` of `("kind", id)` and a second row for the same owner refuses the whole admission; scene detail rows refuse if a node already has a non-EmptyGroup kind, if the row's declared kind differs from what it sets, and a final loop re-checks every node's kind against its declared kind.
- **Why it looks pointless:** Duplicate guards and shape re-validation on rows the SDK's typed C# API assembled; `entry.material = Some(..)` / `node.kind = ..` already make the last row win. The 2440-2444 check compares a kind the function just set against the expected kind it was called with (always true by construction). The 2346-2353 pass is a second whole-document validation after the row loops.
- **Cost:** Per admit/prepare call; O(rows) sets plus a final O(nodes) pass. Not hot.
- **Confidence:** Medium
- **Removal shape:** Delete the `seen` sets and the kind cross-checks; let assignment win.

### Live content admission refuses non-relative dependency paths and duplicate dependencies
- **Where:** `rust/crates/csharp-engine-services/src/content.rs:281-283` and `:306-308`
- **What it does:** `admit_reference` returns 0 if the product-supplied path (or any dependency path) is not `product_container::is_relative_path`, or if two dependencies share a path.
- **Why it looks pointless:** Path shape refusal and duplicate guard on paths the product built. The files only feed a private `BTreeMap` snapshot looked up by exact string (`portable.rs:69-72`); a strange path simply would not match, and a duplicate would just replace. Nothing touches the filesystem.
- **Cost:** Per admission call, O(dependencies). Cheap, but a silent `return 0` with no receipt.
- **Confidence:** Medium
- **Removal shape:** Delete both checks; `files.insert` last-write-wins.

### Persistence scope/key path traversal refusal
- **Where:** `rust/crates/csharp-engine-services/src/persistence.rs:429-447` (called from `:88` and `:103`)
- **What it does:** `storage_path` refuses an empty, absolute, or `.`/`..`-containing scope or key with `CSHARP_PERSISTENCE_PATH` before joining it under the host-selected root.
- **Why it looks pointless:** Traversal refusal on a path the product built itself (criterion: "path traversal / absolute-path refusals on paths the product or SDK built"). The product is trusted first-party code; if it wrote `..` it would write a file where it asked to. The module doc frames "relative-key admission" as a feature, but no concrete failure is named. The empty check is the only one with a representational effect (`root.join("")` is the root dir); that could stay as a one-liner if wanted.
- **Cost:** Per save/load/delete/open, O(path components). Cheap.
- **Confidence:** Medium
- **Removal shape:** Replace with `Ok(root.join(relative))` (optionally keep the empty-string refusal).

### Bundle file byte length re-checked against the inventory after staging
- **Where:** `rust/crates/csharp-engine-services/src/content/bundles.rs:315-329`
- **What it does:** On first read of a build-bundle file, compares the bytes read to the inventory's `byte_length` and refuses with `PRODUCT_BUNDLE_FILE_CHANGED`.
- **Why it looks pointless:** Size re-check on content the SDK staged alongside the inventory it wrote; the brief says the SHA-256 is trusted, and the same argument applies to the length. The comment justifies it as catching "a file cut short by an interrupted restage", a hypothetical not an observed failure; a cut-short JSON file would fail its own parse, a cut-short binary would fail its decoder. The test `opening_reads_no_bodies_and_each_read_reads_only_its_file` (`:804-886`) partly exists to prove this refusal.
- **Cost:** Once per file per open (reads are cached). Cheap.
- **Confidence:** Medium (cheap and the brief flagged it explicitly; the policy rule is "concrete failure", which is absent).
- **Removal shape:** Delete lines 315-329 and the `short.bin` case of the test; `byte_length` then stays inventory metadata only.

### Portable asset readout re-resolves the dependency closure on every read
- **Where:** `rust/crates/csharp-engine-services/src/content/portable.rs:198-205` (`Readout::build` calls `document.resolve(&asset.selected)`), `:358-362` (`read` calls `document.asset(..)` again)
- **What it does:** `read_portable_asset` re-walks the whole document's dependency graph (with cycle detection and per-asset validation in `asset_catalog::portable::resolve`) to produce the same closure `build` (`:55-57`) already resolved at load, then `expect`s it.
- **Why it looks pointless:** Re-validation of data validated at an earlier stage; the closure is immutable once loaded. Deleting the re-resolve and retaining `Vec<usize>`/ids of the closure at load changes nothing observable.
- **Cost:** Per read call, O(closure size) graph walk plus validation.
- **Confidence:** Medium
- **Removal shape:** Retain the resolved closure (or its ids) in `Asset` at load; build the readout from it.

### Duplicate material slot refused in collision/occlusion configuration
- **Where:** `rust/crates/csharp-engine-services/src/voxel.rs:950-964` (`configure_material_collision`: `declared` set exists only to refuse a repeat)
- **What it does:** Collects slots into `declared` and `excluded` sets and refuses the call with `"duplicate material slot"` if a slot repeats.
- **Why it looks pointless:** Duplicate guard where the `excluded` set insert already handles it. The sibling `configure_material_occlusion` (`:1004-1012`) does the same job without the guard.
- **Cost:** Per configure call, O(materials).
- **Confidence:** Medium
- **Removal shape:** Delete `declared`; keep `excluded.insert`.

### Whole admitted catalog / prefab registry cloned on every read
- **Where:** `rust/crates/csharp-engine-services/src/authored_content.rs:1078` (`self.catalogs.get(..)?.clone()` clones catalog + canonical JSON string + hash), `:731` (`prefab_registries.get(..)?.clone()`)
- **What it does:** `read_catalog` and `read_prefab_registry` clone the entire owner so they can call `&mut self` helpers while holding it, then copy every entry into readout rows.
- **Why it looks pointless:** Not a validation, but a whole-collection copy per call of the kind the brief's ResourceList example describes. The clone exists to satisfy the borrow checker (`catalog_result(&mut self, ..)` only needs `self.borrowed`).
- **Cost:** Per read call, O(entries + canonical JSON bytes).
- **Confidence:** Medium
- **Removal shape:** Take `&mut self.borrowed` and `&self.catalogs[..]` separately (split borrows) instead of cloning.

### Linear `find` per row when assembling catalog, prefab and scene documents
- **Where:** `rust/crates/csharp-engine-services/src/authored_content.rs:390` (dependencies), `:1826-1834` (`find_entry_mut`), `:1968-1977` (`find_prefab_definition_mut`), `:2419-2428` (`scene_node_mut`)
- **What it does:** Each payload/part/role/tag/instance/light row does a linear search over all entries/definitions/nodes to find its owner.
- **Why it looks pointless:** Not validation, but O(rows × owners) per admission where an index (`BTreeMap<id, index>` built once) would be O(rows log N). Reported because it is the same shape as the per-call whole-catalog cost the owner is hunting.
- **Cost:** Per admit/prepare call, quadratic in document size.
- **Confidence:** Low (correctness fine; only cost)
- **Removal shape:** Build an id→index map once per call.

### Product-supplied value range / finiteness refusals where a bad value would merely look odd
- **Where:** `rust/crates/csharp-engine-services/src/voxel.rs:169-174` (vertex occlusion strength must be finite in 0..=1), `:232-236` (stamp bounds finite, min<max; a degenerate box already yields `count == 0` at 250), `:1341-1350` (light sample offset/normal/horizon finite), `render_output.rs:172-181` (background channels finite in 0..=1, exposure finite ≥ 0), `:185-187` (pose time in [0,1]), `renderer_settings.rs:196-201` (`settings.valid()`: AO strength/radius finite, render scale 0.5..=1), `tween.rs:579-588` (elapsed seconds finite, ≥ 0), `gameplay_time.rs:15,119-126` (advance ≤ `MAX_GAMEPLAY_ADVANCE_SECONDS = 3600`), `voxel_content.rs:1005-1007` (`Transform::validate` on a presentation transform), `implicit_surfaces.rs:270-281` (material sample spacing finite/non-negative and must be paired with interpolated boundaries)
- **What it does:** Each refuses the call with an operation receipt when a float the product supplied is NaN/infinite or outside a chosen range.
- **Why it looks pointless:** Defensive bounds on trusted product values; in each case the Engine would draw/animate/sample oddly rather than panic (the 3600 s advance cap is a pure "sanity limit"). `render_output.rs:170-171` (width/height 0) and `gameplay_time.rs:105-111` (rate feeds an `expect`) are excluded from this finding because they prevent a concrete failure.
- **Cost:** Per call, O(1). Cost is negligible; the point is ceremony and the receipts/tests that prove each refusal (`gameplay_time.rs:258-263, 295-300`, `voxel.rs:1998-2030`, etc.).
- **Confidence:** Low
- **Removal shape:** Delete, or clamp silently where a range is a real representation limit (render scale).

### u64 handle counters refuse on `checked_add` overflow
- **Where:** `content.rs:134`, `content/bundles.rs:451-454`, `persistence.rs:64,70`, `authored_content.rs:334-336, 636-638, 1005-1007`, `voxel_content.rs:206, 221, 282, 394, 527-532, 584-589`, `rng.rs:24`, `video.rs:178-183`, `world_origin.rs:65-73`, `implicit_audit.rs:20-22`, `implicit_surfaces.rs:216-218, 245-248, 727-729`, `render_output.rs:146-148` (all under `rust/crates/csharp-engine-services/src/`)
- **What it does:** Every bridge that hands out monotonically increasing u64 handles wraps the increment in `checked_add` and surfaces an "exhausted"/"overflowed" error (sometimes a bare `return 0`, sometimes a receipt code such as `CSHARP_PERSISTENCE_HANDLES`, `HandleExhausted`, `CSHARP_VOXEL_CONTENT_SELECTION`).
- **Why it looks pointless:** Double-checking an invariant guaranteed by construction (2^64 increments at one per call). `ui.rs:136,265`, `session.rs:86` and `http.rs:58` already do a plain `+= 1`, so the crate is inconsistent about it. Each one carries an error code, message and sometimes a status enum variant that the product can never see.
- **Cost:** Per handle allocation, O(1). Cost is the dead error surface, not time.
- **Confidence:** Low
- **Removal shape:** `self.next += 1;` everywhere; delete the exhausted error codes/variants.

### Infallible or near-infallible narrowing conversions carry refusal paths
- **Where:** `voxel_scene_presentation.rs:1261-1266` and `kinematic.rs:222-224` (`u64::try_from(usize)` cannot fail on supported targets), `voxel_scene_presentation.rs:862-867, 1003-1008`, `voxel_content.rs:2467-2474` (`narrow`), `perception.rs:203-207` (`checked_count`), `authored_content.rs:1326` (`entry_count: u32::try_from(..).ok()?` turns a huge catalog into a silent `return 0`)
- **What it does:** Counts of retained objects/chunks/entries are narrowed to u32/u64 with error codes such as `CSHARP_VOXEL_CONTENT_LIMIT` / "exceeded the C# receipt range".
- **Why it looks pointless:** The usize→u64 ones are literally infallible; the usize→u32 ones would need >4 billion retained presentations/regions to trigger. ABI layout only requires the field type, not a refusal.
- **Cost:** O(1) per call.
- **Confidence:** Low
- **Removal shape:** `as u64` / `as u32` (or `debug_assert!`).

### Engine-produced SHA-256 hex re-parsed with "was not SHA-256" refusals
- **Where:** `rust/crates/csharp-engine-services/src/voxel_content.rs:2436-2458` (`hash`), used at `:1379-1380` (maps to `CanonicalObject` admission status), `:500-503` (four hashes per annotation edit receipt), `:1575-1578` (annotation query); `content/bundles.rs:683-689` + `:361-372` (`OpenBundle::container` fails the open with `PRODUCT_CONTAINER_CORRUPT "an entry has an invalid SHA-256"` after `Container::open` already validated every digest in `product-container/src/container.rs:223-228`)
- **What it does:** Converts hex digest strings the Engine itself produced (`with_computed_voxel_object_hashes`, `VoxelAnnotationEditReceipt`, the container inventory) back into word form, with error codes for a malformed string.
- **Why it looks pointless:** Re-verifying an identity the Engine computed. The comment at `voxel_content.rs:1377-1378` admits "a conversion failure means the admitted object is not in canonical form", i.e. an Engine bug. The container case is a second validation of the same strings `check()` just validated.
- **Cost:** Per call, O(1) per hash (4 per annotation edit); container case O(entries) per open.
- **Confidence:** Low (cheap; the fix is representational: have owners hand out `[u8; 32]`)
- **Removal shape:** Owners return `[u8; 32]`/`[u64; 4]`; bridge conversion becomes infallible; drop `CanonicalObject`-on-conversion and the container `ok_or_else`.

### Annotation tag count re-checked against the owner's limit at the bridge
- **Where:** `rust/crates/csharp-engine-services/src/voxel_content.rs:2227-2230`
- **What it does:** `borrowed_annotation_tags` refuses (bare `Err(())`, so a silent `return 0`) when `tags_len > VoxelAnnotationLimits::default().max_tags_per_region` before copying the tags.
- **Why it looks pointless:** The owner enforces the same limit (`voxel-annotation/src/codec.rs:372`) when the edit is applied, with a proper error; the bridge copy adds a second, worse-reported check. Copying 33 strings is not a cost worth guarding.
- **Cost:** Per tag edit, O(1).
- **Confidence:** Low
- **Removal shape:** Delete lines 2227-2230.

### Magica-admitted object re-canonicalised by a second admission pass
- **Where:** `rust/crates/csharp-engine-services/src/voxel_content.rs:1386-1390` (after `admit_magica_vox` at `:1375-1376`)
- **What it does:** `admit_magica_bytes` builds a `VoxelObjectAsset` through `with_computed_voxel_object_hashes` (which validates and hashes) and then calls `voxel_object_runtime::admit_voxel_object`, whose first step is `canonicalize_voxel_object` (re-validation) before building the runtime form; a failure is reported as `CanonicalObject`.
- **Why it looks pointless:** Validation of an object the Engine just built and canonicalised. The runtime builder (`AdmissionBuilder`) is needed; the second canonicalise/validate pass is not. (The `.vox` parsing itself in `magica_vox.rs` is import-time handling of a third-party file and is not a finding.)
- **Cost:** Per Magica load, O(voxels). Loads are infrequent.
- **Confidence:** Low (needs an owner-side entry point that skips canonicalisation; cost is modest)
- **Removal shape:** Add/ use an owner entry that admits an already-canonical object.

### Bundle collection identity recomputed on every open
- **Where:** `rust/crates/csharp-engine-services/src/content/bundles.rs:165-193` (`load` rebuilds the `Inventoried` map and hashes every path+digest via `collection_identity` `:417-428` on each `open_bundle`), `:361-372` (same for containers)
- **What it does:** Each `open_bundle` call re-derives the bundle's SHA-256-over-inventory identity and re-parses every file's hex digest.
- **Why it looks pointless:** The identity is a pure function of the admitted inventory and could be computed once in `checked`/`admit` (or lazily once per definition) and stored on `BundleDefinition`. Not a correctness issue; a per-call whole-collection recompute. C# reads it via `ProductContentBundle.Identity` on every open.
- **Cost:** Per open, O(files) hashing. Opens are infrequent.
- **Confidence:** Low
- **Removal shape:** Compute once at admission and store; `load` clones the Arc'd file map.

### Authored diagnostics truncated to a fixed 128
- **Where:** `rust/crates/csharp-engine-services/src/authored_content.rs:35, 2798, 2803`
- **What it does:** `receipt` keeps at most `MAX_DIAGNOSTICS = 128` validation diagnostics of a refused catalog/prefab admission and silently drops the rest.
- **Why it looks pointless:** A max-count limit with no concrete failure behind it; the receipt is borrowed and copied by the generated caller, so a long list costs nothing the product did not ask for, and the dropped diagnostics are exactly the ones a product author needs on a large broken catalog.
- **Cost:** Only on refusal.
- **Confidence:** Low
- **Removal shape:** Delete the `.take(MAX_DIAGNOSTICS)`.

### UI stream sequence must strictly advance; zero-handle special case; arena cycle detection
- **Where:** `rust/crates/csharp-engine-services/src/ui.rs:329-337` (sequence did not advance), `:284-289` (handle zero refused separately from unknown), `:498, 512-517` (`visiting` cycle detection over the SDK-built node arena)
- **What it does:** A projection whose product-assigned sequence is ≤ the last one is refused; a zero stream handle gets its own error before the map lookup; decoding the structured value tracks visited nodes to refuse a cycle.
- **Why it looks pointless:** The sequence rule is a monotonicity guard on a number the product owns (the dedupe at 343-350 already tolerates re-publishing the same value); the zero check is subsumed by the map miss; the arena is produced by the SDK serializer, which cannot emit a cycle. The cycle check does prevent unbounded recursion, so it is the only one with a failure behind it, and that failure is an SDK bug.
- **Cost:** Per publish, O(1) / O(nodes).
- **Confidence:** Low
- **Removal shape:** Delete 329-337 and 284-289; keep or `debug_assert` the cycle check.

### Session identity names restricted to 1-64 `[A-Za-z0-9_-]`
- **Where:** `rust/crates/csharp-engine-services/src/session.rs:61-71`
- **What it does:** Refuses a product-supplied identity name unless it is 1-64 ASCII alphanumerics/`-`/`_`, because it becomes `<root>/engine-session-identities/<name>.key`.
- **Why it looks pointless:** Filename sanitising of a product-built string (same category as the persistence key refusal). A product that passes an odd name gets an odd file; the length cap is arbitrary.
- **Cost:** Per host/join, O(len).
- **Confidence:** Low (a filesystem-portable name is a real representational need on Windows; the shape could be documented instead of refused)
- **Removal shape:** Delete, or keep only the empty-name check.

### `content/` prefix compatibility strip
- **Where:** `rust/crates/csharp-engine-services/src/content.rs:166-179` (`retained_path`, doc says "Compatibility lookup"), `video.rs:173`
- **What it does:** Accepts both `content/foo` and `foo` by stripping a `content/` prefix before catalog lookup.
- **Why it looks pointless:** Compatibility shim between Engine and product on the same build pair; one spelling should be canonical.
- **Cost:** O(1) per lookup.
- **Confidence:** Low
- **Removal shape:** Pick one spelling in the SDK, delete the strip.

---

## Looked at and judged legitimate

- `content.rs:19-35, 436-451` lazy once-per-body SHA-256 (`ContentIdentity`), and `resolve_reference` `:369-381` comparing the request's hash to the catalog/bundle identity: that comparison is the purpose of a persistable reference, hashing happens once and only on first identity request.
- `content/bundles.rs:290-335` read-once body cache and `Missing` vs other container errors; `BundleState::resolve` `:438-449` walking open bundles by inventory digest (no body hashing).
- `content/portable.rs` member resolution refusing a missing dependency file at load (`:69-78`): first load of a content file.
- `product_container::Container::open` / `check` (not in scope; first parse of a packed file).
- `persistence.rs:463-510` `read_blob` header magic / payload-length checks: first parse of an on-disk file that may be damaged; `:512-543` temp-file replace with pre-removal of a stale `.rusty-engine-pending`; `:449-461` revision guards (a product-selected optimistic-concurrency feature, `Any` opts out); `copy_blob` `:381-385` destination length (ABI).
- `http.rs`: no post-receipt re-checks in the bridge; URL/library-name rules live in `svc_http` (external input at receipt).
- `session.rs:117-119` relay-only needs a relay (config coherence); `:229` untaken-payload accounting is the owner's flow control.
- `ui.rs:454-500, 564-609` arena pointer/null/range checks and UTF-8 (ABI pointer/length coherence); `:42-69` image/font sniffing (needed to pick the served content type).
- `diagnostics.rs:56-113` UTF-8 of borrowed strings (ABI); `:20-45` `publish_recoverable_once` dedupe is a tiny once-per-code sink guard (callers not surveyed here).
- `gameplay_time.rs:99-116` rate range check: it guards the `expect` in `GameplayRate::from_parts_per_million`.
- `input.rs:68-83` mapping compile failure reported as `InvalidMappings` outcome: that is the compiler's result, not a guard.
- `host.rs:66-78` exit refused when the host has no window: host capability policy.
- `motion.rs`, `presentation.rs`, `operation_diagnostics.rs`, `rng.rs`: null-pointer / handle-miss checks only.
- `voxel.rs:71-72, 126, 129-130, 136, 237-238, 610-621, 718-722, 753-757, 764-767` u32→u16/u8 narrowing of ABI fields (representation); `:602-609, 737-749, 760-762, 770-781` offset/count spans inside the product's own arrays (prevent slice panics); `:1296-1300` states/slots length coherence; `:250-251, 379-383` `MAX_DENSITY_EDIT_VOXELS` (bounds a `Vec::with_capacity`, owner-defined); `:144-151` transition shorter than chunk (not verified whether `TerrainLayers`/`set_mesh_options` already enforces it; if so, it is a duplicate).
- `voxel_content.rs:1461-1472, 1501-1511` `decode_annotation_layer` + `validate_annotation_layer` on a content file at its first load; `:1306-1316, 1345-1355` Magica caps are product-chosen import options; `magica_vox.rs` as a whole is import-time parsing of a third-party `.vox` file (header, chunk, palette, duplicate-cell checks).
- `voxel_scene_presentation.rs:461-463` `scatter.validate()` (owner's own single-call validation; radius/fade/density feed the placement math); `:417-423` slot narrowing; `:945-953` variant must fit 13 bits (packed representation).
- `implicit_surfaces.rs:663, 1101` `catch_unwind` at the extern "C" edge; `:27-34, 487-503, 764-767` extraction/sample caps are product-chosen budgets with Engine defaults; `implicit_audit.rs:205-207` slice length vs `isize::MAX` (ABI).
- `perception.rs:50-60` stale continuation cursor when the projection identity changed (pagination correctness).
- `render_output.rs:170-171` zero capture dimensions (GPU texture creation would fail).
- `video.rs:17, 148-151` 128-entry realized-fact ring with eviction counts: bounds a queue the product may never drain and reports what it dropped.
- `renderer_settings.rs:93-101` per-setting refusal readouts: GPU/adapter capability policy.
- `tween.rs:594-599` handle-range check (`0 < value <= next_tween`): handle-table lookup shape.

## Coverage

- **Read fully:** `lib.rs`, `content.rs`, `content/bundles.rs`, `content/portable.rs`, `persistence.rs`, `http.rs`, `diagnostics.rs`, `gameplay_time.rs`, `host.rs`, `operation_diagnostics.rs`, `motion.rs`, `presentation.rs`, `input.rs`, `ui.rs`, `session.rs`.
- **Read in targeted regions (vocabulary grep plus the surrounding functions; roughly the stated share of non-test code):** `authored_content.rs` (~60%: bridge state, all admit/read/resolve/prepare paths, row builders, receipt), `voxel_content.rs` (~45%: bridge state, annotation edit/query, presentation materials/projection, Magica admission, helpers), `voxel.rs` (~35%: mesh-option configuration, stamps, density reads/edits, residency payloads, collision/occlusion, light sampling), `voxel_scene_presentation.rs` (~30%: scatter, clear, materials_for_scene, readout), `tween.rs` (refusal sites), `rng.rs`, `video.rs` (fact queue, play), `world_origin.rs`, `implicit_surfaces.rs` (state, limits, generation options), `implicit_audit.rs`, `render_output.rs` (request admission), `renderer_settings.rs`, `perception.rs`, `magica_vox.rs` (constants, error enum, admission entry).
- **Skimmed:** `kinematic.rs` (character-controller/collision integration; treated as another agent's area), `tween_tests.rs`, `implicit_surfaces_tests.rs` (test-only).
- **Skipped (excluded by assignment):** `appearance.rs`, `appearance/`, `render_resources.rs`, `composition.rs`, `camera_view.rs`, `audio.rs`, `spatial.rs`, `spatial/`, `dynamics.rs`, `dynamics/`.
- **Cross-checked owners (read-only, for context):** `asset-catalog/src/admission.rs`, `asset-catalog/src/portable.rs:175-260`, `product-container/src/container.rs:80-250`, `product-container/src/lib.rs:115-135`, `voxel-object-runtime/src/admission.rs:81-125`, `voxel-annotation/src/codec.rs:372`, `render-projection/src/voxel_scatter.rs:73-103`, `csharp/Rusty.Engine/ProductContentBundle.cs:26`.


---

# Appendix 4: 04-product-runtime-abi

## Survey 04: csharp-product-runtime, csharp-engine-abi, csharp-engine-test-host

Scope: `rust/crates/csharp-product-runtime/`, `rust/crates/csharp-engine-abi/`,
`rust/crates/csharp-engine-test-host/`. Read-only.

Headline: this region is already fairly clean. There is no assembly hashing,
no SDK/Engine version pair verification beyond the one ABI fingerprint
compare at bind, no export-table-vs-manifest check, no debug-catalog
validation beyond JSON decode, and no per-tick whole-state re-validation.
The `--exercise` fixture assertions are large but opt-in. What remains is
mostly once-per-load duplicate checks plus one runtime "recovery" mechanism
with no observed failure behind it.

Findings: 12 (High 2, Medium 4, Low 6).

---

### Pending native input queue bound + full input-lane resynchronization on overflow
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:62` (const),
  `lib.rs:2141-2171` (`append_pending_inputs`), `lib.rs:2114-2139`
  (`recover_pending_input_overflow`), `lib.rs:3220-3235` (the
  `pending_resynchronized` receipt in `input`), `lib.rs:3265-3269`
  (`recover_input_overflow`), tests `lib.rs:8891-8933` and `lib.rs:9261-9315`.
- **What it does:** `pending_inputs` (the events held until the next product
  update) is capped at `MAX_RUNTIME_INPUT_WIRE_EVENTS + 1` (1,025). When an
  append would exceed it, the runtime refuses the append, advances the
  control revision, rebinds the whole input lane (clearing all held keys and
  pending direct intents), publishes a `complete-baseline` recovery output,
  logs a `ResyncRequired` warning and answers the page with a
  `resync-required` receipt. The browser then has to re-fence its input.
- **Why it looks pointless:** "Retries, recovery, rollback ... with no
  observed failure behind them." The only thing the bound protects is memory
  in a `Vec` that is drained every update; the "sequence hole" the comment
  worries about is created *by* the bound (a partial append), not by anything
  upstream. `RuntimeInputLane::ingest_batch` already bounds one wire batch at
  1,024; two full batches arriving between two product updates (a slow
  update, a paused tab resuming) is a benign condition that this mechanism
  turns into a disruptive fence: every held key is dropped and the page must
  rebind. Deleting the check and letting the `Vec` grow until the next
  update consumes it has no failure mode a trusted page or product can
  trigger beyond a transient allocation.
- **Cost:** per `input()` call and per admitted step (cheap compare), but
  the recovery path when it fires is expensive (control revision change,
  lane rebind, UI projection snapshot, complete-baseline frontiers) and
  user-visible (lost held input).
- **Confidence:** High.
- **Removal shape:** delete `MAX_PENDING_NATIVE_INPUTS`,
  `PendingInputAdmission`, `recover_pending_input_overflow`, the overflow
  branch in `append_pending_inputs` (make it `self.pending_inputs.extend`),
  `ProductHostInputResult::pending_resynchronized` use, the
  `recover_input_overflow` trait method if nothing else calls it, and the two
  tests. `pending_recovery_outputs` may then only be fed by `execute_debug`
  failures; check whether it can go too.

### Content-root symlink/dir pre-check duplicates `ProductSource::open`
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:4427-4438`
  (`CsharpProductContent::admit`), test `lib.rs:7832-7861`
  (`content_admission_rejects_non_utf8_paths_and_symlink_roots`, second half).
- **What it does:** Before calling `ProductSource::open(root)`, takes
  `symlink_metadata` of the root and refuses a symlink or non-directory with
  `CSHARP_CONTENT_ROOT`.
- **Why it looks pointless:** Pure duplicate. `ProductSource::open`
  (`rust/crates/product-container/src/source.rs:24-41`) does
  `symlink_metadata` itself and returns `NotRegular` for anything that is
  not a plain dir or file; a symlinked root already fails there with
  `PRODUCT_SOURCE_NOT_REGULAR` (mapped to `CSHARP_CONTENT_ENTRY` by
  `content_error`). It is also a "symlink refusal on a path the SDK/`rusty dev`
  built itself". Deleting it changes only the error code.
- **Cost:** once per load (one extra `lstat`).
- **Confidence:** High.
- **Removal shape:** delete the pre-check; drop the symlink-root half of the
  test (or fold the expected code into the existing `CSHARP_CONTENT_ENTRY`
  case).

### Native artifact path: symlink refusal + canonicalize-inside-root on a manifest the SDK wrote
- **Where:** `rust/crates/csharp-product-runtime/src/product_bundle.rs:333-367`
  (`relative_path`, `resolve_regular_file`), test `product_bundle.rs:1084-1090`
  (`reports_the_exact_manifest_path_field_that_escapes`).
- **What it does:** For `nativeAot.module` / `coreclr.assembly` /
  `coreclr.runtimeconfig`, refuses an absolute or `..`-containing path,
  refuses a symlink or non-file, then `canonicalize`s and refuses if the
  result is not under the Product root ("resolved outside Product root").
- **Why it looks pointless:** "Path traversal / absolute-path / symlink
  refusals on paths the product or SDK built itself." `product.json` is
  emitted by `rusty build` / the SDK; the only reader is this host, and the
  artifact is then handed to `libloading`/hostfxr by path. A product whose
  manifest points at a symlinked `.so` (common in dev: a built artifact
  linked into the staged bundle) is refused for no concrete failure. The
  `.is_file()` check in `coreclr_path` (`lib.rs:664-683`) and `Library::new`
  already give a clear error for a missing or unloadable file.
- **Cost:** once per launch; three `lstat` + `realpath`.
- **Confidence:** Medium. The "must be a regular file" part is arguably a
  first-parse check; the symlink and escape refusals are the pointless part.
- **Removal shape:** reduce to `root.join(value)` and let the loader fail on
  a missing file; drop the escape test.

### Four-way ABI handshake compare where the fingerprint alone is the identity
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:599-620`
  (`product_from_handshake`), `lib.rs:585-597` (`host_abi_handshake`),
  `lib.rs:622-662` (mismatch text, `abi_identity_text` 128-byte bound at
  `lib.rs:77,655`), ABI decl
  `rust/crates/csharp-engine-abi/src/product.rs:2082-2113`
  (`NativeProductAbiHandshakeV1`: `protocol_version`, `engine_api_size`,
  `product_api_size`, `build_identity`), test `lib.rs:7600-7656`.
- **What it does:** At bind the host compares `protocol_version`,
  `engine_api_size`, `product_api_size` and the 256-bit `PRODUCT_ABI_FINGERPRINT`
  (SHA-256 of the generated ABI model) against the product's, and formats a
  bounded `build_identity` string into the error.
- **Why it looks pointless:** "Compatibility/version negotiation ... protocol
  version checks between Engine and product." The fingerprint *is* an
  ABI-layout check and is legitimate (a product DLL built against another
  pair would otherwise call function pointers with the wrong struct layout:
  UB, not an odd render). But the generated fingerprint already covers the
  protocol version and both table sizes, so the three extra fields, the
  `PRODUCT_ABI_PROTOCOL_VERSION` constant, the `build_identity` slice with its
  128-byte sanity bound and the "wrote a sentinel?" test are redundant
  ceremony around one `u64x4 ==`.
- **Cost:** once per load; negligible.
- **Confidence:** Low. Keep the fingerprint. Only the redundant fields and
  their diagnostic plumbing are candidates, and they are cheap; this is a
  surface-area simplification, not a performance fix.
- **Removal shape:** collapse the handshake struct to `fingerprint` +
  `product_api` (and keep the `is_null` check); drop
  `version_mismatch_bind`/`table_size_mismatch_bind` test cases.

### `required_function` on every `Option<fn>` in the product table after an exact-fingerprint match
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:528-558`
  (`from_bound_product`), `lib.rs:686-693` (`required_function`), ABI decl
  `rust/crates/csharp-engine-abi/src/product.rs:2021-2069` (`NativeProductApi`
  with 16 `Option<unsafe extern "C" fn>` fields and the comment "Fields stay
  nullable only so the host reports a null as an error rather than calling
  it").
- **What it does:** Each of the 16 product callbacks is declared nullable and
  individually checked for `None` at bind; the ABI doc itself says the
  fingerprint "rules out an older, partial table".
- **Why it looks pointless:** "Double-checking invariants already enforced by
  construction." The generated `ProductBridge` fills every field, and the
  fingerprint has just been matched. Making the fields non-optional `fn`
  pointers removes 16 checks and the `Option` dance in every call site
  (`self.api.start` etc.) at no ABI cost (same layout).
- **Cost:** once per load; negligible.
- **Confidence:** Low. Harmless and tiny; a null pointer here would be UB,
  which is the one thing the brief says a cheap check may guard. Report so it
  is not re-surveyed.
- **Removal shape:** make `NativeProductApi` fields non-`Option` and delete
  `required_function`; or leave as is.

### Create "succeeded but wrote a diagnostic" refusal and 0/1 flag range checks on generated callbacks
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:4061-4068`
  (`call_create`: ABI_OK + non-empty `NativeProductCallError` → error
  `CSHARP_PRODUCT_CALL`), `lib.rs:4262-4271` (`copy_debug_result`:
  `succeeded` must be 0 or 1), `lib.rs:4306-4315` (`call_complete_timeline`:
  `accepted` must be 0 or 1).
- **What it does:** Treats a successful `create` that also left a non-empty
  call-error struct as a load failure; treats any `u8` other than 0/1 from
  the generated debug/timeline callbacks as an ABI error.
- **Why it looks pointless:** The generated C# bridge (SDK-owned) writes
  these; they are not product-authored values. "Defensive ... checks on
  values ... where the value would merely render oddly": a `2` in
  `succeeded` is `!= 0`, and a stray diagnostic after a successful create is
  at most a log line. None of these prevents a panic or UB.
- **Cost:** once per load (create), per debug command, per timeline
  completion. Negligible.
- **Confidence:** Low.
- **Removal shape:** `succeeded != 0` / `accepted != 0`; in `call_create`,
  ignore (or log) the diagnostic when status is OK.

### `prepare_persistence_root` re-checks `is_dir` after a successful `create_dir_all`
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:3929-3957`,
  specifically `lib.rs:3950-3955`; test `lib.rs:9356`.
- **What it does:** Requires an absolute path, `create_dir_all`s it, then
  errors if `!root.is_dir()`.
- **Why it looks pointless:** `create_dir_all` returns `Err` if the path
  exists as a non-directory, so the post-check cannot fire except in a race
  with another process. "Double-checking invariants already enforced by
  construction."
- **Cost:** once per load; one `stat`.
- **Confidence:** Medium.
- **Removal shape:** delete the `is_dir` block. (The absolute-path
  requirement is policy for a host-supplied root; leave it.)

### Manifest cosmetic bounds: title 1..=160 chars, `product.id` identity grammar, unused `ui.assets` validation, input mappings compiled twice
- **Where:** `rust/crates/csharp-product-runtime/src/product_bundle.rs:85-93`
  (`validate_runtime_identity(product.id)`, title length),
  `product_bundle.rs:123-126` (comment: "Validate the declared assets path
  even though the complete UI root is staged"), `product_bundle.rs:324-325`
  (`CompiledInputMappings::standard(...)` result discarded) vs
  `lib.rs:1044-1048` (compiled again for real), `product_bundle.rs:511-521`
  (`uiProjection.expectedStream/expectedContract` identity grammar).
- **What it does:** Refuses a title over 160 bytes; requires `product.id`
  and the uiProjection names to match `validate_runtime_identity`; checks
  that `ui.assets` names a directory although nothing reads it afterwards
  (the whole UI root is staged); compiles the input mapping set once for
  validation and throws it away, then the runtime compiles the same inputs
  again.
- **Why it looks pointless:** "max-string-length limits with no concrete
  failure behind them" (title is only shown in a window title and a
  snapshot); `ui.assets` is a validation of a value that is never used; the
  double compile is "re-validation of data already validated at an earlier
  stage". `product.id` grammar is used for `RuntimeLifecycle`/stream
  identities so it may be a real representation requirement.
- **Cost:** once per launch.
- **Confidence:** Medium for the title bound, the `ui.assets` check and the
  duplicate compile; Low for the identity grammar.
- **Removal shape:** delete the title length clause; delete the `ui.assets`
  line (and the field, if the manifest schema can drop it); drop the compile
  in `input()` and let `load_admitted_with` report the mapping error.

### Harness claim label and lease sanity bounds
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:798`
  (`MAX_INPUT_CLAIM_LEASE`), `lib.rs:3132-3143` (`claim_control`: label
  1..=64 bytes, no control chars; lease 1 ms..=1 h).
- **What it does:** Refuses a harness input claim whose label is empty, over
  64 bytes or contains control characters, or whose lease is zero or over an
  hour.
- **Why it looks pointless:** "max-string-length limits ... with no concrete
  failure." The label is echoed in the binding publication only; the lease
  is compared against `Instant::elapsed`. A zero lease would expire at the
  next observation, which is a usable semantic, and a 2 h lease is a harness
  preference. The caller is the Engine-owned dev tooling.
- **Cost:** per claim (rare).
- **Confidence:** Low. Cheap, product-invisible; only worth removing while
  touching the claim code.
- **Removal shape:** delete both `if` blocks and the const.

### Performance-probe and physical-mapping CLI count limits
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:1205-1210`
  (`performance_probe` iterations 1..=256), `main.rs:1058-1060` (same bound
  at parse), `main.rs:51-52,1035-1039,1372-1379` (`MAX_PHYSICAL_MAPPINGS`
  256, `MAX_MAPPING_CHORD_CONTROLS` 8, chord uniqueness), tests
  `main.rs:1849-1870` (`parser_enforces_mapping_and_chord_declaration_bounds`).
- **What it does:** Caps CLI-declared physical mappings at 256 and chord
  members at 8 (and requires unique chord members), caps probe iterations at
  256, each with a dedicated error and a test.
- **Why it looks pointless:** "max-count limits with no concrete failure";
  these are developer-typed CLI arguments on the legacy launch path
  (`--product` launches take mappings from the manifest, which has no such
  caps). `RuntimeInputMapping::new` / `CompiledInputMappings::standard`
  already refuse a malformed or duplicate mapping; a duplicate chord key is
  harmless. The probe cap exists only so a typo does not run long.
- **Cost:** once per launch.
- **Confidence:** Low. Harmless, but the test exists only to prove the caps.
- **Removal shape:** delete the two consts, the three `if`s and the bounds
  test; keep `RuntimeInputMapping::new` as the only validator.

### Headless Chromium 150 ms startup probe
- **Where:** `rust/crates/csharp-product-runtime/src/headless_browser.rs:84-109`.
- **What it does:** After spawning Chromium, sleeps 150 ms and `try_wait`s
  to "catch invalid browser flags, unusable profiles, and immediate process
  startup failures".
- **Why it looks pointless:** A fixed sleep is not a check; a browser that
  exits at 200 ms is reported the same way by the supervisor's ordinary
  `exited()` polling, and one that fails at 100 ms is reported here with the
  same status. It adds 150 ms to every `--headless` start for no distinct
  outcome.
- **Cost:** once per headless launch (150 ms wall).
- **Confidence:** Low. Not a validation in the brief's sense; listed because
  it is a "recovery with no observed failure" shaped delay.
- **Removal shape:** delete the sleep and the `try_wait` block; let the
  supervisor loop report an early exit.

### Tests whose sole purpose is to prove the above rejections
- **Where:** `rust/crates/csharp-product-runtime/src/lib.rs:7645-7656`
  (version/table-size mismatch cases only; keep the fingerprint case),
  `lib.rs:7832-7861` (symlink-root half), `lib.rs:8891-8933`,
  `lib.rs:9261-9315`, `product_bundle.rs:1084-1090`, `main.rs:1849-1870`.
- **What it does:** Each exists to prove a refusal listed above.
- **Why it looks pointless:** "Tests whose sole purpose is to prove one of
  the above rejections."
- **Cost:** test time only.
- **Confidence:** follows the finding each belongs to.
- **Removal shape:** delete with the mechanism.

---

## Looked at and judged legitimate

- Fingerprint equality at bind (`lib.rs:606`) and in the test host
  (`csharp-engine-test-host/src/lib.rs:122-127`, test `:469-487`): a
  product DLL from another pair would call the table with the wrong layout
  (UB). This is the one ABI-layout check the brief allows; `rusty dev` pairs
  are matched by tooling, not by the host, so the host must notice a stale
  pair somehow. Only the redundant siblings are flagged above.
- `product.product_api.is_null()` after handshake (`lib.rs:609-614`),
  `handle.is_null()` after create (`lib.rs:1127-1132`): pointer coherence.
- `copy_product_error_text` / `copy_debug_result` "length without bytes" and
  UTF-8 checks (`lib.rs:4177-4195`, `4272-4292`): pointer/length coherence
  and UTF-8 of a borrowed string at the boundary. The former 64 KiB debug
  result bound is already gone (test at `lib.rs:7550` proves a 100 KiB result
  is accepted).
- `read_product_call_error` / release pairing and the unconditional
  `release_*` calls (`lib.rs:4121-4142`, `4197-4257`): exact release of owned
  resources.
- `require_current_control_binding`, `require_control_binding`,
  `require_restart_state`, `complete_timeline` binding compare
  (`lib.rs:2403-2476`, `3498-3520`), stale-input drops via
  `RuntimeInputLane::ingest_batch` (`lib.rs:3179-3203`): this is the
  host-to-page lifecycle fence (instance/generation/control revision). Pages
  and harnesses are concurrent and asynchronous; a stale page's input or a
  late timeline completion after restart is a real, observed condition that
  these answer with a typed receipt instead of applying it. Not a
  product-trust check and not whole-state; per-call O(1).
- `input()` refusing input in Created/Faulted/Shutdown (`lib.rs:3170-3176`)
  and `connect()` refusing a Shutdown runtime (`lib.rs:2957-2962`):
  lifecycle state machine, not validation.
- Rebuilding a complete baseline on Start/Restart/Connect/ReportFault/
  Shutdown/fault (`rebind_outputs`, `fault_after_call`,
  `complete_baseline_output`; `lib.rs:2170-2200`, `2487-2537`): this is the
  Engine publishing retained state to a fresh or re-fenced page, once per
  lifecycle event, not per frame. Shutdown producing a baseline is odd but
  harmless.
- `rebaseline_frames` after a failed call (`lib.rs:2772-2797`): rebuilds the
  renderer from the committed world once per product fault; the comment
  gives the concrete reason (a failed call may have lost renderer work).
- `MAX_FACTS_PER_REPORT` chunking (`frame_output.rs:62`, `audio_output.rs:37`):
  a batch size for the feedback ingest, not a refusal.
- `is_finite` on manifest `presentationAspect`, AO strength/radius, render
  scale (`product_bundle.rs:140-150`, `671-682`, `729-736`): first parse of
  the manifest, and a NaN aspect or radius would reach the renderer. Also
  `rusty-scene-render` CLI parsing (`rusty-scene-render.rs:123,237`).
- Manifest enum-string parses (`renderer.output`, `audio.output`, lights,
  AO mode, antialiasing, vsync, fog, clouds, cursor mode), `artifact` and
  `schemaVersion` tags (`product_bundle.rs:76-84`, `main.rs:769-775`
  `StagedLaunchInput`, `video_options.rs:207-212` store version): parse
  errors for a malformed file at first parse.
- `product_directory` / `product_file` going through `ProductSource::is_dir`/
  `is_file` (`product_bundle.rs:370-402`): first parse; they do not duplicate
  anything.
- `fault_line` 800-char truncation (`lib.rs:4380-4391`): output formatting
  for stderr, with a test proving the full detail still reaches the log.
- `update_facts` `u128 -> u64` dropped-step conversion (`lib.rs:3880`):
  a wire-range conversion with an error instead of a silent wrap.
- The `--exercise` fixture assertions (`exercise_updates` and the
  `exercise_*` helpers, `lib.rs:1098-1200`, `1230-1680`,
  `assert_ui_projection_binding`, `complete_voxel_baseline`
  `lib.rs:5175-5262`): ~600 lines of runtime self-checks, but they run only
  under an explicit `--exercise` flag, which the help text already describes
  as a provider-fixture gate rather than a product health check. Not a
  hot-path or load-path cost. (If the owner wants to shrink the crate, this
  is the largest removable block, but it is not validation in the brief's
  sense.)
- `playtest.rs:34-37` advance bound `(0, 2000] ms`: a debug-command argument
  that becomes a loop count; the upper bound keeps a typo from admitting
  thousands of steps in one HTTP call.
- `main.rs:613` 16 KiB header cap in the probe's hand-rolled HTTP reader and
  `supervisor.rs:683` the same in the 503 responder: bounds on a socket
  read, external input.
- `supervisor.rs` one automatic restart, 30 s startup / 10 s shutdown
  timeouts: supervisor policy for a child process with concrete failure modes
  (hung CoreCLR startup, hung disposal), and `--debugger` lifts the startup
  one.
- `supervisor.rs:231-239` `productDirectory must be absolute`: the
  supervisor's cwd and the runtime child's cwd differ; a relative path would
  resolve wrongly.
- `headless_browser.rs` `--chromium` `is_file` check, profile dir
  allocation retry (16 attempts on `AlreadyExists`): file-system races, not
  validation.
- `render_output.rs`, `scene_snapshot.rs`, `video_options.rs`,
  `desktop.rs`, `rusty-live-debug.rs`: no validation, hashing or duplicate
  guards found. `video_options.rs:88-90` `valid()` on a chosen settings
  combination is a renderer-setting domain check (e.g. sample count).
- `audio_output.rs:237` `retain_clips(|hash| audio_clip_bytes(hash).is_some())`
  per update: this is a release of decoded clips the Engine dropped, keyed by
  a map lookup per retained clip, not a scan of the Engine's catalog.
- ABI crate (`csharp-engine-abi`): declarations only (`#[repr(C)]` structs,
  enums, fn pointer types, `Default` impls). No functions, no validation
  code. Doc comments mention validation that lives in
  `csharp-engine-services` (outside this assignment):
  `voxel_content.rs:274-287,323-370` expected-layer-hash "stale-hash
  rejection" on annotation edits/queries, `appearance.rs:777-780` "Rust
  verifies [the texture] against the resolved authored texture hash",
  `audio.rs:161-163` audio `signal_id` idempotency rejection,
  `presentation.rs:349-352` duplicate signal identities, `content.rs:38-41`
  `NativeContentResolveRequest` path + sha256 must both match,
  `authored_content.rs:162-163` "hash-pinned misses remain strict operation
  errors". These ABI fields carry expected hashes for the services to
  compare; whether those comparisons are warranted is a question for the
  csharp-engine-services survey, flagged here as cross-references only.
- `csharp-engine-services` bundle "file first read after a restage must
  still have its inventoried length" (`PRODUCT_BUNDLE_FILE_CHANGED`,
  referenced from `lib.rs:2914-2920` doc and the test-host test
  `csharp-engine-test-host/src/lib.rs:421-467`): a length compare on first
  read of a bundle file, not a SHA-256 recompute; owned by
  `csharp-engine-services/src/content/bundles.rs:321`. Cross-reference only.
- Test host (`csharp-engine-test-host/src/lib.rs`): null-pointer and
  length-without-storage checks (`:140-146`, `:183-192`), UTF-8 of borrowed
  strings (`:175-181`): ABI coherence.

## Coverage

Read fully:
- `rust/crates/csharp-product-runtime/src/lib.rs` lines 1-5268 (all non-test
  code); test module 5268-9398 skimmed by test name, with these read in full:
  7550-7656 (handshake tests), 7718-7861 (content admission tests),
  7298-7362 (duplicate input), 8891-8933 (input pressure), 9261-9315
  (overflow recovery).
- `rust/crates/csharp-product-runtime/src/main.rs` (all; tests by name plus
  1805-1870).
- `rust/crates/csharp-product-runtime/src/product_bundle.rs` lines 1-855
  (all non-test code); tests by name.
- `rust/crates/csharp-product-runtime/src/supervisor.rs` (all, incl. tests).
- `rust/crates/csharp-product-runtime/src/headless_browser.rs` (all).
- `rust/crates/csharp-product-runtime/src/playtest.rs` (all).
- `rust/crates/csharp-product-runtime/src/render_output.rs` (all).
- `rust/crates/csharp-product-runtime/src/desktop.rs` (all).
- `rust/crates/csharp-product-runtime/src/frame_output.rs` lines 1-1000
  (all non-test code except the trailing `ghost_plate_fact` helper and
  tests, skimmed).
- `rust/crates/csharp-product-runtime/src/audio_output.rs` lines 1-420
  (all non-test code); tests skipped.
- `rust/crates/csharp-product-runtime/src/scene_snapshot.rs` lines 1-420
  (all non-test code); the `SceneSnapshot::open` tail and tests skimmed.
- `rust/crates/csharp-product-runtime/src/video_options.rs` lines 1-260
  (all non-test code); tests skipped.
- `rust/crates/csharp-product-runtime/src/bin/rusty-live-debug.rs` lines
  1-140, 360-481 (middle transport code skimmed).
- `rust/crates/csharp-product-runtime/src/bin/rusty-scene-render.rs` lines
  1-340 (parse + setup); gallery/report tail skimmed via grep.
- `rust/crates/csharp-product-runtime/build.rs`, `Cargo.toml`.
- `rust/crates/csharp-engine-test-host/src/lib.rs` (all, incl. tests),
  `Cargo.toml`.
- `rust/crates/csharp-engine-abi/src/lib.rs`, `generated_abi_identity.rs`,
  `host.rs`, `test_host.rs`, `product.rs` lines 2000-2158 (handshake and
  product table), `authored_content.rs` 1-300, `voxel_content.rs` 120-420,
  and the doc passages of `content.rs`, `appearance.rs`, `animation.rs`,
  `audio.rs`, `presentation.rs`, `persistence.rs` that grep flagged.

Skimmed by grep only (declaration-only files, no code paths):
- `rust/crates/csharp-engine-abi/src/{animation,appearance,audio,camera_view,
  content,core,diagnostics,dynamics,http,implicit_surfaces,input,kinematic,
  lifecycle,motion,perception,persistence,portable_content,presentation,
  render_output,renderer_settings,rng,session,spatial,tween,ui,video,voxel,
  voxel_content,voxel_scene_presentation,world_origin}.rs` beyond the ranges
  above, and `cbindgen.toml`. Confirmed with `grep -n '^\s*\(pub \)\?fn '`
  that the crate contains only `Default` impls and no validation logic.

Skipped:
- `rust/crates/csharp-product-runtime/src/lib.rs` test fixtures 5269-7297,
  7363-7549, 7657-7717, 7862-8890, 8934-9260, 9316-9398 (read names only).
- `product-container/src/source.rs` 15-150 was read to confirm what
  `ProductSource::open` and `inside()` already guarantee (outside the
  assignment; not surveyed for findings).


---

# Appendix 5: 05-host-cli-runtime

## Survey 05: product-host, product-container, desktop-shell, rusty-cli, runtime-*, svc-http, svc-session

All paths are repo-relative under `rust/crates/`. Line numbers are from the current tree (HEAD, after 2ee8375).

Summary: 36 findings (12 High, 14 Medium, 10 Low). The hottest are in `product-host` (per-input-POST and per-tick re-validation of data the host or runtime itself just produced) and `runtime-input` (a per-event clone of the whole mapping table and a per-batch full-state checkpoint). Two "sanity limits" cause concrete harm today (a 4 KB diagnostics line cap that permanently kills the log writer, and a 4096-px frame-side cap that refuses 5K/8K viewers).

---

## High confidence

### Input batch is decoded, re-encoded and strictly decoded again on every input POST
- **Where:** `product-host/src/host.rs:1741-1758`, `product-host/src/host.rs:4409-4412` (`ProductHostInputRequest { batch: Vec<Value> }`), `product-host/src/model.rs:937-954`
- **What it does:** `invoke_input` deserializes the request body into `Vec<serde_json::Value>` (via `decode_json`, which also re-checks the body size), then `serde_json::to_vec(&request.batch)` re-serializes those values to bytes, then `ProductHostInputBatch::decode_json` checks the size a third time and runs `runtime_input::decode_runtime_input_wire_events_json` (the strict typed decode). The strict decode also stores a copy of the re-encoded bytes (`wire_json: Some(bytes.to_vec())`).
- **Why it looks pointless:** Schema re-validation of the same bytes in three passes. The typed `RuntimeInputWireEvent` decoder could deserialize `batch` directly (`#[serde(deny_unknown_fields)]` wrapper with `batch: Vec<RuntimeInputWireEvent>`), making the `Value` round-trip and the second size check disappear. The "could not be encoded" error branch at 1747-1753 is unreachable for a `Value`.
- **Cost:** Per input POST (every page frame that carries input, ~60 Hz under play): one generic JSON tree allocation, one serialization, one more parse, plus an extra `Vec<u8>` copy retained in the batch.
- **Confidence:** High
- **Removal shape:** Deserialize the typed wire events once from the request body; drop `ProductHostInputBatch::decode_json`'s size check and the retained `wire_json` copy if no worker path still needs it.

### Output groups are validated three times per receipt (publication frontiers, output group, SSE batch encoding)
- **Where:** `product-host/src/model.rs:1719-1726` (`ProductHostRuntimeReceipt::new` runs `output.validate()` on every publication), `product-host/src/model.rs:1738-1746` + `1635-1652` + `1696-1716` (`into_wire_parts` → `validate_output_group` → `complete_baseline_end`), `product-host/src/host.rs:2772-2836` (`encode_output_batches` re-checks the same binding/complete-baseline pairing and errors `PRODUCT_HOST_OUTPUT_BASELINE`), `product-host/src/publication.rs:134-152`, `184-200`
- **What it does:** For every runtime receipt (each scheduled tick, each input batch, each lifecycle/control/debug call) the host scans the publication list for frontier grammar/duplicates, then scans the converted outputs to pair `Binding` markers with `CompleteBaseline` markers, then in `encode_output_batches` walks the same outputs again doing the same pairing with the same error strings. `validate_output_group`'s return value (the whole-baseline binding) is discarded by `into_wire_parts`.
- **Why it looks pointless:** The outputs are produced by `csharp-product-runtime` in the same process a few calls earlier; there is no external producer. The marker pairing is an invariant of the runtime's own publication code, and `encode_output_batches` already enforces it once at the only place that needs it (to split SSE batches). The frontier re-validation (`validate_frontiers`) re-checks what `RuntimePublicationFrontier::new` already enforced at construction (same length/empty/JSON-safe checks at `publication.rs:55-64`) and adds a `BTreeSet` duplicate scan.
- **Cost:** Per receipt, i.e. per 60 Hz tick plus per input POST; each pass is O(outputs), plus a `BTreeSet` allocation per binding/baseline publication.
- **Confidence:** High
- **Removal shape:** Delete `RuntimePublication::validate`/`validate_frontiers` and `validate_output_group`/`complete_baseline_end` (and `publication_error`); keep only the pairing that `encode_output_batches` needs to form batches, demoted to `debug_assert!` where it is a pure self-check.

### Every physical input fact clones the whole compiled mapping table
- **Where:** `runtime-input/src/lane.rs:677-683` (`let mappings = self.mappings.mappings().to_vec();`)
- **What it does:** `queue_physical_mappings` is called for every ingested physical fact that is not a `Clear` (`lane.rs:533-535`): every key edge, pointer delta, wheel tick, controller axis sample. It deep-clones the entire `Vec<CompiledInputMapping>` (each with two `String`s, a `Vec<KeyboardControl>` chord and an `Option<InputContext>` string) to satisfy the borrow checker before iterating, then for each matching mapping clones the intent descriptor again (`descriptor.clone()` at 702, which carries a `serde_json::Value` payload).
- **Why it looks pointless:** Whole-collection copy per call where only a read is needed. Mouse motion produces a `PointerDelta` event per `pointermove`, so this allocates O(mappings × strings) per mouse move.
- **Cost:** Per input event; O(mappings) allocations each.
- **Confidence:** High
- **Removal shape:** Split the borrow (take `self.mapping_active` out with `std::mem::take`, iterate `&self.mappings.mappings()`, write back) or store descriptors behind `Arc`; no clone.

### A 4 KB encoded-line cap discards the diagnostic and permanently fails the log writer
- **Where:** `product-host/src/log.rs:22` (`MAX_LINE_BYTES = 4 * 1024`), `product-host/src/log.rs:304-306`, `product-host/src/log.rs:226-231` (any `Err` from `write_event` flips `HostFileWriterState::Ready` to `Failed`)
- **What it does:** `write_event` encodes the event; if the NDJSON line exceeds 4096 bytes it returns `Err("encoded event exceeded fixed line bound")`. `write()` treats any error as a writer failure and sets the state to `Failed`, after which every later event returns "diagnostic writer is unavailable" and is counted as dropped.
- **Why it looks pointless:** Magnitude cap with no concrete failure behind it, and it contradicts `error.rs:72` ("Keeps the complete diagnostic, including multiline managed stack traces"): one C# exception with a stack trace over ~4 KB ends file logging for the rest of the session. The ring and the SSE path keep the full event; only the durable file silently stops.
- **Cost:** Per published event (an encode + length compare), but the harm is the permanent writer failure.
- **Confidence:** High
- **Removal shape:** Delete the line bound (rotation already bounds the file); at minimum make an oversize line a per-event drop rather than a writer failure.

### Every UI projection envelope walks its entire JSON value to reject integers above 2^53
- **Where:** `runtime-ui/src/model.rs:86-88`, `194-223` (`validate_value`/`validate_number`), `runtime-input/src/model.rs:392`, `794-826` (`validate_product_payload_numbers` for direct-intent payloads)
- **What it does:** Constructing a `RuntimeUiProjectionEnvelope` (every product UI projection, published per tick when the HUD changes) pushes every array element and object value of the product's JSON onto a stack and checks each number against `9_007_199_254_740_991`. The same walk runs on every direct-intent `ProductPayload` claim from the page.
- **Why it looks pointless:** Defensive range check on a value the trusted product supplies; the consequence of an unsafe integer is that JavaScript shows a rounded number ("odd render"), not a Rust panic. The product chooses its own DTOs; a product that wants a u64 sends it as a string (as the Engine itself does with `CanonicalU64`). The test `large_and_deep_values_are_admitted_without_recursive_validation` exists only to keep this walk from blowing the stack.
- **Cost:** Per envelope, O(size of projection JSON) — the whole value, every publication.
- **Confidence:** High
- **Removal shape:** Delete `validate_value`/`validate_number` and `validate_product_payload_numbers` (and `ValueUnsafeInteger`, `ProductPayloadUnsafeInteger`, their tests).

### Browser-diagnostics reports from the Engine's own shell page are bounds-checked and code-allowlisted
- **Where:** `product-host/src/model.rs:245-320` (`validate`, `MAX_PAGE_EVENTS = 8`, `validate_browser_diagnostic`, recoverable-code allowlist at 260-268), `product-host/src/host.rs:1958-1960`
- **What it does:** `POST browser-diagnostics` is rejected with 400 if it carries more than 8 page events, a code over 128 bytes or outside `[A-Za-z0-9._:-]`, a message over 1024 bytes or containing any control character other than tab, an attachment id outside `[A-Za-z0-9_-]{1,128}`, or a `recoverableEvent.code` that is not exactly `CSHARP_LIFECYCLE_CLOCK_REGRESSION` or `BROWSER_LOCAL_REQUEST_UNAVAILABLE`.
- **Why it looks pointless:** The sender is `render/packages/product-browser-host` from the same pair; this is hostile-input ceremony between two halves of one build. The allowlist is a maintenance trap: a new recoverable code in the shell is silently refused by the host until the Rust list is edited. A page error message containing a newline (common in browser `Error.stack`) is refused outright instead of logged.
- **Cost:** Per report (occasional, not per frame), but every string is scanned.
- **Confidence:** High
- **Removal shape:** Delete `validate`, `validate_browser_diagnostic`, `validate_browser_attachment_id` and the allowlist; rely on serde `deny_unknown_fields` typing. Remove the `invalid id` half of test `browser_attachment_report_uses_bounded_typed_baseline_facts` (model.rs:1950-1957).

### The product's generated debug catalog is re-validated against fixed command/parameter/text caps
- **Where:** `product-host/src/model.rs:624-627`, `716-760` (`decode_json` → `validate`, `MAX_COMMANDS = 256`, `MAX_PARAMETERS_PER_COMMAND = 16`, `MAX_TEXT_BYTES = 1024`, `available` must be true, no NUL), consumed at `csharp-product-runtime/src/lib.rs:3355`
- **What it does:** The catalog JSON the product's generated `describe_debug` export returns is parsed and then rejected (500 to the live-debug panel) if it names more than 256 commands, a command has more than 16 parameters, any name/description/type string exceeds 1024 bytes, or `available` is false.
- **Why it looks pointless:** The catalog is emitted by `Rusty.Engine.ProductGenerator` from the trusted product's own debug modules. A product with 257 commands or a long description simply loses its catalog. Nothing downstream depends on these bounds (the panel renders a list).
- **Cost:** Per `debug/catalog` request, O(commands × parameters); not hot, but the failure mode is a hard refusal.
- **Confidence:** High
- **Removal shape:** Delete `validate`/`validate_debug_descriptor_text` and the three `MAX_*` constants; `decode_json` becomes plain `serde_json::from_slice`.

### Input/operation/timeline results re-check the runtime's own receipt arithmetic and binding/readout agreement
- **Where:** `product-host/src/model.rs:1186-1206` (`with_progress`: count ≤ MAX, accepted+dropped == count, accepted_through presence rules, readout binding), `1003-1014` (`accepted`), `1035-1040` (`queued`), `1063-1068` (`mailbox_full`), `818-823`, `886-891`, `1395-1400`, `1436-1441` (`readout.runtime() != binding` in five more constructors)
- **What it does:** Every constructor of a typed result refuses with `PRODUCT_HOST_INPUT_RESULT_BOUNDS` / `PRODUCT_HOST_RESULT_BINDING` if the counts the runtime just computed from `RuntimeInputBatchReceipt` are "incoherent", or if the readout the runtime built from its own lifecycle names a different binding than the one it passed alongside.
- **Why it looks pointless:** Double-checking invariants already enforced by construction in the same process (`RuntimeInputBatchReceipt::new` is the only source of these counts; the readout and binding come from one `RuntimeLifecycle` readout). `queued(count)`/`mailbox_full(count)` check `count > MAX_RUNTIME_INPUT_WIRE_EVENTS` on a count that the strict wire decoder already bounded one line earlier (`host.rs:1755-1764`). On failure the host returns a 500 for a mutation that has already been applied, which is worse than carrying the numbers.
- **Cost:** Per input batch and per operation (every tick under play); cheap individually, but it is eleven refusal branches with their own error codes.
- **Confidence:** High
- **Removal shape:** Delete the checks (or `debug_assert!` the arithmetic); make the constructors infallible and drop the `Result` plumbing. Remove the `queued(MAX+1).is_err()` assertion in test `wire_decode_resync_receipt_preserves_host_bounded_rejected_counts` (model.rs:2008-2011).

### Frame viewers larger than 4096 px per side are refused
- **Where:** `product-host/src/frames.rs:75` (`MAX_FRAME_SIDE = 4096`), `358-363` (viewer frame request), `297-301` (tool capture)
- **What it does:** `GET frames?width=W&height=H` answers 400 "frame size must be 1..=4096 pixels per side" when the page states its presentation size in device pixels above 4096.
- **Why it looks pointless:** "Sanity limit" with no concrete failure named. The width is the viewer's own device-pixel size: a 5K display (5120 px) or any 4K display at DPR > 1 is refused and never gets a frame. The renderer already clamps to the device's max texture dimension where it matters.
- **Cost:** Per frame request (cheap), but it breaks real hardware.
- **Confidence:** High
- **Removal shape:** Delete the cap (keep `>= 1`); drop the `5000` cases from the tests at frames.rs:406 and 534.

### Host-encoded JSON responses are refused above 512 KB
- **Where:** `product-host/src/host.rs:2323-2335` (`encode_runtime_result`, `PRODUCT_HOST_RESPONSE_BOUNDS`), `4449-4460` (`json_response` returns 500 "response could not be encoded" above `MAX_REQUEST_BODY_BYTES`), `2254-2269` (the preflight encode exists only to run this check before publication), `product-host/src/lib.rs:122-123` (one constant shared by request and response)
- **What it does:** Every JSON response the host itself serialized (operation results, the debug catalog, `diagnostics/read` with up to 64 retained events and their full messages) is dropped and replaced by a 500 if it exceeds 512 KB.
- **Why it looks pointless:** Size cap on data the host produced; nothing bounds it on the receiving side (the page just parses JSON). A `diagnostics/read` batch of 64 events carrying managed stack traces can exceed 512 KB (runtime-diagnostics' own test shows 64 dense events exceed 128 KB), at which point the diagnostics panel gets a 500 with no way to page past the oversize batch. The comment at 2254 ("A bad response must not first publish retained output") describes machinery that exists only to serve the cap.
- **Cost:** Per response: one extra length compare; for `call_runtime` the result is encoded before publication purely to apply the cap.
- **Confidence:** High
- **Removal shape:** Delete the response bound in both functions and `PRODUCT_HOST_RESPONSE_BOUNDS`; `call_runtime` can encode after publication like the other routes.

### Container open re-validates writer-produced inventory facts that nothing reads
- **Where:** `product-container/src/container.rs:204-250` (`check`): SHA-256 hex grammar 223-230, 64-byte alignment 231-233, bundle id grammar/uniqueness 205-213, bundle-membership prefix 241-246, sorted/unique paths 220-222; also `114-118` (`actual > file_len` → `Corrupt`)
- **What it does:** On every container open the inventory the Engine's own `write` produced is checked entry by entry: that `sha256` is exactly 64 lowercase hex chars (nothing in the runtime ever hashes or parses it; it is an opaque identity string), that each offset is 64-aligned, that bundle ids/roots match the writer's grammar and each member path starts with its bundle root, and that paths are strictly increasing.
- **Why it looks pointless:** Schema re-validation of the writer's output at load (the doc comment at 82-83 already says "Nothing is hashed: the recorded SHA-256 is each file's identity"). A malformed hex string or an unaligned offset would not panic: lookups are by exact path and slices are bounds-checked separately. The only checks that prevent a Rust panic are the per-entry range check (`entry.offset < start || end > data_end`, 234-240) and the inventory-range check — keep those.
- **Cost:** Once per open, O(entries) with a `format!("{}/", root)` allocation per bundled entry; `bundles.iter().find` per entry is O(entries × bundles).
- **Confidence:** High (for sha256 grammar, alignment, bundle membership, trailing-bytes-is-corrupt); Medium for the sorted/unique check (binary search needs sorted input, but the writer sorts; an unsorted inventory degrades to `Missing`, not a panic).
- **Removal shape:** Reduce `check` to the range/overlap test; drop the corresponding `with_inventory` cases in `damage_fails_at_open_with_a_named_error` (container.rs:570-585).

### Loose Product reads canonicalize and symlink-check every path the Engine itself listed
- **Where:** `product-container/src/source.rs:197-200` + `266-285` (`read` → `inside`: `is_relative_path` twice, `symlink_metadata`, `canonicalize`, `starts_with(root)`), `213-233` (`is_file`/`is_dir` same), `304-307` (`walk` already refuses symlinks); callers `csharp-product-runtime/src/product_bundle.rs:409-441`, `csharp-product-runtime/src/lib.rs:4452-4460`
- **What it does:** Every `read` of a loose (`rusty dev`) Product path does an `lstat` and a `realpath` on the joined path and refuses if it is a symlink or resolves outside the root, after `is_relative_path` already refused `..`. The callers pass paths that came from `source.files()` (the Engine's own walk, which already refuses symlinks) or from the SDK-staged manifest.
- **Why it looks pointless:** Path-traversal/symlink refusal on paths the SDK staged and the Engine enumerated; the staged tree under `obj/` is the product's own build output. Deleting it leaves `fs::read(root.join(path))`.
- **Cost:** Per file at product load and every asset reload (UI, content, bundles): two extra syscalls per file; a loose product with thousands of content files pays this on each `rusty dev` restage.
- **Confidence:** High
- **Removal shape:** `read`/`is_file`/`is_dir` use `root.join(path)` directly; keep `is_relative_path` only at the manifest edge if a clear error is wanted; delete `inside` and test `a_loose_product_refuses_symlinks` (source.rs:388-405).

---

## Medium confidence

### Input batches snapshot and restore the whole lane state as a rollback transaction
- **Where:** `runtime-input/src/lane.rs:160-200` (`ingest_batch`: `let checkpoint = self.checkpoint();` ... `self.restore(checkpoint)`), `431-465` (`InputLaneCheckpoint`, clones keyboard/pointer/controller vectors, axes, `mapping_active`, `pending_intents`, context string); consumer `csharp-product-runtime/src/lib.rs:3179-3185`
- **What it does:** Before ingesting a batch the lane deep-clones its entire state; if any later event fails, the pre-batch state is restored so "a valid prefix never reaches the product".
- **Why it looks pointless:** Rollback mechanism with no observed failure behind it. Every error path inside `ingest` already calls `clear_state()` and the host answers a hard error with a resync (new binding, sequence-zero clear), so the prefix is discarded anyway. The checkpoint exists so that `pending_intents` from before the batch survive a failed batch — but a failed batch triggers a rebind that clears them.
- **Cost:** Per input batch (per input POST / scheduled drain): O(held buttons + pending intents + mappings) allocations and clones.
- **Confidence:** Medium
- **Removal shape:** Delete `InputLaneCheckpoint`/`checkpoint`/`restore`; let a failure `clear_state()` as `ingest` already does. Rewrite or delete test `input_batch_rolls_back_a_valid_prefix_and_preserves_pending_held_state` (tests/runtime_input.rs:117).

### Controller axis/button and direct-intent axis magnitudes are capped at exactly 1.0, and a miss clears all held input
- **Where:** `runtime-input/src/model.rs:13-15`, `259-273` (`validate_controller_axis`, `validate_controller_button_value`), `445-449` (direct-intent axis), applied at `wire.rs:236`, `241`, `lane.rs:522-531`; on error `lane.rs:244-247` runs `clear_state()`
- **What it does:** A gamepad axis of `1.0000001`, a button value of `-0.0001`, or a UI slider claim of `1.5` is refused; because `ingest` treats any error as fatal, every held key and pending intent is cleared and the host rebinds.
- **Why it looks pointless:** Defensive bounds on values the Engine's own shell normalizes from the Gamepad API and the trusted product's UI; an out-of-range axis would at worst move slightly faster. The team already removed the equivalent cap for pointer deltas (test `fast_pointer_movement_accumulates_without_a_magnitude_cap`). The finite check in `AxisValue::new` is the only one with a representation reason.
- **Cost:** Per controller/direct event; cheap, but the failure consequence is disproportionate.
- **Confidence:** Medium
- **Removal shape:** Delete the three validators and `MAX_*_MAGNITUDE` constants (keep `is_finite`); drop the range cases from tests `direct_claims_fail_closed_for_unknown_kind_axis_range_and_pending_overflow` and `controller_button_values_are_bounded_persistent_axes_and_mappable`.

### A lowercase-only identity grammar is enforced on product identifiers, per event
- **Where:** `runtime-lifecycle/src/model.rs:18`, `37-55` (`validate_runtime_identity`: ≤128 bytes, lowercase ASCII/digits, single `.`/`_`/`-` separators, alphanumeric ends); applied per wire event at `runtime-input/src/wire.rs:82`, `112` (`InputContext::new` → `model.rs:216-222`), per direct claim at `model.rs:441-444`, `389-391`, per projection envelope at `runtime-ui/src/model.rs:86-87`, `180-192`, and at compile `runtime-input/src/compile.rs:96`, `135`, `154-159`, `202-210`, `256`; also `product-host/src/timeline.rs:196-206`
- **What it does:** Every input context string, intent name, payload contract, UI stream/contract and timeline correlation must match the grammar or the event/envelope is refused (for input, with a lane clear).
- **Why it looks pointless:** The strings are chosen by the trusted product (C# side) and echoed by the Engine's own shell; nothing in Rust derives behavior from the grammar (they are map keys and labels). A product naming a context `Gameplay` or an intent `jump_` loses input. The `windows(2)` separator scan runs on every event's context string.
- **Cost:** Per input event / per projection envelope: O(len) byte scans.
- **Confidence:** Medium (the compile-time checks are once-per-load and merely annoying; the per-event ones are the cost).
- **Removal shape:** Delete `validate_runtime_identity` and `is_identity` uses; keep non-empty if anything. Remove tests `neutral_runtime_identity_preserves_the_retained_boundary_grammar`, `identities_are_checked`, `neutral_mapping_construction_validates_identity_and_value_kind` (identity half).

### `snapshot_for_step` re-validates the token the same lifecycle just issued
- **Where:** `runtime-input/src/lane.rs:316-336` (phase check, `lifecycle.validate_phase_token`, then a three-field binding comparison that repeats what `validate_phase_token` → `require_current_token` already compared at `runtime-lifecycle/src/lifecycle.rs:560-593`)
- **What it does:** Per admitted step the lane checks the token's phase, asks the lifecycle to validate the token (instance, generation, control revision, known step), then compares instance/generation/control revision against its own binding again.
- **Why it looks pointless:** The token was produced by `RuntimeLifecycle::advance_realtime` in the same owner call a few lines earlier in `csharp-product-runtime`; the second comparison is the first one restated. The lane's `BindingMismatch` here can only fire if the host forgot to `rebind`, which is a programming error, not a runtime condition.
- **Cost:** Per simulation step; cheap, but it is three layers of the same check.
- **Confidence:** Medium
- **Removal shape:** Keep one (`validate_phase_token`) or none; `debug_assert!` the binding equality.

### `rusty dev` spawns the host with `--identity` and diffs it against the pack manifest on every start
- **Where:** `rusty-cli/src/main.rs:1542-1580` (`RuntimePack::verify`: artifact/schemaVersion/target checks, then `Command::new(host).arg("--identity")`, parse JSON, compare to `manifest.runtime`), called at `main.rs:177` and again for the desktop pack at `1754`
- **What it does:** Before each `rusty dev` the CLI runs the pack's `rusty-product-host --identity` as a subprocess and refuses if its JSON differs from `runtime-manifest.json`, after also refusing if `artifact != "rusty.product.runtime-pack"`, `schemaVersion != 1` or `target != TARGET`.
- **Why it looks pointless:** Pack/pair version negotiation inside one immutable cache directory that this same CLI installed after a SHA-256 check (`pair.rs:8-10`: "later builds and launches trust it without ... another payload pass"). The host and manifest ship in one archive; `rusty dev` already delegates to the pair's own `rusty` so the supervisor matches its host by construction. A pack a contributor hand-assembles is the only case, and the host prints its own identity at startup anyway.
- **Cost:** Once per `rusty dev` start (one extra process spawn and JSON parse), twice with window output.
- **Confidence:** Medium
- **Removal shape:** Reduce `verify` to "host binary exists"; drop the manifest identity/schema fields from `RuntimeManifest` if nothing else reads them.

### Publication frontiers are validated at construction and again at every receipt, with a duplicate scan
- **Where:** `product-host/src/publication.rs:55-64` (`RuntimePublicationFrontier::new`), `184-200` (`validate_frontiers`: same two checks plus `BTreeSet` duplicate-stream refusal), `134-152`
- **What it does:** Covered under the High finding above; listed separately because the construction-time check alone is the whole guarantee and the per-receipt pass is pure repetition.
- **Cost:** Per receipt carrying a binding/baseline.
- **Confidence:** Medium
- **Removal shape:** Delete `validate_frontiers`, `RuntimePublication::validate`, `DuplicateFrontierStream`, test `frontier_validation_rejects_invalid_or_duplicate_streams` (publication.rs:300).

### Served-bundle paths built from the staged file listing are `..`-checked, length-capped and duplicate-refused
- **Where:** `product-host/src/bundle.rs:187-210` (`normalize_path`: empty/`len() > 512`/leading slash/backslash/`.`/`..`/charset), `137-167` (`ProductHostBundle::new`: empty-bundle refusal, `checked_add` overflow on the byte total, duplicate-path refusal); producers `csharp-product-runtime/src/product_bundle.rs:409-446`
- **What it does:** Each entry path is `product-ui/<relative>` or `product-content/<relative>` where `relative` came from `ProductSource::files` (already relative, `/`-separated, no `.`/`..`); the bundle refuses the whole product if two entries share a path, if the sum of lengths overflows `usize`, or if any path is longer than 512 bytes.
- **Why it looks pointless:** `..`/absolute/empty-component refusals cannot trigger on `files()` output; a `BTreeMap` insert is last-write-wins and the producers dedupe with a `BTreeSet` already (`product_bundle.rs:433`); `usize` overflow of a byte total is unreachable; the 512-byte cap refuses a deep UI asset path for no reason. The ASCII charset check is the one with a reason (requests are matched undecoded, so a space or non-ASCII name would be unservable) and could stay as a clear staging error.
- **Cost:** Per staged file at load/reload; cheap. Mostly ceremony.
- **Confidence:** Medium
- **Removal shape:** Keep the charset check (or move it to where the file name is first seen); delete the rest and the `PRODUCT_HOST_BUNDLE_DUPLICATE` / `PRODUCT_HOST_BUNDLE_BOUNDS` / `PRODUCT_HOST_BUNDLE_ENTRY_BOUNDS` codes.

### The request-body size cap is re-applied in four places after `read_request` enforced it
- **Where:** `product-host/src/host.rs:4072-4085` (the real bound, applied while reading), then `host.rs:4433-4438` (`decode_json`), `model.rs:938-943` (`ProductHostInputBatch::decode_json`), `model.rs:1682-1687` (`decode_strict_json`), `session.rs:360-365` (`decode_canonical_u64`)
- **What it does:** Each decoder returns `PRODUCT_HOST_BODY_BOUNDS` if the slice is longer than 512 KB, which cannot happen for bytes that came through `read_request`.
- **Why it looks pointless:** Re-checking a bound already enforced upstream; the JSON helpers in `session.rs` exist for in-process callers (`input_json`, `advance_realtime_json`) that never had an HTTP body.
- **Cost:** Per request; one compare each. Low cost, but it binds the model layer to an HTTP constant (`runtime-input` even has a test, `wire_decoder_does_not_inherit_an_http_hosts_byte_budget`, pushing back on this).
- **Confidence:** Medium
- **Removal shape:** Delete the four re-checks; the bound lives in `read_request` only.

### 1024 pending intents between steps clears every held key
- **Where:** `runtime-input/src/model.rs:12` (`MAX_PENDING_INGRESS = 1_024`), `lane.rs:560-563`, `694-697` (on overflow: `clear_state()` + `PendingIngressOverflow`)
- **What it does:** If more than 1024 mapped intents or direct claims accumulate before the next snapshot, the lane drops all held/pending state and errors, which the host answers with a rebind.
- **Why it looks pointless:** Max-count limit with no failure named; a product that holds time (`engine.time` held, debug stepping, long pause with UI claims) and a chatty UI can hit it, and the cost of 1024 small structs is nothing. The host-side mailbox is already bounded (`MAX_HOST_INPUT_BATCHES`), so this is a second bound under the first.
- **Cost:** Per intent push (a length compare); harm is the input wipe.
- **Confidence:** Medium
- **Removal shape:** Delete the constant and both branches; remove the overflow case from `direct_claims_fail_closed_for_unknown_kind_axis_range_and_pending_overflow`.

### Fixed-step configuration refuses more than 240 Hz or 16 catch-up steps
- **Where:** `runtime-lifecycle/src/model.rs:10-12`, `139-161` (`RuntimeLifecycleConfig::new`)
- **What it does:** A product manifest asking for a 360 Hz simulation or a catch-up window of 20 steps fails to load.
- **Why it looks pointless:** Magnitude caps on a product-chosen tuning value; the arithmetic is `u128` and overflow-safe (see the test `realtime_handles_max_host_timestamp_without_integer_multiplication_overflow`). The zero checks are the only ones that prevent a division/loop problem.
- **Cost:** Once per load.
- **Confidence:** Medium
- **Removal shape:** Keep the two `== 0` checks, delete the two maxima and the `FixedStepHzExceedsMaximum`/`CatchUpStepsExceedMaximum` variants; trim `realtime_configuration_enforces_runtime_bounds`.

### Diagnostic events validate their own source/code grammar and refuse a ninth or duplicate field on every publish
- **Where:** `runtime-diagnostics/src/lib.rs:17-25` (`MAX_SOURCE_BYTES`, `MAX_CODE_BYTES`, `MAX_FIELDS = 8`, `MAX_FIELD_KEY_BYTES`, `MAX_RING_CAPACITY`), `186-205` (`bounded_identity` on source and code), `224-247` (`with_field`: count cap, key grammar, linear duplicate-key scan), `362-367` (ring capacity bounds)
- **What it does:** Every `RuntimeDiagnosticEvent::new` (host, runtime and C#-originated diagnostics, published from hot paths such as per-batch input drops at `csharp-product-runtime/src/lib.rs:3190`) scans the source and code strings; every `with_field` scans the key and the existing keys. The callers in `host.rs` unwrap the `Result` with `expect("runtime diagnostics are bounded")` or silently drop the event when a field is refused (`publish_host_diagnostic`, `host.rs:1245-1256`).
- **Why it looks pointless:** All producers are Engine code with literal codes; a diagnostic that fails its own bounds is a diagnostic lost, which is the opposite of what a diagnostics sink is for. Duplicate keys could be last-write-wins.
- **Cost:** Per published event, O(len + fields).
- **Confidence:** Medium
- **Removal shape:** Make `new`/`with_field` infallible; delete `bounded_identity` and the `MAX_*` caps except the ring capacity default.

### Timeline completions validate the correlation twice and cross-check two copies of it
- **Where:** `product-host/src/timeline.rs:96-106` (`RuntimeProvenance::new` validates `correlation`), `142-158` (`TimelineCompletionEnvelope::new` validates the same string again), `product-host/src/model.rs:1264-1269` (`provenance.correlation != correlation` → refuse)
- **What it does:** The wire carries `correlation` at the top level and again inside `provenance`; the host refuses a completion whose two copies differ and grammar-checks each.
- **Why it looks pointless:** The sender is the shell/product; a wire with one field and no grammar removes three refusals. Same identity-grammar objection as above.
- **Cost:** Per timeline completion (rare).
- **Confidence:** Medium
- **Removal shape:** One `correlation` field, no grammar; delete `validate_runtime_identity` in timeline.rs and `PRODUCT_HOST_TIMELINE_CORRELATION`.

### u64 counters carry an exhaustion fault path
- **Where:** `runtime-lifecycle/src/lifecycle.rs:219-221`, `235-238`, `252-255`, `371-373`, `455-458`, `482-485`, `498-501`, `595-600` (`counter_exhausted` → `RuntimeState::Faulted`, `RuntimeFault::CounterExhausted`), `runtime-diagnostics/src/lib.rs:402-407`, `runtime-input/src/lane.rs:742-748` (`SequenceExhausted`), `product-host/src/model.rs:555` (`ProductHostRuntimeFault::CounterExhausted` on the wire)
- **What it does:** Every increment of a generation, control revision, step, presentation or diagnostic sequence is `checked_add`, and overflow of a `u64` (at 60 Hz: ~10^10 years) faults the runtime and is a wire-visible fault kind with its own tests.
- **Why it looks pointless:** A recovery mechanism for a failure that cannot occur, carried through three crates and the TypeScript contract. `u128` for `dropped_realtime_steps` likewise.
- **Cost:** Negligible per call; the cost is the surface (fault variant, error variant, tests `simulation_sequence_exhaustion_faults_instead_of_wrapping`, `sequence_exhaustion_never_wraps_to_a_reused_zero`).
- **Confidence:** Medium (harmless at runtime; worth removing for size).
- **Removal shape:** Plain `+= 1`; delete `CounterExhausted` variants and their tests.

### Presentation reports cap anchors at 64 and anchor names at 64 bytes
- **Where:** `product-host/src/presentation.rs:22-23`, `72-79`
- **What it does:** A layout report with more than 64 anchored UI elements, or an anchor named with more than 64 bytes, is refused with 400 and the renderer keeps the previous anchors.
- **Why it looks pointless:** Max-count/max-length limits on the Engine's own page; a product UI with many camera-anchored elements is just a bigger map. (The finite/positive checks on sizes and rects are cheap and keep NaN out of the renderer; those can stay.)
- **Cost:** Per report (on resize/layout change).
- **Confidence:** Medium
- **Removal shape:** Delete the two constants and their branches; drop the empty-name test case at presentation.rs:245.

---

## Low confidence

### HTTP path grammar and `..` refusal on a server that only does exact map lookups
- **Where:** `product-host/src/host.rs:4134-4147` (`valid_request_path`: `..`/`.` components, `//`, 512-byte cap, ASCII allowlist), `4051-4060` (duplicate header → 400)
- **What it does:** Refuses request lines whose path contains `..`, is over 512 bytes, or has a character outside the allowlist; refuses any repeated header name.
- **Why it looks pointless:** Bundle paths are served from a `BTreeMap` by exact string (`bundle.rs:169-176`); there is no filesystem to traverse. Repeated headers are legal HTTP. These are network bytes, so they are allowed to be checked, but the checks defend nothing.
- **Cost:** Per request, O(path).
- **Confidence:** Low (cheap; network-side).
- **Removal shape:** Keep the leading-`/` requirement; delete the rest; keep the duplicate-header map but take the last value.

### Browser attachment id grammar on a correlation header
- **Where:** `product-host/src/model.rs:283-289`, `host.rs:1338-1343`
- **What it does:** `x-rusty-browser-attachment` is dropped to `"none"` in diagnostics unless it matches `[A-Za-z0-9_-]{1,128}`.
- **Why it looks pointless:** It is only ever logged as a field; the shell generates it.
- **Cost:** Per request.
- **Confidence:** Low
- **Removal shape:** Use the header as given.

### Supervisor command frame cap and self-generated instance-id checks in `rusty dev`
- **Where:** `rusty-cli/src/main.rs:59` (`MAX_SUPERVISOR_COMMAND_BYTES = 16 KB`), `2683-2689`, `2702-2706` (`runtime_instance_id == 0` refusal), `2751-2757` (`checked_add` "identity space exhausted")
- **What it does:** Refuses the two-field JSON command the CLI itself built if over 16 KB; refuses an instance id of 0 that `next_supervised_runtime_instance_id` can never return; faults on a `u64` overflow of pid + counter.
- **Why it looks pointless:** Invariants enforced by construction in the same file.
- **Cost:** Once per restage.
- **Confidence:** Low
- **Removal shape:** Delete; remove tests `initial_supervised_shell_rejects_a_zero_runtime_seed` and `supervisor_allocates_distinct_nonzero_runtime_incarnations` (main.rs:3645, 3657).

### Desktop-pack ABI equality and pair-manifest identity re-checks after the checksum
- **Where:** `rusty-cli/src/pair.rs:593-606` (`RUSTY_DESKTOP_IDENTITY`: `runtime.abi` of the extracted desktop pack must equal the runtime pack's), `701-706` (`RUSTY_INSTALL_ARCHIVE`: manifest `package.id`/`version` must match the archive name)
- **What it does:** After the archive's SHA-256 matched its published `.sha256`, the CLI parses the extracted manifests and refuses on mismatch.
- **Why it looks pointless:** The checksum already proves the archive is the published one for that version; the identity checks guard against a mis-published release, which is a CI concern.
- **Cost:** Once per install.
- **Confidence:** Low (cheap, once; keeps a clearer error for a bad mirror).
- **Removal shape:** Delete both, with test `a_desktop_pack_with_another_abi_is_refused` (pair.rs:1224).

### `--label` length/control-character check
- **Where:** `rusty-cli/src/main.rs:918-923`
- **What it does:** Refuses a session label over 80 chars or with control characters.
- **Why it looks pointless:** It is display text in `rusty dev list`; a long label just wraps.
- **Cost:** Once.
- **Confidence:** Low
- **Removal shape:** Delete (keep non-empty if desired); drop the empty-label assertion at main.rs:3123.

### Diagnostics log config bounds
- **Where:** `product-host/src/log.rs:100-107` (`rotate_bytes >= 4096`, `retention_files in 1..=4`)
- **What it does:** Refuses the host's own fixed defaults if someone changed them outside this range.
- **Why it looks pointless:** The config is constructed by the host with constants; nothing user-facing sets it.
- **Cost:** Once.
- **Confidence:** Low
- **Removal shape:** Delete.

### Container writer re-checks the entries the packer just enumerated
- **Where:** `product-container/src/container.rs:280-285` (`write`: `is_relative_path` + adjacent-duplicate refusal on sorted entries), `176-178` (decompressed-length check per compressed read)
- **What it does:** `pack_product`/`pack_content` build entries from `ProductSource::files` (already relative, unique via `BTreeSet`/sorted walk); `write` re-validates and refuses duplicates. `read` checks the zstd output length against the inventory.
- **Why it looks pointless:** Writer-side repetition; the decompressed-length check is cheap and protects against a short stream (a representation reason), so it may stay.
- **Cost:** Once per pack; per compressed read.
- **Confidence:** Low
- **Removal shape:** Delete the path/duplicate loop in `write`; keep the length check.

### Update-attribution snapshot sorts up to 2048 samples per diagnostics read
- **Where:** `product-host/src/host.rs:474`, `545-583` (collect + `sort_unstable` + `max_by_key` over the window)
- **What it does:** Each `diagnostics/read` copies and sorts the attribution window to compute p50/p95/max.
- **Why it looks pointless:** Not validation; a whole-collection pass per call where the ring could maintain running order or a smaller window. Listed because the brief asks for per-call whole-collection scans.
- **Cost:** Per diagnostics read (polled by the panel, ~1 Hz), O(n log n) with n ≤ 2048.
- **Confidence:** Low
- **Removal shape:** Smaller window or compute lazily; not urgent.

### 1024-event cap per wire input batch
- **Where:** `runtime-input/src/wire.rs:14`, `30-32`; `product-host/src/host.rs:1755-1757` turns it into a mailbox resync
- **What it does:** A batch with more than 1024 events is refused and the runtime is rebound (loopback test `over_limit_malformed_batch_resynchronizes_once_without_becoming_terminal`).
- **Why it looks pointless:** The page batches one frame's events; 1024 per frame is implausible but, if it happened (a stalled tab flushing), the response is to drop them all. A `Vec` of 1025 events costs nothing.
- **Cost:** Per batch, one compare.
- **Confidence:** Low
- **Removal shape:** Delete `WireEventLimit`; delete the loopback test.

### UI projection wire `artifact` tag check
- **Where:** `runtime-ui/src/model.rs:12`, `283-287` (`WireUnknownArtifact`), and the matching `artifact: "rusty.product.runtime-readout"` string field on every readout at `product-host/src/model.rs:454-455`, `470`
- **What it does:** Each projection/readout carries a constant artifact string; the Rust deserializer refuses any other value.
- **Why it looks pointless:** A protocol tag between two halves of one pair, serialized on every readout/projection.
- **Cost:** Bytes per publication; a compare on decode (decode runs only in tests on the Rust side).
- **Confidence:** Low
- **Removal shape:** Drop the fields and the check if the TypeScript side does not key on them.

---

## Looked at and judged legitimate

- `product-host/src/host.rs:3952-4132` `read_request`: header/body byte bounds, `Content-Length` parse, timeouts, trailing-byte refusal, `Transfer-Encoding`/`Upgrade` refusal — bytes from the socket, checked once at receipt.
- `product-host/src/host.rs:4149-4165` `has_admitted_origin` (Host/Origin equality): a localhost server with `debug/execute` and lifecycle routes is reachable from any site open in the same browser; this is CSRF protection against genuinely untrusted web content, not product-protection.
- `product-host/src/host.rs:761-771`, `2656-2663`, `1151`, `2382`, `2480`: mailbox/subscriber-queue/connection/SSE bounds — resource bounds with defined recovery (overflow → fresh baseline), not validation.
- `product-host/src/frames.rs:279-379` query parsing (repeated params, both sides required): parsing a network query at first parse; keep (minus the 4096 cap above).
- `product-host/src/presentation.rs:65-71`, `80-85` finite/positive checks: cheap, keep NaN out of renderer viewports.
- `product-host/src/bundle.rs:157-162` index.html required: the page cannot start without it.
- `product-container/src/container.rs:84-131` header magic, truncation, inventory-in-file, and `check`'s per-entry range/overlap test: prevent out-of-bounds slices on a corrupt or truncated file.
- `product-container/src/source.rs:160-178` `ProductSource::open` dir/file split via `symlink_metadata`: once, needed to choose the mode.
- `product-container/src/pack.rs:60-69`, `153-159` overlap refusal: deleting the inputs is a concrete failure.
- `runtime-input/src/model.rs:244-252` `AxisValue::new` finite check and `-0.0` normalization: NaN would poison accumulated deltas; representation reason.
- `runtime-input/src/lane.rs:205-248` sequence/binding/context admission and `rebind_event` rules: this is the lane's ordering contract with the host, not re-validation.
- `runtime-input/src/compile.rs:340-371` unknown-intent and value-kind mismatch at compile: once per load, and a mismatch would otherwise panic/misroute at ingest.
- `runtime-lifecycle/src/lifecycle.rs:96-189`, `384-432` state machine and token validation API: the lifecycle's whole purpose; callers opt in.
- `runtime-diagnostics/src/lib.rs:391-400` once-per-code recoverable warning coalescing: policy, not validation.
- `svc-http/src/lib.rs:138-145` URL scheme/host check, `472-479` `MEMORY_BODY_LIMIT` (stated reason: unbounded memory for a memory destination), `529-534` content-length completeness, stall/connect timeouts, partial-file naming and leftover cleanup, `609-621` `check_file_name` (the name becomes a path component in a library directory and must not collide with partial names): network bytes and a filesystem edge.
- `svc-session/src/wire.rs:7-10`, `207-209` `PROTOCOL`/`MAX_FRAME`, `lib.rs:959-971` protocol/application/secret handshake, `SLOW_MEMBER_BYTES`, `PENDING_EVENT_BYTES`, `MAX_CHAT_BYTES` (applied symmetrically at `683-689` and `1057-1059`), `check_payload`: peers are other machines running possibly different pairs; every bound has a stated memory reason.
- `rusty-cli/src/pair.rs:725-771` `verify_checksum` on downloaded archives (and `install_archive` for a local archive that came from a download): bytes from the network at receipt.
- `rusty-cli/src/pair.rs:453-466` `validate_version`, `main.rs:1022-1036` `instance_name`: both become path segments / directory keys.
- `rusty-cli/src/main.rs:2901-2918` `check_port` ephemeral-range refusal: documented concrete failure, opt-out flag.
- `rusty-cli/src/main.rs:2390-2408` watch paths must exist; `2460-2467` `verify_staged_product`: cheap existence checks with clear errors on user configuration.
- `rusty-cli/src/main.rs:2769-2860` `FileSnapshot` polling walk every 250 ms: this is the watcher's design, with the share-friendly backoff at 328.
- `rusty-cli/src/session.rs` lock/registry/claim-window logic: process coordination, not validation.
- `desktop-shell/src/placement.rs:21`, `38-57` `SIDE` range when parsing the placement file: once, the file may be stale/edited by hand; falls back to defaults.
- `desktop-shell/src/lib.rs`, `overlay.rs`, `keys.rs`: no validation of product data found.
- `product-host/src/log.rs` rotation/retention and stderr fallback rate limit: policy.
- `product-host/src/activity.rs`: a 1 s write throttle, not validation.

## Coverage

**Read in full:** `product-host/src/{host.rs (all 4475 lines incl. tests region skimmed by name), model.rs, bundle.rs, publication.rs, timeline.rs, scheduler.rs, session.rs, log.rs, frames.rs, engine_debug.rs, presentation.rs, audio.rs, video_options.rs, error.rs, activity.rs, lib.rs}`; `product-container/src/{lib.rs, source.rs, container.rs, pack.rs (to line 200; tests skimmed)}`; `desktop-shell/src/{lib.rs, overlay.rs, placement.rs, keys.rs}`; `rusty-cli/src/{main.rs, pair.rs, session.rs, asset.rs}`; `runtime-diagnostics/src/lib.rs`; `runtime-input/src/{model.rs, lane.rs, wire.rs, compile.rs, lib.rs}`; `runtime-lifecycle/src/{lifecycle.rs, model.rs}`; `runtime-ui/src/model.rs`; `svc-http/src/lib.rs`; `svc-session/src/{lib.rs, wire.rs, invitation.rs}`.

**Skimmed (test names and the cases cited above only):** `product-host/tests/loopback_host.rs`, `runtime-input/tests/runtime_input.rs`, `runtime-lifecycle/tests/runtime_lifecycle.rs`, `runtime-ui/src/tests.rs`, `svc-http/src/tests.rs`, `svc-session/src/tests.rs`, `rusty-cli/tests/*.rs`, the `#[cfg(test)]` modules inside the source files; `csharp-product-runtime/src/{product_bundle.rs:395-446, lib.rs:3175-3195, 3340-3370, 4440-4470}` and `render/packages/product-browser-host/src/local-transport.ts` (callers, to judge trust and cost only).

**Skipped:** `product-host/src/typescript.rs` (test-only wire type emission), `desktop-shell/examples/*`, `runtime-ui/src/lib.rs` (re-exports).


---

# Appendix 6: 06-render-wgpu

## Survey 06: `rust/crates/render-wgpu/`

Read-only pass over every file in `src/` (32,169 lines, all read in full) and
the rejection-shaped tests under `tests/` (grepped, then read at the hits).

Headline: this crate is largely clean of the hashing / re-validation /
adversarial pattern. No SHA-256 or digest is recomputed anywhere (`sha256:`
appears only as a prefix stripped to build resource identities), no schema is
re-validated after `render-model` (`apply.rs:1-3`, `glb.rs:3-4`,
`resources.rs:1-2` say so and the code agrees), and there is no
adversarial/tamper vocabulary. What remains is (a) one duplicate-handle
refusal, (b) a handful of per-frame whole-collection passes that re-establish
state a change-time hook could maintain, and (c) small defensive guards that
duplicate by-construction invariants. Findings: 2 High, 5 Medium, 9 Low.

---

### Per-view-pass liveness sweep of sprite batch buffers
- **Where:** `rust/crates/render-wgpu/src/effects.rs:1400-1404`
- **What it does:** At the top of `prepare_effects` (every view pass, every frame) it runs `self.effects.batches.retain(|handle, _| sprites.contains(handle))`, scanning every retained sprite-batch GPU buffer and probing `tables.sprites` (a `BTreeSet`) for each, to drop buffers whose sprite node was destroyed.
- **Why it looks pointless:** "Reference-counting / liveness scans that walk whole collections" and "whole-collection per frame where only a delta changed". Sprite nodes are removed in exactly two places, `destroy_node` (`apply.rs:698-735`, which already updates `tables.sprites` per handle) and the replace path in `insert_node` (`apply.rs:648-650`). Dropping the batch buffer there is a one-line `effects.batches.remove(&handle)`. Deleting the sweep without that hook would leak a buffer per destroyed batch; with the hook, nothing changes.
- **Cost:** per view pass per frame (world, viewmodel, backdrop, every offscreen view), O(batch buffers) `BTreeSet` lookups; it runs even when no sprite exists (empty map, so cheap then, but it is on the hot path and grows with batches).
- **Confidence:** High
- **Removal shape:** delete the `retain`; remove the batch buffer in `destroy_node` / the `insert_node` replace path.

### Duplicate-handle refusal on particle emitter `Create`
- **Where:** `rust/crates/render-wgpu/src/particles.rs:89-91` (the variant), `particles.rs:178-187` (the refusal); the cleanup it forces at `effects.rs:1178-1190`
- **What it does:** `ParticleProjectionOp::Create` with a handle already in `emitters` returns `Err(ParticleIssue::DuplicateHandle)` instead of replacing the emitter. Because the texture slot was loaded before the refusal, `apply_presentation` then has to check `holds_texture` and drop the orphaned slot ("A refused op leaves a freshly loaded slot unheld").
- **Why it looks pointless:** "Duplicate-id guards / 'already registered' refusals where last-write-wins would do." The product is trusted; the same crate already does last-write-wins for every other handle-keyed table (`insert_node` destroys and replaces at `apply.rs:648-650`; `Slots::insert` replaces; labels `apply` overwrites; `ghosts.insert` overwrites on Create). Deleting the refusal and doing what `Destroy` + `Create` would do (release the old emitter's slot, keep or drop its live particles) makes the op idempotent and removes a branch of the orphaned-slot cleanup. Nothing else in the repo consumes `ParticleIssue::DuplicateHandle` (the `DuplicateHandle` codes in audio/billboard are other crates' enums). No test asserts it.
- **Cost:** per op (cheap), but it is a policy guard and it keeps the `effects.rs:1183-1188` recovery path alive.
- **Confidence:** High
- **Removal shape:** replace the `contains_key` refusal with replace-semantics (let go of the prior emitter's slot as `Destroy` does), delete `ParticleIssue::DuplicateHandle`; the `effects.rs:1183-1188` branch then covers only `Update` of an unknown handle and can shrink or go.

### Every `DefineMaterial` scans all parts and rebuilds every lit-fallback part
- **Where:** `rust/crates/render-wgpu/src/apply.rs:1404-1418` (in `insert_material`); same shape at `apply.rs:1135-1151` (`define_texture` scans all materials), `apply.rs:1271-1294` (`define_shader` scans all materials), `apply.rs:1323-1332` (`is_layer_variant` formats a string per known palette per call), `apply.rs:1344-1360` (`paletted_meshes_drawing` scans all payload meshes), `apply.rs:1549-1562` (`define_static_mesh` scans all nodes), `animated.rs:638-647` and `653-662` (scan all animated instances), `voxel.rs:181-193` (scan all nodes)
- **What it does:** On each material definition the code walks `parts.meta` end to end collecting every part bound to that material **or to `MaterialRef::LitFallback`**, dedups the node handles and calls `rebuild_parts` on each. The sibling define paths each walk a whole table to find dependents by string compare.
- **Why it looks pointless:** This is the ResourceList shape from `2ee8375`: O(N) per call, so a bulk load of M materials over N parts is O(M·N) and rebuilds the same fallback parts M times. The `LitFallback` clause exists so a part whose material arrives later rebinds, but it rebuilds every fallback part on every define of any material, not just the one named by the slot. Nothing is validated here; it is dependency discovery that a per-id reverse index (material id -> nodes, texture id -> materials, asset id -> instances) would answer in O(dependents). Deleting the scan outright would leave parts bound to a late-arriving material on the fallback until something else rebuilds them, so the remedy is an index, not deletion.
- **Cost:** per define op, O(parts) (or O(materials)/O(nodes)/O(instances)); not per frame. For `define_texture` it also clones every dependent `RenderMaterialDescriptor`.
- **Confidence:** Medium (cost-only finding; correctness is fine)
- **Removal shape:** replace with per-id reverse indices maintained at part insert/remove and asset define/release; drop the blanket `LitFallback` rebuild in favour of rebuilding only parts whose unbound slot names the defined id.

### Per-frame re-derivation of pipeline variants from the whole draw list
- **Where:** `rust/crates/render-wgpu/src/frame.rs:1399-1414` (`batch_variants`), called at `frame.rs:2143-2155`, `2190-2196` (every view pass) and `1243-1255` (every stale shadow render)
- **What it does:** For every view pass it iterates every batch of the CPU list, the GPU-cull candidates and the pre-pass list, looks up each part's material and mesh, computes `(features, pass)`, sorts and dedups, and calls `Layouts::prepare*` for each, which is a `contains_key` returning `false` in steady state. `pipelines.rs:6-7` and the `pipelines_created` stat (`frame.rs:95-97`) both say pipelines are made at material definition, so this pass normally finds nothing to do.
- **Why it looks pointless:** "Per-frame whole-collection passes re-checking retained resources." The feature set of a part is fixed at `Parts::insert` (`class.features` already includes the mesh's features, `apply.rs:1062-1071`); the pipeline set per target is created at `pipeline_set` (`frame.rs:2130-2141`). Both are change points where the needed pipelines could be made once. It is partially load-bearing today: `prepare_material` at define time does not cover mesh-level features (`LAYER_WEIGHTS`, `VERTEX_OCCLUSION`; `VERTEX_TANGENTS` is covered by `animated.rs:576-599`), a target format first seen later, or shadows enabled after materials were defined. `Pipelines::get` panics on a missing key, so the pass cannot simply be deleted; it must move.
- **Cost:** per view pass per frame, O(batches) with two table lookups each plus a sort; plus O(all caster batches) whenever any shadow layer is stale.
- **Confidence:** Medium
- **Removal shape:** prepare at `Parts::insert` (features known) and at `pipeline_set` creation (replay known variants for the new target) and when `options.shadows` turns on; delete `batch_variants` from the frame path.

### Per-draw re-lookup of mesh and material liveness, to tolerate releases
- **Where:** `rust/crates/render-wgpu/src/frame.rs:1439-1444` (`draw_batches_bound`), `frame.rs:1139-1146` (`draw_batches_indirect`), `frame.rs:1385-1396` (`part_material` fallback), `frame.rs:1078-1079` (candidate index range), `frame.rs:1403-1404`; the releases that make them necessary: `apply.rs:116-121` (`ReleaseMaterial`), `apply.rs:123-127` (`ReleaseStaticMesh`)
- **What it does:** Each draw, each frame, re-reads `parts.meta[id]` as `Option`, re-resolves `MeshRef` through `Renderer::mesh` (slot index, or a `HashMap` lookup for payload/animated meshes) and re-resolves `MaterialRef` with a fallback to the lit-fallback bind group, skipping or substituting silently when the resource is gone.
- **Why it looks pointless:** "Per-frame validation of retained resources." The draw lists are rebuilt whenever a part is added, removed, shown or regrouped, so a part in a batch is live by construction; the only way a mesh or material can be missing is `ReleaseStaticMesh` / `ReleaseMaterial`, which remove the row but do not rebuild dependent parts (contrast `define_static_mesh`, which does rebuild on redefinition, and `release_voxel_object` / `release_animated_mesh`, which do). If release rebuilt dependents the same way, every per-draw `Option` here becomes a plain index and the fallback branch goes.
- **Cost:** per draw per frame (two lookups, one of them a `HashMap` probe for payload/animated meshes); small per draw, but it is the innermost loop.
- **Confidence:** Medium (cost is modest; the pattern is exactly the brief's)
- **Removal shape:** make `ReleaseStaticMesh`/`ReleaseMaterial` rebuild dependents (same reverse index as the finding above), then index directly in the draw loops (`expect`/`debug_assert`).

### Per-frame scan of all controllers for every animated instance
- **Where:** `rust/crates/render-wgpu/src/animated.rs:929-933` (inside `advance_animations_where`, every frame), again at `animated.rs:1092-1096` (`pose_animated_instance`)
- **What it does:** For each animated instance, each frame, `controllers.values().any(|c| c.target == handle)` decides whether it is controlled; posing then does `controllers.values().find(...)` again. O(instances x controllers) per frame.
- **Why it looks pointless:** Whole-collection per-frame pass where a link set at the change point would do: `apply_animation_op` (`animated.rs:338-379`) knows the target on Create/Update/Destroy and could store the controller handle on the instance (`AnimatedInstance` already carries playback state). Nothing is validated; it is a relationship rediscovered every frame.
- **Cost:** per frame, O(instances x controllers); also `pose_reporting_instances` (`animated.rs:969-976`) scans all instances per apply call, and `report_joint_poses` / `report_pending_bounds` (`animated.rs:1010-1016`, `1184-1190`) scan all instances per frame for a flag.
- **Confidence:** Medium
- **Removal shape:** keep `controller: Option<u64>` on the instance, set on Create/Update/Destroy; keep pending-handle lists instead of flag scans.

### Per-world-view rebuild of the distance-field entry list from every payload mesh
- **Where:** `rust/crates/render-wgpu/src/frame.rs:816-837` (`field_entries`), called at `frame.rs:2262-2270`
- **What it does:** On the `DistanceField` occlusion path, every world view walks every payload mesh, looks its node up, filters shown scene-layer ones and builds the world boxes for `DistanceFields::begin_view`.
- **Why it looks pointless:** Whole-collection per-frame pass; the set changes only on payload replace/release, node visibility or transform, all of which already pass through `rebuild_parts` / `propagate_transforms`. Not validation.
- **Cost:** per world view per frame, O(payload meshes) with a `HashMap` lookup each; only while DF occlusion is on.
- **Confidence:** Medium
- **Removal shape:** maintain the entry list incrementally (dirty on the events above), or cache it keyed on `scene_generation`.

### `MAX_VISIBLE_STRUCTURED` label cap not tied to a buffer limit
- **Where:** `rust/crates/render-wgpu/src/labels.rs:47-48`, `labels.rs:904-906`
- **What it does:** Stops laying out structured labels after 256 per view. The instance buffer grows on demand (`labels.rs:1308-1310`), so nothing concrete overflows.
- **Why it looks pointless:** "'Sanity limits' on counts not tied to a concrete GPU/buffer limit." Honest caveat: the overlap resolution below it (`labels.rs:958-968`) is O(n^2) over `occupied`, so the cap does bound a quadratic; but that is a cost of the layout algorithm, not a reason for a product-facing drop.
- **Cost:** once per view (trivial).
- **Confidence:** Low
- **Removal shape:** delete the cap (or make the layout O(n log n)) and let the product decide how many it shows.

### Whole light-row multiset diff on every light change
- **Where:** `rust/crates/render-wgpu/src/probes/mod.rs:519-535` (`changed_rows`), used at `probes/mod.rs:810-821`
- **What it does:** Each time `lights_dirty` is set (any light node moves, so every frame for a moving light) and an indirect-light volume exists, it hashes every old and new light row into a `HashMap` to find the rows that changed, then marks bricks per changed row.
- **Why it looks pointless:** Whole-collection diff per change where the producers already know which light changed (`UpdateLight`, `update_subtree` on a light node set `lights_dirty` per handle). Not validation; it stands in for a per-light dirty list.
- **Cost:** per light change (per frame with a moving light), O(lights) hashing and a `Vec` clone of all rows. Small for typical light counts.
- **Confidence:** Low
- **Removal shape:** replace `lights_dirty: bool` with the set of changed light handles and mark bricks from those rows only.

### Finite/NaN re-checks on values the model already validated
- **Where:** `rust/crates/render-wgpu/src/composition.rs:380` (`scale.is_finite()` on `render_scale`), `ambient_occlusion.rs:423` (`strength.is_nan()`), `lib.rs:658-662` (`set_pixel_ratio` ignores non-finite/non-positive), `animated.rs:331-335` (`set_animation_time` ignores non-finite/negative), `particles.rs:224` (`!seconds.is_finite()`, after `advance_effects` already dropped `<= 0`), `animated.rs:1418-1419` (bounds finite check)
- **What it does:** Silently ignores or neutralises a non-finite value from settings, host or product.
- **Why it looks pointless:** `RenderDiff::SetRendererSettings { .. }.validate()` already refuses a NaN render scale and out-of-range AO settings in `render-model` (`tests/render_scale.rs:155-168`, `tests/renderer_settings.rs:124-140` prove it), so the renderer-side checks are re-validation. The host-time guards (`set_pixel_ratio`, `set_animation_time`) protect against the Engine's own clocks and scale factors. If deleted, a NaN would render oddly, not panic.
- **Cost:** once per call / per view; negligible.
- **Confidence:** Low
- **Removal shape:** delete the model-covered ones (`composition.rs:380`, `ambient_occlusion.rs:423`); keep or `debug_assert` the host-clock ones.

### Lockstep consistency guard on animated part lists
- **Where:** `rust/crates/render-wgpu/src/animated.rs:1324-1326`
- **What it does:** `write_animated_parts` returns early if `node.parts.len() != instance.part_nodes.len()`.
- **Why it looks pointless:** "Double-checking invariants already enforced by construction": `rebuild_parts` builds `node.parts` from the very list `animated_parts` produced while setting `instance.part_nodes` (`apply.rs:1088-1090`, `animated.rs:1513-1519`). Deleting it would change nothing while the invariant holds; if it ever breaks, silently skipping hides the bug.
- **Cost:** per pose (trivial).
- **Confidence:** Low
- **Removal shape:** `debug_assert_eq!`.

### Silent `else { return }` on resources the same frame just prepared
- **Where:** `rust/crates/render-wgpu/src/effects.rs:742-750`, `755-757`; `compose.rs:200-202`, `243-245`; `finish.rs:364-366`, `710-712`; `ghost.rs:563-565`, `568-570`
- **What it does:** Draw-time lookups of a pipeline set / bind group / HDR target that `prepare_effects`, `prepare_clear`/`prepare_blit`, `Finish::target` and `select_ghost_sectors` created earlier in the same pass ("Call `prepare_*` first" in the docs), with a silent skip if absent.
- **Why it looks pointless:** By-construction invariants re-checked at runtime; a miss would be a sequencing bug that silence hides. Cost is negligible, so this is a clarity point more than a performance one.
- **Cost:** per draw / per pass (a `Vec` position scan or `HashMap` probe).
- **Confidence:** Low
- **Removal shape:** `expect` / `debug_assert` and index directly.

### Redundant second joint lookup in `SetParentJoint`
- **Where:** `rust/crates/render-wgpu/src/apply.rs:448-468`
- **What it does:** Resolves the joint with `joint_node`, stores it, then resolves it again with `joint_pose` only to produce the "not a unique joint" error.
- **Why it looks pointless:** `resolved.is_none()` already answers the question the second lookup asks (both read `asset.joints.get(joint)`). Not validation of product data so much as doing the same lookup twice to build a report.
- **Cost:** per op; trivial.
- **Confidence:** Low
- **Removal shape:** derive the error from `resolved`.

### Per-influence bounds check in the CPU skinning loop
- **Where:** `rust/crates/render-wgpu/src/animated.rs:1227-1235` (`palette.get(joint)`)
- **What it does:** For every vertex and each of its four influences, every frame a skinned instance poses, bounds-checks the GLB joint index against the skin palette.
- **Why it looks pointless:** Joint indices come from the admitted GLB (`glb.rs:3-4`: "The runtime admitted the bytes; this reads them and does not re-validate"); an out-of-range index is import-time data. The brief allows checks that prevent a panic, so this one is defensible; it is listed because it sits in the hottest CPU loop of the crate.
- **Cost:** per vertex x 4 per posed skinned instance per frame (branch, well predicted).
- **Confidence:** Low
- **Removal shape:** keep, or validate once at `define_animated_mesh` and index directly here.

### Tests in this crate that only prove `render-model` rejections
- **Where:** `rust/crates/render-wgpu/tests/backdrop.rs:338-365` (`invalid_links_are_refused_by_the_model`), `tests/renderer_settings.rs:124-140` (`invalid_settings_are_refused_by_the_model`), `tests/volumetric_clouds.rs:235-257` (`invalid_regions_are_refused_by_the_model`), `tests/volumetric_fog.rs:257-277` (`invalid_fog_is_refused_by_the_model`), `tests/render_scale.rs:155-168` (NaN scale refused)
- **What it does:** Each asserts `RenderDiff::...validate().is_err()` for a zero/NaN/out-of-range descriptor value. They exercise `render-model`, not this crate.
- **Why it looks pointless:** "Tests whose sole purpose is to prove one of the above rejections." Whether the rejections themselves are pointless is the `render-model` surveyor's call (range checks on product-supplied floats where the value would merely render oddly). If those checks go, these tests go with them; they add nothing about rendering.
- **Cost:** test time only.
- **Confidence:** Low (dependent on the render-model verdict)
- **Removal shape:** delete alongside the model checks, or move to `render-model`'s own tests.

### `index_count` clamped to the index buffer when walking CPU geometry
- **Where:** `rust/crates/render-wgpu/src/ghost.rs:517-518`, `rust/crates/render-wgpu/src/probes/mod.rs:1174-1175`
- **What it does:** `(first + count).min(cpu.indices.len())` before slicing a part's triangle range for ghost bounds and probe triangles.
- **Why it looks pointless:** A part's `first_index`/`index_count` come from the payload's groups (`apply.rs:487-491`, `1532-1537`), which the GPU draws unclamped (`frame.rs:1462-1463`); if they were out of range the draw would already be wrong. The clamp prevents a slice panic, so it is cheap insurance rather than policy.
- **Cost:** per bake / per capture; trivial.
- **Confidence:** Low
- **Removal shape:** keep, or `debug_assert` and slice directly.

---

## Looked at and judged legitimate

- `apply.rs:174-195` `SetCloudRegion` / `SetFogVolume` caps at `MAX_REGIONS` / `MAX_VOLUMES`: both size fixed GPU buffers (`lib.rs:456-460`, `volumetric_fog.rs:305-309`); `volumetric_fog.rs:495` `.take(MAX_VOLUMES)` likewise.
- `shadows.rs:92-96` resolution clamp to `[256, PAGE_SIZE].next_power_of_two()`: `PAGE_SIZE` is the atlas texture; the floor is policy but trivial.
- `distance_fields.rs:277-280` `data.len() != BRICK_BYTES -> None`: a wrong-sized `write_texture` would be a wgpu validation error.
- `distance_fields.rs:298-302` `release` only frees a slot it still holds: prevents a double free of a brick.
- `resources.rs:267-272` `stream()` bounds check on resource byte ranges: prevents a slice panic on offsets the payload descriptor supplies; cheap, once per load.
- `glb.rs:643-650` index/uv length check before MikkTSpace: prevents a panic in the callback (note it does not cover `positions`, which would panic the same way); once per decode.
- `gpu.rs:172-224` `samples_supported`, `compute_refusal`; all `refused` readouts in `culling.rs`, `light_clusters.rs`, `volumetric_fog.rs`, `distance_fields.rs`, `ambient_occlusion.rs`, `lib.rs:739-797`: adapter capability policy, explicitly out of scope.
- `capture.rs:167-176` texture-limit check on a capture job: concrete device limit.
- `labels.rs:383-386` `Canvas::blend` bounds check: glyphs legitimately fall off the canvas edge during rasterization.
- `labels.rs:147-149`, `ghost.rs:237-240`, `262-265`, `277-281`, `animated.rs:355-359`, `369-371`, `apply.rs:628-633`, `particles.rs:189-192`, `203-206`: unknown-handle lookups returning an error; handle-table errors are allowed by the brief.
- `apply.rs:538-540` `ReplaceMeshDistanceField` on a node without a payload mesh: reports instead of silently dropping; one `contains_key`.
- `apply.rs:648-650` `insert_node` destroying an existing node before re-creating it: last-write-wins done right.
- `apply.rs:88-100` / `effects.rs:1152-1225` collecting `ApplyIssue`s: reporting, not refusing; the ops still apply as far as they can.
- `tables.rs:303-340` `Slots` / `Names`: no uniqueness scans; `len()` counts rows only for diagnostics.
- `tables.rs:606-610`, `615-617` regroup/former-bounds detection in `Parts::write`: per-part delta tracking, the opposite of a sweep.
- `frame.rs:1054-1058`, `1104-1107` view-list reuse and `list == previous` compare: delta detection of an upload, done per change.
- `frame.rs:1226` `layers.iter().all(|l| !l.stale)`: O(layers) per frame, layers are few.
- `effects.rs:1429-1432` `Arc::ptr_eq` + world compare per sprite batch: change detection per batch.
- `finish.rs:271-280`, `volumetric_fog.rs:392-393`, `cloud_march.rs:98-100`, `ambient_occlusion.rs:686-688` cache eviction by last-used frame: bounded resource caches, cheap.
- `probes/mod.rs:790-799` `covered_moved`: incremental, per-id.
- `probes/mod.rs:748-753` same-request early return and `same_lattice` scroll: change detection.
- `sky_light.rs:213` per-frame `source != wanted` compare (clones two `String`s per frame while the sky light is on, `frame.rs:360-364`): cheap change detection, could cache but not validation.
- `effects.rs:1675`, `1702`, `1721` `.max(0.0)` on particle size/softness, `effects.rs:898` strength clamp, `particles.rs:70`/`effects.rs:1698` `frame_count.max(1)`: shader-safety clamps (a divide by zero in the shader otherwise); per particle, trivial.
- `trace.rs`, `bake.rs`, `pose.rs`, `camera.rs`, `shadows.rs` numeric guards (`max(1e-3)`, `normalize_or`, `clamp(-1,1)`, `DEGENERATE`): numerical stability of the Engine's own math.
- `driver.rs:342`, `424` `poisoned.into_inner()`: mutex poisoning recovery, standard.
- `web.rs` popup rect clamping and CEF event remapping: geometry, not validation.
- `video.rs`, `surface.rs`, `timing.rs`, `target.rs`, `compose.rs`, `image_effect.rs`, `precipitation.rs`, `water.rs`, `primitives.rs`, `convert.rs`, `shaders.rs`, `thumb.rs`: nothing of the pattern; `shaders.rs` passes product WGSL straight to `render-shaders`/naga with no re-validation.
- Tests `tests/views.rs:487-491` (`capture_image` with a pose on a non-animated node errors "unknown animated mesh") and `tests/screenshots.rs:865-879` (a view material on a static-mesh node is reported while the rest of the update applies): they prove reporting of a genuine unrealizable op, not a guard.

## Coverage

- **Read fully (every line):** `src/lib.rs`, `apply.rs`, `frame.rs`, `animated.rs`, `effects.rs`, `labels.rs`, `tables.rs`, `batch.rs`, `pipelines.rs`, `shadows.rs`, `composition.rs`, `ambient_occlusion.rs`, `finish.rs`, `glb.rs`, `ghost.rs`, `particles.rs`, `volumetric_fog.rs`, `pose.rs`, `post.rs`, `web.rs`, `culling.rs`, `driver.rs`, `sky_light.rs`, `video.rs`, `distance_fields.rs`, `camera.rs`, `light_clusters.rs`, `resources.rs`, `compose.rs`, `image_effect.rs`, `target.rs`, `gpu.rs`, `cloud_march.rs`, `capture.rs`, `convert.rs`, `precipitation.rs`, `primitives.rs`, `shaders.rs`, `surface.rs`, `timing.rs`, `voxel.rs`, `water.rs`, `probes/mod.rs`, `probes/bake.rs`, `probes/trace.rs`, `probes/thumb.rs` (all 47 files, 32,169 lines).
- **Skimmed (grep for refuse/reject/duplicate/validate/is_err/ApplyIssue, then read at the hits):** `tests/backdrop.rs`, `tests/renderer_settings.rs`, `tests/volumetric_clouds.rs`, `tests/volumetric_fog.rs`, `tests/render_scale.rs`, `tests/views.rs`, `tests/screenshots.rs`, `tests/effects.rs`, `tests/labels.rs`.
- **Skipped:** the remaining `tests/*.rs` (rendering/screenshot tests with no rejection hits), `src/labels.wgsl`, `examples/`, `fonts/`.
- Also checked outside the crate for context only: `git show 2ee8375 --stat`, `render-model/src/core.rs` for `MAX_REGIONS`/`MAX_VOLUMES`, and a repo-wide grep for `DuplicateHandle` consumers.


---

# Appendix 7: 07-render-other

## Survey 07: render-model, render-projection, render-presentation, render-* (other)

Scope: `rust/crates/render-model`, `render-projection`, `render-presentation`,
`render-shaders`, `render-stream`, `render-video`, `render-audio`,
`render-export`, `render-verify`, `render-host-contracts`. Read-only.

Context on 2ee8375: the per-id delta in `render-projection/src/appearance.rs`
(`update_resources` / `apply_kind` / `check_references` / `ResourceUpdate`) is
clean: it touches only changed ids, keeps per-id user counts, and the only
remaining scans are on the refusal path ("found the slow way"). What it left
behind is (a) the **validation inside `apply_kind`**, which still re-validates
every changed mesh/material/texture body that the services already validated
at admission, and (b) the **same whole-collection patterns one layer down in
`render-presentation/src/world.rs`** (reference scans on every release, a
fixpoint scan over all nodes on every destroy), plus (c) whole-frame
re-validation via `RenderFrameDiff::try_from_ops` in the voxel paths.

Counts: High 7, Medium 9, Low 14 (30 findings).

---

### Presentation world scans every node, mesh, material, atlas and ghost capture on every Release* op
- **Where:** `rust/crates/render-presentation/src/world.rs:1233-1251` (ReleaseMaterial), `:1253-1268` (ReleaseTexture), `:1270-1278` (ReleaseSpriteAtlas), `:1280-1288` (ReleaseAnimatedMesh), `:1300-1312` (ReleaseShader), `:1329-1339` (ReleaseStaticMesh), `:1351-1355` (ReleaseVoxelObject)
- **What it does:** For each release op it walks `retained.static_meshes`, `animated_meshes`, `voxel_objects`, every `retained.nodes` entry (matching on material overrides / asset ids), every material's textures, and every ghost capture's full op list, to refuse with `ReferencedResource` if anything still names the id.
- **Why it looks pointless:** This is the exact pattern 2ee8375 removed one layer up. The runtime projector already refuses a still-referenced removal via per-id counts (`appearance.rs:407-492`) and orders destroys before releases (`resource_diffs`, `:680-712`); the voxel projectors never release something in use. A second guard here protects the Engine from the Engine. If deleted, a release of a referenced resource (an Engine bug) would leave a dangling id in the retained world and the renderer's own handle lookup would report it.
- **Cost:** per Release op, every op; O(nodes + meshes + materials + captures×ops). Creating and disposing one content mesh per call (the 2ee8375 benchmark) still pays an O(N nodes) scan per dispose here.
- **Confidence:** High
- **Removal shape:** delete the `bound`/`any(...)` scans and `ReferencedResource`; keep the `contains_key` → `UndefinedResource` lookup if the renderer needs it, or replace with per-id counts as in 2ee8375.

### Presentation world finds descendants by fixpoint over all nodes on every Destroy
- **Where:** `rust/crates/render-presentation/src/world.rs:1062-1085` (also `:454-466` in `capture_output_scene`, rare)
- **What it does:** On `Destroy`, loops over every retained node repeatedly until no new descendant is found, then `retain`s the whole node map.
- **Why it looks pointless:** Whole-collection pass per op where only a delta changed. The runtime projector keeps a `children` index (`runtime_appearance.rs:142`) and already emits a Destroy for every descendant it knows about (depth-sorted, `:695-706`). The world has no children index, so it rediscovers the subtree by scanning N nodes × tree depth, then rebuilds the map.
- **Cost:** per Destroy op; O(nodes × depth) + O(nodes) retain. Every object removal.
- **Confidence:** High (it is a whole-collection pass, not strictly a validation)
- **Removal shape:** keep a `children: BTreeMap<RenderHandle, BTreeSet<RenderHandle>>` beside `entity_nodes`, or trust the projector's explicit child destroys and remove only `handle`.

### `RenderFrameDiff::try_from_ops` / `try_from_published_ops` re-validate every op of frames the Engine built, mesh bodies included
- **Where:** `rust/crates/render-model/src/core.rs:1871-1878`, `:1880-1902`, `:1904-1938`; `:1437-1649` (`RenderDiff::validate`, which for `DefineStaticMesh`/`DefineAnimatedMesh`/`DefineVoxelObject`/`ReplaceMeshPayload` calls the full body validators). Callers: `render-projection/src/voxel_object.rs:233` (every voxel-object projection), `csharp-engine-services/src/voxel_scene_presentation.rs:1224` (every voxel presentation projection; every chunk `ReplaceMeshPayload` the Engine's own mesher produced), `csharp-engine-services/src/audio.rs:355`, `appearance.rs:1929` (presentation frames).
- **What it does:** After the projector has assembled the ops, validates each one again: every inline mesh payload is rescanned for finiteness (all positions/normals/uvs/colors) and index bounds (all indices), groups re-tiled, materials/textures/lights/sprites re-range-checked, handles re-checked against 2^53; then checks that `operation_count == ops.len()` and `revision == base_revision + 1` on values the same function (or the caller one line above) just computed.
- **Why it looks pointless:** Schema re-validation of Engine-produced data (criterion 2) and self-consistency checks of fields the constructor set (criterion "invariants enforced by construction"). The voxel chunk meshes come from `engine-spatial`/`svc_mesh`, not the product. The runtime projector path deliberately does **not** call this (`runtime_appearance.rs:847-850`, `world.rs:317-318` even says "do not rescan the mesh bodies"), so the voxel paths are inconsistent survivors. Deleting it changes nothing for a correct Engine; a bug in the mesher would surface as a renderer upload error instead of a refused frame.
- **Cost:** per projection call; O(total vertices + indices of every chunk/mesh in the frame). On a voxel stream this is a full scan of every rebuilt chunk every projection.
- **Confidence:** High
- **Removal shape:** make `try_from_ops`/`try_from_published_ops` plain constructors (or delete them and build the struct literal as `voxel.rs:581-590` does); keep `RenderDiff::validate` only as a test helper or delete it.

### Projection re-validates every changed mesh/material/texture/shader/atlas body already validated at admission
- **Where:** `rust/crates/render-projection/src/appearance.rs:159-242` (`apply_kind` `validate` closures), via `rust/crates/render-model/src/mesh.rs:661-683` (`StaticMeshAsset::validate`), `:220-346` (`MeshPayloadDescriptor::validate`: finiteness scan of all attribute values, index-bounds scan, groups tiling), `:1146-1212` (`AnimatedMeshAsset::validate`), `assets.rs:737-859`, `:1059-1081`, `:1268-1300`. Admission-side validation that already ran: `csharp-engine-services/src/appearance.rs:4399-4404` (whose comment reads "Every later projection revalidates catalog meshes, so refuse here rather than retaining an entry that would fail each subsequent snapshot"), `:3542`, `:3585`, `:3690`, `:4528`, `render_resources.rs:109`.
- **What it does:** Every time a resource id is pushed/changed in the catalog, the next projection runs the full validator on it again before retaining it.
- **Why it looks pointless:** Criterion 2 (validated at admission, validated again at projection). The `Arc<StaticMeshAsset>` is immutable; the services validated the exact same allocation before pushing it. The comment at services `:4399` shows the duplication is known and worked around rather than removed. 2ee8375 narrowed *which* entries get validated but kept the validation itself. Deleting it: a mesh that fails validation never reaches the catalog anyway.
- **Cost:** per changed resource per projection; O(V+I) for an inline static mesh, O(J²) for an animated mesh rig (see below), O(bytes) for an inline-payload texture (SHA-256 + CRC walk, see below).
- **Confidence:** High
- **Removal shape:** drop the `validate` parameter of `apply_kind` and the `Invalid{Material,Texture,Shader,Atlas,StaticMesh,AnimatedMesh}` variants of `AppearanceProjectionError`; keep `check_references`.

### `decode_mesh_resource_payload` re-hashes the whole resource and validates the payload twice on every open
- **Where:** `rust/crates/render-model/src/mesh_resource.rs:309-435`, in particular `:313-315` (descriptor validate), `:341-352` (re-run identity check + SHA-256 of all bytes, comment: "Repeat the identity check here rather than relying only on descriptor validation so this function remains safe if its admission ordering is changed later"), `:446-470` (per-value `is_finite` during decode), `:431-433` (`decoded.validate()` = second full finiteness + index-bounds scan). Also `PackedMeshResource::validate` `:26-46` re-hashes. Callers: `csharp-engine-services/src/render_resources.rs:1625` (`decode_static_mesh`, every binary static-mesh content open), `csharp-product-runtime/src/scene_snapshot.rs:559`.
- **What it does:** Validates the descriptor, computes SHA-256 over the entire packed resource and compares it to the descriptor's hash, checks the identity string format, decodes each stream with a finiteness check per value, then validates the decoded inline payload again (finiteness again, index bounds, groups).
- **Why it looks pointless:** Criterion 1 (re-hashing content the Engine itself packed; the hash *is* the identity the packer computed, `:187-188`) and explicit defense-in-depth language. The bytes come from the product's content file, which the SDK packed with `pack_mesh_resources`. Only the stream-range check (`decode_stream_range`) prevents a slice panic. If deleted, a corrupted file would decode to wrong geometry instead of refusing; the brief rates that acceptable for first-party content.
- **Cost:** per open: one SHA-256 over the mesh bytes + two full passes over every float/index.
- **Confidence:** High
- **Removal shape:** delete the hash recompute, the repeated identity check, the pre-validate, and the post-decode `validate()`; keep the range computation that slices the streams.

### Voxel-object projection validates instances, materials and the asset body on every call, then validates the frame again
- **Where:** `rust/crates/render-projection/src/voxel_object.rs:271-376` (`validate_and_snapshot`: per instance `transform.validate`, `metadata.validate`, `DuplicateInstanceId`, `ConflictingResource`), `:378-403` (`collect_material`: `material.validate()` and `MaterialIdMismatch` for every palette binding of every instance, every call), `:129-131` (`resource.validate()` = every mesh of a new voxel object), `:232-233` (`try_from_ops` validates the same `DefineVoxelObject` asset and `DefineMaterial` ops again).
- **What it does:** On every projection, every material referenced by every instance's palette is re-validated (so one material shared by 100 instances is validated 100 times), map key vs `material.id` is re-compared, the per-instance transform/metadata are re-range-checked; a newly built asset is body-validated at `:130` and again inside `try_from_ops` at `:233`.
- **Why it looks pointless:** Materials were validated when defined; the key/id mismatch check re-checks a map the Engine built; the asset body was produced by `voxel_object_render_asset_as` from an `AdmittedVoxelObject` one line earlier and is scanned twice. `DuplicateInstanceId` on a BTreeMap insert is a duplicate guard where last-write-wins would do.
- **Cost:** per projection call: O(instances × palette) material validations + O(all mesh vertices) × 2 for a new asset.
- **Confidence:** High
- **Removal shape:** delete `collect_material`'s validate/mismatch, delete `resource.validate()` and the `try_from_ops` (build the frame literal), drop the per-instance transform/metadata validate and `DuplicateInstanceId`.

### `project_catalog_material` validates the entire asset catalog on every single material projection
- **Where:** `rust/crates/render-projection/src/material.rs:26-35`; caller `csharp-engine-services/src/authored_content.rs:322` (per product request).
- **What it does:** Calls `asset_catalog::validate_catalog(catalog)` (duplicate ids, dependency graph, atlas quotas, surface identities over every entry) before projecting one material.
- **Why it looks pointless:** The catalog is an `AdmittedAssetCatalog`; admission already ran `validate_catalog` (`asset-catalog/src/admission.rs:16`). Criterion 3 (whole-collection validation per call). Deleting it changes nothing for an admitted catalog.
- **Cost:** per material projection call; O(catalog entries + references).
- **Confidence:** High
- **Removal shape:** delete `:26-35` and the `InvalidCatalog` variant.

---

### `reject_duplicates` sorts every fact and removal id on every apply/project
- **Where:** `rust/crates/render-projection/src/runtime_appearance.rs:364-367`, `:915-923`; error variants `appearance.rs:1099-1101`, `:1165-1167`.
- **What it does:** Collects all object ids from `facts` + `removals` (and separately lights), sorts, and refuses the batch if any id appears twice.
- **Why it looks pointless:** Duplicate guard on trusted product input (criterion 4). `changes` is a `BTreeMap` (`:371`, `:406`), so last-write-wins already happens; the only effect of the guard is to refuse. `project()` passes the complete object set every update, so the sort runs over every object every frame.
- **Cost:** per call, O(n log n) over the full batch; for `project` that is all objects every update.
- **Confidence:** Medium (cheap per element, but it runs on the hottest path and does nothing useful)
- **Removal shape:** delete the function and `DuplicateObject` / `DuplicateLight`.

### Texture validation re-hashes and CRC-walks inline PNG bytes on every validate
- **Where:** `rust/crates/render-model/src/assets.rs:1123-1163` (`validate_texture_payload`: SHA-256 over `encoded_bytes`, hash string format, resource name reconstruction), `:1165-1229` (`validate_png_rgba8`: full chunk walk), `:1231-1244` (bitwise software CRC-32 over every chunk). Reached from `TextureDescriptor::validate` `:1059-1081` via projection `apply_kind` (`render-projection/src/appearance.rs:173-186`), `RenderDiff::validate` (`core.rs:1490-1492`), and admission `admit_png_rgba8_resource` `:1032-1033`, `:1055`.
- **What it does:** For an `Inline` payload: recomputes SHA-256 of the bytes and compares to the descriptor's hash, then walks every PNG chunk computing CRC-32 bit by bit. For a `Resource` payload: string checks only. At admission it CRC-walks the whole PNG and then calls `descriptor.validate()` on the descriptor it just built from that hash.
- **Why it looks pointless:** Criterion 1 for the re-hash (the hash was computed by the Engine at admission, `:1034`). The CRC walk at first admission is "external file on import" and allowed by the brief, but the PNG decoder in `render-wgpu` rejects a corrupt file anyway, and the byte-wise CRC loop is the slowest possible form. The `descriptor.validate()` at `:1055` re-checks fields built one line earlier.
- **Cost:** per validate of an inline-payload texture: O(bytes) hash + O(bytes×8) CRC; once per admission for resource textures.
- **Confidence:** Medium (High for the `Inline` re-hash/re-CRC on validate; Low for the admission-time CRC)
- **Removal shape:** on `validate()` keep only asset-id/zero-dimension checks; at admission read IHDR for dimensions and drop the chunk/CRC walk.

### `AnimationRigSignature::validate` is O(joints²) and runs on every animated-mesh validate
- **Where:** `rust/crates/render-model/src/mesh.rs:815-897` (duplicate-id set, parent lookup, structural root set equality, ordered-id checks, then `:882-895` an ancestor-chain walk per joint using linear `find`), reached from `AnimatedMeshAsset::validate` `:1161-1164` and `AnimationClipPack::validate` `:1074-1076`.
- **What it does:** Full structural validation of the rig (acyclicity by walking each joint's chain with a linear search per step).
- **Why it looks pointless:** Produced and already validated by `asset-import` (`asset-import/src/animated_glb.rs:295`, `:557`); the doc comment on the type (`:789-792`) says renderer backends "do not re-validate it", yet projection and frame validation do. Criterion 2.
- **Cost:** O(J² ) per `AnimatedMeshAsset::validate`, which runs per changed animated mesh at projection and per `DefineAnimatedMesh` in any validated frame.
- **Confidence:** Medium
- **Removal shape:** validate at import only; drop `rig.validate()` from `AnimatedMeshAsset::validate` / `AnimationClipPack::validate` (or drop those entirely per the projection finding).

### `validate_appearance` clones each changed object into a throwaway descriptor to re-validate ids, slots, transform and metadata
- **Where:** `rust/crates/render-projection/src/appearance.rs:521-628`; called per changed object at `runtime_appearance.rs:487-493`. Underlying: `mesh.rs:741-753`, `:1303-1321` (`validate_asset_id` prefix check, `validate_slots` BTreeSet duplicate scan, `Transform::validate`, `RenderMetadata::validate`), `core.rs:1104-1128`.
- **What it does:** Builds a `StaticMeshInstanceDescriptor` / `AnimatedMeshInstanceDescriptor` / sprite clone (cloning `material_overrides`, `metadata`, `inspection`, `playback`) only to call `.validate()`: the asset id's prefix is checked after the asset was just found in `resources.static_meshes` by that id (`:544`, `:570`); the transform was already validated at `runtime_appearance.rs:475-477`; the metadata is Engine-built (`object_metadata`) and checked for sorted tags/non-empty label; `material_overrides` are scanned for duplicate slots.
- **Why it looks pointless:** Criterion "double-checking invariants already enforced by construction/lookup" plus duplicate guards on trusted input. The useful part (`Missing*` existence checks, `validate_material_references`) is a few map lookups.
- **Cost:** per changed object per call: several heap clones + scans of overrides/tags.
- **Confidence:** Medium
- **Removal shape:** keep the `contains_key` / `validate_material_references` lookups; delete the descriptor clone + `.validate()` and the `Invalid*Instance` / `InvalidPrimitive` / `InvalidSprite*` / `InvalidMaterialParameters` variants.

### JSON-safe (2^53) checks on product ids, handles, revisions and entities
- **Where:** `rust/crates/render-model/src/core.rs:921`, `:936-942` (`RenderHandle::validate`), `:1104-1112`, `:1909-1918`; `assets.rs:1602-1610`; `render-projection/src/runtime_appearance.rs:379-381`, `:446-448` (`UnsafeObjectId`/`UnsafeLightId`); `render-projection/src/retained.rs:8-10`; `render-presentation/src/frame.rs:206-350`, `:922-932` (every handle/entity/revision in every presentation op), `world.rs:272-276`, `:367-371`, `ghost_plate.rs:261-263`, `:326`, `particle.rs:631`; `render-host-contracts/src/lib.rs:212`, `:231`, `:238`, `:244-245`, `:355-361`.
- **What it does:** Refuses any u64 above 2^53−1.
- **Why it looks pointless:** These guard a JavaScript consumer that no longer sits on this path: the renderer is native (`render-wgpu`), frames reach the browser as JPEG (`render-stream`), and the only JSON encodings of render frames outside tests are diagnostics/exercise (`csharp-product-runtime/src/lib.rs:5215`); `serde_json` itself round-trips u64 exactly. A product that derives object ids from a 64-bit hash is refused for no renderer reason. Criterion "compatibility checks for a boundary that is the same build pair".
- **Cost:** per op / per fact; cheap, but spread across ~20 sites and several error variants.
- **Confidence:** Medium (Low if some product-facing JSON view of handles still exists; the owner knows)
- **Removal shape:** delete `JSON_SAFE_U64_MAX` and every check/variant that exists only for it; if one JS boundary remains, check at that boundary alone.

### `PresentationFrameDiff::validate` re-validates descriptors the domain projectors just validated, plus Engine-set sequence/schema fields
- **Where:** `rust/crates/render-presentation/src/frame.rs:100-135` (`try_from_ops` → `validate`: `schema_version` vs the const the constructor set at `:102`, publication self-consistency, `NonContiguousSequence` on sequences assigned from `ops.len()`), `:206-350` + `:352-932` (`validate_json_safe_integers` and the `validate_billboard` / `validate_ghost_plate` / `validate_particle` / `validate_audio*` family, including `synthetic_billboard_assets` `:608-669` which fabricates a fake asset map so `billboard::validate_descriptor` can be re-run). Domain-side validation already ran in `billboard.rs:419-451`, `audio.rs:428-539`, `particle.rs:553-665`, `ghost_plate.rs:254-306`, `animation/projection.rs:587-647`. Callers: `csharp-engine-services/src/audio.rs:355`, `appearance.rs:1929`, `video.rs:127`.
- **What it does:** Second full validation of every presentation op in a frame the projectors assembled.
- **Why it looks pointless:** Criterion 2 and construction invariants: the sequence numbers, schema version and publication fields are set by Engine code in the same crate; the descriptors were accepted by the projector that emitted them. The synthetic asset map exists only so the re-validation can pass its asset lookup.
- **Cost:** per presentation frame (per call that emits one); O(ops × descriptor size).
- **Confidence:** Medium
- **Removal shape:** make `try_from_ops` a constructor; delete `validate_json_safe_integers` and the `validate_*` family in `frame.rs` (and `synthetic_billboard_assets`).

### `DuplicateHandle` / `DuplicateNode` refusals for Engine-allocated handles
- **Where:** `rust/crates/render-presentation/src/audio.rs:448-450`, `:471-473`; `billboard.rs:426-428`; `particle.rs:571-573`; `ghost_plate.rs:264-266`; `animation/projection.rs:595-603` (also `DuplicateController` on `entity_handles`); `world.rs:941-943`; `rust/crates/render-audio/src/lib.rs:840-845`.
- **What it does:** Refuses a Create whose handle is already retained.
- **Why it looks pointless:** Handles come from Engine allocators (`StableHandleRegistry`, `next_handle`), never from the product; a collision is an Engine bug, not a runtime condition to report through a diagnostic queue. Criterion 4 / `debug_assert!` candidate.
- **Cost:** per Create op; one map probe each (cheap) plus a diagnostic path and error variant per domain.
- **Confidence:** Medium
- **Removal shape:** `debug_assert!(!contains_key)` or plain insert (last wins); delete the variants and their diagnostic messages.

### `verify_requirement` / `verify_asset` re-check id, kind, hash and version of an asset just looked up by id
- **Where:** `rust/crates/render-model/src/assets.rs:66-101`, `:113-125`; `rust/crates/render-presentation/src/asset_view.rs:26-44`; used by `audio.rs:561-567`, `billboard.rs:639-646`, `:703-704`, `particle.rs:659-660`, `animation/projection.rs:669-675`, `animation/model.rs:262-273`.
- **What it does:** Builds a `RenderAssetRequirement` from the descriptor, validates both the requirement's and the asset's id prefix, then checks `self.id != requirement.id` (the asset was fetched from the map **by** that id), kind, content-hash equality with the product-supplied hash, and `version < minimum_version` where `minimum_version` is always 0.
- **Why it looks pointless:** Id equality is guaranteed by the lookup; `minimum_version: 0` can never fail; the content hash is the product restating an identity the Engine already holds (criterion "trust identities already computed"). Only the kind check could prevent a later wrong-type use.
- **Cost:** per descriptor validate (per op); several string clones/allocs each.
- **Confidence:** Medium
- **Removal shape:** replace with a map lookup (+ kind check if the renderer would otherwise panic); delete `RenderAssetRequirement`, `verify_requirement`, `IdMismatch`, `VersionTooOld`, `ContentHashMismatch`.

### Voxel projector compares and clones the whole material map every projection
- **Where:** `rust/crates/render-projection/src/voxel.rs:335` (`*materials != self.last_materials`), `:393-402` (clone of the whole map), `:359-362`, `:378-382` (`check_chunks` over **all** chunks of every instance when anything changed).
- **What it does:** Full-map equality + clone per call; on any change, every group of every chunk is re-resolved against the slot map.
- **Why it looks pointless:** Criterion 3 (whole-collection diff per call where the caller knows which slot changed). Only the scan-all-chunks part is costly; it exists to refuse `MissingMaterial`, which the renderer's own lookup would also report.
- **Cost:** per projection: O(materials) compare; on a material change O(all chunks × groups).
- **Confidence:** Medium (materials are few; the chunk scan is the real cost)
- **Removal shape:** have the caller pass the changed slot ids, or diff only on `DefineMaterial`; drop the all-chunk `check_chunks` on material change.

---

### Redefined-mesh scan over all retained objects
- **Where:** `rust/crates/render-projection/src/runtime_appearance.rs:588-605`.
- **What it does:** When any mesh id was redefined, iterates every retained object to find instances of it.
- **Why it looks pointless:** Whole-collection pass; the projector keeps a `users` index by appearance id but not by asset id. Rare (only on redefinition).
- **Cost:** O(objects) per projection that redefines a mesh.
- **Confidence:** Low
- **Removal shape:** index objects by asset id, or accept the rare scan.

### `unchanged()` deep-compares whole mesh bodies when a new Arc is pushed
- **Where:** `rust/crates/render-projection/src/appearance.rs:716-718`, used at `:221`, `:234`.
- **What it does:** `Arc::ptr_eq || **previous == **next`; the fallback compares every vertex of two allocations.
- **Why it looks pointless:** A product that re-pushes the same `Arc` hits `ptr_eq`; one that builds a new allocation with equal content pays O(V) to discover it is equal, and one that replaces a mesh pays O(V) up to the first difference. Treating a new allocation as a redefinition would be cheaper and harmless.
- **Cost:** O(V) per changed mesh in the worst case.
- **Confidence:** Low
- **Removal shape:** `ptr_eq` only.

### `validate_joint` counts all matching rig joints to detect ambiguity
- **Where:** `rust/crates/render-projection/src/runtime_appearance.rs:964-978`; same pattern `render-presentation/src/world.rs:1004-1010`.
- **What it does:** `filter(id == joint).count()` over the rig to refuse `AmbiguousJoint`.
- **Why it looks pointless:** Rig joint ids are unique by import validation (`mesh.rs:826`); `any()` suffices.
- **Cost:** O(joints) per attached object validate; cheap.
- **Confidence:** Low
- **Removal shape:** `any()`; delete `AmbiguousJoint`.

### Magnitude caps on trusted product values with no resource behind them
- **Where (sample):** `rust/crates/render-model/src/lighting.rs:3`, `:125` (intensity ≤ 10 000); `core.rs:128`/`:136` (bloom ≤ 16), `:204`/`:214` (sun radius ≤ 20°), `:232`/`:236` (sun shafts ≤ 16), `:256`/`:265` (wind ≤ 16), `:364-365` (cloud region radius ≤ 1e6), `:422-423` (fog distance 8..1000, ambient ≤ 4), `:486-498` (fog volume density ≤ 4, emission ≤ 16), `:537-571` (precipitation speed/size/streak/extent/colour caps), `:749-753` (sky light ≤ 16), `:878`/`:899` (render scale ≥ 0.5); `scatter.rs:106` (tint ≤ 16), `rust/crates/render-projection/src/voxel_scatter.rs:54`, `:76`, `:96`, `:100-105`; `assets.rs:323`, `:329` (voxel tile scale/origin), `:376` (atlas padding ≤ 32), `:1491-1497` (sprite normal strength ≤ 4); `mesh.rs:502` (vertex colour 0..1), `:261-270` (voxel tile coordinate magnitude), `:1077` (≤ 256 clips), `:1433-1452` (≤ 4 pose clips, weights sum to 1 ± 0.001); `render-presentation/src/billboard.rs:481-486` (height 8..256, distance ≤ 10 000), `audio.rs:546-549` (pitch 0.25..4, pan, blend), `ghost_plate.rs:336-358`, `particle.rs:755-760`; `rust/crates/render-host-contracts/src/lib.rs:12-13`, `:213-226` (target ≤ 2048², ≤ 8M pixels total), `:262` (id ≤ 128 chars, no control chars).
- **What it does:** Refuses values outside a chosen range.
- **Why it looks pointless:** Criterion 5: the product is trusted and an out-of-range value would render oddly, not crash. Exceptions that look tied to a real buffer and are noted as legitimate below: `IndirectLightDescriptor::MAX_PROBES/MAX_AXIS`, `MAX_DROPS`, `MAX_SCATTER_PATCH_INSTANCES`, `MAX_SPRITE_BATCH_INSTANCES`, `MeshDistanceField` length, and the render-target dimension cap if the renderer sizes textures from it (adapter limits would otherwise be the real refusal).
- **Cost:** per set/op; cheap individually, but they anchor the `valid()`/`validate()` plumbing that keeps `RenderDiff::validate` alive.
- **Confidence:** Low (each one individually; the owner may want to keep a few as documented tuning ranges)
- **Removal shape:** delete the caps; keep clamps in the renderer where a shader needs a bounded value.

### Finite/NaN checks on product-supplied transforms, colours and positions
- **Where (sample):** `rust/crates/render-model/src/core.rs:966-981` (`Transform::validate`, including rotation length > ε), `:1013-1026`, `:1041-1047`, `:1720-1724`; `lighting.rs:119-131`, `:204-230`; `assets.rs:1368-1384`, `:1577-1601`; `pose.rs:122-182`; `render-presentation/src/frame.rs:458-480`, `:785-826`, `billboard.rs:752-771`, `particle.rs:802-835`; `render-host-contracts/src/lib.rs:269-353`.
- **What it does:** Refuses NaN/∞ (and sometimes zero-length quaternions/directions).
- **Why it looks pointless:** Criterion 5; a NaN transform draws nothing/garbage rather than panicking. Zero-length quaternion/direction normalisation is the one case that can produce NaN inside the renderer, and a clamp there would be the smaller remedy.
- **Cost:** per op/fact; cheap.
- **Confidence:** Low
- **Removal shape:** delete; normalise defensively (`max(ε)`) at the one or two renderer sites that divide by a length.

### `RenderMetadata::validate` canonical-tag and empty-label checks on Engine-built metadata
- **Where:** `rust/crates/render-model/src/core.rs:1104-1128` (`TagsNotCanonical`: tags must be strictly sorted; `EmptyTag`; `EmptyLabel`); reached from every instance/sprite/node validate.
- **What it does:** Refuses unsorted/duplicate tags or an empty label.
- **Why it looks pointless:** Metadata is built by the Engine (`runtime_appearance.rs:935-942`, `voxel.rs:910-955`) or is trusted product data; sorted-ness is a representation nicety, not a behaviour.
- **Cost:** per validate; O(tags).
- **Confidence:** Low
- **Removal shape:** delete.

### `validate_slots` duplicate-slot scan
- **Where:** `rust/crates/render-model/src/mesh.rs:1686-1699`; used at `:666`, `:748`, `:1211`, `:1310`, `voxel_object.rs:60`, `:156`, `scatter.rs:69`.
- **What it does:** BTreeSet scan for a repeated slot index, plus material-id prefix check per slot.
- **Why it looks pointless:** Duplicate guard where the renderer's slot map would take the last; runs on every instance validate.
- **Cost:** O(slots) with allocations per validate.
- **Confidence:** Low
- **Removal shape:** delete; let the slot map insert last-wins.

### Sprite atlas duplicate-frame scan
- **Where:** `rust/crates/render-model/src/assets.rs:1273-1298` (`NoFrames`, `DuplicateFrame`, per-frame uv/size range checks); test `:1687-1710` exists only to prove the rejection.
- **What it does:** BTreeSet scan over all frames on every atlas validate.
- **Why it looks pointless:** `frame_rect` (`:1302-1304`) is a linear `find` that already takes the first match; a duplicate frame is harmless. Criterion 4 and the brief's "atlas uniqueness scans".
- **Cost:** O(frames) per validate (per changed atlas at projection, per `DefineSpriteAtlas` in validated frames).
- **Confidence:** Low
- **Removal shape:** delete the set scan and `DuplicateFrame`; delete the test.

### Animated-mesh clip/pack uniqueness scans and provenance text checks
- **Where:** `rust/crates/render-model/src/mesh.rs:1160-1208` (`DuplicateClipId`, `DuplicateClipPack`, `EffectiveClipCollision`, `DefaultClipMissing`), `:1059-1092` (`AnimationClipPack::validate`: `valid_sha256` format of three hashes, non-empty producer/license strings), `:1095-1110` (`valid_sha256`, `valid_joint_id` ≤ 4096 chars, `valid_ordered_joint_ids`), `:1229-1249` (dense/duplicate embedded slot checks).
- **What it does:** Set-based uniqueness over clips and packs, hash-string format checks, and a check that a licence string is non-empty, on every `AnimatedMeshAsset::validate`.
- **Why it looks pointless:** All produced by `asset-import`, which validates once (criterion 2); a licence string has no render meaning.
- **Cost:** O(clips + packs) per validate, repeated per the projection finding.
- **Confidence:** Low
- **Removal shape:** validate at import only.

### Voxel-object render asset count/geometry caps and frame-id uniqueness
- **Where:** `rust/crates/render-model/src/voxel_object.rs:11-14` (8 193 frames/meshes, 8M vertices, 12M indices), `:50-59`, `:84-87`, `:88-100` (frame id non-empty + unique, mesh index range).
- **What it does:** Refuses an asset outside the caps or with a repeated frame id.
- **Why it looks pointless:** The asset is built by `voxel_object_render_asset_as` from an `AdmittedVoxelObject` (Engine data). `8_193` is a sanity limit with no buffer behind it. Only `frame.mesh < meshes.len()` prevents an index panic.
- **Cost:** O(frames + meshes) per validate (twice per new asset, see above).
- **Confidence:** Low
- **Removal shape:** keep the mesh-index bound as `debug_assert`; delete the rest.

### Presentation animation `StaleRevision` and identity-immutability refusals
- **Where:** `rust/crates/render-presentation/src/animation/projection.rs:622-630`.
- **What it does:** Refuses an Update whose `revision` is not greater than the retained one, or whose entity/graph id/version differ.
- **Why it looks pointless:** Revision ordering on a single product-owned call stream is a compatibility/ordering guard; last-write-wins is the natural rule.
- **Cost:** per op; cheap.
- **Confidence:** Low
- **Removal shape:** delete `StaleRevision`; treat identity change as a replace.

### Voxel-object `ChangedMaterialOverrides` refusal
- **Where:** `rust/crates/render-projection/src/voxel_object.rs:194-198`.
- **What it does:** Refuses a projection whose instance changed its material overrides since last time.
- **Why it looks pointless:** Unnecessary fail-closed behaviour on a product fact; the appearance projector recreates the node in the same situation (`requires_recreate`).
- **Cost:** per instance per projection; cheap.
- **Confidence:** Low (a restriction rather than a validation; classify as lifecycle deficiency)
- **Removal shape:** destroy + create like `runtime_appearance.rs:606-616`.

### Per-op retained-equality dedupe including deep `DefineTexture` comparison
- **Where:** `rust/crates/render-presentation/src/world.rs:282-307`.
- **What it does:** Compares each settings/define op against the retained value and skips it if equal; `DefineTexture` compares the full descriptor (bytes, for inline payloads).
- **Why it looks pointless:** The projectors already emit only changes; the comparison is a second diff. Cheap except for inline-payload textures.
- **Cost:** per op; O(1) except O(bytes) for inline textures.
- **Confidence:** Low
- **Removal shape:** drop the `DefineTexture` deep compare, or the whole block if the projectors are trusted.

### `Transform::validate` run twice per changed object
- **Where:** `rust/crates/render-projection/src/runtime_appearance.rs:475-477` and again inside `validate_appearance` → descriptor `.validate()` (`appearance.rs:537`, `:560`, `:594`, `:609`, `:624`).
- **What it does:** Same finiteness/rotation-length check twice on the same value.
- **Why it looks pointless:** Pure duplication (subsumed by the `validate_appearance` finding, listed so it is not missed if that one is only trimmed).
- **Cost:** per changed object; cheap.
- **Confidence:** Low
- **Removal shape:** delete one (or both).

### Presentation diagnostic de-dup scans
- **Where:** `rust/crates/render-presentation/src/audio.rs:412-426`, `billboard.rs:404-417`, `ghost_plate.rs:239-252`, `particle.rs` (same shape), `animation/projection.rs:571-585`.
- **What it does:** Linear search of the retained diagnostics (≤ 128) for an equal entry before pushing.
- **Why it looks pointless:** Bounded and only on the failure path; listed for completeness. Not worth removing on its own; it goes away with the refusals it records.
- **Cost:** O(128) per refusal.
- **Confidence:** Low
- **Removal shape:** none on its own.

---

## Looked at and judged legitimate

- `render-projection/src/appearance.rs:133-152`, `:245-279`, `:281-394`, `:407-492` — the 2ee8375 per-id delta, undo, and `check_references`: per-id, no scans on the success path; `MissingTexture`/`MissingMaterial` prevent a renderer lookup failure.
- `render-projection/src/runtime_appearance.rs:494-509` — `ParentCycle` walk bounded by `objects.len() + changes.len()`: prevents an infinite loop; O(depth).
- `runtime_appearance.rs:662-671`, `retained.rs:61-65`, `:73-87` — handle-namespace exhaustion (2^40): resource ownership, cheap.
- `runtime_appearance.rs:540-566`, `:569-582` — `MissingParent` / `MissingLightParent`: the renderer's parent lookup would fail.
- `runtime_appearance.rs:262-270` (`omitted_objects` sort + binary search) — inherent to the "complete set" adapter, not a check.
- `resource_list.rs:149-164` — `reindex` last-wins dedupe on `From<Vec>`: the correct replacement for the old duplicate refusal.
- `render-projection/src/voxel.rs:756-774` (`check_chunks` on dirty/built chunks only), `:819-900` (coarse planning) — delta-scoped.
- `render-projection/src/voxel_scatter.rs:541-600` — placement, not validation; `take(MAX_SCATTER_PATCH_INSTANCES)` sizes a buffer.
- `render-model/src/mesh.rs:187-206` (`MeshDistanceField` length == cells³) — prevents an out-of-bounds upload; keep (as a debug assert if the mesher is trusted).
- `render-model/src/mesh.rs:359-436` (resource stream range/overlap) — needed once to slice safely; the finding above is about repeating it.
- `render-model/src/mesh_resource.rs:58-240` (`pack_mesh_resources` validating inline payloads once before packing) — once at pack/import.
- `render-model/src/mesh_partition.rs:17-21`, `:51-53` — finite origin / i64 range: prevents overflow and bin aliasing; rare.
- `render-model/src/core.rs:707-735` (`IndirectLightDescriptor::MAX_PROBES`/`MAX_AXIS`) — 3D-texture limit on every adapter.
- `core.rs:537` (`MAX_DROPS`), `scatter.rs:17`, `sprite_batch.rs:17` — instance buffer sizes.
- `render-model/src/settings_options.rs:504-560` — player-facing option ranges/choices; that is the mechanism's job.
- `render-model/src/audio_resource.rs:34-81` — container identification of external bytes at first parse.
- `render-audio/src/opus.rs`, `soundtrack.rs`, `lib.rs:967-992` — decoder/format errors on external bytes; clip cache keyed by the hash the Engine computed (trusts it, good).
- `render-video/src/lib.rs:61-145`, `:190-231` — demux/decode errors at first parse of external WebM.
- `render-shaders/src/lib.rs:540-562` (`check_keyword`, compose) — compiling product WGSL; naga validation is the compiler.
- `render-stream/src/lib.rs` — no validation; poisoned-mutex recovery only.
- `render-export/src/lib.rs` — errors are "no glTF counterpart" for a node; policy, not validation.
- `render-verify/*` — the GPU verification lane and image compare: test/CI tooling, not the product path (`lane.rs:111-120` unknown-setting refusal is tool input).
- `render-host-contracts/src/lib.rs:338-353` viewport in 0..1 — cheap, once per composition; a view outside the surface would be scissored away silently, so arguably a clear error is worth it. Borderline; listed under the caps finding only by reference.
- `render-presentation/src/world.rs:975-984`, `:1063-1065`, `:1137-1219` — `UnknownNode` / `WrongNodeKind`: handle-table lookups.
- `render-presentation/src/animation/model.rs:236-470` — catalog validation at catalog load (once), with diagnostics.
- `render-presentation/src/motion/tween.rs:177-231` — validated once at definition; normalises rotations (a construction step).
- `render-presentation/src/video.rs:69-75` — nonzero handle / media type at play; trivial.
- `render-presentation/src/particle.rs:577-605` (`BudgetExceeded` against `max_reserved_particles`) — a real particle buffer budget.

## Coverage

**Read in full (non-test code):** `render-projection/src/{appearance.rs, runtime_appearance.rs, resource_list.rs, retained.rs, material.rs, lib.rs, voxel.rs, voxel_object.rs, voxel_scatter.rs}`; `render-model/src/{mesh.rs, assets.rs, mesh_resource.rs, voxel_object.rs, scatter.rs, sprite_batch.rs, audio_resource.rs, mesh_partition.rs, irradiance.rs, pose.rs, lib.rs}`; `render-model/src/core.rs` lines 1-130, 900-1200, 1420-1960 in full, 130-900 and 1200-1420 by targeted reads/grep (descriptor `valid()` fns and the `RenderDiff` enum); `render-model/src/lighting.rs` 90-235; `render-presentation/src/{frame.rs, video.rs, asset_view.rs}`; `render-presentation/src/world.rs` 195-700 and 920-1360; `render-host-contracts/src/{lib.rs, render_output.rs}`; `render-video/src/lib.rs`; `render-audio/src/stream.rs`.

**Read in part / skimmed:** `render-presentation/src/{billboard.rs 380-800, audio.rs 370-640, particle.rs 540-840, ghost_plate.rs 230-400, animation/projection.rs 560-720, animation/model.rs 225-470, motion/tween.rs 165-235}` (remaining lines: outline + vocabulary grep only); `render-presentation/src/animation/controller.rs`, `motion/easing.rs` (grep only); `render-audio/src/lib.rs` 395-470 and 761-1130, `opus.rs` 1-120, `soundtrack.rs` 1-80, `rolloff.rs` (grep); `render-model/src/settings_options.rs` (grep + 470-560); `render-stream/src/lib.rs` 40-330; `render-shaders/src/lib.rs` 540-700 + grep; `render-export/src/lib.rs` (grep for error sites only); `render-verify/src/*` (grep + `lane.rs` outline).

**Skipped:** all `tests/` directories, `#[cfg(test)]` modules, `render-projection/examples/*`, `render-model/src/mesh_partition_tests.rs`, `render-audio/src/tests.rs`, `render-verify/src/bin/rusty-gpu-lane.rs` body.

**Outside scope but read for call-site evidence:** `csharp-engine-services/src/appearance.rs` ~1850-1935 and ~4380-4420, `voxel_scene_presentation.rs` ~440-475 and ~1130-1240, `render_resources.rs` ~1595-1635, `authored_content.rs` ~300-335, `camera_view.rs` ~1330-1525; `csharp-product-runtime/src/lib.rs` ~5195-5225.


---

# Appendix 8: 08-assets-content

## 08 — Assets & content crates: pointless validation survey

Scope: `rust/crates/{asset-catalog, asset-import, core-assets, authored-scene, voxel-annotation, voxel-asset, voxel-convert, voxel-object-runtime, core-voxel, environment-authoring, entity-state, core-ids, core-math, core-space, core-time}` plus `scripts/inventory-validation.py`. Read-only survey; no edits, builds or tests.

Headline: the voxel content family (`voxel-asset` → `voxel-object-runtime` / `voxel-annotation` / `voxel-convert`) is built around "self-validating artifacts" whose SHA-256 identities are recomputed and compared on every decode, encode, canonicalize, resolve, query and edit, and immediately after the Engine computes them. The policy line "a staged inventory's SHA-256 is not recomputed on bundle open" is violated at essentially every entry point in that family. The second cluster is whole-document re-validation per call in `authored-scene` / `entity-state` (the ResourceList pattern), and a projection path that re-validates an already-admitted asset catalog with an O(N²) scan per material.

Findings: 34 (High 11, Medium 16, Low 7). Ordered by confidence then cost.

---

## HIGH

### 1. `validate_voxel_asset` recomputes both SHA-256 identities on every decode, encode, canonicalize and resolve; `with_computed_content_hash` hashes, then re-validates its own hashes
- **Where:** `rust/crates/voxel-asset/src/codec.rs:136-169` (validate recomputes `voxel_data_hash` at 144 and `content_hash` at 157); `:122-134` (`with_computed_content_hash` computes both hashes at 130-131 then calls `validate_voxel_asset` at 132, which computes them again); `:88` (decode validates), `:95` (encode validates), `:115` (canonicalize validates); `rust/crates/voxel-asset/src/frame.rs:149` (`resolve_voxel_asset` validates = rehash) and `:137` (`resolve_voxel_frame` validates = rehash). Consumers: `rust/crates/csharp-engine-services/src/voxel_content.rs:1087,1120` (decode on load), `rust/crates/csharp-engine-services/src/spatial.rs:884` (`resolve_voxel_asset` at spatial staging rehashes the same asset again), `rust/crates/voxel-convert/src/planning.rs:254` (`apply_conversion` re-validates the output that `with_computed_content_hash` produced at `convert.rs:125`), `rust/crates/voxel-convert/src/query.rs:187-198` (`occupied_voxels` → `resolve_voxel_frame` rehash per query), `rust/crates/environment-authoring/src/materialization.rs:402` then `:177` (compute hashes, then `encode_voxel_asset` validates+rehashes).
- **What it does:** `computed_content_hash` (`codec.rs:493-500`) clones the whole asset, canonicalizes (sorts palette, map, runs), serializes to JSON and SHA-256s it; `computed_voxel_data_hash` (`frame.rs:503-515`) clones and sorts all runs and SHA-256s them. Both are compared byte-for-byte against the stored strings.
- **Why it looks pointless:** The hash is an identity the Engine computed and wrote. Recomputing it on open is exactly the named policy violation. In `with_computed_content_hash` the comparison is tautological (same function, same input, one line later). Deleting the decode/encode/resolve rehash leaves the stored identity as the identity; a hand-edited file would simply carry a stale id.
- **Cost:** per load, per encode, per resolve, per conversion apply: 2 full clones + 1 JSON serialization + 2 SHA-256 passes over the whole asset, repeated 2-3× along one load path (decode → spatial resolve).
- **Confidence:** High
- **Removal shape:** compute hashes once in `with_computed_content_hash` (no trailing validate); decode/encode/canonicalize/resolve do semantic checks at most, never rehash.

### 2. Voxel objects: every frame is SHA-256'd up to five times on one admission, plus two whole-object JSON serializations
- **Where:** `rust/crates/voxel-asset/src/object_codec.rs:146-182` (`validate_voxel_object`: `validate_frame_hash` per frame at 148-162 → `frame.rs:93` rehash, then `computed_content_hash` at 170 → `:771-777` clone+canonicalize+JSON+SHA-256); `:673-688` (`append_frame_semantics`, called from `semantic_diagnostics` for every frame, clones the frame and computes its hash *again* at 680 just to call `validate_voxel_frame` with a matching hash); `:131-144` (`with_computed_voxel_object_hashes` hashes all frames + content at 140-141, then `validate_voxel_object` at 142 repeats all of it); `:96` (decode validates), `:103` (encode), `:125` (canonicalize). Runtime path: `rust/crates/voxel-object-runtime/src/admission.rs:85-86` (decode = validate) → `:106` (`canonicalize_voxel_object` = validate again) → `:122-124,140-142` (`resolve_default_frame` / `resolve_clip_frame` → `frame.rs:137` rehash per frame). Query path: `rust/crates/voxel-convert/src/object_query.rs:272-285` (`validate_snapshot` runs full `validate_voxel_object` on every window query before a cheap string compare).
- **What it does:** From JSON to admitted mesh: decode (2 hash passes per frame + content), canonicalize (2 per frame + content), resolve (1 per frame) = 5 SHA-256 passes per frame and 2 full-object JSON serializations, for objects of up to 8,192 frames / 16M voxels.
- **Why it looks pointless:** Same as #1: Engine-computed identities verified against themselves. The second validate inside `canonicalize_voxel_object` runs on a value that `decode_voxel_object` validated one call earlier. The test `rust/crates/voxel-asset/tests/voxel_object.rs:159-168` ("tampered") exists only to prove the rejection.
- **Cost:** per object load (hot for any scene with several animated objects), per window query, scales with frames × voxels.
- **Confidence:** High
- **Removal shape:** hash once in `with_computed_voxel_object_hashes`; drop `validate_frame_hash`/content-hash comparison from `validate_voxel_object`; make `resolve_*_frame` expand runs without validating; `object_query::validate_snapshot` keeps only the string compare.

### 3. Asset publication re-reads and re-hashes every staged file against a manifest built from the same bytes, then re-encodes the manifest and byte-compares it
- **Where:** `rust/crates/asset-import/src/publish.rs:291-322` (`stage_and_verify`: writes `file.bytes` at 297, then `fs::read` + `fingerprint_hex` + length compare at 303-313, then `encode_import_manifest` + byte compare at 314-320). The manifest's hashes were computed from the identical `artifact.bytes` at `rust/crates/asset-import/src/manifest.rs:163-165` moments earlier in `plan.rs`.
- **What it does:** Reads every output file back from disk and SHA-256s it to confirm the OS wrote what it was given; re-serializes the manifest to confirm JSON encoding is deterministic.
- **Why it looks pointless:** "Integrity check on data the Engine itself produced"; `VerificationFailed` can only fire on a disk fault that `fs::write` did not report. Deleting it leaves write + rename.
- **Cost:** once per publish; doubles I/O and hashes the entire output set (GLB resources can be tens of MB).
- **Confidence:** High
- **Removal shape:** delete the read-back/hash loop and the manifest re-encode; keep the staging-directory rename.

### 4. `project_catalog_material` re-validates the entire (already admitted) asset catalog, O(N²), on every material projection
- **Where:** `rust/crates/render-projection/src/material.rs:26-35` (consumer of this area, flagged here because the cost lives in `asset-catalog`); called from `rust/crates/csharp-engine-services/src/authored_content.rs:322` with `catalog.catalog()`, an `AdmittedAssetCatalog` that was validated at admission (`rust/crates/asset-catalog/src/admission.rs:14-28`). `validate_catalog` (`rust/crates/asset-catalog/src/validation.rs:267-366`) calls `catalog.get` per dependency and per surface, and `AssetCatalog::get` is a linear scan (`rust/crates/asset-catalog/src/catalog.rs:98-100`), then builds a `DependencyGraph` and runs cycle detection (`validation.rs:362-364`).
- **What it does:** Whole-catalog validation (duplicate scan, per-entry dependency resolution, atlas rect overlap O(R²), dependency graph + DFS) before projecting one material.
- **Why it looks pointless:** Exactly the ResourceList pattern from 2ee8375: the catalog can only enter the handle table through `AdmittedAssetCatalog::admit`, which already ran this validation. If deleted, projection behaves identically for every admitted catalog.
- **Cost:** per material projection call; O(N·D) linear lookups ≈ O(N²) for a catalog with N entries, plus graph allocation. Tests `rust/crates/render-projection/tests/voxel_surface_material_projection.rs:116-158` exist to prove the `InvalidCatalog` rejection.
- **Confidence:** High
- **Removal shape:** delete the `validate_catalog` call (or take `&AdmittedAssetCatalog`); separately, give `AssetCatalog::get` a map index so admission validation is O(N).

### 5. Annotation layer: two SHA-256 passes on every query, ~12 on every single edit command, 4 on every load
- **Where:** `rust/crates/voxel-annotation/src/query.rs:71-72` (`query_annotation_layer` calls `validate_annotation_layer` first); `rust/crates/voxel-annotation/src/codec.rs:87-118` (`validate_annotation_layer` recomputes `canonical_layer_hash` (clone + normalize + JSON + SHA-256) and `membership_data_hash` (clone + normalize + SHA-256) at 93-94 and compares); `codec.rs:185-198` (`normalize_and_rehash`: `set_hashes` computes both, then `validate_annotation_layer` recomputes both); `rust/crates/voxel-annotation/src/edit.rs:109-110` (edit validates the current layer first) and `:134-135` (normalize_and_rehash on the candidate); `codec.rs:59-85` (`finalize_annotation_draft`: `set_hashes` then validate); `codec.rs:160` (decode validates), `:123` (encode validates), `:176` (export → encode → validate). Consumer doubles it: `rust/crates/csharp-engine-services/src/voxel_content.rs:544,555` applies each edit transaction twice (preflight on a clone, then for real) and `:1461-1470,1501-1509` validates again right after `decode_annotation_layer` already validated.
- **What it does:** Every entry point clones and re-hashes the whole layer (up to 4,096 regions × 16,384 runs) to compare against the stored hash strings.
- **Why it looks pointless:** Engine-computed identities verified against themselves; the `expected_layer_hash` compare-and-swap at `query.rs:81-88` / `edit.rs:111-116` is the only check that carries information and it is a string compare. Tests `rust/crates/voxel-annotation/tests/voxel_annotation.rs:55-83,90-108,166` exist to prove the mismatch rejections.
- **Cost:** per query (hot: product queries annotations per cell/bounds), per edit, per load.
- **Confidence:** High
- **Removal shape:** hash once in `set_hashes`; drop hash recomputation from `validate_annotation_layer`; query/edit skip validation and keep the string CAS; services apply once.

### 6. `SceneEditService::apply` validates the whole scene before and after every edit and clones the document twice
- **Where:** `rust/crates/authored-scene/src/edit.rs:173-178` (`validate_scene` on the document that the previous `apply` validated at 296-301), `:192` (`document.canonical()` full clone + sort), `:296-301` (validate after), `:186` and `:305` (`SceneObjectSnapshot::from_document` clones + canonicalizes the whole document again just to list node ids). `validate_scene` (`rust/crates/authored-scene/src/validation.rs:305-519`) builds several `BTreeSet`s over all nodes, `composed_world_transforms` for every node (498), and `detect_cycles` (488). Same pattern in `rust/crates/environment-authoring/src/materialization.rs:162-171` (validate before), `:467-476` (validate after) and `:180` (`encode_scene` validates a third time, `rust/crates/authored-scene/src/codec.rs:204`).
- **What it does:** Full-document validation + two full clones per single-node edit (rename, set transform, select).
- **Why it looks pointless:** "Whole-collection validation per call where only a delta changed." The before-validation can only fail if a previous after-validation lied. Deleting the before pass changes nothing for any document the service produced.
- **Cost:** per edit command, O(N nodes) × several passes; `Select` (edit.rs:182-192) does a full snapshot clone for no mutation.
- **Confidence:** High
- **Removal shape:** drop `InvalidBefore`; validate only the touched node(s) after an edit; return node lists without cloning the document.

### 7. `DuplicateTag` / `DuplicateLabel` refusals for lists the Engine silently dedups anyway
- **Where:** `rust/crates/authored-scene/src/validation.rs:390-399` (`DuplicateTag` error) vs `rust/crates/authored-scene/src/model.rs:231-232` (`canonicalize` does `tags.sort(); tags.dedup()`); `rust/crates/entity-state/src/definition.rs:179-187` (`DuplicateLabel` error) vs `rust/crates/entity-state/src/model.rs:612-615` (`insert_definition` does `labels.sort_unstable(); labels.dedup()`); `rust/crates/authored-scene/src/validation.rs:328-335` (`DuplicateAssetDependency`) vs `rust/crates/authored-scene/src/edit.rs:363-372` (`reconcile_dependencies` rebuilds and dedups the list itself).
- **What it does:** Rejects a document/definition because a tag, label or dependency appears twice, although the very next step dedups.
- **Why it looks pointless:** "Duplicate guards where a map insert would do"; the canonical form is already deduped by construction. Deleting the refusal makes the input accepted and canonicalized.
- **Cost:** per validate (so per edit, per admit, per decode); builds a `BTreeSet` per node.
- **Confidence:** High
- **Removal shape:** delete the three error variants and their checks; keep the dedup.

### 8. `replace_character_motion_state` re-validates values the Engine's own character controller just computed, on every character step
- **Where:** `rust/crates/entity-state/src/character_motion_publication.rs:105-144`: seven precondition lookups at 110-139 (entity exists/active, transform present, motion present, no Kinematic, no RigidBody, no transform parent, unit scale), `transform_is_valid` at 140-141 and `validate_character_motion` (≈15 finite checks, `rust/crates/entity-state/src/character_motion.rs:46-88`) at 143-144, then `RevisionScopeMismatch` at 152-158. Sole caller: `rust/crates/engine-spatial/src/character_controller.rs:2187` publishing its own `prepared.solution`.
- **What it does:** Checks that the controller's output is finite/unit-quaternion and that the `ComponentRevision` guards (constructed by `component_revision::<T>(entity)`, `rust/crates/entity-state/src/model.rs:279-290`) name the same entity/type they were created for.
- **Why it looks pointless:** All inputs are Engine-produced two frames of call stack earlier; the scope check double-checks a by-construction invariant of the `ComponentRevision` type. If removed, nothing changes for the controller; a NaN would already have been caught where the solver produced it (or not — but validating the publication does not fix the solver).
- **Cost:** per character per simulation step (hot path); ~25 branches and 6 map lookups each.
- **Confidence:** High (the scope check and the seven preconditions), Medium (finite checks)
- **Removal shape:** keep the two revision-equality CAS compares; delete the rest or demote to `debug_assert!`.

### 9. Importer re-hashes the source twice and re-validates the artifacts it just built
- **Where:** `rust/crates/asset-import/src/animated_glb.rs:106` (`sha256_hex(source)`), `:139` (passes it as `expected_source_sha256`), then `rust/crates/voxel-convert/src/source.rs:207-208` (`source_sha256(&request.source_bytes)` and `validate_expected_source_hash` compare the two). `rust/crates/asset-import/src/importer.rs:264` (`static_mesh.validate()` on a `StaticMeshAsset` the importer just assembled from validated streams), `:372` (`validate_catalog` on a catalog built at 292-353 from `push_unique` entries with hashes the importer computed); `rust/crates/asset-import/src/animated_glb.rs:295,310` (same two re-validations); `:304-305` (`AssetHash::parse(&source_hash_hex).expect(..)` re-parsing its own hex).
- **What it does:** Hashes the same byte slice twice to compare the results; runs the catalog validator over entries whose ids, kinds and dependencies the importer constructed.
- **Why it looks pointless:** "Re-validating the produced asset after import." The only possible failures are importer bugs, which `debug_assert!` would surface in tests.
- **Cost:** once per import (offline), but the second SHA-256 is over the full GLB.
- **Confidence:** High
- **Removal shape:** pass `expected_source_sha256: None` from the importer (or drop the field) and delete the post-build `validate()`/`validate_catalog` calls.

### 10. Conversion "apply" re-derives three SHA-256 identities of a plan the planner built in-process and compares ~16 of its own fields
- **Where:** `rust/crates/voxel-convert/src/planning.rs:416-450` (`validate_prepared_identity`: recomputes `conversion_plan_hash` (JSON + SHA-256), `conversion_plan_id` (SHA-256), `plan_settings_sha256` (JSON + SHA-256) and compares them and 13 other fields against the `PreparedVoxelConversion` that `plan_conversion` (`:140-194`) filled from the same values); `:254` (`validate_voxel_asset` on the output = finding #1 again); `rust/crates/voxel-convert/src/object_conversion/planning.rs:623-650` (same, for objects). Test `rust/crates/voxel-convert/tests/conversion_authority.rs:215-227` ("forged_plan") exists to prove the rejection.
- **What it does:** Anti-tamper verification of an in-memory struct against itself.
- **Why it looks pointless:** The request's `plan_id`/`expected_plan_hash` string compare (the CAS, 2 lines) is the only input from outside; the rest is "adversarial" ceremony on Engine-built data. Deleting it leaves the CAS.
- **Cost:** per conversion apply (offline tooling): 3 JSON serializations + 3 SHA-256 + full asset rehash.
- **Confidence:** High
- **Removal shape:** keep `plan_id == prepared.plan.plan_id && expected_plan_hash == prepared.plan.plan_hash`; delete the rest.

### 11. Entity-state refuses idempotent operations as errors instead of no-ops
- **Where:** `rust/crates/entity-state/src/activation.rs:151-157` (`AlreadyInState`: enabling enabled collision is an error); `rust/crates/entity-state/src/authoring.rs:576-581` (`enable` on an active entity → `InvalidLifecycleTransition`), `:252-253` (`DuplicateLabel`), `:275-277` (`MissingLabel`), `:291-300` (`ComponentAlreadyPresent`); `rust/crates/entity-state/src/relationship.rs:282-286,299-303,311-315` (`NoSuchRelation` on clearing an absent relation); `rust/crates/entity-state/src/component.rs:322-347` (`DuplicateStableId`/`IncompatibleCodec` on re-registering the identical component registration).
- **What it does:** Forces the product to query state before every set and to handle an error for "already true".
- **Why it looks pointless:** "`already registered` refusals where last-write-wins or a map insert would do." Trusted product code gains nothing from the error; a no-op receipt (`changed: false`) is what `replace_component` already returns at `authoring.rs:330-336`.
- **Cost:** per call (cheap), but it leaks into every C# call site as a query-then-set pair.
- **Confidence:** High
- **Removal shape:** return a receipt with `revision_after == revision_before` instead of `Err`.

---

## MEDIUM

### 12. Magnitude / count / string-length caps in voxel-asset with no concrete failure behind them
- **Where:** `rust/crates/voxel-asset/src/codec.rs:11-14` (`MAX_ARTIFACT_BYTES` 16 MiB checked at `:57` and `:101`, `MAX_REPRESENTED_VOXELS` 1M at `:456-462`, `MAX_MATERIAL_MAPPINGS` 4095, `MAX_STRING_BYTES` 4096 at `:534-547`), `:270-281` (origin within ±1,000,000), `:438-454` (every run endpoint × 3 axes re-mapped and bounded to ±1,000,000), `:263-268` (`chunkSize` 1..=64); `rust/crates/voxel-asset/src/frame.rs:11,289-302` (same ±1M on frame coordinates); `rust/crates/voxel-asset/src/object_codec.rs:13-22` (`MAX_VOXEL_OBJECT_*`: 256 clips, 4,096 frames/clip, 8,192 total, 16M voxels, 240 fps, 60 s duration, 64 anchors, 32 hit regions, ±1M local coordinates at `:498-517`); `rust/crates/voxel-asset/src/conversion.rs:12-13` (`MAX_CONVERSION_RESOLUTION_AXIS` 2,000,001; `MAX_CONVERSION_CELLS` 8×10¹⁸ — unreachable because `checked_mul` already guards overflow), `:209-239`; `rust/crates/voxel-annotation/src/model.rs:15-26` (`VoxelAnnotationLimits`), `rust/crates/voxel-annotation/src/codec.rs:13,132,142` (`MAX_ANNOTATION_BYTES` 32 MiB on encode and decode), `rust/crates/voxel-annotation/src/edit.rs:11,120-125` (`MAX_ANNOTATION_COMMANDS_PER_TRANSACTION` 256).
- **What it does:** Refuses artifacts and requests that exceed round-number quotas; the per-run address check alone is 6 `checked_add` + compares per run.
- **Why it looks pointless:** "Sanity limits with no concrete failure." The real cost limits live where the allocations happen (`voxel-object-runtime::VoxelObjectRuntimeLimits`, `svc_mesh::SurfaceMeshLimits`, `MAX_GEOMETRIC_VOXELIZATION_WORK`). A 1,000,001-voxel asset would mesh fine; a 61-second frame would play fine.
- **Cost:** per validate (so multiplied by findings #1/#2/#5); the address bound is O(runs).
- **Confidence:** Medium
- **Removal shape:** delete the schema-level quotas; keep representation checks (`length > 0`, overflow) and the runtime allocation limits.

### 13. `validate_catalog` policy and magnitude checks on authored material/atlas data
- **Where:** `rust/crates/asset-catalog/src/material.rs:3-8` (`MAX_ATLAS_REGIONS` 1024, `MAX_AGGREGATE_ATLAS_REGIONS` 4096, `MAX_ATLAS_PADDING` 32, `MIN/MAX_TILE_SCALE_CELLS`, `MAX_TILE_ORIGIN_CELLS` 16,777,216); `rust/crates/asset-catalog/src/validation.rs:358-360` (aggregate quota), `:596-600` (per-atlas quota), `:641-654` (padding cap), `:561-582` (tile scale/origin range), `:655-668` (`InsufficientAtlasPadding`: linear-filtered atlas with zero padding refused — it would merely bleed at edges), `:677-687` (`AtlasRegionOverlap`, O(R²) rect scan — overlapping regions render fine), `:690-698` (`valid_region_id`: ≤64 bytes lowercase-kebab), `:488-492,590` (`schema_version != 1` on surface/atlas), `:464-469` (`UnpinnedSurfaceReference`: surface refs must carry exact version AND hash), `:470-479` (`UndeclaredSurfaceDependency`: the same reference must also be repeated in `dependencies`), `:327-333` (`DuplicateDependency`), `:272-278` (`DuplicateAssetId`, while `get` at `catalog.rs:98` returns the first match anyway).
- **What it does:** Policing authored content shape beyond what any consumer needs; `UndeclaredSurfaceDependency` forces authors to write every reference twice.
- **Why it looks pointless:** "Defensive bounds where the value would merely render oddly"; "duplicate guards where first/last-wins would do"; `UnpinnedSurfaceReference` is a compatibility-pin requirement in a same-build pair.
- **Cost:** per admission and, via #4, per material projection; the overlap check is quadratic in regions.
- **Confidence:** Medium
- **Removal shape:** keep kind/dependency-resolution errors (`WrongKindReference`, `UnknownDependency`, `MissingAtlasRegion`, cycle); delete the quota, padding, overlap, id-grammar, schema-version, pin and duplicate checks.

### 14. `finite()` checks on JSON-decoded floats are unreachable, and the same values are finite-checked three times
- **Where:** `rust/crates/asset-catalog/src/codec.rs:431,440` (`finite` on roughness/emissive at decode), `:619-650` (`finite_pair` × 4 and cutoff), `:802-811` (`rgba_from_array` → `ensure_rgba_finite`), helpers `:827-840`; encode-side `:376-392`; then `rust/crates/asset-catalog/src/validation.rs:494,567-568,575-576` checks `is_finite` on the same fields again.
- **What it does:** Rejects NaN/∞ after `serde_json` parsing.
- **Why it looks pointless:** JSON has no NaN/∞ token; `serde_json` cannot produce one, so the decode-side branches are dead. The encode-side check only improves on serde_json writing `null` for an Engine-built NaN (Low). The validation-time repeat is redundant with both.
- **Cost:** per decode/admission; negligible CPU, but ~60 lines of dead code.
- **Confidence:** Medium (decode: dead by construction), Low (encode)
- **Removal shape:** delete decode-side `finite*` helpers and the `is_finite` arms in `validate_*`.

### 15. Manifest and sidecar are validated at encode of a freshly built value, at decode of an Engine-written file, and again before publish
- **Where:** `rust/crates/asset-import/src/manifest.rs:48-90` (`validate`: schema version, non-zero versions, `AssetId::parse`, hash hex grammar, `is_safe_relative_path` + duplicate-path scan on paths the importer formatted at `rust/crates/asset-import/src/artifact.rs:33,37,60,64`), called at `:183` (encode) — from `rust/crates/asset-import/src/plan.rs:117-118,236-237,379-380` with `.expect("newly built import manifest is valid")` — at `:231` (decode), and again at `rust/crates/asset-import/src/publish.rs:317`; `rust/crates/asset-import/src/publish.rs:93-104` (path grammar + duplicate scan on the same importer-built paths) and `:324-344` (`validate_file_closure`); `rust/crates/asset-import/src/sidecar.rs:325` (encode validates), `:392` (decode validates), `:396-463` (`validate_sidecar`: hash grammar ×2, path grammar, duplicate paths, `is_scoped_key` labels); `plan.rs:134-141` builds `sidecar_update` from a decoded (validated) sidecar and then `encode_sidecar` validates it again.
- **What it does:** Path-traversal and duplicate guards on paths the Engine formatted; schema/hash grammar checks on files the Engine wrote.
- **Why it looks pointless:** "Path traversal refusals on paths the SDK built itself"; "schema re-validation at multiple stages." A publish can never reach `UnsafeRelativePath` or `DuplicatePath` with importer output.
- **Cost:** once per import; cheap but ~150 lines.
- **Confidence:** Medium
- **Removal shape:** validate on decode only (one place), or not at all; delete `is_safe_relative_path` and the duplicate scans in publish/encode paths.

### 16. Scene validator enforces per-node "requires-schema-N" gates although edits auto-upgrade the version
- **Where:** `rust/crates/authored-scene/src/validation.rs:372-376` (`RequiresSchema5`), `:428-431` (`RequiresSchema2`), `:445-450` (`requires-schema-4`), `:529-534,560-565,605-610` (`requires-schema-3/4`), `:307-318` (schema range + `AuthoringVersionAheadOfSchema`); vs `rust/crates/authored-scene/src/edit.rs:374-396` (`upgrade_schema_for_kind` bumps both versions whenever a node kind needs it).
- **What it does:** Refuses a document whose stored `schemaVersion`/`authoringFormatVersion` is lower than the newest node kind in it.
- **Why it looks pointless:** "Compatibility/version negotiation between Engine and product (same build pair)"; the Engine both writes and reads the format and already auto-upgrades on edit. A decoded older document could just be treated as the current schema.
- **Cost:** per validate; small.
- **Confidence:** Medium
- **Removal shape:** keep one `schema_version <= CURRENT` check at decode; delete the per-kind gates and `authoring_format_version` entirely.

### 17. `replace_voxel_palette` double CAS plus post-condition that cannot fail
- **Where:** `rust/crates/voxel-asset/src/palette.rs:43-48` (expected `content_hash` compare), `:49-54` (expected `voxel_data_hash` compare — implied by the content hash), `:63` (`with_computed_content_hash` = finding #1, two hash passes + validate), `:64-69` (`VoxelDataChanged` — the function only replaced `material_palette`, so the voxel hash cannot change).
- **What it does:** Three guards for one palette swap.
- **Why it looks pointless:** Second CAS is redundant with the first; the post-condition double-checks an invariant of the code three lines above.
- **Cost:** per palette edit; 2 full hashes + whole-asset clone.
- **Confidence:** Medium
- **Removal shape:** keep `expected_content_hash` compare; recompute only `content_hash`; delete `VoxelDataChanged`.

### 18. `AssetCatalogStore` is a revision-counting wrapper with no non-test caller
- **Where:** `rust/crates/asset-catalog/src/admission.rs:49-80` (`AssetCatalogStore`, `replace` re-admits the whole catalog and increments a `u64` with `RevisionExhausted` on overflow); only use: `rust/crates/asset-catalog/tests/asset_catalog.rs:493`.
- **What it does:** Nothing the Engine uses; `csharp-engine-services` keeps `AdmittedAssetCatalog`s in its own handle map (`authored_content.rs:38`).
- **Why it looks pointless:** Dead mechanism; `RevisionExhausted` guards a counter that would take 5×10¹¹ years to overflow.
- **Cost:** none at runtime; maintenance.
- **Confidence:** Medium
- **Removal shape:** delete `AssetCatalogStore` and its test.

### 19. Variant overrides are validated at registry admission and again on every `resolve_prefab`
- **Where:** `rust/crates/authored-scene/src/prefab.rs:627-717` (`validate_variant_delta`: role existence, removed-part safety, duplicate overrides, `validate_override_value` against context) inside `ValidatedPrefabRegistry::new`; then `rust/crates/authored-scene/src/prefab_resolution.rs:144-235` (`apply_override` re-checks the same kinds/asset-ids/context for the *variant* overrides it iterates at `:126`) and `:117-125` (duplicate scan of instance overrides). `resolve_prefab` also finds definitions by linear scan (`:60-64,68-73`).
- **What it does:** Re-validates authored variant overrides every time a prefab instance is resolved.
- **Why it looks pointless:** `ValidatedPrefabRegistry` is a newtype whose whole purpose is "already validated"; only instance overrides are new input.
- **Cost:** per prefab instantiation; O(overrides) with `AssetId::parse` per override.
- **Confidence:** Medium
- **Removal shape:** apply variant overrides unchecked; validate only instance overrides.

### 20. Snapshot restore validates Engine-written entity snapshots for shapes the encoder cannot produce
- **Where:** `rust/crates/entity-state/src/snapshot.rs:617-636` (`validate_tombstone_shape`: a tombstone carrying components), `:320-330` (relationship targets a tombstone), `:316-318` (duplicate entity id), `:412-414,419-450` (`validate_restored_motion_authority` scans every entity for kinematic/rigid-body/character conflicts); `rust/crates/entity-state/src/component.rs:233-250` (`UnknownEntity`, `TombstonedEntity`, `DuplicateEntityValue` per restored component value), `:267-273` (validator per value). Tests proving the rejections use hand-"forged" snapshots: `rust/crates/entity-state/tests/rigid_body.rs:246-265`, `rust/crates/entity-state/tests/character_motion.rs:31-75`.
- **What it does:** Re-derives invariants that `snapshot_filtered` (`snapshot.rs:197-287`) enforces by construction (tombstones are written without components; conflicts cannot exist in a live state).
- **Why it looks pointless:** "Schema re-validation of data the Engine produced"; `lib.rs:9` says these are not the product's persistence format, so the only writer is the Engine.
- **Cost:** per restore, O(entities × component kinds).
- **Confidence:** Medium
- **Removal shape:** delete the shape/conflict scans; keep decode errors and the per-value codec decode.

### 21. `MAX_COMPONENT_REPLACEMENTS = 32` caps a batch primitive
- **Where:** `rust/crates/entity-state/src/authoring.rs:19`, `:357-362`, `:429-434`.
- **What it does:** Refuses `replace_components` / `replace_root_transforms_for_local_frame` with more than 32 entries.
- **Why it looks pointless:** "Max-count limits with no concrete failure"; a 33-entity world-origin rebase must be split by the caller for no reason.
- **Cost:** per call; the real cost is the product-side workaround.
- **Confidence:** Medium
- **Removal shape:** delete the constant and both checks.

### 22. Relationship commands copy the whole containment map per call and scan all parents on destroy
- **Where:** `rust/crates/entity-state/src/relationship.rs:290-295` (`SetContainment` collects `state.containment` into a fresh `BTreeMap` just to run `ensure_acyclic`), `:246-261` (`reroot_transform_children` scans every `transform_parents` entry), `rust/crates/entity-state/src/model.rs:656` (`transform_parents.retain` scan per destroy).
- **What it does:** O(N) copies/scans for a check that could walk the existing map.
- **Why it looks pointless:** Whole-collection work per call for a per-id question (`ensure_acyclic` only needs `get`).
- **Cost:** per containment set / per destroy / per transform detach.
- **Confidence:** Medium
- **Removal shape:** pass `&state.containment` directly; keep a reverse index for transform children like `containment_children`.

### 23. `EntityAuthoringService::admit` clones the whole world to stage definitions
- **Where:** `rust/crates/entity-state/src/authoring.rs:182` (`state.clone()`), `:199` (`commit` swaps the clone in).
- **What it does:** Copies every entity and component table so that a failed relationship validation at `:197` leaves the original untouched.
- **Why it looks pointless:** "Rollback mechanism with no observed failure"; all definition validation (`:185-190`) is already done before any mutation, and `validate_command` for relationships (`relationship.rs:263-324`) is a pure check that could run up front too.
- **Cost:** per admission, O(world).
- **Confidence:** Medium
- **Removal shape:** validate all relationships first, then mutate in place.

### 24. `generate_tunnel` hashes its voxels with a second implementation of the run-length hash that `with_computed_content_hash` recomputes anyway
- **Where:** `rust/crates/environment-authoring/src/generation.rs:296-330` (`hash_voxels`: sort, run-length encode, SHA-256 — a copy of `voxel-asset/src/frame.rs:475-515`), `:261-263` (stored as `voxel_data_sha256`); `rust/crates/environment-authoring/src/materialization.rs:402` (`with_computed_content_hash` computes the same hash from the same cells) then `:177` (`encode_voxel_asset` rehashes again); the test `rust/crates/environment-authoring/tests/environment_authoring.rs:93-94` asserts the two implementations agree.
- **What it does:** Two run-length SHA-256 implementations kept in sync by a test; three hash passes per materialization.
- **Why it looks pointless:** Identity computed twice by two code paths; the provenance could copy `asset.voxel_data_hash`.
- **Cost:** per materialization (one-off), O(voxels log voxels) ×3.
- **Confidence:** Medium
- **Removal shape:** delete `hash_voxels`; fill provenance from the asset's hash.

### 25. Environment materialization validates targets, limits and recipes that only it produces
- **Where:** `rust/crates/environment-authoring/src/materialization.rs:307-335` (`enforce_limits` with `requested.min(ceiling)`; `MAX_GENERATED_MARKERS` 128 at `:24` while the generator emits exactly two markers at `generation.rs:233-241`), `:342-352` (sparse-run cap 65,536), `:222-305` (`validate_target`: marker-id uniqueness, exact `{"exit_hint","player_start"}` source set, node-kind compatibility, `RecipeMismatch`), `rust/crates/environment-authoring/src/generation.rs:392-402` (`DuplicateMaterial`: wall/floor/accent slots must differ), `:333-342,356-361` (`voxel_size` checked as f64 and again after `as f32`).
- **What it does:** Quota and policy checks on a generator with one preset whose outputs are fully determined by `TunnelGeneratorConfig`.
- **Why it looks pointless:** Caps that cannot trigger (markers), policy that is product tuning (same material for wall and floor is a legitimate look), limits on the Engine's own output.
- **Cost:** per materialization (one-off).
- **Confidence:** Medium
- **Removal shape:** delete `enforce_limits`, `MAX_GENERATED_MARKERS`, `DuplicateMaterial`, the f32 re-check; keep "target node ids exist" checks.

### 26. `VoxelObjectPlayer::sample_at` re-validates the posture every sample
- **Where:** `rust/crates/voxel-object-runtime/src/player.rs:270` (`validate_posture` in `sample_at`), `:317-331` (rate non-zero, status/clip consistency, `require_clip` linear scan over clips at `:333-342` → `model.rs:129-131`), while `play`/`scrub`/`restore` already validated (`:255-256,283,318`).
- **What it does:** Per-frame re-check of a posture only the player mutates, plus a linear clip lookup.
- **Why it looks pointless:** Invariant held by construction; `sample_at` then calls `require_clip` a second time at `:284`.
- **Cost:** per object per frame (hot).
- **Confidence:** Medium
- **Removal shape:** delete `validate_posture` from `sample_at`; cache the clip index in the posture.

### 27. `portable_relative_path` traversal refusal on paths that only index a content-store map
- **Where:** `rust/crates/asset-catalog/src/portable.rs:328-340` (rejects `..`, `.`, `/`-prefix, `\\`, `:`), called at `:258,265`; consumer `rust/crates/csharp-engine-services/src/content/portable.rs:68-72` builds `format!("{directory}{path}")` and does `source.files.get(&full)` (a lookup by key, not a filesystem open). Also `:194-206` (`asset()` scans the entire asset list twice per lookup to detect ambiguity, and `resolve` calls it per dependency).
- **What it does:** Path-traversal defence for a logical key; per-lookup ambiguity scan.
- **Why it looks pointless:** A `..` path misses the map and yields "missing file"; nothing escapes. Duplicate ids: first match would do.
- **Cost:** per portable asset load; O(assets × dependencies).
- **Confidence:** Medium
- **Removal shape:** delete `portable_relative_path`; replace `asset()` with a single `find`.

---

## LOW

### 28. Per-command finite checks on product-supplied translations/velocities/transforms
- **Where:** `rust/crates/entity-state/src/command.rs:176` (`translation_is_valid`), `:213` (`velocity_is_valid`); `rust/crates/entity-state/src/transform.rs:106` (`transform_is_valid` incl. unit-quaternion tolerance); `rust/crates/entity-state/src/rigid_body.rs:68-127` (`validate_rigid_body`: finite/positive ranges, `LockedTranslationAxisVelocity`/`LockedRotationAxisVelocity` at `:102-107` — velocity on a locked axis is refused rather than ignored, `EmptyCollisionGroups` at `:123-125`); `rust/crates/authored-scene/src/light.rs:85-90` (light colour must be within 0..=1 — HDR colours refused), `:105-111` (spot angle ≤ π/2), `:111-113` (penumbra 0..=1).
- **What it does:** Range policing of trusted product values.
- **Why it looks pointless:** "Would merely render oddly" (lights, groups=0, locked-axis velocity); NaN translations are the exception — they poison spatial queries, which is a concrete failure, so those stay.
- **Cost:** per command; cheap.
- **Confidence:** Low (keep NaN checks; drop ranges/policy)
- **Removal shape:** keep `is_finite` on transforms/velocities; delete colour/angle/penumbra ranges, locked-axis refusal, `EmptyCollisionGroups`.

### 29. Duplicate `max_frames` cap in runtime limits
- **Where:** `rust/crates/voxel-object-runtime/src/model.rs:26` (`max_frames: 8_193`), `rust/crates/voxel-object-runtime/src/admission.rs:107-119`; already enforced by `object_codec.rs:286-293` (`MAX_VOXEL_OBJECT_TOTAL_FRAMES` 8,192).
- **Why it looks pointless:** Same bound twice, one off by one. The other `VoxelObjectRuntimeLimits` (faces/vertices/indices/sampled cells/bytes) are real allocation bounds and look legitimate.
- **Confidence:** Low
- **Removal shape:** delete `max_frames`.

### 30. `GlobalPosition` deserialization rejects non-canonical cell/offset pairs it could just normalize
- **Where:** `rust/crates/core-space/src/origin.rs:31-52` (custom `Deserialize` calls `Self::new`, which canonicalizes, then errors at `:45-49` if the stored pair was not already canonical).
- **Why it looks pointless:** The Engine serializes canonical values; `new` already produces the canonical form, so returning it is strictly more useful than refusing.
- **Cost:** per decode; trivial.
- **Confidence:** Low
- **Removal shape:** return `value` without the equality check.

### 31. String-length caps and id-grammar checks on request fields the Engine/CLI builds
- **Where:** `rust/crates/voxel-convert/src/source.rs:13-15,403-415` (`MAX_MESH_SOURCE_ASSET_ID_BYTES` 1024, `MAX_MESH_SOURCE_PATH_BYTES` 8192, `MAX_MESH_PRIMITIVE_BYTES` 1024), `:423-431` (hex grammar of `expected_source_sha256`); `rust/crates/voxel-asset/src/conversion.rs:117-123` (same grammar check); `rust/crates/voxel-convert/src/object_conversion/validation.rs:45-51` (target id length cap followed by `AssetId::parse`, whose grammar already bounds it); `rust/crates/entity-state/src/component/registration.rs:7-10,407-427` (`MAX_COMPONENT_TYPE_ID_BYTES` 128, `MAX_REGISTERED_COMPONENT_TYPES` 256 at `component.rs:348-352`, identity grammar on constants the product declares).
- **Why it looks pointless:** "Max-string-length limits with no concrete failure"; the type-count cap refuses a 257th product component.
- **Confidence:** Low
- **Removal shape:** delete the caps; keep `AssetId::parse`.

### 32. Annotation query refuses `max_results` above the region quota instead of clamping
- **Where:** `rust/crates/voxel-annotation/src/query.rs:73-80` (`InvalidLimit` when `max_results == 0 || > 4096`).
- **Why it looks pointless:** The result is truncated anyway at `:122-123`; clamping is the obvious behaviour.
- **Confidence:** Low
- **Removal shape:** `min(max_results, regions.len())`.

### 33. `MAX_ANIMATED_GLB_TEXTURE_TRANSFORM_COMPONENT = 1e6` magnitude cap on glTF UV transforms
- **Where:** `rust/crates/asset-import/src/animated_glb.rs:35`, `:1127-1133`.
- **Why it looks pointless:** Import-time third-party input (legitimate place to validate), but this specific cap has no failure behind it — a UV offset of 2e6 renders with precision loss, nothing more. The `is_finite` check beside it is the legitimate part.
- **Confidence:** Low
- **Removal shape:** delete the magnitude branch.

### 34. `validate_sparse_runs` requires declared bounds to equal computed bounds, and sorts every row to detect overlaps
- **Where:** `rust/crates/voxel-asset/src/codec.rs:463-476` (row sort + overlap → `duplicateVoxel`), `:477-490` (declared `bounds != actual_bounds` → error); same in `rust/crates/voxel-asset/src/frame.rs:311-324,329-338`; `rust/crates/voxel-annotation/src/codec.rs:405-443`.
- **Why it looks pointless:** Bounds are derivable (the code derives them to check); overlapping runs with the same material are harmless after `canonicalize_sparse_runs` merges them (`frame.rs:475-501`). A representation concern only for mixed-material overlap, which last-run-wins would resolve.
- **Cost:** O(runs log runs) per validate (multiplied by #1/#2).
- **Confidence:** Low
- **Removal shape:** compute bounds instead of comparing; drop overlap detection or resolve last-wins in canonicalize.

---

## Looked at and judged legitimate

- `core-assets` `AssetId::parse` / `AssetHash::parse` / `AssetRef::new` kind check: grammar enforced once at construction of a typed id; the type carries the guarantee afterwards.
- `core-space` `SpatialGridSpec::new` / `VoxelGridSpec::new` finite+positive cell size, `ChunkDims::new` non-zero: prevent division by zero / NaN→int casts in every conversion.
- `core-space` `GlobalPosition::new` overflow and finite checks, `local()` envelope check: representation limits of i64/f32.
- `core-voxel` `MaterialCatalog::validate`, `VoxelState::from_raw` 15-bit bound, `from_encoded` tripwire: representation.
- `core-time`, `core-math`, `core-ids`: nothing to report.
- `asset-catalog` `resolve_reference` version/hash pin matching (`catalog.rs:245-269`) and `admission.rs` `resolve_asset`: reference resolution, the feature itself.
- `asset-catalog` `lock.rs` `validate_lock`: a drift-detection authoring tool; its whole purpose is comparing.
- `asset-catalog` `AdmittedAssetCatalog::admit` canonical JSON + SHA-256 (`admission.rs:14-28`): the hash is exposed to the product as the catalog identity (`authored_content.rs:1079`), computed once per admission. (Its validation pass is covered by #13; its re-run at projection by #4.)
- `asset-catalog` `VoxelMaterialTable::validate_used`: explicit opt-in report of unresolved slots.
- `asset-import` `source.rs` parse (schema, unsupported features, collision labels), `importer.rs:541-666` stream-length/index/finite checks, `push_unique` duplicate-id (two source materials with the same name do collide), `gltf_package.rs` resource closure checks and `animated_glb.rs` glTF validation: first parse of third-party input at import.
- `asset-import` `source.rs:172-193` `MAX_SOURCE_VERTICES = u32::MAX`: prevents the `as u32` truncation at `:226`; practically unreachable but representation-shaped.
- `asset-import` `publish.rs` staging-dir rename + backup restore: atomic replace is the feature; only the hash read-back (#3) is excess.
- `asset-import` `plan_reimport` / `detect_source_drift` / sidecar `reconcile`: comparing a stored hash to a freshly hashed *source file* is the purpose of a sidecar.
- `authored-scene` `validate_scene` structural errors (`DuplicateNodeId`, `UnknownParent`, `Cycle`, `AssetKindMismatch`, `UnknownSpawnMarker`, `InvalidVoxelVolumeTransform` non-identity rotation/scale): representation constraints consumers rely on.
- `authored-scene` `SceneAdmissionPlan::prepare` single validate+canonicalize at admission; `resolve_references` against available assets.
- `authored-scene` `SceneEditService` `StaleRevision` and `voxel-annotation`/`voxel-asset`/`entity-state` `expected_*` revision / hash compare-and-swap string compares: optimistic concurrency, cheap and informative.
- `authored-scene` `prefab.rs` registry validation at `ValidatedPrefabRegistry::new` (once); only its repetition in resolution (#19) is excess.
- `voxel-convert` `validate_source` / `validate_request` identity equality between request and imported snapshot (`planning.rs:324-346`, `object_conversion/validation.rs:20-27`), `animation/sample.rs:393-405` `validate_source_identity`: string compares binding a request to a snapshot.
- `voxel-convert` `validate_transform` / `validate_affine_matrix` / `transform_position` finite checks: a NaN matrix would corrupt every vertex.
- `voxel-convert` geometry/animation import limits (`animation.rs:18-30`, `MAX_GEOMETRIC_VOXELIZATION_WORK`, `MAX_CONVERSION_TEXTURE_TEXELS`, `validate_triangles`, `validate_closed_topology`): bounded work on third-party glTF at import.
- `voxel-convert` `store.rs` pending-file + rename: atomic install.
- `voxel-object-runtime` `VoxelObjectRuntimeLimits` (faces, vertices, indices, sampled cells, field bytes, partitions) and `SurfaceMeshLimits` plumbing: allocation bounds at the point of allocation.
- `voxel-object-runtime` `collision.rs` `ExternalProxy` id kind check, `player.rs` `TimeMovedBackwards`/`TimeOverflow`: cheap representation checks on caller-supplied clocks.
- `entity-state` `ComponentRevision` stale compares (`ensure_component_revision` revision equality), `ensure_revision`, `ensure_alive`, `StaticColliderMovement` (active static collider cannot move — spatial invariant), `validate_detach` (kinematic requires transform): invariants other services depend on.
- `entity-state` `rebase_revisions_from` table-key equality: cheap guard on an in-process restore primitive.
- `entity-state` `MAX_COMPONENT_INSPECTION_ENTITIES` sample truncation: a readout bound, not a refusal.
- `environment-authoring` `validate_config` `TooSmall` / `ExceedsChunk`: generator preconditions it would otherwise index out of range on.
- `scripts/inventory-validation.py`: a heuristic grep-survey tool that writes a review queue; it performs no validation and excludes itself. No Rust mirrors it.

## Coverage

**Read in full:** `asset-catalog/src/{lib,validation,catalog,admission,lock,change,dependency,fallback,material,voxel,codec,portable}.rs`; `asset-import/src/{lib,fingerprint,artifact,manifest,publish,plan,sidecar,importer,source,diagnostic,bin/rusty_asset_import}.rs`; `core-assets/src/lib.rs` (body; doc comments skipped); `authored-scene/src/{validation,admission,model,codec,prefab_codec,light,edit,prefab,prefab_resolution}.rs`; `voxel-asset/src/{lib,asset,palette,codec,object_codec,object,frame,conversion}.rs`; `voxel-annotation/src/{model,query,edit,codec}.rs`; `voxel-object-runtime/src/{lib,model,admission,collision,player}.rs`; `core-voxel/src/lib.rs`; `environment-authoring/src/{lib,generation,materialization}.rs`; `entity-state/src/{lib,model,definition,component,component/registration,authoring,command,snapshot,components,transform,relationship,rigid_body,activation,character_motion,character_motion_publication}.rs`; `core-ids/src/lib.rs`; `core-math/src/lib.rs`; `core-space/src/{lib,voxel,world,direction,region,origin,grid}.rs`; `core-time/src/lib.rs`; `scripts/inventory-validation.py`; `voxel-convert/src/{source.rs (parts), object_conversion/validation.rs, store.rs}`; `render-projection/src/material.rs:1-80`.

**Skimmed (grep + targeted ranges):** `asset-import/src/{gltf_package,animated_glb}.rs` (import-time glTF validation; ranges 95-145, 215-240, 1100-1140 read); `voxel-convert/src/{planning.rs (120-300, 300-450), convert.rs (60-140), query.rs (185-235), object_query.rs (255-310), object_conversion/planning.rs (625-650), import.rs (400-560), animation/sample.rs (25-130, 393-405)}`; consumer context in `csharp-engine-services/src/{authored_content.rs, voxel_content.rs, content/portable.rs, spatial.rs}` and `engine-spatial/src/character_controller.rs` (only to establish call frequency).

**Skipped:** `voxel-convert/src/{animation/import.rs, animation/import/clip.rs, animation/sample/*.rs, import/gltf_scene.rs, voxelize.rs, voxelize/tests.rs, material.rs, gltf_document.rs, diagnostic.rs, lib.rs, object_conversion/{model,mod,planning (rest)}.rs, bin/*}` (third-party glTF import and voxelization internals; limits listed in the legitimate section from grep); `entity-state/src/value.rs`; all `tests/` directories except where cited as rejection-proving tests.


---

# Appendix 9: 09-svc-spatial

## Survey 09: svc-collision, svc-implicit, svc-mesh, svc-pathfinding, svc-rng, svc-spatial, svc-volume, engine-spatial

Read-only survey against the brief. 38 findings: 10 High, 20 Medium, 8 Low. Ordered by confidence, then cost. All paths are repo-relative under `rust/crates/`.

The single biggest structural item is the chunk-collider `source_hash` plumbing (finding 1): the only readers of that hash (`CollisionProjection::identity` and `is_chunk_stale`) have no callers anywhere in the repo, yet engine-spatial still hashes every rebuilt chunk's voxels to feed it.

---

## High confidence

### 1. Dead chunk-collider staleness key: every collider rebuild hashes the whole chunk for a hash nothing reads
- **Where:** `svc-collision/src/lib.rs:229` (`ChunkCollider.source_hash`), `svc-collision/src/lib.rs:1020-1070` (`identity()`), `svc-collision/src/lib.rs:1346-1358` (`is_chunk_stale()`), `svc-collision/src/lib.rs:1074-1077` (`set_chunk` computes `chunk.content_hash()`), `svc-collision/src/lib.rs:1142-1151`, `:1227-1272` (`source_hash` threaded through `set_chunk_parts`/`install_chunk_parts`); the feeders in `engine-spatial/src/lib.rs:1244` and `engine-spatial/src/lib.rs:1272` (`prepare_collider` computes `collided.content_hash().0` / `chunk.content_hash().0` solely to pass as `source_hash`).
- **What it does:** Each chunk collider records the FNV-1a `content_hash` of the `VoxelChunk` it was built from (a full pass over every cell, every density, every stored crossing). `is_chunk_stale` re-hashes the live chunk and compares; `identity()` re-hashes every resident chunk into a formatted string and hashes that again.
- **Why it looks pointless:** `rg` over `rust/` and `csharp/` finds no caller of `is_chunk_stale`, `identity(`, `source_hash_hex` or `projection_hash_label` outside `svc-collision/src/lib.rs` itself and its own tests. engine-spatial already drives rebuilds by its own dirty sets (`mesh_neighbourhood`, `rebuild_chunks`); the "derived, not authoritative, staleness key" story in the crate doc is a second, unused mechanism. Deleting it changes no observable behavior.
- **Cost:** Per collider rebuild (every edit batch, every residency op, every rebase, `set_noncollidable_materials`, `set_mesh_options`), one O(chunk volume) hash per rebuilt chunk, on the meshing threads. `identity()` is O(world) but dead.
- **Confidence:** High.
- **Removal shape:** Delete `is_chunk_stale`, `identity`, `CollisionProjectionIdentity`, the `source_hash` field and parameters; drop the `content_hash()` calls at engine-spatial lib.rs:1244/1272; delete tests `projection_detects_staleness_and_rebuilds`, `first_solid_in_untracked_chunk_reads_as_stale`, `unloading_a_chunk_makes_its_collider_stale_then_dropped` (lib.rs:2373-2430).

### 2. Static-mesh residency delta re-walks, re-hashes and re-clones the whole catalog per call (the ResourceList pattern)
- **Where:** `svc-collision/src/static_mesh.rs:275-316` (`apply_residency` clones every asset Arc and rebuilds a `BTreeMap` of every instance), `:317-390` (`commit_projection` walks every instance; `:368-375` re-folds `topology` over every asset and instance; `:388` `projection_hash: candidate.identity_hash()`), `:230-253` (`identity_hash` hashes every asset and every instance transform), `:192-228` (`translated` rebuilds every trimesh on a world-origin rebase).
- **What it does:** A bounded residency delta (add/remove a few instances) rebuilds the complete asset and instance maps, re-validates every transform, re-derives the whole-catalog topology sum, and computes a whole-catalog FNV hash for the receipt. The struct comment at `:150-152` says topology is "maintained by replacement so per-step identity reads stay O(moving)" but `commit_projection` recomputes it from scratch.
- **Why it looks pointless:** Whole-collection validation/hashing per call where only a delta changed. The receipt's `projection_hash` is identity bookkeeping; the topology sum could be maintained by add/subtract exactly as `NavProjection::set_walkable_cells` and `VoxelCollisionScene::account_voxel` already do.
- **Cost:** Per `apply_residency`/`replace_all` call, O(assets + instances) in maps, clones, hashing and validation; plus O(all trimeshes) on every rebase.
- **Confidence:** High (this is precisely the commit-2ee8375 shape).
- **Removal shape:** Replace with per-id delta: apply removals/upserts in place, maintain `topology` by wrapping add/sub, drop `identity_hash` from the receipt (or maintain it the same way).

### 3. Static-mesh duplicate-id refusals where a map insert would do
- **Where:** `svc-collision/src/static_mesh.rs:262-266` (`DuplicateAsset` in `replace_all`), `:306-311` (`DuplicateInstance` in `apply_residency` upserts), `:326-330` (`DuplicateInstance` in `commit_projection`), error variants `:92-93`.
- **What it does:** Refuses the whole replacement if the product lists an id twice.
- **Why it looks pointless:** Duplicate guard on trusted product input; `BTreeMap::insert` is last-write-wins and nothing downstream needs uniqueness scanning. The owner already removed the renderer's equivalent `DuplicateResource`.
- **Cost:** Per residency call, one `contains_key` per instance; cheap, but it also forces the fail-atomic candidate structure.
- **Confidence:** High.
- **Removal shape:** Delete the three checks and the two error variants; let later entries replace earlier ones. Test `invalid_replacement_is_fail_atomic` (`:902`) keeps its `MissingAsset` half only.

### 4. Navigation traversal overlays refuse duplicate cells and cap at 65,536 records
- **Where:** `svc-pathfinding/src/lib.rs:636` (`MAX_NAV_TRAVERSAL_CELLS`), `:675-682` (`DuplicateCell`, `TooManyCells` in `NavTraversalOverlay::from_cells`), `:764-773` (same in `VolumetricNavTraversalOverlay::from_cells`), error variants `:720-725`, `:811-816`, labels `:819-837`.
- **What it does:** Builds a `BTreeMap` of per-cell rules but errors on a repeated coordinate instead of inserting, and errors past 65,536 cells ("bounds both retained owner state and the deterministic search lookup surface").
- **Why it looks pointless:** Duplicate guard and a magnitude cap with no concrete failure behind it: the search does one `BTreeMap::get` per candidate regardless of overlay size, and the product owns how many cells it blocks. Deleting them means the last record for a coordinate wins and a 70k-cell overlay simply works.
- **Cost:** Per overlay build, O(cells); plus the test at `:2903-2920` builds 65,537 cells to prove the cap.
- **Confidence:** High.
- **Removal shape:** Delete the two checks, the constant and the `DuplicateCell`/`TooManyCells` variants; trim tests `traversal_overlay_is_canonical_and_rejects_invalid_records` (`:2835`) and `weighted_volumetric_overlay_blocks_and_rejects_invalid_records` (`:3352`).

### 5. Every weighted volumetric path query hashes every resident chunk, twice
- **Where:** `svc-pathfinding/src/lib.rs:1856` (`let source_hash = volumetric_navigation_source_hash(world);` inside `find_weighted_volumetric_path`), `:2198-2218` (the function: collects all resident chunks into a `Vec`, feeds coordinate and `content_hash()` of each); the C# bridge computes it again per request at `csharp-engine-services/src/spatial.rs:2314`, `:2010`, `:2048`.
- **What it does:** Produces a `source_hash` field on the readout so "callers can distinguish a volumetric occupancy source". `VoxelChunk::content_hash()` is itself a full pass over the chunk's cells.
- **Why it looks pointless:** Identity re-derivation per call over a whole collection that did not change. engine-spatial already maintains `authority_hash` incrementally and the scene has a `source_revision`; either is the identity. Nothing in the search uses `source_hash`.
- **Cost:** Per path request, O(resident chunks × chunk volume) hashing, done twice (service + bridge).
- **Confidence:** High.
- **Removal shape:** Drop `source_hash` from the readout (or report the scene's maintained `authority_hash`/revision); delete `volumetric_navigation_source_hash`.

### 6. Keyed RNG scope/key byte quotas
- **Where:** `svc-rng/src/lib.rs:30` (`MAX_KEYED_RNG_SCOPE_BYTES = 256`), `:33` (`MAX_KEYED_RNG_KEY_BYTES = 4096`), `:172-187` (`ScopeQuotaExceeded`, `KeyQuotaExceeded` in `validate_keyed_input`), variants `:43-46`, test `keyed_v1_reports_typed_input_errors` `:379-417`.
- **What it does:** Refuses a keyed draw whose scope string exceeds 256 bytes or whose key exceeds 4 KiB.
- **Why it looks pointless:** Max-length limits with no failure behind them: the input is FNV-hashed byte by byte, a 5 KiB key costs 5 K multiplies. Called from the C# bridge (`csharp-engine-services/src/rng.rs:68`) with product-built keys.
- **Cost:** Per draw, two length compares; the cost is the API ceremony, not CPU.
- **Confidence:** High.
- **Removal shape:** Delete the two constants, two checks, two variants and the matching test arms.

### 7. `set_edge_crossing` range/finite check duplicates the clamp in `pack_crossing`
- **Where:** `svc-volume/src/lib.rs:441-444` (`!(normal all finite && (0.0..=1.0).contains(&at))` → `InvalidDensity`), versus `svc-volume/src/lib.rs:115-126` (`pack_crossing` already does `clamp(-1.0, 1.0)` / `clamp(0.0, 1.0)` then `as i16`, which saturates NaN to 0).
- **What it does:** Refuses a crossing whose `at` is outside `[0, 1]` or whose normal is non-finite, one line before packing clamps the same values.
- **Why it looks pointless:** Double-checking an invariant the representation enforces by construction. Deleting it changes nothing observable except that an out-of-range value is clamped instead of refused. (The `axis > 2` half of the same condition is legitimate: a larger axis would alias another voxel's key.)
- **Cost:** Per crossing written (per density stamp voxel edge).
- **Confidence:** High.
- **Removal shape:** Reduce the condition to `axis > 2`.

### 8. Every scene rebuild re-validates every voxel of every resident chunk
- **Where:** `engine-spatial/src/lib.rs:596-607` (`build_from_voxel_world_at_revision` iterates all resident chunks, all cells, calling `validate_material_voxel` on each solid), reached from `set_mesh_options` (`:1018`), `rebased_candidate` (`:1065`) and the constructors.
- **What it does:** Re-applies the coordinate cap and material-slot range to voxels already in the world.
- **Why it looks pointless:** Schema re-validation of data that was validated when it entered: `build_voxel_world` (`:514`), `VoxelEditService` (`voxel_edit.rs:256`), `VoxelDensityEditService` (`voxel_density.rs:313`), `VoxelChunkResidencyService::validate_payload` (`voxel_residency.rs:463-472`) and `validate_chunk_identity` all validate on entry. No path writes an unvalidated voxel into `voxel_world`.
- **Cost:** O(world) per rebase and per `set_mesh_options`, on top of the remesh those already do.
- **Confidence:** High.
- **Removal shape:** Delete the `validate_material_voxel` call in that loop (keep the count/hash accumulation), or make it a `debug_assert`.

### 9. Residency payload refuses densities whose sign disagrees with the material, then discards the signs
- **Where:** `engine-spatial/src/voxel_residency.rs:486-499` (`InvalidDensities` when any density is non-finite or `(density < 0) != (material != 0)`), feeding `svc-volume/src/lib.rs:378-386` (`set_densities` keeps `v.abs()` only and re-checks `is_finite` itself at `:381`).
- **What it does:** A full per-voxel scan of the payload, rejecting a sign mismatch and non-finite values; `set_densities` then re-scans for finiteness and throws the signs away.
- **Why it looks pointless:** The sign agreement is a "you must say it our way" consistency rule with no representation behind it (the stored density has no sign; `density_at` derives sign from solidity). The finiteness scan runs twice on the same slice. Trusted product payload.
- **Cost:** Per residency operation, two O(chunk volume) scans.
- **Confidence:** High for the sign check; Medium for keeping one finiteness scan at all (a NaN magnitude would place a reconstructed vertex at NaN, which is "renders oddly", not a panic).
- **Removal shape:** Delete the sign comparison; keep at most one `is_finite` scan (in `set_densities`) or none.

### 10. The character controller validates its ~45-field config on every command
- **Where:** `engine-spatial/src/character_controller.rs:1600` (`command.validate_against(config)?` at the top of `solve`), `:380-388` (`validate_against` calls `config.validate()` then `validate_command`), `:161-240` (`CharacterControllerConfig::validate`: `positive`/`nonnegative`/`range` over every tunable).
- **What it does:** Re-checks the long-lived, product-owned controller configuration for finiteness/sign/range on every step of every character, before validating the command itself.
- **Why it looks pointless:** Schema re-validation per call of data that does not change per call. A product sets its config once; a bad value would be caught the first time.
- **Cost:** Per command per character (~45 float compares plus a `Result` chain), on the controller's hot path.
- **Confidence:** High that it should not run per step; Medium on whether to keep it at all.
- **Removal shape:** Validate once where the config is installed (or `debug_assert`), keep only `validate_command` on the step path.

---

## Medium confidence

### 11. `MAX_VOXEL_COORDINATE_ABS = 1_000_000` refuses large product coordinates everywhere
- **Where:** `engine-spatial/src/voxel_edit.rs:16` (const, "Keeps chunk addressing and projection work in a reviewable world-space span"), `:46-56` (`validate_voxel_address`), applied per edit (`voxel_edit.rs:359`), per density edit (`voxel_density.rs:903-908`), per residency op (`voxel_residency.rs:510-528` `validate_chunk_identity`), per primitive/template voxel (`voxel_primitive.rs:112-113, 166-167, 190`, `voxel_template.rs:55, 70`), per scene build (`lib.rs:514, 602`).
- **What it does:** Rejects any voxel address with a component beyond ±1,000,000.
- **Why it looks pointless:** A magnitude cap with no overflow or stability reason: addressing is `i64`, world positions are `f64` and the world-origin rebase service exists precisely to keep `f32` render coordinates small. A product with a 2,000,000-voxel world is refused rather than served.
- **Cost:** Per voxel on every mutation path; cheap individually, but it is the reason finding 8 exists.
- **Confidence:** Medium (an owner may want some bound for chunk-coordinate × chunk-size overflow; `i64` makes that ~1e17, not 1e6).
- **Removal shape:** Delete, or raise to the actual `i64` overflow bound and check only in `validate_chunk_identity`.

### 12. `MAX_VOXEL_MATERIAL_SLOT = 4_095` is not a representation limit
- **Where:** `engine-spatial/src/voxel_edit.rs:18`, `:59-68` (`validate_voxel_material_slot`), per-voxel in `voxel_residency.rs:463-472`, and every edit path above.
- **What it does:** Refuses material slots above 4095.
- **Why it looks pointless:** `core-voxel/src/lib.rs:185` decodes the material as `(bits & 0xFFFF) as u16`: the value stores 16 bits. Nothing in render-projection/render-wgpu/csharp pins a 4096-entry palette (grep found none). Slot 0 = empty is the only real rule.
- **Cost:** Per voxel per mutation.
- **Confidence:** Medium.
- **Removal shape:** Reduce to `slot != 0`.

### 13. `SurfaceMeshLimits` is a family of sanity caps checked in the inner meshing loops
- **Where:** `svc-mesh/src/lib.rs:174-192` (six limits; defaults 2M faces, 4M cells, 8M vertices, 12M indices, 256 MiB, 4096 partitions), `svc-mesh/src/surface.rs:353-380` (`check_lattice_budget`), `:544-558` (per-quad `TooManyFaces` and `check_output_growth`), `:727-729` (per-vertex `TooManyVertices` against 8M, not `u32::MAX`), `:934`, `:1378-1383` (`TooManyMaterialPartitions`), `:1499-1507` (final vertex/index caps), `:1673-1699` (`check_output_growth`), `svc-mesh/src/lib.rs:1110-1114` (greedy `TooManyFaces`), `svc-implicit/src/surface.rs:245-262` (`max_vertices == 0` etc.), `svc-implicit/src/surface/refinement.rs:17-18, 46, 85`, `svc-implicit/src/surface/regions.rs:154-167`.
- **What it does:** Refuses a mesh that would exceed product-configurable work/retention ceilings, checked quad by quad and vertex by vertex.
- **Why it looks pointless:** The only representation bound is "vertex index fits `u32`"; everything else refuses a big input rather than serving it, and does so on the hottest path (two `checked_add` + four compares per emitted quad). A chunk is at most 66³ samples (`MAX_CHUNK_SIZE = 64`), so the caps are unreachable for scene chunks and only ever bite product scalar volumes/voxel objects, where the product chose the size.
- **Cost:** Per quad and per vertex of every meshed chunk.
- **Confidence:** Medium (a product may want an explicit budget for voxel-object admission; that can be one check on the input size, not per-quad).
- **Removal shape:** Keep a single `u32::MAX` index check at payload assembly; delete the limits struct, `check_output_growth`, `check_lattice_budget` and the per-quad/per-vertex checks, with their tests.

### 14. Scalar samples are validated finite on write and again on every extraction
- **Where:** `svc-implicit/src/volume.rs:118` (`write` → `super::finite(samples)`), `:326` (rasterize batches), versus `svc-mesh/src/surface.rs:292-300` (`Lattice::scalar` re-scans every sample: `InvalidIsovalue`, `InvalidScalarSample`), reached from `svc-mesh/src/lib.rs:937` and `:985` on every `mesh_scalar_surface` / region generate.
- **What it does:** The volume only ever holds finite samples, yet the mesher re-validates all of them per extraction.
- **Why it looks pointless:** Schema re-validation at a later stage of data validated at entry. If the owner also removes the write-time scan (a NaN sample yields a NaN vertex, not a panic), neither is needed.
- **Cost:** Per extraction, O(samples in region).
- **Confidence:** Medium-High.
- **Removal shape:** Delete the scan in `Lattice::scalar` (keep `InvalidSampleCount`/`InvalidSampleDimensions`, which guard indexing); optionally delete the write-time scan too.

### 15. Per-vertex finiteness / range checks on the mesher's own output
- **Where:** `svc-mesh/src/surface.rs:1445-1450` (`PositionOutOfRange` if the engine-computed scaled position or normal is non-finite, per axis per vertex), `svc-mesh/src/lib.rs:655-730` (`checked_add(...).ok_or(PositionOutOfRange)` on every neighbour offset in greedy meshing), `svc-mesh/src/texture_mapping.rs:123-129` (`CoordinateOutOfExactRange` beyond ±2²⁴ per projected tile point), `:155-176` (`repeat_voxel_tile_coordinate` finite/range/precision checks).
- **What it does:** Guards against f32 inexactness or overflow on coordinates that are bounded elsewhere (finding 11 caps at 1e6 ≪ 2²⁴; `checked_add` on an `i64` voxel coordinate cannot overflow in practice).
- **Why it looks pointless:** Checks that `debug_assert!` would cover, on the per-vertex/per-face path, with no reachable failure.
- **Cost:** Per vertex / per face of every mesh.
- **Confidence:** Medium.
- **Removal shape:** Replace with `debug_assert!` or delete, and drop the `PositionOutOfRange`/`CoordinateOutOfExactRange` variants and the texture-mapping tests at `:294-400`.

### 16. Character controller re-validates and re-hashes its collider set per sub-step and again at commit
- **Where:** `engine-spatial/src/character_controller.rs:1675` and `:2168` (`validate_mesh_instances` in `solve_step` and in `commit_with_obstacles_and_mesh_instances`), `:3010-3058` (two `BTreeSet` duplicate scans, existence lookups, finite velocities, unit-scale per instance), `:1683` and `:2175` (`character_environment` → `hash_obstacles` over every obstacle `:3068-3092`, `hash_mesh_instances` sorts and hashes every instance `:2905`), `:2175-2178` (`StaleEnvironment` refusal if the re-derived identity differs between prepare and commit).
- **What it does:** Each sub-step of a command and the final commit both validate the product-supplied obstacle/mesh lists for duplicates and finiteness and hash them in full; commit refuses if anything changed since prepare.
- **Why it looks pointless:** Duplicate guards and finite checks on trusted product lists, run several times per command; and a prepare/commit staleness rollback for a single-threaded engine where the product is the only thing that could change the world in between. Only `source_revision`/`authority_hash`/`projection_version`/`static_mesh_topology_hash`/`mesh_instance_hash` feed the `collision_world_hash` that clears support (`:1685-1688`); `obstacle_hash` and `mesh_motion_hash` exist only for the commit-time comparison.
- **Cost:** Per sub-step (up to `count` per command) plus once at commit: O(obstacles + mesh instances) with sorts and hashing.
- **Confidence:** Medium.
- **Removal shape:** Validate once per command (or not at all), drop `obstacle_hash`/`mesh_motion_hash` and the `StaleEnvironment` comparison; keep the support-clearing hash.

### 17. `DuplicateOrOldCommand` sequence guard
- **Where:** `engine-spatial/src/character_controller.rs:1601-1606`, variant `:552`, code `:582`.
- **What it does:** Refuses a command whose `sequence` is not greater than the last one applied.
- **Why it looks pointless:** A replay/ordering guard against the product's own command stream; the product owns sequencing and a repeated step would just step again.
- **Cost:** One compare per command.
- **Confidence:** Medium-Low (cheap; the question is whether the API surface is wanted).
- **Removal shape:** Delete, with the `stale step` halves of tests `character_tether.rs:144` and `mechanics.rs:296`.

### 18. Capsule/obstacle NaN-and-sign checks on every collision query
- **Where:** `svc-collision/src/lib.rs:1808-1837` (`validate_character_query`), called at `:485, :549, :718, :806, :843, :917`; `:1839-1863` (`validate_obstacle`) per obstacle at `:498, :555`.
- **What it does:** Rejects a non-finite capsule/translation/skin or a non-positive radius/half-extent before each cast, overlap or intersection test.
- **Why it looks pointless:** Defensive finite checks on values the trusted controller/product supplies, on the hottest query path (the character step issues many casts per sub-step). Parry returns no hit for a NaN pose; it does not panic.
- **Cost:** Per query (≈12 compares) plus ≈15 per obstacle per query.
- **Confidence:** Medium.
- **Removal shape:** Delete or `debug_assert`; drop `InvalidCapsule`/`InvalidTranslation`/`InvalidContactSkin`.

### 19. Dynamics body admission: duplicate refusal and unit-quaternion tolerance
- **Where:** `svc-collision/src/dynamics.rs:298-300` (`DuplicateBody`), `:301`, `:335` (`validate_body`), `:777-800` (`validate_body`: finiteness, positive shape, `unit_rotation` within 1e-4), `:383-384` (`set_body_motion` unit check), `:823-825`; joint frames `svc-collision/src/joint.rs:101-107` (`unit` within 1e-3) although `DynamicsJointFrame::pose()` at `:23-27` normalizes anyway; `svc-collision/src/dynamics.rs:611-612` (per-action `handle` lookup then `is_finite`, with a second `self.bodies[&action.body]` lookup at `:619`).
- **What it does:** Refuses insertion of an id already present (although `replace_body` exists), refuses quaternions off unit length by 1e-4, and re-checks each step action.
- **Why it looks pointless:** Duplicate guard where upsert would do; the quaternion tolerance is a "must agree with us" check when normalizing is cheaper than refusing (joint frames already normalize, making their check redundant).
- **Cost:** Per insert; per action per step.
- **Confidence:** Medium (positive mass/radius protect rapier from degenerate mass properties and are reasonable single-value checks).
- **Removal shape:** Make `insert_body` upsert; normalize rotations instead of refusing; delete the joint `unit` check; keep positive mass/shape.

### 20. Perception query dedupes its observer and target lists per call
- **Where:** `engine-spatial/src/perception.rs:142-156` (sort + `ensure_unique_observers`/`ensure_unique_targets` + `validate_observer` per observer + finite per target), `:322-341`.
- **What it does:** Copies, sorts and window-scans both product lists for duplicate entity ids on every page evaluation; errors on a duplicate.
- **Why it looks pointless:** Duplicate guard on trusted input; a repeated observer would merely produce repeated pairs.
- **Cost:** Per query, O((n + m) log) copies and scans before any work.
- **Confidence:** Medium.
- **Removal shape:** Delete the uniqueness scans and `DuplicateObserver`/`DuplicateTarget`; trim test `perception.rs:127`.

### 21. Trigger volume duplicate/lifecycle refusals and identifier string rules
- **Where:** `engine-spatial/src/trigger.rs:251-257` (`DuplicateDefinition` on re-register), `:291-305` (`DuplicateLifecycle` when `set_active` is already in that state), `:334-343` (restore list contains a duplicate, after it was already deduped into a `BTreeSet`), `:544-571` (`validate_definition`/`valid_identifier`: scope and tags must be ≤128 bytes of `[A-Za-z0-9._-]`).
- **What it does:** Turns idempotent operations into errors and constrains product-authored scope/tag strings.
- **Why it looks pointless:** Duplicate guards where insert/set would do; max-string-length and charset limits with no consumer that needs them (tags are only sorted, deduped and copied into facts).
- **Cost:** Per register / per lifecycle change; small.
- **Confidence:** Medium.
- **Removal shape:** Make `register` replace, `set_active` idempotent (return an empty receipt), drop the restore duplicate check and `valid_identifier`; trim `mechanics.rs:296`.

### 22. Voxel pick "untrusted hint" re-validation
- **Where:** `engine-spatial/src/voxel_picking.rs:6-7` ("Every field is untrusted until the scene re-casts"), `:74-92` (`VoxelPickService::validate` re-casts and refuses `HintMismatch` if the caller's claimed voxel/face differs), `:182-208` (`validate_transform`: finite, positive scale, quaternion norm within 0.002), `:168-180` (`validate_ray`).
- **What it does:** The product passes a claimed hit; the engine recasts and errors if the claim differs instead of just returning the authoritative hit.
- **Why it looks pointless:** Hostile-input framing of a product-supplied value. The authoritative answer is the recast; the "claim" and `HintMismatch` are ceremony. The 0.002 quaternion tolerance is a refusal where normalizing would serve.
- **Cost:** Per pick; small, but it shapes the C# API (`csharp-engine-services/src/spatial.rs:3082`).
- **Confidence:** Medium.
- **Removal shape:** Return the recast hit (and anchor) without comparing to a claim; normalize the transform rotation.

### 23. `propose_direct_nav_movement` NaN/step checks
- **Where:** `svc-pathfinding/src/lib.rs:1944-1949` (`NonFinitePosition`, `InvalidStep`), variants `:1052-1065`, test `direct_nav_movement_rejects_invalid_inputs` `:3529`; bridge mapping at `csharp-engine-services/src/spatial.rs:2440, 2508`.
- **What it does:** Refuses a non-finite from/target or a non-positive step before computing a waypoint.
- **Why it looks pointless:** Defensive finite checks on product positions; a NaN input yields a NaN waypoint (odd, not a panic), a zero step yields `from`.
- **Cost:** Per call; small.
- **Confidence:** Medium.
- **Removal shape:** Delete with the variants and test.

### 24. `CellOutsideProjection` and `ZeroCost` overlay refusals
- **Where:** `svc-pathfinding/src/lib.rs:669-674` (`ZeroCost`, `CellOutsideProjection`), `:761-763` (`ZeroCost`).
- **What it does:** Refuses an overlay cell not in the walkable set, or with cost 0.
- **Why it looks pointless:** A cell outside the projection is never looked up (harmless); a zero cost is a valid Dijkstra weight (no loop, the closed set prevents re-expansion). Both are "must agree with us" rules on product facts.
- **Cost:** Per overlay cell at build; small.
- **Confidence:** Medium-Low.
- **Removal shape:** Delete both and their variants.

### 25. Empty scope / empty key refusals in keyed RNG
- **Where:** `svc-rng/src/lib.rs:170-171`, `:178-179` (`EmptyScope`, `EmptyKey`, "to keep caller domains explicit").
- **What it does:** Refuses a draw with an empty scope or key.
- **Why it looks pointless:** Policy about how the product names its decisions; an empty key hashes fine and deterministically.
- **Cost:** Per draw; trivial.
- **Confidence:** Medium.
- **Removal shape:** Delete with the variants and test arms.

### 26. `set_densities` whole-chunk finiteness scan
- **Where:** `svc-volume/src/lib.rs:381` (also `:403` for `set_density`).
- **What it does:** Scans every density for `is_finite` before storing magnitudes (duplicated upstream by finding 9).
- **Why it looks pointless:** NaN/finite check on product-supplied values that would only displace a reconstructed vertex.
- **Cost:** Per residency op, O(chunk volume).
- **Confidence:** Medium.
- **Removal shape:** Delete (keep the length check, which guards indexing).

### 27. Implicit `Field::generate` refuses its own finished output
- **Where:** `svc-implicit/src/lib.rs:386-390` (`max_vertices`/`max_triangles` checked after the octree and triangulation are complete), `:391-396` (scan of every extracted vertex for non-finite), `:367-369` (depth > 12 refusal).
- **What it does:** Does all the work, then throws it away if a budget was exceeded or the engine's own output has a non-finite vertex.
- **Why it looks pointless:** Post-hoc caps serve nothing (the memory is already spent); the finite scan is a check on engine output that `assemble_with` repeats (finding 28).
- **Cost:** Per generate, O(output).
- **Confidence:** Medium (the depth cap is a recursion/memory guard; Low on that one).
- **Removal shape:** Delete the post-hoc caps and the finite scan; keep the depth cap or turn it into the documented practical bound.

### 28. `assemble_with` re-validates geometry it was handed by `generate`
- **Where:** `svc-implicit/src/surface.rs:159` and `:304-308` (`validate_positions` over all positions), `:177-183` (`length_squared.is_finite()` per face), `:200-204` (centroid finite per face), `:169-178` (`triangle_positions` bounds check per corner), `svc-implicit/src/surface/regions.rs:36-38` (field samples finite).
- **What it does:** Scans all positions for finiteness, then per face re-derives finiteness of area and centroid, and bounds-checks indices that `triangulate` produced.
- **Why it looks pointless:** Re-validation of engine-produced data already validated at extraction (finding 27). The comment at `:171-174` says it is "for hand-built or partially recovered Geometry values".
- **Cost:** Per assembly, O(vertices + faces).
- **Confidence:** Medium-Low (the index bounds check prevents a panic and is cheap; the finiteness scans are the finding).
- **Removal shape:** Delete `validate_positions` and the per-face finite checks.

### 29. Rigid-body kinematic integration refuses non-finite product state per body
- **Where:** `engine-spatial/src/physics.rs:276-293` (`NonFiniteInput` on position/velocity/acceleration/gravity/scale, `StepOverflow`, re-check of `next_velocity`), `engine-spatial/src/entity_motion.rs:97, 136` (`InvalidDelta`), `:265-271` (`is_valid_spatial_entity`), `:444-461` (`validate_first_person_input`, including `speed < 0` refusal).
- **What it does:** Finite/sign checks on product-owned body and input values per body per tick.
- **Why it looks pointless:** NaN would propagate to a NaN transform (odd), not panic; negative speed just moves backwards.
- **Cost:** Per body per tick; small.
- **Confidence:** Medium-Low.
- **Removal shape:** Delete or `debug_assert`.

### 30. `MAX_DENSITY_EDIT_VOXELS` and `MAX_MOTION_DELTA_SECONDS`
- **Where:** `engine-spatial/src/voxel_density.rs:37`, `:315-319` (16,777,216-voxel batch cap); `engine-spatial/src/lib.rs:159`, `:2064` (motion step > 1.0 s refused "so a single accidental multi-second step cannot become an unbounded entity-state edit").
- **What it does:** Refuses a big product number.
- **Why it looks pointless:** Sanity caps without a numeric-stability reason: the density batch is bounded by the product's own edit list; the motion step is a product choice (a pause-resume hitch would be refused rather than integrated or clamped).
- **Cost:** One compare each.
- **Confidence:** Medium-Low.
- **Removal shape:** Delete `TooManyVoxels`; delete or clamp instead of refusing the motion delta.

---

## Low confidence

### 31. Meshing computes `content_hash()` of the source chunk per rebuild for a "diagnostic provenance" field
- **Where:** `engine-spatial/src/lib.rs:831`, `:1627` (`source_chunk_hash` on `VoxelMeshChunk`, documented at `:191-192` as "diagnostic provenance, not the retained-render replacement key").
- **What it does:** One O(chunk) hash per remeshed chunk.
- **Why it looks pointless:** A hash computed per hot rebuild for diagnostics only. (Not confirmed whether any consumer reads `source_chunk_hash`.)
- **Cost:** Per remeshed chunk, small relative to meshing.
- **Confidence:** Low.
- **Removal shape:** Delete the field if no consumer reads it.

### 32. `SurfaceMaterials::new` duplicate-slot refusal and `SurfaceCharacter::validate` ranges
- **Where:** `svc-mesh/src/lib.rs:146-156`, `:110-118`.
- **What it does:** Errors on a repeated material slot (sort + windows) and on crease/roughness outside documented ranges (roughness is clamped anyway at `surface.rs:717-719`).
- **Cost:** Once per options change.
- **Confidence:** Low (cheap, once).
- **Removal shape:** Last-wins on duplicates; drop the range check or keep as doc.

### 33. Scatter `MAX_SURFACE_POINTS` cap and terrain-layer slot cap
- **Where:** `svc-mesh/src/scatter.rs:40`, `:103-105` (1,048,576 points per chunk sampling, "denser requests are refused by the caller before they get here"); `svc-mesh/src/terrain_layers.rs:28-29`, `:69` (`MAX_TERRAIN_LAYER_SLOTS = 16`, "so the set stays short"; the 16-*layer* bound at `:23` is a real `[f32; 16]` representation limit).
- **What it does:** Truncates/refuses product-chosen sizes for performance reasons.
- **Cost:** Per point / once.
- **Confidence:** Low.
- **Removal shape:** Delete the slot cap; keep or document the point cap as truncation.

### 34. World-origin envelope and origin-cell caps
- **Where:** `engine-spatial/src/world_origin.rs:9` (`MAX_WORLD_ORIGIN_CELL_ABS = 9e18`), `:233-247` (`validate_origin`, `validate_envelope` requiring 1.0..=1,000,000).
- **What it does:** Range-checks product-supplied rebase parameters.
- **Cost:** Per rebase; trivial.
- **Confidence:** Low.
- **Removal shape:** Delete the envelope range; the origin cap is effectively `i64` already.

### 35. Audit duplicate-piece-id refusals and piece mesh validation
- **Where:** `svc-implicit/src/audit.rs:187-192`, `svc-implicit/src/audit/integrity.rs:109-114` (duplicate ids), `audit.rs:60-61`, `:601-616` (`validate_mesh`: non-empty, finite, index bounds on `Piece::new`).
- **What it does:** Opt-in diagnostics validate their inputs.
- **Cost:** Once per audit.
- **Confidence:** Low (opt-in, cold; the index-bounds half prevents a panic).
- **Removal shape:** Drop the duplicate-id refusals (BTreeMap keyed by id); keep index bounds.

### 36. Dead `VoxelWorld::request` / `AlreadyTracked` / `Pending` lifecycle
- **Where:** `svc-spatial/src/lib.rs:44-52`, `:123-137`.
- **What it does:** A reserve-before-generate state that errors if the coordinate is already tracked.
- **Why it looks pointless:** No caller outside its own tests (`rg '\.request\('` finds none); engine-spatial inserts chunks directly.
- **Cost:** None (dead).
- **Confidence:** Low (dead code rather than hot validation).
- **Removal shape:** Delete `request`, `Pending`, `AlreadyTracked` and the lifecycle test.

### 37. `set_densities` whole-slice equality for no-op detection
- **Where:** `svc-volume/src/lib.rs:388` (`if next == self.densities`).
- **What it does:** Compares the new density slice bit-for-bit against the old to avoid a version bump.
- **Cost:** O(chunk) per call, same order as the copy it precedes.
- **Confidence:** Low (version-bump-only-on-change is a documented contract; cost matches the write).
- **Removal shape:** Leave, or bump unconditionally.

### 38. Residency no-op detection hashes both chunks
- **Where:** `engine-spatial/src/voxel_residency.rs:319` (`current.content_hash() == chunk.content_hash()` per admit/replace).
- **What it does:** Two O(chunk) hashes to skip an identical replacement and its remesh.
- **Confidence:** Low (a remesh costs more than two hashes; a direct `==` on the chunks would be cheaper than hashing both, which is the only suggestion).
- **Removal shape:** Compare chunks with `==` instead of hashing.

---

## Looked at and judged legitimate

- `svc-volume` `in_bounds`/`index`/`check_corner`/`from_values` length (`lib.rs:181-199, 226-262`): guard slice indexing.
- `svc-volume` `set_edge_crossing` `axis > 2` half (`lib.rs:441`): a larger axis aliases another voxel's key.
- `svc-rng` `InvalidRange` (`lib.rs:189`): prevents the `expect` in `inclusive_span`; `next_bounded_u32(0) → None` (`:145`): division by zero; `KEYED_RNG_V1_MAX_ATTEMPTS` (`:36`, `:231-241`): rejection-sampling loop termination.
- `svc-pathfinding` `max_visited == 0 → InvalidQueryBudget` and `VolumetricAgentVolume::is_valid` (`:1208, 1384, 1768, 1840, 1830`): cheap single-value checks (a zero volume would make every cell traversable); start/goal walkable checks: `table.position(..).expect` relies on them; `CostOverflow` via `checked_add`: real overflow; `ColumnTable::new` dedup (`columns.rs:118-122`): first-wins, not a refusal; `NavComponents::separation` `debug_assert_eq` (`:594-598`).
- `svc-collision` `validate_geometry` at `StaticMeshColliderAsset::new` (`static_mesh.rs:485-516`): import-time, index bounds prevent a trimesh panic; `MissingAsset`; `RevisionExhausted` (`checked_add`); `shape_world_aabb`/`swept_capsule_bounds` fail-open `None`; `raycast` zero-length/non-finite direction (`lib.rs:1444-1447`, `static_mesh.rs:395-397`): division by length; `character_query_may_intersect` fail-open.
- `svc-collision` tether `OutOfReach` (`tether.rs:162`, `dynamics.rs:489-493`): concrete solver yank; `DynamicsRopeSolverConfig::validate` zero substeps/iterations: division by zero; `DynamicsTetherEndpoint` same-body refusal; joint `SameBody`/`UnknownBody`; `step` `step_seconds <= 0` (`dynamics.rs:605`): rapier integration parameter.
- `svc-collision` positive mass/radius/half-extent in `validate_body` (`dynamics.rs:785-792`): degenerate rapier mass properties.
- `engine-spatial` residency `PayloadDimensionsMismatch`/`PayloadSlotCountMismatch` (`voxel_residency.rs:421-443`): protect the `expect`s in `from_values`; `InvalidCellStates` and edit `InvalidState` (`:446-458`, `voxel_edit.rs:386-389`): `VoxelState::from_raw(..).expect` and the "reconstructed surfaces cannot mesh states" representation rule.
- `engine-spatial` terrain transition < chunk size (`lib.rs:588-595`): neighbourhood correctness for local remesh; `MAX_CHUNK_SIZE = 64` (`:160, :506`): bounds lattice arrays (`ChunkDims::cubic(..).expect`).
- `engine-spatial` `MAXIMUM_STEP_SECONDS`/`solver_steps` and `maximum_displacement_per_step` in `validate_command` (`character_controller.rs:2845-2852`): a cap that prevents a genuine solver tunnel/blow-up (sub-step sizing).
- `engine-spatial` `occlusion.rs:127-145` zero-length direction / positive max distance: division by length.
- `engine-spatial` `TransformService.eligible`, `ParentedEntity`, `MissingCollider` etc. in `entity_motion.rs`: semantic preconditions, not validation ceremony.
- `svc-mesh` `InvalidSampleDimensions`/`InvalidSampleCount` (`surface.rs:284-291, 301-309`), `checked_product → CoordinateRangeTooLarge`: index/overflow guards; `MAX_TERRAIN_LAYERS = 16` and `layer < 16` (`terrain_layers.rs:23, 69-74`): `[f32; 16]` representation; transition 1..=4: bounds the halo reach used by `mesh_neighbourhood_of_voxel`; scatter `density > 0` (`scatter.rs:60`): division.
- `svc-implicit` `Bounds::validate` (`lib.rs:41-50`): fidget needs positive extents; `positive(cell_size)`: divisor; `SampledVolume::new` dims ≥ 2 and f32-advance check (`volume.rs:68-86`): lattice addressing; `range()` (`:105-111`): slice bounds; `next_revision` `checked_add`; `MAX_DIRTY_BOXES` (`:23, 181`): merges rather than refuses.
- `svc-spatial` `unload → NotResident` (`lib.rs:156-170`): the call returns the evicted data.

## Coverage

Read in full (non-test source): `svc-rng/src/lib.rs`, `svc-spatial/src/lib.rs`, `svc-volume/src/lib.rs`, `svc-pathfinding/src/lib.rs` (1-2300) and `columns.rs`, `svc-collision/src/lib.rs` (1-2000), `static_mesh.rs` (1-640), `dynamics.rs` (1-894), `joint.rs` (1-175), `ragdoll.rs` (1-278), `tether.rs` (1-200), `engine-spatial/src/lib.rs` (all 2190), `surface_collision.rs`, `active_collision.rs`, `rigid_body.rs`, `voxel_picking.rs` (1-30, 60-210), `world_origin.rs` (1-60, 225-249).

Read in targeted regions around every grep hit: `engine-spatial/src/character_controller.rs` (150-240, 370-400, 540-680, 1585-1700, 2150-2200, 2820-2910, 3000-3100, 3380-3595; the remaining ~2000 lines of solver geometry grep-skimmed for the brief's vocabulary with no further hits), `voxel_edit.rs` (1-80, 240-420), `voxel_residency.rs` (255-360, 400-541), `voxel_density.rs` (290-330, 880-1052), `trigger.rs` (230-370, 540-583), `perception.rs` (130-175, 320-377), `entity_motion.rs` (80-150, 255-275, 370-470), `physics.rs` (1-30, 270-300), `occlusion.rs` (80-145), `character_tether.rs` (30-70), `character_modes.rs` (30-80), `voxel_primitive.rs`/`voxel_template.rs` (grep only); `svc-mesh/src/lib.rs` (90-230, 380-500, 740-800, 1050-1130, 1740-1790), `surface.rs` (150-380, 440-600, 700-740, 1360-1510, 1670-1700), `terrain_layers.rs` (1-135), `texture_mapping.rs` (1-220), `scatter.rs` (30-120); `svc-implicit/src/lib.rs` (1-130, 330-460), `surface.rs` (140-345), `volume.rs` (1-200, 370-405), `audit.rs` (1-235, 580-630), `audit/integrity.rs` (95-140), `audit/continuity.rs` (200-260), `surface/regions.rs` (20-60).

Grep-skimmed only (no vocabulary hits): `svc-mesh/src/occlusion.rs`, `svc-mesh/src/distance_field.rs`, `svc-implicit/src/triangulate.rs`, `svc-implicit/src/surface/refinement.rs` (limits hits noted in finding 13).

Skipped: `svc-collision/src/tether.rs` 200-300, `svc-implicit/src/audit/continuity_tests.rs`, all `tests/` and `examples/` directories (test names consulted only to list what goes with a finding), and the `#[cfg(test)]` modules of every file.


---

# Appendix 10: 10-csharp-scripts

## Survey 10: C# SDK, generators and scripts — pointless validation candidates

Scope: `csharp/**/*.cs` (excluding obj/bin) and `scripts/`. Read-only; nothing was built or run.
Policy applied: AGENTS.md "Keeping mechanisms small" and the survey brief. ABI/lifetime checks
(null pointers, pointer/length coherence, strict UTF-8 at the boundary, handle-table misses)
are not reported. Product-facing argument checks are reported only where they refuse a value
the Engine would accept or re-check something already guaranteed.

Counts: **High 7, Medium 12, Low 11** (30 findings). The hot-path items are the first six.

---

## High confidence

### 1. EffectsComponent re-validates the whole effect collection on every mutation
- **Where:** `csharp/Rusty.Engine/Mechanics/Effects.cs:308-332` (Publish), `:350-386` (ValidateCollection); callers `:177-230` (Apply), `:232-250` (Refresh), `:252-288` (Replace), `:298-306` (Remove/Expire).
- **What it does:** Every Apply/Refresh/Replace/Remove/Expire rebuilds the full list, sorts it by instance id, then `ValidateCollection` does `Distinct().Count()` over all instances, `GroupBy` stacking group, checks every group's policy consistency, instance limits and provenance uniqueness, then replaces `_effects` wholesale. `Apply` (`:187-223`) already checked exactly the delta constraints (duplicate instance, duplicate provenance, instance limit, refresh/replace exclusivity) before calling Publish, so the second pass can only re-prove what the first pass proved. Remove/Expire cannot violate any invariant at all. `Effects` (`:174-175`) also allocates a fresh array on every read.
- **Why it looks pointless:** "Whole-collection validation per call where only a delta changed" — the ResourceList pattern. The collection is only ever mutated through these methods, so the invariants hold by construction; `ValidateCollection` guards nothing except the product's own `ActiveEffect` constructor, which already enforces stacks bounds. Deleting it changes no accepted or refused input.
- **Cost:** Per mutation (effects apply/expire are common gameplay events), O(N log N) sort + two LINQ scans + N allocations, N = all active effects on the entity.
- **Confidence:** High.
- **Removal shape:** Delete `ValidateCollection` and the sort/rebuild in `Publish`; mutate `_effects` in place (insert at sorted position or just append).

### 2. InventoryStore equipment mutations re-validate every equipped item and diff the whole assignment map
- **Where:** `csharp/Rusty.Engine/Mechanics/Equipment.cs:306-373` (ValidateEquipment), called at `:200`, `:230`, `:290`; `:413-435` (Changes); `:63` (`ContainsItem` linear scan); `csharp/Rusty.Engine/Mechanics/Item.cs:411-412` (`IsEquippedAnywhere` scans every owner's equipment).
- **What it does:** Equip/Unequip/Swap already check the delta (item contained by owner, not already equipped, requested slot count, slot occupancy). Then `ValidateEquipment` walks *every* assignment of the owner: rebuilds a by-item dictionary, re-does `RequireItem`, `EnsureContainedBy`, policy presence, required-slot count, classification intersection and exclusivity-group check for every item already equipped, and recomputes every item's source activations. `Changes` then materialises both before/after assignment lists into dictionaries and diffs them to list the one or two slots that changed. `Unequip` (`:216-244`) revalidates items it did not touch.
- **Why it looks pointless:** Whole-collection validation per call; the untouched items were validated when they were equipped and nothing can have changed them except these same methods. `IsEquippedAnywhere` on destroy scans every owner when `_containment` already names the only owner that could hold it.
- **Cost:** Per equip/unequip/swap/destroy, O(items × slots × classifications) plus several allocations.
- **Confidence:** High.
- **Removal shape:** Validate only the incoming item's slots/exclusivity against the existing map; build `Changes` from the touched slot ids; keep `SourceActivations` for the incoming item only (or compute lazily).

### 3. InventoryStore recomputes full capacity (twice), clones the inventory and rescans stacks on every fungible/unique mutation
- **Where:** `csharp/Rusty.Engine/Mechanics/Inventory.cs:1118-1174` (ComputeCapacity, throws at `:1164-1169`), called before+after in GrantCore `:752,755`, ConsumeCore `:797,800`, TransferFungibleCore `:909-916` (four times), Split `:977`, Merge `:1032`, `Item.cs:232-233` (Materialize), `Item.cs:280-289` (TransferUnique, four times), `Item.cs:358-363` (SetCapacityCosts with rollback); `Inventory.cs:1230-1236` + `:728` (`EnsureNoDefinitionConflict` scans every stack on each Grant), `:501-512` (same scan per stack on Restore, O(n²)); `:407-419` + `:753,:798,:911-912,:974,:1029` (`InventoryState.Clone()` copies every stack per mutation); `:635-639` (`View` recomputes capacity and *throws* on a read if over capacity).
- **What it does:** Each mutation walks all stacks and all contained unique items, allocates a `used` dictionary, a `limits` dictionary and a sorted result list, twice (before and after), to produce `CapacityBefore/CapacityAfter` receipt lists and to refuse over-capacity. It also clones the whole `InventoryState` to apply one entry change, and `EnsureNoDefinitionConflict` compares the incoming definition against every existing stack's definition (`ItemDefinition.Matches`) to detect two different definitions with the same id.
- **Why it looks pointless:** The capacity delta of one change is `cost × quantity` for the touched stack; the full rescan re-derives a number that could be kept incrementally. Clone-on-write exists only so a failing mid-way check leaves the state untouched, but every check happens before the write. The definition-conflict scan guards against the product registering two inconsistent definitions under one id — the product owns both definitions; last-write-wins (or trusting `ItemDefinitionId` identity) would do.
- **Cost:** Per inventory operation, O(stacks + unique items + limits) ×2 with ~6 allocations; the per-Grant conflict scan is O(stacks).
- **Confidence:** High for the before/after recompute, Clone-on-write and conflict scan; Medium for dropping capacity from receipts (an API change).
- **Removal shape:** Keep `used[metric]` per inventory and update it with the delta; mutate `InventoryState` in place; drop `EnsureNoDefinitionConflict`/`EnsureDefinitionMatches`; make receipts carry the delta or the after-value only.

### 4. Engine-type component validators re-validate values the Engine just computed, on every entity write
- **Where:** `csharp/Rusty.Engine/Entities/EngineComponentTypes.cs:47-111` (ValidateTransform, ValidateCharacterMotion [14 fields incl. quaternion normalisation], ValidateSpatialCollider, ValidateKinematic); invoked from `csharp/Rusty.Engine/Entities/EntityStore.cs:730` on every `Set`, and `:701-709` (`BindDescriptor` re-validates every stored value when a descriptor is registered over an automatic table). Hot callers writing Engine-returned values: `csharp/Rusty.Engine/Entities/EntityCharacterController.cs:66-72` (`native.Transform`, `native.Motion`), `EntityDynamicsAdapter.cs:84-88` (`readout.Transform` per body), `EntityKinematicMotion.cs:84-91` (`candidate.AfterTransform`), `EntityOriginRebaser.cs:103-118` (`fact.LocalTransform`, `motion.Rebased`).
- **What it does:** Every write of a `Transform`/`CharacterMotion`/`Kinematic`/`SpatialCollider` runs a finite/positive/normalised check. The main per-step writers pass values the Rust controller, dynamics and world-origin services returned a few lines earlier.
- **Why it looks pointless:** "Defensive NaN/finite checks on values that would merely render oddly", and re-validation of Engine output. The Rust side already refuses non-finite scale/frames (`csharp-engine-services/src/appearance.rs:347`) and validates spatial inputs where it matters. A product writing NaN gets a NaN entity either way; the Engine does not panic on it. The quaternion tolerance check (0.001) can even refuse a rotation the Engine itself returned after float drift.
- **Cost:** Per entity per simulation step (every character step, every dynamics body readout, every kinematic candidate); `BindDescriptor` is O(values) once per registration.
- **Confidence:** High for the per-step validators on Engine-returned values; the `ComponentValidator<T>` mechanism itself is product-facing and can stay.
- **Removal shape:** Pass `validator: null` for the four Engine descriptors (delete `ValidateTransform` etc.); keep `ComponentValidator` for products that want one.

### 5. EntityGraphicsProjection duplicates the Engine's snapshot checks (duplicate ids, missing parent, parent cycle) and re-sorts every publish
- **Where:** `csharp/Rusty.Engine/Entities/EntityGraphicsProjection.cs:63-93` (OrderEntries: sort, adjacent-duplicate scan, positions dictionary, depths dictionary, visiting set, second sort), `:95-122` (GetDepth: recursive cycle/missing-parent refusal), `:45-48` (per-entry `GetLifecycle` must-be-active throw).
- **What it does:** Before `PublishSnapshot`, builds three dictionaries/sets over all entries, refuses duplicate entity ids, parents not in the snapshot, and parent cycles, then sorts by depth.
- **Why it looks pointless:** The Rust projector already does exactly this: `rust/crates/render-projection/src/runtime_appearance.rs:364` (`reject_duplicates`), `:496` (`MissingParent`), `:504` (`ParentCycle`). The C# layer therefore refuses the same inputs a second time, with the same outcome. (Whether Rust requires parent-before-child *ordering* should be checked before dropping the depth sort; the refusals are definitely duplicated.)
- **Cost:** Per publish (typically per frame), O(N log N) + 3 hash containers + recursion over N entries.
- **Confidence:** High for the duplicate/cycle/missing-parent refusals; Medium for the depth sort.
- **Removal shape:** Delete the scans and `GetDepth`; if Rust needs parent-first order, keep one stable sort, otherwise pass entries through.

### 6. Release-pair pipeline re-hashes and re-verifies the same bytes four to five times
- **Where:**
  - `scripts/build-runtime-pack.sh:196-204` hashes every pack file into `runtime-manifest.json`.
  - `scripts/build-csharp-release-pair.sh:141-147` hashes every payload (runtime-pack/** again) into `pair-manifest.json`; `:125-128` compares package/runtime identities; then `:151` runs `verify-csharp-release-pair.sh --directory` on the directory it just built.
  - `scripts/verify-csharp-release-pair.sh:83-93` re-hashes every pair payload, `:145-150` re-hashes every runtime-pack file *again* via runtime-manifest (each runtime file is hashed twice per verify run), `:116-137` repeats the identity comparisons build already did, `:139-143` re-runs `rusty-product-host --identity`.
  - `.github/workflows/pair.yml:101-141` (same job, same `$RUNNER_TEMP` bytes): build (verify #1) → `test-csharp-release-pair.sh` which runs `verify --archive` (`:20`, archive checksum + extract + hash everything = #2) and then `verify-pair.sh --directory` on the extracted tree (`:42` = #3) → `build-csharp-release-info.sh` → `publish-csharp-release-pair.sh:39` runs `verify --archive` (#4) and hashes the archive again at `:100`.
- **Why it looks pointless:** "Hashing performed at staging and then re-verified at a later step" and "inventory validation run more than once in a pipeline". Within one CI job nothing can change the bytes between steps; the archive checksum (`build-csharp-release-pair.sh:157`) is the one identity a downloader needs. The in-verify double hashing of runtime files (pair payload + runtime manifest) is redundant even for a single external verification.
- **Cost:** 4–5 full SHA-256 passes over the pack (hundreds of MB with symbols; the desktop variant is far larger) per pipeline run, plus 3 extra tar extractions.
- **Confidence:** High for the repeats inside `test-csharp-release-pair.sh` (`:42` right after `:20`) and the publish re-verify; Medium for the verify inside build (it doubles as the "the verifier works" proof — but that is what the tamper test is for, see #12).
- **Removal shape:** Verify once per job (or only in `publish`, which is the trust boundary); in the verifier hash runtime files once; drop `verify-pair.sh --directory` from the consumer test.

### 7. test-runtime-pack.sh hashes the entire pack before and after to prove an unrelated directory write did not change it
- **Where:** `scripts/test-runtime-pack.sh:57`, `:103-104`.
- **What it does:** `find | sha256sum | sha256sum` over every pack file twice, asserting equality after appending bytes to `$PRODUCTS/native/ui/main.js` and `content/trial.txt` — files in a different tree.
- **Why it looks pointless:** An "audit step that re-checks what the build already guarantees": nothing in the exercise writes under `$PACK`; the host runs with `--product` pointing elsewhere. The hash equality is a tautology.
- **Cost:** Two full hashes of the pack per test run.
- **Confidence:** High.
- **Removal shape:** Delete lines 57 and 103-104.

---

## Medium confidence

### 8. InventoryStore edits clone the whole store, reads clone states, and publication uses optimistic revision conflicts
- **Where:** `csharp/Rusty.Engine/Mechanics/Inventory.cs:646` + `:1053-1062` (`Prepare` copies every dictionary and every `SortedSet` of contained children), `:1064-1080` (`PublishCandidate` RevisionConflict), `:591-601` and `:615-625` (`TryGetInventory`/`TryGetEquipment` clone on read), `:537-549` (`RegisterInventory` clones and recomputes capacity).
- **What it does:** A multi-operation edit is a full copy of the store that replaces the original on publish, refusing if the store's revision moved meanwhile; single reads return defensive copies.
- **Why it looks pointless:** "Rollback/transaction mechanisms with no observed failure" and copying beyond what lifetime requires. The product is single-threaded and owns both the store and the edit; a revision conflict can only come from the product interleaving its own calls. Defensive copies on `TryGetInventory` protect against the product mutating its own state.
- **Cost:** O(whole store) per `Prepare`; O(stacks) per read.
- **Confidence:** Medium (the atomic-edit *semantics* may be wanted by the product; the whole-store copy is the pointless part).
- **Removal shape:** Record the edit's operations and apply them in order on publish (or drop `InventoryEdit` and let the product sequence calls); return the live `InventoryState` from reads.

### 9. Interaction/aim queries revalidate constant configuration and scan for duplicate ids every frame
- **Where:** `csharp/Rusty.Engine/Interaction/InteractionFocus.cs:168-176` (Validate(query) on every Evaluate/Revalidate), `:124-127` (HashSet duplicate-candidate-id refusal per query), `:144-145` (per-candidate finite checks), `:53` (cycleDirection range); `csharp/Rusty.Engine/Interaction/AimAssist.cs:100-109` (Validate(config) on every Update and CorrectShot), `:31-33`.
- **Why it looks pointless:** The query/config are product tuning constants validated on every per-frame call; the duplicate-id scan is a "duplicate guard where last-write-wins would do" on product-supplied candidate lists. A NaN candidate point would merely fail to be selected.
- **Cost:** Per frame, O(candidates) hash set + ~20 float checks.
- **Confidence:** Medium.
- **Removal shape:** Delete `Validate`, the HashSet scan and the finite checks; validate config once where the product constructs it if at all.

### 10. Input helpers re-validate deadzone/look configuration on every per-frame call although FpsInput validated it once
- **Where:** `csharp/Rusty.Engine/Input/AnalogInput.cs:24-41` + `:85-92` (ApplyRadialDeadzone validates deadzone params and finite stick on every call), `:44-61` (ApplyTriggerCurve), `:67-77` (CombineMovement finite checks); `csharp/Rusty.Engine/Input/FpsInput.cs:99-102` (finite delta per Read), `:149-164` (ValidateConfig in ctor — the right place); `csharp/Rusty.Engine/Look.cs:124-139` (Diagnose validates config + state on every Integrate/IntegrateClamped), `:173-184`.
- **Why it looks pointless:** `FpsInput` already validates the config at construction (`:156`), then calls the same helpers twice per frame which validate again. `Look.Diagnose` re-checks a constant `LookConfig` per frame. A non-finite pointer delta from the Engine cannot occur; a product-supplied one would merely produce a NaN look that `Reset` clears.
- **Cost:** Per frame, small but on every input read.
- **Confidence:** Medium.
- **Removal shape:** Remove per-call `Validate`/finite checks from `AnalogInput` and `Look`; keep (or drop) the one-time check in `FpsInput`'s constructor.

### 11. CameraQueries validates every input and every intermediate vector per call
- **Where:** `csharp/Rusty.Engine/CameraQueries.cs:82` (Project), `:115-118` (Ray), `:177-192` (Resolve: aspect, position, pitch, yaw, projection every call), `:235-262` (ValidateProjection), `:282-290` (Normalize refuses degenerate basis), `:297-305` (`ToVector3` refuses non-finite or > float.MaxValue on every intermediate), `:307-318`.
- **Why it looks pointless:** These are product-supplied camera values the Engine's own camera path accepts; a bad one yields a NaN projection, not a panic. `ToVector3`'s representability check runs on every `Add`/`Scale`/`Cross` result.
- **Cost:** Per call; products project many points per frame (labels, reticles).
- **Confidence:** Medium.
- **Removal shape:** Delete `RequireFinite`, `ValidateProjection` and the `ToVector3` check; keep the `TryLookAtPose` false-return (that is the API's documented contract).

### 12. Tests whose sole purpose is to prove a rejection or a tautology (scripts)
- **Where:** `scripts/test-csharp-release-pair.sh:358-369` (copies the entire pair with `cp -a`, appends a newline to `runtime-manifest.json`, runs the verifier again to prove it fails); `scripts/test-csharp-sdk-package.sh:57-60` (runs `pack-csharp-sdk.sh` a second time to prove it refuses to overwrite), `:503-518` (runs MSBuild `ValidateRustyEngineProduct` with an invalid value to prove it rejects); `scripts/test-ci-routing-checker.sh:20-47` (four negative probes of the routing checker, each copying `.github/workflows`).
- **Why it looks pointless:** "Tests whose sole purpose is to prove one of the above rejections." The tamper probe also duplicates the whole pair on disk.
- **Cost:** One extra full pair copy + verify per release test; one extra pack run per SDK test.
- **Confidence:** Medium (the tamper test is the only evidence the verifier verifies; if finding #6 keeps one verify step it can stay, cheaper, by tampering a single small file in place).
- **Removal shape:** Delete the overwrite/invalid-value probes; keep at most one tamper probe without `cp -a`.

### 13. Stat/Track validate finiteness and enum definedness on every evaluation and every arithmetic result
- **Where:** `csharp/Rusty.Engine/Mechanics/Stat.cs:223-229` (four `Finite` + `ValidateRounding` + inverted bounds + quantum per Evaluate), `:279-280,:287` (`Finite(amount)` and `Result()` per modifier op), `:91-92` (AddModifier `Enum.IsDefined`), `:33`, `:61`, `:186-190` (`ToFloat` range throw), `:251-252` (per-group policy consistency scan); `csharp/Rusty.Engine/Mechanics/Track.cs:15-22` (four `Enum.IsDefined` + finite checks in ctor), `:64-66`, `:100-110`; `csharp/Rusty.Engine/Mechanics/StatSources.cs:38-39`, `:57`.
- **Why it looks pointless:** Every stat change re-checks the stat's own stored bounds/quantum/rounding (set through the same validated setters) and re-checks `Enum.IsDefined` on enums the product passed from its own code. An `OverflowException` for a non-finite sum is a "sanity" refusal with no concrete failure behind it.
- **Cost:** Per stat/track change (combat-frequency), proportional to modifiers + sources.
- **Confidence:** Medium.
- **Removal shape:** Delete `Finite`/`ValidateRounding`/`Result`/`Enum.IsDefined` calls; keep the bounds clamp.

### 14. MechanicsSourceOrdering refuses duplicate provenance on every SetSources
- **Where:** `csharp/Rusty.Engine/Mechanics/MechanicsSources.cs:53-58` (HashSet scan after sorting), doc at `:28` ("Duplicate provenance is rejected instead of silently overwriting a source"); called from `Stat.cs:128-135` on every `SetSources`/`RemoveSource`.
- **Why it looks pointless:** Duplicate guard where a map insert or last-write-wins would do; the product authored both sources.
- **Cost:** Per SetSources, O(N) hash set.
- **Confidence:** Medium.
- **Removal shape:** Delete the scan (or `DistinctBy(identity)`).

### 15. InventoryState.Restore re-validates the product's own saved captures
- **Where:** `csharp/Rusty.Engine/Mechanics/Inventory.cs:449-479` (quantity/definition presence, `> MaximumQuantity`, `EnsureCompatibleDefinition` per stack = O(n²), `TryAdd` duplicate refusal, `ValidateFungibleCapacity`), `:481-499`; `InventoryStackCapture` ctor `:37-51`.
- **Why it looks pointless:** "Validated on import, then validated again on load": the captures were produced by `CaptureStacks` from an already-valid state and saved by the product's own codec. A hand-edited save is the product's concern (the Persistence docs say exactly that for state shape).
- **Cost:** Per load, O(stacks²).
- **Confidence:** Medium.
- **Removal shape:** Insert captures directly; at most keep the capacity total.

### 16. RustyEngineContentBundleManifestTask treats the product's own bundle declarations as hostile paths
- **Where:** `csharp/Rusty.Engine/buildTransitive/Tasks/RustyEngineContentBundleManifestTask.cs:68-74` + `:192-209` (IsSafeBundleId ASCII whitelist), `:88-92` + `:211-246` (TryCanonicalizeRelativeRoot refuses `.`, `..`, `<>:"|?*` and control chars), `:94-100` + `:248-255` (IsUnderRoot), `:122-137` (O(n²) root-overlap refusal), `:263-274` (throws "Staged bundle file escaped its declared root" on a path it enumerated itself).
- **Why it looks pointless:** Path-traversal / "unsafe" refusals on paths the product wrote in its own `.csproj`; the files come from `Directory.EnumerateFiles` under the declared root so they cannot escape it. The SHA-256 per file (`:179-180`) is legitimate: it is the one-time staged identity the Engine trusts (`content/bundles.rs:802`).
- **Cost:** Build time, once per stage; cheap.
- **Confidence:** Medium (cheap, but exactly the vocabulary the policy names).
- **Removal shape:** Keep "root does not exist"/"declared twice" errors; delete the character whitelist, `..` refusal, `IsUnderRoot`, overlap scan and the escape throw.

### 17. install-rusty.sh verifies the archive SHA-256 and then hands it to `rusty install --archive`, which verifies it again
- **Where:** `scripts/install-rusty.sh:82` (`sha256sum --check`), `:97` (`rusty install --archive`, whose Rust side re-hashes the same file: `rust/crates/rusty-cli/src/pair.rs:744-767`); same pattern in `scripts/install-rusty.ps1:53-57`.
- **Why it looks pointless:** One download, two full hashes in one flow. (The first hash of a downloaded archive is legitimate; the second is a repeat.)
- **Cost:** One extra hash of a ~100 MB+ archive per install.
- **Confidence:** Medium (the Rust side is out of this survey's scope; the script-side check is the removable half).
- **Removal shape:** Drop line 82 and let `rusty install` do the one check (or vice versa).

### 18. Generated debug catalog refuses re-registration of a module instead of replacing it
- **Where:** `csharp/Rusty.Engine.ProductGenerator/DebugCommandCatalogGenerator.cs:290-291` (emits `AlreadyRegistered` into every product's catalog); mirrored by `csharp/Rusty.Engine/Debugging/EntityStoreDebugModule.cs:35-43`, `:71-80`, `:84-89` and `Debugging/EntityStoreMechanicsDebug.cs:14-17` (`TryAdd` + "already registered" throws), which then needed `ReplaceStore` (`:50-59`) as a workaround.
- **Why it looks pointless:** Duplicate-registration guard where last-write-wins would do; products replace stores and modules on restart, which is why `ReplaceStore` exists.
- **Cost:** Once per registration; negligible, but generator output lands in every product.
- **Confidence:** Medium.
- **Removal shape:** Assign the field unconditionally; delete `AlreadyRegistered`, `ReplaceStore` and the `TryAdd` throws.

### 19. PlanarRecipes proves convexity, non-degeneracy and O(n²) non-self-intersection of product-authored polygons
- **Where:** `csharp/Rusty.Engine/Implicit/PlanarRecipes.cs:96-169` (ValidateConvexPrism: per-vertex finite, per-edge length, winding, coefficient finite), `:195-234` (ValidateNoSelfIntersections O(n²) with another finite check per pair), `:38-72` (Walkway re-validates each segment, then builds a rectangle and validates *that* via ValidateConvexPrism again at `:85`); also `csharp/Rusty.Engine/Implicit/RecipeContinuity.cs:9-19`, `:43-61` (matrix invertibility/axis checks), `Implicit/RoomRecipes.cs:28-51`, `Implicit/WallLayout.cs:40-54`.
- **Why it looks pointless:** A non-convex or self-intersecting prism yields odd geometry from the Engine's plane intersections, not a panic; the Engine accepts any planes. Walkway rectangles are constructed by this code and cannot be non-convex.
- **Cost:** Per recipe build (level construction), O(n²) per polygon.
- **Confidence:** Medium (one-time cost; policy still says odd-geometry checks are not findings worth keeping).
- **Removal shape:** Keep the `vertices.Length < 3` check that prevents an index panic; delete the rest, at least for internally constructed rectangles.

---

## Low confidence

### 20. Entity-store ceremony: enum-definedness, identity-space exhaustion, descriptor re-check per access, duplicate-registration refusals
- **Where:** `csharp/Rusty.Engine/Entities/EntityId.cs:16-25` + `EntityStore.cs:74,:126` (`EnsureDefined` on an enum); `EntityStore.cs:79-82` (ulong identity exhausted); `:571-580` (`GetTable` re-checks `ReferenceEquals(table.Descriptor, componentType)` on every typed Get/Set/Query); `:525-537` (family/key "already registered" throws); `:252-255` (`Add` refuses an existing component where `Set` would replace); `:444-450` (`Diagnostics` sample cap 64); `Entities/ComponentType.cs:6-12`, `:33-40`, `:92-98` (key-range refusals); `EntityBatch.cs:25-28` (expected-id check); `StateMachine/StateMachineDefinition.cs:121-124` (revision exhausted), `:35-41`, `:53-58` (duplicate state/transition refusals where a HashSet insert suffices).
- **Why it looks pointless:** Checks the type system or construction already guarantees; ulong exhaustion cannot occur; duplicate guards on product-declared graphs.
- **Cost:** Per access for `GetTable`; otherwise once.
- **Confidence:** Low (cheap; mostly API-shape choices).
- **Removal shape:** Delete `EnsureDefined`, exhaustion checks and the `GetTable` ReferenceEquals; let `Add` behave as `Set`.

### 21. Mechanics identity and definition constructors: null/empty/duplicate ceremony
- **Where:** `csharp/Rusty.Engine/Mechanics/MechanicsIdentity.cs:21-33` (null + empty refusal) with twelve `TryParse` wrappers that catch their own exception (`:42-54` etc.); `Effects.cs:83-86` (source Distinct), `:39-51` (limits must be > 0); `Equipment.cs:32-35` (classification Distinct), `:375-386` (requested-slot Distinct); `ItemDefinitions.cs:16-21`, `:78-80`, `:164`; `Item.cs:17-20` (entity != 0); `Inventory.cs:8-18`, `:345-353` (duplicate capacity metric).
- **Why it looks pointless:** Duplicate guards at construction on product-authored definitions; `TryParse`-via-exception ceremony.
- **Cost:** Once per definition; negligible.
- **Confidence:** Low.
- **Removal shape:** Keep null-safety where a null would NRE later; delete Distinct scans and empty-string refusals; implement `TryParse` without throwing.

### 22. Debug tooling caps and format-only validation
- **Where:** `csharp/Rusty.Engine/Debugging/EntityStoreDebugModule.cs:26-28` (MaximumPageSize 64, MaximumResultLength 4096, MaximumStoreNameLength 64), `:235-241` (store-name whitespace/length rules), `:116-119` (`Enum.IsDefined(selector)`), `:226-233`; `Debugging/InteractionDebugModule.cs:14-15` (1..256 cap); `Debugging/SpatialMapSnapshot.cs:121-127`, `:421-432` (1..1024-cell cap, finite everything), `:434-442`, `:444-453`, `:331-337` (duplicate stable-id scan), `:130-135` (asserts the Engine returned `Columns*Rows` cells); `Debugging/SpatialGridSnapshot.cs:16-20` (≤31/≤8192 caps); `Debugging/SpatialClearanceSnapshot.cs:16-17` (8-unit cap); `Debugging/PlaytestTraversal.cs:39-40` ((0,8] cap), `:19`.
- **Why it looks pointless:** "Max-count limits with no concrete failure behind them" and re-checking an invariant the Engine guarantees (cell count). Debug-only; the only consumer is a developer or agent.
- **Cost:** Per debug command; negligible.
- **Confidence:** Low (debug surface, cheap).
- **Removal shape:** Delete the caps and the Engine-output assertion; keep page limits only if output size matters for the transport.

### 23. ProductContent/ProductContentBundle: dictionary `Add` on Engine-supplied inventories
- **Where:** `csharp/Rusty.Engine/ProductContent.cs:26-27`, `csharp/Rusty.Engine/ProductContentBundle.cs:28` (`Add` throws on a duplicate path the Engine's catalog cannot contain), `ProductContent.cs:28` (extra sorted copy for `ReadDirectory`).
- **Confidence:** Low. **Removal shape:** Use the indexer; drop the sorted copy if `ReadDirectory` order can follow the Engine's order.

### 24. PortableAssetContent.AtlasFrames re-checks Engine-decoded frames against the Engine-decoded texture size
- **Where:** `csharp/Rusty.Engine/PortableAssetContent.cs:68-75` (finite/positive extent, "frame exceeds texture" throw per frame).
- **Why it looks pointless:** Both facts come from Rust (`LoadPortableAsset`, `ReadTextureInfo`); a frame outside the texture draws clamped UVs, not a panic.
- **Confidence:** Low. **Removal shape:** Delete the checks.

### 25. SimulationScheduler scans and re-sorts every pending item on every admitted step
- **Where:** `csharp/Rusty.Engine/Application/SimulationScheduler.cs:263-294` (`DispatchStep`: `Where/OrderBy/ToArray` over all pending per step, then `ContainsKey`/`TryGetValue` re-lookups per item), `:314-317` (saturating counters), `:304-310`.
- **Why it looks pointless:** Whole-collection pass per step where a due-step ordered structure would visit only due work; the per-item re-lookup guards against a callback cancelling a sibling, which the revision counter already handles.
- **Confidence:** Low (design more than validation). **Removal shape:** Keep a `SortedDictionary<(dueStep, insertionOrder)>` or a min-heap.

### 26. Rusty.Engine.targets re-validates product configuration the runtime's product.json parser will refuse anyway
- **Where:** `csharp/Rusty.Engine/buildTransitive/Rusty.Engine.targets:140-173` (eighteen regex `Error` conditions on `RustyEngineProduct*` properties).
- **Why it looks pointless:** Schema re-validation at build of values the Rust host validates on load; the build-time message is nicer, which is the only reason to keep it.
- **Cost:** Build time, once. **Confidence:** Low.
- **Removal shape:** Keep only the "declared together" pairs; drop enum regexes if the host's load error already names the property.

### 27. ProductBridge/EngineTestHost small ceremony
- **Where:** `csharp/Rusty.Engine/NativeProduct/ProductBridge.cs:116` (`ProductFault.MaximumLength` 600 truncation of the summary line), `csharp/Rusty.Engine/Testing/EngineTestHost.cs:121` (`_inCall` re-entrancy throw), `:166-167` (`File.Exists` before `NativeLibrary.Load`).
- **Confidence:** Low (all cheap, once per call; the truncation is a format choice). **Removal shape:** Leave or delete; no performance effect.

### 28. Binding generator emits a per-product enum-discriminant check only for one request type
- **Where:** `csharp/Rusty.Engine.BindingGenerator/Program.cs:908-920` (`RequireEnum` for `NativePresentationParticleDescriptor` only).
- **Why it looks borderline:** An undefined discriminant reaching Rust is a real ABI concern (UB on a `#[repr(u32)]` enum match), so the check is legitimate in kind; it is inconsistent (no other enum-bearing request gets it) and is emitted into every product. If Rust reads these as integers and matches with a fallback, it can go.
- **Confidence:** Low. **Removal shape:** Either make Rust tolerate unknown discriminants (then delete) or leave as the one ABI check it is.

### 29. test-csharp-sdk-package.sh / test-csharp-release-pair.sh audit greps of whole trees
- **Where:** `scripts/test-csharp-sdk-package.sh:267-274`, `:519-526` (`rg -F` the repo path across the consumer tree and package cache; `find` for provider files), `scripts/test-csharp-release-pair.sh:161-168`; `scripts/test-csharp-release-pair.sh:294`, `:324` (hash two refusal fixtures before/after the host run).
- **Why it looks borderline:** The tree greps prove packaging isolation (a real guarantee of the package, not of the build), so they are closer to legitimate; the refusal-fixture hashes prove a documented Engine guarantee on two tiny files.
- **Confidence:** Low. **Removal shape:** Leave; listed so the owner does not re-survey.

### 30. generate-csharp-native-bindings.sh hashes every generator input and output to decide whether to regenerate
- **Where:** `scripts/generate-csharp-native-bindings.sh:42-58`, `:94`.
- **Why it looks borderline:** It is an incremental-build cache keyed by content hashes (inputs + outputs), run per build. Legitimate purpose; the output-hash half only guards against someone editing an ignored generated file.
- **Confidence:** Low. **Removal shape:** Could key on inputs only (mtime or hash); not a priority.

---

## Looked at and judged legitimate

- `ProductBridge.cs` null-pointer / pointer-length coherence checks (`:239`, `:272`, `:316-320`, `:339`, `:364`, `:686-691`, `CopyKeyboardControls`), `GCHandle` lifetime, `catch`-all at every `UnmanagedCallersOnly` edge, `StrictUtf8` on borrowed command text — ABI.
- `ProductBridge.Bind` protocol/fingerprint handshake (`:216-233`) and `csharp-product-runtime/src/lib.rs:603-640`: one pair identity (protocol + table sizes + fingerprint) checked once at bind — the single allowed pair check.
- Generated `NativeConversions` "length without storage" throws and `NativeAnimationFeedbackText` 96-byte inline bound (`BindingGenerator/Program.cs:580`, `:609`) — pointer/length coherence and inline-array bound.
- `BindingGenerator` `Validate*` family (`Program.cs:127-370`) — generator-time ABI grammar; runs once at generation, and `test-csharp-binding-generator-results-fixture.sh` proves it against a hand-written header.
- `AbiIdentity` SHA-256 of the parsed ABI model (`Program.cs:34-66`) — computed once at generation; the pack script reads it rather than recomputing (`pack-csharp-sdk.sh:66-73`).
- `RustyEngineContentBundleManifestTask.ComputeSha256` per staged file (`:179-180`, `:276-288`) — the one-time staged inventory identity the Engine trusts on open (`content/bundles.rs:802`); not recomputed in C#.
- `DebugCommandCatalogGenerator` duplicate-command-name diagnostic (`:61-72`) — ambiguous dispatch at compile time, once.
- `DebugCommandLine.TryTokenize` quoting rules; `VoxelCellState.Encode` bit-packing range checks (`VoxelStateValues.cs:7-9`) — representation.
- `PersistenceStorageException.Classify`, `ProductStateStore` key/scope whitespace checks — cheap, product-facing, one call per save.
- `EngineTestHost.Create` pinning and `NativeCall.Require` — ABI.
- `EntityStore.SetContainment` cycle walk (`:178-184`) — O(depth), prevents an infinite loop in the store's own ancestor walks (a concrete failure).
- `EntityStore.RequireEntity/RequireAlive` — map lookups that give a clear error where `Get` would otherwise throw a bare KeyNotFound.
- `Track.PrepareMaximum`/`Stat.PrepareTracks` two-phase update — needed so a refused stat change leaves dependent tracks consistent.
- `UpdatePipeline.Register` unknown-phase check — once at setup.
- `WorldInteraction.Use` revalidation at use time (`WorldInteraction.cs:49-65`) — the documented contract (fresh facts at use) rather than defensive; its *inner* `Validate(query)` is covered by #9.
- `dependency_boundary_check.py`, `audit-standalone.sh`, `check-ci-routing.py`, `check-doc-links.sh` — repository policy gates, run once in `verify.sh`/`verify-docs.sh`.
- `pack-csharp-sdk.sh:29-32`, `build-runtime-pack.sh:52-56`, `build-csharp-release-pair.sh:60-71` — immutable-artifact / clean-checkout refusals (publication policy, once).
- `build-csharp-release-pair.sh:157`, `build-desktop-runtime-pack-archive.sh:63` — the one archive checksum a downloader needs.
- `publish-windows-pair-packs.sh:54`, `:90` and `publish-csharp-release-pair.sh:71` — checksums of archives that crossed a network/SSH hop.
- `inventory-validation.py` — the owner's own heuristic survey tool; not a pipeline step (excluded from CI).
- `performance-results.mjs` `validateRecord/validateArtifact` — parses JSON produced by another process at first read (file-format parse).
- `capture-playtest-warning-delta.mjs:53` identity hash — a baseline key, computed once per capture.

## Coverage

**Read fully (C#):** `Rusty.Engine/NativeProduct/ProductBridge.cs`, `ProductContent.cs`, `ProductContentBundle.cs`, `PortableAssetContent.cs`, `Testing/EngineTestHost.cs`, `buildTransitive/Tasks/RustyEngineContentBundleManifestTask.cs`, `buildTransitive/Rusty.Engine.targets`, `Rusty.Engine.props.in`, `Rusty.Engine.NativeAotRestore.props`; `Entities/*` (EntityStore, EngineComponentTypes, ComponentType, EntityId, EntityGraphicsProjection, EntityTriggerProjection, Actor, EntityOriginRebaser, EntityDynamicsAdapter, EntityCharacterController, EntityKinematicMotion, EntityMotionResolver, EntityBatch, EntityTypeId, EntityStoreDiagnostics, EntityStoreDebugSnapshot); `Mechanics/*` (Inventory, Item, ItemDefinitions, Equipment, Effects, Stat, Track, StatSources, MechanicsSources, MechanicsIdentity, StatsComponent, StatsComponentCapture, InventoryComponent, EquipmentComponent); `Debugging/*` (EntityStoreDebugModule, EntityStoreMechanicsDebug, DebugCommandContracts, DebugExecutionContext, InteractionDebugModule, PlaytestDebugModule, PlaytestTraversal, SpatialMapSnapshot, SpatialGridSnapshot, SpatialClearanceSnapshot); `Implicit/*` (PlanarRecipes, WallLayout, RoomRecipes, ImplicitRecipe, RecipeContinuity, SampledRecipeSurface, ArchitecturalRecipes); `Interaction/*`; `Input/*`; `Persistence/*`; `StateMachine/*`; `Application/*`; `CameraQueries.cs`, `GraphicsConvenience.cs`, `TweenConvenience.cs`, `PresentationConvenience.cs`, `SpatialConvenience.cs`, `Look.cs`, `SessionConvenience.cs`, `GameplayTimeConvenience.cs`, `UiValues.cs`, `SurfaceCharacterValues.cs`, `HostConvenience.cs`, `UiFonts.cs`, `UiImages.cs`, `DynamicsBodyConfig.cs`, `DynamicsRagdollConvenience.cs`, `WorldOriginValues.cs`, `VoxelScatterConvenience.cs`, `VoxelStateValues.cs`, `CameraViewConvenience.cs`, `CameraViewports.cs`, `CameraTargetReference.cs`, `CharacterMovementValues.cs`, `CharacterTetherValues.cs`, `AnimationPoseConvenience.cs`, `ImplicitGenerateRequest.cs`, `ImplicitTextureMapping.cs`, `MeshResourceReference.cs`, `RenderResourceReference.cs`, `RenderResourceRequest.cs`, `PersistenceSaveReceipt.cs`; `Rusty.Engine.BindingGenerator/Program.cs` (all 1382 lines); `Rusty.Engine.ProductGenerator/ProductGenerator.cs`, `DebugCommandCatalogGenerator.cs`; `Rusty.Engine.ApiSurface/Program.cs`.

**Skimmed (C# examples, grep + one file read):** `Rusty.Engine.Mechanics.Example/ManagedLimitsExercise.cs` (read fully; it exists to prove former caps are gone and one overflow refusal), `Mechanics.Example/Program.cs` and the other `*Exercise.cs`, `Entities.Example/*`, `Application.Example`, `Implicit.Example`, `Content.Example`, `Input.Example`, `Interaction.Example`, `CameraQueries.Example` — grep-skimmed for `Expect*/Throws` patterns (45 rejection-expectation sites across the examples; they test the SDK refusals listed above and go with whichever of those are removed).

**Read fully (scripts):** `inventory-validation.py`, `generate-csharp-native-bindings.sh`, `pack-csharp-sdk.sh`, `pair-platform.sh`, `audit-standalone.sh`, `build-runtime-pack.sh`, `build-csharp-release-pair.sh`, `verify-csharp-release-pair.sh`, `build-csharp-release-info.sh`, `publish-csharp-release-pair.sh`, `build-desktop-runtime-pack-archive.sh`, `publish-windows-pair-packs.sh`, `test-runtime-pack.sh`, `test-csharp-release-pair.sh`, `test-csharp-sdk-package.sh`, `verify-csharp.sh`, `verify.sh`, `verify-all.sh`, `verify-docs.sh`, `check-doc-links.sh`, `install-rusty.sh`, `test-csharp-binding-generator-results-fixture.sh`, `test-ci-routing-checker.sh`, `dependency_boundary_check.py`, `check-ci-routing.py`, `gpu-lane.sh`, `gpu-lane-pair.sh`, `gpu-lane-recapture.sh`; plus `.github/workflows/csharp.yml`, `verify.yml`, `pair.yml` (grep) for pipeline ordering.

**Grep-skimmed only (scripts):** `capture-playtest-warning-delta.mjs` (+ `.test.mjs`), `performance-results.mjs` (+ `.test.mjs`), `find-coreclr-runtime.py`, `install-rusty.ps1`, `run-performance-regression.sh`, `run-cpu-performance-baseline.sh`, `measure-*.sh`, `test_architecture_checks.py`, `scripts/fixtures/*`.

**Rust files consulted only to confirm duplication (not surveyed):** `render-projection/src/runtime_appearance.rs` (duplicate/parent checks), `csharp-engine-services/src/content.rs` and `content/bundles.rs` (inventory identity use), `csharp-product-runtime/src/lib.rs:587-640` (handshake), `rusty-cli/src/pair.rs:744-767` (install checksum).
