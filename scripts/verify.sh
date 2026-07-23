#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo fmt --all --check
cargo test --workspace
pnpm run build:native
pnpm run typecheck
pnpm run test:ts
