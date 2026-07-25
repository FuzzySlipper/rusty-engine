#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_disposition="$repo_root/render/behavior-disposition.tsv"
missing_item=$(mktemp -t rusty-render-missing-behavior.XXXXXX)
fake_evidence=$(mktemp -t rusty-render-fake-evidence.XXXXXX)
generic_rationale=$(mktemp -t rusty-render-generic-rationale.XXXXXX)
output=$(mktemp -t rusty-render-negative-check.XXXXXX)
trap 'rm -f "$missing_item" "$fake_evidence" "$generic_rationale" "$output"' EXIT

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
  !changed && $2 == "test" {
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

echo "render completeness negative checker probes passed"
