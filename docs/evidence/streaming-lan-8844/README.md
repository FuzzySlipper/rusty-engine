# The streaming mode over one LAN hop (#8844)

#8786 measured the streaming browser mode on localhost only. This run adds the
LAN hop, with the same scripts and the same build measured both ways.

## Setup

- **Runtime (192.168.1.10).**
  - Doom `3ab47cf`, room study, `RUSTY_RENDER_OUTPUT=stream`.
  - A runtime pack from Engine `e9e7aa95a`.
  - Bound to the LAN with `LOADING_BAY_BIND_HOST=192.168.1.10`. JPEG q80, the
    default.
- **Viewer across the hop (192.168.1.24).**
  - Another machine on the same 1 GbE switch: ping 0.18 ms average,
    0.11–0.39 ms.
  - Its own Chromium 152, headless. ANGLE on Vulkan reports an AMD Radeon 8060S
    (RADV).
  - Run from a copy of the #8786 scripts. `latency-bandwidth.mjs` now takes
    `PLAYWRIGHT_CORE` and `CHROMIUM` so it runs outside the Engine checkout.
- **Localhost baseline.** The same runtime and scripts from the runtime's own
  machine, with the crew-services Chromium 151. Load average 2.9.
- **Inputs.** 40 trials of `l` (turn) per size. Frame probe runs are 10 s each.

## Results

| Size | Path | Frames B/s | Input → visible change, median | p90 | Probe fps | Median frame | Median / p95 gap |
|---|---|---|---|---|---|---|---|
| 1280x720 | localhost | 5.36 MB/s | 28.0 ms | 34.5 ms | 60.1 | 88.8 KB | 16.66 / 17.98 ms |
| 1280x720 | LAN hop | 5.28 MB/s | 36.2 ms | 42.1 ms | 60.1 | 89.0 KB | 16.66 / 18.11 ms |
| 1920x1080 | localhost | 10.66 MB/s | 39.4 ms | 46.9 ms | 60.0 | 175.5 KB | 16.65 / 18.74 ms |
| 1920x1080 | LAN hop | 12.56 MB/s | 30.9 ms | 37.6 ms | 60.1 | 177.1 KB | 16.65 / 18.06 ms |

The SSE outputs add 67–76 KB/s in every row (`measurements/`). No frame was
skipped and no sample was blank.

## Reading

- **Bandwidth is as the localhost numbers predicted.** The prediction was 5.3
  MB/s (42 Mbit/s) at 720p and 10.6 MB/s (85 Mbit/s) at 1080p. The hop carries
  the stream at 60 fps with the same frame gaps.
  - The browser run's 12.6 MB/s at 1080p exceeds the probe's 10.6 MB/s. The
    other machine's page drew every frame (`frames` is bytes received over
    5 s), where the loaded localhost page skipped some, as #8786 found.
- **Latency.**
  - At 720p the hop measured 8 ms more (median). One 89 KB frame is about
    0.7 ms on 1 GbE, and the request round trip adds about 0.2 ms. The rest is
    the other browser and machine, which this run cannot separate from the hop.
  - At 1080p the LAN viewer was faster, 30.9 ms against 39.4 ms: its page
    decodes on a machine that is not also running the runtime. #8786 found the
    page to be the 1080p bottleneck on localhost.
- **Raw RGBA** (1.8 and 4 Gbit/s) exceeds 1 GbE and was not measured, as the
  task says.

## Reproduce

```sh
# runtime machine
LOADING_BAY_BIND_HOST=<lan-ip> RUSTY_RENDER_OUTPUT=stream ./scripts/run-csharp-product.sh --runtime <pack>
# viewer machine, with a copy of playwright-core and the two scripts
PLAYWRIGHT_CORE=<dir>/playwright-core CHROMIUM=/usr/bin/chromium \
  node latency-bandwidth.mjs http://<lan-ip>:4394 stream-lan-720 1280 720 40
python3 frame-probe.py http://<lan-ip>:4394 1280 720 10
```
