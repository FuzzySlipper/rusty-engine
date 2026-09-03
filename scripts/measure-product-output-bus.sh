#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo test --release -p product-dev-host --lib \
  full_queue_publication_shares_retained_payloads_and_reports_throughput \
  --locked -- --nocapture
