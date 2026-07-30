export type RendererGpuSubmissionFencePoll =
  | 'failed'
  | 'pending'
  | 'signaled';

export interface RendererGpuSubmissionFenceDriver {
  readonly create: () => object | null;
  readonly delete: (fence: object) => void;
  readonly flush: () => void;
  readonly poll: (fence: object) => RendererGpuSubmissionFencePoll;
}

/**
 * Bounds automatic WebGL work to one submitted GPU command stream.
 *
 * Explicit rendering remains caller-owned. The browser surface consults this
 * fence only before its automatic loop submits another frame.
 */
export class RendererGpuSubmissionFence {
  readonly #driver: RendererGpuSubmissionFenceDriver | null;
  #disabled = false;
  #pending: object | null = null;

  constructor(driver: RendererGpuSubmissionFenceDriver | null) {
    this.#driver = driver;
  }

  ready(): boolean {
    if (this.#driver === null || this.#disabled || this.#pending === null) {
      return true;
    }
    let status: RendererGpuSubmissionFencePoll;
    try {
      status = this.#driver.poll(this.#pending);
    } catch {
      this.#disable();
      return true;
    }
    if (status === 'pending') {
      return false;
    }
    if (status === 'failed') {
      this.#disable();
      return true;
    }
    try {
      this.#driver.delete(this.#pending);
    } catch {
      this.#disabled = true;
    }
    this.#pending = null;
    return true;
  }

  submitted(): void {
    if (this.#driver === null || this.#disabled) {
      return;
    }
    try {
      if (this.#pending !== null) {
        this.#driver.delete(this.#pending);
        this.#pending = null;
      }
      this.#pending = this.#driver.create();
      if (this.#pending === null) {
        this.#disabled = true;
        return;
      }
      this.#driver.flush();
    } catch {
      this.#disable();
    }
  }

  dispose(): void {
    this.#disable();
  }

  #disable(): void {
    if (this.#driver !== null && this.#pending !== null) {
      try {
        this.#driver.delete(this.#pending);
      } catch {
        // Synchronization is an optional pacing mechanism. Context-loss and
        // driver cleanup failures must not become a renderer lifecycle failure.
      }
    }
    this.#pending = null;
    this.#disabled = true;
  }
}
