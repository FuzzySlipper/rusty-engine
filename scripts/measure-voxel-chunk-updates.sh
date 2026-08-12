#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

frame_dir="$(mktemp -d)"
trap 'rm -rf -- "$frame_dir"' EXIT

VOXEL_BENCH_FRAME_DIR="$frame_dir" \
  cargo run --release -p render-projection --example voxel_chunk_updates --locked
pnpm --dir render --filter @rusty-engine/render-contracts build
pnpm --dir render --filter @rusty-engine/render-projection build
pnpm --dir render --filter @rusty-engine/renderer-three build
node render/scripts/measure-voxel-frame-application.mjs "$frame_dir"
