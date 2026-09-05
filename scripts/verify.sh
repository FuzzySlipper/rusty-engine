#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo fmt --all --check
./scripts/audit-standalone.sh
python3 ./scripts/dependency_boundary_check.py
PYTHONDONTWRITEBYTECODE=1 python3 ./scripts/test_architecture_checks.py
cargo metadata --format-version 1 --locked --no-deps > /dev/null
# The optional native webview has its own explicit verification script.
# Packaged products use the browser host and do not require GTK/WebKit.
cargo test --workspace --exclude renderer-webview-host --locked
cargo clippy --workspace --exclude renderer-webview-host --all-targets --locked -- -D warnings
