#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"

if [[ -z "$DEMO_ROOT" || "$DEMO_ROOT" != /* ]]; then
  echo "usage: $0 <absolute-rusty-engine-demo-root>" >&2
  exit 2
fi
if [[ ! -f "$DEMO_ROOT/Cargo.toml" || ! -f "$DEMO_ROOT/apps/loading-bay-studio/project.json" ]]; then
  echo "not a Loading Bay Studio consumer checkout: $DEMO_ROOT" >&2
  exit 1
fi

DEMO_ROOT="$(realpath "$DEMO_ROOT")"
ADAPTER_BINARY="$DEMO_ROOT/target/debug/studio-adapter"
if [[ ! -x "$ADAPTER_BINARY" ]]; then
  echo "the exact-pinned integration gate must build $ADAPTER_BINARY before browser proof" >&2
  exit 1
fi

CONSUMER_TEST_ROOT="$(mktemp -d /tmp/rusty-engine-entity-inspector-browser.XXXXXX)"
CONSUMER_SETTINGS_ROOT="$(mktemp -d /tmp/rusty-engine-entity-inspector-settings.XXXXXX)"
CONSUMER_EVIDENCE_FILE="$(mktemp /tmp/rusty-engine-entity-inspector-evidence.XXXXXX)"
cleanup() {
  rm -rf -- "$CONSUMER_TEST_ROOT"
  rm -rf -- "$CONSUMER_SETTINGS_ROOT"
  rm -f -- "$CONSUMER_EVIDENCE_FILE"
}
trap cleanup EXIT

cp -a "$DEMO_ROOT/content" "$CONSUMER_TEST_ROOT/content"
cp -a "$DEMO_ROOT/fixtures" "$CONSUMER_TEST_ROOT/fixtures"

pnpm --dir "$DEMO_ROOT" install --frozen-lockfile
pnpm --dir "$DEMO_ROOT" run test:studio
pnpm --dir "$DEMO_ROOT" run build:studio

CONSUMER_STATIC_ROOT="$DEMO_ROOT/dist/apps/loading-bay-studio/browser"
if [[ ! -f "$CONSUMER_STATIC_ROOT/index.html" ]]; then
  echo "Loading Bay Studio build did not produce $CONSUMER_STATIC_ROOT/index.html" >&2
  exit 1
fi

run_consumer_browser() {
  RUSTY_STUDIO_ADAPTER_BINARY="$ADAPTER_BINARY" \
  RUSTY_STUDIO_CONSUMER_STATIC_ROOT="$CONSUMER_STATIC_ROOT" \
  RUSTY_STUDIO_PROJECT_ROOT="$CONSUMER_TEST_ROOT" \
  RUSTY_STUDIO_SETTINGS_ROOT="$CONSUMER_SETTINGS_ROOT" \
  RUSTY_STUDIO_ENTITY_INSPECTOR_EVIDENCE="$CONSUMER_EVIDENCE_FILE" \
  pnpm --dir "$REPO_ROOT/studio" exec playwright test \
    --config entity-inspector-consumer.playwright.config.ts "$@"
}

# Each invocation starts a new host and project-owned Rust adapter. The first
# proves the Engine-owned contribution inside the downstream composition, the
# second proves fallback plus the product mutation, and the third reconstructs
# that mutation from durable bytes in a fresh process.
run_consumer_browser --grep 'animated voxel objects convert'
run_consumer_browser --grep 'unknown identity fallback|real Loading Bay Weapon mutation'
run_consumer_browser --grep 'fresh adapter process preserves'
