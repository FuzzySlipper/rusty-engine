#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY="$REPO_ROOT/render/donor-inventory.txt"
MATRIX="$REPO_ROOT/render/completeness.tsv"
DISPOSITION="$REPO_ROOT/render/donor-disposition.tsv"
ASHA_ITEM_MAP="$REPO_ROOT/migration/asha-equivalence/item-map.tsv"
ASHA_INVENTORY_DIR="$REPO_ROOT/migration/asha-equivalence/inventory"
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
render_items="$(mktemp -t rusty-render-items.XXXXXX)"
mapped_render_items="$(mktemp -t rusty-render-item-map.XXXXXX)"
trap 'rm -f "$inventory_paths" "$disposition_paths" "$render_items" "$mapped_render_items"' EXIT
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

awk -F '\t' 'FNR > 1 { print $1 }' \
  "$ASHA_INVENTORY_DIR/protocol-presentation.tsv" \
  "$ASHA_INVENTORY_DIR/protocol-render.tsv" \
  "$ASHA_INVENTORY_DIR/rule-animation-controller.tsv" | sort > "$render_items"
awk -F '\t' 'NR > 1 && ($3 == "protocol-presentation" || $3 == "protocol-render" || $3 == "rule-animation-controller") { print $1 }' \
  "$ASHA_ITEM_MAP" | sort > "$mapped_render_items"
if ! cmp -s "$render_items" "$mapped_render_items"; then
  echo "render behavior item disposition is incomplete" >&2
  diff -u "$render_items" "$mapped_render_items" >&2 || true
  exit 1
fi
if ! awk -F '\t' '$1 ~ /ModelMaterialPreview(Request|Snapshot)$/ && $6 ~ /model_preview.rs/ && $7 ~ /model_material_preview.rs/ { found++ } END { exit found != 2 }' "$ASHA_ITEM_MAP"; then
  echo "render behavior map lacks explicit model/material preview evidence" >&2
  exit 1
fi

echo "render completeness manifest passed"
echo "render behavior-level disposition passed"
