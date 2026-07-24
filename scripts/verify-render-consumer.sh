#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 || ! "$1" =~ ^[0-9a-f]{40}$ ]]; then
  echo "usage: $0 <public-40-character-rusty-engine-revision>" >&2
  exit 2
fi

ENGINE_REVISION="$1"
PROBE_ROOT="$(mktemp -d -t rusty-engine-render-consumer.XXXXXX)"
trap 'rm -rf "$PROBE_ROOT"' EXIT

cat > "$PROBE_ROOT/package.json" <<JSON
{
  "name": "rusty-engine-render-consumer-proof",
  "private": true,
  "type": "module",
  "packageManager": "pnpm@11.7.0",
  "dependencies": {
    "@rusty-engine/render-contracts": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/render-contracts",
    "@rusty-engine/render-projection": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/render-projection",
    "@rusty-engine/renderer-host": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/renderer-host",
    "@rusty-engine/renderer-three": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/renderer-three"
  }
}
JSON

cat > "$PROBE_ROOT/pnpm-workspace.yaml" <<YAML
packages:
  - "."

allowBuilds:
  "@rusty-engine/render-contracts@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:render/packages/render-contracts": true
  "@rusty-engine/render-projection@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:render/packages/render-projection": true
  "@rusty-engine/renderer-host@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:render/packages/renderer-host": true
  "@rusty-engine/renderer-three@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:render/packages/renderer-three": true
YAML

pnpm --dir "$PROBE_ROOT" install

if grep -En "(^|[[:space:]'\"])(workspace:|link:|file:)|/home/dev/" "$PROBE_ROOT/pnpm-lock.yaml"; then
  echo "clean renderer consumer resolved a local or workspace dependency" >&2
  exit 1
fi

for package_name in render-contracts render-projection renderer-host renderer-three; do
  if ! grep -Fq "${ENGINE_REVISION}#path:render/packages/${package_name}" "$PROBE_ROOT/pnpm-lock.yaml"; then
    echo "clean renderer consumer did not lock ${package_name} to ${ENGINE_REVISION}" >&2
    exit 1
  fi
done

pnpm --dir "$PROBE_ROOT" exec node --input-type=module <<'JS'
import { decodeRenderFrameDiff } from '@rusty-engine/render-contracts';
import { RenderProjection } from '@rusty-engine/render-projection';
import { createRendererDefaultSurfaceFrame } from '@rusty-engine/renderer-host';
import { ThreeRenderer } from '@rusty-engine/renderer-three';

const frame = decodeRenderFrameDiff(createRendererDefaultSurfaceFrame());
const projection = new RenderProjection();
const instructions = projection.applyFrame(frame);
const renderer = new ThreeRenderer();
renderer.applyFrame(frame);
const snapshot = renderer.snapshot();
renderer.dispose();

if (instructions.length === 0 || projection.handleCount === 0 || snapshot.length === 0) {
  throw new Error('exact-revision renderer packages did not execute one coherent retained frame');
}
console.log('EXACT_PUBLIC_RENDER_CONSUMER_OK');
JS
