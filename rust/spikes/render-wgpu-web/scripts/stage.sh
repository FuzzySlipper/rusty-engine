#!/usr/bin/env bash
# Build the wasm viewer and stage a static site that serves it with a capture.
#
#   stage.sh <capture dir> <site dir>
#
# WASM_BINDGEN must be the CLI matching the locked wasm-bindgen (0.2.127).
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
capture="$(cd "$1" && pwd)"
site="$2"
target="${CARGO_TARGET_DIR:-$here/target}"

(cd "$here" && cargo build --release --lib --target wasm32-unknown-unknown)
rm -rf "$site"
mkdir -p "$site"
"${WASM_BINDGEN:-wasm-bindgen}" --target web --no-typescript --out-dir "$site/pkg" \
  "$target/wasm32-unknown-unknown/release/render_wgpu_web.wasm"
cp "$here/web/index.html" "$here/web/main.js" "$here/web/shaders.html" "$site/"
# The WGSL sources as the renderer assembles them (video's is a Rust string).
src="$here/../../crates/render-wgpu/src"
mkdir -p "$site/shaders"
cp "$src"/*.wgsl "$site/shaders/"
python3 "$here/scripts/extract-video-shader.py" "$src/video.rs" "$site/shaders/video.wgsl"
mkdir -p "$site/capture"
ln -s "$capture/world-frame.json" "$capture/view.json" "$capture/resources" "$site/capture/"
python3 - "$capture/resources" "$site/capture/manifest.json" <<'EOF'
import json, os, sys
json.dump({"resources": sorted(os.listdir(sys.argv[1]))}, open(sys.argv[2], "w"))
EOF
ls -l "$site/pkg"
