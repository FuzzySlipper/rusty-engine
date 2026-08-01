#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"

if [[ -z "$DEMO_ROOT" || "$DEMO_ROOT" != /* ]]; then
  echo "usage: $0 <absolute-rusty-engine-demo-root>" >&2
  exit 2
fi
if [[ ! -f "$DEMO_ROOT/Cargo.toml" || ! -f "$DEMO_ROOT/AGENTS.md" ]]; then
  echo "not a rusty-engine-demo checkout: $DEMO_ROOT" >&2
  exit 1
fi

STUDIO_STATIC_ROOT="$REPO_ROOT/studio/dist/apps/studio-app/browser"
ADAPTER_BINARY="$DEMO_ROOT/target/debug/studio-adapter"
if [[ ! -f "$STUDIO_STATIC_ROOT/index.html" ]]; then
  echo "the parent integration gate must build Studio before browser proof: $STUDIO_STATIC_ROOT/index.html" >&2
  exit 1
fi
if [[ ! -x "$ADAPTER_BINARY" ]]; then
  echo "the parent integration gate must build the exact consumer adapter before browser proof: $ADAPTER_BINARY" >&2
  exit 1
fi

STUDIO_TEST_ROOT="$(mktemp -d /tmp/rusty-engine-studio-browser.XXXXXX)"
STUDIO_SETTINGS_ROOT="$(mktemp -d /tmp/rusty-engine-studio-settings.XXXXXX)"
cleanup() {
  rm -rf -- "$STUDIO_TEST_ROOT"
  rm -rf -- "$STUDIO_SETTINGS_ROOT"
}
trap cleanup EXIT

cp -a "$DEMO_ROOT/content" "$STUDIO_TEST_ROOT/content"
cp -a "$DEMO_ROOT/fixtures" "$STUDIO_TEST_ROOT/fixtures"

cd "$REPO_ROOT"
RUSTY_STUDIO_ADAPTER_BINARY="$ADAPTER_BINARY" \
RUSTY_STUDIO_PROJECT_ROOT="$STUDIO_TEST_ROOT" \
RUSTY_STUDIO_SETTINGS_ROOT="$STUDIO_SETTINGS_ROOT" \
pnpm --dir studio run test:browser-integration
