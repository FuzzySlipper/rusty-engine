# Implicit topology diagnosis

The strongly displaced closed-field stress case has a known dual-contouring
topology limitation. The compact deterministic repro is
[`displaced_topology_repro.rs`](../rust/crates/svc-implicit/tests/displaced_topology_repro.rs):
seed 10, three joined thin capsules, CraftSurvive's five `Disrupt` bands, a
protected core, and a closed enclosing solid. Its extracted topology is zero
boundary edges, two non-manifold edges, and zero inconsistent-winding edges.

The two non-manifold edges already exist in Fidget's raw `Octree::walk_dual()`
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

This stress case supports no clean or generally manifold meshing claim. A
repair must establish consistent shared-face connectivity and verify manifold
vertex links. Duplicating indices, concealing the readout, or forcing a new
sampling resolution would only hide or move the failure.
