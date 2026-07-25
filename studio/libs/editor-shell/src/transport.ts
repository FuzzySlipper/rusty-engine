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
