#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cpu_model="$(lscpu | sed -n 's/^Model name:[[:space:]]*//p' | head -n 1)"
printf 'rigid_body_characterization_host date_utc=%s cpu=%q toolchain=%q\n' \
  "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  "$cpu_model" \
  "$(rustc --version)"

cargo test --release -p engine-spatial --test rigid_body \
  rigid_body_per_tick_release_characterization --locked -- \
  --ignored --nocapture --exact
