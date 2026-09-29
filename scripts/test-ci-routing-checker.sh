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

cp -a "$PROBE_ROOT" "$PROBE_ROOT.missing-browser"
sed -i "/'render\/\*\*'/d" "$PROBE_ROOT.missing-browser/.github/workflows/browser.yml"
PROBE_ROOT="$PROBE_ROOT.missing-browser" expect_rejection "missing browser shell owner"
rm -rf "$PROBE_ROOT.missing-browser"

cp -a "$PROBE_ROOT" "$PROBE_ROOT.no-cancel"
sed -i '/^concurrency:/,/^permissions:/d' "$PROBE_ROOT.no-cancel/.github/workflows/docs.yml"
PROBE_ROOT="$PROBE_ROOT.no-cancel" expect_rejection "missing superseded-run cancellation"
rm -rf "$PROBE_ROOT.no-cancel"

cp -a "$PROBE_ROOT" "$PROBE_ROOT.missing-csharp"
sed -i '/scripts\/verify-csharp/d' "$PROBE_ROOT.missing-csharp/.github/workflows/csharp.yml"
PROBE_ROOT="$PROBE_ROOT.missing-csharp" expect_rejection "missing C# verification routing"
rm -rf "$PROBE_ROOT.missing-csharp"

echo "ci routing checker negative probes passed"
