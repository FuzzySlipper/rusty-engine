/**
 * Transport-neutral client for the product-owned generated live-debug catalog.
 * Descriptor data is read-only help/completion data; this client never derives
 * command schemas or dispatches anything except one command-line string.
 */
import type {
  ProductHostDebugCatalog,
  ProductHostDebugCommandDescriptor,
  ProductHostDiagnosticsReadRequest,
  ProductHostDiagnosticsReadResponse,
  ProductHostErrorResponse,
  RuntimeDiagnosticEvent,
} from './generated/contracts.js';

// The shapes the Engine host answers with, generated from their Rust declarations.
export * from './generated/contracts.js';

/** A command's outcome: the host answers 200 or 422 with the message as text. */
export interface LiveDebugResult {
  readonly succeeded: boolean;
  readonly message: string;
}

export interface LiveDebugTransport {
  catalog(signal?: AbortSignal): Promise<ProductHostDebugCatalog>;
  execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult>;
  diagnostics?(after?: string, signal?: AbortSignal): Promise<ProductHostDiagnosticsReadResponse>;
}

export interface LiveDebugHttpTransportOptions {
  /** Defaults to the current page origin, preserving same-origin product-host use. */
  readonly origin?: string;
  readonly fetch?: typeof globalThis.fetch;
}

const CATALOG_PATH = '/__rusty/product/runtime/debug/catalog';
const EXECUTE_PATH = '/__rusty/product/runtime/debug/execute';
const DIAGNOSTICS_READ_PATH = '/__rusty/product/runtime/diagnostics/read';

/** Creates the default same-origin HTTP transport without owning UI state. */
export function createLiveDebugHttpTransport(options: LiveDebugHttpTransportOptions = {}): LiveDebugTransport {
  const request = options.fetch ?? globalThis.fetch;
  const origin = options.origin ?? globalThis.location?.origin;
  if (origin === undefined || origin === 'null') throw new Error('A live-debug HTTP origin is required outside a browser page.');
  const url = (path: string): string => new URL(path, origin).toString();
  return {
    async catalog(signal?: AbortSignal): Promise<ProductHostDebugCatalog> {
      const response = await request(url(CATALOG_PATH), { method: 'GET', signal });
      if (response.status === 404) return { available: false, commands: [] };
      return await requireSuccess(response) as ProductHostDebugCatalog;
    },
    async execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult> {
      const response = await request(url(EXECUTE_PATH), {
        method: 'POST', signal, headers: { 'content-type': 'text/plain; charset=utf-8' }, body: command,
      });
      const message = await response.text();
      if (response.status === 200) return { succeeded: true, message };
      if (response.status === 422) return { succeeded: false, message };
      throw new Error(message || `Live-debug host request failed (${response.status}).`);
    },
    async diagnostics(after?: string, signal?: AbortSignal): Promise<ProductHostDiagnosticsReadResponse> {
      const body: ProductHostDiagnosticsReadRequest = after === undefined ? {} : { after };
      const response = await request(url(DIAGNOSTICS_READ_PATH), {
        method: 'POST', signal, headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      });
      return await requireSuccess(response) as ProductHostDiagnosticsReadResponse;
    },
  };
}

/** Small UI/CLI-neutral helper for catalog-derived completion. */
export function completeLiveDebug(
  catalog: ProductHostDebugCatalog,
  prefix: string,
): readonly ProductHostDebugCommandDescriptor[] {
  return catalog.commands.filter((command) => command.name.startsWith(prefix));
}

async function requireSuccess(response: Response): Promise<unknown> {
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    // Host errors carry a code; anything else in front of it may not.
    const error = (body as ProductHostErrorResponse | null)?.error;
    throw new Error(`${error?.code ?? `HTTP_${response.status}`}: ${error?.diagnostic ?? 'Live-debug host request failed.'}`);
  }
  return body;
}

/**
 * Computes how old a diagnostic event is at the response read clock. This is
 * distinct from any age fact carried in the event's own fields.
 */
export function diagnosticEventAgeMilliseconds(
  batch: ProductHostDiagnosticsReadResponse,
  event: RuntimeDiagnosticEvent,
): number | null {
  const elapsed = BigInt(batch.readMonotonicNanoseconds) - BigInt(event.monotonicNanoseconds);
  if (elapsed < 0n || elapsed / 1_000_000n > BigInt(Number.MAX_SAFE_INTEGER)) return null;
  return Number(elapsed / 1_000_000n);
}
