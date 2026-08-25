#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# These integration tests deliberately consume the isolated Rules build and
# checked renderer artifacts. verify-all prepares both owners before this gate.
if [[ ! -d rules/packages/runtime-composition-authoring/dist ]] \
  || [[ ! -d rules/node_modules/typescript ]] \
  || [[ ! -d render/node_modules/vite ]]; then
  echo "product materializer integration requires prepared Rules and renderer workspaces; run scripts/verify-rules.sh and scripts/verify-render.sh first" >&2
  exit 1
fi

cargo test -p product-materializer --locked --lib -- --ignored
cargo test -p product-materializer --locked --test product_assembly \
  materialized_product_assembles_relocates_and_serves_closed_browser_bundle -- --ignored
