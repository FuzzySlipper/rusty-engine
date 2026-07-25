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
done < <(find . -path ./.git -prune -o -type l -print)

if rg -n -i '(\.\./(asha-engine|asha-studio|asha-testing|rusty-engine-demo)|/home/dev/(asha-engine|asha-studio|asha-testing|rusty-engine-demo)|(?:path|link|file)[[:space:]]*=[[:space:]]*"[^"]*(asha-engine|asha-studio|asha-testing|rusty-engine-demo))' \
  --glob '!docs/**' --glob '!studio/boundary-policy.json' --glob '!scripts/audit-standalone.sh' .; then
  echo "operational sibling-repository reference found" >&2
  failed=1
fi

if [[ -e .gitmodules ]]; then
  echo "unexpected Git submodule configuration found" >&2
  failed=1
fi

while IFS= read -r manifest; do
  if rg -n 'engine-inspector' "$manifest"; then
    echo "runtime crate depends on the read-only engine-inspector leaf" >&2
    failed=1
  fi
done < <(
  find rust/crates -mindepth 2 -maxdepth 2 -name Cargo.toml \
    ! -path 'rust/crates/engine-inspector/Cargo.toml' -print
)

if (( failed != 0 )); then
  exit 1
fi

cargo metadata --format-version 1 --locked --no-deps > /dev/null
echo "standalone audit passed: local Cargo paths and symlinks remain inside Rusty Engine"
