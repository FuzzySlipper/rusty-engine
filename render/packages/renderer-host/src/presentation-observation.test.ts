import assert from 'node:assert/strict';
import test from 'node:test';

import type { RenderPublicationFrontier } from '@rusty-engine/render-contracts';

import {
  createPresentationSurfaceId,
  observeRendererPresentation,
  type RendererSubmittedFrontier,
} from './presentation-observation.js';

interface TestPresentation extends RendererSubmittedFrontier {
  readonly marker: string;
}

const FRONTIER_1: RenderPublicationFrontier = Object.freeze({ stream: 'scene', revision: 1 });
const FRONTIER_2: RenderPublicationFrontier = Object.freeze({ stream: 'scene', revision: 2 });
const VIEWPORT = Object.freeze({
  cssWidth: 800,
  cssHeight: 600,
  backingWidth: 1600,
  backingHeight: 1200,
});

function presentation(overrides: Partial<TestPresentation> = {}): TestPresentation {
  return Object.freeze({
    marker: 'submitted-frame',
    publicationFrontiers: [FRONTIER_1],
    viewRevision: 4,
    viewport: VIEWPORT,
    ...overrides,
  });
}

void test('no completed submission is reported as pending', () => {
  const observation = observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_1], 4, VIEWPORT, null,
  );

  assert.equal(observation.state, 'pending');
  assert.equal(observation.pendingRealizations, 0);
  assert.equal(observation.submitted, null);
});

void test('an applied newer publication remains pending until a matching submission is retained', () => {
  const prior = presentation();
  const pending = observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_2], 4, VIEWPORT, prior,
  );

  assert.equal(pending.state, 'pending');
  assert.equal(pending.submitted, prior);
  assert.equal(pending.submitted?.publicationFrontiers[0]?.revision, 1);

  const submitted = presentation({ publicationFrontiers: [FRONTIER_2] });
  const settled = observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_2], 4, VIEWPORT, submitted,
  );
  assert.equal(settled.state, 'submitted');
});

void test('a newer view revision or viewport size keeps the prior presentation pending', () => {
  const prior = presentation();

  const changedView = observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_1], 5, VIEWPORT, prior,
  );
  assert.equal(changedView.state, 'pending');
  assert.equal(changedView.submitted, prior);

  const changedViewport = observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_1], 4,
    { ...VIEWPORT, cssWidth: 1024, backingHeight: 1536 }, prior,
  );
  assert.equal(changedViewport.state, 'pending');
  assert.equal(changedViewport.submitted, prior);
});

void test('pending realizations keep an otherwise matching presentation pending', () => {
  const prior = presentation();
  const observation = observeRendererPresentation(
    'surface-1', true, 1, [FRONTIER_1], 4, VIEWPORT, prior,
  );

  assert.equal(observation.state, 'pending');
  assert.equal(observation.pendingRealizations, 1);
  assert.equal(observation.submitted, prior);
});

void test('lost or disposed availability reports unavailable while preserving the prior submission', () => {
  const prior = presentation();

  for (const lifecycle of ['context-lost', 'disposed'] as const) {
    const observation = observeRendererPresentation(
      `surface-${lifecycle}`, false, 0, [FRONTIER_1], 4, VIEWPORT, prior,
    );
    assert.equal(observation.state, 'unavailable');
    assert.equal(observation.submitted, prior);
  }
});

void test('observation preserves the exact submitted object and makes no GPU or capture claim', () => {
  const prior = presentation({ marker: 'same-object' });
  const observation = observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_1], 4, VIEWPORT, prior,
  );

  assert.equal(observation.state, 'submitted');
  assert.equal(observation.submitted, prior);
  assert.equal(observation.submitted?.marker, 'same-object');
  assert.equal(observation.gpuCompletion, 'unavailable');
  assert.equal(observation.captureCorrelation, 'unavailable');
});

void test('a requested redraw remains pending even without a new publication or view counter', () => {
  const submitted = presentation();
  assert.equal(observeRendererPresentation(
    'surface-1', true, 0, [FRONTIER_1], 4, VIEWPORT, submitted, true,
  ).state, 'pending');
});

test('surface identities work without secure-context randomUUID and differ across surfaces', (t) => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  Object.defineProperty(globalThis, 'crypto', { configurable: true, value: {
    getRandomValues: globalThis.crypto.getRandomValues.bind(globalThis.crypto),
  } });
  t.after(() => Object.defineProperty(globalThis, 'crypto', descriptor!));
  assert.equal(globalThis.crypto.randomUUID, undefined);
  const first = createPresentationSurfaceId();
  const second = createPresentationSurfaceId();
  assert.match(first, /^surface-[0-9a-f]{32}$/u);
  assert.notEqual(first, second);
});
