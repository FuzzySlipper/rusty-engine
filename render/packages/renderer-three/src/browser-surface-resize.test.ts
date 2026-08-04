import assert from 'node:assert/strict';
import test from 'node:test';

import { resolveRendererBrowserSurfaceLogicalViewport } from './browser-surface.js';

void test('software backing-buffer caps do not feed back into logical canvas size', () => {
  const first = resolveRendererBrowserSurfaceLogicalViewport(0, 0, 1280, 720, 1);
  assert.deepEqual(first, { width: 1280, height: 720 });

  // setSize(1280, 720) at a 0.25 renderer ratio produces this backing buffer.
  const second = resolveRendererBrowserSurfaceLogicalViewport(0, 0, 320, 180, 1, first.width, first.height);
  assert.deepEqual(second, { width: 1280, height: 720 });
  const third = resolveRendererBrowserSurfaceLogicalViewport(0, 0, 80, 45, 1, second.width, second.height);
  assert.deepEqual(third, { width: 1280, height: 720 });
});

void test('explicit CSS dimensions remain the logical viewport', () => {
  assert.deepEqual(
    resolveRendererBrowserSurfaceLogicalViewport(640, 360, 320, 180, 0.25, 1280, 720),
    { width: 640, height: 360 },
  );
});
