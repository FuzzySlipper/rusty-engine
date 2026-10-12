# Water field

Ordinary packaged C# product. A river valley whose water is an Engine water
field ([water fields](../../docs/water-fields.md)): 64 × 40 cells of 2 m, a
channel 4 cells wide and 1.5 m deep down the middle, banks, a floodplain
sloping back to the channel, and a closed hollow in the floodplain that soaks
slowly. The river enters at the head (x = 0) and leaves over an open edge at
the foot. The product's water owner (`ValleyWater`) sets the edges and soak
and answers the swim box from the field's samples; the Engine draws the wet
surface with a water material. The swimmer swims wherever the field is wet
and the current carries it; out of the water it stands on the field's bed,
since the fixture has no terrain collision.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-water-field/CsharpWaterField.csproj --live-debug
```

WASD swims (forward is down the valley), Space rises, Shift sinks.

Live debug commands:

- `water.inspect`: the field's diagnostics (snapshot time and age, dropped
  time, volumes, wet and drawn cells, the worker's last advance and load), the
  wet floodplain and hollow cells, and the swimmer.
- `water.storm`: raise the river to 90 m³/s; `water.calm` returns it to
  6 m³/s, inside the channel; `water.flow <m³/s>` sets any inflow.
- `water.dam`, `water.breach`: build or remove a 4 m wall across the valley.
- `water.sample <x> <z>`: the water over a cell.
- `water.swim <x> <z>`: put the swimmer at a scene point.
- `water.camera <follow|overview>`.
- `water.expect <stage>`: assert a stage of the headless sequence.

## Headless sequence

Run headless with live debug, hold time with `engine.time.mode manual`, and
advance admitted time between stages with `engine.time.advance 2000`
repeated (each admits two seconds). Each `water.expect` fails the command if
its stage does not hold.

1. Advance 120 s, `water.expect channel`: the base flow stays in the channel
   and runs downstream.
2. `water.storm`, advance 180 s, `water.expect overbank`: the floodplain
   floods and the hollow holds more than 0.3 m.
3. `water.calm`, advance 600 s, `water.expect drained`: the floodplain drains
   back to the river; only the hollow still holds water.
4. `water.dam`, advance 180 s, `water.expect dammed`: the surface behind the
   dam stands more than 0.3 m above where it stood, and the snapshot carries
   the edit.
