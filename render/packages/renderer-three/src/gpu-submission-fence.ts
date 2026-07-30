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

export interface RendererGpuSubmissionFenceOptions {
  readonly maximumPendingSubmissions?: number;
}

export type RendererGpuSubmissionFenceMode =
  | 'active'
  | 'disabled'
  | 'unsupported';

/** Read-only completion-fence capacity; reading it never polls WebGL. */
export interface RendererGpuSubmissionFenceSample {
  readonly schemaVersion: 1;
  readonly mode: RendererGpuSubmissionFenceMode;
  readonly maximumPendingSubmissions: number;
  readonly pendingSubmissionCount: number;
}

/**
 * Bounds automatic WebGL work to a fixed number of submitted command streams.
 *
 * Explicit rendering remains caller-owned. The browser surface consults this
 * fence only before its automatic loop submits another frame.
 */
export class RendererGpuSubmissionFence {
  readonly #driver: RendererGpuSubmissionFenceDriver | null;
  readonly #maximumPendingSubmissions: number;
  #disabled = false;
  readonly #pending: object[] = [];

  constructor(
    driver: RendererGpuSubmissionFenceDriver | null,
    options: RendererGpuSubmissionFenceOptions = {},
  ) {
    this.#driver = driver;
    this.#maximumPendingSubmissions = positiveInteger(
      options.maximumPendingSubmissions ?? 1,
      'maximum pending GPU submissions',
    );
  }

  ready(maximumPendingSubmissions = this.#maximumPendingSubmissions): boolean {
    const admissionLimit = Math.min(
      this.#maximumPendingSubmissions,
      positiveInteger(
        maximumPendingSubmissions,
        'automatic pending GPU submission limit',
      ),
    );
    if (this.#driver === null || this.#disabled) {
      return true;
    }
    for (let index = this.#pending.length - 1; index >= 0; index -= 1) {
      const fence = this.#pending[index];
      if (fence === undefined) {
        continue;
      }
      let status: RendererGpuSubmissionFencePoll;
      try {
        status = this.#driver.poll(fence);
      } catch {
        this.#disable();
        return true;
      }
      if (status === 'failed') {
        this.#disable();
        return true;
      }
      if (status === 'signaled') {
        this.#delete(fence);
        this.#pending.splice(index, 1);
      }
    }
    return this.#pending.length < admissionLimit;
  }

  submitted(): void {
    if (this.#driver === null || this.#disabled) {
      return;
    }
    try {
      // Automatic callers consult ready() first. An explicit caller is allowed
      // to replace the oldest observation; the newly inserted fence covers all
      // earlier commands in WebGL submission order.
      while (this.#pending.length >= this.#maximumPendingSubmissions) {
        const oldest = this.#pending.shift();
        if (oldest !== undefined) {
          this.#delete(oldest);
        }
      }
      const fence = this.#driver.create();
      if (fence === null) {
        this.#disabled = true;
        return;
      }
      this.#pending.push(fence);
      this.#driver.flush();
    } catch {
      this.#disable();
    }
  }

  sample(): RendererGpuSubmissionFenceSample {
    return Object.freeze({
      schemaVersion: 1,
      mode: this.#driver === null
        ? 'unsupported'
        : this.#disabled
          ? 'disabled'
          : 'active',
      maximumPendingSubmissions: this.#maximumPendingSubmissions,
      pendingSubmissionCount: this.#pending.length,
    });
  }

  dispose(): void {
    this.#disable();
  }

  #disable(): void {
    for (const fence of this.#pending) {
      this.#delete(fence);
    }
    this.#pending.length = 0;
    this.#disabled = true;
  }

  #delete(fence: object): void {
    if (this.#driver === null) {
      return;
    }
    try {
      this.#driver.delete(fence);
    } catch {
      // Synchronization is an optional pacing mechanism. Context-loss and
      // driver cleanup failures must not become a renderer lifecycle failure.
    }
  }
}

function positiveInteger(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new RangeError(`${label} must be a positive safe integer`);
  }
  return value;
}
