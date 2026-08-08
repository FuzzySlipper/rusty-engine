#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT="$REPO_ROOT/rust/crates/renderer-webview-host/artifacts/renderer-webview.js"
ARTIFACT_COPY="$(mktemp -t rusty-renderer-webview-artifact.XXXXXX.js)"
trap 'rm -f "$ARTIFACT_COPY"' EXIT

cp "$ARTIFACT" "$ARTIFACT_COPY"
pnpm --dir "$REPO_ROOT/render" run typecheck
pnpm --dir "$REPO_ROOT/render" run build:webview-artifact
if ! cmp -s "$ARTIFACT_COPY" "$ARTIFACT"; then
  echo "checked renderer webview artifact does not match its reproducible build" >&2
  exit 1
fi

cargo test -p render-host-contracts -p renderer-webview-host --locked
cargo clippy -p render-host-contracts -p renderer-webview-host --all-targets --locked -- -D warnings

if [[ "$(uname -s)" == "Linux" ]]; then
  xvfb-run -a env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
    GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
    cargo run -p renderer-webview-host --example webview_smoke --locked
fi
