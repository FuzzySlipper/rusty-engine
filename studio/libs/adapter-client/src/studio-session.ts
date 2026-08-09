import {
  decodeStudioHostStatus,
  type StudioHostStatus,
} from './host-status.js';
import {
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  decodeStudioAdapterResponse,
  type AdapterDescription,
  type StudioProjectReadout,
} from './protocol.js';

export const STUDIO_SESSION_SCHEMA_VERSION = 1 as const;

export interface StudioSessionOpenedResponse {
  readonly schemaVersion: typeof STUDIO_SESSION_SCHEMA_VERSION;
  readonly type: 'studioSessionOpened';
  readonly adapter: AdapterDescription;
  readonly project: StudioProjectReadout;
  readonly hostStatus: StudioHostStatus;
}

export function decodeStudioSessionOpenedResponse(input: unknown): StudioSessionOpenedResponse {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    throw new TypeError('Studio session response must be an object');
  }
  const value = input as Record<string, unknown>;
  const keys = Object.keys(value).sort();
  if (keys.join(',') !== 'adapter,hostStatus,project,schemaVersion,type') {
    throw new TypeError('Studio session response has unknown or missing fields');
  }
  if (value['schemaVersion'] !== STUDIO_SESSION_SCHEMA_VERSION || value['type'] !== 'studioSessionOpened') {
    throw new TypeError('Studio session response has an unsupported schema');
  }
  const described = decodeStudioAdapterResponse({
    type: 'described',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'studio-session-describe',
    adapter: value['adapter'],
  });
  if (described.type !== 'described') throw new TypeError('Studio session adapter is invalid');
  const opened = decodeStudioAdapterResponse({
    type: 'projectOpened',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'studio-session-open',
    project: value['project'],
  });
  if (opened.type !== 'projectOpened') throw new TypeError('Studio session project is invalid');
  const hostStatus = decodeStudioHostStatus(value['hostStatus']);
  return Object.freeze({
    schemaVersion: STUDIO_SESSION_SCHEMA_VERSION,
    type: 'studioSessionOpened',
    adapter: described.adapter,
    project: opened.project,
    hostStatus,
  }) as StudioSessionOpenedResponse;
}
