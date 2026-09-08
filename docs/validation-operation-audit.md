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


## Fourth pass — host content and bundle quotas (#7876, continuing)

Removed the immutable host bundle's 64 MiB per-entry, 256 MiB aggregate and
4,096-entry admission limits. Bundle bodies are byte slices stored by path and
served with their actual length; those mesh-derived policies were unrelated to
the HTTP consumer. The bundle retains checked `usize` addition for its reported
byte total, unique normalized paths, admitted media types and required index.

Tracing the removed constant found another owner: ordinary C# content import
borrowed the mesh/bundle 64 MiB cutoff, with an 8,192-file and 256 MiB total quota.
Removed that private quota machinery, its metadata-size accounting and repeated
post-read quota admission. Content still loads once as immutable bytes in
canonical order. Path collision, regular-file and read-error behavior remain.
These changes do not bypass codec checks when C# selects a renderer resource.

This is the host-content slice of #7876. Mesh-format and browser resource quotas
remain unresolved, so admitting a large file or bundle is not a claim that all
renderer paths can consume it yet. Browser ownership/copy consolidation also
remains open. No size-derived output serialization was introduced in its place.

Validation: 43 host unit tests, 25 host integration tests and five focused C#
content-admission tests pass; focused runtime/host Clippy passes with warnings
denied. New tests admit an actual file above 64 MiB and a bundle above 4,096
entries / 256 MiB logical bytes. Shared immutable bodies keep the latter test's
allocation to one 64 MiB body and verify that sharing survives admission. The
old quota-only test was replaced with actual content admission evidence.

SDK/runtime `audit5` built and replaced CraftSurvive's pair; broker launch/current
fingerprints match. Seed 11 / disrupted / normal / hub runtime readback reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges tracked by #7870, 0 inconsistent winding).
No new GPU visual proof or performance measurement is claimed.

The bounded browser survey identified the remaining coupled owners:
`product-browser-host/src/renderer-preload.ts`,
`application-host/src/application-content.ts`, renderer-host texture/mesh resource
loaders, `render-contracts/src/validation.ts`, and Three's retained texture budget.
The next cap decision must trace all of these together; merely sharing constants
would preserve the restrictions. Decoded texture memory is distinct from encoded
file size, but neither a format label nor a backend label proves a numeric cap
necessary. Likewise, copies currently implement caller-byte detachment and
resolver snapshots; removing them needs a clear retained/borrowed ownership
contract. The survey is evidence, not approval of those existing policies.

## Fifth pass — packed mesh capacity across consumers (#7876, continuing)

The RMesh format stores body lengths and stream offsets as `u32`; 64 MiB was a
policy ceiling, not an encoding requirement. `MAX_MESH_RESOURCE_BYTES` now names
that actual `u32::MAX` capacity. Removed the 256 MiB aggregate packing preflight
and its private size-only summation/error: resource bodies are separate buffers,
with no aggregate length encoded in the format. Explicit `maximum_resource_bytes`
packing requests still determine partition size and reject a single mesh that
cannot fit. Partition arithmetic now avoids intermediate `usize` addition
overflow when a caller selects the full format range on a 32-bit host.

Found and removed a remaining copy of the previous host's generic 64 MiB byte
quota in C# resource selection. Its six duplicate normalized-path checks also
went away: each path was already admitted by `renderer_path`. Resource codecs,
identities and transaction semantics retain their existing owning checks.

The browser mesh path is updated as a unit: preload, application-content,
renderer-host manifest and render-contract payload validation. RMesh/GLB length
representation and structural checks remain; arbitrary mesh resource-count and
aggregate-byte policies are removed. Texture/audio policies, Three texture
realization budgets and borrowed-resource/copy contracts remain unresolved.

The same audit found animated clip packs had separate 16-pack / 32 MiB total
admission policies in renderer-host; those are removed too. Ordered clip-pack
loading and loader failures remain. A GLB length check belongs at the Three
loader, not an additional duplicate in renderer-host. The application-content
GLB branch now covers clip packs as well as animated meshes; the prior branch
incorrectly sent clip packs through the RMesh media-type check.

Rust evidence: all 39 render-model unit and five contract integration tests
passed before the added large-body regression; the final 11 mesh-resource tests
include a real RMesh body above 64 MiB, admitted and decoded back to the expected
streams. Two voxel-object projection tests and selected-resource transactional
admission passed. Focused Rust Clippy passed with warnings denied. Capacity
metadata tests cover `u32::MAX` descriptors without allocating a 4 GiB fixture.
This does not claim a particular GPU can allocate that much memory.

Browser validation: 26 render-contract tests, 126 renderer-host tests, 35
application-host tests, 86 product-browser-host tests and 251 compiled Three
tests passed. Descriptor/fake-resolver probes cross the old 64 MiB / 256 MiB /
1,024-resource policies without allocating a giant browser scene; clip-pack
coverage admits 17 packs and a 32 MiB-plus-one body. These are focused admission
proofs, not GPU capacity or frame-time measurements. Runtime pack and SDK
`e3bf354c9f51.audit6` built successfully.
CraftSurvive adopted that pair; broker launch/current fingerprints match.
Seed 11 / disrupted / normal / hub readback reports `generationError=none`,
183,158 triangles, 142,802 vertices and unchanged topology diagnostics
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding).


## Sixth pass — texture/audio byte and collection quotas (#7876, continuing)

Removed texture 16 MiB encoded-body, 128 MiB collection and 256-resource policy
limits across preload, application-content and renderer-host, with Rust and
render-contract texture payload lengths retaining their existing `u32`
representation. Removed audio 8 MiB body / 32 MiB collection / 64-resource
policies in preload and application-content. Existing media/identity/length
coherence checks remain, as do codec failures and actual allocation failures.

Three's retained texture limits (256 textures, 128 MiB encoded, 256 MiB decoded)
were constants, not GPU capacity queries. Removed that prospective budget
helper and the per-frame readout-map/summation work used only to enforce it.
A set still tracks texture payload presence for material validation. Actual
resource statistics and prepare-before-commit/disposal behavior remain.

Texture dimensions (4,096) and texel count (16,777,216) remain explicitly
unresolved: those require a separate decoder/device-capacity decision across
render-model, asset-catalog, contract validation and backend realization. This
pass does not approve them, remove embedded-image import policies, or change
browser resource ownership/copy/hash contracts.

Validation: three focused Rust texture tests and focused Rust Clippy passed;
26 render-contract, 251 Three renderer, 127 renderer-host, 35 application-host
and 87 product-browser-host tests passed. Tests admit descriptor sets beyond
old texture/audio byte/count totals and retain/dispose 257 actual Three texture
resources. The former quota-only helper test is gone with the helper. Metadata
capacity evidence does not claim a multi-gigabyte GPU upload succeeds.
SDK/runtime `e3bf354c9f51.audit7` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match; seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). No new
GPU visual proof or performance benchmark is claimed.

## Seventh pass — embedded image references (#7876, continuing)

Removed animated GLB preflight's 16 MiB image-body / 32 MiB image-total quotas
and 256-image / 256-texture count caps. Images point at buffer views: the old
aggregate sum charged repeated references to the same body repeatedly, so it
was not even a measure of retained byte storage. Normal container/buffer-view
validation, MIME support, nonempty images and external-image policy remain.
Material/joint counts and the shared 64 MiB conversion-source limit are separate
unresolved restrictions; this does not claim arbitrary-size GLB import now works.

A focused container preflight test admits 257 images/textures referencing one
body larger than 16 MiB. It intentionally proves reference admission, not image
decoding or visual rendering. Ten existing animated-GLB import tests and focused
Rust Clippy pass. No change was made to browser source or image decoding.

Dimension investigation: installed Three WebGLTextures.resizeImage reads
`capabilities.maxTextureSize`, but oversized DataTexture data cannot take the
canvas/image resize branch; it warns and returns the image unchanged for upload.
Removing Engine's 4,096 dimension policy without handling actual backend
capacity would expose this incomplete failure path. Dimension/texel limits
remain pending an owning decoder/backend change, not approved as correct.
SDK/runtime `e3bf354c9f51.audit8` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub readback
reports `generationError=none`, 183,158 triangles, 142,802 vertices and unchanged
topology (0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding).
No new GPU visual proof or performance benchmark is claimed.
The bounded source trace located the capacity handoff: renderer-three's
`browser-surface.ts` constructs/applies the initial ThreeRenderer frame before
creating WebGLRenderer. The WebGL object exposes `capabilities.maxTextureSize`,
but Engine does not forward it. PNG decoding in `png-texture.ts` allocates
`width * height * 4` pixels before upload. The next owning change should obtain
and pass device capacity before initial texture preparation; no product-side
GPU policy or second renderer is needed.


## Eighth pass — device-owned texture dimensions (#7876, continuing)

Removed fixed 4,096 texture dimensions and 16,777,216 texels from render-model,
asset-catalog and the TS texture contract. Rust dimensions remain nonzero u32;
the TS descriptor uses that same representation. PNG decode checks that decoded
byte-length arithmetic is exactly representable before decompression/allocation.
No device capacity is fabricated for CPU-only ThreeRenderer use.

Browser mount now constructs WebGLRenderer inside the existing cleanup
transaction before ThreeRenderer/initial frame application. It passes the actual
`capabilities.maxTextureSize` to retained PNG preparation. Oversized dimensions
fail before acquiring encoded resource bytes, decoding or creating DataTexture;
failed preparation leaves the retained frame unchanged. Isolated capture
renderers inherit that device capability. Construction failures dispose both
owners through the existing cleanup list. GLB loader image behavior is separate.

Evidence: 59 Rust model/catalog tests and focused Clippy passed, including
large metadata dimensions and retained zero rejection. 253 Three tests and 26
contract tests pass. New tests decode an 8,192-wide PNG with sufficient supplied
capacity, reject an oversized replacement before borrowing, preserve resource
statistics on rejection, and enforce the same limit in isolated captures. These
are backend-unit capability tests, not a claim of physical 8K GPU upload proof.
SDK/runtime `e3bf354c9f51.audit9` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). No physical
large-texture upload, new GPU visual proof or performance benchmark is claimed.

## Ninth pass — binary GLB source capacity (#7876, continuing)

Removed the voxel-conversion 64 MiB input policy from the general typed GLB
import request and embedded GLB parser, plus the animated/static asset wrapper.
The shared GLB closure-root parser and asset-import CLI binary paths likewise
stop borrowing the text/source JSON 64 MiB limit. Actual binary GLB header and
length representation remain, with no new JSON parse-and-discard preflight.
CLI source selection remains unchanged; .glb reads use the ordinary file reader.

This pass concerns binary GLB roots, not every importer policy. Text/JSON input,
serialized mesh-import request envelopes, explicit voxel conversion CLI bounds,
external closure resource count/byte totals, material/joint counts and geometry
limits remain separate unresolved audit items. They are not approved by omission.

The integration regression pads both a static and an animated GLB above 64 MiB,
updates coherent container/BIN lengths, runs the full asset import, and asserts
exact retained source bytes. It proves the wrapper, typed importer and parser
agree; descriptor-only acceptance would not answer this path's question.
Validation passed: the focused large-GLB integration case, all 33 offline-import
tests, 13 voxel-convert unit tests, and Clippy for asset-import, voxel-convert and
C# Engine services with warnings denied. The source-size gate was not replaced
with serialized-size measurement or another full parse.
SDK/runtime `e3bf354c9f51.audit10` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). No new
GPU visual proof or performance benchmark is claimed.

## Tenth pass — browser resolver copy ownership (#7876, continuing)

Application surface resolvers now borrow prepared content bytes for meshes,
textures, animated meshes and clip packs instead of allocating a full buffer
copy immediately before renderer admission makes its own snapshot. The helper
contract explicitly forbids consumers from mutating or detaching these borrowed
buffers. No trust flag, registry or parallel resource path was introduced.

The initial application preparation copy still separates caller-owned buffers;
renderer admission still snapshots before asynchronous hashing and retains its
own verified bytes. Audio keeps a separate copy because decoder ownership may
consume/detach the input. Hash verification and other remaining copy sites are
not approved by omission; this pass removes the demonstrated adjacent duplicate.

All 35 application-host and 127 renderer-host tests pass. The revised integration
case checks original caller isolation, shared intermediate buffer identity and
renderer isolation from later prepared-owner mutation. Existing renderer tests
continue to cover mutation during asynchronous admission. No performance gain
has been measured.
SDK/runtime `e3bf354c9f51.audit11` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). This is
runtime debug readback, not a new GPU visual or performance measurement.

## Eleventh pass — external glTF resource capacity (#7876, continuing)

Removed the 256-resource, 64 MiB per-resource and 128 MiB aggregate quotas
from glTF/GLB source closure admission and the external-resource CLI reader.
Data URI decoding no longer inherits the external-file byte quota. Text/JSON
root and sidecar policies remain separate unresolved audit items; this does not
claim unlimited embedded data URIs through the bounded JSON-root path.

Resource indexing now borrows the supplied closure bytes. External buffer/image
resolution borrows until packing; decoded data URIs own their decoded bytes.
Binary GLB packing also borrows its parsed BIN chunk instead of cloning it again.
The packed output remains independently owned. Exact closure identities, URI
canonicalization, filesystem containment, duplicate/missing resource rejection,
nonempty resources, declared buffer lengths and actual packed GLB u32 length
remain. Removed quota-only byte accumulation; retained the reported source-byte
count's checked u64 arithmetic.

All 34 offline-import integration tests and the CLI unit test pass. The new
packing case contains 257 referenced resources, two above 64 MiB, a total above
128 MiB, and verifies all packed buffer bytes and reported source size. The CLI
case reads a real 64 MiB + 1 byte file. All-target asset-import Clippy passes
with warnings denied after a mechanical modulo-to-is_multiple_of cleanup in an
existing animated GLB test. No performance gain is measured.
SDK/runtime `e3bf354c9f51.audit12` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). This is
runtime debug readback, not a new GPU visual or performance measurement.

## Twelfth pass — offline JSON and request envelopes (#7876, continuing)

Removed inherited 64 MiB mesh-JSON/glTF-root admission limits and the mesh-source
request JSON envelope cap derived from the old conversion-source budget. These
are in-process decoders/offline files, not transport limits. Asset-import CLI
source reads and 4 MiB sidecar/prior-manifest reads now use ordinary filesystem
readers. The inspector's import-source command likewise stops imposing the
retired source cap; its other artifact policies remain separate.

The CLI previously swallowed a too-large prior-manifest read as missing via
`.ok()`, changing reimport planning despite having written the artifact itself.
The regression imports valid JSON above 64 MiB, pads the resulting manifest
above 4 MiB and requires the next CLI plan to report `reimportPlan: noop`.
A second case packs a glTF root above 64 MiB to the identical mesh bytes. The
mesh-source request regression decodes above the retired ~256 MiB envelope cap
and still rejects trailing non-JSON input. Padding isolates byte policy from
geometry complexity; these are not claims of large-geometry performance.

Also removed redundant glTF/GLB root parsing for resource discovery inside
admission: it now uses the parsed document already in hand. Real JSON parsing,
end-of-input checks, schema/field validation and geometry-specific restrictions
remain. Other material/joint/geometry, explicit conversion CLI and inspector
artifact policies, plus remaining copy/hash/parse work, are still unreviewed.

All 35 offline-import integration cases passed, with the strengthened CLI reuse
case rerun after its final assertion. The external-file CLI unit case and the
focused mesh-request envelope regression passed. All-target Clippy passed for
asset-import, voxel-convert and engine-inspector with warnings denied. No
performance improvement is measured.
SDK/runtime `e3bf354c9f51.audit13` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). This is
runtime debug readback, not a new GPU visual or performance measurement.

## Thirteenth pass — material slot representation (#7876, continuing)

Removed the animated GLB document-wide 256-material quota and the general glTF
importer's borrowed 4,095-entry voxel palette cap. Imported glTF material indices
are already checked against u32. The animated renderer's override mapping uses
u16 for both dense slots and source material indices; conversion now checks the
actually used indices and returns an import diagnostic instead of relying on
`expect` justified by a much smaller blanket quota. Unused material definitions
do not consume override slots. Voxel palette policies remain separate.

A full import with 4,096 used materials passes and preserves all override slots
and exact GLB bytes. The focused mapping case accepts used index 65,535 even
with more unused definitions, and rejects used index 65,536 as unrepresentable.
All 36 offline-import integration and 3 library tests pass. All-target Clippy
passes for asset-import and voxel-convert with warnings denied.

Joint and geometry limits remain unresolved. The joint family includes the
outer 4,096 summed-entry quota, Rust skin/per-skin limits and a browser animation
sampling limit; removing only the outer quota would not establish broad support.
These need a connected pass with downstream representation and sampling evidence.
No large-material GPU draw or performance result is claimed.
SDK/runtime `e3bf354c9f51.audit14` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). This is
runtime debug readback, not a new GPU visual or performance measurement.

## Fourteenth pass — joint capacity across import and sampling (#7876, continuing)

Removed the animated wrapper's 4,096 summed skin-joint quota, the general
importer's 128-document-skin quota and the Rust rig's 256-joint quota. Skin-local
vertex ordinals are u16; the per-skin table capacity now reflects 65,536 entries.
Joint node identities remain checked u32, with reachable-node, duplicate-joint,
inverse-bind and vertex ordinal checks intact. Document geometry/node/depth
quotas remain separate, so this is not a claim that arbitrary 65K-joint scenes
currently import or render on every device.

The new full animated import uses 257 named joints shared by 129 skins (33,153
summed entries), exercises vertex ordinal 256 and retains the unique 257-joint
rig, all skin accounting and exact GLB bytes. This distinguishes repeated skin
entries from unique skeleton nodes. All 37 offline-import integration, 3
asset-import library and 40 render-model tests pass. All-target Clippy passes
for asset-import, voxel-convert and render-model with warnings denied.
TS rig admission and Three CPU sampling no longer impose 256 joints/bones.
The contract accepts a 257-joint rig; the backend samples 257 named bones with
independent cloned skeleton state. All 27 render-contract and 253 renderer-three
tests pass. Rollback fixtures now invalidate an already-admitted decoded clip
before a later sample frame: each proves a replacement texture was borrowed,
then released after backend instance preparation failed, with live state intact.
The fault is deliberately after contract admission; an invalid requested clip
name would fail too early to establish that cleanup behavior.
SDK/runtime `e3bf354c9f51.audit15` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). This is
runtime debug readback, not a new GPU visual or performance measurement.

## Fifteenth pass — scene breadth quotas (#7876, continuing)

Removed the 4,096-node, 4,096-mesh, 4,096-instance, 16,384-edge and
8,192-primitive quotas from general glTF scene import and instance flattening.
Removed counters used only to enforce those quotas. Actual checked u32 node,
parent, mesh and primitive identities remain, as do cycle/shared-parent
rejection, referenced resource validity, affine transforms and nonempty geometry.

Depth is deliberately not included: source traversal is iterative, but animated
pose composition still recursively follows parents in animation/sample/pose.rs.
The existing depth guard remains pending replacement of that recursive path and
focused deep-hierarchy validation. Vertex/index stream budgets and TEXCOORD
policies likewise remain unresolved rather than being approved by this pass.
The shared-accessor breadth regression contains 17,000 nodes, 16,999 edges and
mesh instances, 4,100 meshes and 12,300 source primitives. Flattening preserves
50,997 groups/triangles, 152,991 vertices, UV alignment, source identities and
composed transforms. All 5 scene-import tests pass, including retained hierarchy
and malformed-source rejection cases. The 257-joint/129-skin animated regression
also passes. All-target Clippy passes for voxel-convert and asset-import with
warnings denied. This proves import/flattening, not a large-scene GPU draw.
SDK/runtime `e3bf354c9f51.audit16` built and replaced CraftSurvive's pair. Broker
launch/current fingerprints match. Seed 11 / disrupted / normal / hub reports
`generationError=none`, 183,158 triangles, 142,802 vertices and unchanged topology
(0 boundary edges, 9 nonmanifold edges #7870, 0 inconsistent winding). This is
runtime debug readback, not a new GPU visual or performance measurement.


## Final resource pass — source capacity and remaining ownership (#7876)

Replaced recursive animated parent composition with an iterative parent-chain
walk and removed the 256-depth quota. Source hierarchy traversal was already
iterative. The reverse-ordered 2,048-node pose chain and a 300-node GLB chain
preserve composed transforms; missing parents, cycles and non-finite transforms
remain errors.

General imported vertex/index streams now use their actual u32 count
representation instead of 2M/6M or 16M/48M policies. Checked addition still
prevents overflow. The numeric capacity test crosses the old quotas without
allocating a u32-sized mesh. The generic importer preserves 16 TEXCOORD sets
rather than rejecting at eight; this does not expand the renderer's supported
selected texture channels (0..3).

Removed the conversion source 64 MiB cap, its provenance restriction, the two
1 MiB request-envelope quotas and the inspector's 4 MiB import-manifest read
cap. Typed parsing, malformed-input errors, source identity and conversion work
contracts remain. The conversion tests exercise actual CLI import above 64 MiB
with small geometry and both CLIs above 1 MiB requests. Details and retained
conversion ownership are in [the conversion disposition](audit-7876-conversion.md).
The conversion work/output and voxel palette contracts govern requested voxel
work and its artifact representation; they do not constrain generic mesh import.
Other inspector commands' artifact policies are outside this resource operation.

Mesh JSON admission no longer reparses the entire document into a generic Value
tree to detect unsupported fields. The initial typed parse records field
presence while skipping their contents. Explicitly null unsupported fields
remain rejected, covered for all five fields.

Final copy/hash dispositions for the traced resource operation:

- Rust admitted bodies share immutable Arc storage through resources, bundles
  and responses (earlier passes). Canonical asset encoding used to produce a
  content hash remains actual identity input, not a discarded size preflight.
- Browser content preparation retains its initial snapshot of mutable caller
  bytes. Resource loaders snapshot independently supplied resolver buffers before
  asynchronous hash/retention; otherwise caller mutation can invalidate identity.
- Public Three resource providers remain independently usable. Their identity
  verification is retained; no new trust flag or registration layer is added
  solely to skip a hash on the usual path. Provider bytes remain stable until
  release, allowing the earlier redundant encoded texture copy to stay removed.
- Decoded GPU streams/pixels and audio decode buffers retain their own lifetime;
  those copies are realization or consuming-decoder ownership, not validation.
- Actual file/JSON/GLB/PNG/RMesh parsing, ABI counts, content identities and device
  texture dimensions stay with their owning codec, representation or backend.

This closes the resource operation audit and the specific outstanding decisions
from audit16. It does not certify every raw validation-inventory row, guarantee
arbitrary allocation success, or report a large-scene GPU benchmark.


## Completion checks for #7874–#7877

The final resource tests pass (14 voxel-convert library, 5 pose, 6 scene-import,
unsupported-JSON-field regression, conversion/inspector CLI and provenance
regressions). Retained publication integration passes 43 host unit, 5 world and
6 publication tests, in addition to the two focused mesh bridge tests.
Opaque payload and diagnostic evidence is recorded in
[audit-7875](audit-7875.md) and [audit-7877](audit-7877.md); the retained-frame
per-site decisions are in [audit-7874](audit-7874.md).
All-target Clippy with warnings denied passes for all 13 touched Rust packages.
The full Render build regenerates tracked bundles/declarations successfully.
The existing Vite ineffective dynamic-import warning remains build-only.
