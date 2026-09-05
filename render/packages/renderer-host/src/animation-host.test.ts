import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  animationProjectionHandle,
  renderHandle,
  type AnimationControllerProjectionState,
  type PresentationFrameDiff,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';
import {
  RendererAnimationHost,
  RendererAnimationCueDefinitionError,
  RendererPresentationHostSet,
  createRendererAnimatedMeshProjection,
  type RendererAnimatedMeshResourceManifest,
} from './index.js';

const ANIMATED_ASSET = 'mesh-animation/kenney-retro-character-medium';
const ANIMATED_HASH = 'sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674';
const ANIMATED_FIXTURE = resolve(
  import.meta.dirname,
  '../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb',
);
const ANIMATED_MANIFEST: RendererAnimatedMeshResourceManifest = {
  kind: 'rusty_renderer_animated_mesh_resources.v1',
  resources: [{
    asset: ANIMATED_ASSET,
    contentHash: ANIMATED_HASH,
    clipIds: ['idle', 'run', 'jump'],
  }],
};

function sceneFrame(sourceEntity: number | null = null): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineAnimatedMesh',
        asset: {
          asset: ANIMATED_ASSET,
          runtimeFormat: 'glb',
          contentHash: ANIMATED_HASH,
          clips: [
            { id: 'idle', name: 'idle', durationSeconds: 1.04166662693024 },
            { id: 'run', name: 'run', durationSeconds: 0.666666686534882 },
            { id: 'jump', name: 'jump', durationSeconds: 0.5 },
          ],
          defaultClip: 'idle',
          materialSlots: [],
          bounds: { min: [-0.02, -0.01, 0], max: [0.02, 0.01, 0.04] },
        },
      },
      {
        op: 'createAnimatedMeshInstance',
        handle: renderHandle(4100),
        parent: null,
        instance: {
          asset: ANIMATED_ASSET,
          transform: { translation: [0, 0, -2.5], rotation: [0, 0, 0, 1], scale: [40, 40, 40] },
          materialOverrides: [],
          playback: null,
          visible: true,
          metadata: {
            sourceEntity,
            sourceSceneNode: null,
            tags: [],
            label: 'controller target',
          },
        },
      },
    ],
  };
}

function controller(
  revision: number,
  elapsedTicks: number | null,
  targetClip = 'run',
): AnimationControllerProjectionState {
  return {
    entity: 1,
    graphId: 'player',
    graphVersion: 1,
    stateId: 'idle',
    revision,
    controllerTick: elapsedTicks ?? 0,
    motion: {
      clipA: 'idle',
      clipB: null,
      blendWeightMilli: 0,
      speedMilli: 1_000,
    },
    transition: elapsedTicks === null ? null : {
      transitionId: 'idle.move',
      fromStateId: 'idle',
      toStateId: 'locomotion',
      elapsedTicks,
      durationTicks: 2,
      targetMotion: {
        clipA: targetClip,
        clipB: null,
        blendWeightMilli: 0,
        speedMilli: 1_000,
      },
    },
    transitionFact: null,
  };
}

function createFrame(): PresentationFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      domain: 'animation',
      meta: { sequence: 0 },
      op: {
        op: 'create',
        handle: animationProjectionHandle(1),
        descriptor: {
          target: renderHandle(4100),
          asset: ANIMATED_ASSET,
          contentHash: ANIMATED_HASH,
          tickDurationMillis: 50,
          controller: controller(0, null),
        },
      },
    }],
  };
}

function updateFrame(targetClip = 'run'): PresentationFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      domain: 'animation',
      meta: { sequence: 0 },
      op: {
        op: 'update',
        handle: animationProjectionHandle(1),
        // FSM revision remains zero while fixed-tick transition progress moves.
        controller: controller(0, 1, targetClip),
      },
    }],
  };
}

function fixtureResolver(): Promise<ArrayBuffer> {
  const bytes = readFileSync(ANIMATED_FIXTURE);
  return Promise.resolve(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
}

void test('cue definition validation is typed and preserves the prior retained cue snapshot', () => {
  const host = new RendererAnimationHost({
    subscribeNaturalCompletions: () => () => undefined,
  } as never);
  const valid = [{
    cueId: 'footfall', asset: 'mesh/player', clip: 'walk', atSeconds: 0.1,
    signal: { domain: 'audio' as const, id: 'footstep' },
  }];
  host.replaceCueDefinitions(valid);
  assert.throws(
    () => host.replaceCueDefinitions([{ ...valid[0]!, cueId: '' }]),
    RendererAnimationCueDefinitionError,
  );
  assert.deepEqual(host.cueDefinitions(), valid);
});

void test('G1 controller sequence drives deterministic renderer-local blend and smooth sampling', async () => {
  const testGlobal = globalThis as unknown as { self: unknown };
  const priorSelf = testGlobal.self;
  testGlobal.self = globalThis;
  const priorWarn = console.warn;
  const priorError = console.error;
  console.warn = () => undefined;
  console.error = () => undefined;
  try {
    const projection = await createRendererAnimatedMeshProjection({
      manifest: ANIMATED_MANIFEST,
      resolveResource: fixtureResolver,
    });
    assert.equal(projection.applyFrame(sceneFrame()).applied, true);
    const resource = ANIMATED_MANIFEST.resources[0];
    assert.ok(resource);
    const host = new RendererAnimationHost(projection);
    host.replaceCueDefinitions([
      { cueId: 'd', asset: 'a:b', clip: 'c', atSeconds: 0, signal: { domain: 'audio', id: 'first' } },
      { cueId: 'd', asset: 'a', clip: 'b:c', atSeconds: 0, signal: { domain: 'audio', id: 'second' } },
    ]);
    host.replaceCueDefinitions([{
      cueId: 'locomotion.footfall',
      asset: resource.asset,
      clip: 'run',
      atSeconds: 0.04,
      signal: { domain: 'particle', id: 'locomotion.footfall.spark' },
    }]);
    assert.deepEqual(host.cueDefinitions(), [{
      cueId: 'locomotion.footfall',
      asset: resource.asset,
      clip: 'run',
      atSeconds: 0.04,
      signal: { domain: 'particle', id: 'locomotion.footfall.spark' },
    }]);
    assert.equal(host.applyPresentation(createFrame()).applied, 1);
    assert.deepEqual(projection.playback(renderHandle(4100)).controllerClips, [
      { clip: 'idle', weight: 1, speed: 1 },
    ]);

    assert.equal(host.applyPresentation(updateFrame()).applied, 1);
    const firstSample = host.advance(0.025);
    assert.deepEqual(firstSample.cues, []);
    assert.deepEqual(projection.playback(renderHandle(4100)).controllerClips, [
      { clip: 'idle', weight: 0.75, speed: 1 },
      { clip: 'run', weight: 0.25, speed: 1 },
    ]);
    const halfwayPose = projection.playback(renderHandle(4100)).poseSample;
    const cueSample = host.advance(0.025);
    assert.deepEqual(projection.playback(renderHandle(4100)).controllerClips, [
      { clip: 'idle', weight: 0.5, speed: 1 },
      { clip: 'run', weight: 0.5, speed: 1 },
    ]);
    assert.notDeepEqual(
      projection.playback(renderHandle(4100)).poseSample?.hierarchyRotationSum,
      halfwayPose?.hierarchyRotationSum,
    );
    assert.deepEqual(cueSample.cues, [{
      kind: 'rusty.animation.sampled_cue.v1',
      cueId: 'locomotion.footfall',
      handle: animationProjectionHandle(1),
      target: renderHandle(4100),
      asset: resource.asset,
      clip: 'run',
      markerSeconds: 0.04,
      sampledAtSeconds: 0.05,
      signal: { domain: 'particle', id: 'locomotion.footfall.spark' },
    }]);
    assert.deepEqual(host.advance(0.025).cues, []);
    assert.equal(host.readout().sampledFrames, 3);
    const observed = host.realizedFacts();
    assert.ok(observed.facts.length >= 2);
    const firstFactId = observed.facts[0]!.factId;
    const lastFactId = observed.facts[observed.facts.length - 1]!.factId;
    host.acknowledgeRealizedFacts(firstFactId);
    assert.ok(host.realizedFacts().facts.every((fact) => fact.factId > firstFactId));
    host.reset();
    assert.equal(host.realizedFacts().facts.length, 0);
    assert.equal(host.realizedFacts().evictedFactCount, 0);
    host.advance(0);
    assert.ok(host.realizedFacts().facts[0]!.factId > lastFactId);
    const cleanup = host.cleanup();
    assert.equal(cleanup.applied, 1);
    assert.equal(cleanup.readout.activeControllers, 0);
    assert.equal(projection.playback(renderHandle(4100)).status, 'stopped');

    const rebuilt = new RendererAnimationHost(projection);
    assert.equal(rebuilt.applyPresentation(createFrame()).applied, 1);
    assert.equal(rebuilt.applyPresentation(updateFrame()).applied, 1);
    const destroyed = rebuilt.applyPresentation({
      schemaVersion: 1,
      ops: [{
        domain: 'animation',
        meta: { sequence: 0 },
        op: { op: 'destroy', handle: animationProjectionHandle(1) },
      }],
    });
    assert.equal(destroyed.applied, 1);
    assert.equal(destroyed.readout.activeControllers, 0);
    assert.equal(projection.playback(renderHandle(4100)).status, 'stopped');
  } finally {
    console.warn = priorWarn;
    console.error = priorError;
    testGlobal.self = priorSelf;
  }
});

void test('renderer host maps only direct LoopOnce mixer completion into bounded facts', async () => {
  const testGlobal = globalThis as unknown as { self: unknown };
  const priorSelf = testGlobal.self;
  testGlobal.self = globalThis;
  try {
    const projection = await createRendererAnimatedMeshProjection({
      manifest: ANIMATED_MANIFEST,
      resolveResource: fixtureResolver,
    });
    assert.equal(projection.applyFrame(sceneFrame(77)).applied, true);
    const host = new RendererAnimationHost(projection);
    assert.equal(projection.applyFrame({
      schemaVersion: 1,
      ops: [{
        op: 'setAnimatedMeshPlayback',
        handle: renderHandle(4100),
        playback: { kind: 'play', clip: 'jump', loop: 'once', speed: 1, weight: 1, restart: true, fadeSeconds: null },
      }],
    }).applied, true);
    host.advance(1);
    const completions = host.realizedFacts().facts.filter((fact) => fact.kind === 'naturalCompletion');
    assert.deepEqual(completions, [{
      kind: 'naturalCompletion', factId: 1, objectId: 77, generation: 1, clip: 'jump',
    }]);

    // Controller weights are LoopRepeat/Infinity and therefore cannot make a
    // natural-completion fact even after many renderer advances.
    assert.equal(host.applyPresentation(createFrame()).applied, 1);
    host.advance(2);
    assert.equal(host.realizedFacts().facts.filter((fact) => fact.kind === 'naturalCompletion').length, 1);
  } finally {
    testGlobal.self = priorSelf;
  }
});

void test('a reconstructed controller begins cue sampling at its retained clip phase', async () => {
  const testGlobal = globalThis as unknown as { self: unknown };
  const priorSelf = testGlobal.self;
  testGlobal.self = globalThis;
  try {
    const projection = await createRendererAnimatedMeshProjection({
      manifest: ANIMATED_MANIFEST,
      resolveResource: fixtureResolver,
    });
    assert.equal(projection.applyFrame(sceneFrame()).applied, true);
    const host = new RendererAnimationHost(projection, {
      cues: [{
        cueId: 'old-footfall', asset: ANIMATED_ASSET, clip: 'idle', atSeconds: 0.1,
        signal: { domain: 'audio', id: 'footfall' },
      }, {
        cueId: 'next-footfall', asset: ANIMATED_ASSET, clip: 'idle', atSeconds: 0.55,
        signal: { domain: 'audio', id: 'next-footfall' },
      }],
    });
    const baseline = createFrame();
    const operation = baseline.ops[0];
    assert.ok(operation?.domain === 'animation' && operation.op.op === 'create');
    assert.equal(host.applyPresentation({
      ...baseline,
      ops: [{
        ...operation,
        op: {
          ...operation.op,
          descriptor: {
            ...operation.op.descriptor,
            controller: {
              ...operation.op.descriptor.controller,
              clipPhases: [{ clip: 'idle', timeSeconds: 0.5 }],
            },
          },
        },
      }],
    }).applied, 1);
    assert.deepEqual(host.advance(0.01).cues, []);
    assert.equal(host.applyPresentation({
      schemaVersion: 1,
      ops: [{ domain: 'animation', meta: { sequence: 0 }, op: {
        op: 'update', handle: animationProjectionHandle(1),
        controller: { ...controller(1, null), clipPhases: [{ clip: 'idle', timeSeconds: 50 }] },
      } }],
    }).applied, 1);
    assert.deepEqual(host.advance(0.05).cues.map((cue) => cue.cueId), ['next-footfall']);
    assert.ok(Math.abs((projection.playback(renderHandle(4100)).actionTimeSeconds ?? 0) - 0.56) < 1e-6);
    assert.equal(projection.playback(renderHandle(4100)).controllerClips
      .some((clip) => clip.timeSeconds !== undefined), false);
  } finally {
    testGlobal.self = priorSelf;
  }
});

void test('animation host isolates missing targets and clips with typed diagnostics', async () => {
  const testGlobal = globalThis as unknown as { self: unknown };
  const priorSelf = testGlobal.self;
  testGlobal.self = globalThis;
  try {
    const projection = await createRendererAnimatedMeshProjection({
      manifest: ANIMATED_MANIFEST,
      resolveResource: fixtureResolver,
    });
    projection.applyFrame(sceneFrame());
    const host = new RendererAnimationHost(projection);
    host.applyPresentation(createFrame());
    const missingClip = host.applyPresentation(updateFrame('missing'));
    assert.equal(missingClip.applied, 0);
    assert.equal(missingClip.diagnostics[0]?.code, 'clipMissing');

    const missingTarget = createFrame();
    const operation = missingTarget.ops[0];
    assert.ok(operation?.domain === 'animation' && operation.op.op === 'create');
    const otherHost = new RendererAnimationHost(projection);
    const receipt = otherHost.applyPresentation({
      ...missingTarget,
      ops: [{
        ...operation,
        op: {
          ...operation.op,
          descriptor: { ...operation.op.descriptor, target: renderHandle(999) },
        },
      }],
    });
    assert.equal(receipt.diagnostics[0]?.code, 'unknownTarget');
    assert.equal(receipt.diagnostics[0]?.sequence, 0);
  } finally {
    testGlobal.self = priorSelf;
  }
});

void test('missing animation host returns an explicit typed domain diagnostic', async () => {
  const receipt = await new RendererPresentationHostSet({}).apply(createFrame());
  const animation = receipt.domains.find((domain) => domain.domain === 'animation');
  assert.equal(animation?.applied, 0);
  assert.equal(animation?.configured, false);
  assert.equal(animation?.diagnostics[0]?.code, 'unavailableHost');
  assert.equal(animation?.diagnostics[0]?.sequence, 0);
});
