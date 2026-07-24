#!/usr/bin/env bash
set -euo pipefail

ENGINE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ENGINE_ROOT"

failed=0
while IFS=: read -r document line match; do
  target="${match#*](}"
  target="${target%)}"
  target="${target%%#*}"
  target="${target#<}"
  target="${target%>}"
  if [[ -z "$target" || "$target" == http://* || "$target" == https://* || "$target" == mailto:* ]]; then
    continue
  fi
  resolved="$(dirname "$document")/$target"
  if [[ ! -e "$resolved" ]]; then
    printf '%s:%s: missing local Markdown target %s\n' "$document" "$line" "$target" >&2
    failed=1
  fi
done < <(rg -n --no-heading -o '\]\([^)]+\)' README.md AGENTS.md docs)

if (( failed != 0 )); then
  exit 1
fi

echo "documentation links passed"
