#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"
PIN_FILE="$REPO_ROOT/studio/demo-consumer-source.json"

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

CONSUMER_PIN_OUTPUT="$(node --input-type=module - "$PIN_FILE" <<'NODE'
import { readFileSync } from 'node:fs';

const pin = JSON.parse(readFileSync(process.argv[2], 'utf8'));
if (
  pin.schemaVersion !== 1
  || typeof pin.repository !== 'string'
  || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(pin.repository)
) {
  throw new Error('demo consumer pin has an unsupported shape');
}
if (typeof pin.commit !== 'string' || !/^[0-9a-f]{40}$/.test(pin.commit)) {
  throw new Error('demo consumer pin must contain one exact 40-character commit');
}
process.stdout.write(`${pin.repository}\n${pin.commit}\n`);
NODE
)"
mapfile -t CONSUMER_PIN <<< "$CONSUMER_PIN_OUTPUT"
EXPECTED_REPOSITORY="${CONSUMER_PIN[0]:-}"
EXPECTED_COMMIT="${CONSUMER_PIN[1]:-}"
DEMO_ROOT="$(realpath "$DEMO_ROOT")"
DEMO_TOP="$(git -C "$DEMO_ROOT" rev-parse --show-toplevel 2>/dev/null || true)"
if [[ "$DEMO_TOP" != "$DEMO_ROOT" ]]; then
  echo "rusty-engine-demo root must be an explicit checkout root: $DEMO_ROOT" >&2
  exit 1
fi
DEMO_COMMIT="$(git -C "$DEMO_ROOT" rev-parse HEAD)"
if [[ "$DEMO_COMMIT" != "$EXPECTED_COMMIT" ]]; then
  echo "rusty-engine-demo revision mismatch: expected $EXPECTED_REPOSITORY@$EXPECTED_COMMIT, found $DEMO_COMMIT" >&2
  exit 1
fi
DEMO_STATUS="$(git -C "$DEMO_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$DEMO_STATUS" ]]; then
  echo "rusty-engine-demo checkout must be clean, including non-ignored untracked inputs:" >&2
  echo "$DEMO_STATUS" >&2
  exit 1
fi

cd "$REPO_ROOT"
pnpm --dir studio --filter @rusty-engine/studio-editor-shell run build
cargo build --manifest-path "$DEMO_ROOT/Cargo.toml" --bin studio-adapter
node studio/test/integration/demo-adapter.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
node studio/test/integration/demo-voxel-objects.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
./scripts/verify-studio-browser-integration.sh "$DEMO_ROOT"

DEMO_STATUS="$(git -C "$DEMO_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$DEMO_STATUS" ]]; then
  echo "integration verification changed the reviewed consumer checkout:" >&2
  echo "$DEMO_STATUS" >&2
  exit 1
fi
