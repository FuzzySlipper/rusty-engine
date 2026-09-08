# Retained mesh publication audit

Task #7874. The public RenderFrameDiff and projector resource catalogue remain
mutable; this audit preserves validation at their actual admission points.

| Site | Disposition |
| --- | --- |
| C# create_mesh_resource | Retain ABI span/count coherence, stream and definition validation, live material references and atomic state update. Removed the arbitrary 256 group/binding quota; slots remain u16, geometry counts u32. |
| Appearance projector catalogue | Retain admission validation: resources_mut exposes mutable definitions, so a previous service call is not an immutable guarantee for this general Rust API. |
| Projector frame construction | Retain validation of newly synthesized operations, handles and public source facts. No trusted mode is added to a public mutable frame constructor. |
| PresentationWorld::apply input | Retain frame admission plus retained-reference/lifetime rules and candidate-state transaction. |
| PresentationWorld::apply output | Remove the second full operation scan. Output contains only unchanged copies of admitted operations; checked revision/count metadata is constructed locally before commit. |
| RuntimePublication frame/presentation constructors | Validate once and detach into privately held, read-only snapshots. Consuming a snapshot yields an ordinary mutable frame that must be admitted again before publication. |
| RuntimePublication::validate, receipt, host adaptation | Admitted snapshots no longer rescan frame or captured mesh data. Mutable enum metadata and other contracts remain checked. Host adaptation consumes the snapshot without another body copy. |
| Decoded worker/browser frames | Retain actual wire admission; the immutable in-process snapshot guarantee does not cross serialization. |

The browser mesh validator uses u16 material slots, requires group coverage and
bound materials, and has no 256 group/binding count limit. Three builds groups
and material arrays from those descriptors; this change does not promise that
many separate material draws are cheap.

Evidence includes a real C# bridge request with 300 groups and 300 distinct
bindings, missing-binding rejection without partial resource retention, and the
existing copied-stream/release test. A retained-world regression checks the
emitted mesh delta and metadata, then mutates the input index stream out of range
and proves the world remains unchanged. Publication tests demonstrate detached
snapshots, rejection of mutated source frames, and fresh admission after consuming
a snapshot. Existing host ordering, wire conversion and presentation tests remain
applicable. No renderer protocol, lifetime/revision semantics or product policy
was introduced.
