#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${RUSTY_RENDER_DEPS_READY:-0}" != "1" ]]; then
  pnpm --dir "$REPO_ROOT/render" install --frozen-lockfile --ignore-scripts
fi

pnpm --dir "$REPO_ROOT/render" run boundary
"$REPO_ROOT/scripts/verify-render-artifacts.sh"
pnpm --dir "$REPO_ROOT/render" run typecheck:browser
pnpm --dir "$REPO_ROOT/render" run test:compiled

if [[ -z "${PLAYWRIGHT_CHROMIUM_EXECUTABLE:-}" ]] && command -v chromium >/dev/null 2>&1; then
  PLAYWRIGHT_CHROMIUM_EXECUTABLE="$(command -v chromium)"
  export PLAYWRIGHT_CHROMIUM_EXECUTABLE
fi

pnpm --dir "$REPO_ROOT/render" run test:browser
