# Heightfield

Ordinary packaged C# product. Rolling hills, ridged ranges and a lake drawn by
the Engine's heightfield service (#9842): rings of 1 m cells within 48 m, 4 m
within 400 m and 32 m within 4 km of the camera. The product samples tiles on
worker threads through `HeightfieldStreamer`; the Engine meshes them, draws
them with each ring geomorphing onto the next, and picks the drawn ground,
which the camera uses to hold its height as it flies a 1.5 km circle. The
ground draws with linear vertex colours, or with a terrain layer material of
grass and rock blended by slope; the lake is its own unblended material.
`generate-textures.py` writes `content/grass.png` and `content/rock.png`.

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
- `heightfield.pick <x> <y> <z> <dx> <dy> <dz>`: pick the drawn ground.
