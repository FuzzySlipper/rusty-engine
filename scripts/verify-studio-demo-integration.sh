#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"

if [[ -z "$DEMO_ROOT" || "$DEMO_ROOT" != /* ]]; then
  echo "usage: $0 <absolute-rusty-engine-demo-root>" >&2
  exit 2
fi
if [[ ! -f "$DEMO_ROOT/Cargo.toml" || ! -f "$DEMO_ROOT/.rusty-studio.json" ]]; then
  echo "checkout is missing Rusty Studio launch inputs: $DEMO_ROOT" >&2
  exit 1
fi

DEMO_ROOT="$(realpath "$DEMO_ROOT")"
cd "$REPO_ROOT"
pnpm --dir studio run check:boundaries
pnpm --dir studio run build
cargo build --locked --manifest-path "$DEMO_ROOT/Cargo.toml" --bin studio-adapter
node studio/test/integration/demo-adapter.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
node studio/test/integration/demo-voxel-objects.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
./scripts/verify-studio-browser-integration.sh "$DEMO_ROOT"

echo "Engine-hosted Studio verified the selected demo project through its Rust adapter"
