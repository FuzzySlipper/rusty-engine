#!/usr/bin/env bash
# Recaptures the GPU lane's scenes from a pair and makes that pair's renderer
# this machine's baseline, together, with the reason recorded: what a change
# to the scene snapshot format needs (docs/verification.md#recapturing-scenes).
#
#   scripts/gpu-lane-recapture.sh --pair DIR --reason TEXT [--scene NAME]...
#
# Each scene file's recipe is in the lane's captures.json. A scene whose
# capture fails keeps its file, and then nothing is accepted. The previous
# scene files stay beside the new ones as <file>.previous, and the previous
# baseline record in machines/<machine>/ as baseline-<time>.json. Each
# recapture is appended to the lane's recaptures.log.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lane="${RUSTY_GPU_LANE_ROOT:-/data/rusty-engine-gpu-lane}"
machine="${RUSTY_GPU_LANE_MACHINE:-$(hostname | tr '[:upper:]' '[:lower:]')}"
pair=""
reason=""
scenes=()
while [ $# -gt 0 ]; do
  case "$1" in
    --pair) pair="$2"; shift 2 ;;
    --reason) reason="$2"; shift 2 ;;
    --scene) scenes+=(--scene "$2"); shift 2 ;;
    *) echo "unexpected argument $1" >&2; exit 3 ;;
  esac
done
if [ -z "$pair" ] || [ -z "$reason" ]; then
  echo "usage: scripts/gpu-lane-recapture.sh --pair DIR --reason TEXT [--scene NAME]..." >&2
  exit 3
fi
pair="$(cd "$pair" && pwd)"
cd "$root"

cargo build --release -p render-verify --bin rusty-gpu-lane
target/release/rusty-gpu-lane capture --root "$lane" --pair "$pair" --checkout "$root" \
  --work "$lane/capture-work" "${scenes[@]}"

version="$(basename "$pair"/sdk-feed/Rusty.Engine.*.nupkg .nupkg)"
version="${version#Rusty.Engine.}"
target/release/rusty-gpu-lane accept --root "$lane" --machine "$machine" \
  --renderer "$pair/runtime-pack/bin/rusty-scene-render" \
  --source "pair $version (scenes recaptured)" --reason "$reason"
printf '%s\t%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$version" \
  "${scenes[*]:-all scenes}" "$reason" >> "$lane/recaptures.log"
echo "recaptured on pair $version and accepted it as the baseline of $machine"
