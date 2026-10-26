# Heightfield

Ordinary packaged C# product. Rolling hills, ridged ranges and a lake drawn by
the Engine's heightfield service (#9842): rings of 1 m cells within 48 m, 4 m
within 400 m and 32 m within 4 km of the camera. The product samples tiles on
worker threads through `HeightfieldStreamer`; the Engine meshes them, draws
them with each ring geomorphing onto the next, and picks the drawn ground,
which the camera uses to hold its height as it flies a 1.5 km circle. The
ground draws with linear vertex colours, or with a terrain layer material of
grass and rock blended by slope; the lake is its own unblended material.
Grass and trees grow over the hills in two vegetation fields (#10016): a near
field on the finest ring's 1 m lattice and a far one on the middle ring's 4 m
lattice, so copies stand on the drawn triangles, with the grass handed from one
to the other one for one between 40 and 44 m; trees are placement records.
The far grass thins with distance, its kept clumps growing so the hills stay as
green, and trees draw a cheaper cone from 150 m and baked impostor cards from
320 m, crossfading between levels (#10017).
`generate-textures.py` writes `content/grass.png`, `content/rock.png`, the
decal atlas `content/marks.png`, and the coal atlas `content/coals.png` with
the light it emits, `content/coals-emission.png`.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-heightfield/CsharpHeightfield.csproj --live-debug
```

Live debug commands:

- `heightfield.inspect`: the rings' focus, tiles held, drawn and waiting,
  vertices, sample and mesh memory, the last meshing, and the streamer's
  commits and sampling time.
- `heightfield.fly <0|1>`: hold the camera or fly on.
- `heightfield.angle <radians>`: jump along the circle.
- `heightfield.layers <0|1>`: vertex colours, or the two-layer material.
- `heightfield.invalidate <metres>`: sample again around the camera.
- `vegetation.inspect`: both vegetation fields' tiles, patches, copies, budget
  and deferred work, the last call's placement, selection and publication
  time, invalidations and sampling.
- `vegetation.show <0|1>`, `vegetation.tune <grass per m²> <reach>`,
  `vegetation.invalidate <metres>`.
- `heightfield.pick <x> <y> <z> <dx> <dy> <dz>`: pick the drawn ground.
- `heightfield.height <metres>`: fly this high over the hills.
- `decals.scatter <count> <size> <0|1>`: scatter decals from the
  `marks.png` atlas (a footprint, a scorch, a splash and an ash ring;
  `generate-textures.py` writes it) over the ground where the camera looks,
  without or with the angle falloff, and report `CameraView.ReadDecals`.
- `decals.coals <count> <size> <seconds>`: night (a low exposure, bloom, a
  dark sky) and that many patches of char with glowing seams and
  single-texel coals (`coals.png` and its emission atlas, 32 texels a metre
  at size 1) over the ground where the camera looks, each placed again by
  its id every update as it cools over `seconds` and kindles again (0 lays
  them cold, emitting nothing); a count of 0 removes them and brings the day
  back.
- `weather.wetness <wetness> <puddles> <frozen>`: the hills wet after rain,
  puddles gathering, frozen from their edges in (1: ice), as
  `CameraView.ReadWetness` reports it.
- `weather.snow <amount> <texels per metre>`: snow over the hills, grass and
  trees, clear under the trees (an ambient light's sky layer follows the
  camera), and the snow in force as `CameraView.ReadSnow` reports it. The
  grass's material sinks under snow (`SinksUnderSnow`): clumps in the open
  shorten and pale, those under the trees stand.
