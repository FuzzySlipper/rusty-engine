# Surface crawlers

A surface crawler is a small mover whose up direction follows the surface
under it: it runs on floors, up walls and across ceilings, carries over the
edges between them, and lets go to fall when the product asks or when it loses
its surface. `Spatial.ProposeCrawlerStep` moves one against a Spatial
session's voxel and static-mesh collision; the product holds its state between
steps, as it does a character's. The upright
[character controller](csharp-lifecycle.md#character-steps) is unchanged
and separate.

```csharp
CrawlerConfig config = engine.Spatial.DefaultCrawlerConfig() with { Radius = 0.25f };
CrawlerMotion motion = default;           // not yet on a surface: the first step holds one within reach or falls onto it
Vector3 position = spawn;                 // the ball's centre

CrawlerStepReceipt step = engine.Spatial.ProposeCrawlerStep(new(session, position, motion, config,
    new CrawlerCommand(Direction: towardTarget, Cling: true, StepSeconds: update.StepSeconds, Sequence: tick)));
position = step.Position;
motion = step.Motion;
body.Transform = new(step.Position, step.Rotation, Vector3.One);
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

## Cost

A crawler step is a few swept-ball and nearest-surface queries per sub-step.
Going round a room at 60 Hz it measured about 6 µs per step on cube collision
and 18 µs on dual-contoured surfaces (den-agents, shared and not quiet), with
`cargo test --release -p engine-spatial --test crawler -- --ignored
measure_crawler_step_cost --nocapture`.
