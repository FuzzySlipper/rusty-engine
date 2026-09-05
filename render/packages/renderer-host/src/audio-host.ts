import type {
  AudioBus,
  AudioClipRef,
  AudioEmitter,
  AudioHandle,
  AudioSignalHandle,
  AudioProjectionOp,
  AudioSourceDescriptor,
  AudioSourcePatch,
  PresentationFrameDiff,
  PresentationOp,
} from '@rusty-engine/render-contracts';
import type {
  AudioProjectionDiagnostic,
  AudioProjectionReadout,
} from './host-types.js';
import { rendererResourceContentHash } from './resource-content-hash.js';

export interface RendererAudioResource {
  readonly bytes: ArrayBuffer;
  readonly contentHash: string;
}

export type RendererAudioResourceResolver = (clip: AudioClipRef) => Promise<RendererAudioResource>;
export type RendererAudioEntityPositionResolver = (
  entity: number,
) => readonly [number, number, number] | null;

interface RendererAudioParam {
  setValueAtTime(value: number, time: number): void;
}

interface RendererAudioNode {
  connect(destination: RendererAudioNode): unknown;
  disconnect(): void;
}

interface RendererGainNode extends RendererAudioNode {
  readonly gain: RendererAudioParam;
}

interface RendererStereoPannerNode extends RendererAudioNode {
  readonly pan: RendererAudioParam;
}

interface RendererPannerNode extends RendererAudioNode {
  distanceModel: DistanceModelType;
  maxDistance: number;
  panningModel: PanningModelType;
  refDistance: number;
  rolloffFactor: number;
  readonly positionX: RendererAudioParam;
  readonly positionY: RendererAudioParam;
  readonly positionZ: RendererAudioParam;
}

interface RendererAudioListener {
  readonly forwardX: RendererAudioParam;
  readonly forwardY: RendererAudioParam;
  readonly forwardZ: RendererAudioParam;
  readonly positionX: RendererAudioParam;
  readonly positionY: RendererAudioParam;
  readonly positionZ: RendererAudioParam;
  readonly upX: RendererAudioParam;
  readonly upY: RendererAudioParam;
  readonly upZ: RendererAudioParam;
}

interface RendererBufferSourceNode extends RendererAudioNode {
  buffer: unknown;
  loop: boolean;
  onended: (() => void) | null;
  readonly playbackRate: RendererAudioParam;
  start(when?: number, offset?: number): void;
  stop(): void;
}

export interface RendererAudioContext {
  readonly currentTime: number;
  readonly destination: RendererAudioNode;
  readonly listener: RendererAudioListener;
  readonly state: AudioContextState;
  close(): Promise<void>;
  createBufferSource(): RendererBufferSourceNode;
  createGain(): RendererGainNode;
  createPanner(): RendererPannerNode;
  createStereoPanner(): RendererStereoPannerNode;
  decodeAudioData(bytes: ArrayBuffer): Promise<unknown>;
  resume(): Promise<void>;
}

export interface RendererAudioHostOptions {
  readonly createContext?: () => RendererAudioContext;
  /** Maximum realized facts retained for product-owned feedback consumption. */
  readonly maxRetainedFacts?: number;
  /** Maximum host diagnostics retained for local renderer inspection. */
  readonly maxRetainedDiagnostics?: number;
  readonly resolveEntityPosition?: RendererAudioEntityPositionResolver;
  readonly resolveResource: RendererAudioResourceResolver;
}

export interface RendererAudioListenerPose {
  readonly position: readonly [number, number, number];
  readonly forward: readonly [number, number, number];
  readonly up: readonly [number, number, number];
}

export interface RendererAudioFrameReceipt {
  readonly applied: number;
  readonly diagnostics: readonly AudioProjectionDiagnostic[];
  readonly readout: AudioProjectionReadout;
}

/** A concrete audio realization, intentionally separate from projection/admission readout. */
export type RendererAudioRealizedFact =
  | {
    readonly kind: 'naturalCompletion';
    readonly factId: number;
    readonly source: 'oneShot';
    readonly sequence: number;
    readonly signalHandle: AudioSignalHandle;
  }
  | {
    readonly kind: 'naturalCompletion';
    readonly factId: number;
    readonly source: 'retainedVoice';
    readonly sequence: number;
    readonly handle: AudioHandle;
  }
  | {
    readonly kind: 'diagnostic';
    readonly factId: number;
    readonly diagnostic: AudioProjectionDiagnostic;
  };

export interface RendererAudioRealizedFactsReadout {
  readonly retainedFactCount: number;
  readonly evictedFactCount: number;
  readonly facts: readonly RendererAudioRealizedFact[];
}

type RendererAudioRealizedFactInput =
  | Omit<Extract<RendererAudioRealizedFact, { readonly source: 'oneShot' }>, 'factId'>
  | Omit<Extract<RendererAudioRealizedFact, { readonly source: 'retainedVoice' }>, 'factId'>
  | Omit<Extract<RendererAudioRealizedFact, { readonly kind: 'diagnostic' }>, 'factId'>;

interface RendererAudioSourceGraph {
  descriptor: AudioSourceDescriptor;
  sequence: number;
  readonly source: RendererBufferSourceNode;
  readonly duration: number | null;
  readonly dryGain: RendererGainNode;
  readonly wetGain: RendererGainNode;
  readonly stereoPanner: RendererStereoPannerNode;
  readonly panner: RendererPannerNode | null;
  startedAt: number;
  startedOffset: number;
  playbackRate: number;
  readonly epoch: number;
  disposed: boolean;
}

interface RendererRetainedVoice {
  descriptor: AudioSourceDescriptor;
  sequence: number;
  state: 'playing' | 'paused' | 'completed';
  cursor: number;
  graph: RendererAudioSourceGraph | null;
}

interface RendererAudioBusState {
  volume: number;
  muted: boolean;
}

export class RendererAudioHost {
  readonly #context: RendererAudioContext;
  readonly #resolveEntityPosition: RendererAudioEntityPositionResolver;
  readonly #resolveResource: RendererAudioResourceResolver;
  readonly #buses: Readonly<Record<AudioBus, RendererGainNode>>;
  readonly #busStates: Record<AudioBus, RendererAudioBusState> = {
    sfx: { volume: 1, muted: false },
    ambient: { volume: 1, muted: false },
    ui: { volume: 1, muted: false },
  };
  readonly #cache = new Map<string, Promise<unknown>>();
  readonly #retained = new Map<number, RendererRetainedVoice>();
  readonly #oneShots = new Set<RendererAudioSourceGraph>();
  readonly #seenSignals = new Set<string>();
  readonly #diagnostics: AudioProjectionDiagnostic[] = [];
  readonly #realizedFacts: RendererAudioRealizedFact[] = [];
  readonly #maxRetainedFacts: number;
  readonly #maxRetainedDiagnostics: number;
  #emittedSignals = 0;
  #evictedDiagnosticCount = 0;
  #evictedFactCount = 0;
  #nextFactId = 1;
  #epoch = 1;
  #disposed = false;

  constructor(options: RendererAudioHostOptions) {
    this.#maxRetainedFacts = options.maxRetainedFacts ?? 128;
    if (!Number.isSafeInteger(this.#maxRetainedFacts) || this.#maxRetainedFacts < 1) {
      throw new RangeError('maxRetainedFacts must be a positive safe integer');
    }
    this.#maxRetainedDiagnostics = options.maxRetainedDiagnostics ?? 128;
    if (!Number.isSafeInteger(this.#maxRetainedDiagnostics) || this.#maxRetainedDiagnostics < 1) {
      throw new RangeError('maxRetainedDiagnostics must be a positive safe integer');
    }
    this.#context = options.createContext?.() ?? createBrowserAudioContext();
    this.#resolveResource = options.resolveResource;
    this.#resolveEntityPosition = options.resolveEntityPosition ?? (() => null);
    const sfx = this.#context.createGain();
    const ambient = this.#context.createGain();
    const ui = this.#context.createGain();
    sfx.connect(this.#context.destination);
    ambient.connect(this.#context.destination);
    ui.connect(this.#context.destination);
    this.#buses = { sfx, ambient, ui };
    for (const bus of ['sfx', 'ambient', 'ui'] as const) {
      this.#applyBusGain(bus);
    }
  }

  async resume(): Promise<readonly AudioProjectionDiagnostic[]> {
    try {
      await this.#context.resume();
      if (this.#context.state === 'running') {
        return [];
      }
      return this.#recordHostDiagnostic(
        'audioContextBlocked',
        'audio context remained ' + this.#context.state,
      );
    } catch (error) {
      return this.#recordHostDiagnostic(
        'audioContextBlocked',
        errorMessage(error, 'audio context resume failed'),
      );
    }
  }

  updateListener(pose: RendererAudioListenerPose): readonly AudioProjectionDiagnostic[] {
    if (this.#disposed) {
      return this.#recordHostDiagnostic('hostFailure', 'audio host is disposed');
    }
    if (![...pose.position, ...pose.forward, ...pose.up].every(Number.isFinite)) {
      return this.#recordHostDiagnostic('invalidDescriptor', 'audio listener pose must be finite');
    }
    const time = this.#context.currentTime;
    setVector(this.#context.listener, 'position', pose.position, time);
    setVector(this.#context.listener, 'forward', pose.forward, time);
    setVector(this.#context.listener, 'up', pose.up, time);
    return [];
  }

  async applyPresentation(presentation: PresentationFrameDiff): Promise<RendererAudioFrameReceipt> {
    if (this.#disposed) {
      return this.#receipt(
        0,
        this.#recordHostDiagnostic('hostFailure', 'audio host is disposed'),
      );
    }
    const epoch = this.#epoch;
    const diagnostics: AudioProjectionDiagnostic[] = [];
    let applied = 0;
    for (const operation of presentation.ops) {
      if (operation.domain !== 'audio') {
        continue;
      }
      if (!this.#isCurrentEpoch(epoch)) {
        return this.#receipt(0, []);
      }
      let diagnostic: AudioProjectionDiagnostic | null;
      try {
        diagnostic = await this.#applyOperation(operation, epoch);
      } catch (error) {
        if (error instanceof StaleAudioOperation) {
          return this.#receipt(0, []);
        }
        throw error;
      }
      if (!this.#isCurrentEpoch(epoch)) {
        return this.#receipt(0, []);
      }
      if (diagnostic === null) {
        applied += 1;
      } else {
        diagnostics.push(diagnostic);
        this.#recordDiagnostic(diagnostic);
      }
    }
    return this.#receipt(applied, diagnostics);
  }

  readout(): AudioProjectionReadout {
    return {
      activeSources: this.#retained.size,
      cachedClips: this.#cache.size,
      emittedSignals: this.#emittedSignals,
      retainedDiagnosticCount: this.#diagnostics.length,
      evictedDiagnosticCount: this.#evictedDiagnosticCount,
      diagnostics: [...this.#diagnostics],
    };
  }

  /** Read host-realized facts without conflating them with projection admission. */
  realizedFacts(): RendererAudioRealizedFactsReadout {
    return {
      retainedFactCount: this.#realizedFacts.length,
      evictedFactCount: this.#evictedFactCount,
      facts: [...this.#realizedFacts],
    };
  }

  /** Acknowledge submitted realization facts without erasing facts that arrived in flight. */
  acknowledgeRealizedFacts(throughFactId: number): void {
    if (!Number.isSafeInteger(throughFactId) || throughFactId < 0) {
      throw new RangeError('throughFactId must be a non-negative safe integer');
    }
    const firstRetained = this.#realizedFacts.findIndex((fact) => fact.factId > throughFactId);
    if (firstRetained < 0) {
      this.#realizedFacts.length = 0;
    } else if (firstRetained > 0) {
      this.#realizedFacts.splice(0, firstRetained);
    }
  }

  /** Invalidate current playback ownership so late Web Audio callbacks cannot leak forward. */
  reset(): void {
    if (this.#disposed) return;
    this.#epoch += 1;
    const retainedGraphs = [...this.#retained.values()]
      .flatMap((voice) => voice.graph === null ? [] : [voice.graph]);
    for (const graph of [...retainedGraphs, ...this.#oneShots]) disposeGraph(graph);
    this.#retained.clear();
    this.#oneShots.clear();
    this.#seenSignals.clear();
    this.#emittedSignals = 0;
    this.#diagnostics.length = 0;
    this.#evictedDiagnosticCount = 0;
    this.#realizedFacts.length = 0;
    this.#evictedFactCount = 0;
  }

  refreshLayout(): readonly AudioProjectionDiagnostic[] {
    if (this.#disposed) {
      return this.#recordHostDiagnostic('hostFailure', 'audio host is disposed');
    }
    const diagnostics: AudioProjectionDiagnostic[] = [];
    for (const [handle, voice] of this.#retained) {
      const graph = voice.graph;
      if (graph === null) {
        continue;
      }
      if (graph.descriptor.emitter.kind !== 'entityAttached' || graph.panner === null) {
        continue;
      }
      const position = emitterPosition(graph.descriptor.emitter, this.#resolveEntityPosition);
      if (position === null || !position.every(Number.isFinite)) {
        diagnostics.push({
          code: 'hostFailure',
          sequence: graph.sequence,
          handle: handle as AudioHandle,
          message: 'entity-attached audio source has no finite projected position',
        });
        continue;
      }
      setPannerPosition(graph.panner, position, this.#context.currentTime);
    }
    this.#recordDiagnostics(diagnostics);
    return diagnostics;
  }

  async dispose(): Promise<void> {
    if (this.#disposed) {
      return;
    }
    this.reset();
    this.#disposed = true;
    for (const bus of Object.values(this.#buses)) {
      bus.disconnect();
    }
    await this.#context.close();
  }

  async #applyOperation(
    operation: Extract<PresentationOp, { readonly domain: 'audio' }>,
    epoch: number,
  ): Promise<AudioProjectionDiagnostic | null> {
    const { meta, op } = operation;
    try {
      if (op.op === 'emit') {
        if (this.#seenSignals.has(op.signalId)) {
          return null;
        }
        const graph = await this.#createGraph(op.descriptor, meta.sequence, epoch);
        this.#assertCurrentEpoch(epoch);
        this.#seenSignals.add(op.signalId);
        this.#oneShots.add(graph);
        this.#startOneShotGraph(graph, op.signalHandle);
        this.#emittedSignals += 1;
        return null;
      }
      if (op.op === 'create') {
        if (this.#retained.has(op.handle as number)) {
          return operationDiagnostic('duplicateHandle', meta, op.handle, 'audio handle is active');
        }
        const voice: RendererRetainedVoice = {
          descriptor: op.descriptor,
          sequence: meta.sequence,
          state: 'playing',
          cursor: 0,
          graph: null,
        };
        const graph = await this.#createGraph(voice.descriptor, voice.sequence, epoch);
        this.#assertCurrentEpoch(epoch);
        voice.graph = graph;
        this.#retained.set(op.handle as number, voice);
        this.#startRetainedGraph(op.handle, voice, graph, 0);
        return null;
      }
      if (op.op === 'restore') {
        if (this.#retained.has(op.handle as number)) {
          return operationDiagnostic('duplicateHandle', meta, op.handle, 'audio handle is active');
        }
        const voice: RendererRetainedVoice = {
          descriptor: op.descriptor,
          sequence: meta.sequence,
          state: op.desiredState,
          cursor: op.cursorSeconds,
          graph: null,
        };
        if (voice.state === 'playing') {
          const graph = await this.#createGraph(voice.descriptor, voice.sequence, epoch);
          this.#assertCurrentEpoch(epoch);
          voice.graph = graph;
          this.#startRetainedGraph(op.handle, voice, graph, voice.cursor);
        }
        this.#retained.set(op.handle as number, voice);
        return null;
      }
      if (op.op === 'destroy') {
        const voice = this.#retained.get(op.handle as number);
        if (voice === undefined) {
          return operationDiagnostic('unknownHandle', meta, op.handle, 'audio handle is unknown');
        }
        this.#retained.delete(op.handle as number);
        disposeGraph(voice.graph);
        return null;
      }
      if (op.op === 'voiceControl') {
        return await this.#applyVoiceControl(meta, op.handle, op.control, epoch);
      }
      if (op.op === 'busControl') {
        this.#applyBusControl(op.bus, op.control);
        return null;
      }
      return await this.#updateGraph(meta, op.handle, op.patch, epoch);
    } catch (error) {
      if (error instanceof StaleAudioOperation) {
        throw error;
      }
      return operationDiagnostic(
        classifyHostError(error),
        meta,
        operationHandle(op),
        errorMessage(error, 'audio host operation failed'),
      );
    }
  }

  async #updateGraph(
    meta: Extract<PresentationOp, { readonly domain: 'audio' }>['meta'],
    handle: AudioHandle,
    patch: AudioSourcePatch,
    epoch: number,
  ): Promise<AudioProjectionDiagnostic | null> {
    const voice = this.#retained.get(handle as number);
    if (voice === undefined) {
      return operationDiagnostic('unknownHandle', meta, handle, 'audio handle is unknown');
    }
    const next = patchedDescriptor(voice.descriptor, patch);
    const graph = voice.graph;
    if (graph === null) {
      voice.descriptor = next;
      voice.sequence = meta.sequence;
      return null;
    }
    const cursor = playbackCursor(graph, this.#context.currentTime);
    if (patch.emitter !== null) {
      const replacement = await this.#createGraph(next, meta.sequence, epoch);
      this.#assertCurrentEpoch(epoch);
      disposeGraph(graph);
      voice.graph = replacement;
      voice.descriptor = next;
      voice.sequence = meta.sequence;
      this.#startRetainedGraph(handle, voice, replacement, cursor);
      return null;
    }
    graph.descriptor = next;
    graph.sequence = meta.sequence;
    graph.startedAt = this.#context.currentTime;
    graph.startedOffset = normalizeCursor(cursor, graph.duration, next.looping);
    applyGraphParameters(this.#context, graph, next, this.#resolveEntityPosition);
    return null;
  }

  async #applyVoiceControl(
    meta: Extract<PresentationOp, { readonly domain: 'audio' }>['meta'],
    handle: AudioHandle,
    control: 'pause' | 'resume' | 'retrigger',
    epoch: number,
  ): Promise<AudioProjectionDiagnostic | null> {
    const voice = this.#retained.get(handle as number);
    if (voice === undefined) {
      return operationDiagnostic('unknownHandle', meta, handle, 'audio handle is unknown');
    }
    voice.sequence = meta.sequence;
    if (control === 'pause') {
      if (voice.state === 'paused') {
        return null;
      }
      if (voice.graph !== null) {
        voice.cursor = playbackCursor(voice.graph, this.#context.currentTime);
        disposeGraph(voice.graph);
        voice.graph = null;
      }
      voice.state = 'paused';
      return null;
    }
    if (control === 'resume') {
      if (voice.state === 'playing') {
        return null;
      }
      const graph = await this.#createGraph(voice.descriptor, voice.sequence, epoch);
      this.#assertCurrentEpoch(epoch);
      voice.graph = graph;
      voice.state = 'playing';
      this.#startRetainedGraph(handle, voice, graph, voice.cursor);
      return null;
    }
    const replacement = await this.#createGraph(voice.descriptor, voice.sequence, epoch);
    this.#assertCurrentEpoch(epoch);
    disposeGraph(voice.graph);
    voice.cursor = 0;
    voice.graph = replacement;
    voice.state = 'playing';
    this.#startRetainedGraph(handle, voice, replacement, 0);
    return null;
  }

  #applyBusControl(
    bus: AudioBus,
    control: Extract<AudioProjectionOp, { readonly op: 'busControl' }>['control'],
  ): void {
    const state = this.#busStates[bus];
    if (control.kind === 'setVolume') {
      if (!Number.isFinite(control.volume) || control.volume < 0 || control.volume > 1) {
        throw new Error('audio bus volume must be finite and between 0 and 1');
      }
      state.volume = control.volume;
    } else {
      state.muted = control.muted;
    }
    this.#applyBusGain(bus);
  }

  #applyBusGain(bus: AudioBus): void {
    const state = this.#busStates[bus];
    this.#buses[bus].gain.setValueAtTime(
      state.muted ? 0 : state.volume,
      this.#context.currentTime,
    );
  }

  async #createGraph(
    descriptor: AudioSourceDescriptor,
    sequence: number,
    epoch: number,
  ): Promise<RendererAudioSourceGraph> {
    const buffer = await this.#decodeClip(descriptor.clip);
    this.#assertCurrentEpoch(epoch);
    const source = this.#context.createBufferSource();
    source.buffer = buffer;
    const graph: RendererAudioSourceGraph = {
      descriptor,
      sequence,
      source,
      duration: bufferDuration(buffer),
      dryGain: this.#context.createGain(),
      wetGain: this.#context.createGain(),
      stereoPanner: this.#context.createStereoPanner(),
      panner: descriptor.emitter.kind === 'global2d' ? null : this.#context.createPanner(),
      startedAt: this.#context.currentTime,
      startedOffset: 0,
      playbackRate: descriptor.pitch,
      epoch,
      disposed: false,
    };
    source.connect(graph.stereoPanner);
    graph.stereoPanner.connect(graph.dryGain);
    graph.dryGain.connect(this.#buses[descriptor.bus]);
    if (graph.panner !== null) {
      source.connect(graph.panner);
      graph.panner.connect(graph.wetGain);
      graph.wetGain.connect(this.#buses[descriptor.bus]);
    }
    applyGraphParameters(this.#context, graph, descriptor, this.#resolveEntityPosition);
    return graph;
  }

  #isCurrentEpoch(epoch: number): boolean {
    return !this.#disposed && this.#epoch === epoch;
  }

  #assertCurrentEpoch(epoch: number): void {
    if (!this.#isCurrentEpoch(epoch)) {
      throw new StaleAudioOperation();
    }
  }

  async #decodeClip(clip: AudioClipRef): Promise<unknown> {
    const existing = this.#cache.get(clip.contentHash);
    if (existing !== undefined) {
      return existing;
    }
    const decoded = this.#resolveResource(clip).then(async (resource) => {
      if (resource.contentHash !== clip.contentHash) {
        throw new RendererAudioResourceError(
          'contentHashMismatch',
          'resolved audio content hash does not match the requested clip',
        );
      }
      const actualHash = await rendererResourceContentHash(resource.bytes, clip.contentHash)
        .catch((error: unknown) => {
          throw new RendererAudioResourceError(
            'contentHashMismatch',
            error instanceof Error ? error.message : String(error),
          );
        });
      if (actualHash !== clip.contentHash) {
        throw new RendererAudioResourceError(
          'contentHashMismatch',
          `audio bytes hash ${actualHash} does not match ${clip.contentHash}`,
        );
      }
      try {
        return await this.#context.decodeAudioData(resource.bytes.slice(0));
      } catch (error) {
        throw new RendererAudioResourceError(
          'decodeFailed',
          errorMessage(error, 'audio clip decoding failed'),
        );
      }
    });
    this.#cache.set(clip.contentHash, decoded);
    try {
      return await decoded;
    } catch (error) {
      this.#cache.delete(clip.contentHash);
      throw error;
    }
  }

  #recordHostDiagnostic(
    code: AudioProjectionDiagnostic['code'],
    message: string,
  ): readonly AudioProjectionDiagnostic[] {
    const diagnostic = hostDiagnostic(code, message);
    this.#recordDiagnostic(diagnostic);
    return [diagnostic];
  }

  #recordDiagnostics(diagnostics: readonly AudioProjectionDiagnostic[]): void {
    for (const diagnostic of diagnostics) this.#recordDiagnostic(diagnostic);
  }

  #recordDiagnostic(diagnostic: AudioProjectionDiagnostic): void {
    const duplicate = this.#diagnostics.findIndex((candidate) => (
      candidate.code === diagnostic.code
      && candidate.handle === diagnostic.handle
      && candidate.message === diagnostic.message
    ));
    if (duplicate >= 0) {
      this.#diagnostics[duplicate] = diagnostic;
      return;
    }
    if (this.#diagnostics.length === this.#maxRetainedDiagnostics) {
      this.#diagnostics.shift();
      this.#evictedDiagnosticCount += 1;
    }
    this.#diagnostics.push(diagnostic);
    this.#appendRealizedFact({ kind: 'diagnostic', diagnostic });
  }

  #appendRealizedFact(
    fact: RendererAudioRealizedFactInput,
  ): void {
    if (this.#realizedFacts.length === this.#maxRetainedFacts) {
      this.#realizedFacts.shift();
      this.#evictedFactCount += 1;
    }
    this.#realizedFacts.push({ ...fact, factId: this.#nextFactId++ } as RendererAudioRealizedFact);
  }

  #startOneShotGraph(
    graph: RendererAudioSourceGraph,
    signalHandle: AudioSignalHandle,
  ): void {
    graph.source.onended = () => {
      if (graph.disposed || graph.epoch !== this.#epoch || !this.#oneShots.delete(graph)) return;
      if (!graph.descriptor.looping) {
        this.#appendRealizedFact({
          kind: 'naturalCompletion',
          source: 'oneShot',
          sequence: graph.sequence,
          signalHandle,
        });
      }
      disposeGraph(graph);
    };
    startGraph(graph, 0, this.#context.currentTime);
  }

  #startRetainedGraph(
    handle: AudioHandle,
    voice: RendererRetainedVoice,
    graph: RendererAudioSourceGraph,
    offset: number,
  ): void {
    const normalizedOffset = normalizeCursor(offset, graph.duration, graph.descriptor.looping);
    if (!graph.descriptor.looping && graph.duration !== null && normalizedOffset >= graph.duration) {
      // A baseline can carry an Engine cursor from a format whose byte-rate
      // metadata was absent or disagreed with browser decoding. It is already
      // complete in this realization, so retain that state quietly rather
      // than starting at the end and fabricating a new completion callback.
      voice.graph = null;
      voice.state = 'completed';
      voice.cursor = graph.duration;
      disposeGraph(graph);
      return;
    }
    graph.source.onended = () => {
      if (graph.disposed || graph.epoch !== this.#epoch) return;
      if (this.#retained.get(handle as number)?.graph !== graph) return;
      if (!graph.descriptor.looping) {
        voice.graph = null;
        voice.state = 'completed';
        voice.cursor = graph.duration ?? playbackCursor(graph, this.#context.currentTime);
        this.#appendRealizedFact({
          kind: 'naturalCompletion',
          source: 'retainedVoice',
          sequence: graph.sequence,
          handle,
        });
      }
      disposeGraph(graph);
    };
    startGraph(graph, normalizedOffset, this.#context.currentTime);
  }

  #receipt(
    applied: number,
    diagnostics: readonly AudioProjectionDiagnostic[],
  ): RendererAudioFrameReceipt {
    return { applied, diagnostics, readout: this.readout() };
  }
}

class StaleAudioOperation extends Error {}

class RendererAudioResourceError extends Error {
  constructor(
    readonly code: 'contentHashMismatch' | 'decodeFailed',
    message: string,
  ) {
    super(message);
  }
}

function createBrowserAudioContext(): RendererAudioContext {
  const Context = globalThis.AudioContext;
  if (Context === undefined) {
    throw new Error('Web Audio AudioContext is unavailable');
  }
  return new Context() as unknown as RendererAudioContext;
}

function applyGraphParameters(
  context: RendererAudioContext,
  graph: RendererAudioSourceGraph,
  descriptor: AudioSourceDescriptor,
  resolveEntityPosition: RendererAudioEntityPositionResolver,
): void {
  const time = context.currentTime;
  graph.source.loop = descriptor.looping;
  graph.source.playbackRate.setValueAtTime(descriptor.pitch, time);
  graph.playbackRate = descriptor.pitch;
  graph.stereoPanner.pan.setValueAtTime(descriptor.pan, time);
  const blend = descriptor.emitter.kind === 'global2d' ? 0 : descriptor.spatialBlend;
  graph.dryGain.gain.setValueAtTime(descriptor.volume * (1 - blend), time);
  graph.wetGain.gain.setValueAtTime(descriptor.volume * blend, time);
  if (graph.panner === null) {
    return;
  }
  const position = emitterPosition(descriptor.emitter, resolveEntityPosition);
  if (position === null) {
    throw new Error('entity-attached audio source has no projected position');
  }
  graph.panner.panningModel = 'equalpower';
  graph.panner.distanceModel = 'inverse';
  graph.panner.refDistance = 1;
  graph.panner.maxDistance = descriptor.attenuation;
  graph.panner.rolloffFactor = 1;
  setPannerPosition(graph.panner, position, time);
}

function setPannerPosition(
  panner: RendererPannerNode,
  position: readonly [number, number, number],
  time: number,
): void {
  panner.positionX.setValueAtTime(position[0], time);
  panner.positionY.setValueAtTime(position[1], time);
  panner.positionZ.setValueAtTime(position[2], time);
}

function setVector(
  listener: RendererAudioListener,
  prefix: 'position' | 'forward' | 'up',
  value: readonly [number, number, number],
  time: number,
): void {
  listener[`${prefix}X`].setValueAtTime(value[0], time);
  listener[`${prefix}Y`].setValueAtTime(value[1], time);
  listener[`${prefix}Z`].setValueAtTime(value[2], time);
}

function emitterPosition(
  emitter: AudioEmitter,
  resolveEntityPosition: RendererAudioEntityPositionResolver,
): readonly [number, number, number] | null {
  if (emitter.kind === 'global2d') {
    return [0, 0, 0];
  }
  if (emitter.kind === 'world3d') {
    return emitter.position;
  }
  const base = resolveEntityPosition(emitter.entity as number);
  return base === null
    ? null
    : [
        base[0] + emitter.offset[0],
        base[1] + emitter.offset[1],
        base[2] + emitter.offset[2],
      ];
}

function patchedDescriptor(
  descriptor: AudioSourceDescriptor,
  patch: AudioSourcePatch,
): AudioSourceDescriptor {
  return {
    ...descriptor,
    volume: patch.volume ?? descriptor.volume,
    pitch: patch.pitch ?? descriptor.pitch,
    looping: patch.looping ?? descriptor.looping,
    spatialBlend: patch.spatialBlend ?? descriptor.spatialBlend,
    attenuation: patch.attenuation ?? descriptor.attenuation,
    pan: patch.pan ?? descriptor.pan,
    emitter: patch.emitter ?? descriptor.emitter,
  };
}

function startGraph(
  graph: RendererAudioSourceGraph,
  offset: number,
  time: number,
): void {
  const normalizedOffset = normalizeCursor(offset, graph.duration, graph.descriptor.looping);
  graph.startedAt = time;
  graph.startedOffset = normalizedOffset;
  graph.source.start(0, normalizedOffset);
}

function playbackCursor(graph: RendererAudioSourceGraph, time: number): number {
  const elapsed = Math.max(0, time - graph.startedAt);
  return normalizeCursor(
    graph.startedOffset + elapsed * graph.playbackRate,
    graph.duration,
    graph.descriptor.looping,
  );
}

function normalizeCursor(cursor: number, duration: number | null, looping: boolean): number {
  if (duration === null) {
    return cursor;
  }
  if (!looping) {
    return Math.min(cursor, duration);
  }
  const remainder = cursor % duration;
  return remainder < 0 ? remainder + duration : remainder;
}

function bufferDuration(buffer: unknown): number | null {
  if (typeof buffer !== 'object' || buffer === null || !('duration' in buffer)) {
    return null;
  }
  const duration = (buffer as { readonly duration: unknown }).duration;
  return typeof duration === 'number' && Number.isFinite(duration) && duration > 0
    ? duration
    : null;
}

function disposeGraph(graph: RendererAudioSourceGraph | null): void {
  if (graph === null) {
    return;
  }
  if (graph.disposed) {
    return;
  }
  graph.disposed = true;
  graph.source.onended = null;
  try {
    graph.source.stop();
  } catch {
    // A naturally-ended one-shot is already stopped.
  }
  graph.source.disconnect();
  graph.stereoPanner.disconnect();
  graph.dryGain.disconnect();
  graph.panner?.disconnect();
  graph.wetGain.disconnect();
}

function operationHandle(op: AudioProjectionOp): AudioHandle | null {
  return op.op === 'emit' || op.op === 'busControl' ? null : op.handle;
}

function operationDiagnostic(
  code: AudioProjectionDiagnostic['code'],
  meta: Extract<PresentationOp, { readonly domain: 'audio' }>['meta'],
  handle: AudioHandle | null,
  message: string,
): AudioProjectionDiagnostic {
  return { code, sequence: meta.sequence, handle, message };
}

function hostDiagnostic(
  code: AudioProjectionDiagnostic['code'],
  message: string,
): AudioProjectionDiagnostic {
  return { code, sequence: 0, handle: null, message };
}

function classifyHostError(error: unknown): AudioProjectionDiagnostic['code'] {
  return error instanceof RendererAudioResourceError ? error.code : 'hostFailure';
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}
