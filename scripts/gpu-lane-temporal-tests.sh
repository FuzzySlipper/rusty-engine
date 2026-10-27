#!/usr/bin/env bash
# Run ignored hardware-only cloud, TAAU and label image/history tests.
# WGPU_ADAPTER_NAME uses wgpu's standard adapter selector; choosing a software
# adapter makes these tests fail instead of reporting an empty visual pass.
#
#   WGPU_ADAPTER_NAME="NVIDIA GeForce RTX 3080" scripts/gpu-lane-temporal-tests.sh
#   WGPU_ADAPTER_NAME="NVIDIA GeForce RTX 3080" scripts/gpu-lane-temporal-tests.sh --dlss

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

features=()
case "${1:-}" in
  "") ;;
  --dlss) features+=(--features dlss) ;;
  *) echo "usage: scripts/gpu-lane-temporal-tests.sh [--dlss]" >&2; exit 2 ;;
esac
if [ "$#" -gt 1 ]; then
  echo "usage: scripts/gpu-lane-temporal-tests.sh [--dlss]" >&2
  exit 2
fi

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export WGPU_BACKEND="${WGPU_BACKEND:-vulkan}"

cargo test --release -p render-wgpu --locked "${features[@]}" --test taau -- --ignored
cargo test --release -p render-wgpu --locked "${features[@]}" --test labels -- --ignored
cargo test --release -p render-wgpu --locked "${features[@]}" --test volumetric_clouds -- --ignored
cargo test --release -p render-wgpu --locked "${features[@]}" --test clouds a_sun_below_the_horizon_lights_the_clouds_through_the_twilight_reddening_flat_and_volumetric_alike -- --exact --ignored
