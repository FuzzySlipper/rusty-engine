import {
  decodeRenderFrameDiff,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';

export const STUDIO_ADAPTER_PROTOCOL_VERSION = 1 as const;

export type StudioAdapterRequest =
  | DescribeRequest
  | OpenProjectRequest
  | ReadProjectRequest
  | SetEntityTranslationRequest
  | CloseProjectRequest;

export interface DescribeRequest {
  readonly type: 'describe';
  readonly protocolVersion: 1;
  readonly requestId: string;
}

export interface OpenProjectRequest {
  readonly type: 'openProject';
  readonly protocolVersion: 1;
  readonly requestId: string;
  readonly root: string;
  readonly projectFile: string;
}

export interface ReadProjectRequest {
  readonly type: 'readProject';
  readonly protocolVersion: 1;
  readonly requestId: string;
}

export interface SetEntityTranslationRequest {
  readonly type: 'setEntityTranslation';
  readonly protocolVersion: 1;
  readonly requestId: string;
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly translation: readonly [number, number, number];
}

export interface CloseProjectRequest {
  readonly type: 'closeProject';
  readonly protocolVersion: 1;
  readonly requestId: string;
}

export type StudioAdapterResponse =
  | DescribedResponse
  | ProjectOpenedResponse
  | ProjectReadResponse
  | EntityTranslationAppliedResponse
  | ProjectClosedResponse
  | RejectedResponse;

export interface DescribedResponse extends ResponseHeader {
  readonly type: 'described';
  readonly adapter: AdapterDescription;
}

export interface ProjectOpenedResponse extends ResponseHeader {
  readonly type: 'projectOpened';
  readonly project: StudioProjectReadout;
}

export interface ProjectReadResponse extends ResponseHeader {
  readonly type: 'projectRead';
  readonly project: StudioProjectReadout;
}

export interface EntityTranslationAppliedResponse extends ResponseHeader {
  readonly type: 'entityTranslationApplied';
  readonly receipt: EntityTranslationReceipt;
  readonly project: StudioProjectReadout;
}

export interface ProjectClosedResponse extends ResponseHeader {
  readonly type: 'projectClosed';
}

export interface RejectedResponse {
  readonly type: 'rejected';
  readonly protocolVersion: 1;
  readonly requestId?: string;
  readonly error: AdapterRejection;
}

interface ResponseHeader {
  readonly protocolVersion: 1;
  readonly requestId: string;
}

export interface AdapterDescription {
  readonly adapterId: string;
  readonly adapterVersion: number;
  readonly protocolVersion: 1;
  readonly projectKind: string;
  readonly projectSchemaVersion: number;
  readonly operations: readonly [
    'describe',
    'openProject',
    'readProject',
    'setEntityTranslation',
    'closeProject',
  ];
}

export interface StudioProjectReadout {
  readonly identity: StudioProjectIdentity;
  readonly canonical: CanonicalOwnerContent;
  readonly inspections: OwnerInspections;
  readonly voxel?: Readonly<Record<string, unknown>>;
  readonly loadingBay: LoadingBayDomainReadout;
  readonly projection: RenderFrameDiff;
  readonly projectionReadout: ProjectionReadout;
}

export interface StudioProjectIdentity {
  readonly projectId: string;
  readonly name: string;
  readonly entryScene: string;
  readonly sourceSchemaVersion: number;
  readonly currentSchemaVersion: number;
  readonly projectHash: string;
  readonly sceneRevision: number;
  readonly relativeProjectFile: string;
}

export interface CanonicalOwnerContent {
  readonly projectJson: string;
  readonly assetCatalogJson: string;
  readonly authoredSceneJson: string;
  readonly entityStateJson: string;
  readonly contentManifestJson: string;
}

export interface OwnerInspections {
  readonly catalog: Readonly<Record<string, unknown>>;
  readonly scene: Readonly<Record<string, unknown>>;
  readonly entityState: Readonly<Record<string, unknown>>;
  readonly persistence: Readonly<Record<string, unknown>>;
}

export interface LoadingBayDomainReadout {
  readonly sceneName: string;
  readonly entityCount: number;
  readonly doorCount: number;
  readonly switchCount: number;
  readonly enemyCount: number;
  readonly encounterCount: number;
  readonly extractionBeaconCount: number;
  readonly navigatorCount: number;
  readonly playerControllerCount: number;
  readonly weaponCount: number;
  readonly voxelEnvironment: string;
}

export interface ProjectionReadout {
  readonly sourceRevision: number;
  readonly retainedEntities: number;
  readonly diagnostics: readonly ProjectionDiagnosticReadout[];
}

export interface ProjectionDiagnosticReadout {
  readonly code: string;
  readonly entityId: number;
  readonly asset: string;
  readonly assetKind?: string;
}

export interface EntityTranslationReceipt {
  readonly entityId: number;
  readonly translation: readonly [number, number, number];
  readonly projectHashBefore: string;
  readonly projectHashAfter: string;
  readonly sceneRevisionBefore: number;
  readonly sceneRevisionAfter: number;
  readonly contentCandidateHash: string;
}

export interface AdapterRejection {
  readonly code: string;
  readonly path?: string;
  readonly message: string;
}

export class StudioAdapterDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'StudioAdapterDecodeError';
  }
}

export function decodeStudioAdapterResponse(input: unknown): StudioAdapterResponse {
  const base = looseRecord(input, '$');
  const type = text(base['type'], '$.type');
  switch (type) {
    case 'described': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'adapter']);
      responseHeader(value);
      adapterDescription(value['adapter'], '$.adapter');
      return input as DescribedResponse;
    }
    case 'projectOpened':
    case 'projectRead': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId', 'project']);
      responseHeader(value);
      projectReadout(value['project'], '$.project');
      return input as ProjectOpenedResponse | ProjectReadResponse;
    }
    case 'entityTranslationApplied': {
      const value = record(input, '$', [
        'type',
        'protocolVersion',
        'requestId',
        'receipt',
        'project',
      ]);
      responseHeader(value);
      translationReceipt(value['receipt'], '$.receipt');
      projectReadout(value['project'], '$.project');
      return input as EntityTranslationAppliedResponse;
    }
    case 'projectClosed': {
      const value = record(input, '$', ['type', 'protocolVersion', 'requestId']);
      responseHeader(value);
      return input as ProjectClosedResponse;
    }
    case 'rejected': {
      const value = record(
        input,
        '$',
        ['type', 'protocolVersion', 'error'],
        ['requestId'],
      );
      protocolVersion(value['protocolVersion'], '$.protocolVersion');
      optional(value['requestId'], '$.requestId', text);
      rejection(value['error'], '$.error');
      return input as RejectedResponse;
    }
    default:
      fail('$.type', `${JSON.stringify(type)} is not a closed Studio adapter response`);
  }
}

function responseHeader(value: Readonly<Record<string, unknown>>): void {
  protocolVersion(value['protocolVersion'], '$.protocolVersion');
  text(value['requestId'], '$.requestId');
}

function protocolVersion(input: unknown, path: string): void {
  if (input !== STUDIO_ADAPTER_PROTOCOL_VERSION) {
    fail(path, `must equal ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)}`);
  }
}

function adapterDescription(input: unknown, path: string): void {
  const value = record(input, path, [
    'adapterId',
    'adapterVersion',
    'protocolVersion',
    'projectKind',
    'projectSchemaVersion',
    'operations',
  ]);
  text(value['adapterId'], `${path}.adapterId`);
  integer(value['adapterVersion'], `${path}.adapterVersion`);
  protocolVersion(value['protocolVersion'], `${path}.protocolVersion`);
  text(value['projectKind'], `${path}.projectKind`);
  integer(value['projectSchemaVersion'], `${path}.projectSchemaVersion`);
  const operations = list(value['operations'], `${path}.operations`).map((entry, index) =>
    text(entry, `${path}.operations[${String(index)}]`),
  );
  const expected = [
    'describe',
    'openProject',
    'readProject',
    'setEntityTranslation',
    'closeProject',
  ];
  if (operations.length !== expected.length || operations.some((entry, index) => entry !== expected[index])) {
    fail(`${path}.operations`, 'must name the protocol 1 operation set in order');
  }
}

function projectReadout(input: unknown, path: string): void {
  const value = record(
    input,
    path,
    ['identity', 'canonical', 'inspections', 'loadingBay', 'projection', 'projectionReadout'],
    ['voxel'],
  );
  projectIdentity(value['identity'], `${path}.identity`);
  canonicalOwnerContent(value['canonical'], `${path}.canonical`);
  ownerInspections(value['inspections'], `${path}.inspections`);
  optional(value['voxel'], `${path}.voxel`, looseRecord);
  loadingBayReadout(value['loadingBay'], `${path}.loadingBay`);
  try {
    decodeRenderFrameDiff(value['projection']);
  } catch (error) {
    fail(
      `${path}.projection`,
      error instanceof Error ? error.message : 'renderer contract rejected the frame',
    );
  }
  projectionReadout(value['projectionReadout'], `${path}.projectionReadout`);
}

function projectIdentity(input: unknown, path: string): void {
  const value = record(input, path, [
    'projectId',
    'name',
    'entryScene',
    'sourceSchemaVersion',
    'currentSchemaVersion',
    'projectHash',
    'sceneRevision',
    'relativeProjectFile',
  ]);
  for (const field of ['projectId', 'name', 'entryScene', 'projectHash', 'relativeProjectFile']) {
    text(value[field], `${path}.${field}`);
  }
  for (const field of ['sourceSchemaVersion', 'currentSchemaVersion', 'sceneRevision']) {
    integer(value[field], `${path}.${field}`);
  }
}

function canonicalOwnerContent(input: unknown, path: string): void {
  const fields = [
    'projectJson',
    'assetCatalogJson',
    'authoredSceneJson',
    'entityStateJson',
    'contentManifestJson',
  ];
  const value = record(input, path, fields);
  for (const field of fields) text(value[field], `${path}.${field}`);
}

function ownerInspections(input: unknown, path: string): void {
  const value = record(input, path, ['catalog', 'scene', 'entityState', 'persistence']);
  for (const field of ['catalog', 'scene', 'entityState', 'persistence']) {
    looseRecord(value[field], `${path}.${field}`);
  }
}

function loadingBayReadout(input: unknown, path: string): void {
  const textFields = ['sceneName', 'voxelEnvironment'];
  const countFields = [
    'entityCount',
    'doorCount',
    'switchCount',
    'enemyCount',
    'encounterCount',
    'extractionBeaconCount',
    'navigatorCount',
    'playerControllerCount',
    'weaponCount',
  ];
  const value = record(input, path, [...textFields, ...countFields]);
  for (const field of textFields) text(value[field], `${path}.${field}`);
  for (const field of countFields) integer(value[field], `${path}.${field}`);
}

function projectionReadout(input: unknown, path: string): void {
  const value = record(input, path, ['sourceRevision', 'retainedEntities', 'diagnostics']);
  integer(value['sourceRevision'], `${path}.sourceRevision`);
  integer(value['retainedEntities'], `${path}.retainedEntities`);
  list(value['diagnostics'], `${path}.diagnostics`).forEach((entry, index) => {
    const itemPath = `${path}.diagnostics[${String(index)}]`;
    const item = record(entry, itemPath, ['code', 'entityId', 'asset'], ['assetKind']);
    text(item['code'], `${itemPath}.code`);
    integer(item['entityId'], `${itemPath}.entityId`);
    text(item['asset'], `${itemPath}.asset`);
    optional(item['assetKind'], `${itemPath}.assetKind`, text);
  });
}

function translationReceipt(input: unknown, path: string): void {
  const value = record(input, path, [
    'entityId',
    'translation',
    'projectHashBefore',
    'projectHashAfter',
    'sceneRevisionBefore',
    'sceneRevisionAfter',
    'contentCandidateHash',
  ]);
  integer(value['entityId'], `${path}.entityId`);
  vector3(value['translation'], `${path}.translation`);
  for (const field of ['projectHashBefore', 'projectHashAfter', 'contentCandidateHash']) {
    text(value[field], `${path}.${field}`);
  }
  integer(value['sceneRevisionBefore'], `${path}.sceneRevisionBefore`);
  integer(value['sceneRevisionAfter'], `${path}.sceneRevisionAfter`);
}

function rejection(input: unknown, path: string): void {
  const value = record(input, path, ['code', 'message'], ['path']);
  text(value['code'], `${path}.code`);
  text(value['message'], `${path}.message`);
  optional(value['path'], `${path}.path`, text);
}

function vector3(input: unknown, path: string): readonly [number, number, number] {
  const value = list(input, path);
  if (value.length !== 3) fail(path, 'must have exactly 3 entries');
  return [
    finiteNumber(value[0], `${path}[0]`),
    finiteNumber(value[1], `${path}[1]`),
    finiteNumber(value[2], `${path}[2]`),
  ];
}

function record(
  input: unknown,
  path: string,
  required: readonly string[],
  optionalFields: readonly string[] = [],
): Readonly<Record<string, unknown>> {
  const value = looseRecord(input, path);
  for (const field of required) {
    if (!Object.hasOwn(value, field)) fail(`${path}.${field}`, 'is required');
  }
  const allowed = new Set([...required, ...optionalFields]);
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) fail(`${path}.${field}`, 'is unknown');
  }
  return value;
}

function looseRecord(input: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    fail(path, 'must be an object');
  }
  return input as Readonly<Record<string, unknown>>;
}

function list(input: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(input)) fail(path, 'must be an array');
  return input;
}

function text(input: unknown, path: string): string {
  if (typeof input !== 'string') fail(path, 'must be a string');
  return input;
}

function integer(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isSafeInteger(input) || input < 0) {
    fail(path, 'must be a nonnegative safe integer');
  }
  return input;
}

function finiteNumber(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) {
    fail(path, 'must be a finite number');
  }
  return input;
}

function optional(
  input: unknown,
  path: string,
  validate: (input: unknown, path: string) => unknown,
): void {
  if (input !== undefined) validate(input, path);
}

function fail(path: string, message: string): never {
  throw new StudioAdapterDecodeError(`${path}: ${message}`);
}
