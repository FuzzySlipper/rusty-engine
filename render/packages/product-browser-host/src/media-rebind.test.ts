import assert from 'node:assert/strict';
import test from 'node:test';

import { audioHandle } from '@rusty-engine/render-contracts';
import {
  RendererAudioHost,
  RendererPresentationHostSet,
  RendererVideoHost,
} from '@rusty-engine/renderer-host';

import {
  createProductBrowserAudioFeedbackReporter,
  createProductBrowserVideoFeedbackReporter,
} from './product-browser-host.js';

// Minimal Web Audio and <video> stand-ins; the hosts, host set and
// reporters under test are the real implementations.
class FakeParam { value = 0; setValueAtTime(value: number): void { this.value = value; } }
class FakeNode {
  connect<T>(destination: T): T { return destination; }
  disconnect(): void {}
}
class FakeGain extends FakeNode { gain = new FakeParam(); }
class FakeStereoPanner extends FakeNode { pan = new FakeParam(); }
class FakePanner extends FakeNode {
  distanceModel = 'inverse'; maxDistance = 0; panningModel = 'HRTF'; refDistance = 0; rolloffFactor = 0;
  positionX = new FakeParam(); positionY = new FakeParam(); positionZ = new FakeParam();
}
class FakeSource extends FakeNode {
  buffer: unknown = null; loop = false; onended: (() => void) | null = null;
  playbackRate = new FakeParam(); stopped = false;
  start(): void {}
  stop(): void { this.stopped = true; }
}
class FakeContext {
  currentTime = 2;
  destination = new FakeNode();
  listener = {
    forwardX: new FakeParam(), forwardY: new FakeParam(), forwardZ: new FakeParam(),
    positionX: new FakeParam(), positionY: new FakeParam(), positionZ: new FakeParam(),
    upX: new FakeParam(), upY: new FakeParam(), upZ: new FakeParam(),
  };
  state = 'running';
  sources: FakeSource[] = [];
  async close(): Promise<void> { this.state = 'closed'; }
  async resume(): Promise<void> { this.state = 'running'; }
  createBufferSource(): FakeSource { const source = new FakeSource(); this.sources.push(source); return source; }
  createGain(): FakeGain { return new FakeGain(); }
  createPanner(): FakePanner { return new FakePanner(); }
  createStereoPanner(): FakeStereoPanner { return new FakeStereoPanner(); }
  async decodeAudioData(): Promise<unknown> { return { duration: 2 }; }
}
class FakeVideo {
  style = { cssText: '' }; src = ''; removed = false; paused = false;
  addEventListener(): void {}
  play(): Promise<void> { return Promise.resolve(); }
  pause(): void { this.paused = true; }
  removeAttribute(): void {}
  load(): void {}
  remove(): void { this.removed = true; }
}

const running = { instanceId: '7', generation: '1', controlRevision: '2' } as const;
const paused = { ...running, controlRevision: '3' } as const;
const restarted = { ...running, generation: '2', controlRevision: '4' } as const;

void test('a control-revision rebind keeps retained audio and video; a new generation resets them', async () => {
  const context = new FakeContext();
  const audio = new RendererAudioHost({
    createContext: () => context as never,
    resolveEntityPosition: () => [0, 0, 0],
    resolveResource: async (clip) => ({ bytes: new Uint8Array([1, 2, 3, 4]).buffer, contentHash: clip.contentHash }),
  });
  const hosts = new RendererPresentationHostSet({ audio });
  const audioReporter = createProductBrowserAudioFeedbackReporter({
    renderer: {
      resetAudioRealizationOwner: () => hosts.resetAudioRealizationOwner(),
      audioRealizedFacts: () => hosts.readAudioRealizedFacts(),
      acknowledgeAudioRealizedFacts: (id: number) => hosts.acknowledgeAudioRealizedFacts(id),
    } as never,
    initialRuntime: running,
    report: async () => ({ accepted: true, runtime: paused }) as never,
  });
  await audio.applyPresentation({
    schemaVersion: 1,
    ops: [{
      domain: 'audio', meta: { sequence: 0 },
      op: {
        op: 'create', handle: audioHandle(44),
        descriptor: {
          clip: { asset: 'audio/theme', contentHash: 'sha256:theme', durationSeconds: 2 },
          bus: 'music', volume: 0.8, pitch: 1, looping: true, spatialBlend: 0,
          attenuation: 24, pan: 0, emitter: { kind: 'global2d' },
        },
      },
    }],
  } as never);
  const element = new FakeVideo();
  const video = new RendererVideoHost({
    container: { append(): void {} } as never,
    resolveResource: async (clip) => ({ bytes: new Uint8Array([1, 2, 3]).buffer, contentHash: clip.contentHash, mediaType: 'video/webm' }),
    createVideoElement: () => element as never,
  });
  const videoReporter = createProductBrowserVideoFeedbackReporter({
    renderer: {
      resetVideoRealizationOwner: () => { video.reset(); return true; },
      videoRealizedFacts: () => video.realizedFacts(),
      acknowledgeVideoRealizedFacts: (id: number) => video.acknowledgeRealizedFacts(id),
    } as never,
    report: async () => ({ accepted: true, runtime: paused }) as never,
  });
  videoReporter.bindRuntime(running);
  await video.applyPresentation({
    schemaVersion: 1,
    ops: [{ domain: 'video', meta: { sequence: 0 }, op: { op: 'play', handle: 1, clip: { asset: 'video/intro', contentHash: 'sha256:video', mediaType: 'video/webm' } } }],
  } as never);
  assert.equal(audio.readout().activeSources, 1);

  // Pause, resume, remap and control replace move only the control revision.
  audioReporter.bindRuntime(paused);
  videoReporter.bindRuntime(paused);
  assert.equal(audio.readout().activeSources, 1, 'the retained voice keeps playing');
  assert.equal(context.sources[0]!.stopped, false);
  const update = await audio.applyPresentation({
    schemaVersion: 1,
    ops: [{ domain: 'audio', meta: { sequence: 0 }, op: { op: 'update', handle: audioHandle(44), patch: { volume: 0.5 } } }],
  } as never);
  assert.equal(update.applied, 1, 'later updates still reach the retained voice');
  assert.equal(element.removed, false, 'the active video element survives');
  assert.equal(element.paused, false);

  // A new generation is a replacement incarnation: owners reset.
  audioReporter.bindRuntime(restarted);
  videoReporter.bindRuntime(restarted);
  assert.equal(audio.readout().activeSources, 0);
  assert.equal(context.sources[0]!.stopped, true);
  assert.equal(element.removed, true);

  await audio.dispose();
  video.dispose();
});
