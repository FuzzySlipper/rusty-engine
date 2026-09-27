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

The Engine mapping is world-aligned: floor each world coordinate divided by
cell size, using the surface support height for Y. Publication bounds select
sampling and do not set an origin. For ordinary live feet, use
`EvaluateNavigationStep` and its world-space `NextWaypoint` instead of guessing
support levels. See `docs/csharp-sdk.md#collision-navigation-coordinates`.
