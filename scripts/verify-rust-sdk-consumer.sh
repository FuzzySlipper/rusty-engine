#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE_ROOT="$(mktemp -d -t rusty-engine-sdk-consumer.XXXXXX)"
trap 'rm -rf "$PROBE_ROOT"' EXIT
PROBE_CRATE="$PROBE_ROOT/consumer"

if [[ "$#" -gt 1 ]] || { [[ "$#" -eq 1 ]] && [[ ! "$1" =~ ^[0-9a-f]{40}$ ]]; }; then
  echo "usage: $0 [public-40-character-rusty-engine-revision]" >&2
  exit 2
fi

cargo new --quiet --bin --name rusty-engine-sdk-consumer-proof "$PROBE_CRATE"
if [[ "$#" -eq 1 ]]; then
  cargo add --quiet --manifest-path "$PROBE_CRATE/Cargo.toml" rusty-engine \
    --git https://github.com/FuzzySlipper/rusty-engine --rev "$1"
else
  cargo add --quiet --manifest-path "$PROBE_CRATE/Cargo.toml" rusty-engine \
    --path "$REPO_ROOT/rust/crates/rusty-engine"
fi
cp "$REPO_ROOT/fixtures/rust-sdk-consumer/main.rs" "$PROBE_CRATE/src/main.rs"

if [[ "$(sed -n '/^\[dependencies\]/,$p' "$PROBE_CRATE/Cargo.toml" | rg -c '^[a-zA-Z0-9_-]+[[:space:]]*=')" -ne 1 ]]; then
  echo "clean Rust SDK consumer must declare exactly one dependency" >&2
  exit 1
fi

CARGO_TARGET_DIR="$REPO_ROOT/target/rust-sdk-consumer" \
  cargo run --manifest-path "$PROBE_CRATE/Cargo.toml"
CARGO_TARGET_DIR="$REPO_ROOT/target/rust-sdk-consumer" \
  cargo check --manifest-path "$PROBE_CRATE/Cargo.toml" --locked

if find "$PROBE_ROOT" -name package.json -o -name pnpm-lock.yaml | grep -q .; then
  echo "clean Rust SDK consumer unexpectedly acquired a renderer package carrier" >&2
  exit 1
fi

if [[ "$#" -eq 1 ]] && ! rg -Fq "#$1" "$PROBE_CRATE/Cargo.lock"; then
  echo "clean Rust SDK consumer did not resolve the requested Engine revision" >&2
  exit 1
fi
