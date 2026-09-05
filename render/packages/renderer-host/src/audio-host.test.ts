import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  audioSignalHandle,
  audioHandle,
  type AudioSourceDescriptor,
  type PresentationFrameDiff,
  type PresentationOp,
} from '@rusty-engine/render-contracts';
import {
  RendererAudioHost,
  type RendererAudioContext,
} from './audio-host.js';
import { RendererPresentationHostSet } from './presentation-host-set.js';

const FIXTURE_AUDIO_HASH = '9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a';
const FIXTURE_FNV_AUDIO_HASH = 'be7a5e775165785d';

class FakeParam {
  value = 0;
  readonly writes: number[] = [];

  setValueAtTime(value: number): void {
    this.value = value;
    this.writes.push(value);
  }
}

class FakeNode {
  readonly connections: FakeNode[] = [];
  disconnected = false;

  connect(destination: FakeNode): FakeNode {
    this.connections.push(destination);
    return destination;
  }

  disconnect(): void {
    this.disconnected = true;
  }
}

class FakeGain extends FakeNode {
  readonly gain = new FakeParam();
}

class FakeStereoPanner extends FakeNode {
  readonly pan = new FakeParam();
}

class FakePanner extends FakeNode {
  distanceModel: DistanceModelType = 'inverse';
  maxDistance = 0;
  panningModel: PanningModelType = 'HRTF';
  refDistance = 0;
  rolloffFactor = 0;
  readonly positionX = new FakeParam();
  readonly positionY = new FakeParam();
  readonly positionZ = new FakeParam();
}

class FakeListener {
  readonly forwardX = new FakeParam();
  readonly forwardY = new FakeParam();
  readonly forwardZ = new FakeParam();
  readonly positionX = new FakeParam();
  readonly positionY = new FakeParam();
  readonly positionZ = new FakeParam();
  readonly upX = new FakeParam();
  readonly upY = new FakeParam();
  readonly upZ = new FakeParam();
}

class FakeSource extends FakeNode {
  buffer: unknown = null;
  loop = false;
  onended: (() => void) | null = null;
  readonly playbackRate = new FakeParam();
  readonly starts: Array<{ readonly when: number | undefined; readonly offset: number | undefined }> = [];
  started = false;
  stopped = false;

  start(when?: number, offset?: number): void {
    this.started = true;
    this.starts.push({ when, offset });
  }

  stop(): void {
    this.stopped = true;
  }

  endNaturally(): void {
    this.onended?.();
  }
}

class FakeContext {
  currentTime = 2;
  readonly destination = new FakeNode();
  readonly listener = new FakeListener();
  state: AudioContextState = 'suspended';
  readonly gains: FakeGain[] = [];
  readonly panners: FakePanner[] = [];
  readonly stereoPanners: FakeStereoPanner[] = [];
  readonly sources: FakeSource[] = [];
  decodeCount = 0;
  closed = false;
  blockResume = false;

  async close(): Promise<void> {
    this.closed = true;
    this.state = 'closed';
  }

  createBufferSource(): FakeSource {
    const source = new FakeSource();
    this.sources.push(source);
    return source;
  }

  createGain(): FakeGain {
    const gain = new FakeGain();
    this.gains.push(gain);
    return gain;
  }

  createPanner(): FakePanner {
    const panner = new FakePanner();
    this.panners.push(panner);
    return panner;
  }

  createStereoPanner(): FakeStereoPanner {
    const panner = new FakeStereoPanner();
    this.stereoPanners.push(panner);
    return panner;
  }

  async decodeAudioData(): Promise<unknown> {
    this.decodeCount += 1;
    return { decoded: true, duration: 2 };
  }

  async resume(): Promise<void> {
    if (!this.blockResume) {
      this.state = 'running';
    }
  }
}

interface Deferred<T> {
  readonly promise: Promise<T>;
  resolve(value: T): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => { resolve = accept; });
  return { promise, resolve };
}

class DeferredDecodeContext extends FakeContext {
  readonly decodeStarted = deferred<void>();
  readonly decoded = deferred<unknown>();

  override async decodeAudioData(): Promise<unknown> {
    this.decodeCount += 1;
    this.decodeStarted.resolve();
    return this.decoded.promise;
  }
}

function descriptor(
  emitter: AudioSourceDescriptor['emitter'] = {
    kind: 'world3d',
    position: [1, 2, 3],
  },
): AudioSourceDescriptor {
  return {
    clip: { asset: 'audio/primary-fire-pulse', contentHash: FIXTURE_AUDIO_HASH, durationSeconds: 2 },
    bus: 'sfx',
    volume: 0.8,
    pitch: 1,
    looping: false,
    spatialBlend: 1,
    attenuation: 24,
    pan: 0.2,
    emitter,
  };
}

function operation(
  sequence: number,
  op: Extract<PresentationOp, { readonly domain: 'audio' }>['op'],
): PresentationOp {
  return {
    domain: 'audio',
    meta: { sequence },
    op,
  };
}

function frame(ops: readonly PresentationOp[]): PresentationFrameDiff {
  return { schemaVersion: 1, ops };
}

function host(context: FakeContext): RendererAudioHost {
  return new RendererAudioHost({
    createContext: () => context as unknown as RendererAudioContext,
    resolveEntityPosition: () => [10, 11, 12],
    resolveResource: async (clip) => ({
      bytes: new Uint8Array([1, 2, 3, 4]).buffer,
      contentHash: clip.contentHash,
    }),
  });
}

void test('Web Audio host emits catalog-hash-bound 3D cues and caches decoded clips', async () => {
  const context = new FakeContext();
  const audio = host(context);
  assert.deepEqual(await audio.resume(), []);
  assert.deepEqual(audio.updateListener({
    position: [4, 5, 6],
    forward: [0, 0, -1],
    up: [0, 1, 0],
  }), []);

  const receipt = await audio.applyPresentation(
    frame([
      operation(0, {
        op: 'emit',
        signalHandle: audioSignalHandle(11),
        signalId: 'shot:44',
        descriptor: descriptor(),
      }),
      operation(1, {
        op: 'emit',
        signalHandle: audioSignalHandle(12),
        signalId: 'impact:44',
        descriptor: descriptor(),
      }),
    ]),
  );

  assert.equal(receipt.applied, 2);
  assert.equal(receipt.readout.emittedSignals, 2);
  assert.equal(receipt.readout.cachedClips, 1);
  assert.equal(context.decodeCount, 1);
  assert.equal(context.sources.every((source) => source.started), true);
  assert.equal(context.panners.length, 2);
  assert.deepEqual(context.panners[0]?.positionX.writes, [1]);
  assert.deepEqual(context.listener.positionX.writes, [4]);
  assert.deepEqual(context.listener.forwardZ.writes, [-1]);
  assert.equal(context.panners[0]?.maxDistance, 24);
  assert.equal(context.panners[0]?.panningModel, 'equalpower');
  assert.deepEqual(receipt.diagnostics, []);

  const repeated = await audio.applyPresentation(
    frame([
      operation(0, {
        op: 'emit',
        signalHandle: audioSignalHandle(11),
        signalId: 'shot:44',
        descriptor: descriptor(),
      }),
    ]),
  );
  assert.equal(repeated.applied, 1);
  assert.equal(repeated.readout.emittedSignals, 2);
  assert.equal(context.sources.length, 2, 're-reading a frame does not replay a one-shot signal');
});

void test('audio realization facts use numeric correlations and reject stale completion callbacks', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const voice = audioHandle(44);
  await audio.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalHandle: audioSignalHandle(101),
      signalId: 'idempotency-only',
      descriptor: descriptor(),
    }),
    operation(1, { op: 'create', handle: voice, descriptor: descriptor() }),
  ]));

  context.sources[0]?.endNaturally();
  context.sources[1]?.endNaturally();
  assert.deepEqual(audio.realizedFacts().facts, [
    {
      kind: 'naturalCompletion', factId: 1, source: 'oneShot', sequence: 0,
      signalHandle: audioSignalHandle(101),
    },
    {
      kind: 'naturalCompletion', factId: 2, source: 'retainedVoice', sequence: 1, handle: voice,
    },
  ]);
  const ownerDestroy = await audio.applyPresentation(frame([
    operation(0, { op: 'destroy', handle: voice }),
  ]));
  assert.equal(ownerDestroy.applied, 1, 'natural end keeps the retained voice owner alive');
  assert.equal(audio.realizedFacts().facts.length, 2);

  const replacement = audioHandle(45);
  await audio.applyPresentation(frame([
    operation(0, { op: 'create', handle: replacement, descriptor: descriptor() }),
  ]));
  const staleEnded = context.sources[2]?.onended;
  await audio.applyPresentation(frame([
    operation(0, { op: 'destroy', handle: replacement }),
  ]));
  staleEnded?.();
  assert.equal(audio.realizedFacts().facts.length, 2, 'destroy must not report completion');

  const paused = audioHandle(46);
  await audio.applyPresentation(frame([
    operation(0, { op: 'create', handle: paused, descriptor: descriptor() }),
  ]));
  const pausedEnded = context.sources[3]?.onended;
  await audio.applyPresentation(frame([
    operation(0, { op: 'voiceControl', handle: paused, control: 'pause' }),
  ]));
  pausedEnded?.();
  assert.equal(audio.realizedFacts().facts.length, 2, 'pause must not report completion');

  const retriggered = audioHandle(47);
  await audio.applyPresentation(frame([
    operation(0, { op: 'create', handle: retriggered, descriptor: descriptor() }),
  ]));
  const retriggeredEnded = context.sources[4]?.onended;
  await audio.applyPresentation(frame([
    operation(0, { op: 'voiceControl', handle: retriggered, control: 'retrigger' }),
  ]));
  retriggeredEnded?.();
  assert.equal(audio.realizedFacts().facts.length, 2, 'retrigger replacement must suppress old completion');
  const disposedEnded = context.sources[5]?.onended;
  await audio.dispose();
  disposedEnded?.();
  assert.equal(audio.realizedFacts().facts.length, 0, 'dispose clears owner facts and suppresses callbacks');

  audio.reset();
  staleEnded?.();
  assert.deepEqual(audio.realizedFacts(), {
    retainedFactCount: 0, evictedFactCount: 0, facts: [],
  });
});

void test('audio realization and diagnostic retention are bounded while fact IDs stay monotonic', async () => {
  const context = new FakeContext();
  const audio = new RendererAudioHost({
    createContext: () => context as unknown as RendererAudioContext,
    maxRetainedFacts: 1,
    maxRetainedDiagnostics: 1,
    resolveResource: async () => { throw new Error('fixture audio resource unavailable'); },
  });
  await audio.applyPresentation(frame([
    operation(0, {
      op: 'create', handle: audioHandle(1), descriptor: descriptor(),
    }),
    operation(1, {
      op: 'create', handle: audioHandle(2), descriptor: descriptor(),
    }),
  ]));

  assert.equal(audio.readout().retainedDiagnosticCount, 1);
  assert.equal(audio.readout().evictedDiagnosticCount, 1);
  assert.deepEqual(audio.realizedFacts(), {
    retainedFactCount: 1,
    evictedFactCount: 1,
    facts: [{ kind: 'diagnostic', factId: 2, diagnostic: audio.readout().diagnostics[0] }],
  });
  audio.acknowledgeRealizedFacts(1);
  assert.equal(audio.realizedFacts().retainedFactCount, 1, 'acknowledgement preserves later facts');
  audio.acknowledgeRealizedFacts(2);
  assert.equal(audio.realizedFacts().retainedFactCount, 0);
  assert.equal(audio.realizedFacts().evictedFactCount, 1);
});

void test('listener updates after disposal retain a diagnostic without writing Web Audio state', async () => {
  const context = new FakeContext();
  const audio = host(context);
  await audio.dispose();

  const diagnostics = audio.updateListener({
    position: [4, 5, 6], forward: [0, 0, -1], up: [0, 1, 0],
  });
  audio.updateListener({ position: [4, 5, 6], forward: [0, 0, -1], up: [0, 1, 0] });
  assert.equal(diagnostics[0]?.code, 'hostFailure');
  assert.deepEqual(context.listener.positionX.writes, []);
  assert.equal(audio.readout().retainedDiagnosticCount, 1);
  assert.equal(audio.realizedFacts().retainedFactCount, 1);
  assert.equal(audio.readout().diagnostics.at(-1)?.message, 'audio host is disposed');
});

void test('retained 2D/3D sources create update destroy and clean up independently', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const handle = audioHandle(7);
  const receipt = await audio.applyPresentation(
    frame([
      operation(0, {
        op: 'create',
        handle,
        descriptor: { ...descriptor({ kind: 'global2d' }), looping: true, bus: 'ambient' },
      }),
      operation(1, {
        op: 'update',
        handle,
        patch: {
          volume: 0.25,
          pitch: 1.5,
          looping: true,
          spatialBlend: null,
          attenuation: null,
          pan: -0.5,
          emitter: { kind: 'entityAttached', entity: 5 as never, offset: [1, 0, -1] },
        },
      }),
      operation(2, { op: 'destroy', handle }),
    ]),
  );

  assert.equal(receipt.applied, 3);
  assert.equal(receipt.readout.activeSources, 0);
  assert.equal(context.sources.length, 2, 'emitter-mode update rebuilds the node graph');
  assert.equal(context.sources.every((source) => source.stopped), true);
  assert.deepEqual(context.panners[0]?.positionX.writes, [11]);
});

void test('retained voice controls retain logical state and reconstruct sources at the correct cursor', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const handle = audioHandle(31);
  await audio.applyPresentation(frame([
    operation(0, {
      op: 'create',
      handle,
      descriptor: { ...descriptor(), looping: true, pitch: 1.5 },
    }),
  ]));
  assert.deepEqual(context.sources[0]?.starts, [{ when: 0, offset: 0 }]);

  context.currentTime = 4;
  const paused = await audio.applyPresentation(frame([
    operation(1, { op: 'voiceControl', handle, control: 'pause' }),
    operation(2, { op: 'voiceControl', handle, control: 'pause' }),
    operation(3, {
      op: 'update',
      handle,
      patch: {
        volume: null,
        pitch: null,
        looping: null,
        spatialBlend: null,
        attenuation: null,
        pan: null,
        emitter: { kind: 'global2d' },
      },
    }),
  ]));
  assert.equal(paused.applied, 3);
  assert.equal(context.sources.length, 1, 'paused emitter replacement does not start a source');
  assert.equal(context.sources[0]?.stopped, true);

  const resumed = await audio.applyPresentation(frame([
    operation(4, { op: 'voiceControl', handle, control: 'resume' }),
    operation(5, { op: 'voiceControl', handle, control: 'resume' }),
  ]));
  assert.equal(resumed.applied, 2);
  assert.equal(context.sources.length, 2, 'resume is idempotent after rebuilding the source once');
  assert.deepEqual(context.sources[1]?.starts, [{ when: 0, offset: 1 }]);
  assert.equal(context.panners.length, 1, 'the resumed graph uses the paused replacement emitter');

  const retriggered = await audio.applyPresentation(frame([
    operation(6, { op: 'voiceControl', handle, control: 'retrigger' }),
  ]));
  assert.equal(retriggered.applied, 1);
  assert.equal(context.sources[1]?.stopped, true);
  assert.deepEqual(context.sources[2]?.starts, [{ when: 0, offset: 0 }]);
});

void test('restore realizes the Engine cursor without restarting paused or completed voices', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const playing = audioHandle(39);
  const paused = audioHandle(40);
  const completed = audioHandle(41);
  const receipt = await audio.applyPresentation(frame([
    operation(0, {
      op: 'restore', handle: playing, descriptor: { ...descriptor(), looping: true },
      desiredState: 'playing', cursorSeconds: 0.75,
    }),
    operation(1, {
      op: 'restore', handle: paused, descriptor: descriptor(),
      desiredState: 'paused', cursorSeconds: 2,
    }),
    operation(2, {
      op: 'restore', handle: completed, descriptor: descriptor(),
      desiredState: 'playing', cursorSeconds: 99,
    }),
  ]));

  assert.equal(receipt.applied, 3);
  assert.equal(receipt.readout.activeSources, 3);
  assert.equal(context.sources.length, 2, 'only playing and decoded past-end voices allocate graphs');
  assert.deepEqual(context.sources[0]?.starts, [{ when: 0, offset: 0.75 }]);
  assert.equal(context.sources[1]?.started, false, 'a past-end restore must not restart');
  assert.equal(context.sources[1]?.stopped, true, 'the temporary completed graph is disposed');
  assert.equal(audio.realizedFacts().facts.length, 0, 'restore does not synthesize completion feedback');
});

void test('reset and disposal fence pending decode before an old graph can enter a new owner', async () => {
  const resetContext = new DeferredDecodeContext();
  const resetAudio = host(resetContext);
  const pendingReset = resetAudio.applyPresentation(frame([
    operation(0, { op: 'create', handle: audioHandle(61), descriptor: descriptor() }),
  ]));
  await resetContext.decodeStarted.promise;
  resetAudio.reset();
  resetContext.decoded.resolve({ decoded: true, duration: 2 });
  const resetReceipt = await pendingReset;
  assert.equal(resetReceipt.applied, 0);
  assert.equal(resetContext.sources.length, 0, 'stale decode must not allocate a source graph');
  assert.deepEqual(resetAudio.readout(), {
    activeSources: 0, cachedClips: 1, emittedSignals: 0,
    retainedDiagnosticCount: 0, evictedDiagnosticCount: 0, diagnostics: [],
  });
  assert.deepEqual(resetAudio.realizedFacts().facts, []);

  const disposeContext = new DeferredDecodeContext();
  const disposeAudio = host(disposeContext);
  const pendingDispose = disposeAudio.applyPresentation(frame([
    operation(0, { op: 'emit', signalHandle: audioSignalHandle(62), signalId: 'pending:62', descriptor: descriptor() }),
  ]));
  await disposeContext.decodeStarted.promise;
  await disposeAudio.dispose();
  disposeContext.decoded.resolve({ decoded: true, duration: 2 });
  const disposeReceipt = await pendingDispose;
  assert.equal(disposeReceipt.applied, 0);
  assert.equal(disposeContext.sources.length, 0, 'disposed host must reject its delayed graph');
  assert.deepEqual(disposeAudio.realizedFacts().facts, []);
});

void test('fixed bus controls own existing and future graph gain state', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const handle = audioHandle(32);
  assert.deepEqual(context.gains.slice(0, 3).map((gain) => gain.gain.writes), [[1], [1], [1]]);

  const receipt = await audio.applyPresentation(frame([
    operation(0, { op: 'busControl', bus: 'sfx', control: { kind: 'setVolume', volume: 0.25 } }),
    operation(1, { op: 'busControl', bus: 'sfx', control: { kind: 'setMuted', muted: true } }),
    operation(2, {
      op: 'create',
      handle,
      descriptor: descriptor(),
    }),
    operation(3, { op: 'busControl', bus: 'sfx', control: { kind: 'setMuted', muted: false } }),
  ]));
  assert.equal(receipt.applied, 4);
  assert.deepEqual(context.gains[0]?.gain.writes, [1, 0.25, 0, 0.25]);
  assert.equal(context.sources[0]?.connections.includes(context.stereoPanners[0]!), true);
  assert.equal(context.gains[4]?.connections.includes(context.gains[0]!), true);
});

void test('retained entity-attached audio follows scene movement without descriptor updates', async () => {
  const context = new FakeContext();
  let entityPosition: readonly [number, number, number] | null = [10, 11, 12];
  const audio = new RendererAudioHost({
    createContext: () => context as unknown as RendererAudioContext,
    resolveEntityPosition: () => entityPosition,
    resolveResource: async (clip) => ({
      bytes: new Uint8Array([1, 2, 3, 4]).buffer,
      contentHash: clip.contentHash,
    }),
  });
  const handle = audioHandle(9);
  await audio.applyPresentation(frame([
    operation(0, {
      op: 'create',
      handle,
      descriptor: {
        ...descriptor({ kind: 'entityAttached', entity: 5 as never, offset: [1, 0, -1] }),
        looping: true,
      },
    }),
  ]));
  assert.deepEqual(context.panners[0]?.positionX.writes, [11]);

  entityPosition = [20, 21, 22];
  const refreshed = audio.refreshLayout();

  assert.deepEqual(context.panners[0]?.positionX.writes, [11, 21]);
  assert.deepEqual(context.panners[0]?.positionY.writes, [11, 21]);
  assert.deepEqual(context.panners[0]?.positionZ.writes, [11, 21]);
  assert.equal(audio.readout().activeSources, 1);
  assert.deepEqual(refreshed, []);

  entityPosition = null;
  const missing = audio.refreshLayout();
  assert.equal(missing[0]?.code, 'hostFailure');
  assert.equal(missing[0]?.handle, handle);
  assert.equal(audio.readout().activeSources, 1);
});

void test('missing audio host returns an explicit typed domain diagnostic', async () => {
  const receipt = await new RendererPresentationHostSet({}).apply(
    frame([
      operation(0, {
        op: 'emit',
        signalHandle: audioSignalHandle(11),
        signalId: 'shot:44',
        descriptor: descriptor(),
      }),
    ]),
  );

  const audioReceipt = receipt.domains.find((domain) => domain.domain === 'audio');
  assert.equal(audioReceipt?.applied, 0);
  assert.equal(audioReceipt?.configured, false);
  assert.equal(audioReceipt?.diagnostics[0]?.code, 'unavailableHost');
  assert.equal(audioReceipt?.diagnostics[0]?.sequence, 0);
});

void test('audio host hashes resolved bytes before decode and reports catalog drift', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const badDescriptor = {
    ...descriptor(),
    clip: {
      asset: 'audio/primary-fire-pulse',
      contentHash: '0'.repeat(64),
    },
  };
  const receipt = await audio.applyPresentation(
    frame([
      operation(0, {
        op: 'emit',
        signalHandle: audioSignalHandle(13),
        signalId: 'bad-hash',
        descriptor: badDescriptor,
      }),
    ]),
  );

  assert.equal(receipt.applied, 0);
  assert.equal(receipt.diagnostics[0]?.code, 'contentHashMismatch');
  assert.equal(receipt.readout.cachedClips, 0);
  assert.equal(receipt.readout.emittedSignals, 0);
  assert.equal(context.decodeCount, 0);
});

void test('audio host accepts a manifest-native FNV content hash', async () => {
  const context = new FakeContext();
  const audio = host(context);
  const receipt = await audio.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalHandle: audioSignalHandle(14),
      signalId: 'fnv-audio',
      descriptor: {
        ...descriptor(),
        clip: {
          asset: 'audio/primary-fire-pulse',
          contentHash: FIXTURE_FNV_AUDIO_HASH,
        },
      },
    }),
  ]));

  assert.equal(receipt.applied, 1);
  assert.deepEqual(receipt.diagnostics, []);
  assert.equal(context.decodeCount, 1);
});

void test('missing audio resources fail locally with typed operation diagnostics', async () => {
  const context = new FakeContext();
  const audio = new RendererAudioHost({
    createContext: () => context as unknown as RendererAudioContext,
    resolveResource: async () => {
      throw new Error('fixture audio resource unavailable');
    },
  });
  const receipt = await audio.applyPresentation(frame([
    operation(0, {
      op: 'emit',
      signalHandle: audioSignalHandle(15),
      signalId: 'missing-audio:44',
      descriptor: descriptor(),
    }),
  ]));

  assert.equal(receipt.applied, 0);
  assert.equal(receipt.diagnostics[0]?.code, 'hostFailure');
  assert.equal(receipt.diagnostics[0]?.sequence, 0);
  assert.equal(receipt.readout.emittedSignals, 0);
  assert.equal(receipt.readout.activeSources, 0);
});

void test('blocked AudioContext and malformed frame return explicit failures', async () => {
  const context = new FakeContext();
  context.blockResume = true;
  const audio = host(context);
  const diagnostics = await audio.resume();
  assert.equal(diagnostics[0]?.code, 'audioContextBlocked');

  const set = new RendererPresentationHostSet({ audio });
  await assert.rejects(
    set.apply(
      frame([
        operation(1, {
          op: 'emit',
          signalHandle: audioSignalHandle(16),
          signalId: 'bad-sequence',
          descriptor: descriptor(),
        }),
      ]),
    ),
    /ordered index 0/,
  );
  assert.equal(context.sources.length, 0, 'malformed framing rejects before host effects');
});
