import { test } from 'node:test';
import assert from 'node:assert/strict';

import { videoPlaybackHandle, type PresentationFrameDiff, type VideoProjectionOp } from '@rusty-engine/render-contracts';
import { RendererVideoHost } from './video-host.js';

class FakeVideo {
  autoplay = false;
  controls = true;
  playsInline = false;
  readonly style = { cssText: '' };
  src = '';
  readonly #listeners = new Map<string, (() => void)[]>();
  paused = false;
  removed = false;
  addEventListener(type: string, listener: () => void): void { this.#listeners.set(type, [...(this.#listeners.get(type) ?? []), listener]); }
  emit(type: string): void { for (const listener of this.#listeners.get(type) ?? []) listener(); }
  play(): Promise<void> { return Promise.resolve(); }
  pause(): void { this.paused = true; }
  removeAttribute(): void {}
  load(): void {}
  remove(): void { this.removed = true; }
}

const frame = (op: VideoProjectionOp): PresentationFrameDiff => ({ schemaVersion: 1, ops: [{ domain: 'video', meta: { sequence: 0 }, op }] });

test('video host records completion, removes its media element, and ignores stale replacement events', async () => {
  const videos: FakeVideo[] = [];
  const children: FakeVideo[] = [];
  const host = new RendererVideoHost({
    container: { append: (child: FakeVideo) => { children.push(child); } } as unknown as HTMLElement,
    resolveResource: async (clip) => ({ bytes: new Uint8Array([1, 2, 3]).buffer, contentHash: clip.contentHash, mediaType: 'video/webm' }),
    createVideoElement: () => { const video = new FakeVideo(); videos.push(video); return video as unknown as HTMLVideoElement; },
  });
  const first = videoPlaybackHandle(1);
  const second = videoPlaybackHandle(2);
  await host.applyPresentation(frame({ op: 'play', handle: first, clip: { asset: 'video/first', contentHash: 'sha256:first', mediaType: 'video/webm' } }));
  await host.applyPresentation(frame({ op: 'play', handle: second, clip: { asset: 'video/second', contentHash: 'sha256:second', mediaType: 'video/webm' } }));
  assert.equal(children.length, 2);
  assert.match(videos[1]!.src, /^blob:/u);
  assert.equal(videos[0]!.removed, true);
  videos[0]!.emit('ended');
  assert.equal(host.realizedFacts().facts.length, 0);
  videos[1]!.emit('ended');
  assert.deepEqual(host.realizedFacts().facts, [{ kind: 'completed', factId: 1, handle: second }]);
  assert.equal(videos[1]!.removed, true);
});

test('video host reports a resolver failure and fences a pending play cancelled by skip', async () => {
  const gate: { resolve?: (value: { bytes: ArrayBuffer; contentHash: string; mediaType: 'video/webm' }) => void } = {};
  const videos: FakeVideo[] = [];
  const pending = new RendererVideoHost({
    container: { append: () => undefined } as unknown as HTMLElement,
    resolveResource: (clip) => new Promise((done) => { gate.resolve = () => done({ bytes: new Uint8Array([1]).buffer, contentHash: clip.contentHash, mediaType: 'video/webm' }); }),
    createVideoElement: () => { const video = new FakeVideo(); videos.push(video); return video as unknown as HTMLVideoElement; },
  });
  const handle = videoPlaybackHandle(9);
  const play = pending.applyPresentation(frame({ op: 'play', handle, clip: { asset: 'video/pending', contentHash: 'sha256:pending', mediaType: 'video/webm' } }));
  await pending.applyPresentation(frame({ op: 'skip', handle }));
  gate.resolve?.({ bytes: new Uint8Array([1]).buffer, contentHash: 'sha256:pending', mediaType: 'video/webm' });
  await play;
  assert.equal(videos.length, 0);
  assert.deepEqual(pending.realizedFacts().facts, [{ kind: 'skipped', factId: 1, handle }]);

  const failing = new RendererVideoHost({
    container: { append: () => undefined } as unknown as HTMLElement,
    resolveResource: async () => Promise.reject(new Error('resource unavailable')),
    createVideoElement: () => new FakeVideo() as unknown as HTMLVideoElement,
  });
  await failing.applyPresentation(frame({ op: 'play', handle, clip: { asset: 'video/missing', contentHash: 'sha256:missing', mediaType: 'video/webm' } }));
  assert.deepEqual(failing.realizedFacts().facts, [{ kind: 'failed', factId: 1, handle, code: 'hostFailure' }]);
});

test('video host skip records a terminal observation and cleanup removes the active element', async () => {
  const videos: FakeVideo[] = [];
  const host = new RendererVideoHost({
    container: { append: () => undefined } as unknown as HTMLElement,
    resolveResource: async (clip) => ({ bytes: new Uint8Array([1]).buffer, contentHash: clip.contentHash, mediaType: 'video/webm' }),
    createVideoElement: () => { const video = new FakeVideo(); videos.push(video); return video as unknown as HTMLVideoElement; },
  });
  const handle = videoPlaybackHandle(8);
  await host.applyPresentation(frame({ op: 'play', handle, clip: { asset: 'video/skip', contentHash: 'sha256:skip', mediaType: 'video/webm' } }));
  await host.applyPresentation(frame({ op: 'skip', handle }));
  assert.deepEqual(host.realizedFacts().facts, [{ kind: 'skipped', factId: 1, handle }]);
  assert.equal(videos[0]!.paused, true);
  assert.equal(videos[0]!.removed, true);
});

test('video host reset clears its retained and evicted terminal-fact accounting', async () => {
  const host = new RendererVideoHost({
    container: { append: () => undefined } as unknown as HTMLElement,
    resolveResource: async (clip) => ({ bytes: new Uint8Array([1]).buffer, contentHash: clip.contentHash, mediaType: 'video/webm' }),
  });
  for (let index = 1; index <= 129; index += 1) {
    const handle = videoPlaybackHandle(index);
    await host.applyPresentation(frame({ op: 'skip', handle }));
  }
  assert.deepEqual(host.realizedFacts().retainedFactCount, 128);
  assert.deepEqual(host.realizedFacts().evictedFactCount, 1);
  host.reset();
  assert.deepEqual(host.realizedFacts(), { retainedFactCount: 0, evictedFactCount: 0, facts: [] });
});
