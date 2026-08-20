#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_ROOT="$(mktemp -d -t rusty-engine-ci-routing.XXXXXX)"
trap 'rm -rf "$PROBE_ROOT"' EXIT

mkdir -p "$PROBE_ROOT/.github" "$PROBE_ROOT/scripts" "$PROBE_ROOT/render/packages"
cp -a "$REPO_ROOT/.github/workflows" "$PROBE_ROOT/.github/workflows"
cp "$REPO_ROOT/scripts/verify-render.sh" "$PROBE_ROOT/scripts/verify-render.sh"
cp "$REPO_ROOT/scripts/verify-render-artifacts.sh" "$PROBE_ROOT/scripts/verify-render-artifacts.sh"
cp "$REPO_ROOT/render/package.json" "$PROBE_ROOT/render/package.json"
for package in application-host render-contracts render-projection renderer-host renderer-three; do
  mkdir -p "$PROBE_ROOT/render/packages/$package"
  cp "$REPO_ROOT/render/packages/$package/package.json" "$PROBE_ROOT/render/packages/$package/package.json"
done

python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$PROBE_ROOT" >/dev/null

expect_rejection() {
  local label="$1"
  if python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$PROBE_ROOT" >/dev/null 2>&1; then
    echo "ci routing checker accepted negative probe: $label" >&2
    exit 1
  fi
}

cp -a "$PROBE_ROOT" "$PROBE_ROOT.missing-host"
sed -i "/rust\/crates\/renderer-webview-host/d" "$PROBE_ROOT.missing-host/.github/workflows/render.yml"
PROBE_ROOT="$PROBE_ROOT.missing-host" expect_rejection "missing renderer webview owner"
rm -rf "$PROBE_ROOT.missing-host"

cp -a "$PROBE_ROOT" "$PROBE_ROOT.broad-studio"
sed -i "s#render/packages/renderer-three/\\*\\*#render/**#g" "$PROBE_ROOT.broad-studio/.github/workflows/studio.yml"
PROBE_ROOT="$PROBE_ROOT.broad-studio" expect_rejection "broad Studio renderer routing"
rm -rf "$PROBE_ROOT.broad-studio"

cp -a "$PROBE_ROOT" "$PROBE_ROOT.studio-render-fixture"
sed -i "/render\/packages\/renderer-three\/\\*\\*/a\\      - 'fixtures/render/**'" \
  "$PROBE_ROOT.studio-render-fixture/.github/workflows/studio.yml"
PROBE_ROOT="$PROBE_ROOT.studio-render-fixture" expect_rejection \
  "browser-only render fixture routed to Studio"
rm -rf "$PROBE_ROOT.studio-render-fixture"

cp -a "$PROBE_ROOT" "$PROBE_ROOT.no-cancel"
sed -i '/^concurrency:/,/^permissions:/d' "$PROBE_ROOT.no-cancel/.github/workflows/docs.yml"
PROBE_ROOT="$PROBE_ROOT.no-cancel" expect_rejection "missing superseded-run cancellation"
rm -rf "$PROBE_ROOT.no-cancel"

cp -a "$PROBE_ROOT" "$PROBE_ROOT.duplicate-build"
sed -i 's/ --artifacts-ready//' "$PROBE_ROOT.duplicate-build/scripts/verify-render.sh"
PROBE_ROOT="$PROBE_ROOT.duplicate-build" expect_rejection "aggregate webview rebuild"
rm -rf "$PROBE_ROOT.duplicate-build"

echo "ci routing checker negative probes passed"
