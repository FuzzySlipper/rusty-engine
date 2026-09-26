# Task 8640: two voxel atlases

Local packaged SDK `0.1.0-task8640.1` and contributor runtime evidence, before
final immutable release publication. See [fixture](../../../fixtures/csharp-voxel-atlases).
The source baseline is `9ed75974edd56ff149af251046426ce785316f19` plus the task diff.

- [Independent browser observation](observer.json) indexes original captures,
  commands, receipts and successful session cleanup. It observed red and blue
  cubes throughout solid/opaque, decorative/opaque and decorative/blend modes.
- [NativeAOT proof](native.json) indexes the same six-command exercise using the
  packaged NativeAOT product. Both materials stayed visible and distinct after
  repeated removal/reintroduction and caught binding rejections. The parent
  inspected original CoreCLR and NativeAOT images directly.
- [Report-only warnings](warnings.json): complete browser/Engine reads, no
  lag/drops. Browser screenshot ReadPixels performance warnings are recorded.
  No baseline was supplied; this is not a clean warning-delta claim.

The independent observer first used an incompatible MCP launch tool, then
successfully used the supplied Crew CLI profile. The report preserves that tool
boundary and cleanup details. No renderer rewrite or product-side atlas packing
was used. The original published `c30c1ef18861` pair also rendered the basic
two-atlas fixture; atlas identity alone did not reproduce the reported worker
EOF. The bridge did enforce an exact-used-slot palette requirement and discard
error details; the change removes that restriction and preserves diagnostics. Current proof covers the reported material variants and
recoverable binding failures; it is not a claim to reproduce every configuration
of the originating product or to certify transparent-water sorting.
