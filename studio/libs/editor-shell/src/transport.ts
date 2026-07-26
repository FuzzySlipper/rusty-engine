import {
  MAX_STUDIO_ADAPTER_REQUEST_BYTES,
  MAX_STUDIO_ADAPTER_RESPONSE_BYTES,
  type StudioAdapterRequest,
  type StudioAdapterTransport,
} from '@rusty-engine/studio-adapter-client';

export type StudioFetch = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, 'ok' | 'status' | 'text' | 'headers'>>;

export class HttpStudioAdapterTransport implements StudioAdapterTransport {
  readonly #endpoint: string;
  readonly #fetch: StudioFetch;

  constructor(
    endpoint = '/api/studio-adapter',
    fetchImplementation: StudioFetch = globalThis.fetch.bind(globalThis),
  ) {
    this.#endpoint = endpoint;
    this.#fetch = fetchImplementation;
  }

  async exchange(request: StudioAdapterRequest): Promise<unknown> {
    const body = JSON.stringify(request);
    if (new TextEncoder().encode(body).byteLength > MAX_STUDIO_ADAPTER_REQUEST_BYTES) {
      throw new Error('Studio adapter request exceeds the protocol byte bound');
    }
    const response = await this.#fetch(this.#endpoint, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body,
    });
    const declaredLength = response.headers.get('content-length');
    if (declaredLength !== null && Number(declaredLength) > MAX_STUDIO_ADAPTER_RESPONSE_BYTES) {
      throw new Error('Studio adapter response exceeds the protocol byte bound');
    }
    const text = await response.text();
    if (new TextEncoder().encode(text).byteLength > MAX_STUDIO_ADAPTER_RESPONSE_BYTES) {
      throw new Error('Studio adapter response exceeds the protocol byte bound');
    }
    if (!response.ok) {
      throw new Error(`Studio host rejected the adapter exchange with HTTP ${String(response.status)}`);
    }
    try {
      return JSON.parse(text) as unknown;
    } catch {
      throw new Error('Studio host returned malformed JSON');
    }
  }
}

export interface StudioHostFileEntry {
  readonly name: string;
  readonly path: string;
  readonly kind: 'directory' | 'file';
}

export interface StudioHostDirectoryReadout {
  readonly directory: string;
  readonly parent: string | null;
  readonly entries: readonly StudioHostFileEntry[];
  readonly truncated: boolean;
}

export interface StudioHostPathRequest {
  readonly kind: 'file' | 'directory';
  readonly title: string;
  readonly initialPath: string;
  readonly extensions?: readonly string[];
}

export class HttpStudioHostFileBrowser {
  readonly #endpoint: string;
  readonly #fetch: typeof globalThis.fetch;

  constructor(
    endpoint = '/api/studio-host-files',
    fetchImplementation: typeof globalThis.fetch = globalThis.fetch.bind(globalThis),
  ) {
    this.#endpoint = endpoint;
    this.#fetch = fetchImplementation;
  }

  async list(
    directory: string,
    extensions: readonly string[] = [],
  ): Promise<StudioHostDirectoryReadout> {
    const query = new URLSearchParams({ directory });
    for (const extension of extensions) query.append('extension', extension);
    const response = await this.#fetch(`${this.#endpoint}?${query.toString()}`, {
      method: 'GET',
      headers: { accept: 'application/json' },
    });
    const decoded = await response.json() as unknown;
    if (!response.ok) throw new Error(hostFileError(decoded, response.status));
    return decodeHostDirectory(decoded);
  }
}

const MAX_STUDIO_RENDER_RESOURCE_BYTES = 64 * 1024 * 1024;

export type StudioRenderResourceFetch = (
  input: string,
  init: RequestInit,
) => Promise<Pick<Response, 'ok' | 'status' | 'arrayBuffer' | 'headers'>>;

export class HttpStudioRenderResourceClient {
  readonly #endpoint: string;
  readonly #fetch: StudioRenderResourceFetch;

  constructor(
    endpoint = '/api/studio-render-resource',
    fetchImplementation: StudioRenderResourceFetch = globalThis.fetch.bind(globalThis),
  ) {
    this.#endpoint = endpoint;
    this.#fetch = fetchImplementation;
  }

  async read(
    projectRoot: string,
    sourcePath: string,
    contentHash: string,
  ): Promise<ArrayBuffer> {
    const query = new URLSearchParams({ projectRoot, sourcePath, contentHash });
    const response = await this.#fetch(`${this.#endpoint}?${query.toString()}`, {
      method: 'GET',
      headers: { accept: 'model/gltf-binary' },
    });
    const declaredLength = response.headers.get('content-length');
    if (declaredLength !== null && Number(declaredLength) > MAX_STUDIO_RENDER_RESOURCE_BYTES) {
      throw new Error('Studio render resource exceeds the byte bound');
    }
    if (!response.ok) {
      throw new Error(`Studio host rejected the render resource with HTTP ${String(response.status)}`);
    }
    const bytes = await response.arrayBuffer();
    if (bytes.byteLength > MAX_STUDIO_RENDER_RESOURCE_BYTES) {
      throw new Error('Studio render resource exceeds the byte bound');
    }
    return bytes;
  }
}

function decodeHostDirectory(input: unknown): StudioHostDirectoryReadout {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    throw new TypeError('Studio host-file response must be an object');
  }
  const value = input as Record<string, unknown>;
  if (value['ok'] !== true
    || typeof value['directory'] !== 'string'
    || (value['parent'] !== null && typeof value['parent'] !== 'string')
    || typeof value['truncated'] !== 'boolean'
    || !Array.isArray(value['entries'])) {
    throw new TypeError('Studio host-file response has an invalid shape');
  }
  const entries = value['entries'].map((entry): StudioHostFileEntry => {
    if (entry === null || typeof entry !== 'object' || Array.isArray(entry)) {
      throw new TypeError('Studio host-file entry must be an object');
    }
    const candidate = entry as Record<string, unknown>;
    if (typeof candidate['name'] !== 'string'
      || typeof candidate['path'] !== 'string'
      || (candidate['kind'] !== 'file' && candidate['kind'] !== 'directory')) {
      throw new TypeError('Studio host-file entry has an invalid shape');
    }
    return { name: candidate['name'], path: candidate['path'], kind: candidate['kind'] };
  });
  return {
    directory: value['directory'],
    parent: value['parent'] as string | null,
    entries,
    truncated: value['truncated'],
  };
}

function hostFileError(input: unknown, status: number): string {
  if (input !== null && typeof input === 'object' && !Array.isArray(input)) {
    const message = (input as Record<string, unknown>)['message'];
    if (typeof message === 'string') return message;
  }
  return `Studio host rejected file browsing with HTTP ${String(status)}`;
}
