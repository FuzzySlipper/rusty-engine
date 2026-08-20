#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VOXEL_ROOT="${1:-${RUSTY_ENGINE_VOXELS_ROOT:-}}"

if [[ -z "$VOXEL_ROOT" || "$VOXEL_ROOT" != /* ]]; then
  echo "usage: $0 <absolute-rusty-engine-voxels-root>" >&2
  exit 2
fi
if [[ ! -f "$VOXEL_ROOT/Cargo.toml" || ! -f "$VOXEL_ROOT/AGENTS.md" || ! -f "$VOXEL_ROOT/.rusty-studio.json" ]]; then
  echo "checkout is missing generic Studio launch inputs: $VOXEL_ROOT" >&2
  exit 1
fi

STATIC_ROOT="$REPO_ROOT/studio/dist/apps/studio-app/browser"
cd "$REPO_ROOT"
pnpm --dir studio run build >/dev/null
if [[ ! -f "$STATIC_ROOT/index.html" ]]; then
  echo "Studio build did not produce the browser shell: $STATIC_ROOT/index.html" >&2
  exit 1
fi

SETTINGS_ROOT="$(mktemp -d /tmp/rusty-engine-studio-generic-settings.XXXXXX)"
cleanup() { rm -rf -- "$SETTINGS_ROOT"; }
trap cleanup EXIT

RUSTY_STUDIO_GENERIC_VOXEL_ROOT="$(realpath "$VOXEL_ROOT")" \
RUSTY_STUDIO_GENERIC_SETTINGS_ROOT="$SETTINGS_ROOT" \
pnpm --dir studio exec playwright test --config generic-playwright.config.ts
