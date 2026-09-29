#!/usr/bin/env bash
# Doom legacy-voxel in window mode on X11 at a forced winit scale factor;
# screenshot the display after it settles, then stop.
# usage: dpr_run.sh <runtime-pack> <scale> <out.png>
set -u
PACK="$1" SCALE="$2" OUT="$3"
D=/tmp/claude-1000/-home-agent-dev-rusty-engine/6151265a-602d-409f-b175-43208550fc54/scratchpad/desk
cd /home/agent/dev/rusty-doom
env -u WAYLAND_DISPLAY DISPLAY=:1 WINIT_X11_SCALE_FACTOR="$SCALE" DOTNET_ROOT=/home/agent/.dotnet TMPDIR=/home/agent/tmp \
  RUSTY_RENDER_OUTPUT=window LOADING_BAY_LIVE_DEBUG=1 LOADING_BAY_SCENE=legacy-voxel \
  ./scripts/run-csharp-product.sh --runtime "$PACK" > "$OUT.log" 2>&1 &
for _ in $(seq 1 90); do grep -q 'listening at' "$OUT.log" && break; sleep 2; done
sleep 20
(. "$D/state2/env.sh"; timeout 30 spectacle -b -n -f -o "$OUT" >/dev/null 2>&1)
pkill -INT -f 'rusty dev --project /home/agent/dev/rusty-doom/csharp'
for _ in $(seq 1 20); do ss -ltn | grep -q ':4394 ' || break; sleep 1; done
