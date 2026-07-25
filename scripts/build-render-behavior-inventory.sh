#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/asha-engine" >&2
  exit 2
fi

donor=$1
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
inventory="$repo_root/render/donor-inventory.txt"
typescript_extractor="$repo_root/render/scripts/extract-donor-typescript-behavior.mjs"
pin=6462a6de20d48ea1a3b7456826804bd9507860a5

git -C "$donor" cat-file -e "$pin^{commit}"
printf 'item_id\tkind\tdonor_path\tline\tsymbol\n'

while IFS= read -r file; do
  case "$file" in
    *.rs)
      line_number=0
      waiting_for_test=0
      emitted=0
      while IFS= read -r line || [[ -n $line ]]; do
        ((line_number += 1))
        if [[ $line =~ ^[[:space:]]*#\[(test|tokio::test)\] ]]; then
          waiting_for_test=1
          continue
        fi
        if (( waiting_for_test )) && [[ $line =~ ^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
          symbol=${BASH_REMATCH[3]}
          printf 'test:%s:%s\ttest\t%s\t%s\t%s\n' "$file" "$symbol" "$file" "$line_number" "$symbol"
          ((emitted += 1))
          waiting_for_test=0
        fi

        api_kind=
        symbol=
        if [[ $line =~ ^[[:space:]]*pub[[:space:]]+((async|const|unsafe)[[:space:]]+)*fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
          api_kind=fn
          symbol=${BASH_REMATCH[3]}
        elif [[ $line =~ ^[[:space:]]*pub[[:space:]]+(struct|enum|trait|type|const|static|mod)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
          api_kind=${BASH_REMATCH[1]}
          symbol=${BASH_REMATCH[2]}
        fi
        if [[ -n $api_kind ]]; then
          printf 'api:%s:%s:%s\tapi\t%s\t%s\t%s:%s\n' \
            "$file" "$line_number" "$symbol" "$file" "$line_number" "$api_kind" "$symbol"
          ((emitted += 1))
        fi
      done < <(git -C "$donor" show "$pin:$file")
      if (( emitted == 0 )); then
        printf 'internal:%s\tinternal\t%s\t0\tno-public-api-or-test\n' "$file" "$file"
      fi
      ;;
    *.ts)
      node "$typescript_extractor" "$donor" "$pin" "$file"
      ;;
  esac
done < "$inventory"
