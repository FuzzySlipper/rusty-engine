#!/usr/bin/env bash
# Idle SSE bytes by batch kind over 10 s with one subscriber, then process-tree
# CPU over 10 s, for one topology of the staged SDK fixture (port 40821).
#   RUSTY_PRODUCT  staged Product directory (contains product.json)
#   RUSTY_WORK     scratch directory for persistence roots
# Usage: idle.sh worker|owned <rusty-product-host>
set -u
: "${RUSTY_PRODUCT:?}" "${RUSTY_WORK:?}"
PROBE=$(cd "$(dirname "$0")/../../worker-owned-io-8740/scripts" && pwd)
mode=$1 bin=$2
rm -rf "$RUSTY_WORK/p-idle" "$RUSTY_WORK/cs-idle"
extra=(); [[ $mode == owned ]] && extra=(--worker-owned-io)
timeout 120 "$bin" --product "$RUSTY_PRODUCT" --loader coreclr "${extra[@]}" \
  --persistence-root "$RUSTY_WORK/p-idle" --content-store-root "$RUSTY_WORK/cs-idle" > /dev/null 2>&1 &
host=$!
for _ in $(seq 60); do ss -ltn | grep -q ":40821 " && break; sleep 0.5; done
sleep 3
echo "== $mode SSE bytes by batch kind, 10 s idle, one subscriber"
python3 "$PROBE/probe.py" 40821 10
echo "== $mode CPU"
bash "$PROBE/cpu.sh" 40821
kill -INT $host; wait $host 2>/dev/null
