#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

METADATA_DIR="$(mktemp -d -t rusty-engine-verify-metadata.XXXXXX)"
trap 'rm -rf "$METADATA_DIR"' EXIT
METADATA_PATH="$METADATA_DIR/cargo-metadata.json"

cargo fmt --all --check
# The dependency checker and architecture tests share this resolved graph.
cargo metadata --format-version 1 --locked --all-features > "$METADATA_PATH"
./scripts/audit-standalone.sh
python3 ./scripts/dependency_boundary_check.py --metadata "$METADATA_PATH"
PYTHONDONTWRITEBYTECODE=1 python3 ./scripts/test_architecture_checks.py --metadata "$METADATA_PATH"
# Every test target runs by default, including new renderer targets. Individual
# hardware-only tests stay visibly ignored here and run through the GPU lane.
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
