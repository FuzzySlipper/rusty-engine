import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  particleEmitterHandle,
  type ParticleEmitterDescriptor,
  type PresentationFrameDiff,
  type PresentationOp,
} from '@rusty-engine/render-contracts';
import { RendererPresentationHostSet } from './presentation-host-set.js';
import {
  RendererParticleHost,
  type RendererParticleBillboard,
  type RendererParticleBillboardSink,
} from './particle-host.js';

const SPRITE_HASH = '9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a';
const SPRITE_FNV_HASH = 'be7a5e775165785d';

class FakeParticleSink implements RendererParticleBillboardSink {
  readonly active = new Map<number, RendererParticleBillboard>();
  readonly created: RendererParticleBillboard[] = [];
  readonly updated: RendererParticleBillboard[] = [];
  readonly destroyed: number[] = [];

  create(particle: RendererParticleBillboard): void {
    this.active.set(particle.id, particle);
    this.created.push(particle);
  }

  update(particle: RendererParticleBillboard): void {
    this.active.set(particle.id, particle);
    this.updated.push(particle);
  }

  destroy(id: number): void {
    this.active.delete(id);
    this.destroyed.push(id);
  }
}

class FailingParticleSink extends FakeParticleSink {
  readonly #failAt: number;

  constructor(failAt: number) {
    super();
    this.#failAt = failAt;
  }

  override create(particle: RendererParticleBillboard): void {
    super.create(particle);
    if (this.created.length === this.#failAt) throw new Error('injected particle sink failure');
  }
}

function descriptor(
  overrides: Partial<ParticleEmitterDescriptor> = {},
): ParticleEmitterDescriptor {
  const base: ParticleEmitterDescriptor = {
    anchor: { kind: 'world', position: [1, 2, 3] },
    visual: {
      kind: 'billboard',
      sprite: {
        asset: 'sprite-sheet/fixture-sparks',
        contentHash: SPRITE_HASH,
        frameCount: 4,
      },
    },
    ratePerSecond: 8,
    burstCount: 3,
    lifetimeSeconds: [0.2, 0.4],
    velocityMin: [-1, 1, -1],
    velocityMax: [1, 2, 1],
    acceleration: [0, -3, 0],
    sizeCurve: [
      { age: 0, value: 0.4 },
      { age: 1, value: 0 },
    ],
    colorCurve: [
      { age: 0, color: [1, 0.8, 0.2, 1] },
      { age: 1, color: [1, 0.2, 0, 0] },
    ],
    flipbookFramesPerSecond: 12,
    seed: 44,
    maxParticles: 16,
    visible: true,
  };
  return { ...base, ...overrides } as ParticleEmitterDescriptor;
}

function operation(
  sequence: number,
  op: Extract<PresentationOp, { readonly domain: 'particle' }>['op'],
): PresentationOp {
  return {
    domain: 'particle',
    meta: { sequence },
    op,
  };
}

function frame(ops: readonly PresentationOp[]): PresentationFrameDiff {
  return { schemaVersion: 1, ops };
}

function host(sink: FakeParticleSink, maxParticles = 64): RendererParticleHost {
  return new RendererParticleHost({
    maxParticles,
    resolveEntityPosition: (entity) => entity === 42 ? [10, 11, 12] : null,
    resolveResource: async () => ({
      bytes: new Uint8Array([1, 2, 3, 4]).buffer,
      url: '/sprites/fixture-sparks.png',
    }),
    sink,
  });
}

void test('particle host realizes deterministic bursts and expires disposable billboards', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink);
  const presentation = frame([
    operation(0, {
      op: 'emit',
      signalId: 'impact:44',
      descriptor: descriptor(),
    }),
  ]);

  const receipt = await particles.applyPresentation(presentation);
  assert.equal(receipt.applied, 1);
  assert.equal(receipt.readout.emittedBursts, 1);
  assert.equal(receipt.readout.activeParticles, 3);
  assert.equal(receipt.readout.loadedSprites, 1);
  assert.deepEqual(sink.created.map((particle) => particle.position), [
    [1, 2, 3],
    [1, 2, 3],
    [1, 2, 3],
  ]);

  const repeated = await particles.applyPresentation(presentation);
  assert.equal(repeated.applied, 1);
  assert.equal(repeated.readout.emittedBursts, 1);
  assert.equal(sink.created.length, 3, 'stable signal ids prevent duplicate realization');

  particles.advance(0.1);
  assert.equal(sink.updated.length, 3);
  assert.notDeepEqual(sink.updated[0]?.position, [1, 2, 3]);
  particles.advance(0.4);
  assert.equal(particles.readout().activeParticles, 0);
  assert.equal(sink.destroyed.length, 3);
});

void test('a missing entity anchor diagnoses without consuming the burst signal', async () => {
  const sink = new FakeParticleSink();
  let entityPosition: readonly [number, number, number] | null = null;
  const particles = new RendererParticleHost({
    resolveEntityPosition: () => entityPosition,
    resolveResource: async () => ({
      bytes: new Uint8Array([1, 2, 3, 4]).buffer,
      url: '/sprites/fixture-sparks.png',
    }),
    sink,
  });
  const presentation = frame([
    operation(0, {
      op: 'emit',
      signalId: 'late-anchor:44',
      descriptor: descriptor({
        anchor: { kind: 'entityAttached', entity: 404, offset: [0, 1, 0] },
      }),
    }),
  ]);

  const missing = await particles.applyPresentation(presentation);
  assert.equal(missing.applied, 0);
  assert.equal(missing.diagnostics[0]?.code, 'anchorMissing');
  assert.equal(missing.readout.emittedBursts, 0);
  assert.equal(missing.readout.activeParticles, 0);

  entityPosition = [4, 5, 6];
  const retried = await particles.applyPresentation(presentation);
  assert.equal(retried.applied, 1);
  assert.equal(retried.readout.emittedBursts, 1);
  assert.equal(retried.readout.activeParticles, 3);
  assert.deepEqual(sink.created[0]?.position, [4, 6, 6]);

  const repeated = await particles.applyPresentation(presentation);
  assert.equal(repeated.readout.emittedBursts, 1);
  assert.equal(repeated.readout.activeParticles, 3);
});

void test('missing particle resources fail locally without consuming the burst', async () => {
  const sink = new FakeParticleSink();
  const particles = new RendererParticleHost({
    resolveEntityPosition: () => [0, 0, 0],
    resolveResource: async () => null,
    sink,
  });
  const presentation = frame([
    operation(0, {
      op: 'emit',
      signalId: 'missing-particle:44',
      descriptor: descriptor(),
    }),
  ]);

  const receipt = await particles.applyPresentation(presentation);
  assert.equal(receipt.applied, 0);
  assert.equal(receipt.diagnostics[0]?.code, 'spriteLoadFailed');
  assert.equal(receipt.diagnostics[0]?.sequence, 0);
  assert.equal(receipt.readout.emittedBursts, 0);
  assert.equal(receipt.readout.activeParticles, 0);
  assert.equal(sink.created.length, 0);
});

void test('a partial sink failure rolls back the whole burst and leaves its signal retryable', async () => {
  const sink = new FailingParticleSink(2);
  const particles = host(sink);
  const presentation = frame([
    operation(0, {
      op: 'emit',
      signalId: 'retry-after-sink-failure',
      descriptor: descriptor(),
    }),
  ]);

  const failed = await particles.applyPresentation(presentation);
  assert.equal(failed.applied, 0);
  assert.equal(failed.diagnostics[0]?.code, 'hostFailure');
  assert.equal(failed.readout.activeParticles, 0);
  assert.equal(failed.readout.emittedBursts, 0);
  assert.equal(sink.active.size, 0);

  const retried = await particles.applyPresentation(presentation);
  assert.equal(retried.applied, 1);
  assert.equal(retried.readout.activeParticles, 3);
  assert.equal(sink.active.size, 3);
});

void test('a retained create does not publish its handle when its sink batch fails', async () => {
  const sink = new FailingParticleSink(2);
  const particles = host(sink);
  const receipt = await particles.applyPresentation(frame([
    operation(0, {
      op: 'create',
      handle: particleEmitterHandle(19),
      descriptor: descriptor(),
    }),
  ]));

  assert.equal(receipt.applied, 0);
  assert.equal(receipt.diagnostics[0]?.code, 'hostFailure');
  assert.equal(receipt.readout.activeEmitters, 0);
  assert.equal(receipt.readout.activeParticles, 0);
  assert.equal(sink.active.size, 0);
});

void test('particle host accepts a manifest-native FNV content hash', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink);
  const receipt = await particles.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalId: 'fnv-particle',
      descriptor: descriptor({
        visual: {
          kind: 'billboard',
          sprite: {
            asset: 'sprite/primary-fire-spark',
            contentHash: SPRITE_FNV_HASH,
            frameCount: 1,
          },
        },
      }),
    }),
  ]));

  assert.equal(receipt.applied, 1);
  assert.deepEqual(receipt.diagnostics, []);
  assert.equal(sink.created.length, 3);
});

void test('cube debris sweeps against emitter-local planes without resolving sprite resources', async () => {
  const sink = new FakeParticleSink();
  let resourceResolutions = 0;
  const particles = new RendererParticleHost({
    resolveEntityPosition: () => null,
    resolveResource: async () => {
      resourceResolutions += 1;
      return null;
    },
    sink,
  });
  const receipt = await particles.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalId: 'cube-bounce',
      descriptor: descriptor({
        visual: { kind: 'cube' },
        burstCount: 1,
        lifetimeSeconds: [2, 2],
        velocityMin: [0, -10, 0],
        velocityMax: [0, -10, 0],
        acceleration: [0, 0, 0],
        flipbookFramesPerSecond: 0,
        collision: {
          radius: 0.1,
          restitution: 0.5,
          friction: 0,
          maximumImpacts: 4,
          sleepSpeed: 0,
          limitBehavior: 'sleep',
          volumes: [{ kind: 'plane', normal: [0, 1, 0], offset: -1.5 }],
        },
      }),
    }),
  ]));

  assert.equal(receipt.applied, 1);
  assert.equal(resourceResolutions, 0);
  particles.advance(0.2);
  const afterImpact = sink.active.get(1)!;
  assert.ok(afterImpact.position[1] >= 0.6, 'swept radius remains above the local ground plane');
  const firstHeight = afterImpact.position[1];
  particles.advance(0.1);
  assert.ok(sink.active.get(1)!.position[1] > firstHeight, 'restitution sends debris upward');
  assert.equal(particles.readout().collisionImpacts, 1);
  assert.equal(particles.readout().collisionTests, 3);
  assert.equal(particles.readout().highWaterMark, 1);
  assert.equal(sink.active.get(1)!.visual.kind, 'cube');
});

void test('a terminal low-speed impact honors kill before the ordinary sleep threshold', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink);
  await particles.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalId: 'terminal-kill',
      descriptor: descriptor({
        visual: { kind: 'cube' },
        burstCount: 1,
        lifetimeSeconds: [2, 2],
        velocityMin: [0, -10, 0],
        velocityMax: [0, -10, 0],
        acceleration: [0, 0, 0],
        flipbookFramesPerSecond: 0,
        collision: {
          radius: 0.1,
          restitution: 0,
          friction: 0,
          maximumImpacts: 1,
          sleepSpeed: 100,
          limitBehavior: 'kill',
          volumes: [{ kind: 'plane', normal: [0, 1, 0], offset: 1 }],
        },
      }),
    }),
  ]));

  particles.advance(0.2);

  assert.equal(particles.readout().collisionImpacts, 1);
  assert.equal(particles.readout().activeParticles, 0);
  assert.deepEqual(sink.destroyed, [1]);
});

void test('a pre-limit low-speed impact may still sleep under kill-at-limit policy', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink);
  await particles.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalId: 'pre-limit-sleep',
      descriptor: descriptor({
        visual: { kind: 'cube' },
        burstCount: 1,
        lifetimeSeconds: [2, 2],
        velocityMin: [0, -10, 0],
        velocityMax: [0, -10, 0],
        acceleration: [0, 0, 0],
        flipbookFramesPerSecond: 0,
        collision: {
          radius: 0.1,
          restitution: 0,
          friction: 0,
          maximumImpacts: 2,
          sleepSpeed: 100,
          limitBehavior: 'kill',
          volumes: [{ kind: 'plane', normal: [0, 1, 0], offset: 1 }],
        },
      }),
    }),
  ]));

  particles.advance(0.2);
  const sleepingPosition = sink.active.get(1)!.position;
  particles.advance(0.2);

  assert.equal(particles.readout().collisionImpacts, 1);
  assert.equal(particles.readout().activeParticles, 1);
  assert.deepEqual(sink.active.get(1)!.position, sleepingPosition);
  assert.deepEqual(sink.destroyed, []);
});

void test('retained emitters can explicitly clear optional collision', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink);
  const handle = particleEmitterHandle(6);
  await particles.applyPresentation(frame([
    operation(0, {
      op: 'create',
      handle,
      descriptor: descriptor({
        visual: { kind: 'cube' },
        burstCount: 0,
        ratePerSecond: 0,
        lifetimeSeconds: [2, 2],
        flipbookFramesPerSecond: 0,
        collision: {
          radius: 0.1,
          restitution: 0.5,
          friction: 0,
          maximumImpacts: 4,
          sleepSpeed: 0,
          limitBehavior: 'sleep',
          volumes: [{ kind: 'plane', normal: [0, 1, 0], offset: -1 }],
        },
      }),
    }),
    operation(1, {
      op: 'update',
      handle,
      patch: {
        anchor: null,
        sprite: null,
        ratePerSecond: 1,
        burstCount: null,
        lifetimeSeconds: null,
        velocityMin: null,
        velocityMax: null,
        acceleration: null,
        sizeCurve: null,
        colorCurve: null,
        flipbookFramesPerSecond: null,
        maxParticles: null,
        visible: null,
        collision: null,
      },
    }),
  ]));

  particles.advance(1);
  particles.advance(0.1);
  assert.equal(particles.readout().activeParticles, 1);
  assert.equal(particles.readout().collisionTests, 0);
});

void test('retained emitter create update destroy owns continuous simulation and cleanup', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink);
  const handle = particleEmitterHandle(7);
  const created = await particles.applyPresentation(frame([
    operation(0, {
      op: 'create',
      handle,
      descriptor: descriptor({
        anchor: { kind: 'entityAttached', entity: 42, offset: [0, 1, 0] },
        burstCount: 0,
        ratePerSecond: 4,
        lifetimeSeconds: [1, 1],
      }),
    }),
  ]));
  assert.equal(created.readout.activeEmitters, 1);
  particles.advance(0.5);
  assert.equal(sink.created.length, 2);
  assert.deepEqual(sink.created[0]?.position, [10, 12, 12]);

  const updated = await particles.applyPresentation(frame([
    operation(0, {
      op: 'update',
      handle,
      patch: {
        anchor: null,
        sprite: null,
        ratePerSecond: 8,
        burstCount: null,
        lifetimeSeconds: null,
        velocityMin: null,
        velocityMax: null,
        acceleration: null,
        sizeCurve: null,
        colorCurve: null,
        flipbookFramesPerSecond: null,
        maxParticles: null,
        visible: false,
      },
    }),
  ]));
  assert.equal(updated.applied, 1);
  particles.advance(0.5);
  assert.equal(sink.created.length, 2, 'invisible retained emitter pauses new realization');

  const destroyed = await particles.applyPresentation(frame([
    operation(0, { op: 'destroy', handle }),
  ]));
  assert.equal(destroyed.readout.activeEmitters, 0);
  assert.equal(destroyed.readout.activeParticles, 0);
  assert.equal(sink.destroyed.length, 2);
});

void test('missing anchor budgets and unavailable host degrade independently after scene', async () => {
  const sink = new FakeParticleSink();
  const particles = host(sink, 2);
  const missing = await particles.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalId: 'missing-anchor',
      descriptor: descriptor({
        anchor: { kind: 'entityAttached', entity: 99, offset: [0, 0, 0] },
      }),
    }),
  ]));
  assert.equal(missing.diagnostics[0]?.code, 'anchorMissing');

  const budgeted = await particles.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalId: 'large-burst',
      descriptor: descriptor({ burstCount: 4 }),
    }),
  ]));
  assert.equal(budgeted.diagnostics[0]?.code, 'budgetExceeded');
  assert.equal(budgeted.readout.activeParticles, 2);
  assert.equal(budgeted.readout.droppedParticles, 2);

  const unavailable = await new RendererPresentationHostSet({}).apply(frame([
    operation(0, {
      op: 'emit',
      signalId: 'unavailable',
      descriptor: descriptor(),
    }),
  ]));
  const unavailableParticle = unavailable.domains.find((domain) => domain.domain === 'particle');
  assert.equal(unavailableParticle?.diagnostics[0]?.code, 'unavailableHost');
  assert.equal(unavailableParticle?.diagnostics[0]?.sequence, 0);
});
