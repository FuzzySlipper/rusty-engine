# Implicit topology diagnosis

Engine #7879 repairs the ambiguous shared-face connectivity exposed by the
strongly displaced closed-field stress case. The compact deterministic repro is
[`displaced_topology_repro.rs`](../rust/crates/svc-implicit/tests/displaced_topology_repro.rs):
seed 10, three joined thin capsules, CraftSurvive's five `Disrupt` bands, a
protected core, and a closed enclosing solid. Before the repair it had zero
boundary edges, two non-manifold edges, and zero inconsistent-winding edges.

The two non-manifold edges already existed in Fidget's raw `Octree::walk_dual()`
mesh, before `svc-implicit` converts edge-intersection fans into QEF-boundary
polygons. The raw indices are `1991/1992` and `2066/2068`; disabling octree
collapse retains two four-incident QEF-QEF edges (`9552/9553` and
`9629/9631`) with 5,700 final vertices. The failure therefore is neither the
Engine's diagonal selection nor adaptive collapse. It is an ambiguous leaf
connectivity case.

Grouping transitions by connected filled-corner start region exposed the
ambiguous construction. Replacing that grouping with a `(start_region,
end_region)` key traded the two non-manifold edges for eight raw boundary
edges, so it is not a repair.

The repair traces each cell's boundary contour loops, pairing checkerboard-face
crossings around filled corners consistently on both sides. Each ambiguous face
arc gets a shared vertex at the mean of its two Hermite crossings. This keeps two
distinct arcs between the same cell QEF vertices from becoming one four-incident
mesh edge. Ordinary faces retain the existing dual representation.

Collapse rejects checkerboard parent or child faces so that both sides retain
the shared face identity. Sampling depth is unchanged; collapse remains enabled
elsewhere. Engine polygon conversion preserves the affected fans instead of
introducing a diagonal that would identify the arcs again.

The compact fixture now checks seeds 10, 11, 29, and 47 for zero boundary,
non-manifold, and inconsistent-winding edges and connected cyclic vertex links.
A separate raw Fidget stress field exercises checkerboard faces across three
axis permutations and two depths, checking directed edge pairing and vertex
links before Engine polygon conversion. Existing box, cap, cavity, displaced,
and carved-volume regressions cover the surrounding behavior.

These are index-topology regressions, not a guarantee that arbitrary fields are
self-intersection-free or that details below sampling resolution survive. The
separate uniform sampled-volume mesher is unchanged by this repair.
