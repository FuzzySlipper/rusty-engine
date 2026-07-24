#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo fmt --all --check
if rg -n 'GameplayRuntimeHost|GameplayFabric|NativeRuntimeBridge|RuntimeSession|ReactionFrame|DecisionReceipt|ReplayRecord|ProposalEnvelope' rust ts/packages/browser-shell/src ts/packages/project-content/src; then
  echo "forbidden old runtime spine surfaced in active source" >&2
  exit 1
fi
pnpm run typecheck
pnpm run check:content
pnpm run test:ts
pnpm run test:shell
pnpm run build:shell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm run test:browser
