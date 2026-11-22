# Surface crawlers

A surface crawler is a small mover whose up direction follows the surface
under it: it runs on floors, up walls and across ceilings, carries over the
edges between them, and lets go to fall when the product asks or when it loses
its surface. `Spatial.ProposeCrawlerStep` moves one against a Spatial
session's voxel and static-mesh collision; the product holds its state between
steps, as it does a character's. A [surface grid](#navigation) routes one
across floors, walls and ceilings. The upright
[character controller](csharp-lifecycle.md#character-steps) is unchanged
and separate.

```csharp
CrawlerConfig config = engine.Spatial.DefaultCrawlerConfig() with { Radius = 0.25f };
CrawlerMotion motion = default;           // not yet on a surface: the first step holds one within reach or falls onto it
Vector3 position = spawn;                 // the ball's centre

CrawlerStepReceipt step = engine.Spatial.ProposeCrawlerStep(new(session, position, motion, config,
    new CrawlerCommand(Direction: towardTarget, Cling: true, StepSeconds: (float)update.Facts.FixedDeltaSeconds, Sequence: tick)));
position = step.Position;
motion = step.Motion;
Transform drawn = new(step.Position, step.Rotation, bodyScale);   // the body's appearance fact
```

## The body is a ball

The crawler's whole collision body is a ball of `Radius`, resting
`ContactSkin` off the surface it holds. A ball is the same from every side,
so the collision shape never turns; up is a fact of where the ball touches.
The drawn body is presentation: `Rotation` turns +Y to `Motion.Up` and +Z to
`Motion.Forward`, so a model authored facing +Z with its back up is drawn
lying on the surface and heading where it moves. A drawn body longer than the
ball (a 1 m lizard on a 0.25 m ball) can dip into the surface round a tight
concave corner, where the ball fits and the body does not; that is
presentation to pose or smooth, not collision.

## Movement

`Direction` is a world direction. It is laid onto the plane of the surface the
crawler is on, so the same direction carries it across edges: toward a point
up a wall, it drives a crawler on the floor into the wall and up it. Longer
than one is one; zero brakes at `Braking`. The crawler accelerates at
`Acceleration` up to `Speed`. A command is solved as equal sub-steps no
longer than 1/60 s, each moving at most half the radius (`SubSteps` says how
many).

- **Concave edges.** Moving into a surface it may hold (floor into wall, wall
  into ceiling), the crawler takes that surface's normal as up and turns the
  rest of its move, its velocity and its heading with it.
- **Convex edges.** Past a lip nothing lies under the ball; the nearest
  surface within `ContactSkin + StickDistance` is the edge itself, so up turns
  about the edge and the ball rounds it onto the face beyond in a few
  sub-steps. A surface farther than that is lost.
- **Letting go.** `Cling` holds walls and ceilings. Without it the crawler
  holds only floors: on a wall or ceiling it lets go and falls under
  `Gravity`, no faster than `TerminalFallSpeed`, and holds the first floor it
  lands on. A falling crawler with `Cling` holds whatever it lands on, a wall
  included.
- **Lost.** With nothing within reach (its wall cut away by an edit, an edge
  sharper than it can round) it falls.

A step starting inside collision, as after an edit fills the crawler's space,
pushes the ball out along the nearest surface's normal; one too deep to push
out of is refused as `unresolved-crawler-penetration`. An invalid
configuration is `invalid-crawler-config`, a bad command (a step outside
1 ms–1 s, a non-finite direction) `invalid-crawler-command`, and a non-finite
motion `invalid-crawler-motion`. A crawler meets the session's retained voxel
and static-mesh collision, on cube and reconstructed surfaces alike; product
obstacle boxes and moving platforms are not part of its step.

| Option | Meaning | Default |
|---|---|---|
| `Radius` | The ball, in metres. | 0.25 |
| `ContactSkin` | How far off a surface it rests; less than the radius. | 0.02 |
| `Speed`, `Acceleration`, `Braking` | Along a surface, m/s and m/s². | 4, 30, 40 |
| `Gravity`, `TerminalFallSpeed` | While falling. | 20, 55 |
| `StickDistance` | How far beyond its skin a surface may fall away and still be held: what rounds a convex edge. | 0.1 |
| `FloorMaximumSlopeRadians` | A surface this near level is a floor; this near upside down, a ceiling; between, a wall. | 50° |

## What a step reports

- `Surface`: `Floor`, `Wall` or `Ceiling` while attached, by
  `FloorMaximumSlopeRadians`; `None` while falling.
- `Contact`: the surface held, with its `Point`, `Normal` (also `Motion.Up`),
  the ball centre's `Distance` from it, and its source: a voxel collision
  chunk (`SourceChunkX/Y/Z`) or a static-mesh instance (`SourceInstance`,
  `SourceAsset`, `SourceGeometryHash`). Absent while falling.
- `Transition`: the last thing that happened to its hold during the step:
  `Concave`, `Convex`, `Detached` (let go without `Cling`), `Landed` or
  `Lost`; `None` when it kept to one surface.
- `Motion`: `Attached`, `Up`, `Forward`, `Velocity` and `LastSequence`, to pass
  to the next step. A default motion reads as upright, facing +Z, not
  attached.

## Navigation

`Spatial.ReplaceSurfaceNavigation` derives a surface grid for one crawler from
the session's voxel and static-mesh collision in a world box, and
`RequestSurfaceNavigationPath` routes across it:

```csharp
SurfaceNavigationConfig grid = engine.Spatial.DefaultSurfaceNavigationConfig() with { GridId = 4, Crawler = config };
engine.Spatial.ReplaceSurfaceNavigation(new(session, boxMin, boxMax, grid, Deferred: true));
// A later call, once ReadSurfaceNavigation(new(session, 4)) is no longer Pending:
SurfaceNavigationPathResult path = engine.Spatial.RequestSurfaceNavigationPath(new(session, 4, position, target, MaxVisited: 10_000));
```

A grid is cells of `CellSize`, each holding up to six places, one per facing
(`SurfaceFace`: `PositiveY` a floor, `NegativeY` a ceiling, the other four
walls). A surface belongs to the cell half a cell in front of it. It is a
place where the ball fits resting its skin off it, its centre outside
collision; a surface tilted more than 45° from a facing is the next facing's.
Places join:

- **along** one facing to the next cell, or one cell up or down for a slope;
- **concave**, to another facing in the same cell, as floor into wall;
- **convex**, over a lip to the facing beyond it one cell down, the ball
  swept round the edge on an arc.

The ball is swept along each edge. A hit facing more against the move than
along the surfaces blocks it; anything else is the surface itself, which the
crawler follows. Edges go both ways, and a path pays `FloorCost`, `WallCost`
or `CeilingCost` to enter each place after its start, so it keeps to floors
where it can. The start and goal snap to the place whose resting ball is
nearest, within `SnapDistance`.

The path's `Waypoints` run start to goal. Each gives its `Cell`, the resting
ball's `Position`, the surface's `Normal`, its `Surface`, and the `Edge` it is
entered by. Steer toward each in turn with `ProposeCrawlerStep`. A waypoint
round a concave corner can lie straight along the normal of the surface the
crawler is on, as the first one up a wall does from the floor, where the
direction laid onto the floor is nothing. When a waypoint's normal differs
from `Motion.Up` and faces back toward the crawler, lean into it:

```csharp
Vector3 direction = Vector3.Normalize(waypoint.Position - position);
if (Vector3.Dot(waypoint.Normal, motion.Up) < 0.9f && Vector3.Dot(waypoint.Normal, position - waypoint.Position) >= 0)
    direction -= waypoint.Normal;
```

`Outcome` is `Reached`, `NoPath` (no edges join them; `Nearest` is the place
nearest the goal that can be reached), `BudgetExhausted` (`MaxVisited` places
visited first; `Nearest` is the visited place estimated nearest),
`StartOffSurface`, `GoalOffSurface`, or `Unavailable` with no such grid.
`TotalCost` is what the path paid.

| Option | Meaning | Default |
|---|---|---|
| `GridId` | Not 0, and not a planar grid's: the product names it. | 0 |
| `CellSize` | At least the ball's diameter plus its skin on both sides. | 1 m |
| `MaximumCells` | The most cells the box may cover. | 1,048,576 |
| `Crawler` | The crawler's `Radius`, `ContactSkin` and `FloorMaximumSlopeRadians`. | `DefaultCrawlerConfig()` |
| `FloorCost`, `WallCost`, `CeilingCost` | What entering a place costs, at least 1. | 1, 2, 3 |
| `SnapDistance` | How far a start or goal may be from a resting ball. | 1 m |

A surface grid stands beside the planar grids under its own `GridId`.
`ReadNavigationGrids` lists it as `CollisionSurface` with its places as
`WalkableCellCount`, and `ClearNavigation` clears it by its id or with every
grid. Planar publications and requests that name it are refused
(`navigation-grid-is-surface`), as a surface publication naming a planar grid
is (`surface-navigation-grid-in-use`), and publishing planar navigation of any
kind leaves it. Every publication derives the whole box, in the call on every
core, or with `Deferred` on Engine workers as a
[deferred collision-derived grid](csharp-lifecycle.md#deriving-a-grid-off-the-update)
does: until it installs the grid's previous publication answers, and
`ReadSurfaceNavigation` says `Pending`, `Refused` or the `Installed` receipt.
A collision edit does not change a grid; publish it again. A world-origin
rebase moves a grid by whole cells, and drops it otherwise. A bad
configuration is `invalid-surface-navigation-config`, a box that is not finite
`invalid-surface-navigation-region`, and one over `MaximumCells`
`surface-navigation-over-budget`.

`ExplainSurfaceNavigationCell` samples one place again against current
collision: `Surface` with the contact and resting position, `NoSurface`,
`OtherFacing`, `BallOverlap` with what the ball overlaps, `InsideCollision`,
or `OutsideGrid`; `Admitted` says whether the installed grid holds it.
`ExplainSurfaceNavigationEdge` sweeps one edge again: its `Kind`, `Outcome`
(`Traversable`, `FromNotSurface`, `ToNotSurface`, `NotNeighbor` or
`Blocked`), the `Corner` a concave or convex edge turns, and for `Blocked`
where the ball stopped (`BlockedAt`) and the `Blocker` it met. A shelf beside
a lip, for example, blocks the convex edge over it, and the explanation names
the lip and the shelf.

`fixtures/csharp-surface-crawler` routes a crawler across a room that a
bottomless trench cuts in two below its ceiling: up a wall, across the ceiling
and down the far side. Its live-debug commands are `crawler.route`,
`crawler.follow wall|ceiling|trench|end`, `crawler.drop`, `crawler.inspect`
and `crawler.camera`.

## Cost

A crawler step is a few swept-ball and nearest-surface queries per sub-step.
Going round a room at 60 Hz it measured about 6 µs per step on cube collision
and 18 µs on dual-contoured surfaces (den-agents, shared and not quiet), with
`cargo test --release -p engine-spatial --test crawler -- --ignored
measure_crawler_step_cost --nocapture`.

A surface grid derives each line of cells along each axis with one ray per
surface it meets, then sweeps the ball once for each neighbouring pair. Over a
CraftSurvive creature's box (64 × 32 × 64 one-metre cells of ruins on rolling
ground) it measured:

| Collision | Places | Edges | In the call | On a worker |
|---|---|---|---|---|
| Cubes | 24,329 | 112,014 | 39 ms | 85 ms |
| Dual-contoured | 22,078 | 95,966 | 161 ms | 411 ms |

These are medians of five on den-agents (20 cores, load about 2, not quiet), with
`cargo test --release -p csharp-engine-services measure_surface_derivation_cost
-- --ignored --nocapture`. A worker takes one thread for every four cores.
