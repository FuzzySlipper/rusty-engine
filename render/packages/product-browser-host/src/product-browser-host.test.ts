import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ProductBrowserHostError,
  PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE,
  bindProductBrowserInitialRendererFrame,
  createProductBrowserAudioFeedbackReporter,
  createProductBrowserGhostPlateFeedbackReporter,
  createProductBrowserRendererDiagnosticsReporter,
  isDroppedClockRegression,
  productBrowserAtomicReceiptMayContinue,
  productBrowserPresentationReceiptMayContinue,
  createProductBrowserRuntimeTransport,
  flushProductBrowserAudioFeedbackBeforeUpdate,
  productBrowserInitialRendererFrameRequired,
  prepareProductBrowserInitialRendererBaseline,
  productBrowserBundleAssets,
  productBrowserBundleDescriptor,
} from './product-browser-host.js';
import type {
  ProductBrowserRuntimeAdapter,
  ProductBrowserRuntimeOutput,
} from './product-browser-host.js';
import type {
  RustyApplicationFrame,
  RustyApplicationRuntimeInputEnvelope,
} from '@rusty-engine/application-host';
import { createProductBrowserCadence } from './realtime-cadence.js';

const AUDIO_RUNTIME = { instanceId: '7', generation: '1', controlRevision: '2' } as const;
const ACCEPTED_FAULT = { code: 'DEV_HOST_ACCEPTED', disposition: 'accepted' } as const;
const RECOVERABLE_FAULT = { code: 'DEV_HOST_TEST_REJECTED', disposition: 'rejected-recoverable' } as const;

const adapter: ProductBrowserRuntimeAdapter = {
  lifecycle: async (operation) => ({
    accepted: true,
    ...ACCEPTED_FAULT,
    operation: operation.kind,
  }),
  input: async (batch: readonly RustyApplicationRuntimeInputEnvelope[]) => ({ accepted: true, ...ACCEPTED_FAULT, count: batch.length }),
  reportAudioFeedback: async (feedback) => ({
    accepted: true,
    ...ACCEPTED_FAULT,
    runtime: feedback.runtime,
    ...(feedback.facts.length === 0
      ? {}
      : { acceptedThroughFactId: feedback.facts[feedback.facts.length - 1]!.factId }),
  }),
  reportAnimationFeedback: async (feedback) => ({
    accepted: true,
    ...ACCEPTED_FAULT,
    runtime: feedback.runtime,
    ...(feedback.facts.length === 0
      ? {}
      : { acceptedThroughFactId: feedback.facts[feedback.facts.length - 1]!.factId }),
  }),
  reportGhostPlateFeedback: async (feedback) => ({ accepted: true, ...ACCEPTED_FAULT, runtime: feedback.runtime }),
  advanceRealtime: async () => ({ accepted: true, ...ACCEPTED_FAULT, operation: 'advance-realtime' as const }),
  subscribeOutputs: () => () => undefined,
  dispose: () => undefined,
};

test('fixed runtime transport preserves only named operations', async () => {
  const transport = createProductBrowserRuntimeTransport(adapter);
  assert.equal((await transport.lifecycle({ kind: 'start' })).accepted, true);
  assert.equal((await transport.input([])).count, 0);
  assert.equal((await transport.advanceRealtime('1000000')).operation, 'advance-realtime');
  assert.equal('call' in transport, false);
});

test('only the typed lifecycle clock regression is a dropped cadence observation', () => {
  const dropped = {
    accepted: false,
    code: 'CSHARP_LIFECYCLE_CLOCK_REGRESSION',
    disposition: 'rejected-recoverable' as const,
    operation: 'advance-realtime' as const,
    diagnostic: 'observed clock regressed',
  };
  assert.equal(isDroppedClockRegression(dropped), true);
  assert.equal(isDroppedClockRegression({ ...dropped, code: 'CSHARP_LIFECYCLE_COUNTER_EXHAUSTED' }), false);
  assert.equal(isDroppedClockRegression({ ...dropped, disposition: 'terminal' }), false);
  assert.equal(isDroppedClockRegression({ ...dropped, operation: 'admit-demand-step' }), false);
});

test('renderer receipt policy keeps atomic rejections and partial presentation observable for later valid output', () => {
  assert.equal(productBrowserAtomicReceiptMayContinue('rejected_atomic'), true);
  assert.equal(productBrowserAtomicReceiptMayContinue('applied'), true);
  assert.equal(productBrowserPresentationReceiptMayContinue('rejected_atomic'), true);
  assert.equal(productBrowserPresentationReceiptMayContinue('partial'), true);
  assert.equal(productBrowserPresentationReceiptMayContinue('applied'), true);
});

test('renderer receipt policy closes only a typed terminal outcome', () => {
  assert.equal(productBrowserAtomicReceiptMayContinue('terminal'), false);
  assert.equal(productBrowserPresentationReceiptMayContinue('terminal'), false);
});

test('animation preloads bind to the first retained frame without replacing admitted bytes', () => {
  const digest = 'a'.repeat(64);
  const resources = Object.freeze([{
    identity: `animated-mesh-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    mediaType: 'model/gltf-binary',
    bytes: new Uint8Array(20),
  }]);
  const renderer = {
    initialContent: {
      frame: { schemaVersion: 1, ops: [] },
      resources,
    },
  } as const;
  const retainedFrame = {
    schemaVersion: 1,
    ops: [{
      op: 'defineAnimatedMesh',
      asset: {
        asset: 'mesh-animation/test', runtimeFormat: 'glb', contentHash: `sha256:${digest}`,
        clips: [], defaultClip: null, materialSlots: [],
        bounds: { min: [0, 0, 0], max: [1, 1, 1] },
      },
    }],
  } as unknown as RustyApplicationFrame;

  assert.equal(productBrowserInitialRendererFrameRequired(renderer), true);
  const bound = bindProductBrowserInitialRendererFrame(renderer, retainedFrame);
  assert.equal(bound.initialContent?.frame, retainedFrame);
  assert.equal(bound.initialContent?.resources, resources);
  assert.notEqual(bound, renderer);
  assert.equal(productBrowserInitialRendererFrameRequired(bound), false);
  assert.equal(productBrowserInitialRendererFrameRequired({
    initialContent: {
      frame: { schemaVersion: 1, ops: [] },
      resources: [{
        ...resources[0]!,
        identity: `texture-resource/${digest}`,
        mediaType: 'image/png',
      }],
    },
  }), false);
});

test('initial renderer baseline folds pre-publication diffs and removes only repeated definitions', () => {
  const animatedDefinition = { op: 'defineAnimatedMesh', asset: { asset: 'mesh/test' } };
  const animated = { schemaVersion: 1, ops: [animatedDefinition] } as unknown as RustyApplicationFrame;
  const textureDefinition = { op: 'defineTexture', texture: { id: 'texture/test', version: 1 } };
  const textures = { schemaVersion: 1, ops: [textureDefinition] } as unknown as RustyApplicationFrame;
  const published = {
    schemaVersion: 1,
    publication: { stream: 'voxel:test', baseRevision: 0, revision: 1, operationCount: 2 },
    ops: [textureDefinition, { op: 'create', handle: 1, node: {} }],
  } as unknown as RustyApplicationFrame;
  const outputs = [
    { kind: 'frame', frame: animated },
    { kind: 'frame', frame: textures },
    { kind: 'frame', frame: published },
  ] as readonly ProductBrowserRuntimeOutput[];

  const baseline = prepareProductBrowserInitialRendererBaseline(outputs, animated);
  assert.deepEqual(baseline.frame['ops'], [animatedDefinition, textureDefinition]);
  assert.equal(baseline.remainingOutputs.length, 1);
  const remaining = baseline.remainingOutputs[0];
  assert.equal(remaining?.kind, 'frame');
  if (remaining?.kind !== 'frame') throw new Error('expected retained frame');
  assert.deepEqual(remaining.frame['ops'], [{ op: 'create', handle: 1, node: {} }]);
  assert.deepEqual(remaining.frame['publication'], {
    stream: 'voxel:test', baseRevision: 0, revision: 1, operationCount: 1,
  });
});

test('audio feedback claims the initial owner, retries without loss, and acknowledges only the submitted range', async () => {
  const reports: Array<Record<string, unknown>> = [];
  const acknowledgements: number[] = [];
  let resets = 0;
  let facts: Array<Record<string, unknown>> = [];
  const deferred: { resolve: ((value: unknown) => void) | null } = { resolve: null };
  const renderer = {
    audioRealizedFacts: () => ({ retainedFactCount: facts.length, evictedFactCount: 0, facts }),
    acknowledgeAudioRealizedFacts: (throughFactId: number) => {
      acknowledgements.push(throughFactId);
      facts = facts.filter((fact) => (fact['factId'] as number) > throughFactId);
      return true;
    },
    resetAudioRealizationOwner: () => { resets += 1; return true; },
  } as unknown as Parameters<typeof createProductBrowserAudioFeedbackReporter>[0]['renderer'];
  const reporter = createProductBrowserAudioFeedbackReporter({
    renderer,
    report: async (feedback) => {
      reports.push(feedback as unknown as Record<string, unknown>);
      if (reports.length === 1) {
        return { accepted: true as const, ...ACCEPTED_FAULT, runtime: feedback.runtime };
      }
      if (reports.length === 2) {
        return { accepted: false as const, ...RECOVERABLE_FAULT, runtime: feedback.runtime, diagnostic: 'retry' };
      }
      if (reports.length === 3) {
        return new Promise((resolve) => { deferred.resolve = resolve; }) as never;
      }
      const acceptedThroughFactId = feedback.facts.at(-1)?.factId;
      if (acceptedThroughFactId === undefined) return { accepted: true as const, ...ACCEPTED_FAULT, runtime: feedback.runtime };
      return {
        accepted: true as const,
        ...ACCEPTED_FAULT,
        runtime: feedback.runtime,
        acceptedThroughFactId,
      };
    },
    initialRuntime: AUDIO_RUNTIME,
  });

  // A remount with an identical binding must still claim the feedback owner;
  // later duplicate binding markers do not reset the renderer again.
  await reporter.flush();
  assert.equal((reports[0]!['replaceOwner']), true);
  facts.push({ kind: 'naturalCompletion', factId: 1, source: 'oneShot', sequence: 3, signalHandle: 11 });
  reporter.bindRuntime(AUDIO_RUNTIME);
  assert.equal(resets, 0);
  await reporter.flush();
  assert.deepEqual(acknowledgements, []);

  const inFlight = reporter.flush();
  facts.push({
    kind: 'diagnostic',
    factId: 2,
    diagnostic: { code: 'decodeFailed', sequence: 4, handle: null, message: 'test-only' },
  });
  assert.ok(deferred.resolve);
  deferred.resolve({ accepted: true, ...ACCEPTED_FAULT, runtime: AUDIO_RUNTIME, acceptedThroughFactId: '1' });
  await inFlight;
  assert.deepEqual(acknowledgements, [1]);
  assert.deepEqual((reports[1]!['facts'] as Array<Record<string, unknown>>).map((fact) => fact['factId']), ['1']);
  assert.deepEqual((reports[2]!['facts'] as Array<Record<string, unknown>>).map((fact) => fact['factId']), ['1']);

  await reporter.flush();
  assert.deepEqual((reports[3]!['facts'] as Array<Record<string, unknown>>).map((fact) => fact['factId']), ['2']);
  assert.deepEqual(acknowledgements, [1, 2]);
});

test('terminal feedback rejection preserves the candidate and fails the caller', async () => {
  const acknowledgements: number[] = [];
  const renderer = {
    audioRealizedFacts: () => ({ evictedFactCount: 0, facts: [{
      kind: 'naturalCompletion', factId: 4, source: 'oneShot', sequence: 1, signalHandle: 3,
    }] }),
    acknowledgeAudioRealizedFacts: (throughFactId: number) => { acknowledgements.push(throughFactId); return true; },
    resetAudioRealizationOwner: () => true,
  } as unknown as Parameters<typeof createProductBrowserAudioFeedbackReporter>[0]['renderer'];
  const reporter = createProductBrowserAudioFeedbackReporter({
    renderer,
    report: async (feedback) => ({
      accepted: false,
      code: 'CSHARP_LIFECYCLE_COUNTER_EXHAUSTED',
      disposition: 'terminal',
      runtime: feedback.runtime,
      diagnostic: 'counter exhausted',
    }),
    initialRuntime: AUDIO_RUNTIME,
  });
  await assert.rejects(reporter.flush(), /counter exhausted/u);
  assert.deepEqual(acknowledgements, []);
});

test('ghost plate feedback replaces an active snapshot with an empty snapshot after disposal', async () => {
  const reports: Array<{ readonly facts: readonly unknown[]; readonly replaceOwner: boolean }> = [];
  let plates: readonly unknown[] = [
    {
      handle: 9,
      sourceMatch: true,
      currentSector: 2,
      localAzimuthDegrees: 12,
      fallbackActive: false,
      fallbackReason: null,
      limitationMask: 125,
      preparationCpuMilliseconds: 1,
      captureCpuSubmissionMilliseconds: 2,
      retainedResourceCounts: { sectors: 4, meshes: 1, materials: 1, borrowedTextures: 0 },
    },
  ];
  const renderer = { ghostPlateReadout: () => ({ activePlates: plates.length, plates }) } as unknown as Parameters<typeof createProductBrowserGhostPlateFeedbackReporter>[0]['renderer'];
  const reporter = createProductBrowserGhostPlateFeedbackReporter({
    renderer,
    report: async (feedback) => {
      reports.push(feedback);
      return { accepted: true, ...ACCEPTED_FAULT, runtime: feedback.runtime };
    },
    initialRuntime: AUDIO_RUNTIME,
  });
  await reporter.flush();
  plates = [];
  await reporter.flush();
  assert.equal(reports.length, 2);
  assert.equal(reports[0]?.facts.length, 1);
  assert.equal(reports[1]?.facts.length, 0);
  assert.equal(reports[0]?.replaceOwner, true);
  assert.equal(reports[1]?.replaceOwner, false);
});

test('renderer diagnostics publishes only a newly accepted renderer submission', async () => {
  const reports: unknown[] = [];
  const observations: number[] = [];
  let renderSequence = 4;
  const renderer = {
    diagnosticsReadout: () => ({ schemaVersion: 1, submission: { renderSequence } }),
  } as unknown as Parameters<typeof createProductBrowserRendererDiagnosticsReporter>[0]['renderer'];
  const reporter = createProductBrowserRendererDiagnosticsReporter({
    renderer,
    report: async (feedback) => {
      reports.push(feedback.snapshot);
      return { accepted: true, ...ACCEPTED_FAULT, runtime: feedback.runtime };
    },
    initialRuntime: AUDIO_RUNTIME,
    onObservation: (sequence) => observations.push(sequence),
  });
  await reporter.flush();
  await reporter.flush();
  renderSequence = 5;
  await reporter.flush();
  assert.equal(reports.length, 2);
  assert.deepEqual(observations, [4, 5]);
});

test('audio feedback flush precedes every browser-host C# update admission lane', async () => {
  for (const operation of ['advance-realtime', 'admit-demand-step', 'admit-external-step']) {
    const order: string[] = [];
    await flushProductBrowserAudioFeedbackBeforeUpdate(
      async () => { order.push('feedback'); },
      async () => { order.push(operation); return operation; },
    );
    assert.deepEqual(order, ['feedback', operation]);
  }
});

test('realtime owner controls advancement without dropping typed cadence input', async () => {
  const input: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };

  const run = async (realtimeAdvanceOwner: 'browser' | 'rust-host') => {
    const inputBatches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
    const observedTimes: string[] = [];
    const failures: unknown[] = [];
    const cadence = createProductBrowserCadence({
      lifecycleMode: 'realtime',
      realtimeAdvanceOwner,
      isReady: () => true,
      enqueueOperation: (operation) => operation(),
      sampleInput: () => [input],
      sendInput: async (batch) => {
        inputBatches.push(batch);
      },
      advanceRealtime: async (observedTimeNs) => {
        observedTimes.push(observedTimeNs);
      },
      admitDemandStep: async () => undefined,
      onFailure: (cause) => {
        failures.push(cause);
      },
    });
    cadence.enqueue(16.5);
    await cadence.settle();
    cadence.dispose();
    return { inputBatches, observedTimes, failures };
  };

  const browser = await run('browser');
  assert.deepEqual(browser.inputBatches, [[input]]);
  assert.deepEqual(browser.observedTimes, ['16500000']);
  assert.deepEqual(browser.failures, []);

  const rustHost = await run('rust-host');
  assert.deepEqual(rustHost.inputBatches, [[input]]);
  assert.deepEqual(rustHost.observedTimes, []);
  assert.deepEqual(rustHost.failures, []);
});

test('Rust-host output pulse drains typed input without browser advancement', async () => {
  const input: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const batches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
  const advances: string[] = [];
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'rust-host',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => [input],
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => { advances.push(time); },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });
  cadence.pulseRustHost();
  await cadence.settle();
  cadence.dispose();
  assert.deepEqual(batches, [[input]]);
  assert.deepEqual(advances, []);
});

test('input availability wakes static realtime and demand admission without a second loop', async () => {
  const input: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    intent: 'fixture.regenerate',
    value: {
      kind: 'product-payload',
      contract: 'fixture.regenerate.v1',
      data: { seed: 7, preset: 'spread' },
    },
  };
  const run = async (
    lifecycleMode: 'realtime' | 'demand' | 'external',
    realtimeAdvanceOwner: 'browser' | 'rust-host' = 'browser',
  ) => {
    const batches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
    const advances: string[] = [];
    let demandSteps = 0;
    const cadence = createProductBrowserCadence({
      lifecycleMode,
      realtimeAdvanceOwner,
      isReady: () => true,
      enqueueOperation: (operation) => operation(),
      sampleInput: () => [input],
      sendInput: async (batch) => { batches.push(batch); },
      advanceRealtime: async (time) => { advances.push(time); },
      admitDemandStep: async () => { demandSteps += 1; },
      onFailure: (cause) => { assert.fail(String(cause)); },
    });
    cadence.pulseInput(25);
    await cadence.settle();
    cadence.dispose();
    return { batches, advances, demandSteps };
  };

  assert.deepEqual(await run('realtime'), { batches: [[input]], advances: ['25000000'], demandSteps: 0 });
  assert.deepEqual(await run('realtime', 'rust-host'), { batches: [[input]], advances: [], demandSteps: 0 });
  assert.deepEqual(await run('demand'), { batches: [[input]], advances: [], demandSteps: 1 });
  assert.deepEqual(await run('external'), { batches: [[input]], advances: [], demandSteps: 0 });
});

test('slow cadence preserves input pulse boundaries and chronological advancement', async () => {
  const pressed: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const released: RustyApplicationRuntimeInputEnvelope = {
    ...pressed,
    sequence: '2',
    fact: { kind: 'key', code: 'key-w', edge: 'released' },
  };
  const queued: RustyApplicationRuntimeInputEnvelope[] = [];
  const batches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
  const advances: string[] = [];
  let releaseFirstAdvance: () => void = () => undefined;
  const firstAdvance = new Promise<void>((resolve) => { releaseFirstAdvance = resolve; });
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => queued.splice(0),
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => {
      advances.push(time);
      if (advances.length === 1) await firstAdvance;
    },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });

  cadence.enqueue(10);
  queued.push(pressed);
  cadence.pulseInput(20);
  for (let time = 61; time <= 100; time += 1) cadence.enqueue(time);
  queued.push(released);
  cadence.pulseInput(120);
  releaseFirstAdvance();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(batches, [[pressed], [released]]);
  assert.deepEqual(advances, ['10000000', '20000000', '100000000', '120000000']);
});

test('cadence keeps an older same-frame RAF timestamp monotonic after an input wakeup', async () => {
  const pressed: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-a', edge: 'pressed' },
  };
  const queued: RustyApplicationRuntimeInputEnvelope[] = [];
  const advances: string[] = [];
  const batches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
  let releaseFirstAdvance: () => void = () => undefined;
  const firstAdvance = new Promise<void>((resolve) => { releaseFirstAdvance = resolve; });
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'browser',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => queued.splice(0),
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => {
      advances.push(time);
      if (advances.length === 1) await firstAdvance;
    },
    admitDemandStep: async () => undefined,
    onFailure: (cause) => { assert.fail(String(cause)); },
  });

  cadence.enqueue(6_720);
  queued.push(pressed);
  cadence.pulseInput(6_721.4);
  cadence.enqueue(6_721.1);
  releaseFirstAdvance();
  await cadence.settle();
  cadence.dispose();

  assert.deepEqual(batches, [[pressed]]);
  assert.deepEqual(advances, ['6720000000', '6721400000', '6721400000']);
});

test('transport rejects an adapter with an arbitrary or missing operation surface', () => {
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      lifecycle: undefined,
    } as never),
    /lifecycle must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      subscribeOutputs: undefined,
    } as never),
    /subscribeOutputs must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      reportAudioFeedback: undefined,
    } as never),
    /reportAudioFeedback must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      subscribeTerminalFailures: true,
    } as never),
    /subscribeTerminalFailures must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      admitDemandStep: true,
    } as never),
    /admitDemandStep must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      admitExternalStep: true,
    } as never),
    /admitExternalStep must be a function/u,
  );
});

test('bundle assets are fixed JS composition roots and descriptor bytes are reproducible', () => {
  const options = {
    engineHostModule: 'export const engineHost = true;\n',
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'demand' as const,
    uiProjection: {
      expectedStream: 'product.hud',
      expectedContract: 'product.hud.current',
    },
  };
  const first = productBrowserBundleAssets(options);
  const second = productBrowserBundleAssets(options);
  assert.deepEqual(first, second);
  assert.deepEqual(first.map((asset) => asset.name), [
    'index.html',
    'main.js',
    'bridge.js',
    PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE,
  ]);
  assert.equal(first.some((asset) => asset.name.endsWith('.ts')), false);
  assert.equal(first.some((asset) => asset.content.includes('globalThis.__rustyProductBrowserHost')), false);
  assert.equal(first.some((asset) => asset.content.includes('product.ui.v1')), false);
  assert.match(first[0]!.content, /main\.js/u);
  assert.match(first[1]!.content, /\.\/engine\/product-browser-host\.js/u);
  assert.match(first[1]!.content, /initialInteractionMode: 'gameplay'/u);
  assert.match(first[1]!.content, /renderer-preload\.json/u);
  assert.match(first[1]!.content, /renderer: \{ initialContent: rendererInitialContent \}/u);
  assert.match(first[1]!.content, /mountProductBrowserHost, rendererResourceContentHash/u);
  assert.match(first[1]!.content, /await rendererResourceContentHash\(data, resource\.contentHash\)/u);
  assert.match(first[1]!.content, /resource\.contentHash !== digest/u);
  assert.doesNotMatch(first[1]!.content, /crypto\.subtle/u);
  assert.match(first[1]!.content, /PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_COUNT/u);
  assert.match(first[1]!.content, /PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_TOTAL_BYTES/u);
  assert.match(first[1]!.content, /PRODUCT_RENDERER_PRELOAD_MESH_MAX_TOTAL_BYTES/u);
  assert.match(first[1]!.content, /application\/octet-stream/u);
  assert.match(first[1]!.content, /hasMeshResourceHeader/u);
  assert.match(first[1]!.content, /version !== 49 && version !== 50 && version !== 51/u);
  assert.match(first[1]!.content, /new TextEncoder\(\)\.encode\(path\)\.byteLength <= 512/u);
  assert.match(first[1]!.content, /!path\.includes\('%'\)/u);
  assert.match(first[1]!.content, /realtimeAdvanceOwner: bridge\.realtimeAdvanceOwner/u);
  assert.match(first[2]!.content, /\.\/engine\/product-browser-host\.js/u);
  assert.equal(first[3]!.content, options.engineHostModule);
  assert.match(first[2]!.content, /lifecycleMode: "demand"/u);
  assert.match(first[2]!.content, /realtimeAdvanceOwner: "browser"/u);
  assert.match(first[2]!.content, /createProductBrowserLocalHttpAdapter/u);
  assert.match(first[2]!.content, /PRODUCT_RUNTIME_HTTP_BASE_PATH/u);
  const descriptor = productBrowserBundleDescriptor(options);
  assert.equal(descriptor.artifact, 'rusty.product.bundle');
  assert.deepEqual(
    descriptor.files.map((file) => [file.name, file.utf8Bytes]),
    first.map((file) => [file.name, new TextEncoder().encode(file.content).byteLength]),
  );
});

test('bundle path and identity admission is fail-closed', () => {
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: '../ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
    }),
    /must not escape/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
      uiProjection: { expectedStream: 'product hud', expectedContract: 'product.hud' },
    }),
    /bounded product identity/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: "export { mountProductBrowserHost } from '@rusty-engine/product-browser-host';\n",
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
    }),
    /bare Engine package imports/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'demand',
      realtimeAdvanceOwner: 'rust-host',
    }),
    /requires realtime lifecycle mode/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
      realtimeAdvanceOwner: 'unknown' as never,
    }),
    /realtimeAdvanceOwner must be browser or rust-host/u,
  );
  assert.equal(ProductBrowserHostError.prototype.name, 'Error');
});

test('packaged Rust-host realtime ownership propagates through the generated bridge', () => {
  const assets = productBrowserBundleAssets({
    engineHostModule: 'export const engineHost = true;\n',
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'rust-host',
  });
  assert.match(assets[1]!.content, /realtimeAdvanceOwner: bridge\.realtimeAdvanceOwner/u);
  assert.match(assets[2]!.content, /realtimeAdvanceOwner: "rust-host"/u);
});
