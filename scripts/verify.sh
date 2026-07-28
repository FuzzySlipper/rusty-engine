#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo fmt --all --check
./scripts/audit-standalone.sh
./scripts/audit-render-isolation.sh
./scripts/audit-studio-isolation.sh
./scripts/check-doc-links.sh
./scripts/check-asha-equivalence.sh --final
./scripts/check-gameplay-mechanics-donor-disposition.sh
./scripts/check-gameplay-rules-donor-disposition.sh
./scripts/test-asha-equivalence-checker.sh
./scripts/check-render-completeness.sh --strict
./scripts/test-render-completeness-checker.sh
if rg -n 'GameplayRuntimeHost|GameplayFabric|NativeRuntimeBridge|RuntimeSession|ReactionFrame|DecisionReceipt|ReplayRecord|ProposalEnvelope' rust; then
  echo "forbidden old runtime spine surfaced in active source" >&2
  exit 1
fi
cargo metadata --format-version 1 --locked --no-deps > /dev/null
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
