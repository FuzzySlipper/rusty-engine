#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

./scripts/check-doc-links.sh
python3 ./scripts/check-ci-routing.py
./scripts/test-ci-routing-checker.sh
