# Anchored camera viewport

Engine fixture for camera views that follow product UI layout. The UI lays
out a side panel and a hero panel with CSS and anchors the hero under
`hero` (`context.viewport.anchor`). The product anchors its one camera to the
same name (`CameraView.SetViewportAnchor`), so the view draws over the hero
panel and follows it on resize with no product code running. The camera's
own viewport, a lower-left quarter, applies while no page reports the anchor
or after `viewport.proof.anchor false`.

Build with `-p:RustyEngineFixtureSdkVersion=VERSION` using the matching pair's
SDK feed, and launch with that pair's `rusty dev --project ... --live-debug`.

- `viewport.proof.surface` reads `CameraView.ReadSurface()`: the surface in
  CSS and device pixels, its pixel ratio, the UI scale and how many changes
  Update has seen.
- `viewport.proof.hero` reads `CameraView.ReadViewportAnchor("hero")`: the
  hero panel's rect normalized to the surface, and how many changes Update
  has seen. Widening the side panel in the page changes it without changing
  the surface.
- `window.rustyFixtureUi.setScale(1.5)` in the page sets the UI scale, which
  the next readout reports.
