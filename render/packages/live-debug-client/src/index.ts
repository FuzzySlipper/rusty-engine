/**
 * Transport-neutral client for the product-owned generated live-debug catalog.
 * Descriptor data is read-only help/completion data; this client never derives
 * command schemas or dispatches anything except one command-line string.
 */

export interface LiveDebugParameterDescriptor {
  readonly name: string;
  readonly type: string;
}

export interface LiveDebugCommandDescriptor {
  readonly name: string;
  readonly description: string;
  readonly parameters: readonly LiveDebugParameterDescriptor[];
}

export interface LiveDebugCatalog {
  readonly available: boolean;
  readonly commands: readonly LiveDebugCommandDescriptor[];
}

export interface LiveDebugResult {
  readonly succeeded: boolean;
  readonly message: string;
}

export interface LiveDebugTransport {
  catalog(signal?: AbortSignal): Promise<LiveDebugCatalog>;
  execute(command: string, signal?: AbortSignal): Promise<LiveDebugResult>;
}

export interface LiveDebugHttpTransportOptions {
  /** Defaults to the current page origin, preserving same-origin dev-host use. */
  readonly origin?: string;
  readonly fetch?: typeof globalThis.fetch;
}

const CATALOG_PATH = '/__rusty/product/runtime/debug/catalog';
const EXECUTE_PATH = '/__rusty/product/runtime/debug/execute';

/** Creates the default same-origin HTTP transport without owning UI state. */
export function createLiveDebugHttpTransport(options: LiveDebugHttpTransportOptions = {}): LiveDebugTransport {
  const request = options.fetch ?? globalThis.fetch;
  const origin = options.origin ?? globalThis.location?.origin;
  if (origin === undefined || origin === 'null') throw new Error('A live-debug HTTP origin is required outside a browser page.');
  const url = (path: string): string => new URL(path, origin).toString();
  return {
    async catalog(signal?: AbortSignal): Promise<LiveDebugCatalog> {
      const response = await request(url(CATALOG_PATH), { method: 'GET', signal });
      if (response.status === 404) return { available: false, commands: [] };
      return decodeCatalog(await requireSuccess(response));
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
  };
}

/** Small UI/CLI-neutral helper for catalog-derived completion. */
export function completeLiveDebug(catalog: LiveDebugCatalog, prefix: string): readonly LiveDebugCommandDescriptor[] {
  return catalog.commands.filter((command) => command.name.startsWith(prefix));
}

async function requireSuccess(response: Response): Promise<unknown> {
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const error = body as { error?: { code?: unknown; diagnostic?: unknown } } | null;
    const code = typeof error?.error?.code === 'string' ? error.error.code : `HTTP_${response.status}`;
    const diagnostic = typeof error?.error?.diagnostic === 'string' ? error.error.diagnostic : 'Live-debug host request failed.';
    throw new Error(`${code}: ${diagnostic}`);
  }
  return body;
}

function decodeCatalog(value: unknown): LiveDebugCatalog {
  const candidate = object(value);
  if (typeof candidate.available !== 'boolean' || !Array.isArray(candidate.commands)) throw new Error('Live-debug catalog response is invalid.');
  if (!candidate.available) {
    if (candidate.commands.length !== 0) throw new Error('Unavailable live-debug catalogs cannot carry commands.');
    return { available: false, commands: [] };
  }
  return { available: true, commands: candidate.commands.map(decodeCommand) };
}

function decodeCommand(value: unknown): LiveDebugCommandDescriptor {
  const candidate = object(value);
  if (typeof candidate.name !== 'string' || typeof candidate.description !== 'string' || !Array.isArray(candidate.parameters)) throw new Error('Live-debug command descriptor is invalid.');
  return { name: candidate.name, description: candidate.description, parameters: candidate.parameters.map(decodeParameter) };
}

function decodeParameter(value: unknown): LiveDebugParameterDescriptor {
  const candidate = object(value);
  if (typeof candidate.name !== 'string' || typeof candidate.type !== 'string') throw new Error('Live-debug parameter descriptor is invalid.');
  return { name: candidate.name, type: candidate.type };
}

function object(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('Live-debug response is invalid.');
  return value as Record<string, unknown>;
}
