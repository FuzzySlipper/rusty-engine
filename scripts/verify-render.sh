#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$REPO_ROOT/scripts/audit-render-isolation.sh"
"$REPO_ROOT/scripts/check-render-completeness.sh" --strict
"$REPO_ROOT/scripts/test-render-completeness-checker.sh"

pnpm --dir "$REPO_ROOT/render" install --frozen-lockfile

if [[ -z "${PLAYWRIGHT_CHROMIUM_EXECUTABLE:-}" ]] && command -v chromium >/dev/null 2>&1; then
  PLAYWRIGHT_CHROMIUM_EXECUTABLE="$(command -v chromium)"
  export PLAYWRIGHT_CHROMIUM_EXECUTABLE
fi

pnpm --dir "$REPO_ROOT/render" run verify
