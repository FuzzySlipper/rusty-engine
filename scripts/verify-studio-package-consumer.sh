#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 || ! "$1" =~ ^[0-9a-f]{40}$ ]]; then
  echo "usage: $0 <public-40-character-rusty-engine-revision>" >&2
  exit 2
fi

ENGINE_REVISION="$1"
PROBE_ROOT="$(mktemp -d -t rusty-engine-studio-consumer.XXXXXX)"
trap 'rm -rf "$PROBE_ROOT"' EXIT

cat > "$PROBE_ROOT/package.json" <<JSON
{
  "name": "rusty-engine-studio-consumer-proof",
  "private": true,
  "type": "module",
  "packageManager": "pnpm@11.7.0",
  "dependencies": {
    "@angular/common": "~21.2.0",
    "@angular/core": "~21.2.0",
    "@angular/forms": "~21.2.0",
    "@rusty-engine/render-contracts": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/render-contracts",
    "@rusty-engine/render-projection": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/render-projection",
    "@rusty-engine/renderer-host": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/renderer-host",
    "@rusty-engine/renderer-three": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/renderer-three",
    "@rusty-engine/studio-adapter-client": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/adapter-client",
    "@rusty-engine/studio-editor-shell": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/editor-shell",
    "@rusty-engine/studio-user-settings": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/user-settings",
    "@rusty-engine/studio-viewport": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/viewport",
    "@rusty-engine/studio-voxel-editor": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/voxel-editor",
    "typescript": "~5.9.2"
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
  "@rusty-engine/studio-adapter-client@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:studio/libs/adapter-client": true
  "@rusty-engine/studio-editor-shell@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:studio/libs/editor-shell": true
  "@rusty-engine/studio-user-settings@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:studio/libs/user-settings": true
  "@rusty-engine/studio-viewport@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:studio/libs/viewport": true
  "@rusty-engine/studio-voxel-editor@https://codeload.github.com/FuzzySlipper/rusty-engine/tar.gz/${ENGINE_REVISION}#path:studio/libs/voxel-editor": true
YAML

pnpm --dir "$PROBE_ROOT" install

if grep -En "(^|[[:space:]'\"])(workspace:|link:|file:)|/home/dev/" "$PROBE_ROOT/pnpm-lock.yaml"; then
  echo "clean Studio consumer resolved a local or workspace dependency" >&2
  exit 1
fi

for package_path in \
  render/packages/render-contracts \
  render/packages/render-projection \
  render/packages/renderer-host \
  render/packages/renderer-three \
  studio/libs/adapter-client \
  studio/libs/editor-shell \
  studio/libs/user-settings \
  studio/libs/viewport \
  studio/libs/voxel-editor
do
  if ! grep -Fq "${ENGINE_REVISION}#path:${package_path}" "$PROBE_ROOT/pnpm-lock.yaml"; then
    echo "clean Studio consumer did not lock ${package_path} to ${ENGINE_REVISION}" >&2
    exit 1
  fi
done

cat > "$PROBE_ROOT/probe.ts" <<'TS'
import {
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  VOXEL_OBJECT_COMPONENT_TYPE_ID,
} from '@rusty-engine/studio-adapter-client';
import {
  RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS,
  StudioShellComponent,
  admitStudioEntityInspectorContributions,
} from '@rusty-engine/studio-editor-shell';
import { buildDefaultStudioHostUserSettings } from '@rusty-engine/studio-user-settings';
import { StudioViewportComponent } from '@rusty-engine/studio-viewport';
import { VoxelObjectPlaybackComponent } from '@rusty-engine/studio-voxel-editor';

const contributions = admitStudioEntityInspectorContributions(
  RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS,
);
if (
  STUDIO_ADAPTER_PROTOCOL_VERSION !== 10
  || contributions[0]?.componentTypeId !== VOXEL_OBJECT_COMPONENT_TYPE_ID
  || typeof StudioShellComponent !== 'function'
  || typeof StudioViewportComponent !== 'function'
  || typeof VoxelObjectPlaybackComponent !== 'function'
  || buildDefaultStudioHostUserSettings('consumer').schemaVersion !== 1
) {
  throw new Error('exact-revision Studio packages did not compose one coherent typed surface');
}
TS

cat > "$PROBE_ROOT/tsconfig.json" <<'JSON'
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "preserve",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": false
  },
  "include": ["probe.ts"]
}
JSON

pnpm --dir "$PROBE_ROOT" exec tsc --project tsconfig.json

pnpm --dir "$PROBE_ROOT" exec node --input-type=module <<'JS'
import { STUDIO_ADAPTER_PROTOCOL_VERSION } from '@rusty-engine/studio-adapter-client';
import { buildDefaultStudioHostUserSettings } from '@rusty-engine/studio-user-settings';
import { decodeRenderFrameDiff } from '@rusty-engine/render-contracts';
import { RenderProjection } from '@rusty-engine/render-projection';
import { createRendererDefaultSurfaceFrame } from '@rusty-engine/renderer-host';
import { ThreeRenderer } from '@rusty-engine/renderer-three';

const frame = decodeRenderFrameDiff(createRendererDefaultSurfaceFrame());
const projection = new RenderProjection();
projection.applyFrame(frame);
const renderer = new ThreeRenderer();
renderer.applyFrame(frame);
const snapshot = renderer.snapshot();
renderer.dispose();

if (
  STUDIO_ADAPTER_PROTOCOL_VERSION !== 10
  || buildDefaultStudioHostUserSettings('consumer').schemaVersion !== 1
  || projection.handleCount === 0
  || snapshot.length === 0
) {
  throw new Error('exact-revision host-neutral packages did not execute coherently');
}
console.log('EXACT_PUBLIC_STUDIO_CONSUMER_OK');
JS
