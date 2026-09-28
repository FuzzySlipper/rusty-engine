#!/usr/bin/env bash
# CPU seconds used over 10 s by the process tree serving port $1, one subscriber attached.
P=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(ss -ltnp | grep ":$1 " | grep -o "pid=[0-9]*" | head -1 | cut -d= -f2)
# The listener may be held by the child; walk up to the top rusty-product-host.
while ppid=$(ps -o ppid= -p "$root" | tr -d ' '); [[ "$(ps -o comm= -p "$ppid")" == rusty-product-h* ]]; do root=$ppid; done
tree() { echo "$1"; for c in $(pgrep -P "$1"); do tree "$c"; done; }
ticks() { local t=0; for p in $(tree "$root"); do read -r u s < <(awk '{print $14, $15}' /proc/$p/stat); t=$((t+u+s)); done; echo $t; }
(timeout 13 python3 $P/probe.py "$1" 12 > /dev/null 2>&1 &)
sleep 1.5; a=$(ticks); sleep 10; b=$(ticks)
echo "{\"processes\":$(tree "$root" | wc -l),\"cpuPercentOfOneCore\":$(awk "BEGIN{print ($b-$a)/10}")}"
