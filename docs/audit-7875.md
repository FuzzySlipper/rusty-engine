# Audit 7875: opaque UI, input, and timeline data

UI projections and direct input payloads remain plain JSON data. Their
identities, lifecycle fences, contracts, sequence rules, finite/safe numeric
representation, and private immutable snapshots remain enforced.

The former UI and direct-input byte, node, depth, string, array, and object
quotas were local admission policy rather than a transport capability. The UI
host now detaches and freezes JSON iteratively without an inherited size cap.
The input host does the same before queuing a claim. Rust retains one canonical
input JSON encoding because the C# ABI copies those exact bytes for the product
update; it does not impose an additional payload quota.

Timeline opaque data is a private immutable wrapper. Its former construction
and snapshot-restoration encodes only measured discarded bytes, while the
actual C# completion callback encodes the data immediately before the ABI copy.
The wrapper therefore no longer reconstructs or revalidates its owned value.

ProductDev HTTP framing remains independently bounded at 512 KiB. That request
framing policy is separate from the removed opaque-data admission quotas.


Focused verification passed: runtime-ui 9, runtime-input 13, runtime-timeline
20, application-host 37, product-browser-host 87, product-dev-host 68 and
csharp-product-runtime 57 tests. Browser tests exercise depth 1,024, many nodes,
and payloads above the retired byte quotas. The full Render build regenerated
tracked artifacts and checked declarations. This is admission/transport evidence,
not a claim that arbitrary nesting survives every JSON codec or allocation.
