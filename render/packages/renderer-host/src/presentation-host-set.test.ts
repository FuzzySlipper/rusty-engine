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

void test('billboard hosts advance while indicators are active', () => {
  let billboardActive = true;
  let advances = 0;
  const hosts = new RendererPresentationHostSet({
    billboard: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      advance: () => {
        advances += 1;
        return EMPTY_RECEIPT;
      },
      requiresAnimationFrame: () => billboardActive,
    },
  });

  assert.equal(hosts.requiresAnimationFrame(), true);
  assert.deepEqual(hosts.advance(1 / 60).advancedDomains, ['billboard']);
  assert.equal(advances, 1);
  billboardActive = false;
  assert.equal(hosts.requiresAnimationFrame(), false);
});

void test('listener synchronization is typed, local, and forwards only to audio hosts that support it', () => {
  const poses: unknown[] = [];
  const hosts = new RendererPresentationHostSet({
    audio: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      updateListener: (pose) => {
        poses.push(pose);
        return [];
      },
    },
  });
  const receipt = hosts.syncListener({
    position: [1, 2, 3],
    forward: [0, 0, -1],
    up: [0, 1, 0],
  });
  assert.deepEqual(receipt, {
    schemaVersion: 1, configured: true, applied: true, diagnostics: [],
  });
  assert.deepEqual(poses, [{
    position: [1, 2, 3], forward: [0, 0, -1], up: [0, 1, 0],
  }]);

  assert.deepEqual(new RendererPresentationHostSet({}).syncListener({
    position: [0, 0, 0], forward: [0, 0, -1], up: [0, 1, 0],
  }), {
    schemaVersion: 1, configured: false, applied: false, diagnostics: [],
  });

  const rejected = new RendererPresentationHostSet({
    audio: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      updateListener: () => [{
        code: 'hostFailure', sequence: 0, handle: null, message: 'audio host is disposed',
      }],
    },
  }).syncListener({
    position: [0, 0, 0], forward: [0, 0, -1], up: [0, 1, 0],
  });
  assert.equal(rejected.configured, true);
  assert.equal(rejected.applied, false);
  assert.equal(rejected.diagnostics[0]?.code, 'hostFailure');
});

void test('audio feedback acknowledgement remains separate from audio owner replacement', () => {
  let acknowledged = 0;
  let ownerResets = 0;
  const hosts = new RendererPresentationHostSet({
    audio: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      realizedFacts: () => ({
        retainedFactCount: 1,
        evictedFactCount: 0,
        facts: [],
      }),
      resetRealizedFacts: () => { acknowledged += 1; },
      reset: () => { ownerResets += 1; },
    },
  });

  assert.equal(hosts.readAudioRealizedFacts()?.retainedFactCount, 1);
  assert.equal(hosts.resetAudioRealizedFacts(), true);
  assert.equal(ownerResets, 0);
  assert.equal(hosts.resetAudioRealizationOwner(), true);
  assert.equal(acknowledged, 1);
  assert.equal(ownerResets, 1);
});
