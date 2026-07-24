#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 /path/to/asha-engine [donor-crate]" >&2
  exit 2
fi

donor=$1
only_crate=${2:-}
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
crate_table="$repo_root/migration/asha-equivalence/donor-crates.tsv"
pin=6462a6de20d48ea1a3b7456826804bd9507860a5

git -C "$donor" cat-file -e "$pin^{commit}"
printf 'item_id\tkind\tdonor_crate\tpath\tline\tsymbol\n'

while IFS=$'\t' read -r crate path scope note; do
  [[ $crate == donor_crate ]] && continue
  [[ -n $only_crate && $crate != "$only_crate" ]] && continue
  while IFS= read -r file; do
    printf '%s\tfile\t%s\t%s\t0\t-\n' "file:$file" "$crate" "$file"
    [[ $file == *.rs ]] || continue

    line_number=0
    waiting_for_test=0
    while IFS= read -r line || [[ -n $line ]]; do
      ((line_number += 1))
      if [[ $line =~ ^[[:space:]]*#\[(test|tokio::test)\] ]]; then
        waiting_for_test=1
        continue
      fi
      if (( waiting_for_test )) && [[ $line =~ ^[[:space:]]*fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
        symbol=${BASH_REMATCH[1]}
        printf 'test:%s:%s\ttest\t%s\t%s\t%s\t%s\n' "$file" "$symbol" "$crate" "$file" "$line_number" "$symbol"
        waiting_for_test=0
      fi

      api_kind=
      symbol=
      if [[ $line =~ ^[[:space:]]*pub[[:space:]]+(async[[:space:]]+)?(const[[:space:]]+)?fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
        api_kind=fn
        symbol=${BASH_REMATCH[3]}
      elif [[ $line =~ ^pub[[:space:]]+(struct|enum|trait|type|const|static|mod)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
        api_kind=${BASH_REMATCH[1]}
        symbol=${BASH_REMATCH[2]}
      fi
      if [[ -n $api_kind ]]; then
        printf 'api:%s:%s:%s\tapi\t%s\t%s\t%s\t%s:%s\n' "$file" "$line_number" "$symbol" "$crate" "$file" "$line_number" "$api_kind" "$symbol"
      fi
    done < <(git -C "$donor" show "$pin:$file")
  done < <(git -C "$donor" ls-tree -r --name-only "$pin" -- "$path")
done < "$crate_table"

if [[ -n $only_crate ]] && ! awk -F '\t' -v crate="$only_crate" 'NR > 1 && $1 == crate { found = 1 } END { exit !found }' "$crate_table"; then
  echo "unknown donor crate: $only_crate" >&2
  exit 2
fi
