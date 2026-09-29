# #8868: `rusty dev --headless` stays, without its WebGL flags

## Decision

An unattended page still has a job. The runtime starts when the first page
attaches: `connect` issues `Start` while the lifecycle is `Created`
(`csharp-product-runtime/src/lib.rs`). With no page, the product never starts.
`--headless` is how a run starts and keeps going with nobody watching.

Removing it would move that start into the host, which changes lifecycle
behavior for every product. That goes beyond this task, so `--headless`
stays.

What changed is the Chromium the flag launches.
- **Before.** It asked for WebGL on SwiftShader (`--use-gl=angle`,
  `--use-angle=swiftshader`, `--enable-unsafe-swiftshader`, `--enable-webgl`,
  `--ignore-gpu-blocklist`, `--disable-gpu-sandbox`) so the Three renderer
  could draw with no display.
- **After.** It passes `--disable-gpu`. Since #8792 the page draws the
  runtime's frames on a 2D canvas and hosts the product UI.

The help texts and `docs/csharp-sdk.md` now say what the flag is for.

## Run

Doom ran with `--engine-source` pointed at this tree, whose runtime pack was
rebuilt with `scripts/build-runtime-pack.sh`, and with `--headless`.
`RUSTY_CHROMIUM_PATH` selected the crew-services Chromium.

- **Launch.** The host logged `RUSTY_HEADLESS_BROWSER started`, and the
  process ran with `--headless=new --no-sandbox --disable-dev-shm-usage
  --disable-gpu …`.
- **The product started.** No other page was open, yet `engine.time` answered
  `realtime`, with the simulation step at 1297 and then, 3 s later, at 1477
  (60 Hz).
- **Frames.** `engine.renderer` reported `output: stream` at 60.0 frames per
  second, for the headless page's 1280×577 viewport.
- **No page errors.** The diagnostics read held two browser-host events, both
  info-level `BROWSER_HOST_STATUS` snapshots.
- **Clean stop.** Stopping `rusty dev` closed Chromium with the host and
  removed its temporary profile.

`rusty dev --help`: [rusty-dev-help.txt](rusty-dev-help.txt).
