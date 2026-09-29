#!/usr/bin/env bash
# One X11 window run: launch, read the client geometry, stop, print the saved placement.
set -u
PACK="$1" LOG="$2"
D=${DESK:?set DESK to the directory holding the compositor state (state3/env.sh)}
(. $D/state3/env.sh; unset WAYLAND_DISPLAY; export DISPLAY=:1; cd /home/agent/dev/rusty-doom && env MSBUILDDISABLENODEREUSE=1 DOTNET_ROOT=/home/agent/.dotnet TMPDIR=/home/agent/tmp RUSTY_RENDER_OUTPUT=window setsid ./scripts/run-csharp-product.sh ${PACK:+--runtime "$PACK"} > "$LOG" 2>&1 < /dev/null &)
for _ in $(seq 1 60); do grep -q 'listening at' "$LOG" && break; sleep 2; done
sleep 8
echo "client: $(DISPLAY=:1 xwininfo -root -tree | grep -i 'loading bay' | grep -oE '[0-9]+x[0-9]+\+[0-9-]+\+[0-9-]+' | head -1)"
P=$(pgrep -f "rusty dev --project /home/agent/dev/rusty-doom" | head -1)
kill -INT "$P"; sleep 5
echo "saved: $(cat /home/agent/dev/rusty-doom/.runtime/persistence/desktop-window)"
