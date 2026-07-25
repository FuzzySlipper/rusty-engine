#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_disposition="$repo_root/render/behavior-disposition.tsv"
missing_item=$(mktemp -t rusty-render-missing-behavior.XXXXXX)
fake_evidence=$(mktemp -t rusty-render-fake-evidence.XXXXXX)
generic_rationale=$(mktemp -t rusty-render-generic-rationale.XXXXXX)
semantic_status=$(mktemp -t rusty-render-semantic-status.XXXXXX)
mixed_scope=$(mktemp -t rusty-render-mixed-scope.XXXXXX)
output=$(mktemp -t rusty-render-negative-check.XXXXXX)
trap 'rm -f "$missing_item" "$fake_evidence" "$generic_rationale" "$semantic_status" "$mixed_scope" "$output"' EXIT

awk -F '\t' '
  NR == 1 { print; next }
  !removed && $2 == "api" { removed = 1; next }
  { print }
  END { exit !removed }
' "$source_disposition" > "$missing_item"
if RENDER_BEHAVIOR_DISPOSITION="$missing_item" \
  "$repo_root/scripts/check-render-completeness.sh" --strict > "$output" 2>&1; then
  echo "render completeness checker accepted an omitted API behavior" >&2
  exit 1
fi
grep -q 'lacks exact behavior disposition' "$output" || {
  cat "$output" >&2
  echo "render missing-behavior probe failed for the wrong reason" >&2
  exit 1
}

awk -F '\t' -v OFS='\t' '
  NR == 1 { print; next }
  !changed && $2 == "test" && index($7, "Removed concept:") == 0 {
    $6 = "missing/render-behavior-evidence.ts"
    changed = 1
  }
  { print }
  END { exit !changed }
' "$source_disposition" > "$fake_evidence"
if RENDER_BEHAVIOR_DISPOSITION="$fake_evidence" \
  "$repo_root/scripts/check-render-completeness.sh" --strict > "$output" 2>&1; then
  echo "render completeness checker accepted nonexistent behavior evidence" >&2
  exit 1
fi
grep -q 'names missing successor evidence' "$output" || {
  cat "$output" >&2
  echo "render behavior-evidence probe failed for the wrong reason" >&2
  exit 1
}

awk -F '\t' -v OFS='\t' '
  NR == 1 { print; next }
  !changed && $2 == "api" {
    $7 = "The donor file is covered by its file-level disposition."
    changed = 1
  }
  { print }
  END { exit !changed }
' "$source_disposition" > "$generic_rationale"
if RENDER_BEHAVIOR_DISPOSITION="$generic_rationale" \
  "$repo_root/scripts/check-render-completeness.sh" --strict > "$output" 2>&1; then
  echo "render completeness checker accepted a generic file-level behavior claim" >&2
  exit 1
fi
grep -q 'lacks an exact explicit rationale' "$output" || {
  cat "$output" >&2
  echo "render exact-rationale probe failed for the wrong reason" >&2
  exit 1
}

awk -F '\t' -v OFS='\t' '
  NR == 1 { print; next }
  $1 ~ /snapshot_decode_rejects_state_that_is_not_derived_from_its_replay_log$/ {
    $4 = "equivalent"
    sub(/explicitly obsolete/, "explicitly equivalent", $7)
    changed = 1
  }
  { print }
  END { exit !changed }
' "$source_disposition" > "$semantic_status"
if RENDER_BEHAVIOR_DISPOSITION="$semantic_status" \
  "$repo_root/scripts/check-render-completeness.sh" --strict > "$output" 2>&1; then
  echo "render completeness checker accepted a replay-only item as equivalent" >&2
  exit 1
fi
grep -q 'requires status obsolete' "$output" || {
  cat "$output" >&2
  echo "render replay-status probe failed for the wrong reason" >&2
  exit 1
}

awk -F '\t' -v OFS='\t' '
  NR == 1 { print; next }
  $1 ~ /identical_inputs_produce_identical_state_and_replay_hashes$/ {
    $7 = "Exact donor test identical_inputs_produce_identical_state_and_replay_hashes is explicitly adapted under animation-controller. Deterministic state remains covered."
    changed = 1
  }
  { print }
  END { exit !changed }
' "$source_disposition" > "$mixed_scope"
if RENDER_BEHAVIOR_DISPOSITION="$mixed_scope" \
  "$repo_root/scripts/check-render-completeness.sh" --strict > "$output" 2>&1; then
  echo "render completeness checker accepted a mixed replay item without its removed scope" >&2
  exit 1
fi
grep -q 'status/rationale mismatch' "$output" || {
  cat "$output" >&2
  echo "render mixed-scope probe failed for the wrong reason" >&2
  exit 1
}

echo "render completeness negative checker probes passed"
