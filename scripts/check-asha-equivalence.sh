#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ledger_dir="$repo_root/migration/asha-equivalence"
crate_table="$ledger_dir/donor-crates.tsv"
inventory_dir="$ledger_dir/inventory"
disposition="$ledger_dir/disposition.tsv"
source_map="$ledger_dir/source-map.tsv"
item_map="${ASHA_EQUIVALENCE_ITEM_MAP:-$ledger_dir/item-map.tsv}"
meta="$ledger_dir/inventory.meta"
final=0
[[ ${1:-} == --final ]] && final=1

required=(
  core-catalog core-entity core-scene svc-entity-authoring svc-environment-authoring
  svc-serialization protocol-voxel-asset protocol-voxel-annotation
  protocol-voxel-conversion protocol-voxel-edit-history svc-voxel-annotation
  svc-voxel-asset svc-voxel-conversion rule-voxel-edit svc-levelgen svc-mesh-import
  svc-physics rule-trigger-volume rule-animation-controller asset-import protocol-assets
  protocol-entity-authoring protocol-ids protocol-presentation protocol-render protocol-scene
  rule-state-machine protocol-diagnostics scene-diagnostics voxel-diagnostics state-inspector
  rule-relationship
)

fail() {
  echo "asha equivalence check: $*" >&2
  exit 1
}

[[ -d $inventory_dir ]] || fail "missing frozen donor inventory directory"
[[ $(head -n 1 "$crate_table") == $'donor_crate\tdonor_path\tscope\tnote' ]] || fail "bad donor-crates header"
[[ $(head -n 1 "$disposition") == $'donor_crate\tstatus\towner_task\tsuccessor_location\tevidence\trationale' ]] || fail "bad disposition header"
[[ $(head -n 1 "$source_map") == $'donor_crate\tdonor_path\tstatus\tsuccessor_location\tevidence\trationale' ]] || fail "bad source-map header"
[[ $(head -n 1 "$item_map") == $'item_id\tkind\tdonor_crate\tdonor_path\tstatus\tsuccessor_location\tevidence\trationale' ]] || fail "bad item-map header"

mapfile -t inventories < <(find "$inventory_dir" -maxdepth 1 -type f -name '*.tsv' -print | sort)
[[ ${#inventories[@]} -eq ${#required[@]} ]] || fail "expected ${#required[@]} inventory shards, found ${#inventories[@]}"
for inventory in "${inventories[@]}"; do
  [[ $(head -n 1 "$inventory") == $'item_id\tkind\tdonor_crate\tpath\tline\tsymbol' ]] || fail "bad inventory header in $inventory"
done

expected_hash=$(sed -n 's/^inventory_sha256=//p' "$meta")
actual_hash=$(for inventory in "${inventories[@]}"; do tail -n +2 "$inventory"; done | sha256sum | cut -d' ' -f1)
[[ -n $expected_hash && $actual_hash == "$expected_hash" ]] || fail "frozen inventory hash mismatch"

for crate in "${required[@]}"; do
  [[ $(awk -F '\t' -v crate="$crate" 'NR > 1 && $1 == crate { count++ } END { print count + 0 }' "$crate_table") -eq 1 ]] || fail "$crate must occur once in donor-crates.tsv"
  [[ $(awk -F '\t' -v crate="$crate" 'NR > 1 && $1 == crate { count++ } END { print count + 0 }' "$disposition") -eq 1 ]] || fail "$crate must occur once in disposition.tsv"
  shard="$inventory_dir/$crate.tsv"
  [[ -f $shard ]] || fail "$crate has no named inventory shard"
  [[ $(awk -F '\t' -v crate="$crate" 'NR > 1 && $3 == crate && $2 == "file" { count++ } END { print count + 0 }' "$shard") -gt 0 ]] || fail "$crate has no frozen files"
done

duplicates=$(awk -F '\t' 'FNR > 1 { print $1 }' "${inventories[@]}" | sort | uniq -d)
[[ -z $duplicates ]] || fail "duplicate inventory item ids: $duplicates"
duplicate_maps=$(tail -n +2 "$source_map" | cut -f2 | sort | uniq -d)
[[ -z $duplicate_maps ]] || fail "duplicate source-map paths: $duplicate_maps"
duplicate_item_maps=$(tail -n +2 "$item_map" | cut -f1 | sort | uniq -d)
[[ -z $duplicate_item_maps ]] || fail "duplicate item-map ids: $duplicate_item_maps"

validate_repo_path() {
  local item_id=$1
  local field=$2
  local relative=$3
  [[ $relative != /* && $relative != *'..'* ]] || fail "$item_id has unsafe $field path: $relative"
  relative=${relative#./}
  [[ -e "$repo_root/$relative" ]] || fail "$item_id $field does not exist: $relative"
}

validate_cargo_evidence() {
  local item_id=$1
  local evidence=$2
  local expect_package=0
  local token
  for token in $evidence; do
    if [[ $expect_package -eq 1 ]]; then
      grep -Fq "name = \"$token\"" "$repo_root/Cargo.lock" \
        || fail "$item_id evidence names unknown Cargo package: $token"
      expect_package=0
      continue
    fi
    if [[ $token == -p ]]; then
      expect_package=1
    elif [[ $token == */* ]]; then
      validate_repo_path "$item_id" "evidence reference" "$token"
    fi
  done
  [[ $expect_package -eq 0 ]] || fail "$item_id evidence has -p without a package"
}

while IFS=$'\t' read -r item_id kind _ _ status successor_location evidence rationale; do
  [[ $item_id != item_id ]] || continue
  for relative in $successor_location; do
    validate_repo_path "$item_id" "successor location" "$relative"
  done

  case $evidence in
    cargo\ *|RUSTDOCFLAGS=*' cargo '*)
      [[ $evidence == *'cargo test '* || $evidence == *'cargo doc '* || $evidence == *'cargo run '* ]] \
        || fail "$item_id has unsupported evidence command: $evidence"
      validate_cargo_evidence "$item_id" "$evidence"
      ;;
    ./*)
      command_path=${evidence%% *}
      validate_repo_path "$item_id" "evidence command" "$command_path"
      ;;
    *)
      for relative in $evidence; do
        validate_repo_path "$item_id" "evidence reference" "$relative"
      done
      ;;
  esac

  if [[ $kind != file ]]; then
    symbol=${item_id##*:}
    [[ $rationale == *"$symbol"* ]] || fail "$item_id rationale does not name its exact item"
    [[ $rationale == *"explicitly ${status}"* ]] \
      || fail "$item_id rationale does not state an explicit disposition"
  fi
done < "$item_map"

awk -F '\t' '
  NR == FNR && NR > 1 { crates[$1] = 1; next }
  FNR == 1 { next }
  NF != 6 { printf "inventory row %d has %d fields\n", FNR, NF > "/dev/stderr"; bad = 1 }
  !($3 in crates) { printf "inventory row %d names unknown crate %s\n", FNR, $3 > "/dev/stderr"; bad = 1 }
  $2 != "file" && $2 != "api" && $2 != "test" { printf "inventory row %d has bad kind %s\n", FNR, $2 > "/dev/stderr"; bad = 1 }
  END { exit bad }
' "$crate_table" "${inventories[@]}" || fail "invalid frozen inventory"

awk -F '\t' '
  ARGIND == 1 && FNR > 1 { closed[$1] = ($2 != "pending"); next }
  ARGIND == 2 && FNR > 1 {
    if (NF != 6) { printf "source-map row %d has %d fields\n", FNR, NF > "/dev/stderr"; bad = 1 }
    if ($3 != "adapted" && $3 != "equivalent" && $3 != "obsolete") {
      printf "source-map row %d has bad status %s\n", FNR, $3 > "/dev/stderr"; bad = 1
    }
    if ($4 == "" || $4 == "-" || $5 == "" || $5 == "-" || $6 == "" || $6 == "-") {
      printf "source-map row %d lacks successor evidence or rationale\n", FNR > "/dev/stderr"; bad = 1
    }
    mapped[$2] = $1
    next
  }
  FNR == 1 { next }
  {
    inventory[$4] = $3
    if ($2 == "file" && closed[$3] && !($4 in mapped)) {
      printf "closed crate %s lacks exact source map for %s\n", $3, $4 > "/dev/stderr"; bad = 1
    }
  }
  END {
    for (path in mapped) {
      if (!(path in inventory)) { printf "source-map path is not inventoried: %s\n", path > "/dev/stderr"; bad = 1 }
      else if (inventory[path] != mapped[path]) { printf "source-map crate mismatch for %s\n", path > "/dev/stderr"; bad = 1 }
    }
    exit bad
  }
' "$disposition" "$source_map" "${inventories[@]}" || fail "invalid exact source map"

awk -F '\t' '
  ARGIND == 1 && FNR > 1 { closed[$1] = ($2 != "pending"); next }
  ARGIND == 2 && FNR > 1 {
    if (NF != 8) { printf "item-map row %d has %d fields\n", FNR, NF > "/dev/stderr"; bad = 1 }
    if ($5 != "adapted" && $5 != "equivalent" && $5 != "obsolete") {
      printf "item-map row %d has bad status %s\n", FNR, $5 > "/dev/stderr"; bad = 1
    }
    if ($6 == "" || $6 == "-" || $7 == "" || $7 == "-" || $8 == "" || $8 == "-") {
      printf "item-map row %d lacks successor evidence or rationale\n", FNR > "/dev/stderr"; bad = 1
    }
    mapped[$1] = $2 FS $3 FS $4
    next
  }
  FNR == 1 { next }
  {
    inventory[$1] = $2 FS $3 FS $4
    if (closed[$3] && !($1 in mapped)) {
      printf "closed crate %s lacks exact item map for %s\n", $3, $1 > "/dev/stderr"; bad = 1
    }
  }
  END {
    for (item in mapped) {
      if (!(item in inventory)) { printf "item-map id is not inventoried: %s\n", item > "/dev/stderr"; bad = 1 }
      else if (inventory[item] != mapped[item]) { printf "item-map metadata mismatch for %s\n", item > "/dev/stderr"; bad = 1 }
    }
    exit bad
  }
' "$disposition" "$item_map" "${inventories[@]}" || fail "invalid exact item map"

generated_item_map=$(mktemp -t rusty-asha-generated-item-map.XXXXXX)
trap 'rm -f "$generated_item_map"' EXIT
"$repo_root/scripts/build-asha-equivalence-item-map.sh" "$generated_item_map"
cmp -s "$generated_item_map" "$item_map" || fail "item-map.tsv is not the canonical explicit-decision build"

pending=$(awk -F '\t' -v final="$final" '
  NR == 1 { next }
  NF != 6 { printf "disposition row %d has %d fields\n", NR, NF > "/dev/stderr"; bad = 1; next }
  $2 != "adapted" && $2 != "equivalent" && $2 != "obsolete" && $2 != "pending" {
    printf "disposition row %d has bad status %s\n", NR, $2 > "/dev/stderr"; bad = 1
  }
  $3 !~ /^[0-9]+$/ { printf "disposition row %d has invalid owner task\n", NR > "/dev/stderr"; bad = 1 }
  $6 == "" || $6 == "-" { printf "disposition row %d lacks rationale\n", NR > "/dev/stderr"; bad = 1 }
  $2 == "pending" { pending++ }
  $2 != "pending" && ($4 == "" || $4 == "-" || $5 == "" || $5 == "-") {
    printf "closed disposition row %d lacks successor evidence\n", NR > "/dev/stderr"; bad = 1
  }
  END {
    if (bad) exit 2
    if (final && pending) exit 3
    print pending + 0
  }
' "$disposition") || {
  status=$?
  [[ $status -eq 3 ]] && fail "--final rejects pending dispositions"
  fail "invalid disposition ledger"
}

items=$(awk 'FNR > 1 { count++ } END { print count + 0 }' "${inventories[@]}")
files=$(awk -F '\t' 'FNR > 1 && $2 == "file" { count++ } END { print count + 0 }' "${inventories[@]}")
apis=$(awk -F '\t' 'FNR > 1 && $2 == "api" { count++ } END { print count + 0 }' "${inventories[@]}")
tests=$(awk -F '\t' 'FNR > 1 && $2 == "test" { count++ } END { print count + 0 }' "${inventories[@]}")
[[ $items == "$(sed -n 's/^items=//p' "$meta")" ]] || fail "inventory item count mismatch"
[[ $files == "$(sed -n 's/^files=//p' "$meta")" ]] || fail "inventory file count mismatch"
[[ $apis == "$(sed -n 's/^apis=//p' "$meta")" ]] || fail "inventory API count mismatch"
[[ $tests == "$(sed -n 's/^tests=//p' "$meta")" ]] || fail "inventory test count mismatch"
echo "asha equivalence inventory ok: $items items ($files files, $apis APIs, $tests tests), $pending pending crate dispositions"
