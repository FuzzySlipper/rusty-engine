#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ $# -ne 2 ]]; then
  echo "usage: $0 OUTPUT_DIRECTORY ENVIRONMENT_LABEL" >&2
  exit 2
fi
mkdir -p "$1"
OUTPUT_DIRECTORY="$(cd "$1" && pwd)"
cd "$REPO_ROOT"
# Build/crossover probes retain their existing workload. Repeat complete runs
# so the artifact exposes between-run noise, not just within-run percentiles.
for run in 1 2 3; do
  RUSTY_PERF_SKIP_BROWSER=1 "$REPO_ROOT/scripts/run-performance-regression.sh" \
    > "$OUTPUT_DIRECTORY/layers-$run.log" 2>&1
done
cargo run --release --locked -p svc-mesh --example dual_contouring_performance \
  > "$OUTPUT_DIRECTORY/voxel.log" 2>&1
node scripts/performance-results.mjs capture --output "$OUTPUT_DIRECTORY/baseline.json" \
  --environment "$2" "$OUTPUT_DIRECTORY"/layers-*.log "$OUTPUT_DIRECTORY/voxel.log"
