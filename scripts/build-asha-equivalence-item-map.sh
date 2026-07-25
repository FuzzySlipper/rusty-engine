#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ledger_dir="$repo_root/migration/asha-equivalence"
source_map="$ledger_dir/source-map.tsv"
overrides="$ledger_dir/item-overrides.tsv"
output=${1:-"$ledger_dir/item-map.tsv"}

mapfile -t inventories < <(find "$ledger_dir/inventory" -maxdepth 1 -type f -name '*.tsv' -print | sort)
temporary=$(mktemp -t rusty-asha-item-map.XXXXXX)
trap 'rm -f "$temporary"' EXIT

awk -F '\t' -v OFS='\t' '
  ARGIND == 1 && FNR > 1 {
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
    item_status = ($1 in override_status) ? override_status[$1] : status[$4]
    item_location = ($1 in override_location) ? override_location[$1] : location[$4]
    item_evidence = ($1 in override_evidence) ? override_evidence[$1] : evidence[$4]
    item_rationale = ($1 in override_rationale) ? override_rationale[$1] : rationale[$4] " Exact " $2 " item: " $6 "."
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
