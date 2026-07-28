import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';

import { MAX_STUDIO_ADAPTER_RESPONSE_BYTES } from '../libs/adapter-client/src/index.js';

// Retain enough recent diagnostics for an actionable failure without allowing
// an adapter's stderr stream to become unbounded host state.
const MAX_ADAPTER_STDERR_BYTES = 64 * 1024;

interface PendingExchange {
  readonly resolve: (line: string) => void;
  readonly reject: (error: Error) => void;
}

export class StudioAdapterResponseLimitError extends Error {
  readonly code = 'studio_adapter_response_too_large';

  constructor(
    readonly limitBytes: number,
    readonly actualBytes: number,
  ) {
    super(
      `Studio adapter response is ${String(actualBytes)} bytes; `
      + `the protocol limit is ${String(limitBytes)} bytes`,
    );
    this.name = 'StudioAdapterResponseLimitError';
  }
}

/**
 * Own one serial JSONL adapter process.
 *
 * The response bound is a liveness guard: a wedged adapter must not grow the
 * host heap forever while the host waits for a newline. An oversized complete
 * line fails only that exchange. Its bytes are drained without retention so a
 * conforming next request can continue on the same child process.
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

  constructor(binary: string, responseByteLimit = MAX_STUDIO_ADAPTER_RESPONSE_BYTES) {
    if (!Number.isSafeInteger(responseByteLimit) || responseByteLimit < 1) {
      throw new TypeError('Studio adapter response byte limit must be a positive safe integer');
    }
    this.#responseByteLimit = responseByteLimit;
    this.#child = spawn(binary, [], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.#child.stdout.on('data', (chunk: Buffer) => this.#receive(chunk));
    this.#child.stderr.setEncoding('utf8');
    this.#child.stderr.on('data', (chunk: string) => {
      this.#stderr = `${this.#stderr}${chunk}`.slice(-MAX_ADAPTER_STDERR_BYTES);
    });
    this.#child.on('error', (error) => this.#fail(error));
    this.#child.on('exit', (code, signal) => {
      this.#fail(new Error(
        `Studio adapter exited code=${String(code)} signal=${String(signal)}${
          this.#stderr.length === 0 ? '' : `: ${this.#stderr}`
        }`,
      ));
    });
  }

  exchange(requestLine: string): Promise<string> {
    const exchange = this.#serial.then(() => this.#exchangeOne(requestLine));
    this.#serial = exchange.then(() => undefined, () => undefined);
    return exchange;
  }

  close(): void {
    if (!this.#child.stdin.destroyed) this.#child.stdin.end();
  }

  #exchangeOne(requestLine: string): Promise<string> {
    if (this.#closedError !== null) return Promise.reject(this.#closedError);
    if (this.#pending !== null) {
      return Promise.reject(new Error('Studio adapter exchange overlap'));
    }
    return new Promise((resolvePromise, rejectPromise) => {
      this.#pending = { resolve: resolvePromise, reject: rejectPromise };
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
      pending.reject(new Error('Studio adapter emitted more than one response'));
      this.#fail(new Error('Studio adapter emitted more than one response'));
      this.#child.kill();
      return;
    }
    if (oversized) {
      pending.reject(new StudioAdapterResponseLimitError(this.#responseByteLimit, actualBytes));
      return;
    }
    pending.resolve((responseBytes as Buffer).toString('utf8').replace(/\r$/, ''));
  }

  #retainResponseBytes(bytes: Buffer): void {
    this.#responseByteCount += bytes.byteLength;
    if (this.#discardingOversizedResponse) return;
    if (this.#responseByteCount > this.#responseByteLimit) {
      this.#discardingOversizedResponse = true;
      this.#responseChunks = [];
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
    pending?.reject(error);
  }
}
