#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_ROOT="$(mktemp -d -t rusty-engine-ci-routing.XXXXXX)"
trap 'rm -rf "$PROBE_ROOT"' EXIT

mkdir -p "$PROBE_ROOT/.github"
cp -a "$REPO_ROOT/.github/workflows" "$PROBE_ROOT/.github/workflows"

python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$PROBE_ROOT" >/dev/null

expect_rejection() {
  local label="$1"
  if python3 "$REPO_ROOT/scripts/check-ci-routing.py" --root "$PROBE_ROOT" >/dev/null 2>&1; then
    echo "ci routing checker accepted negative probe: $label" >&2
    exit 1
  fi
}

cp -a "$PROBE_ROOT" "$PROBE_ROOT.missing-host"
sed -i "/rust\/crates\/render-host-contracts/d" "$PROBE_ROOT.missing-host/.github/workflows/render.yml"
PROBE_ROOT="$PROBE_ROOT.missing-host" expect_rejection "missing render contract owner"
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

cp -a "$PROBE_ROOT" "$PROBE_ROOT.missing-csharp"
sed -i '/scripts\/verify-csharp/d' "$PROBE_ROOT.missing-csharp/.github/workflows/csharp.yml"
PROBE_ROOT="$PROBE_ROOT.missing-csharp" expect_rejection "missing C# verification routing"
rm -rf "$PROBE_ROOT.missing-csharp"

echo "ci routing checker negative probes passed"
