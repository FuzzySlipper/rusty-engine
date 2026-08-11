#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

FORBIDDEN='@asha/|asha-engine|runtime[-_]bridge|runtime[-_]session|RuntimeBridge|RuntimeSession|NativeRuntimeBridge|GameplayFabric|ReplayRecord|ReactionFrame|DecisionReceipt|ProposalEnvelope|replay-certification|core[-_]catalog|project[-_]bundle|global[-_]codegen|generated[-_]tunnel|provider[-_]registry'

if rg -n -i "$FORBIDDEN" \
  rust/crates/render-model \
  rust/crates/render-projection \
  rust/crates/render-presentation \
  rust/crates/render-host-contracts \
  rust/crates/renderer-webview-host \
  render/packages \
  render/browser \
  render/product-playtest \
  render/private \
  --glob '*.rs' \
  --glob '*.ts' \
  --glob 'Cargo.toml' \
  --glob 'package.json'; then
  echo "forbidden historical runtime dependency surfaced in operational render source" >&2
  exit 1
fi

if find rust/crates/render-model rust/crates/render-projection rust/crates/render-presentation \
  rust/crates/render-host-contracts rust/crates/renderer-webview-host render \
  \( -path '*/node_modules' -o -path '*/dist' \) -prune -o -type l -print -quit | grep -q .; then
  echo "render subsystem contains a symlink and is not clone-isolated" >&2
  exit 1
fi

if rg -n 'pub fn (eval|evaluate|invoke|dispatch|import_module)|pub (struct|enum) .*JavaScript' \
  rust/crates/renderer-webview-host/src; then
  echo "renderer webview host exposed a generic JavaScript escape hatch" >&2
  exit 1
fi

if rg -n '@rusty-engine/|render/private|renderer-webview.js|package.json|pnpm' \
  fixtures/rust-sdk-consumer rust/crates/rusty-engine; then
  echo "Rust SDK consumer or facade knows the private renderer package topology" >&2
  exit 1
fi

echo "render isolation audit passed: no historical runtime dependency or external symlink"
