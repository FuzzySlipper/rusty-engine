#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY="$REPO_ROOT/render/donor-inventory.txt"
MATRIX="$REPO_ROOT/render/completeness.tsv"
EXPECTED_COUNT=134
EXPECTED_HASH="99b33ece319e614695bd60c26f723aa0f5bdd48c83488dbd6d6dc4151b67b001"

actual_count="$(wc -l < "$INVENTORY")"
if [[ "$actual_count" != "$EXPECTED_COUNT" ]]; then
  echo "render donor inventory has $actual_count entries; expected $EXPECTED_COUNT" >&2
  exit 1
fi

actual_hash="$(sha256sum "$INVENTORY" | awk '{print $1}')"
if [[ "$actual_hash" != "$EXPECTED_HASH" ]]; then
  echo "render donor inventory hash $actual_hash does not match pinned $EXPECTED_HASH" >&2
  exit 1
fi

expected_header=$'capability\tstatus\tdonor_surface\tsuccessor_owner\tevidence'
if [[ "$(head -n 1 "$MATRIX")" != "$expected_header" ]]; then
  echo "render completeness matrix header is invalid" >&2
  exit 1
fi

awk -F '\t' '
  NR == 1 { next }
  NF != 5 { printf "render completeness row %d has %d fields; expected 5\n", NR, NF > "/dev/stderr"; failed = 1 }
  $1 == "" || $2 == "" || $3 == "" || $4 == "" || $5 == "" {
    printf "render completeness row %d has an empty required field\n", NR > "/dev/stderr"; failed = 1
  }
  $2 != "planned" && $2 != "ported" && $2 != "adapted" && $2 != "equivalent" {
    printf "render completeness row %d has forbidden status %s\n", NR, $2 > "/dev/stderr"; failed = 1
  }
  END { exit failed }
' "$MATRIX"

if [[ "${1:-}" == "--strict" ]] && awk -F '\t' 'NR > 1 && $2 == "planned" { found = 1 } END { exit !found }' "$MATRIX"; then
  echo "render completeness matrix still contains planned capabilities" >&2
  exit 1
fi

echo "render completeness manifest passed"
