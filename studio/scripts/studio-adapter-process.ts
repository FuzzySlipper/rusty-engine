import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';

import { MAX_STUDIO_ADAPTER_RESPONSE_BYTES } from '../libs/adapter-client/src/index.js';

// Retain enough recent diagnostics for an actionable failure without allowing
// an adapter's stderr stream to become unbounded host state.
const MAX_ADAPTER_STDERR_BYTES = 64 * 1024;
const ADAPTER_CLOSE_GRACE_MILLISECONDS = 2_000;

export interface AdapterProcessOptions {
  readonly args?: readonly string[];
  readonly cwd?: string;
}

interface PendingExchange {
  readonly resolve: (line: string) => void;
  readonly reject: (error: Error) => void;
  readonly finish: () => void;
  responseSettled: boolean;
}

export class StudioAdapterResponseLimitError extends Error {
  readonly code = 'studio_adapter_response_too_large';

  constructor(
    readonly limitBytes: number,
    readonly actualBytes: number,
  ) {
    super(
      `Studio adapter response exceeded the ${String(limitBytes)}-byte protocol limit `
      + `after receiving ${String(actualBytes)} bytes`,
    );
    this.name = 'StudioAdapterResponseLimitError';
  }
}

/**
 * Own one serial JSONL adapter process.
 *
 * The response bound is a liveness guard: a wedged adapter must not grow the
 * host heap forever while the host waits for a newline. An oversized line
 * rejects its caller as soon as the bound is crossed, while its remaining
 * bytes are drained without retention before the serial queue writes another
 * request. This preserves response attribution and keeps a conforming child
 * available for the next exchange.
 */
export class AdapterProcess {
  readonly #child: ChildProcessWithoutNullStreams;
  readonly #responseByteLimit: number;
  #pending: PendingExchange | null = null;
  #responseChunks: Buffer[] = [];
  #responseByteCount = 0;
  #discardingOversizedResponse = false;
  #stderr = '';
  #serial: Promise<void> = Promise.resolve();
  #closedError: Error | null = null;

  constructor(
    binary: string,
    responseByteLimit = MAX_STUDIO_ADAPTER_RESPONSE_BYTES,
    options: AdapterProcessOptions = {},
  ) {
    if (!Number.isSafeInteger(responseByteLimit) || responseByteLimit < 1) {
      throw new TypeError('Studio adapter response byte limit must be a positive safe integer');
    }
    this.#responseByteLimit = responseByteLimit;
    this.#child = spawn(binary, options.args ?? [], {
      cwd: options.cwd,
      // Adapters may be launchers which build or supervise another process.
      // Own a process group so host shutdown cannot leave those descendants
      // behind when a project is switched or the host exits.
      detached: true,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.#child.stdout.on('data', (chunk: Buffer) => this.#receive(chunk));
    this.#child.stderr.setEncoding('utf8');
    this.#child.stderr.on('data', (chunk: string) => {
      this.#stderr = `${this.#stderr}${chunk}`.slice(-MAX_ADAPTER_STDERR_BYTES);
    });
    this.#child.on('error', (error) => {
      this.#fail(error);
      // A spawn failure does not reliably produce a later exit event. The
      // failed child has no process group, so close() can finish immediately.
    });
    this.#child.on('exit', (code, signal) => {
      this.#fail(new Error(
        `Studio adapter exited code=${String(code)} signal=${String(signal)}${
          this.#stderr.length === 0 ? '' : `: ${this.#stderr}`
        }`,
      ));
    });
  }

  exchange(requestLine: string): Promise<string> {
    let resolveResponse!: (line: string) => void;
    let rejectResponse!: (error: Error) => void;
    const response = new Promise<string>((resolvePromise, rejectPromise) => {
      resolveResponse = resolvePromise;
      rejectResponse = rejectPromise;
    });
    const lifecycle = this.#serial.then(
      () => this.#exchangeOne(requestLine, resolveResponse, rejectResponse),
    );
    this.#serial = lifecycle.then(() => undefined, () => undefined);
    return response;
  }

  async close(): Promise<void> {
    if (!this.#child.stdin.destroyed) this.#child.stdin.end();
    if (await this.#waitForProcessGroup(ADAPTER_CLOSE_GRACE_MILLISECONDS)) return;
    this.#signalProcessGroup('SIGTERM');
    if (await this.#waitForProcessGroup(ADAPTER_CLOSE_GRACE_MILLISECONDS)) return;
    this.#signalProcessGroup('SIGKILL');
    if (!await this.#waitForProcessGroup(ADAPTER_CLOSE_GRACE_MILLISECONDS)) {
      throw new Error(
        `studio_adapter_process_group_cleanup_failed: process group ${String(this.#child.pid)} survived SIGKILL`,
      );
    }
  }

  async #waitForProcessGroup(milliseconds: number): Promise<boolean> {
    if (!this.#processGroupAlive()) return true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timedOut = new Promise<false>((resolve) => {
      timer = setTimeout(() => resolve(false), milliseconds);
    });
    const exited = (async () => {
      while (this.#processGroupAlive()) {
        await new Promise<void>((resolvePromise) => {
          setTimeout(resolvePromise, 25);
        });
      }
      return true;
    })();
    const result = await Promise.race([exited, timedOut]);
    clearTimeout(timer);
    return result;
  }

  #processGroupAlive(): boolean {
    const pid = this.#child.pid;
    if (pid === undefined) return false;
    try {
      process.kill(-pid, 0);
      return true;
    } catch {
      return false;
    }
  }

  #signalProcessGroup(signal: NodeJS.Signals): void {
    const pid = this.#child.pid;
    if (pid === undefined) return;
    try {
      process.kill(-pid, signal);
    } catch {
      // The group may have exited between the liveness probe and signal.
    }
  }

  #exchangeOne(
    requestLine: string,
    resolveResponse: (line: string) => void,
    rejectResponse: (error: Error) => void,
  ): Promise<void> {
    if (this.#closedError !== null) {
      rejectResponse(this.#closedError);
      return Promise.resolve();
    }
    if (this.#pending !== null) {
      rejectResponse(new Error('Studio adapter exchange overlap'));
      return Promise.resolve();
    }
    return new Promise((finish) => {
      this.#pending = {
        resolve: resolveResponse,
        reject: rejectResponse,
        finish,
        responseSettled: false,
      };
      this.#child.stdin.write(`${requestLine}\n`, (error) => {
        if (error !== null && error !== undefined) this.#fail(error);
      });
    });
  }

  #receive(chunk: Buffer): void {
    const newline = chunk.indexOf(0x0a);
    if (newline === -1) {
      this.#retainResponseBytes(chunk);
      return;
    }

    this.#retainResponseBytes(chunk.subarray(0, newline));
    const actualBytes = this.#responseByteCount;
    const oversized = this.#discardingOversizedResponse;
    const responseBytes = oversized ? null : Buffer.concat(this.#responseChunks, actualBytes);
    this.#resetResponseBuffer();

    const pending = this.#pending;
    this.#pending = null;
    if (pending === null) {
      this.#fail(new Error('Studio adapter emitted an unsolicited response'));
      this.#child.kill();
      return;
    }
    if (chunk.byteLength !== newline + 1) {
      const error = new Error('Studio adapter emitted more than one response');
      if (!pending.responseSettled) {
        pending.responseSettled = true;
        pending.reject(error);
      }
      pending.finish();
      this.#fail(error);
      this.#child.kill();
      return;
    }
    if (oversized) {
      if (!pending.responseSettled) {
        pending.responseSettled = true;
        pending.reject(new StudioAdapterResponseLimitError(this.#responseByteLimit, actualBytes));
      }
      pending.finish();
      return;
    }
    pending.responseSettled = true;
    pending.resolve((responseBytes as Buffer).toString('utf8').replace(/\r$/, ''));
    pending.finish();
  }

  #retainResponseBytes(bytes: Buffer): void {
    this.#responseByteCount += bytes.byteLength;
    if (this.#discardingOversizedResponse) return;
    if (this.#responseByteCount > this.#responseByteLimit) {
      this.#discardingOversizedResponse = true;
      this.#responseChunks = [];
      const pending = this.#pending;
      if (pending !== null && !pending.responseSettled) {
        pending.responseSettled = true;
        pending.reject(new StudioAdapterResponseLimitError(
          this.#responseByteLimit,
          this.#responseByteCount,
        ));
      }
      return;
    }
    this.#responseChunks.push(bytes);
  }

  #resetResponseBuffer(): void {
    this.#responseChunks = [];
    this.#responseByteCount = 0;
    this.#discardingOversizedResponse = false;
  }

  #fail(error: Error): void {
    if (this.#closedError === null) this.#closedError = error;
    const pending = this.#pending;
    this.#pending = null;
    if (pending !== null) {
      if (!pending.responseSettled) {
        pending.responseSettled = true;
        pending.reject(error);
      }
      pending.finish();
    }
  }
}
