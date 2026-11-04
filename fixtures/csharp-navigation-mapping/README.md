# Collision navigation mapping

The packaged consumer invokes `NavigationMappingChecks.Run` through generated
C# services on the real CoreCLR host:

```sh
scripts/test-csharp-sdk-package.sh --coreclr-smoke
```

Two three-cell floors use positive/negative world coordinates, positive/negative
support heights and 0.5-unit cells. Each publication box is deliberately
unaligned with cell and chunk boundaries. The fixture derives cells directly
from world support positions, queries every reported cell, checks a three-cell
route and its returned middle cell, and exercises world-position steering with
standing clearance. Box-relative coordinates reproduce `StartNotWalkable`.
A static-mesh floor whose columns differ by float noise must route both ways
as level floor.

A static-mesh wall with gaps of 0.8, 1.4 and 2.5 m carries three grids for
agents of radius 0.3, 0.6 and 1 on 1 m cells. Each routes through only the gaps
it fits, and a grid replaced or cleared leaves the others answering.

The Engine mapping is world-aligned: floor each world coordinate divided by
cell size, using the surface support height for Y. Publication bounds select
sampling and do not set an origin. For ordinary live feet, use
`EvaluateNavigationStep` and its world-space `NextWaypoint` instead of guessing
support levels. See `docs/csharp-lifecycle.md#collision-navigation-coordinates`.
