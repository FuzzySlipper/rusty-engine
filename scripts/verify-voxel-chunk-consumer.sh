#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]] || [[ "$1" != /* ]]; then
  echo "usage: $0 /absolute/path/to/rusty-craftsurvive" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_root="$1"
consumer_probe="$consumer_root/scripts/edit-performance.mjs"

if [[ ! -f "$consumer_root/Cargo.toml" ]] || [[ ! -f "$consumer_root/package.json" ]] || \
  [[ ! -f "$consumer_probe" ]]; then
  echo "selected consumer is missing Cargo.toml, package.json, or scripts/edit-performance.mjs: $consumer_root" >&2
  exit 2
fi

expected_path="$(realpath "$repo_root/rust/crates/rusty-engine")"
declared_path="$(cd "$consumer_root" && realpath ../rusty-engine/rust/crates/rusty-engine)"
if [[ "$declared_path" != "$expected_path" ]] || \
  ! rg -q '^rusty-engine = \{ path = "\.\./rusty-engine/rust/crates/rusty-engine" \}$' \
    "$consumer_root/Cargo.toml"; then
  echo "consumer must use the adjacent complete rusty-engine facade at $expected_path" >&2
  exit 1
fi

(cd "$consumer_root" && pnpm perf:edit)
