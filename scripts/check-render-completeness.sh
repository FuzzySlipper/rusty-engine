#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY="$REPO_ROOT/render/donor-inventory.txt"
MATRIX="$REPO_ROOT/render/completeness.tsv"
DISPOSITION="$REPO_ROOT/render/donor-disposition.tsv"
BEHAVIOR_INVENTORY="$REPO_ROOT/render/behavior-inventory.tsv"
BEHAVIOR_DISPOSITION="${RENDER_BEHAVIOR_DISPOSITION:-$REPO_ROOT/render/behavior-disposition.tsv}"
EXPECTED_COUNT=134
EXPECTED_HASH="99b33ece319e614695bd60c26f723aa0f5bdd48c83488dbd6d6dc4151b67b001"
EXPECTED_BEHAVIOR_COUNT=1076
EXPECTED_BEHAVIOR_HASH="ad67729a837be928acffeb9934b6f82a4698e84e8b05b2463c92d0e94ed84608"

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

expected_disposition_header=$'donor_path\tstatus\tcapability\tsuccessor_evidence\tadaptation'
if [[ "$(head -n 1 "$DISPOSITION")" != "$expected_disposition_header" ]]; then
  echo "render donor disposition header is invalid" >&2
  exit 1
fi

awk -F '\t' '
  NR == 1 { next }
  NF != 5 { printf "render donor disposition row %d has %d fields; expected 5\n", NR, NF > "/dev/stderr"; failed = 1 }
  $1 == "" || $2 == "" || $3 == "" || $4 == "" || $5 == "" {
    printf "render donor disposition row %d has an empty required field\n", NR > "/dev/stderr"; failed = 1
  }
  $2 != "adapted" && $2 != "equivalent" {
    printf "render donor disposition row %d has non-final status %s\n", NR, $2 > "/dev/stderr"; failed = 1
  }
  END { exit failed }
' "$DISPOSITION"

inventory_paths="$(mktemp -t rusty-render-inventory.XXXXXX)"
disposition_paths="$(mktemp -t rusty-render-disposition.XXXXXX)"
behavior_items="$(mktemp -t rusty-render-behavior-items.XXXXXX)"
mapped_behavior_items="$(mktemp -t rusty-render-behavior-map.XXXXXX)"
trap 'rm -f "$inventory_paths" "$disposition_paths" "$behavior_items" "$mapped_behavior_items"' EXIT
sort "$INVENTORY" > "$inventory_paths"
tail -n +2 "$DISPOSITION" | cut -f1 | sort > "$disposition_paths"
if ! cmp -s "$inventory_paths" "$disposition_paths"; then
  echo "render donor disposition does not account for every frozen donor path exactly once" >&2
  diff -u "$inventory_paths" "$disposition_paths" >&2 || true
  exit 1
fi

while IFS=$'\t' read -r donor_path _ capability successor_evidence _; do
  [[ "$donor_path" == "donor_path" ]] && continue
  if ! awk -F '\t' -v capability="$capability" 'NR > 1 && $1 == capability { found = 1 } END { exit !found }' "$MATRIX"; then
    echo "render donor $donor_path names unknown capability $capability" >&2
    exit 1
  fi
  for evidence_path in $successor_evidence; do
    if [[ ! -e "$REPO_ROOT/$evidence_path" ]]; then
      echo "render donor $donor_path names missing successor evidence $evidence_path" >&2
      exit 1
    fi
  done
done < "$DISPOSITION"

while IFS=$'\t' read -r capability _ _ _ evidence; do
  [[ "$capability" == "capability" ]] && continue
  for evidence_path in $evidence; do
    if [[ ! -e "$REPO_ROOT/$evidence_path" ]]; then
      echo "render capability $capability names missing evidence $evidence_path" >&2
      exit 1
    fi
  done
done < "$MATRIX"

if [[ "${1:-}" == "--strict" ]] && awk -F '\t' 'NR > 1 && $2 == "planned" { found = 1 } END { exit !found }' "$MATRIX"; then
  echo "render completeness matrix still contains planned capabilities" >&2
  exit 1
fi

expected_behavior_header=$'item_id\tkind\tdonor_path\tline\tsymbol'
if [[ "$(head -n 1 "$BEHAVIOR_INVENTORY")" != "$expected_behavior_header" ]]; then
  echo "render behavior inventory header is invalid" >&2
  exit 1
fi

actual_behavior_count="$(($(wc -l < "$BEHAVIOR_INVENTORY") - 1))"
if [[ "$actual_behavior_count" != "$EXPECTED_BEHAVIOR_COUNT" ]]; then
  echo "render behavior inventory has $actual_behavior_count items; expected $EXPECTED_BEHAVIOR_COUNT" >&2
  exit 1
fi

actual_behavior_hash="$(sha256sum "$BEHAVIOR_INVENTORY" | awk '{print $1}')"
if [[ "$actual_behavior_hash" != "$EXPECTED_BEHAVIOR_HASH" ]]; then
  echo "render behavior inventory hash $actual_behavior_hash does not match pinned $EXPECTED_BEHAVIOR_HASH" >&2
  exit 1
fi

awk -F '\t' '
  ARGIND == 1 { donor_paths[$1] = 1; if ($1 ~ /\.(rs|ts)$/) code_paths[$1] = 1; next }
  ARGIND == 2 && FNR == 1 { next }
  ARGIND == 2 {
    if (NF != 5) {
      printf "render behavior inventory row %d has %d fields; expected 5\n", FNR, NF > "/dev/stderr"
      failed = 1
    }
    if ($1 == "" || $2 == "" || $3 == "" || $4 == "" || $5 == "") {
      printf "render behavior inventory row %d has an empty required field\n", FNR > "/dev/stderr"
      failed = 1
    }
    if ($2 != "api" && $2 != "test" && $2 != "internal") {
      printf "render behavior item %s has unsupported kind %s\n", $1, $2 > "/dev/stderr"
      failed = 1
    }
    if (index($1, $2 ":") != 1) {
      printf "render behavior item %s does not match kind %s\n", $1, $2 > "/dev/stderr"
      failed = 1
    }
    if (!($3 in donor_paths) || !($3 in code_paths)) {
      printf "render behavior item %s names a non-code or unfrozen donor path %s\n", $1, $3 > "/dev/stderr"
      failed = 1
    }
    if ($4 !~ /^[0-9]+$/ || ($2 != "internal" && $4 == 0)) {
      printf "render behavior item %s has invalid source line %s\n", $1, $4 > "/dev/stderr"
      failed = 1
    }
    if ($1 in seen) {
      printf "duplicate render behavior inventory item %s\n", $1 > "/dev/stderr"
      failed = 1
    }
    seen[$1] = 1
    covered[$3] = 1
  }
  END {
    for (path in code_paths) {
      if (!(path in covered)) {
        printf "render code donor lacks behavior inventory coverage: %s\n", path > "/dev/stderr"
        failed = 1
      }
    }
    exit failed
  }
' "$INVENTORY" "$BEHAVIOR_INVENTORY"

expected_behavior_disposition_header=$'item_id\tkind\tdonor_path\tstatus\tcapability\tsuccessor_evidence\trationale'
if [[ "$(head -n 1 "$BEHAVIOR_DISPOSITION")" != "$expected_behavior_disposition_header" ]]; then
  echo "render behavior disposition header is invalid" >&2
  exit 1
fi

awk -F '\t' '
  ARGIND == 1 && FNR > 1 { capabilities[$1] = 1; next }
  ARGIND == 2 && FNR > 1 {
    item_kind[$1] = $2
    item_path[$1] = $3
    item_symbol[$1] = $5
    next
  }
  ARGIND == 3 && FNR == 1 { next }
  ARGIND == 3 {
    if (NF != 7) {
      printf "render behavior disposition row %d has %d fields; expected 7\n", FNR, NF > "/dev/stderr"
      failed = 1
    }
    if ($1 == "" || $2 == "" || $3 == "" || $4 == "" || $5 == "" || $6 == "" || $7 == "") {
      printf "render behavior disposition row %d has an empty required field\n", FNR > "/dev/stderr"
      failed = 1
    }
    if (!($1 in item_kind)) {
      printf "render behavior disposition names unknown item %s\n", $1 > "/dev/stderr"
      failed = 1
    } else if ($2 != item_kind[$1] || $3 != item_path[$1]) {
      printf "render behavior disposition metadata differs from inventory for %s\n", $1 > "/dev/stderr"
      failed = 1
    }
    if ($4 != "adapted" && $4 != "equivalent" && $4 != "obsolete") {
      printf "render behavior item %s has non-final status %s\n", $1, $4 > "/dev/stderr"
      failed = 1
    }
    if (!($5 in capabilities)) {
      printf "render behavior item %s names unknown capability %s\n", $1, $5 > "/dev/stderr"
      failed = 1
    }
    if (index($7, item_symbol[$1]) == 0 || index($7, "explicitly " $4) == 0) {
      printf "render behavior item %s lacks an exact explicit rationale\n", $1 > "/dev/stderr"
      failed = 1
    }
    if ($1 in mapped) {
      printf "duplicate render behavior disposition item %s\n", $1 > "/dev/stderr"
      failed = 1
    }
    mapped[$1] = 1
  }
  END {
    for (item in item_kind) {
      if (!(item in mapped)) {
        printf "render behavior item lacks exact behavior disposition: %s\n", item > "/dev/stderr"
        failed = 1
      }
    }
    exit failed
  }
' "$MATRIX" "$BEHAVIOR_INVENTORY" "$BEHAVIOR_DISPOSITION"

tail -n +2 "$BEHAVIOR_INVENTORY" | cut -f1 | sort > "$behavior_items"
tail -n +2 "$BEHAVIOR_DISPOSITION" | cut -f1 | sort > "$mapped_behavior_items"
if ! cmp -s "$behavior_items" "$mapped_behavior_items"; then
  echo "render behavior disposition does not account for every frozen behavior item exactly once" >&2
  diff -u "$behavior_items" "$mapped_behavior_items" >&2 || true
  exit 1
fi

while IFS=$'\t' read -r item_id _ _ _ _ successor_evidence _; do
  [[ "$item_id" == "item_id" ]] && continue
  for evidence_path in $successor_evidence; do
    if [[ "$evidence_path" == /* || "$evidence_path" == ".." || "$evidence_path" == ../* || "$evidence_path" == */../* || "$evidence_path" == */.. ]]; then
      echo "render behavior item $item_id names unsafe evidence $evidence_path" >&2
      exit 1
    fi
    if [[ ! -e "$REPO_ROOT/$evidence_path" ]]; then
      echo "render behavior item $item_id names missing successor evidence $evidence_path" >&2
      exit 1
    fi
  done
done < "$BEHAVIOR_DISPOSITION"

echo "render completeness manifest passed"
echo "render behavior-level disposition passed: $actual_behavior_count exact API, test, and internal items"
