#!/usr/bin/env bash
# Idle SSE bytes by batch kind over 10 s with one subscriber, then process-tree
# CPU over 10 s, for the staged SDK fixture (port 40821). Adapted from
# docs/evidence/runtime-readout-8768/scripts/idle.sh after the worker topology
# (#8766) and --content-store-root (#8763) were removed.
#   RUSTY_PRODUCT  staged Product directory (contains product.json)
#   RUSTY_WORK     scratch directory for the persistence root
# Usage: idle.sh <rusty-product-host>
set -u
: "${RUSTY_PRODUCT:?}" "${RUSTY_WORK:?}"
PROBE=$(cd "$(dirname "$0")/../../worker-owned-io-8740/scripts" && pwd)
bin=$1
rm -rf "$RUSTY_WORK/p-idle"
timeout 120 "$bin" --product "$RUSTY_PRODUCT" --loader coreclr \
  --persistence-root "$RUSTY_WORK/p-idle" > /dev/null 2>&1 &
host=$!
for _ in $(seq 60); do ss -ltn | grep -q ":40821 " && break; sleep 0.5; done
ss -ltn | grep -q ":40821 " || { echo "idle.sh: the host did not start" >&2; exit 1; }
sleep 3
echo "== SSE bytes by batch kind, 10 s idle, one subscriber"
python3 "$PROBE/probe.py" 40821 10
echo "== CPU"
bash "$PROBE/cpu.sh" 40821
kill -INT $host; wait $host 2>/dev/null
