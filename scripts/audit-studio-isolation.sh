#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

required=(
  studio/package.json
  studio/pnpm-lock.yaml
  studio/pnpm-workspace.yaml
  studio/nx.json
  studio/donor-source.json
  studio/demo-consumer-source.json
  studio/donor-inventory.tsv
  studio/donor-surface-disposition.tsv
  studio/owner-adoption.tsv
)
for path in "${required[@]}"; do
  if [[ ! -f "$path" ]]; then
    echo "missing isolated Studio file: $path" >&2
    exit 1
  fi
done

if rg -n 'studio/(apps|libs)|rusty-engine-studio' Cargo.toml pnpm-lock.yaml; then
  echo "ordinary Engine workspaces must not include Studio" >&2
  exit 1
fi

if rg -n '(@asha/|\.\./asha-(engine|studio|testing)|/home/dev/asha-(engine|studio|testing))' \
  studio/package.json studio/pnpm-lock.yaml studio/pnpm-workspace.yaml studio/nx.json; then
  echo "Studio operational workspace contains an Asha or sibling-checkout dependency" >&2
  exit 1
fi

if find studio \
  \( -type d \( -name node_modules -o -name .nx -o -name dist -o -name coverage \) \) -prune -o \
  -type l -print -quit | grep -q .; then
  echo "Studio workspace contains a symbolic link" >&2
  exit 1
fi

tracked_cache=$(git ls-files studio | rg '(^|/)(node_modules|\.nx|dist|coverage|playwright-report|test-results)(/|$)' || true)
if [[ -n "$tracked_cache" ]]; then
  echo "Studio workspace tracks cache or generated output:" >&2
  echo "$tracked_cache" >&2
  exit 1
fi

echo "Studio isolation audit passed"
