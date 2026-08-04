#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${1:-${RUSTY_ENGINE_DEMO_ROOT:-}}"
VOXEL_ROOT="${2:-${RUSTY_ENGINE_VOXELS_ROOT:-}}"

if [[ -z "$DEMO_ROOT" || -z "$VOXEL_ROOT" || "$DEMO_ROOT" != /* || "$VOXEL_ROOT" != /* ]]; then
  echo "usage: $0 <absolute-rusty-engine-demo-root> <absolute-rusty-engine-voxels-root>" >&2
  exit 2
fi
for root in "$DEMO_ROOT" "$VOXEL_ROOT"; do
  if [[ ! -f "$root/Cargo.toml" || ! -f "$root/AGENTS.md" || ! -f "$root/.rusty-studio.json" ]]; then
    echo "checkout is missing generic Studio launch inputs: $root" >&2
    exit 1
  fi
done

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

RUSTY_STUDIO_GENERIC_DEMO_ROOT="$(realpath "$DEMO_ROOT")" \
RUSTY_STUDIO_GENERIC_VOXEL_ROOT="$(realpath "$VOXEL_ROOT")" \
RUSTY_STUDIO_GENERIC_SETTINGS_ROOT="$SETTINGS_ROOT" \
pnpm --dir studio exec playwright test --config generic-playwright.config.ts
