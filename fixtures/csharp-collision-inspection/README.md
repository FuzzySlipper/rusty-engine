# Collision inspection

One collider of each kind in one Spatial session, for the collision overlay
and readout ([playtest inspection](../../docs/playtest-inspection.md#collision)):

- a 12 × 12 voxel floor under y = 0;
- `content/platform.json`, a spatial content artifact: a 2 m platform at
  y = 1 with a steep wall under its +x edge. The product draws its box a metre
  higher, so its collision shows under the drawn platform;
- a product static-mesh ramp (instance 51), walkable then steep, that is
  never drawn;
- a character stepped every update beside a product obstacle box (entity 3);
- a trigger zone (entity 5);
- a dynamics box (body 1) dropped onto the floor.

Run it with the same pair's SDK and runtime, then:

```sh
rusty-live-debug --command "engine.inspect.overlay show collision"
rusty-live-debug --command "engine.inspect.collision"
```

The wires draw in the player view and from an `engine.renderer.camera`
observer; the readout lists the same colliders with their sources.
