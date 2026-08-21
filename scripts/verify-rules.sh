#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

pnpm --dir rules install --frozen-lockfile
pnpm --dir rules run verify
cargo test -p gameplay-rules --locked
cargo test -p gameplay-standard --locked
