import wavUrl from '../../fixtures/audio-containers/tone.wav?url';
import oggUrl from '../../fixtures/audio-containers/tone.ogg?url';
import opusUrl from '../../fixtures/audio-containers/tone.opus?url';
import mp3Url from '../../fixtures/audio-containers/tone.mp3?url';
import flacUrl from '../../fixtures/audio-containers/tone.flac?url';
const urls: Record<string, string> = { wav: wavUrl, ogg: oggUrl, opus: opusUrl, mp3: mp3Url, flac: flacUrl };
import { RendererAudioHost, type RendererAudioContext } from '@rusty-engine/renderer-host';
import { audioHandle, audioSignalHandle } from '@rusty-engine/render-contracts';

const media: HTMLAudioElement[] = [];
const context = new AudioContext();
const analyser = context.createAnalyser();
analyser.connect(context.destination);
// Observe actual rendered samples without replacing the Engine source/decoder.
const destination = context.destination;
const audioContext = new Proxy(context, { get(target, key) {
  if (key === 'destination') return analyser;
  const value: unknown = Reflect.get(target, key, target);
  return typeof value === 'function' ? value.bind(target) : value;
} });
const host = new RendererAudioHost({
  createContext: () => audioContext as unknown as RendererAudioContext,
  createMediaElement: () => { const element = new Audio(); media.push(element); return element; },
  resolveResource: async (clip) => {
    const extension = clip.asset.split('/').at(-1)!;
    const response = await fetch(urls[extension]!);
    if (!response.ok) throw new Error('fixture unavailable');
    return { bytes: await response.arrayBuffer(), contentHash: clip.contentHash,
      mediaType: extension === 'wav' ? 'audio/wav' : extension === 'mp3' ? 'audio/mpeg' : extension === 'flac' ? 'audio/flac' : 'audio/ogg' };
  },
});
const extension = new URL(location.href).searchParams.get('format') ?? 'ogg';
let started = false;
document.querySelector('#play')!.addEventListener('click', () => { void (async () => {
  await host.resume();
  const descriptor = { clip: { asset: 'audio/' + extension, contentHash: extension }, bus: 'ambient' as const,
    volume: 0.2, pitch: 1, looping: true, spatialBlend: 0, attenuation: 1, pan: 0, emitter: { kind: 'global2d' as const } };
  const receipt = await host.applyPresentation({ schemaVersion: 1, ops: [
    { domain: 'audio', meta: { sequence: 1 }, op: { op: 'create', handle: audioHandle(1), descriptor } },
    { domain: 'audio', meta: { sequence: 2 }, op: { op: 'emit', signalId: 'proof', signalHandle: audioSignalHandle(2), descriptor: { ...descriptor, looping: false } } },
  ] });
  started = receipt.applied === 2;
  document.querySelector('#result')!.textContent = JSON.stringify(receipt);
})().catch(error => { document.querySelector('#result')!.textContent = String(error); }); });
window.__audioContainerProof = {
  read: () => {
    const samples = new Float32Array(analyser.fftSize); analyser.getFloatTimeDomainData(samples);
    return { started, format: extension, energy: samples.reduce((sum, sample) => sum + sample * sample, 0),
      media: media.map(element => ({ time: element.currentTime, duration: element.duration, loop: element.loop, paused: element.paused, src: element.src })),
      facts: host.realizedFacts().facts, readout: host.readout(), time: context.currentTime };
  },
  stop: async () => { await host.dispose(); analyser.disconnect(destination); },
};
declare global { interface Window { __audioContainerProof: { read(): unknown; stop(): Promise<void> }; } }
