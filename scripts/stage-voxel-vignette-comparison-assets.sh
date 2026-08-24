#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/render/voxel-vignette-playtest/comparison-staging-manifest.tsv"
destination="$repo_root/render/voxel-vignette-playtest/assets"
check_only="${1:-}"

if [[ "$check_only" != "" && "$check_only" != "--check" ]]; then
  echo "usage: $0 [--check]" >&2
  exit 2
fi

verify_file() {
  local path="$1" expected_hash="$2" expected_bytes="$3"
  [[ -f "$path" ]] || { echo "missing comparison input: $path" >&2; return 1; }
  [[ "$(stat -c '%s' "$path")" == "$expected_bytes" ]] || { echo "size drift: $path" >&2; return 1; }
  [[ "$(sha256sum "$path" | awk '{print $1}')" == "$expected_hash" ]] || { echo "SHA-256 drift: $path" >&2; return 1; }
}

while IFS=$'\t' read -r variant filename source_path sha256 bytes receipt; do
  [[ -z "$variant" || "$variant" == \#* ]] && continue
  [[ -n "$filename" && -n "$source_path" && -n "$sha256" && -n "$bytes" ]] || { echo "malformed comparison manifest row for $variant" >&2; exit 1; }
  destination_path="$destination/$variant/$filename"
  verify_file "$source_path" "$sha256" "$bytes"
  if [[ "$check_only" == "--check" ]]; then
    verify_file "$destination_path" "$sha256" "$bytes"
    continue
  fi
  mkdir -p "$(dirname "$destination_path")"
  temporary_path="$destination_path.tmp.$$"
  cp "$source_path" "$temporary_path"
  verify_file "$temporary_path" "$sha256" "$bytes"
  mv -f "$temporary_path" "$destination_path"
  echo "staged $variant/$filename"
done < "$manifest"
