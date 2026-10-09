#!/usr/bin/env bash
# Builds an Engine SDK/runtime pair from this checkout into OUT, laid out as
# the `rusty` CLI installs one (runtime-pack/, sdk-feed/), for recapturing
# the GPU lane's scenes (scripts/gpu-lane-recapture.sh,
# docs/verification.md#recapturing-scenes).
#
#   scripts/gpu-lane-pair.sh OUT [VERSION]
#
# VERSION defaults to 0.1.0-dev.lane<commit>, a version no published pair
# has, so restores take this package.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:?usage: scripts/gpu-lane-pair.sh OUT [VERSION]}"
cd "$root"
version="${2:-0.1.0-dev.lane$(git rev-parse --short=12 HEAD)}"

rm -rf target/runtime-pack
scripts/build-runtime-pack.sh
rm -rf "$out"
mkdir -p "$out"
cp -r target/runtime-pack/linux-x64 "$out/runtime-pack"
scripts/pack-csharp-sdk.sh "$version" "$out/sdk-feed"
echo "pair $version: $out"
