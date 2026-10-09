#!/usr/bin/env bash
# Runs the GPU verification lane (docs/verification.md#gpu-verification) on
# this machine for the checked-out commit: builds the candidate renderer and
# the lane tool, renders every scene against the machine's accepted baseline,
# and prints the report. Exits 0 on pass, 1 when flagged, 2 when a candidate
# render failed.
#
#   scripts/gpu-lane.sh [--scene NAME]... [--repeats N]
#
# RUSTY_GPU_LANE_ROOT is the lane directory (scenes.json, scenes/, machines/,
# runs/); RUSTY_GPU_LANE_MACHINE names this machine's baseline (else the host
# name).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lane="${RUSTY_GPU_LANE_ROOT:-/data/rusty-engine-gpu-lane}"
cd "$root"

cargo build --release -p csharp-product-runtime --bin rusty-scene-render -p render-verify --bin rusty-gpu-lane

head="$(git rev-parse HEAD)"
source="$head"
if ! git diff --quiet HEAD -- rust Cargo.toml Cargo.lock; then
  source="$head (with uncommitted changes)"
fi
machine="${RUSTY_GPU_LANE_MACHINE:-$(hostname | tr '[:upper:]' '[:lower:]')}"
out="$lane/runs/$machine/$(date -u +%Y%m%dT%H%M%SZ)-${head:0:12}"

set +e
target/release/rusty-gpu-lane run --root "$lane" --machine "$machine" \
  --candidate target/release/rusty-scene-render --source "$source" \
  --checkout "$root" --out "$out" "$@"
status=$?
set -e
echo "run: $out"
exit "$status"
