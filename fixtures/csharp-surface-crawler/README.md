# Surface crawler

Ordinary packaged C# product (#10967). A room of one-metre voxels, open at the
front for the camera, is cut in two by a trench two metres wide that runs
through its floor, back wall and sides up to the ceiling and has no bottom, so
the halves join only over the ceiling. `Spatial.ReplaceSurfaceNavigation`
derives a surface grid for a crawler with a 0.25 m ball, and
`RequestSurfaceNavigationPath` routes it from the left half's floor to the
right's: up a wall, across the ceiling over the trench and down the far wall.
The product steers the crawler at each waypoint with
`Spatial.ProposeCrawlerStep` and draws a body longer than the ball, with a
head, turned by the step's rotation; red markers show the waypoints.

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-surface-crawler/CsharpSurfaceCrawler.csproj --live-debug
```

Live-debug commands:

- `crawler.route`: derives the grid and routes from where the crawler is to
  the far floor; reports the outcome, the surfaces in order, the cost and the
  grid's places, edges and components.
- `crawler.follow wall|ceiling|trench|end`: follows the route until the
  crawler is on a wall, on the ceiling, on the ceiling over the trench, or at
  the goal.
- `crawler.drop`: stops the crawler and lets go; it falls straight and holds
  the first floor it lands on. From the ceiling over a floor, `crawler.route`
  then routes again from where it landed.
- `crawler.inspect`: position, surface, last transition, up, waypoint, the
  surfaces crossed and the transitions so far.
- `crawler.camera <x> <y> <z> <targetX> <targetY> <targetZ>`.
