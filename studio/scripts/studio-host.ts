import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import {
  extname,
  isAbsolute,
  relative,
  resolve,
} from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  MAX_STUDIO_ADAPTER_REQUEST_BYTES,
} from '../libs/adapter-client/src/index.js';
import {
  MAX_STUDIO_USER_SETTINGS_BYTES,
  defaultStudioUserSettingsRoot,
  readStudioUserSettings,
  writeStudioUserSettings,
} from './studio-user-settings-service.js';
import { listStudioHostDirectory } from './studio-host-files-service.js';
import { readStudioRenderResource } from './studio-render-resource-service.js';
import { StudioAdapterHost } from './studio-adapter-host.js';
import { StudioAdapterResponseLimitError } from './studio-adapter-process.js';

const DEFAULT_STATIC_ROOT = fileURLToPath(
  new URL('../dist/apps/studio-app/browser/', import.meta.url),
);
const DEN_PROJECT = 'rusty-engine-studio';

interface HostOptions {
  readonly adapterBinary: string | undefined;
  readonly staticRoot: string;
  readonly host: string;
  readonly port: number;
  readonly settingsRoot: string;
  readonly managedIdentity: ManagedHostIdentity | null;
}

interface ManagedHostIdentity {
  readonly engineSourceCommit: string;
  readonly consumerRepository: string;
  readonly consumerCommit: string;
  readonly adapterBuildCommit: string;
  readonly expectedAdapterId: string;
}

async function readBoundedBody(request: IncomingMessage): Promise<string> {
  const declared = Number(request.headers['content-length'] ?? 0);
  if (Number.isFinite(declared) && declared > MAX_STUDIO_ADAPTER_REQUEST_BYTES) {
    throw new Error('Studio adapter request exceeds the protocol byte bound');
  }
  const chunks: Buffer[] = [];
  let bytes = 0;
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    bytes += buffer.byteLength;
    if (bytes > MAX_STUDIO_ADAPTER_REQUEST_BYTES) {
      throw new Error('Studio adapter request exceeds the protocol byte bound');
    }
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString('utf8');
}

async function readSettingsBody(request: IncomingMessage): Promise<string> {
  const declared = Number(request.headers['content-length'] ?? 0);
  if (Number.isFinite(declared) && declared > MAX_STUDIO_USER_SETTINGS_BYTES * 2) {
    throw new Error('Studio user-settings request exceeds the protocol byte bound');
  }
  const chunks: Buffer[] = [];
  let bytes = 0;
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    bytes += buffer.byteLength;
    if (bytes > MAX_STUDIO_USER_SETTINGS_BYTES * 2) {
      throw new Error('Studio user-settings request exceeds the protocol byte bound');
    }
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString('utf8');
}

async function exchangeUserSettings(
  request: IncomingMessage,
  response: ServerResponse,
  url: URL,
  settingsRoot: string,
): Promise<void> {
  if (request.method === 'GET') {
    const projectRoots = url.searchParams.getAll('projectRoot');
    if (projectRoots.length !== 1) {
      sendError(response, 400, 'Exactly one projectRoot is required');
      return;
    }
    sendJson(response, 200, await readStudioUserSettings({
      projectRoot: projectRoots[0] as string,
      settingsRoot,
    }));
    return;
  }
  if (request.method !== 'PUT') {
    response.writeHead(405, { allow: 'GET, PUT' });
    response.end();
    return;
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(await readSettingsBody(request)) as unknown;
  } catch {
    sendError(response, 400, 'Studio user-settings request is malformed JSON');
    return;
  }
  if (!isRecord(decoded)
    || typeof decoded['projectRoot'] !== 'string'
    || typeof decoded['text'] !== 'string'
    || (decoded['expectedHash'] !== null && typeof decoded['expectedHash'] !== 'string')) {
    sendError(response, 400, 'Studio user-settings request has an invalid shape');
    return;
  }
  const result = await writeStudioUserSettings({
    projectRoot: decoded['projectRoot'],
    text: decoded['text'],
    expectedHash: decoded['expectedHash'],
    settingsRoot,
  });
  sendJson(response, result.ok ? 200 : result.diagnostic === 'stale_user_settings' ? 409 : 400, result);
}

async function exchangeWithAdapter(
  request: IncomingMessage,
  response: ServerResponse,
  adapter: StudioAdapterHost,
): Promise<void> {
  if (request.method !== 'POST') {
    response.writeHead(405, { allow: 'POST' });
    response.end();
    return;
  }
  const body = await readBoundedBody(request);
  let decoded: unknown;
  try {
    decoded = JSON.parse(body) as unknown;
  } catch {
    sendError(response, 400, 'Studio adapter request is malformed JSON');
    return;
  }
  if (decoded === null || typeof decoded !== 'object' || Array.isArray(decoded)) {
    sendError(response, 400, 'Studio adapter request must be a JSON object');
    return;
  }
  const responseLine = await adapter.exchange(JSON.stringify(decoded));
  response.writeHead(200, {
    'cache-control': 'no-store',
    'content-type': 'application/json; charset=utf-8',
    'content-length': String(Buffer.byteLength(responseLine)),
  });
  response.end(responseLine);
}

async function openStudioSession(
  request: IncomingMessage,
  response: ServerResponse,
  host: StudioAdapterHost,
): Promise<void> {
  if (request.method !== 'POST') {
    response.writeHead(405, { allow: 'POST' });
    response.end();
    return;
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(await readBoundedBody(request)) as unknown;
  } catch {
    sendError(response, 400, 'Studio session request is malformed JSON');
    return;
  }
  if (!isRecord(decoded)
    || Object.keys(decoded).sort().join(',') !== 'projectFile,root'
    || typeof decoded['root'] !== 'string'
    || typeof decoded['projectFile'] !== 'string') {
    sendError(response, 400, 'Studio session request has an invalid shape');
    return;
  }
  try {
    const session = await host.openProject(decoded['root'], decoded['projectFile']);
    sendJson(response, 200, {
      schemaVersion: 1,
      type: 'studioSessionOpened',
      adapter: session.adapter,
      project: session.project,
      hostStatus: session.hostStatus,
    });
  } catch (error) {
    sendError(response, 422, error instanceof Error ? error.message : 'Studio session open failed');
  }
}

async function exchangeHostFiles(
  request: IncomingMessage,
  response: ServerResponse,
  url: URL,
): Promise<void> {
  if (request.method !== 'GET') {
    response.writeHead(405, { allow: 'GET' });
    response.end();
    return;
  }
  const directories = url.searchParams.getAll('directory');
  if (directories.length !== 1) {
    sendError(response, 400, 'Exactly one host directory is required');
    return;
  }
  sendJson(response, 200, await listStudioHostDirectory({
    directory: directories[0] as string,
    extensions: url.searchParams.getAll('extension'),
  }));
}

async function exchangeRenderResource(
  request: IncomingMessage,
  response: ServerResponse,
  url: URL,
): Promise<void> {
  if (request.method !== 'GET') {
    response.writeHead(405, { allow: 'GET' });
    response.end();
    return;
  }
  const projectRoots = url.searchParams.getAll('projectRoot');
  const sourcePaths = url.searchParams.getAll('sourcePath');
  const contentHashes = url.searchParams.getAll('contentHash');
  if (projectRoots.length !== 1 || sourcePaths.length !== 1 || contentHashes.length !== 1) {
    sendError(response, 400, 'Exactly one projectRoot, sourcePath, and contentHash are required');
    return;
  }
  const bytes = await readStudioRenderResource({
    projectRoot: projectRoots[0] as string,
    sourcePath: sourcePaths[0] as string,
    contentHash: contentHashes[0] as string,
  });
  response.writeHead(200, {
    'cache-control': 'no-store',
    'content-type': 'application/octet-stream',
    'content-length': String(bytes.byteLength),
  });
  response.end(bytes);
}

async function serveStatic(
  request: IncomingMessage,
  response: ServerResponse,
  staticRoot: string,
  pathname: string,
): Promise<void> {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    response.writeHead(405, { allow: 'GET, HEAD' });
    response.end();
    return;
  }
  let decodedPath: string;
  try {
    decodedPath = decodeURIComponent(pathname);
  } catch {
    sendError(response, 400, 'Malformed static path');
    return;
  }
  if (decodedPath.includes('\0')) {
    sendError(response, 400, 'Malformed static path');
    return;
  }
  const relativePath = decodedPath === '/' ? 'index.html' : decodedPath.replace(/^\/+/, '');
  const file = resolve(staticRoot, relativePath);
  const fromRoot = relative(staticRoot, file);
  if (fromRoot.startsWith('..') || isAbsolute(fromRoot)) {
    sendError(response, 404, 'Not found');
    return;
  }
  let metadata;
  try {
    metadata = await stat(file);
  } catch {
    sendError(response, 404, 'Not found');
    return;
  }
  if (!metadata.isFile()) {
    sendError(response, 404, 'Not found');
    return;
  }
  response.writeHead(200, {
    'content-type': contentType(file),
    'content-length': String(metadata.size),
    'cache-control': relativePath === 'index.html' ? 'no-cache' : 'public, max-age=31536000, immutable',
  });
  if (request.method === 'HEAD') {
    response.end();
    return;
  }
  createReadStream(file).pipe(response);
}

function sendError(response: ServerResponse, status: number, message: string): void {
  sendJson(response, status, { ok: false, message });
}

function sendJson(response: ServerResponse, status: number, value: unknown): void {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    'cache-control': 'no-store',
    'content-type': 'application/json; charset=utf-8',
    'content-length': String(Buffer.byteLength(body)),
  });
  response.end(body);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function contentType(file: string): string {
  switch (extname(file)) {
    case '.css': return 'text/css; charset=utf-8';
    case '.html': return 'text/html; charset=utf-8';
    case '.js': return 'text/javascript; charset=utf-8';
    case '.json': return 'application/json; charset=utf-8';
    case '.svg': return 'image/svg+xml';
    case '.woff2': return 'font/woff2';
    default: return 'application/octet-stream';
  }
}

function argumentValue(name: string, fallback?: string): string {
  const index = process.argv.indexOf(name);
  if (index === -1) {
    if (fallback !== undefined) return fallback;
    throw new Error(`${name} is required`);
  }
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`);
  return value;
}

function options(): HostOptions {
  const adapterBinary = optionalArgumentValue('--adapter-binary');
  const staticRoot = argumentValue('--static-root', DEFAULT_STATIC_ROOT);
  const host = argumentValue('--host', '127.0.0.1');
  const port = Number(argumentValue('--port', '4300'));
  const settingsRoot = argumentValue('--settings-root', defaultStudioUserSettingsRoot());
  const managedIdentity = managedHostIdentity();
  if (adapterBinary !== undefined && !isAbsolute(adapterBinary)) {
    throw new Error('--adapter-binary must be absolute');
  }
  if (!isAbsolute(staticRoot)) throw new Error('--static-root must be absolute');
  if (!isAbsolute(settingsRoot)) throw new Error('--settings-root must be absolute');
  if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
    throw new Error('--port must be an integer from 1 through 65535');
  }
  return { adapterBinary, staticRoot, host, port, settingsRoot, managedIdentity };
}

function managedHostIdentity(): ManagedHostIdentity | null {
  const names = [
    '--engine-source-commit',
    '--consumer-repository',
    '--consumer-commit',
    '--adapter-build-commit',
    '--expected-adapter-id',
  ] as const;
  const values = names.map((name) => optionalArgumentValue(name));
  if (values.every((value) => value === undefined)) return null;
  if (values.some((value) => value === undefined)) {
    throw new Error(`managed Studio identity requires ${names.join(', ')}`);
  }
  return {
    engineSourceCommit: values[0] as string,
    consumerRepository: values[1] as string,
    consumerCommit: values[2] as string,
    adapterBuildCommit: values[3] as string,
    expectedAdapterId: values[4] as string,
  };
}

function optionalArgumentValue(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`);
  return value;
}

async function main(): Promise<void> {
  const configured = options();
  if (configured.adapterBinary === undefined && configured.managedIdentity !== null) {
    throw new Error('managed Studio identity requires --adapter-binary');
  }
  const indexMetadata = await stat(resolve(configured.staticRoot, 'index.html'));
  if (!indexMetadata.isFile()) throw new Error('--static-root must contain index.html');
  const adapter = await StudioAdapterHost.create({
    adapterBinary: configured.adapterBinary,
    managedIdentity: configured.managedIdentity,
  });
  const server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', `http://${configured.host}:${String(configured.port)}`);
      if (url.pathname === '/health') {
        const hostStatus = adapter.status();
        const body = JSON.stringify(hostStatus ?? {
          project: DEN_PROJECT,
          status: 'ready',
          mode: 'generic',
          adapter: null,
        });
        response.writeHead(200, {
          'cache-control': 'no-store',
          'content-type': 'application/json; charset=utf-8',
          'content-length': String(Buffer.byteLength(body)),
          'x-den-project': DEN_PROJECT,
          'x-rusty-engine-source': hostStatus?.engineSourceCommit ?? 'unmanaged',
          'x-studio-consumer': hostStatus?.configuredConsumer?.commit ?? 'unmanaged',
        });
        response.end(body);
        return;
      }
      if (url.pathname === '/api/studio-status') {
        if (request.method !== 'GET') {
          response.writeHead(405, { allow: 'GET' });
          response.end();
          return;
        }
        const hostStatus = adapter.status();
        if (hostStatus === null) {
          sendError(response, 409, 'No project adapter is selected; choose a root with .rusty-studio.json.');
          return;
        }
        sendJson(response, 200, hostStatus);
        return;
      }
      if (url.pathname === '/api/studio-session/open') {
        await openStudioSession(request, response, adapter);
        return;
      }
      if (url.pathname === '/api/studio-adapter') {
        await exchangeWithAdapter(request, response, adapter);
        return;
      }
      if (url.pathname === '/api/studio-user-settings') {
        await exchangeUserSettings(request, response, url, configured.settingsRoot);
        return;
      }
      if (url.pathname === '/api/studio-host-files') {
        await exchangeHostFiles(request, response, url);
        return;
      }
      if (url.pathname === '/api/studio-render-resource') {
        await exchangeRenderResource(request, response, url);
        return;
      }
      await serveStatic(request, response, configured.staticRoot, url.pathname);
    })().catch((error: unknown) => {
      if (!response.headersSent) {
        sendCaughtError(response, error);
      } else {
        response.destroy(error instanceof Error ? error : undefined);
      }
    });
  });
  await new Promise<void>((resolvePromise, rejectPromise) => {
    server.once('error', rejectPromise);
    server.listen(configured.port, configured.host, resolvePromise);
  });
  let shuttingDown = false;
  const shutdown = (): void => {
    if (shuttingDown) return;
    shuttingDown = true;
    server.close();
    void adapter.close();
  };
  process.once('SIGINT', shutdown);
  process.once('SIGTERM', shutdown);
  process.stdout.write(
    `Rusty Engine Studio listening on http://${configured.host}:${String(configured.port)}\n`,
  );
}

function sendCaughtError(response: ServerResponse, error: unknown): void {
  if (error instanceof StudioAdapterResponseLimitError) {
    sendJson(response, 502, {
      ok: false,
      code: error.code,
      message: error.message,
      limitBytes: error.limitBytes,
      actualBytes: error.actualBytes,
    });
    return;
  }
  sendError(response, 502, error instanceof Error ? error.message : 'Studio host failure');
}

await main();
