#!/usr/bin/env bash
# Rewrites render/packages/*/src/generated/contracts.ts from the Rust wire
# types (#8793). `cargo test` fails while one of them is stale.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo test --quiet -p product-host --lib \
  typescript::write_typescript_contracts -- --ignored --exact
