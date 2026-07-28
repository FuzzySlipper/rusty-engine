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
  || pin.repository !== 'FuzzySlipper/rusty-engine-demo'
  || pin.publicRepository !== 'https://github.com/FuzzySlipper/rusty-engine-demo'
) {
  throw new Error('demo consumer pin has an unsupported repository identity');
}
if (typeof pin.commit !== 'string' || !/^[0-9a-f]{40}$/.test(pin.commit)) {
  throw new Error('demo consumer pin must contain one exact 40-character commit');
}
if (typeof pin.engineCommit !== 'string' || !/^[0-9a-f]{40}$/.test(pin.engineCommit)) {
  throw new Error('demo consumer pin must contain one exact Engine commit');
}
if (
  pin.projectFile !== 'content/projects/loading-bay.project.json'
  || pin.voxelProjectFile !== 'content/projects/converted-wall.project.json'
  || pin.cargoPackage !== 'loading-bay-game'
  || pin.adapterBinary !== 'studio-adapter'
  || pin.studioApplication !== 'apps/loading-bay-studio'
  || pin.entityInspectorConsumer?.componentTypeId !== 'rusty-engine-demo.loading-bay.weapon'
  || pin.entityInspectorConsumer?.contractId !== 'rusty-engine-demo.loading-bay.weapon-authoring'
  || pin.entityInspectorConsumer?.contractVersion !== 1
) {
  throw new Error('demo consumer pin has an unsupported Entity inspector target');
}
process.stdout.write(`${pin.repository}\n${pin.commit}\n${pin.engineCommit}\n`);
NODE
)"
mapfile -t CONSUMER_PIN <<< "$CONSUMER_PIN_OUTPUT"
EXPECTED_REPOSITORY="${CONSUMER_PIN[0]:-}"
EXPECTED_COMMIT="${CONSUMER_PIN[1]:-}"
EXPECTED_ENGINE_COMMIT="${CONSUMER_PIN[2]:-}"
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

node --input-type=module - "$DEMO_ROOT/package.json" "$EXPECTED_ENGINE_COMMIT" <<'NODE'
import { readFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const engineCommit = process.argv[3];
const packages = new Map([
  ['@rusty-engine/render-contracts', 'render/packages/render-contracts'],
  ['@rusty-engine/render-projection', 'render/packages/render-projection'],
  ['@rusty-engine/renderer-host', 'render/packages/renderer-host'],
  ['@rusty-engine/renderer-three', 'render/packages/renderer-three'],
  ['@rusty-engine/studio-adapter-client', 'studio/libs/adapter-client'],
  ['@rusty-engine/studio-editor-shell', 'studio/libs/editor-shell'],
  ['@rusty-engine/studio-user-settings', 'studio/libs/user-settings'],
  ['@rusty-engine/studio-viewport', 'studio/libs/viewport'],
  ['@rusty-engine/studio-voxel-editor', 'studio/libs/voxel-editor'],
]);
for (const [name, path] of packages) {
  const expected = `github:FuzzySlipper/rusty-engine#${engineCommit}&path:${path}`;
  if (manifest.dependencies?.[name] !== expected) {
    throw new Error(`${name} must use the reviewed exact Engine revision ${expected}`);
  }
}
NODE

cd "$REPO_ROOT"
pnpm --dir studio run check:boundaries
pnpm --dir studio --filter @rusty-engine/studio-editor-shell run build
cargo build --locked --manifest-path "$DEMO_ROOT/Cargo.toml" --bin studio-adapter
node studio/test/integration/demo-adapter.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
node studio/test/integration/demo-voxel-objects.mjs \
  --demo-root "$DEMO_ROOT" \
  --adapter-binary "$DEMO_ROOT/target/debug/studio-adapter"
./scripts/verify-studio-browser-integration.sh "$DEMO_ROOT"
./scripts/verify-studio-entity-inspector-integration.sh "$DEMO_ROOT"

DEMO_STATUS="$(git -C "$DEMO_ROOT" status --porcelain=v1 --untracked-files=all)"
if [[ -n "$DEMO_STATUS" ]]; then
  echo "integration verification changed the reviewed consumer checkout:" >&2
  echo "$DEMO_STATUS" >&2
  exit 1
fi
