#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_map="$repo_root/migration/asha-equivalence/item-map.tsv"
incomplete=$(mktemp -t rusty-asha-incomplete-map.XXXXXX)
output=$(mktemp -t rusty-asha-negative-check.XXXXXX)
trap 'rm -f "$incomplete" "$output"' EXIT

awk -F '\t' 'BEGIN { removed = 0 } NR == 1 { print; next } !removed && $2 == "api" { removed = 1; next } { print } END { exit !removed }' "$source_map" > "$incomplete"
if ASHA_EQUIVALENCE_ITEM_MAP="$incomplete" "$repo_root/scripts/check-asha-equivalence.sh" --final > "$output" 2>&1; then
  echo "asha equivalence checker accepted an unmapped API item" >&2
  exit 1
fi
grep -q 'lacks exact item map' "$output" || {
  cat "$output" >&2
  echo "asha equivalence negative probe failed for the wrong reason" >&2
  exit 1
}
echo "asha equivalence negative checker probe passed"
