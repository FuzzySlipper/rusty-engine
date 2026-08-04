import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { isAbsolute, relative, resolve } from 'node:path';

import {
  decodeStudioHostStatus,
  decodeStudioAdapterResponse,
  validateStudioEntityInspectorCompatibility,
  StudioAdapterClient,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  type StudioProjectReadout,
  type AdapterDescription,
  type StudioHostStatus,
} from '../libs/adapter-client/src/index.js';
import {
  AdapterProcess,
  type AdapterProcessOptions,
} from './studio-adapter-process.js';

export const STUDIO_ROOT_BOOTSTRAP_FILE = '.rusty-studio.json';
const MAX_BOOTSTRAP_BYTES = 64 * 1024;
const MAX_BOOTSTRAP_COMMAND_ARGUMENTS = 64;
const MAX_BOOTSTRAP_STRING_LENGTH = 4_096;

export interface StudioManagedHostIdentity {
  readonly engineSourceCommit: string;
  readonly consumerRepository: string;
  readonly consumerCommit: string;
  readonly adapterBuildCommit: string;
  readonly expectedAdapterId: string;
}

export interface StudioAdapterBootstrapManifest {
  readonly schemaVersion: 1;
  readonly command: readonly string[];
  readonly cwd: string;
}

export interface StudioAdapterHostOptions {
  readonly adapterBinary: string | undefined;
  readonly managedIdentity: StudioManagedHostIdentity | null;
}

export interface StudioSessionOpenResult {
  readonly adapter: AdapterDescription;
  readonly project: StudioProjectReadout;
  readonly hostStatus: StudioHostStatus;
}

interface AdapterSlot {
  readonly process: AdapterProcess;
  readonly description: AdapterDescription;
  readonly launchDigest: string;
  readonly root: string | null;
  readonly bootstrapFingerprint: string | null;
}

interface PendingSelection {
  readonly previous: AdapterSlot | null;
  readonly candidate: AdapterSlot;
  readonly previousRoot: string | null;
  readonly previousProjectFile: string | null;
}

/**
 * Owns the generic Studio adapter lifecycle. Root-local manifests are only a
 * trusted development bootstrap: they select a process command and working
 * directory. The adapter remains the owner of project schemas and semantics.
 */
export class StudioAdapterHost {
  readonly #managedIdentity: StudioManagedHostIdentity | null;
  readonly #fixedAdapter: boolean;
  #current: AdapterSlot | null = null;
  #pending: PendingSelection | null = null;
  #activeProjectRoot: string | null = null;
  #activeProjectFile: string | null = null;
  #selectionSerial: Promise<void> = Promise.resolve();
  #selectionBusy = false;
  #nextRequestId = 1;

  private constructor(options: StudioAdapterHostOptions) {
    this.#managedIdentity = options.managedIdentity;
    this.#fixedAdapter = options.adapterBinary !== undefined;
  }

  static async create(options: StudioAdapterHostOptions): Promise<StudioAdapterHost> {
    const host = new StudioAdapterHost(options);
    if (options.adapterBinary !== undefined) {
      host.#current = await host.#startFixedAdapter(options.adapterBinary);
    }
    return host;
  }

  hasAdapter(): boolean {
    return this.#current !== null;
  }

  status(): StudioHostStatus | null {
    const current = this.#current;
    if (current === null) return null;
    return decodeStudioHostStatus({
      schemaVersion: 1,
      project: 'rusty-engine-studio',
      status: 'ok',
      mode: this.#managedIdentity !== null
        ? 'managed'
        : this.#fixedAdapter
          ? 'unmanaged'
          : 'generic',
      engineSourceCommit: this.#managedIdentity?.engineSourceCommit ?? null,
      configuredConsumer: this.#managedIdentity === null ? null : {
        repository: this.#managedIdentity.consumerRepository,
        commit: this.#managedIdentity.consumerCommit,
      },
      activeProjectRoot: this.#activeProjectRoot,
      activeProjectFile: this.#activeProjectFile,
      runningAdapter: {
        adapterId: current.description.adapterId,
        adapterVersion: current.description.adapterVersion,
        protocolVersion: current.description.protocolVersion,
        buildCommit: this.#managedIdentity?.adapterBuildCommit ?? null,
        binarySha256: current.launchDigest,
      },
    });
  }

  async selectProject(root: string, projectFile: string): Promise<void> {
    if (this.#fixedAdapter) return;
    return this.#queueSelection(() => this.#selectProjectIfNeeded(root, projectFile));
  }

  async exchange(requestLine: string): Promise<string> {
    await this.#selectionSerial;
    return this.#exchangeCurrent(requestLine);
  }

  async openProject(root: string, projectFile: string): Promise<StudioSessionOpenResult> {
    return this.#queueSelection(async () => {
      await this.#selectProjectIfNeeded(root, projectFile);
      const responseLine = await this.#exchangeCurrent(JSON.stringify({
        type: 'openProject',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: `studio-session-open-${String(this.#nextRequestId++)}`,
        root,
        projectFile,
      }));
      const response = parseRequest(responseLine);
      if (response['type'] !== 'projectOpened' || response['project'] === undefined) {
        throw new Error('studio_project_open_rejected: adapter did not return projectOpened');
      }
      const current = this.#current;
      const hostStatus = this.status();
      if (current === null || hostStatus === null) {
        throw new Error('studio_session_not_active: adapter session ended before publication');
      }
      return {
        adapter: current.description,
        project: response['project'] as StudioProjectReadout,
        hostStatus,
      };
    });
  }

  #queueSelection<Result>(operationFactory: () => Promise<Result>): Promise<Result> {
    if (this.#selectionBusy) {
      return Promise.reject(new Error('studio_adapter_switch_busy: another project selection is active'));
    }
    this.#selectionBusy = true;
    const operation = this.#selectionSerial.then(operationFactory);
    this.#selectionSerial = operation.then(() => undefined, () => undefined);
    void operation.then(
      () => { this.#selectionBusy = false; },
      () => { this.#selectionBusy = false; },
    );
    return operation;
  }

  async #exchangeCurrent(requestLine: string): Promise<string> {
    const current = this.#current;
    if (current === null) {
      throw new Error('studio_adapter_not_selected: choose a project root with a .rusty-studio.json bootstrap');
    }
    const request = parseRequest(requestLine);
    if (this.#pending !== null && request.type !== 'describe' && request.type !== 'openProject') {
      throw new Error('studio_adapter_selection_pending: openProject must complete adapter selection before other operations');
    }
    try {
      const responseLine = await current.process.exchange(requestLine);
      if (request.type === 'openProject') {
        await this.#settlePendingOpen(request, responseLine);
      }
      return responseLine;
    } catch (error) {
      if (this.#pending !== null) await this.#rollbackPending();
      else if (this.#current === current) {
        this.#current = null;
        this.#activeProjectRoot = null;
        this.#activeProjectFile = null;
      }
      throw error;
    }
  }

  async #selectProjectIfNeeded(root: string, projectFile: string): Promise<void> {
    validateProjectSelection(root, projectFile);
    if (this.#fixedAdapter) return;
    const bootstrap = await readStudioAdapterBootstrap(root);
    const fingerprint = bootstrap.fingerprint;
    const current = this.#current;
    if (
      current !== null
      && current.root === root
      && current.bootstrapFingerprint === fingerprint
    ) {
      return;
    }
    const candidate = await this.#startBootstrapAdapter(root, bootstrap.manifest, fingerprint);
    const previous = current;
    this.#pending = {
      previous,
      candidate,
      previousRoot: this.#activeProjectRoot,
      previousProjectFile: this.#activeProjectFile,
    };
    this.#current = candidate;
    this.#activeProjectRoot = null;
    this.#activeProjectFile = null;
  }

  async close(): Promise<void> {
    const current = this.#current;
    const pending = this.#pending;
    this.#current = null;
    this.#pending = null;
    this.#activeProjectRoot = null;
    this.#activeProjectFile = null;
    const processes = new Set<AdapterProcess>();
    if (current !== null) processes.add(current.process);
    if (pending !== null) {
      processes.add(pending.candidate.process);
      if (pending.previous !== null) processes.add(pending.previous.process);
    }
    await Promise.all([...processes].map((process) => process.close()));
  }

  async #settlePendingOpen(
    request: Record<string, unknown>,
    responseLine: string,
  ): Promise<void> {
    const decoded = decodeStudioAdapterResponse(JSON.parse(responseLine) as unknown);
    if (decoded.type === 'projectOpened') {
      const current = this.#current;
      if (current === null) throw new Error('studio_session_not_active: adapter ended before project validation');
      validateStudioEntityInspectorCompatibility(current.description, decoded.project);
    }
    const pending = this.#pending;
    if (pending === null) {
      if (decoded.type === 'projectOpened') {
        this.#activeProjectRoot = request['root'] as string;
        this.#activeProjectFile = request['projectFile'] as string;
      }
      return;
    }
    if (decoded.type === 'projectOpened') {
      this.#pending = null;
      this.#activeProjectRoot = request['root'] as string;
      this.#activeProjectFile = request['projectFile'] as string;
      if (pending.previous !== null) await pending.previous.process.close();
      return;
    }
    await this.#rollbackPending();
  }

  async #rollbackPending(): Promise<void> {
    const pending = this.#pending;
    if (pending === null) return;
    this.#pending = null;
    this.#current = pending.previous;
    this.#activeProjectRoot = pending.previousRoot;
    this.#activeProjectFile = pending.previousProjectFile;
    await pending.candidate.process.close();
  }

  async #startFixedAdapter(binary: string): Promise<AdapterSlot> {
    if (!isAbsolute(binary)) throw new Error('--adapter-binary must be absolute');
    const metadata = await stat(binary);
    if (!metadata.isFile()) throw new Error('--adapter-binary must name a file');
    const bytes = await readFile(binary);
    return this.#startAdapter(binary, [], undefined, createHash('sha256').update(bytes).digest('hex'), null, null);
  }

  async #startBootstrapAdapter(
    root: string,
    manifest: StudioAdapterBootstrapManifest,
    fingerprint: string,
  ): Promise<AdapterSlot> {
    const cwd = resolve(root, manifest.cwd);
    const fromRoot = relative(root, cwd);
    if (fromRoot.startsWith('..') || isAbsolute(fromRoot)) {
      throw new Error('studio_adapter_bootstrap_cwd_outside_root: cwd must remain inside the project root');
    }
    const command = manifest.command[0];
    const args = manifest.command.slice(1);
    if (command === undefined) throw new Error('studio_adapter_bootstrap_invalid_command');
    const executable = command.includes('/') ? resolve(cwd, command) : command;
    if (command.includes('/')) {
      let metadata: Awaited<ReturnType<typeof stat>>;
      try {
        metadata = await stat(executable);
      } catch (error) {
        throw new Error(
          `studio_adapter_bootstrap_command_not_file: ${executable}: `
          + `${error instanceof Error ? error.message : String(error)}`,
          { cause: error },
        );
      }
      if (!metadata.isFile()) throw new Error(`studio_adapter_bootstrap_command_not_file: ${executable}`);
    }
    return this.#startAdapter(
      executable,
      args,
      { cwd } satisfies AdapterProcessOptions,
      fingerprint,
      root,
      fingerprint,
    );
  }

  async #startAdapter(
    executable: string,
    args: readonly string[],
    options: AdapterProcessOptions | undefined,
    launchDigest: string,
    root: string | null,
    bootstrapFingerprint: string | null,
  ): Promise<AdapterSlot> {
    const process = new AdapterProcess(executable, undefined, { ...options, args });
    try {
      const client = new StudioAdapterClient({
        exchange: async (request) => JSON.parse(
          await process.exchange(JSON.stringify(request)),
        ) as unknown,
      });
      let described: { readonly adapter: AdapterDescription };
      try {
        described = await client.describe();
      } catch (error) {
        throw new Error(
          `studio_adapter_handshake_failed: ${error instanceof Error ? error.message : String(error)}`,
          { cause: error },
        );
      }
      if (
        this.#managedIdentity !== null
        && described.adapter.adapterId !== this.#managedIdentity.expectedAdapterId
      ) {
        throw new Error(
          `studio_adapter_identity_mismatch: running ${described.adapter.adapterId}, `
          + `configured ${this.#managedIdentity.expectedAdapterId}`,
        );
      }
      return {
        process,
        description: described.adapter,
        launchDigest,
        root,
        bootstrapFingerprint,
      };
    } catch (error) {
      await process.close();
      throw error;
    }
  }
}

interface ReadBootstrapResult {
  readonly manifest: StudioAdapterBootstrapManifest;
  readonly fingerprint: string;
}

export async function readStudioAdapterBootstrap(root: string): Promise<ReadBootstrapResult> {
  const path = resolve(root, STUDIO_ROOT_BOOTSTRAP_FILE);
  let bytes: Buffer;
  try {
    bytes = await readFile(path);
  } catch (error) {
    const code = error !== null && typeof error === 'object' && 'code' in error
      ? (error as { code?: unknown }).code
      : undefined;
    if (code === 'ENOENT') throw new Error(`studio_adapter_bootstrap_missing: ${path}`);
    throw new Error(
      `studio_adapter_bootstrap_unreadable: ${path}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (bytes.byteLength > MAX_BOOTSTRAP_BYTES) {
    throw new Error(`studio_adapter_bootstrap_too_large: ${path}`);
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(bytes.toString('utf8')) as unknown;
  } catch {
    throw new Error(`studio_adapter_bootstrap_malformed_json: ${path}`);
  }
  const manifest = decodeBootstrap(decoded, path);
  return {
    manifest,
    fingerprint: createHash('sha256').update(bytes).digest('hex'),
  };
}

function decodeBootstrap(input: unknown, path: string): StudioAdapterBootstrapManifest {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    throw new Error(`studio_adapter_bootstrap_invalid_shape: ${path}`);
  }
  const value = input as Record<string, unknown>;
  if (Object.keys(value).sort().join(',') !== 'adapter,schemaVersion') {
    throw new Error(`studio_adapter_bootstrap_unknown_fields: ${path}`);
  }
  const adapter = value['adapter'];
  if (value['schemaVersion'] !== 1 || adapter === null || typeof adapter !== 'object' || Array.isArray(adapter)) {
    throw new Error(`studio_adapter_bootstrap_unsupported_schema: ${path}`);
  }
  const fields = adapter as Record<string, unknown>;
  if (Object.keys(fields).sort().join(',') !== 'command,cwd') {
    throw new Error(`studio_adapter_bootstrap_unknown_adapter_fields: ${path}`);
  }
  if (typeof fields['cwd'] !== 'string' || fields['cwd'].length === 0 || fields['cwd'].length > MAX_BOOTSTRAP_STRING_LENGTH) {
    throw new Error(`studio_adapter_bootstrap_invalid_cwd: ${path}`);
  }
  const command = fields['command'];
  if (
    !Array.isArray(command)
    || command.length < 1
    || command.length > MAX_BOOTSTRAP_COMMAND_ARGUMENTS
    || command.some((part) => typeof part !== 'string' || part.length === 0 || part.length > MAX_BOOTSTRAP_STRING_LENGTH)
  ) {
    throw new Error(`studio_adapter_bootstrap_invalid_command: ${path}`);
  }
  return Object.freeze({
    schemaVersion: 1,
    command: Object.freeze([...command] as string[]),
    cwd: fields['cwd'],
  });
}

function validateProjectSelection(root: string, projectFile: string): void {
  if (!isAbsolute(root) || root.includes('\0')) throw new Error('studio_project_root_must_be_absolute');
  if (projectFile.length === 0 || projectFile.includes('\0') || isAbsolute(projectFile)) {
    throw new Error('studio_project_file_must_be_relative');
  }
  const file = resolve(root, projectFile);
  const fromRoot = relative(root, file);
  if (fromRoot.startsWith('..') || isAbsolute(fromRoot)) {
    throw new Error('studio_project_file_outside_root');
  }
}

function parseRequest(line: string): Record<string, unknown> {
  let decoded: unknown;
  try {
    decoded = JSON.parse(line) as unknown;
  } catch {
    throw new Error('Studio adapter request is malformed JSON');
  }
  if (decoded === null || typeof decoded !== 'object' || Array.isArray(decoded)) {
    throw new Error('Studio adapter request must be a JSON object');
  }
  return decoded as Record<string, unknown>;
}
