import assert from 'node:assert/strict';
import test from 'node:test';

import { telemetryOverlayHandle, type PresentationFrameDiff } from '@rusty-engine/render-contracts';

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

void test('one failing optional advancing domain degrades while later domains and future advances continue', () => {
  let billboardAdvances = 0;
  let particleAdvances = 0;
  const hosts = new RendererPresentationHostSet({
    animation: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      advance: () => {
        throw new Error('injected animation host failure after mutation');
      },
    },
    billboard: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      advance: () => {
        billboardAdvances += 1;
        return EMPTY_RECEIPT;
      },
    },
    particle: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      advance: () => {
        particleAdvances += 1;
        return EMPTY_RECEIPT;
      },
    },
  });

  assert.deepEqual(hosts.advance(1 / 60).advancedDomains, ['billboard', 'particle']);
  assert.deepEqual(hosts.advance(1 / 60).advancedDomains, ['billboard', 'particle']);
  assert.equal(billboardAdvances, 2);
  assert.equal(particleAdvances, 2);
  assert.deepEqual(hosts.failureReadout(), {
    retainedFailureCount: 1,
    evictedFailureCount: 0,
    failures: [{
      domain: 'animation',
      stage: 'advance',
      message: 'injected animation host failure after mutation',
      occurrences: 1,
    }],
  });
});

void test('a local optional presentation rejection preserves other domains and a later valid operation', async () => {
  let rejectAudio = true;
  let overlayApplications = 0;
  const hosts = new RendererPresentationHostSet({
    audio: {
      applyPresentation: () => rejectAudio
        ? {
            applied: 0,
            diagnostics: [{
              code: 'assetMissing', sequence: 0, handle: null, message: 'optional clip is absent',
            }],
          }
        : { applied: 1, diagnostics: [] },
    },
    telemetryOverlay: {
      applyPresentation: () => {
        overlayApplications += 1;
        return { applied: 1, diagnostics: [] };
      },
    },
  });
  const frame = {
    schemaVersion: 1 as const,
    ops: [{
      domain: 'audio' as const,
      meta: { sequence: 0 },
      op: { op: 'busControl' as const, bus: 'sfx' as const, control: { kind: 'setMuted' as const, muted: false } },
    }, {
      domain: 'telemetryOverlay' as const,
      meta: { sequence: 1 },
      op: {
        op: 'create' as const,
        handle: telemetryOverlayHandle(7),
        descriptor: {
          title: 'renderer', corner: 'topLeft' as const, refreshIntervalMs: 100,
          maxFrameTimeSamples: 1, visible: true,
        },
      },
    }],
  } satisfies PresentationFrameDiff;

  const rejected = await hosts.apply(frame);
  assert.equal(rejected.outcome, 'partial');
  assert.equal(rejected.domains.find((domain) => domain.domain === 'audio')?.outcome, 'rejected_atomic');
  assert.equal(overlayApplications, 1, 'the unrelated optional domain remains realized');

  rejectAudio = false;
  const accepted = await hosts.apply(frame);
  assert.equal(accepted.outcome, 'applied');
  assert.equal(overlayApplications, 2, 'a later valid operation still reaches the same host set');
});

void test('listener host exceptions degrade audio once and never escape the render cadence', () => {
  const hosts = new RendererPresentationHostSet({
    audio: {
      applyPresentation: (_frame: PresentationFrameDiff) => EMPTY_RECEIPT,
      updateListener: () => {
        throw new Error('injected listener failure');
      },
    },
  });
  const pose = { position: [0, 0, 0] as const, forward: [0, 0, -1] as const, up: [0, 1, 0] as const };

  assert.equal(hosts.syncListener(pose).diagnostics[0]?.code, 'hostFailure');
  assert.equal(hosts.syncListener(pose).diagnostics[0]?.code, 'hostFailure');
  assert.equal(hosts.failureReadout().retainedFailureCount, 1);
  assert.equal(hosts.failureReadout().failures[0]?.occurrences, 1);
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

void test('realization feedback operations preserve their private-field-backed host receivers', () => {
  class ReceiverSensitiveAudioHost {
    #acknowledged = 0;
    #ownerResets = 0;

    readonly applyPresentation = (_frame: PresentationFrameDiff) => EMPTY_RECEIPT;

    realizedFacts() {
      return { retainedFactCount: 1, evictedFactCount: 0, facts: [] } as const;
    }

    acknowledgeRealizedFacts(throughFactId: number): void {
      this.#acknowledged += throughFactId;
    }

    reset(): void {
      this.#ownerResets += 1;
    }

    read(): readonly [number, number] {
      return [this.#acknowledged, this.#ownerResets];
    }
  }

  class ReceiverSensitiveAnimationHost {
    #acknowledged = 0;
    #ownerResets = 0;

    readonly applyPresentation = (_frame: PresentationFrameDiff) => EMPTY_RECEIPT;
    readonly advance = (_deltaSeconds: number) => EMPTY_RECEIPT;

    acknowledgeRealizedFacts(throughFactId: number): void {
      this.#acknowledged += throughFactId;
    }

    reset(): void {
      this.#ownerResets += 1;
    }

    read(): readonly [number, number] {
      return [this.#acknowledged, this.#ownerResets];
    }
  }

  const audio = new ReceiverSensitiveAudioHost();
  const animation = new ReceiverSensitiveAnimationHost();
  const hosts = new RendererPresentationHostSet({
    audio,
    animation,
  });

  assert.equal(hosts.readAudioRealizedFacts()?.retainedFactCount, 1);
  assert.equal(hosts.acknowledgeAudioRealizedFacts(7), true);
  assert.equal(hosts.resetAudioRealizationOwner(), true);
  assert.equal(hosts.acknowledgeAnimationRealizedFacts(11), true);
  assert.equal(hosts.resetAnimationRealizationOwner(), true);
  assert.deepEqual(audio.read(), [7, 1]);
  assert.deepEqual(animation.read(), [11, 1]);
});

void test('an obsolete empty host receipt cannot acknowledge requested operations', async () => {
  const hosts = new RendererPresentationHostSet({ audio: { applyPresentation: () => EMPTY_RECEIPT } });
  const receipt = await hosts.apply({ schemaVersion: 1, ops: [{
    domain: 'audio', meta: { sequence: 0 },
    op: { op: 'busControl', bus: 'sfx', control: { kind: 'setMuted', muted: false } },
  }] });
  assert.equal(receipt.outcome, 'terminal');
  assert.equal(receipt.diagnostics[0]?.code, 'hostFailure');
  assert.equal(receipt.domains.find((domain) => domain.domain === 'audio')?.requested, 1);
});
