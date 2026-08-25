#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

./scripts/verify.sh
./scripts/verify-rules.sh
./scripts/verify-render.sh
./scripts/verify-product-materializer.sh
./scripts/verify-product-conformance.sh
./scripts/verify-studio.sh
