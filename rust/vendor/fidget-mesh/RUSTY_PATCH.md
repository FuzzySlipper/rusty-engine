# Fidget mesh patch

Source: crates.io `fidget-mesh` 0.5.0, https://github.com/mkeeter/fidget/tree/v0.5.0/fidget-mesh
License: MPL-2.0; see LICENSE.txt. Original source notices are retained.

Engine #7861 modifies `octree.rs` and `qef.rs`: finest-cell QEF solutions that
escape their cell recover to the mean of their own Hermite edge intersections.
These cells cannot be collapsed using the invalid original QEF error. An octree
readout counts recovery events. Existing adaptive connectivity and evaluation
remain Fidget-owned. The public upstream API does not expose leaf placement, so
this small vendored mesh crate permits a source-level correction without copying
the whole evaluator or building a second mesher downstream.

This bounds leaf placement; it does not establish that arbitrary fields produce
self-intersection-free meshes or retain features below sampling resolution.
