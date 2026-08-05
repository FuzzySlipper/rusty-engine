export const STUDIO_HOST_STATUS_SCHEMA_VERSION = 1 as const;

export interface StudioConfiguredConsumerIdentity {
  readonly repository: string;
  readonly commit: string;
}

export interface StudioRunningAdapterIdentity {
  readonly adapterId: string;
  readonly adapterVersion: number;
  readonly protocolVersion: number;
  readonly buildCommit: string | null;
  readonly binarySha256: string;
}

export interface StudioHostStatus {
  readonly schemaVersion: typeof STUDIO_HOST_STATUS_SCHEMA_VERSION;
  readonly project: 'rusty-engine-studio';
  readonly status: 'ok';
  /**
   * `generic` is the trusted root-local development host. It may report the
   * exact Engine revision of an immutable rolling-development build, but it
   * never claims a certified downstream consumer. It is distinct from
   * `unmanaged`, which means an explicit adapter binary was supplied without
   * managed certification.
   */
  readonly mode: 'managed' | 'generic' | 'unmanaged';
  readonly engineSourceCommit: string | null;
  readonly configuredConsumer: StudioConfiguredConsumerIdentity | null;
  readonly activeProjectRoot: string | null;
  readonly activeProjectFile: string | null;
  readonly runningAdapter: StudioRunningAdapterIdentity;
}

const COMMIT = /^[0-9a-f]{40}$/u;
const SHA256 = /^[0-9a-f]{64}$/u;
const REPOSITORY = /^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u;

export function decodeStudioHostStatus(input: unknown): StudioHostStatus {
  const value = record(input, '$', [
    'schemaVersion',
    'project',
    'status',
    'mode',
    'engineSourceCommit',
    'configuredConsumer',
    'activeProjectRoot',
    'activeProjectFile',
    'runningAdapter',
  ]);
  if (value['schemaVersion'] !== STUDIO_HOST_STATUS_SCHEMA_VERSION) {
    fail('$.schemaVersion', 'must equal 1');
  }
  if (value['project'] !== 'rusty-engine-studio') {
    fail('$.project', 'must equal rusty-engine-studio');
  }
  if (value['status'] !== 'ok') fail('$.status', 'must equal ok');
  if (value['mode'] !== 'managed' && value['mode'] !== 'generic' && value['mode'] !== 'unmanaged') {
    fail('$.mode', 'must equal managed, generic, or unmanaged');
  }
  const engineSourceCommit = nullableCommit(value['engineSourceCommit'], '$.engineSourceCommit');
  const configuredConsumer = value['configuredConsumer'] === null
    ? null
    : configuredConsumerIdentity(value['configuredConsumer'], '$.configuredConsumer');
  const activeProjectRoot = nullablePath(value['activeProjectRoot'], '$.activeProjectRoot');
  const activeProjectFile = nullableProjectFile(value['activeProjectFile'], '$.activeProjectFile');
  if ((activeProjectRoot === null) !== (activeProjectFile === null)) {
    fail('$', 'activeProjectRoot and activeProjectFile must be both null or both present');
  }
  const runningAdapter = runningAdapterIdentity(value['runningAdapter'], '$.runningAdapter');
  if (value['mode'] === 'managed') {
    if (engineSourceCommit === null || configuredConsumer === null) {
      fail('$', 'managed status requires exact Engine and consumer identities');
    }
    if (runningAdapter.buildCommit !== configuredConsumer.commit) {
      fail('$.runningAdapter.buildCommit', 'must equal the configured consumer commit');
    }
  } else if (value['mode'] === 'generic') {
    if (configuredConsumer !== null || runningAdapter.buildCommit !== null) {
      fail('$', 'generic status must not claim a managed consumer or adapter build');
    }
  } else if (
    engineSourceCommit !== null
    || configuredConsumer !== null
    || runningAdapter.buildCommit !== null
  ) {
    fail('$', 'unmanaged status must not claim managed source identity');
  }
  return Object.freeze({
    schemaVersion: STUDIO_HOST_STATUS_SCHEMA_VERSION,
    project: 'rusty-engine-studio',
    status: 'ok',
    mode: value['mode'],
    engineSourceCommit,
    configuredConsumer,
    activeProjectRoot,
    activeProjectFile,
    runningAdapter,
  }) as StudioHostStatus;
}

function nullablePath(input: unknown, path: string): string | null {
  if (input === null) return null;
  if (typeof input !== 'string' || input.length === 0 || input.includes('\0')) {
    fail(path, 'must be null or a nonempty path without NUL');
  }
  return input;
}

function nullableProjectFile(input: unknown, path: string): string | null {
  if (input === null) return null;
  if (typeof input !== 'string' || input.length === 0 || input.includes('\0') || input.startsWith('/')) {
    fail(path, 'must be null or a nonempty project-relative path');
  }
  return input;
}

function configuredConsumerIdentity(
  input: unknown,
  path: string,
): StudioConfiguredConsumerIdentity {
  const value = record(input, path, ['repository', 'commit']);
  if (typeof value['repository'] !== 'string' || !REPOSITORY.test(value['repository'])) {
    fail(`${path}.repository`, 'must be one canonical public GitHub repository URL');
  }
  const commit = exactCommit(value['commit'], `${path}.commit`);
  return Object.freeze({ repository: value['repository'], commit });
}

function runningAdapterIdentity(input: unknown, path: string): StudioRunningAdapterIdentity {
  const value = record(input, path, [
    'adapterId',
    'adapterVersion',
    'protocolVersion',
    'buildCommit',
    'binarySha256',
  ]);
  if (typeof value['adapterId'] !== 'string' || value['adapterId'].length < 1) {
    fail(`${path}.adapterId`, 'must be a nonempty string');
  }
  if (!Number.isSafeInteger(value['adapterVersion']) || (value['adapterVersion'] as number) < 0) {
    fail(`${path}.adapterVersion`, 'must be a nonnegative safe integer');
  }
  if (!Number.isSafeInteger(value['protocolVersion']) || (value['protocolVersion'] as number) < 1) {
    fail(`${path}.protocolVersion`, 'must be a positive safe integer');
  }
  const buildCommit = nullableCommit(value['buildCommit'], `${path}.buildCommit`);
  if (typeof value['binarySha256'] !== 'string' || !SHA256.test(value['binarySha256'])) {
    fail(`${path}.binarySha256`, 'must be one lowercase SHA-256 digest');
  }
  return Object.freeze({
    adapterId: value['adapterId'],
    adapterVersion: value['adapterVersion'],
    protocolVersion: value['protocolVersion'],
    buildCommit,
    binarySha256: value['binarySha256'],
  }) as StudioRunningAdapterIdentity;
}

function nullableCommit(input: unknown, path: string): string | null {
  return input === null ? null : exactCommit(input, path);
}

function exactCommit(input: unknown, path: string): string {
  if (typeof input !== 'string' || !COMMIT.test(input)) {
    fail(path, 'must be one lowercase 40-character commit');
  }
  return input;
}

function record(
  input: unknown,
  path: string,
  keys: readonly string[],
): Record<string, unknown> {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    fail(path, 'must be an object');
  }
  const value = input as Record<string, unknown>;
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    fail(path, `must contain exactly ${keys.join(', ')}`);
  }
  return value;
}

function fail(path: string, message: string): never {
  throw new TypeError(`${path}: ${message}`);
}
