#!/usr/bin/env bash
set -euo pipefail

ENGINE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ENGINE_ROOT"

failed=0

while IFS=: read -r manifest _ path_declaration; do
  dependency_path="$(sed -E 's/.*path[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/' <<< "$path_declaration")"
  resolved="$(realpath -m "$(dirname "$manifest")/$dependency_path")"
  case "$resolved" in
    "$ENGINE_ROOT"/*) ;;
    *)
      printf 'outside local Cargo dependency: %s -> %s\n' "$manifest" "$resolved" >&2
      failed=1
      ;;
  esac
done < <(rg -n --no-heading -o 'path[[:space:]]*=[[:space:]]*"[^"]+"' --glob Cargo.toml)

while IFS= read -r link; do
  resolved="$(readlink -f "$link" || true)"
  case "$resolved" in
    "$ENGINE_ROOT"/*) ;;
    *)
      printf 'outside repository symlink: %s -> %s\n' "$link" "$resolved" >&2
      failed=1
      ;;
  esac
done < <(git ls-files --stage | awk '$1 == "120000" { print $4 }')

if (( failed != 0 )); then
  exit 1
fi

cargo metadata --format-version 1 --locked --no-deps > /dev/null
echo "standalone audit passed: local Cargo paths and tracked symlinks remain inside Rusty Engine"
