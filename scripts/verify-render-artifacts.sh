#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPLICATION_ARTIFACT="$REPO_ROOT/render/artifacts/application-host"
WEBVIEW_ARTIFACT="$REPO_ROOT/rust/crates/renderer-webview-host/artifacts/renderer-webview.js"
ARTIFACT_SNAPSHOT="$(mktemp -d -t rusty-render-artifacts.XXXXXX)"
trap 'rm -rf "$ARTIFACT_SNAPSHOT"' EXIT

cp -a "$APPLICATION_ARTIFACT" "$ARTIFACT_SNAPSHOT/application-host"
cp "$WEBVIEW_ARTIFACT" "$ARTIFACT_SNAPSHOT/renderer-webview.js"

pnpm --dir "$REPO_ROOT/render" run build

ARTIFACT_DRIFT=0
if ! diff -ru "$ARTIFACT_SNAPSHOT/application-host" "$APPLICATION_ARTIFACT"; then
  echo "checked application-host artifact does not match its reproducible build" >&2
  ARTIFACT_DRIFT=1
fi
if ! cmp -s "$ARTIFACT_SNAPSHOT/renderer-webview.js" "$WEBVIEW_ARTIFACT"; then
  echo "checked renderer webview artifact does not match its reproducible build" >&2
  ARTIFACT_DRIFT=1
fi
if [[ "$ARTIFACT_DRIFT" -ne 0 ]]; then
  exit 1
fi

echo "renderer artifact freshness passed after one package build"
