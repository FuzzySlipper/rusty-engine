#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 /absolute/path/to/rusty-engine-voxels [/absolute/evidence-directory]" >&2
  exit 2
fi

engine_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_root="$1"
evidence_root="${2:-$engine_root/target/voxel-surface-comparison}"
if [[ "$consumer_root" != /* || "$evidence_root" != /* ]]; then
  echo "consumer and evidence roots must be absolute paths" >&2
  exit 2
fi
if [[ ! -d "$consumer_root/content/projects" ]]; then
  echo "consumer root does not contain content/projects: $consumer_root" >&2
  exit 2
fi

mkdir -p "$evidence_root"
# This directory is a generated evidence target. Remove only this gate's prior
# numbered cards/page/sheet so a renamed corpus entry cannot survive a rerun.
find "$evidence_root" -maxdepth 1 -type f \
  \( -name '[0-9][0-9]-*.png' -o -name 'last-entry-page.png' -o -name 'contact-sheet.png' \) \
  -delete
report_path="$(mktemp /tmp/rusty-voxel-surface-comparison.XXXXXX.json)"
trap 'rm -f "$report_path"' EXIT

project_asset_path() {
  local project_file="$1"
  local asset_id="$2"
  local relative
  relative="$(jq -er --arg asset_id "$asset_id" '.voxelObjects[] | select(.assetId == $asset_id) | .path' "$consumer_root/content/projects/$project_file")"
  printf '%s/%s\n' "$consumer_root" "$relative"
}

normal="$(project_asset_path voxel-lab.project.json voxel-object/retro-character)"
high_fidelity="$(project_asset_path retro-character-high-fidelity.project.json voxel-object/retro-character-high-fidelity)"
animated="$(project_asset_path knight-flipbook.project.json voxel-object/posed-knight-walk)"
hard_surface="$(project_asset_path directional-carve-test.project.json voxel-object/posed-directional-sentinel-carve)"
textured="$(project_asset_path directional-sprite-experiment.project.json voxel-object/posed-directional-sentinel)"
texture="$consumer_root/content/textures/directional-atlas.png"

cd "$engine_root"
cargo run -p render-projection --example voxel_surface_compare --locked -- \
  --output "$report_path" \
  --model "normal=$normal" \
  --model "high-fidelity=$high_fidelity" \
  --animated-model "knight-walk=$animated" \
  --model "hard-surface=$hard_surface" \
  --textured-model "textured-atlas-sentinel=$textured@$texture"

jq '{
  schemaVersion,
  entries: [.entries[] | del(.projection, .resourceIds)],
  aggregateResourceBytes: ([.resources[].byteLength] | add),
  aggregateResourceCount: (.resources | length),
  textureResourceBytes: ([.textureResources[].byteLength] | add // 0),
  textureResourceCount: (.textureResources | length)
}' "$report_path" > "$evidence_root/metrics.json"

RUSTY_SURFACE_REPORT="$report_path" \
RUSTY_SURFACE_EVIDENCE_DIR="$evidence_root" \
PLAYWRIGHT_CHROMIUM_EXECUTABLE="${PLAYWRIGHT_CHROMIUM_EXECUTABLE:-$(command -v chromium)}" \
  pnpm --dir studio exec playwright test \
    --config test/voxel-surface-comparison/playwright.config.ts

mapfile -t captures < <(find "$evidence_root" -maxdepth 1 -type f -name '[0-9][0-9]-*.png' | sort)
if [[ ${#captures[@]} -eq 0 ]]; then
  echo "comparison produced no screenshots" >&2
  exit 1
fi
montage "${captures[@]}" -thumbnail '360x360>' -tile 3x -geometry +8+8 \
  -background '#10141c' "$evidence_root/contact-sheet.png"

echo "voxel surface comparison evidence: $evidence_root"
