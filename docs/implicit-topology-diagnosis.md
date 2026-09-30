# Implicit topology on ambiguous faces

The adaptive implicit mesher keeps index topology manifold where a cell face is
ambiguous (a checkerboard face, with filled corners diagonally opposite). The
mesh builder is the vendored
[`fidget-mesh`](../rust/vendor/fidget-mesh/RUSTY_PATCH.md), patched for Engine
#7879; `svc-implicit` converts its output to Engine polygons.

- Cell vertices follow each cell's boundary contour loops. Crossings on a
  checkerboard face pair up around the filled corners the same way on both
  sides of the face.
- Each ambiguous face arc gets a shared vertex at the mean of its two Hermite
  crossings. Two distinct arcs between the same cell vertices therefore never
  become one four-incident edge.
- Octree collapse keeps any parent or child cell with an ambiguous face, so
  both sides keep the shared face. Sampling depth is unchanged; collapse still
  runs elsewhere.
- The mesh's `face_arc_vertices` mark the fans around those arcs.
  `svc-implicit`'s polygon conversion (`src/triangulate.rs`) keeps them as local
  fans instead of adding a diagonal that would join the arcs again.
- Ordinary faces keep the plain dual-contouring representation.

## Tests

[`displaced_topology_repro.rs`](../rust/crates/svc-implicit/tests/displaced_topology_repro.rs)
builds a strongly displaced closed field: three joined thin capsules, five
`Disrupt` bands, a protected core and a closed enclosing solid. For seeds 10,
11, 29 and 47 it requires zero boundary, non-manifold and inconsistent-winding
edges, and connected cyclic vertex links. A second test meshes a raw Fidget
field with checkerboard faces across three axis permutations and two depths,
and checks directed edge pairing and vertex links before polygon conversion.
The box, cap, cavity and carved-volume tests cover the surrounding behaviour.

These are index-topology checks. They do not show that arbitrary fields are
free of self-intersections, or that details below the sampling resolution
survive. The uniform sampled-volume mesher (`svc_mesh::mesh_scalar_samples`)
does not use this path.
