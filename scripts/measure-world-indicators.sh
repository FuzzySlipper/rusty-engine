#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pnpm --dir render --filter @rusty-engine/render-contracts build
pnpm --dir render --filter @rusty-engine/renderer-host build
PLAYWRIGHT_RENDER_PORT="${PLAYWRIGHT_RENDER_PORT:-4185}" \
  pnpm --dir render exec playwright test \
    browser/world-indicator-performance.browser.spec.ts --reporter=line
