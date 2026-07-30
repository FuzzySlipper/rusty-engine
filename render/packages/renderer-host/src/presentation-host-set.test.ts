import assert from 'node:assert/strict';
import test from 'node:test';

import type { PresentationFrameDiff } from '@rusty-engine/render-contracts';

import { RendererPresentationHostSet } from './presentation-host-set.js';

const EMPTY_RECEIPT = { applied: 0, diagnostics: [] };

void test('presentation demand is limited to active advancing mechanisms', () => {
  let animationActive = false;
  const hosts = new RendererPresentationHostSet({
    animation: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      advance: () => EMPTY_RECEIPT,
      requiresAnimationFrame: () => animationActive,
    },
    audio: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
    },
  });

  assert.equal(hosts.requiresAnimationFrame(), false);
  animationActive = true;
  assert.equal(hosts.requiresAnimationFrame(), true);
});

void test('unknown advancing hosts conservatively retain continuous advancement', () => {
  const hosts = new RendererPresentationHostSet({
    particle: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      advance: () => EMPTY_RECEIPT,
    },
  });

  assert.equal(hosts.requiresAnimationFrame(), true);
});
