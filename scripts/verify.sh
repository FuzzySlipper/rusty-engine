#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo fmt --all --check
pnpm run typecheck
pnpm run check:content
pnpm run test:ts
pnpm run test:shell
pnpm run build:shell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm run test:browser
