#!/usr/bin/env bash
# The browser path for the same motions: Doom in stream mode, Chromium on the
# input test compositor (Wayland or X11), pointer lock by a click, then the
# same relative motion as wayland_probe.sh.
# usage: browser_probe.sh <runtime-pack> <wayland|x11> <out-prefix>
set -u
PACK="$1" MODE="$2" OUT="$3"
D=${WORK:?set WORK to the working directory}
CHROME=/home/system/crew-services/playtest/browser-binaries/chromium-1243/chrome-linux64/chrome
. "$D/desk/state3/env.sh"
cd /home/agent/dev/rusty-doom
env DOTNET_ROOT=/home/agent/.dotnet TMPDIR=/home/agent/tmp RUSTY_RENDER_OUTPUT=stream LOADING_BAY_LIVE_DEBUG=1 \
  setsid ./scripts/run-csharp-product.sh --runtime "$PACK" > "$OUT-host.log" 2>&1 < /dev/null &
for _ in $(seq 1 90); do grep -q 'listening at' "$OUT-host.log" && break; sleep 2; done
PROFILE=$(mktemp -d /home/agent/tmp/chrome-8859.XXXXXX)
if [[ "$MODE" == x11 ]]; then
  env -u WAYLAND_DISPLAY DISPLAY=:1 "$CHROME" --ozone-platform=x11 --user-data-dir="$PROFILE" --no-first-run --no-sandbox \
    --remote-debugging-port=9334 --start-maximized --app=http://127.0.0.1:4394/ > "$OUT-chrome.log" 2>&1 &
else
  "$CHROME" --ozone-platform=wayland --user-data-dir="$PROFILE" --no-first-run --no-sandbox \
    --remote-debugging-port=9334 --start-maximized --app=http://127.0.0.1:4394/ > "$OUT-chrome.log" 2>&1 &
fi
CHROME_PID=$!
sleep 20
"$D/desk/wayland_probe.sh" "$PACK/bin/rusty-live-debug" http://127.0.0.1:4394 http://127.0.0.1:9334 "$OUT"
kill "$CHROME_PID" 2>/dev/null
pkill -INT -f '^rusty dev --project /home/agent/dev/rusty-doom'
sleep 3
rm -rf "$PROFILE"
