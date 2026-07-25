#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_map="$repo_root/migration/asha-equivalence/item-map.tsv"
incomplete=$(mktemp -t rusty-asha-incomplete-map.XXXXXX)
fake_successor=$(mktemp -t rusty-asha-fake-successor.XXXXXX)
fake_evidence=$(mktemp -t rusty-asha-fake-evidence.XXXXXX)
missing_override=$(mktemp -t rusty-asha-missing-override.XXXXXX)
output=$(mktemp -t rusty-asha-negative-check.XXXXXX)
trap 'rm -f "$incomplete" "$fake_successor" "$fake_evidence" "$missing_override" "$output"' EXIT

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

awk -F '\t' -v OFS='\t' '
  NR == 1 { print; next }
  !changed && $2 == "test" {
    $6 = "missing/successor.rs"
    changed = 1
  }
  { print }
  END { exit !changed }
' "$source_map" > "$fake_successor"
if ASHA_EQUIVALENCE_ITEM_MAP="$fake_successor" "$repo_root/scripts/check-asha-equivalence.sh" --final > "$output" 2>&1; then
  echo "asha equivalence checker accepted a nonexistent item successor" >&2
  exit 1
fi
grep -q 'successor location does not exist' "$output" || {
  cat "$output" >&2
  echo "asha equivalence successor probe failed for the wrong reason" >&2
  exit 1
}

awk -F '\t' -v OFS='\t' '
  NR == 1 { print; next }
  !changed && $2 == "test" {
    $7 = "missing/evidence.rs"
    changed = 1
  }
  { print }
  END { exit !changed }
' "$source_map" > "$fake_evidence"
if ASHA_EQUIVALENCE_ITEM_MAP="$fake_evidence" "$repo_root/scripts/check-asha-equivalence.sh" --final > "$output" 2>&1; then
  echo "asha equivalence checker accepted nonexistent item evidence" >&2
  exit 1
fi
grep -q 'evidence reference does not exist' "$output" || {
  cat "$output" >&2
  echo "asha equivalence evidence probe failed for the wrong reason" >&2
  exit 1
}

awk -F '\t' 'NR == 1 { print; next } !removed { removed = 1; next } { print } END { exit !removed }' \
  "$repo_root/migration/asha-equivalence/item-overrides.tsv" > "$missing_override"
if ASHA_EQUIVALENCE_ITEM_MAP="$source_map" \
  ASHA_EQUIVALENCE_ITEM_OVERRIDES="$missing_override" \
  "$repo_root/scripts/build-asha-equivalence-item-map.sh" "$output" > /dev/null 2>&1; then
  echo "asha equivalence builder accepted a non-file item without an explicit decision" >&2
  exit 1
fi

grep -Fq $'test:engine-rs/crates/services/svc-voxel-conversion/src/lib.rs:apply_receipt_is_replay_hash_checked\tobsolete\t' \
  "$repo_root/migration/asha-equivalence/item-overrides.tsv" || {
  echo "replay-hash-only donor test is not explicitly obsolete" >&2
  exit 1
}
echo "asha equivalence negative checker probe passed"
