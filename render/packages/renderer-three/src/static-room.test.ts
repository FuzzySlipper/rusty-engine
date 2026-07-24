import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { renderHandle } from '@rusty-engine/render-contracts';
import {
  createStaticRoomRenderFrame,
  STATIC_ROOM_FIXTURE_NAME,
} from './index.js';
import {
  renderProjectedFrame,
  ThreeRenderer,
} from './backend.js';

const repoRoot = resolve(import.meta.dirname, '../../../..');

void test('static room frame projects and renders through the package-root path', () => {
  const result = renderProjectedFrame(createStaticRoomRenderFrame());

  assert.equal(result.projection.handleCount, 7);
  assert.equal(result.renderer.handleCount, 7);
  assert.equal(result.projection.staticMeshRefCount('mesh/room-wall'), 4);
  assert.equal(result.renderer.instanceCountFor('mesh/room-wall'), 4);
  assert.ok(result.renderer.has(renderHandle(7)), 'origin marker handle should be live');
  assert.match(result.structuralSnapshot, /label "room-ceiling"/);
  assert.equal(result.renderer.fallbackMaterialCount, 0);
});

void test('static-room helper matches the committed successor golden snapshot', () => {
  const renderer = new ThreeRenderer();
  renderer.applyFrame(createStaticRoomRenderFrame());
  const golden = readFileSync(
    resolve(repoRoot, 'fixtures/render/goldens', `${STATIC_ROOM_FIXTURE_NAME}-v1.snapshot`),
    'utf8',
  );
  assert.equal(renderer.snapshot(), golden);
});
