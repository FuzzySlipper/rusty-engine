# #8868: `rusty dev --headless` stays, without its WebGL flags

## Decision

An unattended page still has a job, but not the one first given for it.

**Correction.** The first version of this README said the product starts
when the first page attaches. That holds only for a direct, unsupervised
`rusty-product-host` launch. Under `rusty dev`, the supervised runtime starts
the product at load (`main.rs`: "A supervised product runs from load, with or
without a browser"). In the #8765 exercise, a fresh `rusty dev` of Dagger with
no page open was at step 773 about 13 s after launch.

**What the page does do.** It is the stream's viewer.
- **Frames.** `render-stream` draws only while a viewer asks for frames, or
  when an inspection command requests one. With no page, Dagger simulated for
  100 s (step 6290) and `engine.renderer.presentation` still had no drawn
  frame.
- **Completions.** The Engine learns of animation and video completions and
  ghost plate readouts from drawn frames. So an unwatched product that waits
  on one never gets it: Dagger's opening cinematic cannot finish.
- **UI.** The page also mounts the product UI.

`--headless` stays for that.

**Flags.** Since #8792 the page draws a 2D canvas, so the WebGL-on-SwiftShader
flags (`--use-gl=angle`, `--use-angle=swiftshader`,
`--enable-unsafe-swiftshader`, `--enable-webgl`, `--ignore-gpu-blocklist`,
`--disable-gpu-sandbox`) are replaced by `--disable-gpu`. The help texts and
`docs/csharp-sdk.md` say what the flag is for.

Whether completions should depend on a viewer at all is filed separately; see
Follow-up below.

## Run

Doom ran with `--engine-source` pointed at this tree, whose runtime pack was
rebuilt with `scripts/build-runtime-pack.sh`, and with `--headless`.
`RUSTY_CHROMIUM_PATH` selected the crew-services Chromium.

- **Launch.** The host logged `RUSTY_HEADLESS_BROWSER started`, and the
  process ran with `--headless=new --no-sandbox --disable-dev-shm-usage
  --disable-gpu …`.
- **Simulation.** `engine.time` answered `realtime`, with the step at 1297 and
  then, 3 s later, at 1477 (60 Hz).
- **Frames.** `engine.renderer` reported `output: stream` at 60.0 frames per
  second, for the headless page's 1280×577 viewport.
- **No page errors.** The diagnostics read held two browser-host events, both
  info-level `BROWSER_HOST_STATUS` snapshots.
- **Clean stop.** Stopping `rusty dev` closed Chromium with the host and
  removed its temporary profile.

`rusty dev --help`: [rusty-dev-help.txt](rusty-dev-help.txt).

## Follow-up

- #8871: an unwatched runtime draws nothing, so animation and video
  completions stall until a viewer attaches. It decides whether completions
  should need a viewer at all.
