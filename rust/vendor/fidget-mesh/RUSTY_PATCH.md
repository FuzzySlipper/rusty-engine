# Fidget mesh patch

Source: crates.io `fidget-mesh` 0.5.0, https://github.com/mkeeter/fidget/tree/v0.5.0/fidget-mesh
License: MPL-2.0; see LICENSE.txt. Original source notices are retained.

Engine #7861 modifies `octree.rs` and `qef.rs`: finest-cell QEF solutions that
escape their cell recover to the mean of their own Hermite edge intersections.
These cells cannot be collapsed using the invalid original QEF error. An octree
readout counts recovery events. Evaluation remains Fidget-owned. The public upstream API does not expose leaf placement, so
this small vendored mesh crate permits a source-level correction without copying
the whole evaluator or building a second mesher downstream.

This bounds leaf placement; it does not establish that arbitrary fields produce
self-intersection-free meshes or retain features below sampling resolution.

Engine #7879 additionally modifies `build.rs`, `builder.rs`, `dc.rs`, `types.rs`,
`octree.rs`, and `lib.rs`: cell vertices follow boundary contour loops; ambiguous
shared faces retain distinct Hermite contour arc vertices; collapse preserves
those faces. `Mesh::face_arc_vertices` identifies fans that downstream polygon
conversion must retain. The Engine adapter in `svc-implicit` honors that metadata.
This repairs the displaced-field non-manifold edges without changing requested
sampling depth or duplicating coincident vertex indices.
