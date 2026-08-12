#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo test --release -p engine-spatial --test character_controller \
  representative_character_controller_performance_budget --locked -- \
  --ignored --nocapture --exact
