import {
  buildDefaultStudioHostUserSettings,
  parseStudioHostUserSettings,
  serializeStudioHostUserSettings,
  type StudioHostUserSettingsArtifact,
} from './settings.js';

export interface StudioUserSettingsSnapshot {
  readonly canonicalProjectRoot: string;
  readonly projectKey: string;
  readonly path: string;
  readonly artifact: StudioHostUserSettingsArtifact;
  readonly sha256: string | null;
  readonly writesEnabled: boolean;
  readonly message: string;
}

export type StudioSettingsFetch = (
  input: string,
  init?: RequestInit,
) => Promise<Pick<Response, 'ok' | 'status' | 'json'>>;

export class HttpStudioUserSettingsClient {
  readonly #endpoint: string;
  readonly #fetch: StudioSettingsFetch;

  constructor(
    endpoint = '/api/studio-user-settings',
    fetchImplementation: StudioSettingsFetch = globalThis.fetch.bind(globalThis),
  ) {
    this.#endpoint = endpoint;
    this.#fetch = fetchImplementation;
  }

  async load(projectRoot: string): Promise<StudioUserSettingsSnapshot> {
    const response = await this.#fetch(
      `${this.#endpoint}?projectRoot=${encodeURIComponent(projectRoot)}`,
      { method: 'GET' },
    );
    const payload = await response.json() as unknown;
    const readout = decodeReadPayload(payload, response.ok, response.status);
    if (readout.text === null) {
      return {
        canonicalProjectRoot: readout.canonicalProjectRoot,
        projectKey: readout.projectKey,
        path: readout.path,
        artifact: buildDefaultStudioHostUserSettings(readout.projectKey),
        sha256: null,
        writesEnabled: true,
        message: 'Host-user defaults are active for this canonical project root.',
      };
    }
    const parsed = parseStudioHostUserSettings(readout.text);
    if (parsed.status !== 'loaded') {
      return {
        canonicalProjectRoot: readout.canonicalProjectRoot,
        projectKey: readout.projectKey,
        path: readout.path,
        artifact: buildDefaultStudioHostUserSettings(readout.projectKey),
        sha256: readout.sha256,
        writesEnabled: false,
        message: parsed.diagnostic,
      };
    }
    if (parsed.artifact.projectKey !== readout.projectKey) {
      return {
        canonicalProjectRoot: readout.canonicalProjectRoot,
        projectKey: readout.projectKey,
        path: readout.path,
        artifact: buildDefaultStudioHostUserSettings(readout.projectKey),
        sha256: readout.sha256,
        writesEnabled: false,
        message: 'Host-user settings identity does not match the canonical project root.',
      };
    }
    return {
      canonicalProjectRoot: readout.canonicalProjectRoot,
      projectKey: readout.projectKey,
      path: readout.path,
      artifact: parsed.artifact,
      sha256: readout.sha256,
      writesEnabled: true,
      message: 'Host-user settings loaded for this canonical project root.',
    };
  }

  async save(
    projectRoot: string,
    artifact: StudioHostUserSettingsArtifact,
    expectedHash: string | null,
  ): Promise<{ readonly sha256: string; readonly path: string }> {
    const response = await this.#fetch(this.#endpoint, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        projectRoot,
        text: serializeStudioHostUserSettings(artifact),
        expectedHash,
      }),
    });
    const payload = await response.json() as unknown;
    if (!response.ok || !isRecord(payload) || payload['ok'] !== true
      || typeof payload['sha256'] !== 'string' || typeof payload['path'] !== 'string') {
      throw new Error(errorMessage(payload, response.status));
    }
    return { sha256: payload['sha256'], path: payload['path'] };
  }
}

function decodeReadPayload(
  payload: unknown,
  responseOk: boolean,
  status: number,
): {
  readonly canonicalProjectRoot: string;
  readonly projectKey: string;
  readonly path: string;
  readonly text: string | null;
  readonly sha256: string | null;
} {
  if (!responseOk || !isRecord(payload) || payload['ok'] !== true
    || typeof payload['canonicalProjectRoot'] !== 'string'
    || typeof payload['projectKey'] !== 'string'
    || typeof payload['path'] !== 'string'
    || (payload['text'] !== null && typeof payload['text'] !== 'string')
    || (payload['sha256'] !== null && typeof payload['sha256'] !== 'string')) {
    throw new Error(errorMessage(payload, status));
  }
  return {
    canonicalProjectRoot: payload['canonicalProjectRoot'],
    projectKey: payload['projectKey'],
    path: payload['path'],
    text: payload['text'],
    sha256: payload['sha256'],
  };
}

function errorMessage(payload: unknown, status: number): string {
  if (isRecord(payload) && typeof payload['message'] === 'string') return payload['message'];
  return `Studio user-settings host rejected the request with HTTP ${String(status)}.`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
