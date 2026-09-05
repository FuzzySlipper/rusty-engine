#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cargo fmt --all --check
./scripts/audit-standalone.sh
python3 ./scripts/dependency_boundary_check.py
PYTHONDONTWRITEBYTECODE=1 python3 ./scripts/test_architecture_checks.py
# The host-neutral runtime-session guard is current Engine infrastructure;
# its name is not evidence of the removed gameplay runtime spine.
if rg -n 'GameplayRuntimeHost|GameplayFabric|NativeRuntimeBridge|ReactionFrame|DecisionReceipt|ReplayRecord|ProposalEnvelope' rust; then
  echo "forbidden old runtime spine surfaced in active source" >&2
  exit 1
fi
cargo metadata --format-version 1 --locked --no-deps > /dev/null
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
