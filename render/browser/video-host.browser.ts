import { RendererVideoHost } from '@rusty-engine/renderer-host';
import { videoPlaybackHandle } from '@rusty-engine/render-contracts';

declare global { interface Window { __rustyVideoProof?: { start: () => Promise<void>; skip: () => Promise<void>; stop: () => Promise<void>; read: () => { width: number; height: number; time: number; facts: unknown[]; active: boolean }; }; } }
const root = document.querySelector<HTMLElement>('#video-root');
if (root === null) throw new Error('video proof root is missing');
const host = new RendererVideoHost({
  container: root,
  resolveResource: async (clip) => ({ bytes: await webm(), contentHash: clip.contentHash, mediaType: 'video/webm' }),
});
let handle = videoPlaybackHandle(1);
const play = async () => host.applyPresentation({ schemaVersion: 1, ops: [{ domain: 'video', meta: { sequence: 0 }, op: { op: 'play', handle, clip: { asset: 'video/proof', contentHash: 'sha256:proof', mediaType: 'video/webm' } } }] });
window.__rustyVideoProof = {
  start: async () => { await play(); },
  skip: async () => { await host.applyPresentation({ schemaVersion: 1, ops: [{ domain: 'video', meta: { sequence: 0 }, op: { op: 'skip', handle } }] }); },
  stop: async () => { await host.applyPresentation({ schemaVersion: 1, ops: [{ domain: 'video', meta: { sequence: 0 }, op: { op: 'stop', handle } }] }); },
  read: () => { const video = root.querySelector('video'); return { width: video?.videoWidth ?? 0, height: video?.videoHeight ?? 0, time: video?.currentTime ?? 0, facts: [...host.realizedFacts().facts], active: video !== null }; },
};
async function webm(): Promise<ArrayBuffer> {
  const canvas = document.createElement('canvas'); canvas.width = 32; canvas.height = 24;
  const context = canvas.getContext('2d'); if (context === null) throw new Error('2D canvas unavailable');
  const stream = canvas.captureStream(15);
  const recorder = new MediaRecorder(stream, { mimeType: 'video/webm;codecs=vp9' });
  const chunks: BlobPart[] = [];
  recorder.addEventListener('dataavailable', (event) => chunks.push(event.data));
  const completed = new Promise<void>((resolve) => recorder.addEventListener('stop', () => resolve(), { once: true }));
  recorder.start();
  for (let frame = 0; frame < 18; frame += 1) { context.fillStyle = frame % 2 === 0 ? '#f00' : '#00f'; context.fillRect(0, 0, 32, 24); await new Promise((resolve) => setTimeout(resolve, 50)); }
  recorder.stop(); await completed;
  for (const track of stream.getTracks()) track.stop();
  return new Blob(chunks, { type: 'video/webm' }).arrayBuffer();
}
