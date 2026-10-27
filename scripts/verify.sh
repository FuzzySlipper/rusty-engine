#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

METADATA_DIR="$(mktemp -d -t rusty-engine-verify-metadata.XXXXXX)"
trap 'rm -rf "$METADATA_DIR"' EXIT
METADATA_PATH="$METADATA_DIR/cargo-metadata.json"

cargo fmt --all --check
# The dependency checker and architecture tests share this resolved graph.
cargo metadata --format-version 1 --locked --all-features > "$METADATA_PATH"
./scripts/audit-standalone.sh
python3 ./scripts/dependency_boundary_check.py --metadata "$METADATA_PATH"
PYTHONDONTWRITEBYTECODE=1 python3 ./scripts/test_architecture_checks.py --metadata "$METADATA_PATH"
# CPU/native ownership tests, then software-renderer startup, supported draw,
# refusal and retained-state/capture checks. The full renderer image suite and
# ignored temporal sequences run on hardware through the GPU lane.
cargo test --workspace --exclude render-wgpu --locked
cargo test -p render-wgpu --lib --locked
cargo test -p render-wgpu --locked \
  --test driver --test scene_changes --test views --test inspection \
  --test device_lost --test renderer_settings --test labels --test taau \
  --test volumetric_clouds --test volumetric_fog --test water \
  --test ambient_occlusion --test image_effect
cargo clippy --workspace --all-targets --locked -- -D warnings
