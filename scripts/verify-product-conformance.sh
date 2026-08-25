#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# This integration gate deliberately consumes the isolated authoring and
# browser-host artifacts. Ordinary `cargo test -p rusty-cli` remains clean-safe
# and leaves these prepared-owner cases ignored.
if [[ ! -d rules/packages/runtime-composition-authoring/dist ]] \
  || [[ ! -d rules/node_modules/typescript ]] \
  || [[ ! -d render/node_modules/vite ]] \
  || [[ ! -f render/artifacts/application-host/index.js ]] \
  || [[ ! -f render/artifacts/product-browser-host/product-browser-host.js ]]; then
  echo "product conformance requires prepared Rules and Render owners; run scripts/verify-rules.sh and scripts/verify-render.sh first" >&2
  exit 1
fi

node --check render/scripts/rusty-cli-browser-test.mjs

echo "[prepared-cli] run artifact-dependent CLI coverage explicitly"
cargo test -p rusty-cli --locked --offline -- --ignored

SCRATCH_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/rusty-product-conformance.XXXXXX")"
PRODUCT_ROOT="$SCRATCH_ROOT/product"
cleanup() {
  rm -rf "$SCRATCH_ROOT"
}
trap cleanup EXIT
cp -a "$REPO_ROOT/fixtures/product-conformance/." "$PRODUCT_ROOT"
rm -rf "$PRODUCT_ROOT/generated"
mkdir "$PRODUCT_ROOT/generated"

rusty() {
  cargo run -p rusty-cli --locked --offline -- "$@"
}

expect_failure() {
  if "$@"; then
    echo "expected failure but command succeeded: $*" >&2
    exit 1
  fi
}

echo "[authoring] admit exact Runtime Composition, Product Kernel, and content closure"
rusty check --path "$PRODUCT_ROOT"

echo "[headless] build exact generated Assembly and inspect closed owners"
rusty build --path "$PRODUCT_ROOT"
rusty inspect all --path "$PRODUCT_ROOT"
ASSEMBLY="$PRODUCT_ROOT/generated/product-assembly/assembly.json"
cp "$ASSEMBLY" "$SCRATCH_ROOT/assembly.first.json"

echo "[headless] delete/regenerate has byte-identical Assembly receipt"
rm -rf "$PRODUCT_ROOT/generated"
rusty build --path "$PRODUCT_ROOT"
cmp "$SCRATCH_ROOT/assembly.first.json" "$ASSEMBLY"

echo "[content] missing and changed source bodies fail readback without publishing"
cp "$PRODUCT_ROOT/content/counter.json" "$SCRATCH_ROOT/content.counter.json"
mv "$PRODUCT_ROOT/content/counter.json" "$SCRATCH_ROOT/content.counter.missing.json"
expect_failure rusty check --path "$PRODUCT_ROOT"
mv "$SCRATCH_ROOT/content.counter.missing.json" "$PRODUCT_ROOT/content/counter.json"
cmp "$SCRATCH_ROOT/assembly.first.json" "$ASSEMBLY"
printf '\n' >> "$PRODUCT_ROOT/content/counter.json"
expect_failure rusty check --path "$PRODUCT_ROOT"
cp "$SCRATCH_ROOT/content.counter.json" "$PRODUCT_ROOT/content/counter.json"
cmp "$SCRATCH_ROOT/assembly.first.json" "$ASSEMBLY"

echo "[content] a declared-root extra body is either rejected or changes the exact closure"
cp "$PRODUCT_ROOT/content/counter.json" "$PRODUCT_ROOT/content/extra.json"
if rusty check --path "$PRODUCT_ROOT"; then
  rusty build --path "$PRODUCT_ROOT"
  if cmp -s "$SCRATCH_ROOT/assembly.first.json" "$ASSEMBLY"; then
    echo "declared content-root extra body was accepted without changing closure" >&2
    exit 1
  fi
else
  cmp "$SCRATCH_ROOT/assembly.first.json" "$ASSEMBLY"
fi
rm -f "$PRODUCT_ROOT/content/extra.json"
rusty build --path "$PRODUCT_ROOT"

echo "[package] verify the generated package closure (no installed desktop action)"
rusty package --path "$PRODUCT_ROOT" --wrapper desktop

echo "[browser] run one canvas plus UI-click and physical-W intent convergence"
sed -i '/^\[\[wrappers\]\]/,$d' "$PRODUCT_ROOT/rusty.toml"
rusty test --path "$PRODUCT_ROOT"

if [[ "${RUSTY_PRODUCT_CONFORMANCE_DESKTOP:-0}" == "1" ]]; then
  echo "[packaged-host] explicit installed Tauri proof requested"
  cp "$REPO_ROOT/fixtures/product-conformance/rusty.toml" "$PRODUCT_ROOT/rusty.toml"
  rusty test --path "$PRODUCT_ROOT" --wrapper desktop
else
  echo "[packaged-host] skipped; set RUSTY_PRODUCT_CONFORMANCE_DESKTOP=1 with Tauri/WebDriver prerequisites"
fi
