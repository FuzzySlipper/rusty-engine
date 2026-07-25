#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"

if [[ -z "$DEMO_ROOT" ]]; then
  echo "usage: $0 <absolute-rusty-engine-demo-root>" >&2
  exit 2
fi
if [[ "$DEMO_ROOT" != /* ]]; then
  echo "rusty-engine-demo root must be absolute: $DEMO_ROOT" >&2
  exit 2
fi
if [[ ! -f "$DEMO_ROOT/Cargo.toml" || ! -f "$DEMO_ROOT/AGENTS.md" ]]; then
  echo "not a rusty-engine-demo checkout: $DEMO_ROOT" >&2
  exit 1
fi

cd "$REPO_ROOT"
pnpm --dir studio --filter @rusty-engine/studio-editor-shell run build
cargo build --manifest-path "$DEMO_ROOT/Cargo.toml" --bin studio-adapter
node studio/test/integration/demo-adapter.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
