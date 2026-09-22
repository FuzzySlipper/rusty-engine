import type {
  PresentationFrameDiff,
  VideoClipRef,
  VideoPlaybackHandle,
  VideoProjectionOp,
} from '@rusty-engine/render-contracts';

export interface RendererVideoResource {
  readonly bytes: ArrayBuffer;
  readonly contentHash: string;
  readonly mediaType: 'video/webm';
}

export type RendererVideoResourceResolver = (clip: VideoClipRef) => Promise<RendererVideoResource>;

export type RendererVideoRealizedFact =
  | { readonly kind: 'completed'; readonly factId: number; readonly handle: VideoPlaybackHandle }
  | { readonly kind: 'skipped'; readonly factId: number; readonly handle: VideoPlaybackHandle }
  | { readonly kind: 'failed'; readonly factId: number; readonly handle: VideoPlaybackHandle; readonly code: 'decodeFailed' | 'playbackBlocked' | 'hostFailure' };

export interface RendererVideoRealizedFactsReadout {
  readonly retainedFactCount: number;
  readonly evictedFactCount: number;
  readonly facts: readonly RendererVideoRealizedFact[];
}

export interface RendererVideoHostOptions {
  readonly container: HTMLElement;
  readonly resolveResource: RendererVideoResourceResolver;
  readonly createVideoElement?: () => HTMLVideoElement;
}

/**
 * The browser owner of one retained full-viewport HTMLVideoElement. Each play
 * receives an internal generation, so a late event from a removed/replaced
 * element cannot complete the current playback.
 */
export class RendererVideoHost {
  readonly #container: HTMLElement;
  readonly #resolveResource: RendererVideoResourceResolver;
  readonly #createVideoElement: () => HTMLVideoElement;
  readonly #facts: RendererVideoRealizedFact[] = [];
  #evicted = 0;
  #nextFactId = 1;
  #generation = 0;
  #active: { handle: VideoPlaybackHandle; generation: number; element: HTMLVideoElement; url: string } | null = null;

  constructor(options: RendererVideoHostOptions) {
    this.#container = options.container;
    this.#resolveResource = options.resolveResource;
    this.#createVideoElement = options.createVideoElement ?? (() => document.createElement('video'));
  }

  async applyPresentation(frame: PresentationFrameDiff): Promise<{ readonly applied: number; readonly diagnostics: readonly VideoDiagnostic[] }> {
    const operations = frame.ops.filter((entry): entry is Extract<typeof entry, { readonly domain: 'video' }> => entry.domain === 'video');
    const diagnostics: VideoDiagnostic[] = [];
    let applied = 0;
    for (const entry of operations) {
      try {
        await this.#apply(entry.op);
        applied += 1;
      } catch (cause) {
        diagnostics.push({ code: 'hostFailure', sequence: entry.meta.sequence, handle: entry.op.handle, message: cause instanceof Error ? cause.message : String(cause) });
      }
    }
    return { applied, diagnostics };
  }

  realizedFacts(): RendererVideoRealizedFactsReadout {
    return Object.freeze({ retainedFactCount: this.#facts.length, evictedFactCount: this.#evicted, facts: Object.freeze([...this.#facts]) });
  }

  acknowledgeRealizedFacts(throughFactId: number): void {
    while ((this.#facts[0]?.factId ?? Number.POSITIVE_INFINITY) <= throughFactId) this.#facts.shift();
  }

  reset(): void { this.#disposeActive(); this.#facts.splice(0); this.#evicted = 0; }
  dispose(): void { this.reset(); }

  async #apply(op: VideoProjectionOp): Promise<void> {
    if (op.op === 'play') return this.#play(op.handle, op.clip);
    // A stop/skip may race the asynchronous resource resolver before an
    // element exists. Advance the owner generation in that case so a late
    // resolver completion cannot resurrect a cancelled presentation.
    if (this.#active === null) {
      ++this.#generation;
      if (op.op === 'skip') this.#record({ kind: 'skipped', handle: op.handle });
      return;
    }
    if (this.#active.handle !== op.handle) return;
    if (op.op === 'skip') this.#record({ kind: 'skipped', handle: op.handle });
    this.#disposeActive();
  }

  async #play(handle: VideoPlaybackHandle, clip: VideoClipRef): Promise<void> {
    this.#disposeActive();
    const generation = ++this.#generation;
    let resource: RendererVideoResource;
    try { resource = await this.#resolveResource(clip); }
    catch { if (generation === this.#generation) this.#record({ kind: 'failed', handle, code: 'hostFailure' }); return; }
    if (generation !== this.#generation) return;
    if (resource.contentHash !== clip.contentHash || resource.mediaType !== 'video/webm') {
      this.#record({ kind: 'failed', handle, code: 'decodeFailed' });
      return;
    }
    const element = this.#createVideoElement();
    element.autoplay = true;
    element.controls = false;
    element.playsInline = true;
    element.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;object-fit:contain;background:#000;z-index:1000;';
    const url = URL.createObjectURL(new Blob([resource.bytes], { type: resource.mediaType }));
    const stillActive = () => this.#active?.generation === generation && this.#active.element === element;
    element.addEventListener('ended', () => { if (stillActive()) { this.#record({ kind: 'completed', handle }); this.#disposeActive(); } });
    element.addEventListener('error', () => { if (stillActive()) { this.#record({ kind: 'failed', handle, code: 'decodeFailed' }); this.#disposeActive(); } });
    this.#active = { handle, generation, element, url };
    element.src = url;
    this.#container.append(element);
    try { await element.play(); }
    catch { if (stillActive()) { this.#record({ kind: 'failed', handle, code: 'playbackBlocked' }); this.#disposeActive(); } }
  }

  #record(fact: PendingVideoFact): void {
    if (this.#facts.length === 128) { this.#facts.shift(); this.#evicted += 1; }
    this.#facts.push(Object.freeze({ ...fact, factId: this.#nextFactId++ }) as RendererVideoRealizedFact);
  }

  #disposeActive(): void {
    const active = this.#active;
    this.#active = null;
    ++this.#generation;
    if (active === null) return;
    active.element.pause();
    active.element.removeAttribute('src');
    active.element.load();
    active.element.remove();
    URL.revokeObjectURL(active.url);
  }
}

interface VideoDiagnostic { readonly code: 'hostFailure'; readonly sequence: number; readonly handle: number | null; readonly message: string; }
type PendingVideoFact =
  | { readonly kind: 'completed'; readonly handle: VideoPlaybackHandle }
  | { readonly kind: 'skipped'; readonly handle: VideoPlaybackHandle }
  | { readonly kind: 'failed'; readonly handle: VideoPlaybackHandle; readonly code: 'decodeFailed' | 'playbackBlocked' | 'hostFailure' };
