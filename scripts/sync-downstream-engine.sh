#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]] || [[ ! -f "$1" ]]; then
  echo "usage: $0 /absolute/path/to/downstream/Cargo.toml" >&2
  exit 2
fi

MANIFEST_PATH="$(realpath "$1")"
cargo update --manifest-path "$MANIFEST_PATH" -p rusty-engine
python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check_downstream_engine_freshness.py" \
  --manifest "$MANIFEST_PATH" "$(dirname "$MANIFEST_PATH")/Cargo.lock"
