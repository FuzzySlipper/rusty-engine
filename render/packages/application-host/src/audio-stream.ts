/**
 * The sound the runtime mixes, played in the page that watches its frames.
 *
 * With `stream` output and `audio.output` `stream`, the runtime mixes its
 * committed audio in real time and serves it as one long response of raw
 * interleaved stereo 16-bit little-endian PCM (`AUDIO_STREAM_FORMAT`). The
 * browser lets a page start sound only after a user gesture, so the first
 * pointer press or key press in the page opens the stream; until then the
 * page is silent and the runtime keeps mixing on its own time. A runtime that
 * does not stream audio answers 404 and the page stays silent. Every watching
 * page that has had a gesture plays the same mix.
 *
 * An audio worklet plays from a small jitter buffer. It starts once
 * `AUDIO_START_SECONDS` are buffered, and drops back to that depth whenever
 * more than `AUDIO_MAX_SECONDS` pile up, so the sound stays within a few
 * frames of the picture rather than drifting behind it.
 *
 * Wire format: `rust/crates/product-host/src/audio.rs`.
 */
import { AUDIO_STREAM_FORMAT, AUDIO_STREAM_PATH } from './generated/contracts.js';

const AUDIO_START_SECONDS = 0.06;
const AUDIO_MAX_SECONDS = 0.25;
/** The ring holds this much; more than `AUDIO_MAX_SECONDS` is dropped anyway. */
const AUDIO_RING_SECONDS = 1;
const RETRY_DELAY_MS = 1000;
const BYTES_PER_FRAME = AUDIO_STREAM_FORMAT.channels * 2;
const PROCESSOR = 'rusty-audio-stream';

/** Runs in the audio thread: interleaved stereo samples in, two channels out. */
const WORKLET = `
class RustyAudioStream extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const { ring, start, max } = options.processorOptions;
    this.ring = new Float32Array(ring * 2);
    this.start = start;
    this.max = max;
    this.read = 0;
    this.write = 0;
    this.playing = false;
    this.port.onmessage = (event) => this.push(event.data);
  }
  buffered() {
    return (this.write - this.read + this.ring.length) % this.ring.length / 2;
  }
  push(samples) {
    for (let i = 0; i < samples.length; i += 1) {
      this.ring[this.write] = samples[i];
      this.write = (this.write + 1) % this.ring.length;
    }
    if (this.buffered() > this.max) {
      this.read = (this.write - this.start * 2 + this.ring.length) % this.ring.length;
    }
  }
  process(_inputs, outputs) {
    const [left, right] = outputs[0];
    if (!this.playing && this.buffered() >= this.start) this.playing = true;
    for (let i = 0; i < left.length; i += 1) {
      if (!this.playing || this.read === this.write) {
        this.playing = false;
        left[i] = 0;
        if (right) right[i] = 0;
        continue;
      }
      left[i] = this.ring[this.read];
      if (right) right[i] = this.ring[this.read + 1];
      this.read = (this.read + 2) % this.ring.length;
    }
    return true;
  }
}
registerProcessor('${PROCESSOR}', RustyAudioStream);
`;

export interface RustyApplicationAudioStream {
  readonly dispose: () => void;
}

/** Plays the runtime's streamed mix in `window` from its first user gesture on. */
export function mountRustyApplicationAudioStream(window: Window): RustyApplicationAudioStream {
  const document = window.document;
  let context: AudioContext | null = null;
  let disposed = false;
  const listening = new AbortController();

  const listen = async (node: AudioWorkletNode): Promise<void> => {
    while (!disposed) {
      try {
        const response = await window.fetch(AUDIO_STREAM_PATH, { signal: listening.signal, cache: 'no-store' });
        // This runtime plays its sound elsewhere (or not at all).
        if (response.status === 404) return;
        if (response.ok && response.body !== null) {
          const reader = response.body.getReader();
          let carry = new Uint8Array(0);
          for (;;) {
            const { done, value } = await reader.read();
            if (done || disposed) break;
            const bytes = carry.length === 0 ? value : concat(carry, value);
            const whole = bytes.length - (bytes.length % BYTES_PER_FRAME);
            node.port.postMessage(samples(bytes.subarray(0, whole)));
            carry = bytes.slice(whole);
          }
        }
      } catch {
        if (disposed) return;
      }
      // The runtime was replaced or the connection dropped: listen again.
      await new Promise((resolve) => window.setTimeout(resolve, RETRY_DELAY_MS));
    }
  };

  const start = async (): Promise<void> => {
    const created = new AudioContext({
      latencyHint: 'interactive',
      sampleRate: AUDIO_STREAM_FORMAT.sampleRate,
    });
    context = created;
    const module = URL.createObjectURL(new Blob([WORKLET], { type: 'text/javascript' }));
    try {
      await created.audioWorklet.addModule(module);
    } finally {
      URL.revokeObjectURL(module);
    }
    if (disposed) return;
    const node = new AudioWorkletNode(created, PROCESSOR, {
      numberOfInputs: 0,
      outputChannelCount: [AUDIO_STREAM_FORMAT.channels],
      processorOptions: {
        ring: Math.round(AUDIO_RING_SECONDS * AUDIO_STREAM_FORMAT.sampleRate),
        start: Math.round(AUDIO_START_SECONDS * AUDIO_STREAM_FORMAT.sampleRate),
        max: Math.round(AUDIO_MAX_SECONDS * AUDIO_STREAM_FORMAT.sampleRate),
      },
    });
    node.connect(created.destination);
    await listen(node);
  };

  // A gesture both allows sound to start and resumes a context the browser
  // suspended.
  const gesture = (): void => {
    if (disposed) return;
    if (context === null) {
      void start().catch(() => undefined);
    } else if (context.state === 'suspended') {
      void context.resume().catch(() => undefined);
    }
  };
  const options = { capture: true, signal: listening.signal } as const;
  document.addEventListener('pointerdown', gesture, options);
  document.addEventListener('keydown', gesture, options);

  return {
    dispose: () => {
      if (disposed) return;
      disposed = true;
      listening.abort();
      void context?.close().catch(() => undefined);
      context = null;
    },
  };
}

function concat(first: Uint8Array, second: Uint8Array): Uint8Array {
  const joined = new Uint8Array(first.length + second.length);
  joined.set(first, 0);
  joined.set(second, first.length);
  return joined;
}

/** Little-endian 16-bit PCM as samples in `[-1, 1)`. */
export function samples(bytes: Uint8Array): Float32Array {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out = new Float32Array(bytes.byteLength / 2);
  for (let i = 0; i < out.length; i += 1) out[i] = view.getInt16(i * 2, true) / 32768;
  return out;
}
