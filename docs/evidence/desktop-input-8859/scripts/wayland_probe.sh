#!/usr/bin/env bash
# Doom input through KWin's fake input (one virtual pointer and keyboard for
# the whole session, as a plugged-in mouse and keyboard): click to lock, a known
# relative motion, W, Escape, F3.
# usage: wayland_probe.sh <live-debug> <origin> <cdp-origin|-> <out-prefix> [click-x click-y]
set -u
LIVE="$1" ORIGIN="$2" CDP="$3" OUT="$4" CX="${5:-640}" CY="${6:-420}"
D=${WORK:?set WORK to the working directory}
. "$D/desk/state3/env.sh"
coproc FAKE { "$D/fakeinput/target/release/fakeinput"; }
send() { echo "$*" >&"${FAKE[1]}"; read -r _ <&"${FAKE[0]}"; }
readout() { "$LIVE" --origin "$ORIGIN" --command loading-bay.readout | tr ';' '\n' | grep -E "^$1=" | cut -d= -f2; }
yaw() { python3 -c "import math,sys; print(round(math.degrees(float(sys.argv[1])), 3))" "$(readout yawRadians)"; }
lock() {
  if [[ "$CDP" == - ]]; then echo n/a; return; fi
  node "$D/desk/cdp.mjs" "$CDP" 'document.pointerLockElement ? document.pointerLockElement.tagName : null' 2>/dev/null | tail -1
}
send sleep 500
echo "lock before click: $(lock)"
send abs "$CX" "$CY" sleep 200 button 272 1 sleep 60 button 272 0 sleep 600
echo "lock after click: $(lock)"
y0=$(yaw)
for _ in $(seq 1 10); do send rel 10 0 sleep 20; done
sleep 0.4
y1=$(yaw)
send rel 100 0 sleep 400
y2=$(yaw)
send rel -50 0 sleep 400
y3=$(yaw)
echo "yaw: $y0 -> $y1 (10 x 10 counts) -> $y2 (+100 in one event) -> $y3 (-50)"
p0=$(readout position)
send key 17 1 sleep 600 key 17 0 sleep 300
p1=$(readout position)
echo "W held 600 ms: $p0 -> $p1"
send key 1 1 sleep 50 key 1 0 sleep 400
echo "lock after Escape: $(lock)"
send key 61 1 sleep 50 key 61 0 sleep 800
timeout 30 spectacle -b -n -f -o "$OUT-f3.png" >/dev/null 2>&1
echo "f3 screenshot: $OUT-f3.png"
exec {FAKE[1]}>&-
wait
