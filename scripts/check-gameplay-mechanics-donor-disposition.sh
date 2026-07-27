#!/usr/bin/env bash
set -euo pipefail

ENGINE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPOSITION="$ENGINE_ROOT/migration/gameplay-mechanics-donor/disposition.tsv"
META="$ENGINE_ROOT/migration/gameplay-mechanics-donor/source.meta"

for required in "$DISPOSITION" "$META"; do
  if [[ ! -f "$required" ]]; then
    echo "missing gameplay mechanics donor accounting: $required" >&2
    exit 1
  fi
done

expected_header=$'path\tdisposition\tsuccessor\tproof\tnotes'
if [[ "$(head -n 1 "$DISPOSITION")" != "$expected_header" ]]; then
  echo "invalid gameplay mechanics donor disposition header" >&2
  exit 1
fi

awk -F '\t' '
  NR == 1 { next }
  NF != 5 { printf "invalid donor disposition field count at line %d\n", NR > "/dev/stderr"; failed = 1 }
  $1 == "" || $3 == "" || $4 == "" || $5 == "" {
    printf "empty donor disposition field at line %d\n", NR > "/dev/stderr"; failed = 1
  }
  $2 != "adopted" && $2 != "adapted" && $2 != "excluded" {
    printf "invalid donor disposition at line %d: %s\n", NR, $2 > "/dev/stderr"; failed = 1
  }
  previous != "" && previous >= $1 {
    printf "donor paths are duplicate or not sorted at line %d\n", NR > "/dev/stderr"; failed = 1
  }
  { counts[$2] += 1; previous = $1 }
  END {
    for (kind in counts) {
      if (counts[kind] == 0) failed = 1
    }
    exit failed
  }
' "$DISPOSITION"

item_count="$(awk -F '\t' '$1 == "item_count" { print $2 }' "$META")"
actual_count="$(( $(wc -l < "$DISPOSITION") - 1 ))"
if [[ "$actual_count" != "$item_count" ]]; then
  echo "donor disposition count mismatch: expected=$item_count actual=$actual_count" >&2
  exit 1
fi

expected_path_sha="$(awk -F '\t' '$1 == "path_sha256" { print $2 }' "$META")"
actual_path_sha="$(tail -n +2 "$DISPOSITION" | cut -f 1 | sha256sum | awk '{print $1}')"
if [[ "$actual_path_sha" != "$expected_path_sha" ]]; then
  echo "donor disposition path hash mismatch" >&2
  exit 1
fi

for kind in adopted adapted excluded; do
  count="$(awk -F '\t' -v kind="$kind" 'NR > 1 && $2 == kind { count += 1 } END { print count + 0 }' "$DISPOSITION")"
  if (( count == 0 )); then
    echo "donor disposition has no $kind rows" >&2
    exit 1
  fi
done

echo "gameplay mechanics donor disposition passed: $actual_count items, no pending rows"
