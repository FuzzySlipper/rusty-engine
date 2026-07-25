#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ledger_dir="$repo_root/migration/asha-equivalence"
source_map="$ledger_dir/source-map.tsv"
overrides="${ASHA_EQUIVALENCE_ITEM_OVERRIDES:-$ledger_dir/item-overrides.tsv}"
output=${1:-"$ledger_dir/item-map.tsv"}

mapfile -t inventories < <(find "$ledger_dir/inventory" -maxdepth 1 -type f -name '*.tsv' -print | sort)
temporary=$(mktemp -t rusty-asha-item-map.XXXXXX)
trap 'rm -f "$temporary"' EXIT

awk -F '\t' -v OFS='\t' '
  ARGIND == 1 && FNR > 1 {
    if (NF != 5) {
      printf "item override row %d has %d fields\n", FNR, NF > "/dev/stderr"
      bad = 1
    }
    if ($1 in override_status) {
      printf "duplicate item override: %s\n", $1 > "/dev/stderr"
      bad = 1
    }
    if ($2 != "adapted" && $2 != "equivalent" && $2 != "obsolete") {
      printf "item override %s has bad status %s\n", $1, $2 > "/dev/stderr"
      bad = 1
    }
    if ($3 == "" || $3 == "-" || $4 == "" || $4 == "-" || $5 == "" || $5 == "-") {
      printf "item override %s lacks successor evidence or rationale\n", $1 > "/dev/stderr"
      bad = 1
    }
    override_status[$1] = $2
    override_location[$1] = $3
    override_evidence[$1] = $4
    override_rationale[$1] = $5
    next
  }
  ARGIND == 2 && FNR > 1 {
    status[$2] = $3
    location[$2] = $4
    evidence[$2] = $5
    rationale[$2] = $6
    next
  }
  ARGIND >= 3 && FNR > 1 {
    if ($2 == "file") {
      if ($1 in override_status) {
        printf "file item must use its exact source-map decision: %s\n", $1 > "/dev/stderr"
        bad = 1
      }
      item_status = status[$4]
      item_location = location[$4]
      item_evidence = evidence[$4]
      item_rationale = rationale[$4] " Exact file item: " $6 "."
    } else if ($1 in override_status) {
      item_status = override_status[$1]
      item_location = override_location[$1]
      item_evidence = override_evidence[$1]
      item_rationale = override_rationale[$1]
    } else {
      printf "non-file item lacks explicit decision: %s\n", $1 > "/dev/stderr"
      bad = 1
      next
    }
    print $1, $2, $3, $4, item_status, item_location, item_evidence, item_rationale
    seen[$1] = 1
  }
  END {
    for (item in override_status) {
      if (!(item in seen)) {
        printf "item override is not inventoried: %s\n", item > "/dev/stderr"
        bad = 1
      }
    }
    exit bad
  }
' "$overrides" "$source_map" "${inventories[@]}" > "$temporary"

{
  printf 'item_id\tkind\tdonor_crate\tdonor_path\tstatus\tsuccessor_location\tevidence\trationale\n'
  cat "$temporary"
} > "$output"
