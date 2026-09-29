#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The crate embeds this ignored bundle with include_str!, so build it first.
pnpm --dir "$REPO_ROOT/render" run typecheck
pnpm --dir "$REPO_ROOT/render" run build:webview-artifact

pnpm --dir "$REPO_ROOT/render" exec playwright test --config playwright.webview.config.ts

cargo test -p render-host-contracts -p renderer-webview-host --locked
cargo clippy -p render-host-contracts -p renderer-webview-host --all-targets --locked -- -D warnings

if [[ "$(uname -s)" == "Linux" ]]; then
  xvfb-run -a env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
    GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
    cargo run -p renderer-webview-host --example webview_smoke --locked
fi
