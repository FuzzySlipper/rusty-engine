# Validation operation audit — first pass

Task **#7872**, 2026-09-07. This is an operation/family ledger, not approval of
all rows in the [raw inventory](validation-inventory.md). Bounded source surveys
supply leads; dispositions below belong to the root audit. Unresolved means
unresolved, including when a survey found no actionable defect.

## Operations traced

| Operation | Current ownership and byte consumer | Disposition |
|---|---|---|
| Publish generated mesh | C# borrowed streams → `csharp-engine-services/appearance.rs::create_mesh_resource` → typed appearance projector → `composition.rs` / `PresentationWorld` → `RuntimePublication` → host output → worker IPC / browser SSE | Generated size-only encoding was removed in #7871. Host typed→JSON→typed conversions removed here. Repeated structural scans and group cap remain under #7874. |
| Publish retained presentation | `PresentationWorld::{apply,apply_presentation}` stages state/revisions → runtime publication → host adapter → actual delivery | Preserve staging, reference resolution, revision ordering and transient-event semantics. Remove serialization used only to convert internal representations. Delivery preflight remains under #7873. |
| Load resource | Rust resource admission/packing → retained bundle bytes and preload descriptor → HTTP browser fetch → renderer resource host → decoder/backend | File bytes, descriptor JSON and browser decoding have present consumers. Whether every repeated hash, copy and capacity restriction is needed remains a separate resource-family review; a real boundary does not justify all its checks. |

## Completed changes

- `product-dev-host/src/model.rs`: retain render frames, view composition,
  presentation frames and UI envelopes as typed values. Move owned publication
  payloads into the adapter after its existing admission check, avoiding a
  second validation and clone in those frame constructors. Reverse conversion
  and transient-event selection no longer encode/parse JSON intermediates.
- `runtime-ui/src/model.rs`: serialize the canonical envelope directly from
  borrowed fields, preserving decimal-string identities and sequence. Wire
  deserialization goes through the existing envelope constructor. This removes
  the cloned DTO tree previously used for encoding.
- `runtime-publication/src/lib.rs`: stop calling `encode_json` merely to
  validate an immutable, privately constructed UI envelope. Its public surface
  does not permit arbitrary payload mutation after admission.
- `render/packages/render-projection/src/retained-projection.ts`: replace
  stringify/parse deep copying with structured cloning. Incoming mesh arrays and
  returned node snapshots remain detached; atomic frame tests still pass.
- `product-dev-host/src/host.rs`: serialize the outgoing batch from a borrowed
  typed wrapper, rather than building another `serde_json::Value` tree first.

These changes preserve the browser schema. JSON still belongs at actual worker
and browser delivery, UI DTO input, files, and diagnostic output. This pass does
not replace existing transport protocols or claim that serialization happens
only once throughout the complete pipeline.

## Remaining decisions

| Family | Evidence / next question | Owner |
|---|---|---|
| Output size preflight | `fit_publication_budget` builds host outputs; `validate_output_group` calls `encoded_output_bytes`, discarding bytes before the actual output batch is encoded. Consolidate at the delivery owner and review budgets with baseline recovery/frontiers intact. | #7873 |
| Repeated mesh scans | Definition admission, frame construction, retained world input/output, publication and receipt checks revisit mesh data. Frame fields remain publicly mutable, so validate the actual ownership guarantees before removing each entrypoint scan. | #7874 |
| Resource caps and rehashes | Preload, application content and renderer resource hosts repeat checks. Establish copied-buffer ownership, replacement/cache behavior and concrete consumers before consolidating. | #7876 |
| UI admission size/shape limits | UI construction still measures payload/envelope bytes; browser UI has corresponding limits. Review both owners together. Removing only one check would leave a delayed rejection. | #7875 |

## Evidence and scope

Host tests cover ordered batches, fragmentation, atomic rejected output,
connection baselines and wire round trips. The extended wire test compares
render/UI JSON to the canonical schema and decodes it back to typed publication.
Runtime UI/publication tests exercise existing admission contracts; runtime
publication-budget tests exercise small updates and large retained replacement.

This is a first pass over three operations and selected serialization families.
It is not a semantic inventory of every execution branch. Input, persistence,
spatial queries, importers and codec sibling findings require their own bounded
operation audits; they are not approved by omission. No new browser/GPU visual
or performance measurement is claimed for these internal changes.

The serialization reconnaissance snapshot is
`artifacts/validation-audit-7872/serialization-sites.txt`: line-local matches
for Rust serde_json encoding/from_value, JavaScript JSON.stringify, and C#
JsonSerializer.Serialize across first-party crates, C#, render packages and
scripts. It includes tests and is a lead list, not a count of distinct runtime
serializations; helper functions, other serializers and macro expansion remain
coverage gaps.

Verification passed: 42 host unit tests, 25 host integration tests, 5 publication
tests, 9 UI tests, and 4 focused C# runtime publication-budget tests; focused
Clippy with warnings denied. SDK/runtime `audit2` were built and adopted by
CraftSurvive; the broker restarted with matching launch/current fingerprints.
Debug readout restored seed 11, `level-disrupted`, normal detail and `level-hub`,
with `generationError=none`. The existing nine non-manifold edges remain #7870.

## Resource survey findings

The resource lane continues through `csharp-product-runtime::admit_renderer_resources`
into product-dev bundle admission, browser preload, application-content preparation
and resolvers, renderer-host resource admission and Three realization. Repeated
hash/copy candidates are mapped to #7876. Rust resource admission and product-dev
admission both verify resource bytes; browser preload and renderer-host repeat
hashes; Three's texture path hashes again before PNG decode. Application preparation,
resolvers and realization also copy bodies. Public resolver inputs can be independently
supplied, so eliminating internal repeats requires establishing their ownership,
not declaring every entrypoint trusted solely because the product is trusted.

`render-model::pack_mesh_resources` computes a newly packed body's hash and then
calls `PackedMeshResource::validate`, which recomputes it. That sibling belongs
in the same resource work. A decoder with only test call sites was not counted
as established production overhead. No particular oversized product resource was
reproduced in this audit, so the repeated caps remain policy candidates.

## Serialization family dispositions

- Retained projection `clone`: confirmed in-process stringify/parse with no byte
  consumer; fixed with structured cloning. All 27 projection tests pass,
  including new incoming/returned mesh-array isolation coverage.
- UI value/envelope size-only encodes: confirmed in Rust and application-host
  TypeScript. `runtime-ui/channel.rs` also validates before constructing an
  envelope that validates again. #7875 owns admission and matching browser caps.
- Timeline opaque payload reconstruction: confirmed private-wrapper clone and
  size-only encoding; #7875 owns the current-consumer/ownership cleanup.
- Timeline catalog inspection preflight: bytes are discarded at catalog
  construction. No production consumer of the explicit inspection JSON method
  was established; absence of a search hit is not proof of absence. #7877.
- Diagnostics batches: per-event size encodes under the sink lock precede typed
  batch return. Root traced actual consumers to product-dev-host logs and C#
  worker forwarding; these are real delivery operations, not dormant code.
  #7877 owns consolidation with cursor/lag semantics preserved.

Next review order: #7873 delivery encoding, #7876 resource verification/copies,
#7874 mesh scans/material groups, #7875 opaque payload admission, then #7877
inspection/diagnostics. This is five bounded families, not 35,000 approvals.

## Second pass — delivery owner (#7873)

The first-pass remaining-decision table above is historical. This pass resolves
#7873; #7876 has two local fixes but its cross-stage ownership work remains open.

Removed the 16 MiB incremental / 256 MiB baseline byte caps and the 256-output
admission cap. No physical representation required those values. Their size-only
encodings and `fit_publication_budget` reconstruction path are gone. A large
retained change now remains a delta; connection/recovery snapshots still use the
existing complete-baseline path. No product callback is replayed for size.

The host encodes the actual batch once, then emits it or constructs actual
fragment envelopes from those bytes. The 256 KiB event / 96 KiB fragment sizes
remain delivery chunking choices, not maximum scene/update sizes. The 256-event
reconnect history is a target: it expands to retain the newest incremental batch
whole, preventing self-eviction. Subsequent output can age that transfer out;
existing cursor-lag recovery then requests a fresh private baseline. Baseline
staging, ordering, revisions, transient events and fail-atomic publication remain.

Browser aggregate and output-count cutoffs are removed. `maximumOutputBytes` is
an optional caller-selected per-batch limit and accepts any positive safe integer;
its default only reflects numeric representation. Request/response budgets are
unchanged. Worker framing no longer borrows the resource-bundle capacity limit;
its u32 byte-length prefix remains an actual representation bound. Thus no
arbitrary output byte policy remains on this path, but memory, browser/backend
capacity and worker framing still constrain possible transfers.

The worker adapter also contained `Value → bytes → typed` and
`typed → bytes → Value` conversions. It now converts directly between Value and
the typed wire shape; only the writer encodes the worker message bytes.
`validate_output_group` now checks baseline marker coherence without serialization
or size/count admission. Broader repeated semantic scans remain #7874.

Resource sibling fixes under #7876:

- `pack_mesh_resources` no longer rehashes its newly constructed bodies by
  immediately calling the public resource validator. Independently supplied
  resources still have that validator; pack/round-trip/tamper tests pass.
- Six product-dev resource admission paths no longer clone an entire body into
  a temporary bundle entry only to check its path, media type and byte length.
  Those existing checks use metadata directly. This does not approve the
  resource caps or resolve the remaining Arc/Vec/resolver ownership work.

Validation: 41 host unit tests, 25 host integration tests, 20 worker binary tests,
2 focused runtime publication tests, 10 mesh-resource tests, 5 bundle tests and
89 browser-host tests passed. Focused Rust Clippy passed with warnings denied.
Tests cover >16 MiB ordinary output, an incremental transfer longer than the
reconnect history, >256 MiB baseline metadata without allocating a giant fixture,
257-output baselines, explicit caller budgets, malformed fragments and recovery.
Superseded tests that demanded rejection/reconstruction at old policy caps were
updated to assert current delivery behavior. This is functional evidence, not a
performance benchmark or a claim that arbitrary allocations will succeed.

Deployment for the second pass: SDK/runtime `audit3` built and replaced the
CraftSurvive pair. Broker restart succeeded with matching launch/current
fingerprints; seed 11 / disrupted / normal / hub readout reports
`generationError=none`. Existing topology diagnostics are unchanged (#7870).
No new GPU visual or performance benchmark was required or claimed.

## Third pass — resource ownership (#7876, continuing)

Engine-admitted resource bodies now share immutable `Arc<[u8]>` storage through
C# Engine services, product runtime, product-dev resources, bundle entries and
static HTTP responses. The runtime carries the existing identity and hash via
`ProductDevRendererResource::from_retained` rather than copying the body into a
raw import and decoding/hashing it again. The constructor documents its trusted
Engine-service caller contract. Independently supplied raw resources still use
`admit_*`; existing bundle metadata restrictions remain pending review.

Bundle and response clones share bodies rather than cloning their contents.
This is process-local ownership sharing, not zero-copy IPC or networking: real
worker delivery and socket writes still transfer bytes.

Three's texture resource source now explicitly lends stable encoded bytes until
release. Synchronous decoding creates owned pixels before release, so its extra
encoded-body `.slice()` is removed. The regression test lets the provider erase
its encoded view at release and checks that the decoded texture survives.
Three's hash check remains: its public provider interface can independently
supply bytes, even though the usual renderer-host loader already hashes them.
Consolidating that verification needs an explicit ownership/API decision.

Unresolved #7876 work: browser preload/application-content/resolver copies and
repeat verification; the resource bundle 64 MiB, mesh 64/256 MiB, and texture
16/128 MiB limits. These are not approved by omission. This pass does not claim
all resource admission is necessary or that all copies have been eliminated.

Validation: 42 host unit tests, 25 host integration tests, the focused selected
resource transaction test, and 251 Three renderer tests passed. Product-runtime
compilation and focused Rust Clippy with warnings denied passed. The shared-body
test checks pointer identity through resource/bundle clones and lifetime after
the earlier owners are dropped. No performance benchmark was performed.

SDK/runtime `audit4` built successfully and replaced CraftSurvive's previous
pair. The broker restart reported matching launch/current fingerprints.
Seed 11 / disrupted / normal / hub readout reports `generationError=none`,
183,158 triangles and 142,802 vertices. Topology diagnostics remain 0 boundary
edges, 9 nonmanifold edges (#7870), and 0 inconsistent winding edges. This is
runtime debug readback, not new GPU visual evidence.
