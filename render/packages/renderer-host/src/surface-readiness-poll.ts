export interface RendererSurfaceReadinessPollScheduler {
  readonly request: (callback: () => void, delayMs: number) => () => void;
}

export interface RendererSurfaceReadinessPollOptions {
  readonly isAccelerated: () => boolean;
  readonly isReady: () => boolean;
  readonly onReady: () => void;
  readonly scheduler?: RendererSurfaceReadinessPollScheduler;
}

const ACCELERATED_READINESS_DELAYS_MS = Object.freeze([0, 1, 2, 4, 4, 4]);

const BROWSER_READINESS_SCHEDULER: RendererSurfaceReadinessPollScheduler = {
  request: (callback, delayMs) => {
    const handle = globalThis.setTimeout(callback, delayMs);
    return () => globalThis.clearTimeout(handle);
  },
};

/**
 * Advances optional accelerated-backend readiness between display callbacks.
 *
 * This owner never submits a frame and never requests another animation frame.
 * It runs one bounded burst after accepted demand so WebGL fence and timer-query
 * state can become observable before the surface's existing RAF callback. Slow
 * software and unknown renderers retain RAF-only completion polling.
 */
export class RendererSurfaceReadinessPoll {
  readonly #isAccelerated: () => boolean;
  readonly #isReady: () => boolean;
  readonly #onReady: () => void;
  readonly #scheduler: RendererSurfaceReadinessPollScheduler;
  #attempt = 0;
  #cancelPending: (() => void) | null = null;

  constructor(options: RendererSurfaceReadinessPollOptions) {
    this.#isAccelerated = options.isAccelerated;
    this.#isReady = options.isReady;
    this.#onReady = options.onReady;
    this.#scheduler = options.scheduler ?? BROWSER_READINESS_SCHEDULER;
  }

  request(): void {
    if (this.#cancelPending !== null || !this.#isAccelerated()) {
      return;
    }
    this.#attempt = 0;
    this.#schedule();
  }

  cancel(): void {
    if (this.#cancelPending !== null) {
      this.#cancelPending();
      this.#cancelPending = null;
    }
    this.#attempt = 0;
  }

  #schedule(): void {
    const delayMs = ACCELERATED_READINESS_DELAYS_MS[this.#attempt];
    if (delayMs === undefined) {
      this.#attempt = 0;
      return;
    }
    this.#cancelPending = this.#scheduler.request(() => {
      this.#cancelPending = null;
      if (this.#isReady()) {
        this.#attempt = 0;
        this.#onReady();
        return;
      }
      this.#attempt += 1;
      this.#schedule();
    }, delayMs);
  }
}
