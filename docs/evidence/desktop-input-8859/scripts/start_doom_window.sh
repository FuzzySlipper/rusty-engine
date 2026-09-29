#!/usr/bin/env bash
# Start Doom in window mode on the input test compositor and wait for it.
# usage: start_doom_window.sh <runtime-pack> <log> [x11]
set -u
PACK="$1" LOG="$2" MODE="${3:-wayland}"
D=${WORK:?set WORK to the working directory}/desk
. "$D/state3/env.sh"
if [[ "$MODE" == x11 ]]; then unset WAYLAND_DISPLAY; export DISPLAY="${X11_DISPLAY:-:1}"; fi
cd /home/agent/dev/rusty-doom
env RUST_BACKTRACE=1 DOTNET_ROOT=/home/agent/.dotnet TMPDIR=/home/agent/tmp RUSTY_RENDER_OUTPUT=window \
  LOADING_BAY_LIVE_DEBUG=1 RUSTY_CEF_SWITCHES=remote-debugging-port=9333 \
  setsid ./scripts/run-csharp-product.sh --runtime "$PACK" > "$LOG" 2>&1 < /dev/null &
for _ in $(seq 1 90); do grep -q 'listening at' "$LOG" && break; sleep 2; done
sleep 15
grep -m1 'listening at' "$LOG"
