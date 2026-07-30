import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RendererSurfaceSubmissionDemand,
  type RendererSurfaceContinuousDemand,
  type RendererSurfaceViewportState,
} from './surface-submission-demand.js';

const VIEWPORT: RendererSurfaceViewportState = {
  bufferHeight: 720,
  bufferWidth: 1280,
  clientHeight: 720,
  clientWidth: 1280,
};

const IDLE: RendererSurfaceContinuousDemand = {
  controls: false,
  presentation: false,
  retainedAnimation: false,
};

void test('static automatic submissions are coalesced until owner state changes', () => {
  const demand = new RendererSurfaceSubmissionDemand(VIEWPORT);

  assert.equal(demand.consume(VIEWPORT, IDLE), false);
  demand.request();
  demand.request();
  assert.equal(demand.consume(VIEWPORT, IDLE), true);
  assert.equal(demand.consume(VIEWPORT, IDLE), false);

  const resized = { ...VIEWPORT, clientWidth: 960 };
  assert.equal(demand.consume(resized, IDLE), true);
  assert.equal(demand.consume(resized, IDLE), false);
});

void test('controls presentation and retained animation preserve continuous demand', () => {
  for (const owner of ['controls', 'presentation', 'retainedAnimation'] as const) {
    const demand = new RendererSurfaceSubmissionDemand(VIEWPORT);
    assert.equal(demand.consume(VIEWPORT, { ...IDLE, [owner]: true }), true);
    assert.equal(demand.consume(VIEWPORT, { ...IDLE, [owner]: true }), true);
    assert.equal(demand.consume(VIEWPORT, IDLE), false);
  }
});

void test('an explicit submission settles pending and resize demand', () => {
  const demand = new RendererSurfaceSubmissionDemand(VIEWPORT);
  const resized = { ...VIEWPORT, bufferWidth: 1920, clientWidth: 1920 };
  demand.request();
  demand.submitted(resized);

  assert.equal(demand.consume(resized, IDLE), false);
});
