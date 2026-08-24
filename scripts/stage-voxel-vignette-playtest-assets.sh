#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_SOURCE_ROOT="/home/dev/asset-pipeline/live-evidence/palette-unlit-6925-20260824-001"
SOURCE_ROOT="${VOXEL_VIGNETTE_ASSET_SOURCE_ROOT:-$DEFAULT_SOURCE_ROOT}"
MANIFEST="$REPO_ROOT/render/voxel-vignette-playtest/staging-manifest.tsv"
DESTINATION="$REPO_ROOT/render/voxel-vignette-playtest/assets"
CHECK_ONLY="${1:-}"

if [[ "$CHECK_ONLY" != "" && "$CHECK_ONLY" != "--check" ]]; then
  echo "usage: $0 [--check]" >&2
  exit 2
fi

source_help() {
  echo "expected stable source root: $DEFAULT_SOURCE_ROOT" >&2
  echo "override for an explicitly supplied producer output: VOXEL_VIGNETTE_ASSET_SOURCE_ROOT=/absolute/palette-unlit-6925-20260824-001" >&2
  echo "producer README: /home/dev/asset-pipeline/experiments/voxels/palette-unlit/README.md" >&2
  echo "producer command: node experiments/voxels/palette-unlit/palette_unlit_glb.mjs --input /absolute/read-only-source.glb --output /absolute/ignored-output/asset-palette-unlit.glb --receipt /absolute/ignored-output/asset-palette-unlit.receipt.json" >&2
}

if [[ ! -d "$SOURCE_ROOT" ]]; then
  echo "missing palette-unlit source root: $SOURCE_ROOT" >&2
  source_help
  exit 1
fi

verify_file() {
  local path="$1"
  local expected_hash="$2"
  local expected_bytes="$3"
  [[ -f "$path" ]] || { echo "missing staged input: $path" >&2; return 1; }
  [[ "$(stat -c '%s' "$path")" == "$expected_bytes" ]] || { echo "size drift: $path" >&2; return 1; }
  [[ "$(sha256sum "$path" | awk '{print $1}')" == "$expected_hash" ]] || { echo "SHA-256 drift: $path" >&2; return 1; }
}

verify_source_file() {
  local path="$1"
  local expected_hash="$2"
  local expected_bytes="$3"
  if [[ ! -f "$path" ]]; then
    echo "missing palette-unlit source input: $path" >&2
    source_help
    return 1
  fi
  verify_file "$path" "$expected_hash" "$expected_bytes"
}

while IFS=$'\t' read -r filename sha256 bytes; do
  [[ -z "$filename" || "$filename" == \#* ]] && continue
  source_path="$SOURCE_ROOT/$filename"
  destination_path="$DESTINATION/$filename"
  verify_source_file "$source_path" "$sha256" "$bytes"
  if [[ "$CHECK_ONLY" == "--check" ]]; then
    verify_file "$destination_path" "$sha256" "$bytes"
    continue
  fi
  mkdir -p "$DESTINATION"
  temporary_path="$destination_path.tmp.$$"
  cp "$source_path" "$temporary_path"
  verify_file "$temporary_path" "$sha256" "$bytes"
  mv -f "$temporary_path" "$destination_path"
  echo "staged $filename"
done < "$MANIFEST"
