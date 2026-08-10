#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VOXEL_ROOT="${1:-${RUSTY_ENGINE_VOXEL_CONSUMER_ROOT:-}}"

if [[ -z "$VOXEL_ROOT" || "$VOXEL_ROOT" != /* ]]; then
  echo "usage: $0 <absolute-rusty-engine-voxels-root>" >&2
  exit 2
fi
if [[ ! -f "$VOXEL_ROOT/Cargo.toml" || ! -f "$VOXEL_ROOT/.rusty-studio.json" ]]; then
  echo "checkout is missing Rusty Studio launch inputs: $VOXEL_ROOT" >&2
  exit 1
fi

VOXEL_ROOT="$(realpath "$VOXEL_ROOT")"
PROJECT_FILE="content/projects/voxel-lab.project.json"
LARGE_PROJECT_FILE="content/projects/retro-character-high-fidelity.project.json"
RUNTIME_REPORT="evidence/initial-animated-voxel-report.json"
ADAPTER_BINARY="rusty-engine-voxels-studio-adapter"

STUDIO_TEST_ROOT="$(mktemp -d /tmp/rusty-engine-voxel-browser.XXXXXX)"
STUDIO_SETTINGS_ROOT="$(mktemp -d /tmp/rusty-engine-voxel-settings.XXXXXX)"
cleanup() {
  rm -rf -- "$STUDIO_TEST_ROOT" "$STUDIO_SETTINGS_ROOT"
}
trap cleanup EXIT
cp -a "$VOXEL_ROOT/content" "$STUDIO_TEST_ROOT/content"
cp -a "$VOXEL_ROOT/evidence" "$STUDIO_TEST_ROOT/evidence"

cd "$REPO_ROOT"
pnpm --dir studio run build
cargo build --locked --manifest-path "$VOXEL_ROOT/Cargo.toml" --bin "$ADAPTER_BINARY"

RUSTY_STUDIO_ADAPTER_BINARY="$VOXEL_ROOT/target/debug/$ADAPTER_BINARY" \
RUSTY_STUDIO_PROJECT_ROOT="$STUDIO_TEST_ROOT" \
RUSTY_STUDIO_PROJECT_FILE="$PROJECT_FILE" \
RUSTY_STUDIO_LARGE_PROJECT_FILE="$LARGE_PROJECT_FILE" \
RUSTY_STUDIO_RUNTIME_REPORT="$RUNTIME_REPORT" \
RUSTY_STUDIO_SETTINGS_ROOT="$STUDIO_SETTINGS_ROOT" \
pnpm --dir studio exec playwright test --config voxel-consumer.playwright.config.ts

RUSTY_STUDIO_ADAPTER_BINARY="$VOXEL_ROOT/target/debug/$ADAPTER_BINARY" \
RUSTY_STUDIO_PROJECT_ROOT="$STUDIO_TEST_ROOT" \
RUSTY_STUDIO_PROJECT_FILE="$PROJECT_FILE" \
RUSTY_STUDIO_LARGE_PROJECT_FILE="$LARGE_PROJECT_FILE" \
RUSTY_STUDIO_RUNTIME_REPORT="$RUNTIME_REPORT" \
RUSTY_STUDIO_SETTINGS_ROOT="$STUDIO_SETTINGS_ROOT" \
RUSTY_STUDIO_EXPECT_PREAUTHORED_SURFACE=1 \
pnpm --dir studio exec playwright test \
  --config voxel-consumer.playwright.config.ts \
  --grep "fresh Studio host reopens"

echo "Engine-hosted Studio verified the selected voxel project through its Rust adapter"
