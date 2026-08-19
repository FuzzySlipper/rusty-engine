#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

./scripts/check-doc-links.sh
python3 ./scripts/code_map_freshness.py
PYTHONDONTWRITEBYTECODE=1 python3 ./scripts/test_architecture_checks.py
python3 ./scripts/check-ci-routing.py
./scripts/test-ci-routing-checker.sh
