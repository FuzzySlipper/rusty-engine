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
    "@angular/compiler": "~21.2.0",
    "@angular/core": "~21.2.0",
    "@angular/forms": "~21.2.0",
    "@angular/platform-browser": "~21.2.0",
    "@rusty-engine/render-contracts": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/render-contracts",
    "@rusty-engine/render-projection": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/render-projection",
    "@rusty-engine/renderer-host": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/renderer-host",
    "@rusty-engine/renderer-three": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:render/packages/renderer-three",
    "@rusty-engine/studio-adapter-client": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/adapter-client",
    "@rusty-engine/studio-editor-shell": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/editor-shell",
    "@rusty-engine/studio-user-settings": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/user-settings",
    "@rusty-engine/studio-viewport": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/viewport",
    "@rusty-engine/studio-voxel-editor": "github:FuzzySlipper/rusty-engine#${ENGINE_REVISION}&path:studio/libs/voxel-editor",
    "rxjs": "~7.8.0",
    "tslib": "^2.3.0"
  },
  "devDependencies": {
    "@angular/build": "~21.2.0",
    "@angular/cli": "~21.2.0",
    "@angular/compiler-cli": "~21.2.0",
    "typescript": "~5.9.2"
  }
}
JSON

cat > "$PROBE_ROOT/pnpm-workspace.yaml" <<YAML
packages:
  - "."

allowBuilds:
  "@parcel/watcher": true
  esbuild: true
  less: true
  lmdb: true
  msgpackr-extract: true
  nx: true
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

mkdir -p "$PROBE_ROOT/src"

cat > "$PROBE_ROOT/angular.json" <<'JSON'
{
  "$schema": "./node_modules/@angular/cli/lib/config/schema.json",
  "version": 1,
  "newProjectRoot": "projects",
  "projects": {
    "exact-public-studio-consumer": {
      "projectType": "application",
      "root": "",
      "sourceRoot": "src",
      "architect": {
        "build": {
          "builder": "@angular/build:application",
          "options": {
            "browser": "src/main.ts",
            "index": "src/index.html",
            "outputPath": "dist",
            "tsConfig": "tsconfig.app.json"
          }
        }
      }
    }
  }
}
JSON

cat > "$PROBE_ROOT/tsconfig.json" <<'JSON'
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "preserve",
    "moduleResolution": "bundler",
    "strict": true,
    "isolatedModules": true,
    "skipLibCheck": false
  },
  "angularCompilerOptions": {
    "enableI18nLegacyMessageIdFormat": false,
    "strictInjectionParameters": true,
    "strictInputAccessModifiers": true,
    "strictTemplates": true
  }
}
JSON

cat > "$PROBE_ROOT/tsconfig.app.json" <<'JSON'
{
  "extends": "./tsconfig.json",
  "compilerOptions": {
    "outDir": "./dist/out-tsc"
  },
  "include": ["src/**/*.ts"]
}
JSON

cat > "$PROBE_ROOT/src/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <head><meta charset="utf-8"><title>Exact public Studio package consumer</title></head>
  <body><exact-public-studio-consumer></exact-public-studio-consumer></body>
</html>
HTML

cat > "$PROBE_ROOT/src/main.ts" <<'TS'
import { Component } from '@angular/core';
import { bootstrapApplication } from '@angular/platform-browser';
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
import {
  VoxelEditorComponent,
  VoxelObjectPlaybackComponent,
} from '@rusty-engine/studio-voxel-editor';

const contributions = admitStudioEntityInspectorContributions(
  RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS,
);
if (
  STUDIO_ADAPTER_PROTOCOL_VERSION !== 12
  || contributions[0]?.componentTypeId !== VOXEL_OBJECT_COMPONENT_TYPE_ID
  || typeof StudioShellComponent !== 'function'
  || typeof StudioViewportComponent !== 'function'
  || typeof VoxelEditorComponent !== 'function'
  || typeof VoxelObjectPlaybackComponent !== 'function'
  || buildDefaultStudioHostUserSettings('consumer').schemaVersion !== 1
) {
  throw new Error('exact-revision Studio packages did not compose one coherent typed surface');
}

@Component({
  selector: 'exact-public-studio-consumer',
  standalone: true,
  imports: [
    StudioShellComponent,
    StudioViewportComponent,
    VoxelEditorComponent,
    VoxelObjectPlaybackComponent,
  ],
  template: `
    <rusty-studio-shell />
    <rusty-studio-viewport />
    <rusty-voxel-editor />
    <rusty-voxel-object-playback />
  `,
})
class ExactPublicStudioConsumer {}

void bootstrapApplication(ExactPublicStudioConsumer);
TS

pnpm --dir "$PROBE_ROOT" exec ng build exact-public-studio-consumer

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
  STUDIO_ADAPTER_PROTOCOL_VERSION !== 12
  || buildDefaultStudioHostUserSettings('consumer').schemaVersion !== 1
  || projection.handleCount === 0
  || snapshot.length === 0
) {
  throw new Error('exact-revision host-neutral packages did not execute coherently');
}
console.log('EXACT_PUBLIC_STUDIO_CONSUMER_OK');
JS
