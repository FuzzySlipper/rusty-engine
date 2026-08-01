import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderHandle } from '@rusty-engine/render-contracts';
import {
  captureRendererAnimatedMesh,
  RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_SAMPLES,
} from './animated-mesh-capture.js';
import type { RendererSurface } from './surface.js';

const PROVIDER_REVISION = '1111111111111111111111111111111111111111';

function validationSurface(width = 64, height = 64): RendererSurface {
  return { canvas: { width, height } } as unknown as RendererSurface;
}

void test('animated mesh capture rejects unbounded or inexact requests before touching a surface', () => {
  const base = {
    handle: renderHandle(1),
    clip: 'idle',
    normalizedTimes: [0, 1],
    providerRevision: PROVIDER_REVISION,
  } as const;
  assert.throws(
    () => captureRendererAnimatedMesh(validationSurface(), { ...base, providerRevision: 'main' }),
    /exact 40-character lowercase Git SHA/,
  );
  assert.throws(
    () => captureRendererAnimatedMesh(validationSurface(), {
      ...base,
      normalizedTimes: Array.from(
        { length: RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_SAMPLES + 1 },
        () => 0.5,
      ),
    }),
    /requires one to 32 normalized times/,
  );
  assert.throws(
    () => captureRendererAnimatedMesh(validationSurface(), { ...base, normalizedTimes: [-0.1] }),
    /requires one to 32 normalized times/,
  );
  assert.throws(
    () => captureRendererAnimatedMesh(validationSurface(2048, 2048), base),
    /pixel quota exceeded/,
  );
});
